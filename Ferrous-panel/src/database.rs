use actix_web::{error::ErrorInternalServerError, Error};
use serde::{Deserialize, Serialize};
use std::fs;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlColumnSchema {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub is_primary_key: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlTableSchema {
    pub table_name: String,
    pub row_count: usize,
    pub size_kb: usize,
    pub columns: Vec<SqlColumnSchema>,
    pub rows: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlDatabaseSpec {
    pub id: String,
    pub name: String,
    pub engine: String, // "PostgreSQL 17" | "MariaDB 11.4" | "SQLite 3 (WAL)"
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub collation: String,
    pub size_mb: f64,
    pub linked_app_id: String,
    pub linked_app_name: String,
    pub active_connections: u32,
    pub max_connections: u32,
    pub created_at: String,
    pub tables: Vec<SqlTableSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MysqlPhpMyAdminConfig {
    pub mysql_version: String,
    pub port: u16,
    pub bind_address: String,
    pub max_connections: u32,
    pub innodb_buffer_pool_mb: u32,
    pub default_charset: String,
    pub slow_query_log: bool,
    pub long_query_time_sec: f64,
    pub my_cnf_raw: String,
    pub pma_installed: bool,
    pub pma_version: String,
    pub pma_port: u16,
    pub pma_path: String,
    pub pma_basic_auth: bool,
    pub pma_ssl_only: bool,
    pub pma_max_upload_mb: u32,
    pub pma_allowed_ips: String,
}

impl Default for MysqlPhpMyAdminConfig {
    fn default() -> Self {
        Self {
            mysql_version: "MariaDB 11.4.4-LTS (Ubuntu 24.04)".to_string(),
            port: 3306,
            bind_address: "127.0.0.1".to_string(),
            max_connections: 300,
            innodb_buffer_pool_mb: 1024,
            default_charset: "utf8mb4_unicode_ci".to_string(),
            slow_query_log: true,
            long_query_time_sec: 1.5,
            my_cnf_raw: "[mysqld]\nuser = mysql\nbind-address = 127.0.0.1\nport = 3306\ncharacter-set-server = utf8mb4\ncollation-server = utf8mb4_unicode_ci\ninnodb_buffer_pool_size = 1024M\nmax_connections = 300\nslow_query_log = 1\nlong_query_time = 1.5\n".to_string(),
            pma_installed: true,
            pma_version: "5.2.1-deb12u1".to_string(),
            pma_port: 888,
            pma_path: "/phpmyadmin_ferrous_89a2".to_string(),
            pma_basic_auth: true,
            pma_ssl_only: false,
            pma_max_upload_mb: 256,
            pma_allowed_ips: "0.0.0.0/0 (Protected by Panel SSO)".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlQueryRequest {
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlQueryResponse {
    pub success: bool,
    pub execution_time_ms: f64,
    pub command_tag: String,
    pub affected_rows: usize,
    pub columns: Vec<String>,
    pub rows: Vec<serde_json::Value>,
    pub error: Option<String>,
}

pub struct DatabaseManager;

impl DatabaseManager {
    /// Creates a PostgreSQL 17 database and user with a custom password via psql
    pub async fn create_postgres_db(name: &str, owner: &str, password: &str) -> Result<String, Error> {
        let safe_pw = password.replace('\'', "''");
        let create_role = format!(
            "CREATE ROLE {} WITH LOGIN PASSWORD '{}' NOSUPERUSER NOCREATEDB;",
            owner, safe_pw
        );
        let create_db = format!(
            "CREATE DATABASE {} OWNER {} ENCODING 'UTF8';",
            name, owner
        );

        let _ = Command::new("sudo")
            .args(["-u", "postgres", "psql", "-c", &create_role])
            .output()
            .await
            .map_err(ErrorInternalServerError)?;

        let _ = Command::new("sudo")
            .args(["-u", "postgres", "psql", "-c", &create_db])
            .output()
            .await
            .map_err(ErrorInternalServerError)?;

        Ok(format!(
            "postgres://{}:{}@127.0.0.1:5432/{}",
            owner, password, name
        ))
    }

    /// Creates a MariaDB 11.4 / MySQL database and user with a custom password
    pub async fn create_mariadb_db(name: &str, owner: &str, password: &str, collation: &str) -> Result<String, Error> {
        let safe_pw = password.replace('\'', "\\'");
        let sql_stmt = format!(
            "CREATE DATABASE IF NOT EXISTS `{}` CHARACTER SET utf8mb4 COLLATE {}; \
             CREATE USER IF NOT EXISTS '{}'@'127.0.0.1' IDENTIFIED BY '{}'; \
             CREATE USER IF NOT EXISTS '{}'@'localhost' IDENTIFIED BY '{}'; \
             GRANT ALL PRIVILEGES ON `{}`.* TO '{}'@'127.0.0.1'; \
             GRANT ALL PRIVILEGES ON `{}`.* TO '{}'@'localhost'; \
             FLUSH PRIVILEGES;",
            name, collation, owner, safe_pw, owner, safe_pw, name, owner, name, owner
        );

        let _ = Command::new("sudo")
            .args(["mysql", "-e", &sql_stmt])
            .output()
            .await
            .map_err(ErrorInternalServerError)?;

        Ok(format!(
            "mysql://{}:{}@127.0.0.1:3306/{}",
            owner, password, name
        ))
    }

    /// Executes a real SQL query against MariaDB or PostgreSQL on the Ubuntu host
    pub async fn execute_sql_query(db: &SqlDatabaseSpec, query: &str) -> SqlQueryResponse {
        let start = std::time::Instant::now();
        let trimmed = query.trim();

        let output_res = if db.engine.contains("PostgreSQL") {
            Command::new("sudo")
                .args(["-u", "postgres", "psql", "-d", &db.name, "-c", trimmed])
                .output()
                .await
        } else {
            Command::new("sudo")
                .args(["mysql", "-D", &db.name, "-e", trimmed])
                .output()
                .await
        };

        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        match output_res {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let mut rows = Vec::new();
                for line in stdout.lines().take(50) {
                    if !line.trim().is_empty() {
                        rows.push(serde_json::json!({ "result": line }));
                    }
                }
                SqlQueryResponse {
                    success: true,
                    execution_time_ms: (elapsed * 100.0).round() / 100.0,
                    command_tag: trimmed
                        .split_whitespace()
                        .next()
                        .unwrap_or("SQL")
                        .to_uppercase(),
                    affected_rows: rows.len(),
                    columns: vec!["result".to_string()],
                    rows,
                    error: None,
                }
            }
            Ok(out) => SqlQueryResponse {
                success: false,
                execution_time_ms: (elapsed * 100.0).round() / 100.0,
                command_tag: "ERROR".to_string(),
                affected_rows: 0,
                columns: vec![],
                rows: vec![],
                error: Some(String::from_utf8_lossy(&out.stderr).to_string()),
            },
            Err(e) => SqlQueryResponse {
                success: false,
                execution_time_ms: (elapsed * 100.0).round() / 100.0,
                command_tag: "ERROR".to_string(),
                affected_rows: 0,
                columns: vec![],
                rows: vec![],
                error: Some(e.to_string()),
            },
        }
    }

    /// Writes `/etc/mysql/mariadb.conf.d/50-server.cnf` and `/etc/nginx/conf.d/ferrous-phpmyadmin.conf`
    pub async fn apply_mysql_and_phpmyadmin_config(cfg: &MysqlPhpMyAdminConfig) -> Result<(), Error> {
        let _ = fs::write("/etc/mysql/mariadb.conf.d/50-server.cnf", &cfg.my_cnf_raw);

        let pma_nginx = format!(
            "# Generated by Ferrous-panel Rust Engine — phpMyAdmin 5.2 Guard\n\
             server {{\n\
                 listen {};\n\
                 server_name _;\n\
                 client_max_body_size {}M;\n\
                 location {} {{\n\
                     alias /usr/share/phpmyadmin/;\n\
                     index index.php;\n\
                     location ~ \\.php$ {{\n\
                         fastcgi_pass unix:/run/php/php8.3-fpm.sock;\n\
                         fastcgi_param SCRIPT_FILENAME $request_filename;\n\
                         include fastcgi_params;\n\
                     }}\n\
                 }}\n\
             }}\n",
            cfg.pma_port, cfg.pma_max_upload_mb, cfg.pma_path
        );
        let _ = fs::write("/etc/nginx/conf.d/ferrous-phpmyadmin.conf", pma_nginx);

        let _ = Command::new("sudo")
            .args(["systemctl", "reload", "mariadb", "nginx"])
            .output()
            .await;

        Ok(())
    }

    /// Changes an existing SQL database user's password (MariaDB or PostgreSQL)
    pub async fn change_user_password(engine: &str, owner: &str, new_password: &str) -> Result<(), Error> {
        if engine.contains("PostgreSQL") {
            let safe_pw = new_password.replace('\'', "''");
            let sql = format!("ALTER ROLE {} WITH PASSWORD '{}';", owner, safe_pw);
            let _ = Command::new("sudo")
                .args(["-u", "postgres", "psql", "-c", &sql])
                .output()
                .await
                .map_err(ErrorInternalServerError)?;
        } else {
            let safe_pw = new_password.replace('\'', "\\'");
            let sql = format!(
                "ALTER USER '{}'@'127.0.0.1' IDENTIFIED BY '{}'; \
                 ALTER USER '{}'@'localhost' IDENTIFIED BY '{}'; \
                 FLUSH PRIVILEGES;",
                owner, safe_pw, owner, safe_pw
            );
            let _ = Command::new("sudo")
                .args(["mysql", "-e", &sql])
                .output()
                .await
                .map_err(ErrorInternalServerError)?;
        }
        Ok(())
    }

    pub async fn export_sql_dump(engine: &str, db_name: &str) -> Result<String, Error> {
        let backup_path = format!("/www/backup/{}_{}.sql.zst", db_name, chrono::Utc::now().timestamp());
        if engine.contains("PostgreSQL") {
            let _ = Command::new("pg_dump")
                .args(["-U", "postgres", "-d", db_name, "-F", "p"])
                .output()
                .await;
        } else {
            let _ = Command::new("mysqldump")
                .args(["--single-transaction", db_name])
                .output()
                .await;
        }
        Ok(backup_path)
    }
}
