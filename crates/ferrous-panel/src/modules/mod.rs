pub mod apps;
pub mod cpanel;
pub mod databases;
pub mod files;
pub mod security;

use crate::api::contract::{BackupArchiveItem, ManagedApp, TelemetryHistoryPoint};
use crate::error::PanelError;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct ManagedDbMeta {
    pub id: String,
    pub name: String,
    pub engine: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub collation: String,
    pub linked_app_id: String,
    pub linked_app_name: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct CronRunMeta {
    pub id: String,
    pub last_run_at: String,
    pub last_exit_code: i32,
    pub last_output: String,
}

/// Persistent SQLite metadata store for P5 operational modules (`panel.db`).
///
/// Security guarantee: Database passwords and root credentials are NEVER stored here.
#[derive(Debug)]
pub struct ModuleStore {
    conn: Mutex<Connection>,
}

impl ModuleStore {
    pub fn open(db_path: &Path) -> Result<Self, PanelError> {
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS telemetry_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                recorded_at TEXT NOT NULL,
                time_label TEXT NOT NULL,
                cpu_percent REAL NOT NULL,
                memory_percent REAL NOT NULL,
                memory_used_mb INTEGER NOT NULL,
                requests_per_sec INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS managed_apps (
                id TEXT PRIMARY KEY,
                domain TEXT NOT NULL UNIQUE,
                app_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS managed_databases (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                engine TEXT NOT NULL,
                host TEXT NOT NULL,
                port INTEGER NOT NULL,
                username TEXT NOT NULL,
                collation TEXT NOT NULL,
                linked_app_id TEXT NOT NULL,
                linked_app_name TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS cpanel_cron_meta (
                id TEXT PRIMARY KEY,
                last_run_at TEXT NOT NULL,
                last_exit_code INTEGER NOT NULL,
                last_output TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS cpanel_backups (
                id TEXT PRIMARY KEY,
                filename TEXT NOT NULL UNIQUE,
                target_app TEXT NOT NULL,
                database_included TEXT NOT NULL,
                size_mb REAL NOT NULL,
                compression TEXT NOT NULL,
                sha256 TEXT NOT NULL,
                created_at TEXT NOT NULL,
                status TEXT NOT NULL
            );
            ",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, PanelError> {
        self.conn
            .lock()
            .map_err(|_| PanelError::Internal("module store lock unavailable".to_string()))
    }

    // ---- Telemetry real history ----

    pub fn insert_telemetry_point(&self, point: &TelemetryHistoryPoint) -> Result<(), PanelError> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO telemetry_history (recorded_at, time_label, cpu_percent, memory_percent, memory_used_mb, requests_per_sec)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                Utc::now().to_rfc3339(),
                point.time,
                point.cpu_percent,
                point.memory_percent,
                i64::try_from(point.memory_used_mb).unwrap_or(i64::MAX),
                i64::try_from(point.requests_per_sec.unwrap_or(0)).unwrap_or(i64::MAX),
            ],
        )?;
        tx.execute(
            "DELETE FROM telemetry_history WHERE id NOT IN (SELECT id FROM telemetry_history ORDER BY id DESC LIMIT 288)",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn load_telemetry_history(&self) -> Result<Vec<TelemetryHistoryPoint>, PanelError> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT time_label, cpu_percent, memory_percent, memory_used_mb, requests_per_sec
             FROM telemetry_history ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let mem_used = row.get::<_, i64>(3)?;
            let rps = row.get::<_, i64>(4)?;
            Ok(TelemetryHistoryPoint {
                time: row.get(0)?,
                cpu_percent: row.get(1)?,
                memory_percent: row.get(2)?,
                memory_used_mb: u64::try_from(mem_used.max(0)).unwrap_or(0),
                requests_per_sec: Some(u64::try_from(rps.max(0)).unwrap_or(0)),
            })
        })?;
        let mut out = Vec::new();
        for item in rows {
            out.push(item?);
        }
        Ok(out)
    }

    // ---- Managed Apps ----

    pub fn list_apps(&self) -> Result<Vec<ManagedApp>, PanelError> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("SELECT app_json FROM managed_apps ORDER BY updated_at DESC")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut out = Vec::new();
        for item in rows {
            let raw = item?;
            let app: ManagedApp = serde_json::from_str(&raw)?;
            out.push(app);
        }
        Ok(out)
    }

    pub fn get_app(&self, id: &str) -> Result<Option<ManagedApp>, PanelError> {
        let conn = self.lock_conn()?;
        let raw: Option<String> = conn
            .query_row(
                "SELECT app_json FROM managed_apps WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;
        raw.map(|s| serde_json::from_str(&s).map_err(PanelError::from))
            .transpose()
    }

    pub fn upsert_app(&self, app: &ManagedApp) -> Result<(), PanelError> {
        let raw = serde_json::to_string(app)?;
        let now = Utc::now().to_rfc3339();
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO managed_apps (id, domain, app_json, updated_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET domain = excluded.domain, app_json = excluded.app_json, updated_at = excluded.updated_at",
            params![app.id, app.domain, raw, now],
        )?;
        Ok(())
    }

    pub fn delete_app(&self, id: &str) -> Result<bool, PanelError> {
        let conn = self.lock_conn()?;
        let affected = conn.execute("DELETE FROM managed_apps WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    // ---- Managed Databases (NO plaintext passwords stored) ----

    pub fn list_databases(&self) -> Result<Vec<ManagedDbMeta>, PanelError> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, engine, host, port, username, collation, linked_app_id, linked_app_name, created_at
             FROM managed_databases ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let port_raw = row.get::<_, i64>(4)?;
            Ok(ManagedDbMeta {
                id: row.get(0)?,
                name: row.get(1)?,
                engine: row.get(2)?,
                host: row.get(3)?,
                port: u16::try_from(port_raw).unwrap_or(3306),
                username: row.get(5)?,
                collation: row.get(6)?,
                linked_app_id: row.get(7)?,
                linked_app_name: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_database(&self, id: &str) -> Result<Option<ManagedDbMeta>, PanelError> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT id, name, engine, host, port, username, collation, linked_app_id, linked_app_name, created_at
             FROM managed_databases WHERE id = ?1",
            params![id],
            |row| {
                let port_raw = row.get::<_, i64>(4)?;
                Ok(ManagedDbMeta {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    engine: row.get(2)?,
                    host: row.get(3)?,
                    port: u16::try_from(port_raw).unwrap_or(3306),
                    username: row.get(5)?,
                    collation: row.get(6)?,
                    linked_app_id: row.get(7)?,
                    linked_app_name: row.get(8)?,
                    created_at: row.get(9)?,
                })
            },
        )
        .optional()
        .map_err(PanelError::from)
    }

    pub fn insert_database(&self, meta: &ManagedDbMeta) -> Result<(), PanelError> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO managed_databases (id, name, engine, host, port, username, collation, linked_app_id, linked_app_name, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                meta.id,
                meta.name,
                meta.engine,
                meta.host,
                i64::from(meta.port),
                meta.username,
                meta.collation,
                meta.linked_app_id,
                meta.linked_app_name,
                meta.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn delete_database(&self, id: &str) -> Result<bool, PanelError> {
        let conn = self.lock_conn()?;
        let affected = conn.execute("DELETE FROM managed_databases WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    // ---- Cron Last-Run Metadata ----

    pub fn upsert_cron_run(&self, run: &CronRunMeta) -> Result<(), PanelError> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO cpanel_cron_meta (id, last_run_at, last_exit_code, last_output)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET last_run_at = excluded.last_run_at, last_exit_code = excluded.last_exit_code, last_output = excluded.last_output",
            params![run.id, run.last_run_at, run.last_exit_code, run.last_output],
        )?;
        Ok(())
    }

    pub fn get_cron_run(&self, id: &str) -> Result<Option<CronRunMeta>, PanelError> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT id, last_run_at, last_exit_code, last_output FROM cpanel_cron_meta WHERE id = ?1",
            params![id],
            |row| {
                Ok(CronRunMeta {
                    id: row.get(0)?,
                    last_run_at: row.get(1)?,
                    last_exit_code: row.get(2)?,
                    last_output: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(PanelError::from)
    }

    pub fn delete_cron_run(&self, id: &str) -> Result<(), PanelError> {
        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM cpanel_cron_meta WHERE id = ?1", params![id])?;
        Ok(())
    }

    // ---- Backups Metadata ----

    pub fn insert_backup(&self, item: &BackupArchiveItem) -> Result<(), PanelError> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO cpanel_backups (id, filename, target_app, database_included, size_mb, compression, sha256, created_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                item.id,
                item.filename,
                item.target_app,
                item.database_included,
                item.size_mb,
                item.compression,
                item.sha256,
                item.created_at,
                item.status,
            ],
        )?;
        Ok(())
    }

    pub fn list_backups(&self) -> Result<Vec<BackupArchiveItem>, PanelError> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, filename, target_app, database_included, size_mb, compression, sha256, created_at, status
             FROM cpanel_backups ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(BackupArchiveItem {
                id: row.get(0)?,
                filename: row.get(1)?,
                target_app: row.get(2)?,
                database_included: row.get(3)?,
                size_mb: row.get(4)?,
                compression: row.get(5)?,
                sha256: row.get(6)?,
                created_at: row.get(7)?,
                status: row.get(8)?,
            })
        })?;
        let mut out = Vec::new();
        for item in rows {
            out.push(item?);
        }
        Ok(out)
    }

    pub fn get_backup(&self, id: &str) -> Result<Option<BackupArchiveItem>, PanelError> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT id, filename, target_app, database_included, size_mb, compression, sha256, created_at, status
             FROM cpanel_backups WHERE id = ?1",
            params![id],
            |row| {
                Ok(BackupArchiveItem {
                    id: row.get(0)?,
                    filename: row.get(1)?,
                    target_app: row.get(2)?,
                    database_included: row.get(3)?,
                    size_mb: row.get(4)?,
                    compression: row.get(5)?,
                    sha256: row.get(6)?,
                    created_at: row.get(7)?,
                    status: row.get(8)?,
                })
            },
        )
        .optional()
        .map_err(PanelError::from)
    }

    pub fn delete_backup(&self, id: &str) -> Result<bool, PanelError> {
        let conn = self.lock_conn()?;
        let affected = conn.execute("DELETE FROM cpanel_backups WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }
}
