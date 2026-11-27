use crate::auth::password::{generate_session_token, RedactedSecret};
use crate::error::PanelError;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;
use subtle::ConstantTimeEq;

const STANDARD_TTL_SECS: i64 = 24 * 60 * 60;
const REMEMBER_ME_TTL_SECS: i64 = 30 * 24 * 60 * 60;
const MAX_AUDIT_LOG_ROWS: i64 = 10_000;

/// Verified session identity loaded from SQLite.
#[derive(Debug, Clone)]
pub struct SessionRecord {
    pub id: String,
    pub username: String,
    pub expires_at: i64,
}

/// Persisted security audit log entry returned by admin audit endpoints.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogEntry {
    pub id: i64,
    pub timestamp: String,
    pub username: Option<String>,
    pub ip: String,
    pub action: String,
    pub detail: String,
}

/// Thread-safe SQLite store for authentication sessions and security audit logs.
#[derive(Debug)]
pub struct AuthStore {
    conn: Mutex<Connection>,
}

impl AuthStore {
    pub fn open(db_path: &Path) -> Result<Self, PanelError> {
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(db_path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                token_hash BLOB NOT NULL UNIQUE,
                username TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                last_active_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                ip TEXT NOT NULL,
                user_agent TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at);
            CREATE TABLE IF NOT EXISTS audit_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                username TEXT,
                ip TEXT NOT NULL,
                action TEXT NOT NULL,
                detail TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_audit_logs_timestamp ON audit_logs(timestamp);
            ",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, PanelError> {
        self.conn
            .lock()
            .map_err(|_| PanelError::Internal("database lock unavailable".to_string()))
    }

    /// Creates a persisted session using a 32-byte `OsRng` token (stored hashed via SHA-256).
    pub fn create_session(
        &self,
        username: &str,
        remember_me: bool,
        ip: &str,
        user_agent: &str,
    ) -> Result<(RedactedSecret, i64), PanelError> {
        let raw_token = generate_session_token()?;
        let session_id = generate_session_token()?;
        let token_digest = Sha256::digest(raw_token.expose().as_bytes());
        let now = Utc::now().timestamp();
        let ttl_secs = if remember_me {
            REMEMBER_ME_TTL_SECS
        } else {
            STANDARD_TTL_SECS
        };
        let expires_at = now + ttl_secs;

        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM sessions WHERE expires_at <= ?1", params![now])?;
        conn.execute(
            "INSERT INTO sessions (id, token_hash, username, created_at, last_active_at, expires_at, ip, user_agent)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                session_id.expose(),
                token_digest.as_slice(),
                username,
                now,
                now,
                expires_at,
                ip,
                user_agent,
            ],
        )?;

        Ok((raw_token, ttl_secs))
    }

    /// Verifies a bearer/cookie token against SQLite non-expired sessions via O(1) unique index
    /// lookup on `token_hash` followed by constant-time digest verification.
    pub fn verify_session(&self, presented_token: &str) -> Result<Option<SessionRecord>, PanelError> {
        let clean = presented_token.trim();
        if clean.is_empty() || clean.len() > 256 {
            return Ok(None);
        }

        let presented_digest = Sha256::digest(clean.as_bytes());
        let now = Utc::now().timestamp();
        let conn = self.lock_conn()?;

        let row_opt: Option<(String, Vec<u8>, String, i64)> = conn
            .query_row(
                "SELECT id, token_hash, username, expires_at FROM sessions WHERE token_hash = ?1 AND expires_at > ?2",
                params![presented_digest.as_slice(), now],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;

        let matched = match row_opt {
            Some((id, hash_bytes, username, expires_at))
                if hash_bytes.len() == presented_digest.len()
                    && bool::from(hash_bytes.ct_eq(presented_digest.as_slice())) =>
            {
                Some(SessionRecord {
                    id,
                    username,
                    expires_at,
                })
            }
            _ => None,
        };

        if let Some(ref record) = matched {
            conn.execute(
                "UPDATE sessions SET last_active_at = ?1 WHERE id = ?2",
                params![now, &record.id],
            )?;
        }

        Ok(matched)
    }

    /// Revokes an active session matching `presented_token`.
    pub fn revoke_session(&self, presented_token: &str) -> Result<bool, PanelError> {
        let Some(record) = self.verify_session(presented_token)? else {
            return Ok(false);
        };
        let conn = self.lock_conn()?;
        let affected = conn.execute("DELETE FROM sessions WHERE id = ?1", params![record.id])?;
        Ok(affected > 0)
    }

    /// Revokes all persisted sessions (used after administrator password rotation).
    pub fn revoke_all_sessions(&self) -> Result<usize, PanelError> {
        let conn = self.lock_conn()?;
        let affected = conn.execute("DELETE FROM sessions", [])?;
        Ok(affected)
    }

    /// Checks whether there is at least one active administrator session recorded for `client_ip`.
    pub fn has_active_session_for_ip(&self, client_ip: &str) -> bool {
        let trimmed = client_ip.trim();
        if trimmed.is_empty() || trimmed == "unknown" {
            return false;
        }
        let Ok(conn) = self.lock_conn() else {
            return false;
        };
        let now = Utc::now().timestamp();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE ip = ?1 AND expires_at > ?2",
                params![trimmed, now],
                |r| r.get(0),
            )
            .unwrap_or(0);
        count > 0
    }

    /// Appends an audit log entry (user, client IP, action, detail) with bounded retention.
    pub fn append_audit_log(
        &self,
        username: Option<&str>,
        ip: &str,
        action: &str,
        detail: &str,
    ) -> Result<(), PanelError> {
        let ts = Utc::now().to_rfc3339();
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO audit_logs (timestamp, username, ip, action, detail) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![ts, username, ip, action, detail],
        )?;
        let _ = conn.execute(
            "DELETE FROM audit_logs WHERE id NOT IN (SELECT id FROM audit_logs ORDER BY id DESC LIMIT ?1)",
            params![MAX_AUDIT_LOG_ROWS],
        );
        Ok(())
    }

    /// Lists newest security audit log entries (clamped to `1..=500`).
    pub fn list_audit_logs(&self, limit: usize) -> Result<Vec<AuditLogEntry>, PanelError> {
        let clamped = i64::try_from(limit.clamp(1, 500)).unwrap_or(100);
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, username, ip, action, detail FROM audit_logs ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![clamped], |row| {
            Ok(AuditLogEntry {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                username: row.get(2)?,
                ip: row.get(3)?,
                action: row.get(4)?,
                detail: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for item in rows {
            out.push(item?);
        }
        Ok(out)
    }

    /// Returns total number of audit log rows recorded in SQLite.
    pub fn audit_log_count(&self) -> Result<i64, PanelError> {
        let conn = self.lock_conn()?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM audit_logs", [], |r| r.get(0))?;
        Ok(count)
    }
}
