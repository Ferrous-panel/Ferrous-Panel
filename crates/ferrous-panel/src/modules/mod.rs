pub mod apps;
pub mod capabilities;
pub mod cpanel;
pub mod databases;
pub mod extended;
pub mod files;
pub mod jobs;
pub mod security;
pub mod sysops;

use crate::api::contract::{BackupArchiveItem, ManagedApp, TelemetryHistoryPoint};
use crate::error::PanelError;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

const MAX_PANEL_JOBS: i64 = 200;
const MAX_JOB_LOG_BYTES: usize = 256 * 1024;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelJobRecord {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub status: String,
    pub progress: u8,
    pub output: String,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub finished_at: Option<String>,
}

/// Persistent SQLite metadata store for operational modules (`panel.db`).
///
/// Security guarantee: Database passwords and root credentials are NEVER stored here.
#[derive(Debug)]
pub struct ModuleStore {
    conn: Mutex<Connection>,
    data_dir: PathBuf,
}

impl ModuleStore {
    pub fn open(db_path: &Path) -> Result<Self, PanelError> {
        let data_dir = db_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        if !data_dir.as_os_str().is_empty() && data_dir != Path::new(".") {
            std::fs::create_dir_all(&data_dir)?;
        }
        let conn = Connection::open(db_path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
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
            CREATE TABLE IF NOT EXISTS panel_kv_store (
                key TEXT PRIMARY KEY,
                value_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS panel_jobs (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                title TEXT NOT NULL,
                status TEXT NOT NULL,
                progress INTEGER NOT NULL,
                output TEXT NOT NULL,
                error TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                finished_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_panel_jobs_created_at ON panel_jobs(created_at);
            ",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
            data_dir,
        })
    }

    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    #[must_use]
    pub fn sqlite_databases_dir(&self) -> PathBuf {
        self.data_dir.join("databases")
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

    // ---- Generic Structured State KV Store ----

    pub fn get_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, PanelError> {
        let conn = self.lock_conn()?;
        let raw: Option<String> = conn
            .query_row(
                "SELECT value_json FROM panel_kv_store WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        raw.map(|s| serde_json::from_str(&s).map_err(PanelError::from))
            .transpose()
    }

    pub fn put_json<T: Serialize>(&self, key: &str, value: &T) -> Result<(), PanelError> {
        let raw = serde_json::to_string(value)?;
        let now = Utc::now().to_rfc3339();
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO panel_kv_store (key, value_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![key, raw, now],
        )?;
        Ok(())
    }

    // ---- Background Panel Jobs ----

    pub fn insert_job(&self, job: &PanelJobRecord) -> Result<(), PanelError> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO panel_jobs (id, kind, title, status, progress, output, error, created_at, updated_at, finished_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                job.id,
                job.kind,
                job.title,
                job.status,
                i64::from(job.progress),
                job.output,
                job.error,
                job.created_at,
                job.updated_at,
                job.finished_at,
            ],
        )?;
        let _ = conn.execute(
            "DELETE FROM panel_jobs WHERE id NOT IN (SELECT id FROM panel_jobs ORDER BY created_at DESC LIMIT ?1)",
            params![MAX_PANEL_JOBS],
        );
        Ok(())
    }

    pub fn update_job(
        &self,
        id: &str,
        status: &str,
        progress: u8,
        append_log: Option<&str>,
        error: Option<&str>,
        finished: bool,
    ) -> Result<Option<PanelJobRecord>, PanelError> {
        let now = Utc::now().to_rfc3339();
        let conn = self.lock_conn()?;
        let existing: Option<(String, Option<String>)> = conn
            .query_row(
                "SELECT output, finished_at FROM panel_jobs WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((mut output, existing_finished)) = existing else {
            return Ok(None);
        };

        if let Some(chunk) = append_log {
            if !chunk.is_empty() {
                if !output.is_empty() && !output.ends_with('\n') {
                    output.push('\n');
                }
                output.push_str(chunk);
                if output.len() > MAX_JOB_LOG_BYTES {
                    let trim_start = output.len().saturating_sub(MAX_JOB_LOG_BYTES);
                    let mut boundary = trim_start;
                    while boundary < output.len() && !output.is_char_boundary(boundary) {
                        boundary += 1;
                    }
                    output = output[boundary..].to_string();
                }
            }
        }

        let finished_at = if finished {
            Some(now.clone())
        } else {
            existing_finished
        };

        conn.execute(
            "UPDATE panel_jobs SET status = ?2, progress = ?3, output = ?4, error = ?5, updated_at = ?6, finished_at = ?7
             WHERE id = ?1",
            params![
                id,
                status,
                i64::from(progress.min(100)),
                output,
                error,
                now,
                finished_at,
            ],
        )?;

        conn.query_row(
            "SELECT id, kind, title, status, progress, output, error, created_at, updated_at, finished_at
             FROM panel_jobs WHERE id = ?1",
            params![id],
            |row| {
                let p_raw: i64 = row.get(4)?;
                Ok(PanelJobRecord {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    title: row.get(2)?,
                    status: row.get(3)?,
                    progress: u8::try_from(p_raw.clamp(0, 100)).unwrap_or(0),
                    output: row.get(5)?,
                    error: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    finished_at: row.get(9)?,
                })
            },
        )
        .optional()
        .map_err(PanelError::from)
    }

    pub fn get_job(&self, id: &str) -> Result<Option<PanelJobRecord>, PanelError> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT id, kind, title, status, progress, output, error, created_at, updated_at, finished_at
             FROM panel_jobs WHERE id = ?1",
            params![id],
            |row| {
                let p_raw: i64 = row.get(4)?;
                Ok(PanelJobRecord {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    title: row.get(2)?,
                    status: row.get(3)?,
                    progress: u8::try_from(p_raw.clamp(0, 100)).unwrap_or(0),
                    output: row.get(5)?,
                    error: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    finished_at: row.get(9)?,
                })
            },
        )
        .optional()
        .map_err(PanelError::from)
    }

    pub fn list_jobs(&self, limit: usize) -> Result<Vec<PanelJobRecord>, PanelError> {
        let clamped = i64::try_from(limit.clamp(1, 200)).unwrap_or(50);
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, kind, title, status, progress, output, error, created_at, updated_at, finished_at
             FROM panel_jobs ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![clamped], |row| {
            let p_raw: i64 = row.get(4)?;
            Ok(PanelJobRecord {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                status: row.get(3)?,
                progress: u8::try_from(p_raw.clamp(0, 100)).unwrap_or(0),
                output: row.get(5)?,
                error: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
                finished_at: row.get(9)?,
            })
        })?;
        let mut out = Vec::new();
        for item in rows {
            out.push(item?);
        }
        Ok(out)
    }
}
