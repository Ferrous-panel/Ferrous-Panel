use crate::api::contract::{
    MysqlPhpMyAdminConfig, SqlDatabase, SqlQueryResponse, SqlTableColumn, SqlTableSchema,
};
use crate::error::PanelError;
use crate::modules::{ManagedDbMeta, ModuleStore};
use crate::system::cmd::AllowedBinary;
use crate::system::paths::SafePath;
use crate::system::validate::{
    escape_mysql_literal, escape_postgres_literal, CollationName, SqlIdentifier,
};
use crate::webserver::{atomic_write_file, CommandExecutor};
use chrono::Utc;
use rusqlite::types::ValueRef;
use rusqlite::Connection;
use serde_json::{Number, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static DUMP_SEQ: AtomicU64 = AtomicU64::new(1);

const DB_CMD_TIMEOUT: Duration = Duration::from_secs(20);
const DB_DUMP_TIMEOUT: Duration = Duration::from_secs(180);
const DEFAULT_MARIADB_CNF: &str = "/etc/mysql/mariadb.conf.d/50-server.cnf";
const PREVIEW_ROW_LIMIT: usize = 50;
const MAX_BROWSE_LIMIT: usize = 500;
const MAX_SQL_IMPORT_BYTES: usize = 64 * 1024 * 1024;
const MAX_SQL_EXPORT_BYTES: u64 = 64 * 1024 * 1024;
const FERROUS_ADMIN_KV_KEY: &str = "ferrous_admin_pma_settings";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedPmaSettings {
    pma_path: String,
    pma_port: u16,
    pma_ssl_only: bool,
    pma_basic_auth: bool,
    pma_max_upload_mb: u32,
    pma_allowed_ips: String,
}

impl Default for PersistedPmaSettings {
    fn default() -> Self {
        Self {
            pma_path: "/phpadmin".to_string(),
            pma_port: 888,
            pma_ssl_only: false,
            pma_basic_auth: true,
            pma_max_upload_mb: 64,
            pma_allowed_ips: "127.0.0.1:888 (Pure-Rust Actix-Web · Zero PHP)".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbEngineKind {
    MariaDb,
    Postgres,
    Sqlite,
}

impl DbEngineKind {
    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        let lower = raw.trim().to_ascii_lowercase();
        if lower.contains("sqlite") {
            Ok(Self::Sqlite)
        } else if lower.contains("postgres") || lower == "pgsql" || lower == "pg" {
            Ok(Self::Postgres)
        } else if lower.contains("mariadb") || lower.contains("mysql") {
            Ok(Self::MariaDb)
        } else {
            Err(PanelError::Validation(format!(
                "unsupported SQL engine '{raw}'; allowed: MariaDB 11.4, PostgreSQL 17, SQLite 3 (WAL)"
            )))
        }
    }

    #[must_use]
    pub const fn binary(self) -> Option<AllowedBinary> {
        match self {
            Self::MariaDb => Some(AllowedBinary::Mysql),
            Self::Postgres => Some(AllowedBinary::Psql),
            Self::Sqlite => None,
        }
    }

    #[must_use]
    pub const fn id_prefix(self) -> &'static str {
        match self {
            Self::MariaDb => "mysql",
            Self::Postgres => "psql",
            Self::Sqlite => "sqlite",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::MariaDb => "MariaDB 11.4",
            Self::Postgres => "PostgreSQL 17",
            Self::Sqlite => "SQLite 3 (WAL)",
        }
    }

    #[must_use]
    pub const fn default_port(self) -> u16 {
        match self {
            Self::MariaDb => 3306,
            Self::Postgres => 5432,
            Self::Sqlite => 0,
        }
    }
}

fn ensure_engine_installed(
    executor: &dyn CommandExecutor,
    engine: DbEngineKind,
) -> Result<(), PanelError> {
    if let Some(bin) = engine.binary() {
        if !executor.is_binary_available(bin) {
            return Err(PanelError::Conflict(format!(
                "{} CLI ({}) is not installed on this host (tip: SQLite 3 (WAL) runs 100% natively in Rust without installing external packages)",
                engine.label(),
                bin.label()
            )));
        }
    }
    Ok(())
}

/// On production VPS hosts (`/etc/ferrous-panel` present), automatically installs and starts
/// `mariadb-server` or `postgresql` if the administrator creates a MariaDB or PostgreSQL database
/// before the daemon package has been installed or started.
async fn ensure_sql_engine_ready(
    executor: &dyn CommandExecutor,
    engine: DbEngineKind,
) -> Result<(), PanelError> {
    let Some(bin) = engine.binary() else {
        return Ok(());
    };
    let is_prod_host = Path::new("/etc/ferrous-panel").is_dir();
    if !executor.is_binary_available(bin) && is_prod_host && executor.is_binary_available(AllowedBinary::AptGet) {
        let env = [("DEBIAN_FRONTEND", "noninteractive")];
        let install_timeout = Duration::from_secs(240);
        let _ = executor
            .run_with_env(AllowedBinary::AptGet, &["update", "-qq"], &env, None, install_timeout)
            .await;
        match engine {
            DbEngineKind::MariaDb => {
                let _ = executor
                    .run_with_env(
                        AllowedBinary::AptGet,
                        &["install", "-y", "-qq", "mariadb-server", "mariadb-client"],
                        &env,
                        None,
                        install_timeout,
                    )
                    .await;
            }
            DbEngineKind::Postgres => {
                let _ = executor
                    .run_with_env(
                        AllowedBinary::AptGet,
                        &["install", "-y", "-qq", "postgresql", "postgresql-client"],
                        &env,
                        None,
                        install_timeout,
                    )
                    .await;
            }
            DbEngineKind::Sqlite => {}
        }
    }
    ensure_engine_installed(executor, engine)?;
    if is_prod_host && executor.is_binary_available(AllowedBinary::Systemctl) {
        let svc = match engine {
            DbEngineKind::MariaDb => "mariadb",
            DbEngineKind::Postgres => "postgresql",
            DbEngineKind::Sqlite => return Ok(()),
        };
        let active = executor
            .run(AllowedBinary::Systemctl, &["is-active", "--quiet", svc], DB_CMD_TIMEOUT)
            .await
            .map(|o| o.success)
            .unwrap_or(false);
        if !active {
            let _ = executor
                .run(AllowedBinary::Systemctl, &["enable", "--now", svc], DB_CMD_TIMEOUT)
                .await;
        }
    }
    Ok(())
}

fn generate_random_db_password() -> String {
    use rand::rngs::OsRng;
    use rand::RngCore;
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut raw = [0u8; 20];
    if OsRng.try_fill_bytes(&mut raw).is_err() {
        return "FpDbPass9x2Km7Qv4Lw1".to_string();
    }
    raw.iter()
        .map(|&b| ALPHABET[(b as usize) % ALPHABET.len()] as char)
        .collect()
}

fn should_use_runuser_postgres(executor: &dyn CommandExecutor) -> bool {
    Path::new("/etc/postgresql").is_dir() && executor.is_binary_available(AllowedBinary::Runuser)
}

fn validate_db_password(raw: &str) -> Result<&str, PanelError> {
    if raw.is_empty() || raw.len() > 128 {
        return Err(PanelError::Validation(
            "database user password must be 1..=128 characters".to_string(),
        ));
    }
    if raw.contains('\0') || raw.contains('\n') || raw.contains('\r') {
        return Err(PanelError::Forbidden(
            "database password contains forbidden control characters".to_string(),
        ));
    }
    Ok(raw)
}

fn sqlite_db_path(store: &ModuleStore, db_ident: &SqlIdentifier) -> Result<PathBuf, PanelError> {
    let dir = store.sqlite_databases_dir();
    std::fs::create_dir_all(&dir)?;
    let filename = format!("{}.db", db_ident.as_str());
    let safe = SafePath::under_root(&dir, &filename)?;
    Ok(safe.as_path().to_path_buf())
}

fn open_sqlite_db(store: &ModuleStore, db_ident: &SqlIdentifier) -> Result<Connection, PanelError> {
    let path = sqlite_db_path(store, db_ident)?;
    let conn = Connection::open(&path)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;",
    )?;
    Ok(conn)
}

fn sqlite_value_to_json(val: ValueRef<'_>) -> Value {
    match val {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => Value::Number(Number::from(i)),
        ValueRef::Real(f) => Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(f.to_string())),
        ValueRef::Text(bytes) => Value::String(String::from_utf8_lossy(bytes).into_owned()),
        ValueRef::Blob(bytes) => {
            let mut hex = String::with_capacity(2 + bytes.len() * 2);
            hex.push_str("0x");
            for b in bytes.iter().take(64) {
                hex.push_str(&format!("{b:02x}"));
            }
            if bytes.len() > 64 {
                hex.push_str("...");
            }
            Value::String(hex)
        }
    }
}

fn sqlite_value_to_sql_literal(val: ValueRef<'_>) -> String {
    match val {
        ValueRef::Null => "NULL".to_string(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => {
            if f.is_finite() {
                f.to_string()
            } else {
                "NULL".to_string()
            }
        }
        ValueRef::Text(bytes) => {
            let text = String::from_utf8_lossy(bytes);
            format!("'{}'", text.replace('\'', "''"))
        }
        ValueRef::Blob(bytes) => {
            let mut hex = String::with_capacity(3 + bytes.len() * 2);
            hex.push_str("X'");
            for b in bytes {
                hex.push_str(&format!("{b:02x}"));
            }
            hex.push('\'');
            hex
        }
    }
}

type SqliteQueryRows = (Vec<String>, Vec<BTreeMap<String, Value>>);

fn sqlite_query_rows(
    conn: &Connection,
    sql: &str,
    row_cap: usize,
) -> Result<SqliteQueryRows, PanelError> {
    let mut stmt = conn.prepare(sql)?;
    let columns: Vec<String> = stmt
        .column_names()
        .into_iter()
        .map(ToString::to_string)
        .collect();
    let col_count = columns.len();
    let mut raw_rows = stmt.query([])?;
    let mut out_rows = Vec::new();
    while let Some(row) = raw_rows.next()? {
        let mut map = BTreeMap::new();
        for idx in 0..col_count {
            if let Some(col_name) = columns.get(idx) {
                let cell = row.get_ref(idx)?;
                map.insert(col_name.clone(), sqlite_value_to_json(cell));
            }
        }
        out_rows.push(map);
        if out_rows.len() >= row_cap {
            break;
        }
    }
    Ok((columns, out_rows))
}

fn inspect_sqlite_db(
    store: &ModuleStore,
    db_ident: &SqlIdentifier,
) -> (f64, Vec<SqlTableSchema>, String) {
    let Ok(path) = sqlite_db_path(store, db_ident) else {
        return (0.0, Vec::new(), "error".to_string());
    };
    if !path.is_file() {
        return (0.0, Vec::new(), "offline".to_string());
    }
    let main_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let wal_path = PathBuf::from(format!("{}-wal", path.display()));
    let wal_bytes = std::fs::metadata(&wal_path).map(|m| m.len()).unwrap_or(0);
    let total_bytes = main_bytes.saturating_add(wal_bytes);
    let size_mb = ((total_bytes as f64 / 1_048_576.0) * 100.0).round() / 100.0;

    let Ok(conn) = open_sqlite_db(store, db_ident) else {
        return (size_mb, Vec::new(), "offline".to_string());
    };

    let mut table_names: Vec<String> = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name LIMIT 100",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for item in rows.flatten() {
                table_names.push(item);
            }
        }
    }

    let mut tables = Vec::with_capacity(table_names.len());
    for tname in table_names {
        let Ok(t_ident) = SqlIdentifier::parse(&tname) else {
            continue;
        };
        let qtable = t_ident.quote_postgres();

        let mut columns = Vec::new();
        let pragma_sql = format!("PRAGMA table_info({qtable});");
        if let Ok(mut cstmt) = conn.prepare(&pragma_sql) {
            if let Ok(crows) = cstmt.query_map([], |row| {
                let col_name: String = row.get(1)?;
                let col_type: String = row.get(2)?;
                let notnull: i64 = row.get(3)?;
                let pk: i64 = row.get(5)?;
                Ok(SqlTableColumn {
                    name: col_name,
                    data_type: if col_type.trim().is_empty() {
                        "TEXT".to_string()
                    } else {
                        col_type
                    },
                    nullable: notnull == 0,
                    is_primary_key: Some(pk > 0),
                })
            }) {
                for c in crows.flatten() {
                    columns.push(c);
                }
            }
        }

        let count_sql = format!("SELECT COUNT(*) FROM {qtable};");
        let row_count = conn
            .query_row(&count_sql, [], |row| row.get::<_, i64>(0))
            .ok()
            .and_then(|v| u64::try_from(v.max(0)).ok())
            .unwrap_or(0);

        let select_sql = format!("SELECT * FROM {qtable} LIMIT {PREVIEW_ROW_LIMIT};");
        let preview_rows = sqlite_query_rows(&conn, &select_sql, PREVIEW_ROW_LIMIT)
            .map(|(_, rows)| rows)
            .unwrap_or_default();

        // Approximate per-table size in KB proportional to file size or row count.
        let est_kb = (((row_count.max(1) as f64) * 0.25) * 100.0).round() / 100.0;
        tables.push(SqlTableSchema {
            table_name: tname,
            row_count,
            size_kb: est_kb,
            columns,
            rows: preview_rows,
        });
    }

    (size_mb, tables, "online".to_string())
}

async fn run_mysql(
    executor: &dyn CommandExecutor,
    db: Option<&str>,
    sql: &str,
) -> Result<crate::system::cmd::CmdOutput, PanelError> {
    ensure_engine_installed(executor, DbEngineKind::MariaDb)?;
    let mut args = vec!["--batch", "--raw"];
    let db_owned;
    if let Some(d) = db {
        db_owned = SqlIdentifier::parse(d)?;
        args.push("-D");
        args.push(db_owned.as_str());
    }
    args.push("-e");
    args.push(sql);
    let out = executor
        .run_with_env(AllowedBinary::Mysql, &args, &[], None, DB_CMD_TIMEOUT)
        .await?;
    if !out.success {
        return Err(PanelError::CommandFailed {
            binary: "mysql",
            exit_code: out.exit_code,
            stderr: out.stderr,
        });
    }
    Ok(out)
}

async fn run_psql_internal(
    executor: &dyn CommandExecutor,
    db: Option<&str>,
    sql: &str,
    include_headers: bool,
) -> Result<crate::system::cmd::CmdOutput, PanelError> {
    ensure_engine_installed(executor, DbEngineKind::Postgres)?;
    let db_ident;
    let mut args = if include_headers {
        vec![
            "-v",
            "ON_ERROR_STOP=1",
            "-A",
            "-F",
            "\t",
            "--pset=footer=off",
        ]
    } else {
        vec!["-v", "ON_ERROR_STOP=1", "-At", "-F", "\t"]
    };
    if let Some(d) = db {
        db_ident = SqlIdentifier::parse(d)?;
        args.push("-d");
        args.push(db_ident.as_str());
    } else {
        args.push("-d");
        args.push("postgres");
    }
    args.push("-c");
    args.push(sql);
    let out = if should_use_runuser_postgres(executor) {
        let mut ru_args = Vec::with_capacity(args.len() + 4);
        ru_args.extend_from_slice(&["-u", "postgres", "--", "psql"]);
        ru_args.extend_from_slice(&args);
        executor
            .run_with_env(AllowedBinary::Runuser, &ru_args, &[], None, DB_CMD_TIMEOUT)
            .await?
    } else {
        executor
            .run_with_env(AllowedBinary::Psql, &args, &[], None, DB_CMD_TIMEOUT)
            .await?
    };
    if !out.success {
        return Err(PanelError::CommandFailed {
            binary: "psql",
            exit_code: out.exit_code,
            stderr: out.stderr,
        });
    }
    Ok(out)
}

async fn run_mysql_stdin(
    executor: &dyn CommandExecutor,
    db: Option<&str>,
    sql: &str,
) -> Result<crate::system::cmd::CmdOutput, PanelError> {
    ensure_engine_installed(executor, DbEngineKind::MariaDb)?;
    let mut args = vec!["--batch", "--raw"];
    let db_owned;
    if let Some(d) = db {
        db_owned = SqlIdentifier::parse(d)?;
        args.push("-D");
        args.push(db_owned.as_str());
    }
    let out = executor
        .run_with_stdin(AllowedBinary::Mysql, &args, sql.as_bytes(), DB_CMD_TIMEOUT)
        .await?;
    if !out.success {
        return Err(PanelError::CommandFailed {
            binary: "mysql",
            exit_code: out.exit_code,
            stderr: out.stderr,
        });
    }
    Ok(out)
}

async fn run_psql_stdin(
    executor: &dyn CommandExecutor,
    db: Option<&str>,
    sql: &str,
) -> Result<crate::system::cmd::CmdOutput, PanelError> {
    ensure_engine_installed(executor, DbEngineKind::Postgres)?;
    let db_ident;
    let mut args = vec!["-v", "ON_ERROR_STOP=1", "-At", "-F", "\t"];
    if let Some(d) = db {
        db_ident = SqlIdentifier::parse(d)?;
        args.push("-d");
        args.push(db_ident.as_str());
    } else {
        args.push("-d");
        args.push("postgres");
    }
    let out = if should_use_runuser_postgres(executor) {
        let mut ru_args = Vec::with_capacity(args.len() + 4);
        ru_args.extend_from_slice(&["-u", "postgres", "--", "psql"]);
        ru_args.extend_from_slice(&args);
        executor
            .run_with_stdin(AllowedBinary::Runuser, &ru_args, sql.as_bytes(), DB_CMD_TIMEOUT)
            .await?
    } else {
        executor
            .run_with_stdin(AllowedBinary::Psql, &args, sql.as_bytes(), DB_CMD_TIMEOUT)
            .await?
    };
    if !out.success {
        return Err(PanelError::CommandFailed {
            binary: "psql",
            exit_code: out.exit_code,
            stderr: out.stderr,
        });
    }
    Ok(out)
}

async fn run_psql(
    executor: &dyn CommandExecutor,
    db: Option<&str>,
    sql: &str,
) -> Result<crate::system::cmd::CmdOutput, PanelError> {
    run_psql_internal(executor, db, sql, false).await
}

fn parse_tsv_with_header(stdout: &str) -> (Vec<String>, Vec<BTreeMap<String, Value>>) {
    let mut lines = stdout.lines();
    let Some(header_line) = lines.next() else {
        return (Vec::new(), Vec::new());
    };
    if header_line.trim().is_empty() {
        return (Vec::new(), Vec::new());
    }
    let columns: Vec<String> = header_line.split('\t').map(ToString::to_string).collect();
    let mut rows = Vec::new();
    for line in lines {
        if line.is_empty() && columns.len() > 1 {
            continue;
        }
        let vals: Vec<&str> = line.split('\t').collect();
        let mut map = BTreeMap::new();
        for (idx, col) in columns.iter().enumerate() {
            let val = vals.get(idx).copied().unwrap_or("");
            if val == "NULL" || val == "\\N" {
                map.insert(col.clone(), Value::Null);
            } else {
                map.insert(col.clone(), Value::String(val.to_string()));
            }
        }
        rows.push(map);
    }
    (columns, rows)
}

async fn query_db_size_and_tables(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
    engine: DbEngineKind,
    db_name: &SqlIdentifier,
) -> (f64, Vec<SqlTableSchema>, String) {
    match engine {
        DbEngineKind::Sqlite => inspect_sqlite_db(store, db_name),
        DbEngineKind::MariaDb => {
            let db_lit = match escape_mysql_literal(db_name.as_str()) {
                Ok(v) => v,
                Err(_) => return (0.0, Vec::new(), "error".to_string()),
            };
            let size_sql = format!(
                "SELECT ROUND(COALESCE(SUM(data_length + index_length), 0) / 1048576.0, 2) FROM information_schema.TABLES WHERE table_schema = {db_lit};"
            );
            let Ok(size_out) = run_mysql(executor, None, &size_sql).await else {
                return (0.0, Vec::new(), "offline".to_string());
            };
            let size_mb = size_out
                .stdout
                .lines()
                .nth(1)
                .and_then(|s| s.trim().parse::<f64>().ok())
                .unwrap_or(0.0);

            let tbl_sql = format!(
                "SELECT table_name, COALESCE(table_rows, 0), ROUND(COALESCE(data_length + index_length, 0) / 1024.0, 2) FROM information_schema.TABLES WHERE table_schema = {db_lit} ORDER BY table_name LIMIT 50;"
            );
            let mut tables = Vec::new();
            if let Ok(tbl_out) = run_mysql(executor, None, &tbl_sql).await {
                for line in tbl_out.stdout.lines().skip(1) {
                    let cols: Vec<&str> = line.split('\t').collect();
                    if cols.len() >= 3 {
                        let tname = cols[0].trim().to_string();
                        let row_count = cols[1].trim().parse::<u64>().unwrap_or(0);
                        let size_kb = cols[2].trim().parse::<f64>().unwrap_or(0.0);
                        let mut table_cols = Vec::new();
                        if let Ok(tlit) = escape_mysql_literal(&tname) {
                            let col_sql = format!(
                                "SELECT column_name, column_type, is_nullable, column_key FROM information_schema.COLUMNS WHERE table_schema = {db_lit} AND table_name = {tlit} ORDER BY ordinal_position LIMIT 64;"
                            );
                            if let Ok(col_out) = run_mysql(executor, None, &col_sql).await {
                                for cline in col_out.stdout.lines().skip(1) {
                                    let parts: Vec<&str> = cline.split('\t').collect();
                                    if parts.len() >= 4 {
                                        table_cols.push(SqlTableColumn {
                                            name: parts[0].trim().to_string(),
                                            data_type: parts[1].trim().to_string(),
                                            nullable: parts[2].trim().eq_ignore_ascii_case("YES"),
                                            is_primary_key: Some(parts[3].trim() == "PRI"),
                                        });
                                    }
                                }
                            }
                        }
                        let mut preview_rows = Vec::new();
                        if let Ok(t_ident) = SqlIdentifier::parse(&tname) {
                            let sample_sql = format!(
                                "SELECT * FROM {} LIMIT {PREVIEW_ROW_LIMIT};",
                                t_ident.quote_mysql()
                            );
                            if let Ok(row_out) =
                                run_mysql(executor, Some(db_name.as_str()), &sample_sql).await
                            {
                                let (_, parsed) = parse_tsv_with_header(&row_out.stdout);
                                preview_rows = parsed;
                            }
                        }
                        tables.push(SqlTableSchema {
                            table_name: tname,
                            row_count,
                            size_kb,
                            columns: table_cols,
                            rows: preview_rows,
                        });
                    }
                }
            }
            (size_mb, tables, "online".to_string())
        }
        DbEngineKind::Postgres => {
            let db_lit = match escape_postgres_literal(db_name.as_str()) {
                Ok(v) => v,
                Err(_) => return (0.0, Vec::new(), "error".to_string()),
            };
            let size_sql =
                format!("SELECT ROUND(pg_database_size({db_lit})::numeric / 1048576.0, 2);");
            let Ok(size_out) = run_psql(executor, None, &size_sql).await else {
                return (0.0, Vec::new(), "offline".to_string());
            };
            let size_mb = size_out
                .stdout
                .lines()
                .next()
                .and_then(|s| s.trim().parse::<f64>().ok())
                .unwrap_or(0.0);

            let tbl_sql = "SELECT c.relname, COALESCE(s.n_live_tup, 0), ROUND(pg_total_relation_size(c.oid)::numeric / 1024.0, 2) FROM pg_class c LEFT JOIN pg_stat_user_tables s ON s.relid = c.oid WHERE c.relkind = 'r' AND c.relnamespace = (SELECT oid FROM pg_namespace WHERE nspname = 'public') ORDER BY c.relname LIMIT 50;";
            let mut tables = Vec::new();
            if let Ok(tbl_out) = run_psql(executor, Some(db_name.as_str()), tbl_sql).await {
                for line in tbl_out.stdout.lines() {
                    let cols: Vec<&str> = line.split('\t').collect();
                    if cols.len() >= 3 {
                        let tname = cols[0].trim().to_string();
                        let row_count = cols[1].trim().parse::<u64>().unwrap_or(0);
                        let size_kb = cols[2].trim().parse::<f64>().unwrap_or(0.0);
                        let mut table_cols = Vec::new();
                        if let Ok(tlit) = escape_postgres_literal(&tname) {
                            let col_sql = format!(
                                "SELECT c.column_name, c.data_type, c.is_nullable, \
                                 CASE WHEN tc.constraint_type = 'PRIMARY KEY' THEN 'PRI' ELSE '' END \
                                 FROM information_schema.columns c \
                                 LEFT JOIN information_schema.key_column_usage kcu \
                                   ON c.table_schema = kcu.table_schema AND c.table_name = kcu.table_name AND c.column_name = kcu.column_name \
                                 LEFT JOIN information_schema.table_constraints tc \
                                   ON kcu.constraint_name = tc.constraint_name AND tc.constraint_type = 'PRIMARY KEY' \
                                 WHERE c.table_schema = 'public' AND c.table_name = {tlit} \
                                 ORDER BY c.ordinal_position LIMIT 64;"
                            );
                            if let Ok(col_out) =
                                run_psql(executor, Some(db_name.as_str()), &col_sql).await
                            {
                                for cline in col_out.stdout.lines() {
                                    let parts: Vec<&str> = cline.split('\t').collect();
                                    if parts.len() >= 4 {
                                        table_cols.push(SqlTableColumn {
                                            name: parts[0].trim().to_string(),
                                            data_type: parts[1].trim().to_string(),
                                            nullable: parts[2].trim().eq_ignore_ascii_case("YES"),
                                            is_primary_key: Some(parts[3].trim() == "PRI"),
                                        });
                                    }
                                }
                            }
                        }
                        let mut preview_rows = Vec::new();
                        if let Ok(t_ident) = SqlIdentifier::parse(&tname) {
                            let sample_sql = format!(
                                "SELECT * FROM {} LIMIT {PREVIEW_ROW_LIMIT};",
                                t_ident.quote_postgres()
                            );
                            if let Ok(row_out) = run_psql_internal(
                                executor,
                                Some(db_name.as_str()),
                                &sample_sql,
                                true,
                            )
                            .await
                            {
                                let (_, parsed) = parse_tsv_with_header(&row_out.stdout);
                                preview_rows = parsed;
                            }
                        }
                        tables.push(SqlTableSchema {
                            table_name: tname,
                            row_count,
                            size_kb,
                            columns: table_cols,
                            rows: preview_rows,
                        });
                    }
                }
            }
            (size_mb, tables, "online".to_string())
        }
    }
}

async fn query_db_connections(
    executor: &dyn CommandExecutor,
    engine: DbEngineKind,
    db_name: &SqlIdentifier,
    status: &str,
) -> (u32, u32) {
    if status != "online" {
        return (0, 0);
    }
    match engine {
        DbEngineKind::Sqlite => (1, 1),
        DbEngineKind::MariaDb => {
            let sql = "SELECT (SELECT VARIABLE_VALUE FROM information_schema.GLOBAL_STATUS WHERE VARIABLE_NAME = 'Threads_connected'), @@max_connections;";
            if let Ok(out) = run_mysql(executor, None, sql).await {
                let data_line = out.stdout.lines().nth(1).or_else(|| out.stdout.lines().next());
                if let Some(line) = data_line {
                    let cols: Vec<&str> = line.split('\t').collect();
                    if cols.len() >= 2 {
                        let active = cols[0].trim().parse::<u32>().unwrap_or(0);
                        let max_conn = cols[1].trim().parse::<u32>().unwrap_or(151);
                        return (active, max_conn);
                    }
                }
            }
            (0, 151)
        }
        DbEngineKind::Postgres => {
            let Ok(db_lit) = escape_postgres_literal(db_name.as_str()) else {
                return (0, 100);
            };
            let sql = format!(
                "SELECT (SELECT count(*) FROM pg_stat_activity WHERE datname = {db_lit}), current_setting('max_connections');"
            );
            if let Ok(out) = run_psql(executor, None, &sql).await {
                if let Some(line) = out.stdout.lines().next() {
                    let cols: Vec<&str> = line.split('\t').collect();
                    if cols.len() >= 2 {
                        let active = cols[0].trim().parse::<u32>().unwrap_or(0);
                        let max_conn = cols[1].trim().parse::<u32>().unwrap_or(100);
                        return (active, max_conn);
                    }
                }
            }
            (0, 100)
        }
    }
}

const MYPHPADMIN_SSO_USER: &str = "fp_myphpadmin";
pub const MYPHPADMIN_SSO_PASS_FILE: &str = "/etc/ferrous-panel/myphpadmin.sso.key";

fn is_system_mysql_schema(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "information_schema" | "performance_schema" | "mysql" | "sys" | "phpmyadmin" | "test"
    )
}

/// Ensures a local-only MariaDB superuser (`fp_myphpadmin@localhost` / `127.0.0.1`) exists
/// and stores its random 32-char credential in `/etc/ferrous-panel/myphpadmin.sso.key` (mode `0600`)
/// so `Myphpadmin` automatically logs into the Panel's MariaDB instance when the administrator is
/// authenticated to FerrousPanel (or falls back to standard cookie login otherwise).
pub async fn ensure_myphpadmin_sso_mysql_account(
    executor: &dyn CommandExecutor,
) -> Option<(String, String)> {
    if !executor.is_binary_available(AllowedBinary::Mysql) {
        return None;
    }
    let key_path = Path::new(MYPHPADMIN_SSO_PASS_FILE);
    let existing_pw = std::fs::read_to_string(key_path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() >= 16 && !s.contains('\'') && !s.contains('\\'));

    let sso_pw = existing_pw.unwrap_or_else(|| {
        use rand::rngs::OsRng;
        use rand::RngCore;
        const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let mut raw = [0u8; 32];
        if OsRng.try_fill_bytes(&mut raw).is_err() {
            return String::new();
        }
        let generated: String = raw
            .iter()
            .map(|&b| ALPHABET[(b as usize) % ALPHABET.len()] as char)
            .collect();
        if let Some(parent) = key_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = atomic_write_file(key_path, generated.as_bytes(), 0o600);
        generated
    });

    if sso_pw.is_empty() {
        return None;
    }

    let Ok(quser) = escape_mysql_literal(MYPHPADMIN_SSO_USER) else {
        return None;
    };
    let Ok(qpw) = escape_mysql_literal(&sso_pw) else {
        return None;
    };
    let grant_sql = format!(
        "CREATE USER IF NOT EXISTS {quser}@'localhost' IDENTIFIED BY {qpw}; \
         CREATE USER IF NOT EXISTS {quser}@'127.0.0.1' IDENTIFIED BY {qpw}; \
         ALTER USER {quser}@'localhost' IDENTIFIED BY {qpw}; \
         ALTER USER {quser}@'127.0.0.1' IDENTIFIED BY {qpw}; \
         GRANT ALL PRIVILEGES ON *.* TO {quser}@'localhost' WITH GRANT OPTION; \
         GRANT ALL PRIVILEGES ON *.* TO {quser}@'127.0.0.1' WITH GRANT OPTION; \
         FLUSH PRIVILEGES;"
    );
    if run_mysql_stdin(executor, None, &grant_sql).await.is_ok() {
        Some((MYPHPADMIN_SSO_USER.to_string(), sso_pw))
    } else {
        None
    }
}

/// Automatically discovers user-created MariaDB schemas on the host (e.g. databases created inside
/// `Myphpadmin` `/phpmyadmin/` or `wp-cli`) and registers them into `ModuleStore` so the Panel
/// SQL list, `FerrousAdmin` (`/phpadmin/`), and `Myphpadmin` (`/phpmyadmin/`) stay 100% synchronized.
async fn reconcile_live_mariadb_schemas_into_store(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
) {
    if !executor.is_binary_available(AllowedBinary::Mysql) {
        return;
    }
    let sql = "SELECT SCHEMA_NAME, DEFAULT_COLLATION_NAME FROM information_schema.SCHEMATA ORDER BY SCHEMA_NAME LIMIT 100;";
    let Ok(out) = run_mysql(executor, None, sql).await else {
        return;
    };
    let existing = store.list_databases().unwrap_or_default();
    for line in out.stdout.lines().skip(1) {
        let mut parts = line.split('\t');
        let Some(raw_name) = parts.next().map(str::trim) else {
            continue;
        };
        if raw_name.is_empty() || is_system_mysql_schema(raw_name) {
            continue;
        }
        let Ok(db_ident) = SqlIdentifier::parse(raw_name) else {
            continue;
        };
        let already_tracked = existing
            .iter()
            .any(|m| m.name.eq_ignore_ascii_case(db_ident.as_str()));
        if already_tracked {
            continue;
        }
        let raw_collation = parts.next().map(str::trim).unwrap_or("utf8mb4_unicode_ci");
        let collation = CollationName::parse(raw_collation)
            .map(|c| c.as_str().to_string())
            .unwrap_or_else(|_| "utf8mb4_unicode_ci".to_string());
        let meta = ManagedDbMeta {
            id: format!("db-{}-{}", DbEngineKind::MariaDb.id_prefix(), db_ident.as_str()),
            name: db_ident.as_str().to_string(),
            engine: DbEngineKind::MariaDb.label().to_string(),
            host: "127.0.0.1".to_string(),
            port: DbEngineKind::MariaDb.default_port(),
            username: "root".to_string(),
            collation,
            linked_app_id: String::new(),
            linked_app_name: String::new(),
            created_at: Utc::now().to_rfc3339(),
        };
        let _ = store.insert_database(&meta);
    }
}

pub async fn list_databases(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
) -> Result<Vec<SqlDatabase>, PanelError> {
    reconcile_live_mariadb_schemas_into_store(store, executor).await;
    let metas = store.list_databases()?;
    let mut out = Vec::with_capacity(metas.len());
    for meta in metas {
        let engine_kind = DbEngineKind::parse(&meta.engine)?;
        let db_ident = SqlIdentifier::parse(&meta.name)?;
        let engine_ready = match engine_kind.binary() {
            Some(bin) => executor.is_binary_available(bin),
            None => true, // SQLite 3 (WAL) is embedded natively in Rust!
        };
        let (size_mb, tables, status) = if engine_ready {
            query_db_size_and_tables(store, executor, engine_kind, &db_ident).await
        } else {
            (0.0, Vec::new(), "engine_not_installed".to_string())
        };
        let (active_connections, max_connections) =
            query_db_connections(executor, engine_kind, &db_ident, &status).await;
        out.push(SqlDatabase {
            id: meta.id,
            name: meta.name,
            engine: meta.engine,
            host: meta.host,
            port: meta.port,
            username: meta.username,
            // NEVER store or expose database passwords in SQLite or API responses.
            password: None,
            collation: meta.collation,
            linked_app_id: meta.linked_app_id,
            linked_app_name: meta.linked_app_name,
            size_mb,
            active_connections,
            max_connections,
            status,
            created_at: meta.created_at,
            tables,
        });
    }
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
pub async fn create_database(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
    name_raw: &str,
    engine_raw: &str,
    username_raw: &str,
    password_raw: Option<&str>,
    collation_raw: &str,
    linked_app_id: &str,
) -> Result<Vec<SqlDatabase>, PanelError> {
    let engine = DbEngineKind::parse(engine_raw)?;
    ensure_sql_engine_ready(executor, engine).await?;

    let db_ident = SqlIdentifier::parse(name_raw)?;
    let normalized_user = if username_raw.trim().is_empty() {
        format!("{}_usr", db_ident.as_str())
    } else {
        username_raw.trim().to_string()
    };
    let user_ident = SqlIdentifier::parse(&normalized_user)?;
    let trimmed_col = collation_raw.trim();
    let effective_collation_str = match engine {
        DbEngineKind::MariaDb => {
            if trimmed_col.is_empty()
                || trimmed_col.eq_ignore_ascii_case("UTF-8")
                || trimmed_col.eq_ignore_ascii_case("utf8")
                || trimmed_col.eq_ignore_ascii_case("en_US.UTF-8")
                || trimmed_col.eq_ignore_ascii_case("C.UTF-8")
            {
                "utf8mb4_unicode_ci"
            } else {
                trimmed_col
            }
        }
        DbEngineKind::Postgres => {
            if trimmed_col.is_empty()
                || trimmed_col.starts_with("utf8mb4")
                || trimmed_col.starts_with("latin1")
            {
                "en_US.UTF-8"
            } else {
                trimmed_col
            }
        }
        DbEngineKind::Sqlite => {
            if trimmed_col.is_empty() {
                "UTF-8"
            } else {
                trimmed_col
            }
        }
    };
    let collation = CollationName::parse(effective_collation_str)?;

    match engine {
        DbEngineKind::Sqlite => {
            if let Some(pw) = password_raw {
                if !pw.is_empty() {
                    let _ = validate_db_password(pw)?;
                }
            }
            let conn = open_sqlite_db(store, &db_ident)?;
            conn.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA foreign_keys = ON;",
            )?;
        }
        DbEngineKind::MariaDb => {
            let generated_pw;
            let raw_pw = match password_raw.map(str::trim).filter(|s| !s.is_empty()) {
                Some(p) => p,
                None => {
                    generated_pw = generate_random_db_password();
                    generated_pw.as_str()
                }
            };
            let pw = validate_db_password(raw_pw)?;
            let qdb = db_ident.quote_mysql();
            let quser = escape_mysql_literal(user_ident.as_str())?;
            let qpw = escape_mysql_literal(pw)?;
            let col = collation.as_str();
            let sql = format!(
                "CREATE DATABASE IF NOT EXISTS {qdb} DEFAULT CHARACTER SET utf8mb4 COLLATE {col}; \
                 CREATE USER IF NOT EXISTS {quser}@'localhost' IDENTIFIED BY {qpw}; \
                 CREATE USER IF NOT EXISTS {quser}@'127.0.0.1' IDENTIFIED BY {qpw}; \
                 ALTER USER {quser}@'localhost' IDENTIFIED BY {qpw}; \
                 ALTER USER {quser}@'127.0.0.1' IDENTIFIED BY {qpw}; \
                 GRANT ALL PRIVILEGES ON {qdb}.* TO {quser}@'localhost'; \
                 GRANT ALL PRIVILEGES ON {qdb}.* TO {quser}@'127.0.0.1'; \
                 FLUSH PRIVILEGES;"
            );
            run_mysql_stdin(executor, None, &sql).await?;
        }
        DbEngineKind::Postgres => {
            let generated_pw;
            let raw_pw = match password_raw.map(str::trim).filter(|s| !s.is_empty()) {
                Some(p) => p,
                None => {
                    generated_pw = generate_random_db_password();
                    generated_pw.as_str()
                }
            };
            let pw = validate_db_password(raw_pw)?;
            let qdb = db_ident.quote_postgres();
            let quser = user_ident.quote_postgres();
            let qpw = escape_postgres_literal(pw)?;
            let create_role_sql = format!(
                "DO $$ BEGIN IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = {}) THEN CREATE ROLE {quser} WITH LOGIN PASSWORD {qpw}; ELSE ALTER ROLE {quser} WITH LOGIN PASSWORD {qpw}; END IF; END $$;\n",
                escape_postgres_literal(user_ident.as_str())?
            );
            run_psql_stdin(executor, None, &create_role_sql).await?;
            let create_db_sql = format!("CREATE DATABASE {qdb} OWNER {quser};");
            if let Err(err) = run_psql(executor, None, &create_db_sql).await {
                let msg = err.to_string().to_ascii_lowercase();
                if !msg.contains("already exists") {
                    return Err(err);
                }
                let grant_sql = format!(
                    "ALTER DATABASE {qdb} OWNER TO {quser}; GRANT ALL PRIVILEGES ON DATABASE {qdb} TO {quser};"
                );
                let _ = run_psql(executor, None, &grant_sql).await;
            }
        }
    }

    let linked_app_name = if linked_app_id.trim().is_empty() {
        String::new()
    } else {
        store
            .get_app(linked_app_id)?
            .map(|a| a.domain)
            .unwrap_or_default()
    };

    let host_label = match engine {
        DbEngineKind::Sqlite => sqlite_db_path(store, &db_ident)?
            .display()
            .to_string(),
        DbEngineKind::MariaDb | DbEngineKind::Postgres => "127.0.0.1".to_string(),
    };

    let meta = ManagedDbMeta {
        id: format!("db-{}-{}", engine.id_prefix(), db_ident.as_str()),
        name: db_ident.as_str().to_string(),
        engine: engine.label().to_string(),
        host: host_label,
        port: engine.default_port(),
        username: user_ident.as_str().to_string(),
        collation: collation.as_str().to_string(),
        linked_app_id: linked_app_id.trim().to_string(),
        linked_app_name,
        created_at: Utc::now().to_rfc3339(),
    };
    store.insert_database(&meta)?;
    list_databases(store, executor).await
}

pub async fn change_database_password(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
    id: &str,
    new_password: &str,
) -> Result<Vec<SqlDatabase>, PanelError> {
    let meta = store
        .get_database(id)?
        .ok_or_else(|| PanelError::NotFound(format!("database '{id}' not found")))?;
    let engine = DbEngineKind::parse(&meta.engine)?;
    ensure_engine_installed(executor, engine)?;

    let user_ident = SqlIdentifier::parse(&meta.username)?;
    let pw = validate_db_password(new_password)?;

    match engine {
        DbEngineKind::Sqlite => {
            return Err(PanelError::Validation(
                "SQLite 3 (WAL) uses local POSIX file permissions & Argon2id panel session authentication instead of a database user password".to_string(),
            ));
        }
        DbEngineKind::MariaDb => {
            let quser = escape_mysql_literal(user_ident.as_str())?;
            let qpw = escape_mysql_literal(pw)?;
            let sql = format!(
                "ALTER USER {quser}@'localhost' IDENTIFIED BY {qpw}; \
                 ALTER USER {quser}@'127.0.0.1' IDENTIFIED BY {qpw}; \
                 FLUSH PRIVILEGES;"
            );
            run_mysql_stdin(executor, None, &sql).await?;
        }
        DbEngineKind::Postgres => {
            let quser = user_ident.quote_postgres();
            let qpw = escape_postgres_literal(pw)?;
            let sql = format!("ALTER ROLE {quser} WITH PASSWORD {qpw};\n");
            run_psql_stdin(executor, None, &sql).await?;
        }
    }

    list_databases(store, executor).await
}

pub async fn delete_database(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<Vec<SqlDatabase>, PanelError> {
    let meta = store
        .get_database(id)?
        .ok_or_else(|| PanelError::NotFound(format!("database '{id}' not found")))?;
    let engine = DbEngineKind::parse(&meta.engine)?;
    let db_ident = SqlIdentifier::parse(&meta.name)?;

    match engine {
        DbEngineKind::Sqlite => {
            let path = sqlite_db_path(store, &db_ident)?;
            for suffix in ["", "-wal", "-shm"] {
                let target = if suffix.is_empty() {
                    path.clone()
                } else {
                    PathBuf::from(format!("{}{suffix}", path.display()))
                };
                if target.exists() {
                    let _ = std::fs::remove_file(&target);
                }
            }
        }
        DbEngineKind::MariaDb => {
            ensure_engine_installed(executor, engine)?;
            let qdb = db_ident.quote_mysql();
            let sql = format!("DROP DATABASE IF EXISTS {qdb};");
            run_mysql(executor, None, &sql).await?;
        }
        DbEngineKind::Postgres => {
            ensure_engine_installed(executor, engine)?;
            let qdb = db_ident.quote_postgres();
            let sql = format!("DROP DATABASE IF EXISTS {qdb};");
            run_psql(executor, None, &sql).await?;
        }
    }

    store.delete_database(id)?;
    list_databases(store, executor).await
}

fn execute_sqlite_query_sync(
    store: &ModuleStore,
    db_ident: &SqlIdentifier,
    query: &str,
) -> Result<SqlQueryResponse, PanelError> {
    let start = Instant::now();
    let conn = open_sqlite_db(store, db_ident)?;
    let trimmed = query.trim();

    // Friendly MySQL-style shortcut compatibility inside Pure-Rust FerrousAdmin
    let upper = trimmed.to_ascii_uppercase();
    let effective_sql = if upper == "SHOW TABLES" || upper == "SHOW TABLES;" {
        "SELECT name AS table_name, type, sql FROM sqlite_master WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' ORDER BY name;"
    } else {
        trimmed
    };

    let first_word = effective_sql
        .split_whitespace()
        .next()
        .unwrap_or("SQL")
        .trim_end_matches(';')
        .to_ascii_uppercase();

    let single_stmt = !effective_sql
        .trim_end_matches(';')
        .contains(';');
    let returns_rows = single_stmt
        && matches!(
            first_word.as_str(),
            "SELECT" | "PRAGMA" | "EXPLAIN" | "WITH" | "VALUES"
        );

    if returns_rows {
        match sqlite_query_rows(&conn, effective_sql, MAX_BROWSE_LIMIT) {
            Ok((columns, rows)) => {
                let elapsed_ms =
                    ((start.elapsed().as_secs_f64() * 1000.0) * 100.0).round() / 100.0;
                let affected = u64::try_from(rows.len()).unwrap_or(0);
                Ok(SqlQueryResponse {
                    success: true,
                    execution_time_ms: elapsed_ms,
                    command_tag: first_word,
                    affected_rows: affected,
                    columns,
                    rows,
                    error: None,
                })
            }
            Err(err) => {
                let elapsed_ms =
                    ((start.elapsed().as_secs_f64() * 1000.0) * 100.0).round() / 100.0;
                Ok(SqlQueryResponse {
                    success: false,
                    execution_time_ms: elapsed_ms,
                    command_tag: "ERROR".to_string(),
                    affected_rows: 0,
                    columns: Vec::new(),
                    rows: Vec::new(),
                    error: Some(err.to_string()),
                })
            }
        }
    } else {
        match conn.execute_batch(effective_sql) {
            Ok(()) => {
                let affected = conn.changes();
                let elapsed_ms =
                    ((start.elapsed().as_secs_f64() * 1000.0) * 100.0).round() / 100.0;
                Ok(SqlQueryResponse {
                    success: true,
                    execution_time_ms: elapsed_ms,
                    command_tag: first_word,
                    affected_rows: affected,
                    columns: vec!["status".to_string(), "affected_rows".to_string()],
                    rows: vec![BTreeMap::from([
                        ("status".to_string(), Value::String("OK".to_string())),
                        (
                            "affected_rows".to_string(),
                            Value::Number(Number::from(affected)),
                        ),
                    ])],
                    error: None,
                })
            }
            Err(err) => {
                let elapsed_ms =
                    ((start.elapsed().as_secs_f64() * 1000.0) * 100.0).round() / 100.0;
                Ok(SqlQueryResponse {
                    success: false,
                    execution_time_ms: elapsed_ms,
                    command_tag: "ERROR".to_string(),
                    affected_rows: 0,
                    columns: Vec::new(),
                    rows: Vec::new(),
                    error: Some(err.to_string()),
                })
            }
        }
    }
}

pub async fn execute_sql_query(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
    id: &str,
    query: &str,
) -> Result<SqlQueryResponse, PanelError> {
    let trimmed = query.trim();
    if trimmed.is_empty() || trimmed.len() > 65536 {
        return Err(PanelError::Validation(
            "SQL query must be 1..=65536 characters".to_string(),
        ));
    }
    if trimmed.contains('\0') {
        return Err(PanelError::Forbidden(
            "NUL byte in SQL query is forbidden".to_string(),
        ));
    }

    let meta = store
        .get_database(id)?
        .ok_or_else(|| PanelError::NotFound(format!("database '{id}' not found")))?;
    let engine = DbEngineKind::parse(&meta.engine)?;
    let db_ident = SqlIdentifier::parse(&meta.name)?;

    let start = Instant::now();
    let res = match engine {
        DbEngineKind::Sqlite => return execute_sqlite_query_sync(store, &db_ident, trimmed),
        DbEngineKind::MariaDb => {
            ensure_engine_installed(executor, engine)?;
            run_mysql(executor, Some(db_ident.as_str()), trimmed).await
        }
        DbEngineKind::Postgres => {
            ensure_engine_installed(executor, engine)?;
            run_psql_internal(executor, Some(db_ident.as_str()), trimmed, true).await
        }
    };
    let elapsed_ms = ((start.elapsed().as_secs_f64() * 1000.0) * 100.0).round() / 100.0;

    match res {
        Ok(out) => {
            let (columns, rows) = parse_tsv_with_header(&out.stdout);
            let affected = u64::try_from(rows.len()).unwrap_or(0);
            Ok(SqlQueryResponse {
                success: true,
                execution_time_ms: elapsed_ms,
                command_tag: trimmed
                    .split_whitespace()
                    .next()
                    .unwrap_or("SQL")
                    .to_ascii_uppercase(),
                affected_rows: affected,
                columns,
                rows,
                error: None,
            })
        }
        Err(PanelError::CommandFailed { stderr, .. }) => Ok(SqlQueryResponse {
            success: false,
            execution_time_ms: elapsed_ms,
            command_tag: "ERROR".to_string(),
            affected_rows: 0,
            columns: Vec::new(),
            rows: Vec::new(),
            error: Some(stderr),
        }),
        Err(err) => Err(err),
    }
}

pub async fn browse_table_rows(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
    id: &str,
    table_raw: &str,
    limit: usize,
    offset: usize,
) -> Result<SqlQueryResponse, PanelError> {
    let meta = store
        .get_database(id)?
        .ok_or_else(|| PanelError::NotFound(format!("database '{id}' not found")))?;
    let engine = DbEngineKind::parse(&meta.engine)?;
    let table_ident = SqlIdentifier::parse(table_raw)?;
    let safe_limit = limit.clamp(1, MAX_BROWSE_LIMIT);
    let qtable = match engine {
        DbEngineKind::MariaDb => table_ident.quote_mysql(),
        DbEngineKind::Postgres | DbEngineKind::Sqlite => table_ident.quote_postgres(),
    };
    let sql = format!("SELECT * FROM {qtable} LIMIT {safe_limit} OFFSET {offset};");
    execute_sql_query(store, executor, id, &sql).await
}

pub async fn drop_table(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
    id: &str,
    table_raw: &str,
) -> Result<Vec<SqlDatabase>, PanelError> {
    let meta = store
        .get_database(id)?
        .ok_or_else(|| PanelError::NotFound(format!("database '{id}' not found")))?;
    let engine = DbEngineKind::parse(&meta.engine)?;
    let db_ident = SqlIdentifier::parse(&meta.name)?;
    let table_ident = SqlIdentifier::parse(table_raw)?;

    match engine {
        DbEngineKind::Sqlite => {
            let conn = open_sqlite_db(store, &db_ident)?;
            let sql = format!("DROP TABLE IF EXISTS {};", table_ident.quote_postgres());
            conn.execute_batch(&sql)?;
        }
        DbEngineKind::MariaDb => {
            let sql = format!("DROP TABLE IF EXISTS {};", table_ident.quote_mysql());
            run_mysql(executor, Some(db_ident.as_str()), &sql).await?;
        }
        DbEngineKind::Postgres => {
            let sql = format!("DROP TABLE IF EXISTS {};", table_ident.quote_postgres());
            run_psql(executor, Some(db_ident.as_str()), &sql).await?;
        }
    }
    list_databases(store, executor).await
}

pub async fn truncate_table(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
    id: &str,
    table_raw: &str,
) -> Result<Vec<SqlDatabase>, PanelError> {
    let meta = store
        .get_database(id)?
        .ok_or_else(|| PanelError::NotFound(format!("database '{id}' not found")))?;
    let engine = DbEngineKind::parse(&meta.engine)?;
    let db_ident = SqlIdentifier::parse(&meta.name)?;
    let table_ident = SqlIdentifier::parse(table_raw)?;

    match engine {
        DbEngineKind::Sqlite => {
            let conn = open_sqlite_db(store, &db_ident)?;
            let sql = format!("DELETE FROM {};", table_ident.quote_postgres());
            conn.execute_batch(&sql)?;
        }
        DbEngineKind::MariaDb => {
            let sql = format!("TRUNCATE TABLE {};", table_ident.quote_mysql());
            run_mysql(executor, Some(db_ident.as_str()), &sql).await?;
        }
        DbEngineKind::Postgres => {
            let sql = format!("TRUNCATE TABLE {};", table_ident.quote_postgres());
            run_psql(executor, Some(db_ident.as_str()), &sql).await?;
        }
    }
    list_databases(store, executor).await
}

fn export_sqlite_sql_sync(
    store: &ModuleStore,
    db_ident: &SqlIdentifier,
) -> Result<String, PanelError> {
    let conn = open_sqlite_db(store, db_ident)?;
    let mut out = String::new();
    out.push_str(&format!(
        "-- FerrousAdmin Pure-Rust SQLite 3 (WAL) Dump\n-- Database: {}\n-- Generated: {}\n\nPRAGMA foreign_keys = OFF;\nBEGIN TRANSACTION;\n\n",
        db_ident.as_str(),
        Utc::now().to_rfc3339()
    ));

    let mut objects: Vec<(String, String, String)> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT type, name, sql FROM sqlite_master WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' ORDER BY CASE type WHEN 'table' THEN 1 WHEN 'index' THEN 2 WHEN 'trigger' THEN 3 ELSE 4 END, name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        for item in rows {
            objects.push(item?);
        }
    }

    for (obj_type, name, ddl) in objects {
        out.push_str(ddl.trim_end_matches(';'));
        out.push_str(";\n");
        if obj_type == "table" {
            if let Ok(t_ident) = SqlIdentifier::parse(&name) {
                let qtable = t_ident.quote_postgres();
                let select_sql = format!("SELECT * FROM {qtable};");
                let mut stmt = conn.prepare(&select_sql)?;
                let col_names: Vec<String> = stmt
                    .column_names()
                    .into_iter()
                    .map(|c| format!("\"{}\"", c.replace('"', "\"\"")))
                    .collect();
                let cols_joined = col_names.join(", ");
                let col_count = col_names.len();
                let mut raw_rows = stmt.query([])?;
                while let Some(row) = raw_rows.next()? {
                    let mut vals = Vec::with_capacity(col_count);
                    for idx in 0..col_count {
                        vals.push(sqlite_value_to_sql_literal(row.get_ref(idx)?));
                    }
                    out.push_str(&format!(
                        "INSERT INTO {qtable} ({cols_joined}) VALUES ({});\n",
                        vals.join(", ")
                    ));
                }
            }
        }
        out.push('\n');
    }

    out.push_str("COMMIT;\nPRAGMA foreign_keys = ON;\n");
    Ok(out)
}

struct TempDumpGuard(PathBuf);

impl Drop for TempDumpGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn build_temp_dump_path(store: &ModuleStore, db_ident: &SqlIdentifier) -> Result<TempDumpGuard, PanelError> {
    let tmp_dir = store.data_dir().join("tmp");
    std::fs::create_dir_all(&tmp_dir)?;
    let seq = DUMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let stamp = Utc::now().timestamp_millis();
    let file_name = format!(
        "dump-{}-{}-{}-{}.sql",
        db_ident.as_str(),
        std::process::id(),
        stamp,
        seq
    );
    let safe = SafePath::under_root(&tmp_dir, &file_name)?;
    Ok(TempDumpGuard(safe.as_path().to_path_buf()))
}

fn read_dump_file_bounded(path: &Path) -> Result<String, PanelError> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > MAX_SQL_EXPORT_BYTES {
        return Err(PanelError::Validation(format!(
            "SQL dump size ({} bytes) exceeds 64 MiB HTTP export limit; use cPanel Zstd Backup for larger databases",
            meta.len()
        )));
    }
    let bytes = std::fs::read(path)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub async fn export_database_sql(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<String, PanelError> {
    let meta = store
        .get_database(id)?
        .ok_or_else(|| PanelError::NotFound(format!("database '{id}' not found")))?;
    let engine = DbEngineKind::parse(&meta.engine)?;
    let db_ident = SqlIdentifier::parse(&meta.name)?;

    match engine {
        DbEngineKind::Sqlite => export_sqlite_sql_sync(store, &db_ident),
        DbEngineKind::MariaDb => {
            if !executor.is_binary_available(AllowedBinary::Mysqldump) {
                return Err(PanelError::Conflict(
                    "mysqldump is not installed on this host".to_string(),
                ));
            }
            let dump_guard = build_temp_dump_path(store, &db_ident)?;
            let out = executor
                .run_to_file(
                    AllowedBinary::Mysqldump,
                    &[
                        "--single-transaction",
                        "--routines",
                        "--triggers",
                        db_ident.as_str(),
                    ],
                    &dump_guard.0,
                    DB_DUMP_TIMEOUT,
                )
                .await?;
            if !out.success {
                return Err(PanelError::CommandFailed {
                    binary: "mysqldump",
                    exit_code: out.exit_code,
                    stderr: out.stderr,
                });
            }
            read_dump_file_bounded(&dump_guard.0)
        }
        DbEngineKind::Postgres => {
            if !executor.is_binary_available(AllowedBinary::PgDump) {
                return Err(PanelError::Conflict(
                    "pg_dump is not installed on this host".to_string(),
                ));
            }
            let dump_guard = build_temp_dump_path(store, &db_ident)?;
            let out = if should_use_runuser_postgres(executor) {
                executor
                    .run_to_file(
                        AllowedBinary::Runuser,
                        &[
                            "-u",
                            "postgres",
                            "--",
                            "pg_dump",
                            "--clean",
                            "--if-exists",
                            db_ident.as_str(),
                        ],
                        &dump_guard.0,
                        DB_DUMP_TIMEOUT,
                    )
                    .await?
            } else {
                executor
                    .run_to_file(
                        AllowedBinary::PgDump,
                        &["--clean", "--if-exists", db_ident.as_str()],
                        &dump_guard.0,
                        DB_DUMP_TIMEOUT,
                    )
                    .await?
            };
            if !out.success {
                return Err(PanelError::CommandFailed {
                    binary: "pg_dump",
                    exit_code: out.exit_code,
                    stderr: out.stderr,
                });
            }
            read_dump_file_bounded(&dump_guard.0)
        }
    }
}

pub async fn import_database_sql(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
    id: &str,
    sql_script: &str,
) -> Result<SqlQueryResponse, PanelError> {
    let trimmed = sql_script.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_SQL_IMPORT_BYTES {
        return Err(PanelError::Validation(format!(
            "SQL import script must be 1..={MAX_SQL_IMPORT_BYTES} bytes"
        )));
    }
    if trimmed.contains('\0') {
        return Err(PanelError::Forbidden(
            "NUL byte in SQL import script is forbidden".to_string(),
        ));
    }

    let meta = store
        .get_database(id)?
        .ok_or_else(|| PanelError::NotFound(format!("database '{id}' not found")))?;
    let engine = DbEngineKind::parse(&meta.engine)?;
    let db_ident = SqlIdentifier::parse(&meta.name)?;
    let start = Instant::now();

    match engine {
        DbEngineKind::Sqlite => {
            let conn = open_sqlite_db(store, &db_ident)?;
            match conn.execute_batch(trimmed) {
                Ok(()) => {
                    let affected = conn.changes();
                    let elapsed_ms =
                        ((start.elapsed().as_secs_f64() * 1000.0) * 100.0).round() / 100.0;
                    Ok(SqlQueryResponse {
                        success: true,
                        execution_time_ms: elapsed_ms,
                        command_tag: "IMPORT".to_string(),
                        affected_rows: affected,
                        columns: vec!["status".to_string(), "affected_rows".to_string()],
                        rows: vec![BTreeMap::from([
                            (
                                "status".to_string(),
                                Value::String("SQLite 3 (WAL) script imported".to_string()),
                            ),
                            (
                                "affected_rows".to_string(),
                                Value::Number(Number::from(affected)),
                            ),
                        ])],
                        error: None,
                    })
                }
                Err(err) => {
                    let elapsed_ms =
                        ((start.elapsed().as_secs_f64() * 1000.0) * 100.0).round() / 100.0;
                    Ok(SqlQueryResponse {
                        success: false,
                        execution_time_ms: elapsed_ms,
                        command_tag: "IMPORT_ERROR".to_string(),
                        affected_rows: 0,
                        columns: Vec::new(),
                        rows: Vec::new(),
                        error: Some(err.to_string()),
                    })
                }
            }
        }
        DbEngineKind::MariaDb => {
            ensure_engine_installed(executor, engine)?;
            let out = executor
                .run_with_stdin(
                    AllowedBinary::Mysql,
                    &["-D", db_ident.as_str()],
                    trimmed.as_bytes(),
                    DB_DUMP_TIMEOUT,
                )
                .await?;
            let elapsed_ms = ((start.elapsed().as_secs_f64() * 1000.0) * 100.0).round() / 100.0;
            if !out.success {
                return Ok(SqlQueryResponse {
                    success: false,
                    execution_time_ms: elapsed_ms,
                    command_tag: "IMPORT_ERROR".to_string(),
                    affected_rows: 0,
                    columns: Vec::new(),
                    rows: Vec::new(),
                    error: Some(out.stderr),
                });
            }
            Ok(SqlQueryResponse {
                success: true,
                execution_time_ms: elapsed_ms,
                command_tag: "IMPORT".to_string(),
                affected_rows: 0,
                columns: vec!["status".to_string()],
                rows: vec![BTreeMap::from([(
                    "status".to_string(),
                    Value::String("MariaDB SQL script imported".to_string()),
                )])],
                error: None,
            })
        }
        DbEngineKind::Postgres => {
            ensure_engine_installed(executor, engine)?;
            let out = if should_use_runuser_postgres(executor) {
                executor
                    .run_with_stdin(
                        AllowedBinary::Runuser,
                        &[
                            "-u",
                            "postgres",
                            "--",
                            "psql",
                            "-v",
                            "ON_ERROR_STOP=1",
                            "-d",
                            db_ident.as_str(),
                        ],
                        trimmed.as_bytes(),
                        DB_DUMP_TIMEOUT,
                    )
                    .await?
            } else {
                executor
                    .run_with_stdin(
                        AllowedBinary::Psql,
                        &["-v", "ON_ERROR_STOP=1", "-d", db_ident.as_str()],
                        trimmed.as_bytes(),
                        DB_DUMP_TIMEOUT,
                    )
                    .await?
            };
            let elapsed_ms = ((start.elapsed().as_secs_f64() * 1000.0) * 100.0).round() / 100.0;
            if !out.success {
                return Ok(SqlQueryResponse {
                    success: false,
                    execution_time_ms: elapsed_ms,
                    command_tag: "IMPORT_ERROR".to_string(),
                    affected_rows: 0,
                    columns: Vec::new(),
                    rows: Vec::new(),
                    error: Some(out.stderr),
                });
            }
            Ok(SqlQueryResponse {
                success: true,
                execution_time_ms: elapsed_ms,
                command_tag: "IMPORT".to_string(),
                affected_rows: 0,
                columns: vec!["status".to_string()],
                rows: vec![BTreeMap::from([(
                    "status".to_string(),
                    Value::String("PostgreSQL SQL script imported".to_string()),
                )])],
                error: None,
            })
        }
    }
}

pub async fn get_mysql_config_with_store(
    store: Option<&ModuleStore>,
    executor: &dyn CommandExecutor,
    cnf_path: Option<&Path>,
    pma_port_default: u16,
) -> Result<MysqlPhpMyAdminConfig, PanelError> {
    let path = cnf_path.unwrap_or_else(|| Path::new(DEFAULT_MARIADB_CNF));
    let raw = std::fs::read_to_string(path).unwrap_or_default();

    let mysql_version = if executor.is_binary_available(AllowedBinary::Mysql) {
        executor
            .run(AllowedBinary::Mysql, &["--version"], DB_CMD_TIMEOUT)
            .await
            .ok()
            .filter(|o| o.success && !o.stdout.trim().is_empty())
            .map(|o| o.stdout.trim().to_string())
            .unwrap_or_else(|| "MariaDB / MySQL CLI installed".to_string())
    } else {
        "Pure-Rust SQLite 3 (WAL) Active — MariaDB/MySQL CLI not installed".to_string()
    };

    let mut bind_address = "127.0.0.1".to_string();
    let mut port: u16 = 3306;
    let mut max_connections: u32 = 151;
    let mut long_query_time_sec: u32 = 2;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            let key = k.trim();
            let val = v.trim();
            match key {
                "bind-address" => bind_address = val.to_string(),
                "port" => port = val.parse::<u16>().unwrap_or(port),
                "max_connections" => {
                    max_connections = val.parse::<u32>().unwrap_or(max_connections)
                }
                "long_query_time" => {
                    long_query_time_sec = val.parse::<u32>().unwrap_or(long_query_time_sec)
                }
                _ => {}
            }
        }
    }

    let persisted = store
        .and_then(|s| s.get_json::<PersistedPmaSettings>(FERROUS_ADMIN_KV_KEY).ok().flatten())
        .unwrap_or_else(|| PersistedPmaSettings {
            pma_port: if pma_port_default == 0 {
                888
            } else {
                pma_port_default
            },
            ..PersistedPmaSettings::default()
        });

    Ok(MysqlPhpMyAdminConfig {
        mysql_version,
        socket_path: "/run/mysqld/mysqld.sock".to_string(),
        bind_address,
        port,
        root_password: None,
        innodb_buffer_pool_mb: 128,
        max_connections,
        slow_query_log: raw.contains("slow_query_log"),
        long_query_time_sec,
        default_charset: "utf8mb4".to_string(),
        my_cnf_raw: raw,
        pma_version: "FerrousAdmin 2.5 (100% Pure Rust · Zero PHP)".to_string(),
        pma_path: persisted.pma_path,
        pma_port: persisted.pma_port,
        pma_ssl_only: persisted.pma_ssl_only,
        pma_basic_auth: persisted.pma_basic_auth,
        pma_max_upload_mb: persisted.pma_max_upload_mb,
        pma_allowed_ips: persisted.pma_allowed_ips,
        // Always running natively inside Actix-Web ferrous-panel (no PHP process needed).
        pma_status: "running".to_string(),
    })
}

pub async fn get_mysql_config(
    executor: &dyn CommandExecutor,
    cnf_path: Option<&Path>,
) -> Result<MysqlPhpMyAdminConfig, PanelError> {
    get_mysql_config_with_store(None, executor, cnf_path, 888).await
}

#[derive(Debug, Default)]
pub struct MysqlConfigPatch {
    pub my_cnf_raw: Option<String>,
    pub pma_path: Option<String>,
    pub pma_port: Option<u16>,
    pub pma_ssl_only: Option<bool>,
    pub pma_basic_auth: Option<bool>,
    pub pma_max_upload_mb: Option<u32>,
    pub pma_allowed_ips: Option<String>,
}

pub async fn update_mysql_config_with_store(
    store: Option<&ModuleStore>,
    executor: &dyn CommandExecutor,
    patch: MysqlConfigPatch,
    cnf_path: Option<&Path>,
    pma_port_default: u16,
) -> Result<MysqlPhpMyAdminConfig, PanelError> {
    if let Some(s) = store {
        let mut settings = s
            .get_json::<PersistedPmaSettings>(FERROUS_ADMIN_KV_KEY)?
            .unwrap_or_else(|| PersistedPmaSettings {
                pma_port: if pma_port_default == 0 {
                    888
                } else {
                    pma_port_default
                },
                ..PersistedPmaSettings::default()
            });
        if let Some(p) = patch.pma_path {
            let clean = p.trim();
            if !clean.is_empty() && clean.starts_with('/') && !clean.contains("..") {
                settings.pma_path = clean.trim_end_matches('/').to_string();
            }
        }
        if let Some(port) = patch.pma_port {
            if port > 0 && port != 80 && port != 443 {
                settings.pma_port = port;
            }
        }
        if let Some(v) = patch.pma_ssl_only {
            settings.pma_ssl_only = v;
        }
        if let Some(v) = patch.pma_basic_auth {
            settings.pma_basic_auth = v;
        }
        if let Some(v) = patch.pma_max_upload_mb {
            settings.pma_max_upload_mb = v.clamp(1, 512);
        }
        if let Some(v) = patch.pma_allowed_ips {
            if !v.contains('\0') && v.len() <= 256 {
                settings.pma_allowed_ips = v;
            }
        }
        s.put_json(FERROUS_ADMIN_KV_KEY, &settings)?;
    }

    let path = cnf_path.unwrap_or_else(|| Path::new(DEFAULT_MARIADB_CNF));
    if let Some(my_cnf_raw) = patch.my_cnf_raw {
        if my_cnf_raw.contains('\0') {
            return Err(PanelError::Forbidden(
                "NUL byte in my.cnf is forbidden".to_string(),
            ));
        }
        if !my_cnf_raw.trim().is_empty() && executor.is_binary_available(AllowedBinary::Mysql) {
            atomic_write_file(path, my_cnf_raw.as_bytes(), 0o644)?;
            if executor.is_binary_available(AllowedBinary::Systemctl) {
                let out = executor
                    .run(AllowedBinary::Systemctl, &["reload", "mariadb"], DB_CMD_TIMEOUT)
                    .await?;
                if !out.success {
                    return Err(PanelError::CommandFailed {
                        binary: "systemctl",
                        exit_code: out.exit_code,
                        stderr: out.stderr,
                    });
                }
            }
        }
    }

    get_mysql_config_with_store(store, executor, Some(path), pma_port_default).await
}

pub async fn update_mysql_config(
    executor: &dyn CommandExecutor,
    my_cnf_raw: &str,
    cnf_path: Option<&Path>,
) -> Result<MysqlPhpMyAdminConfig, PanelError> {
    update_mysql_config_with_store(
        None,
        executor,
        MysqlConfigPatch {
            my_cnf_raw: Some(my_cnf_raw.to_string()),
            ..MysqlConfigPatch::default()
        },
        cnf_path,
        888,
    )
    .await
}
