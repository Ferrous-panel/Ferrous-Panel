use crate::api::contract::{
    MysqlPhpMyAdminConfig, SqlDatabase, SqlQueryResponse, SqlTableColumn, SqlTableSchema,
};
use crate::error::PanelError;
use crate::modules::{ManagedDbMeta, ModuleStore};
use crate::system::cmd::AllowedBinary;
use crate::system::validate::{
    escape_mysql_literal, escape_postgres_literal, CollationName, SqlIdentifier,
};
use crate::webserver::{atomic_write_file, CommandExecutor};
use chrono::Utc;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

const DB_CMD_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_MARIADB_CNF: &str = "/etc/mysql/mariadb.conf.d/50-server.cnf";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbEngineKind {
    MariaDb,
    Postgres,
}

impl DbEngineKind {
    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        let lower = raw.trim().to_ascii_lowercase();
        if lower.contains("postgres") || lower == "pgsql" || lower == "pg" {
            Ok(Self::Postgres)
        } else if lower.contains("mariadb") || lower.contains("mysql") {
            Ok(Self::MariaDb)
        } else {
            Err(PanelError::Validation(format!(
                "unsupported SQL engine '{raw}'; allowed: MariaDB 11.4, PostgreSQL 17"
            )))
        }
    }

    #[must_use]
    pub const fn binary(self) -> AllowedBinary {
        match self {
            Self::MariaDb => AllowedBinary::Mysql,
            Self::Postgres => AllowedBinary::Psql,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::MariaDb => "MariaDB 11.4",
            Self::Postgres => "PostgreSQL 17",
        }
    }

    #[must_use]
    pub const fn default_port(self) -> u16 {
        match self {
            Self::MariaDb => 3306,
            Self::Postgres => 5432,
        }
    }
}

fn ensure_engine_installed(
    executor: &dyn CommandExecutor,
    engine: DbEngineKind,
) -> Result<(), PanelError> {
    let bin = engine.binary();
    if !executor.is_binary_available(bin) {
        return Err(PanelError::Conflict(format!(
            "{} CLI ({}) is not installed on this host",
            engine.label(),
            bin.label()
        )));
    }
    Ok(())
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

async fn run_psql(
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
    args.push("-c");
    args.push(sql);
    let out = executor
        .run_with_env(AllowedBinary::Psql, &args, &[], None, DB_CMD_TIMEOUT)
        .await?;
    if !out.success {
        return Err(PanelError::CommandFailed {
            binary: "psql",
            exit_code: out.exit_code,
            stderr: out.stderr,
        });
    }
    Ok(out)
}

async fn query_db_size_and_tables(
    executor: &dyn CommandExecutor,
    engine: DbEngineKind,
    db_name: &SqlIdentifier,
) -> (f64, Vec<SqlTableSchema>, String) {
    match engine {
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
                        tables.push(SqlTableSchema {
                            table_name: tname,
                            row_count,
                            size_kb,
                            columns: table_cols,
                            rows: Vec::new(),
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
            let size_sql = format!("SELECT ROUND(pg_database_size({db_lit})::numeric / 1048576.0, 2);");
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
                        tables.push(SqlTableSchema {
                            table_name: tname,
                            row_count,
                            size_kb,
                            columns: Vec::new(),
                            rows: Vec::new(),
                        });
                    }
                }
            }
            (size_mb, tables, "online".to_string())
        }
    }
}

pub async fn list_databases(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
) -> Result<Vec<SqlDatabase>, PanelError> {
    let metas = store.list_databases()?;
    let mut out = Vec::with_capacity(metas.len());
    for meta in metas {
        let engine_kind = DbEngineKind::parse(&meta.engine)?;
        let db_ident = SqlIdentifier::parse(&meta.name)?;
        let (size_mb, tables, status) = if executor.is_binary_available(engine_kind.binary()) {
            query_db_size_and_tables(executor, engine_kind, &db_ident).await
        } else {
            (0.0, Vec::new(), "engine_not_installed".to_string())
        };
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
            active_connections: 0,
            max_connections: 150,
            status,
            created_at: meta.created_at,
            tables,
        });
    }
    Ok(out)
}

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
    ensure_engine_installed(executor, engine)?;

    let db_ident = SqlIdentifier::parse(name_raw)?;
    let user_ident = SqlIdentifier::parse(username_raw)?;
    let collation = CollationName::parse(if collation_raw.trim().is_empty() {
        "utf8mb4_unicode_ci"
    } else {
        collation_raw
    })?;
    let pw = password_raw.ok_or_else(|| {
        PanelError::Validation("password is required when creating a database user".to_string())
    })?;
    let pw = validate_db_password(pw)?;

    match engine {
        DbEngineKind::MariaDb => {
            let qdb = db_ident.quote_mysql();
            let quser = escape_mysql_literal(user_ident.as_str())?;
            let qpw = escape_mysql_literal(pw)?;
            let col = collation.as_str();
            let sql = format!(
                "CREATE DATABASE IF NOT EXISTS {qdb} DEFAULT CHARACTER SET utf8mb4 COLLATE {col}; \
                 CREATE USER IF NOT EXISTS {quser}@'localhost' IDENTIFIED BY {qpw}; \
                 CREATE USER IF NOT EXISTS {quser}@'127.0.0.1' IDENTIFIED BY {qpw}; \
                 GRANT ALL PRIVILEGES ON {qdb}.* TO {quser}@'localhost'; \
                 GRANT ALL PRIVILEGES ON {qdb}.* TO {quser}@'127.0.0.1'; \
                 FLUSH PRIVILEGES;"
            );
            run_mysql(executor, None, &sql).await?;
        }
        DbEngineKind::Postgres => {
            let qdb = db_ident.quote_postgres();
            let quser = user_ident.quote_postgres();
            let qpw = escape_postgres_literal(pw)?;
            let create_role_sql = format!(
                "DO $$ BEGIN IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = {}) THEN CREATE ROLE {quser} WITH LOGIN PASSWORD {qpw}; END IF; END $$;",
                escape_postgres_literal(user_ident.as_str())?
            );
            run_psql(executor, None, &create_role_sql).await?;
            let create_db_sql = format!("CREATE DATABASE {qdb} OWNER {quser};");
            run_psql(executor, None, &create_db_sql).await?;
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

    let meta = ManagedDbMeta {
        id: format!("db-{}-{}", engine.binary().label(), db_ident.as_str()),
        name: db_ident.as_str().to_string(),
        engine: engine.label().to_string(),
        host: "127.0.0.1".to_string(),
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
        DbEngineKind::MariaDb => {
            let quser = escape_mysql_literal(user_ident.as_str())?;
            let qpw = escape_mysql_literal(pw)?;
            let sql = format!(
                "ALTER USER {quser}@'localhost' IDENTIFIED BY {qpw}; \
                 ALTER USER {quser}@'127.0.0.1' IDENTIFIED BY {qpw}; \
                 FLUSH PRIVILEGES;"
            );
            run_mysql(executor, None, &sql).await?;
        }
        DbEngineKind::Postgres => {
            let quser = user_ident.quote_postgres();
            let qpw = escape_postgres_literal(pw)?;
            let sql = format!("ALTER ROLE {quser} WITH PASSWORD {qpw};");
            run_psql(executor, None, &sql).await?;
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
    ensure_engine_installed(executor, engine)?;

    let db_ident = SqlIdentifier::parse(&meta.name)?;
    match engine {
        DbEngineKind::MariaDb => {
            let qdb = db_ident.quote_mysql();
            let sql = format!("DROP DATABASE IF EXISTS {qdb};");
            run_mysql(executor, None, &sql).await?;
        }
        DbEngineKind::Postgres => {
            let qdb = db_ident.quote_postgres();
            let sql = format!("DROP DATABASE IF EXISTS {qdb};");
            run_psql(executor, None, &sql).await?;
        }
    }

    store.delete_database(id)?;
    list_databases(store, executor).await
}

pub async fn execute_sql_query(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
    id: &str,
    query: &str,
) -> Result<SqlQueryResponse, PanelError> {
    let trimmed = query.trim();
    if trimmed.is_empty() || trimmed.len() > 16384 {
        return Err(PanelError::Validation(
            "SQL query must be 1..=16384 characters".to_string(),
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
    ensure_engine_installed(executor, engine)?;
    let db_ident = SqlIdentifier::parse(&meta.name)?;

    let start = Instant::now();
    let res = match engine {
        DbEngineKind::MariaDb => run_mysql(executor, Some(db_ident.as_str()), trimmed).await,
        DbEngineKind::Postgres => run_psql(executor, Some(db_ident.as_str()), trimmed).await,
    };
    let elapsed_ms = ((start.elapsed().as_secs_f64() * 1000.0) * 100.0).round() / 100.0;

    match res {
        Ok(out) => {
            let mut lines = out.stdout.lines();
            let mut columns = Vec::new();
            let mut rows = Vec::new();

            if engine == DbEngineKind::MariaDb {
                if let Some(header_line) = lines.next() {
                    columns = header_line.split('\t').map(|s| s.to_string()).collect();
                    for line in lines {
                        let vals: Vec<&str> = line.split('\t').collect();
                        let mut map = BTreeMap::new();
                        for (idx, col) in columns.iter().enumerate() {
                            let val = vals.get(idx).copied().unwrap_or("");
                            map.insert(col.clone(), Value::String(val.to_string()));
                        }
                        rows.push(map);
                    }
                }
            } else {
                // psql `-At -F \t` has no header row; synthesize positional column keys c0, c1...
                for line in lines {
                    let vals: Vec<&str> = line.split('\t').collect();
                    if columns.is_empty() {
                        columns = (0..vals.len()).map(|i| format!("col_{i}")).collect();
                    }
                    let mut map = BTreeMap::new();
                    for (idx, col) in columns.iter().enumerate() {
                        let val = vals.get(idx).copied().unwrap_or("");
                        map.insert(col.clone(), Value::String(val.to_string()));
                    }
                    rows.push(map);
                }
            }

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

pub async fn get_mysql_config(
    executor: &dyn CommandExecutor,
    cnf_path: Option<&Path>,
) -> Result<MysqlPhpMyAdminConfig, PanelError> {
    ensure_engine_installed(executor, DbEngineKind::MariaDb)?;
    let path = cnf_path.unwrap_or_else(|| Path::new(DEFAULT_MARIADB_CNF));
    let raw = std::fs::read_to_string(path).unwrap_or_default();

    let ver_out = executor
        .run(AllowedBinary::Mysql, &["--version"], DB_CMD_TIMEOUT)
        .await?;
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
                "max_connections" => max_connections = val.parse::<u32>().unwrap_or(max_connections),
                "long_query_time" => {
                    long_query_time_sec = val.parse::<u32>().unwrap_or(long_query_time_sec)
                }
                _ => {}
            }
        }
    }

    let pma_version = std::fs::read_to_string("/usr/share/phpmyadmin/package.json")
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| {
            v.get("version")
                .and_then(|ver| ver.as_str())
                .map(|ver| ver.to_string())
        })
        .unwrap_or_default();

    let pma_running = if Path::new("/usr/share/phpmyadmin").is_dir() {
        if executor.is_binary_available(AllowedBinary::Systemctl) {
            executor
                .run(
                    AllowedBinary::Systemctl,
                    &["is-active", "--quiet", "ferrous-pma.service"],
                    Duration::from_secs(5),
                )
                .await
                .map(|o| o.success)
                .unwrap_or(false)
        } else {
            true
        }
    } else {
        false
    };

    Ok(MysqlPhpMyAdminConfig {
        mysql_version: ver_out.stdout.trim().to_string(),
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
        pma_version,
        pma_path: "/phpadmin".to_string(),
        pma_port: 888,
        pma_ssl_only: false,
        pma_basic_auth: true,
        pma_max_upload_mb: 64,
        pma_allowed_ips: "0.0.0.0/0 (Protected by Panel Reverse Proxy)".to_string(),
        pma_status: if pma_running {
            "running".to_string()
        } else {
            "stopped".to_string()
        },
    })
}

pub async fn update_mysql_config(
    executor: &dyn CommandExecutor,
    my_cnf_raw: &str,
    cnf_path: Option<&Path>,
) -> Result<MysqlPhpMyAdminConfig, PanelError> {
    ensure_engine_installed(executor, DbEngineKind::MariaDb)?;
    if my_cnf_raw.contains('\0') {
        return Err(PanelError::Forbidden(
            "NUL byte in my.cnf is forbidden".to_string(),
        ));
    }
    let path = cnf_path.unwrap_or_else(|| Path::new(DEFAULT_MARIADB_CNF));
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
    get_mysql_config(executor, Some(path)).await
}
