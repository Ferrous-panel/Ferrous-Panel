use actix_web::{error::ErrorInternalServerError, Error};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlDatabaseSpec {
    pub id: String,
    pub name: String,
    pub engine: String, // "PostgreSQL 17" | "MariaDB 11.4" | "SQLite 3"
    pub port: u16,
    pub db_user: String,
    pub db_password: String,
    pub linked_app: String,
    pub charset: String,
    pub size_mb: f64,
    pub tables_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlQueryRequest {
    pub database_id: String,
    pub query: String,
}

pub struct DatabaseManager;

impl DatabaseManager {
    /// Creates a PostgreSQL 17 database and user with a custom password
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
