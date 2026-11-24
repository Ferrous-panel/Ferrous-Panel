use crate::api::actor_and_ip;
use crate::error::PanelError;
use crate::modules::databases::{self as db_mod, MysqlConfigPatch};
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDatabaseRequest {
    pub name: String,
    pub engine: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub collation: String,
    #[serde(default)]
    pub linked_app_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeDbPasswordRequest {
    pub password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteQueryRequest {
    pub query: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSqlRequest {
    pub sql: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSqlQuery {
    #[serde(default)]
    pub download: Option<String>,
}

impl ExportSqlQuery {
    fn is_download(&self) -> bool {
        self.download
            .as_deref()
            .map(|v| {
                matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseRowsQuery {
    #[serde(default = "default_browse_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_browse_limit() -> usize {
    100
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMysqlConfigRequest {
    #[serde(default)]
    pub my_cnf_raw: Option<String>,
    #[serde(default)]
    pub pma_path: Option<String>,
    #[serde(default)]
    pub pma_port: Option<u16>,
    #[serde(default)]
    pub pma_ssl_only: Option<bool>,
    #[serde(default)]
    pub pma_basic_auth: Option<bool>,
    #[serde(default)]
    pub pma_max_upload_mb: Option<u32>,
    #[serde(default)]
    pub pma_allowed_ips: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSqlResponse {
    pub database_id: String,
    pub sql: String,
}

/// `GET /api/databases` — lists managed databases (SQLite 3 WAL Pure Rust, MariaDB 11.4, PostgreSQL 17).
pub async fn list_databases(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let dbs =
        db_mod::list_databases(&state.modules, state.webserver.executor().as_ref()).await?;
    Ok(HttpResponse::Ok().json(dbs))
}

/// `POST /api/databases` — creates a SQLite 3 (Pure Rust), MariaDB, or PostgreSQL database.
pub async fn create_database(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: web::Json<CreateDatabaseRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.into_inner();
    let dbs = db_mod::create_database(
        &state.modules,
        state.webserver.executor().as_ref(),
        &body.name,
        &body.engine,
        &body.username,
        body.password.as_deref(),
        &body.collation,
        &body.linked_app_id,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "database.create",
        &format!("name={} engine={}", body.name, body.engine),
    )?;
    Ok(HttpResponse::Ok().json(dbs))
}

/// `GET /api/databases/mysql-config` — reads Pure-Rust FerrousAdmin & MariaDB/MySQL configuration.
pub async fn get_mysql_config(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let cfg = db_mod::get_mysql_config_with_store(
        Some(&state.modules),
        state.webserver.executor().as_ref(),
        None,
        state.config.pma_port,
    )
    .await?;
    Ok(HttpResponse::Ok().json(cfg))
}

/// `PUT /api/databases/mysql-config` — updates Pure-Rust FerrousAdmin settings and/or MariaDB `50-server.cnf`.
pub async fn update_mysql_config(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: web::Json<UpdateMysqlConfigRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.into_inner();
    let cfg = db_mod::update_mysql_config_with_store(
        Some(&state.modules),
        state.webserver.executor().as_ref(),
        MysqlConfigPatch {
            my_cnf_raw: body.my_cnf_raw,
            pma_path: body.pma_path,
            pma_port: body.pma_port,
            pma_ssl_only: body.pma_ssl_only,
            pma_basic_auth: body.pma_basic_auth,
            pma_max_upload_mb: body.pma_max_upload_mb,
            pma_allowed_ips: body.pma_allowed_ips,
        },
        None,
        state.config.pma_port,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "database.mysql_config.update",
        "updated FerrousAdmin / MariaDB configuration",
    )?;
    Ok(HttpResponse::Ok().json(cfg))
}

/// `PUT /api/databases/{id}/password` — rotates SQL user password via CLI (never stored or logged).
pub async fn change_password(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: web::Json<ChangeDbPasswordRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let pw = payload.into_inner().password;
    let dbs = db_mod::change_database_password(
        &state.modules,
        state.webserver.executor().as_ref(),
        &id,
        &pw,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "database.password.rotate",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(dbs))
}

/// `POST /api/databases/{id}/query` — executes SQL via Pure-Rust `rusqlite`, `mysql`, or `psql`.
pub async fn execute_query(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: web::Json<ExecuteQueryRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let sql = payload.into_inner().query;
    let res = db_mod::execute_sql_query(
        &state.modules,
        state.webserver.executor().as_ref(),
        &id,
        &sql,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "database.query",
        &format!("id={id} tag={}", res.command_tag),
    )?;
    Ok(HttpResponse::Ok().json(res))
}

/// `GET /api/databases/{id}/tables/{table}/rows` — browses real paginated rows for a table.
pub async fn browse_table_rows(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
    query: web::Query<BrowseRowsQuery>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let (id, table) = path.into_inner();
    let q = query.into_inner();
    let res = db_mod::browse_table_rows(
        &state.modules,
        state.webserver.executor().as_ref(),
        &id,
        &table,
        q.limit,
        q.offset,
    )
    .await?;
    Ok(HttpResponse::Ok().json(res))
}

/// `DELETE /api/databases/{id}/tables/{table}` — drops a table inside the database.
pub async fn drop_table(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let (id, table) = path.into_inner();
    let dbs = db_mod::drop_table(
        &state.modules,
        state.webserver.executor().as_ref(),
        &id,
        &table,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "database.table.drop",
        &format!("id={id} table={table}"),
    )?;
    Ok(HttpResponse::Ok().json(dbs))
}

/// `POST /api/databases/{id}/tables/{table}/truncate` — truncates/empties a table.
pub async fn truncate_table(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let (id, table) = path.into_inner();
    let dbs = db_mod::truncate_table(
        &state.modules,
        state.webserver.executor().as_ref(),
        &id,
        &table,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "database.table.truncate",
        &format!("id={id} table={table}"),
    )?;
    Ok(HttpResponse::Ok().json(dbs))
}

/// `GET /api/databases/{id}/export` — exports full real `.sql` dump (Pure Rust SQLite / mysqldump / pg_dump).
/// Pass `?download=1` to receive a direct `application/sql` file attachment instead of a JSON envelope.
pub async fn export_database(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    query: web::Query<ExportSqlQuery>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let sql = db_mod::export_database_sql(
        &state.modules,
        state.webserver.executor().as_ref(),
        &id,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "database.export",
        &format!("id={id} bytes={}", sql.len()),
    )?;

    if query.into_inner().is_download() {
        let safe_filename: String = id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        let file_label = if safe_filename.is_empty() {
            "database".to_string()
        } else {
            safe_filename
        };
        return Ok(HttpResponse::Ok()
            .content_type("application/sql; charset=utf-8")
            .insert_header((
                "Content-Disposition",
                format!("attachment; filename=\"{file_label}.sql\""),
            ))
            .body(sql));
    }

    Ok(HttpResponse::Ok().json(ExportSqlResponse {
        database_id: id,
        sql,
    }))
}

/// `POST /api/databases/{id}/import` — imports a `.sql` script up to 64 MiB.
/// Accepts either JSON `{ "sql": "..." }` or a raw UTF-8 SQL request body (`text/plain`, `application/sql`).
pub async fn import_database(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Bytes,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();

    let sql = if content_type.contains("application/json")
        || (content_type.is_empty() && body.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{'))
    {
        let parsed: ImportSqlRequest = serde_json::from_slice(&body).map_err(|err| {
            PanelError::Validation(format!("invalid JSON SQL import payload: {err}"))
        })?;
        parsed.sql
    } else {
        std::str::from_utf8(&body)
            .map_err(|_| {
                PanelError::Validation("raw SQL import body must be valid UTF-8 text".to_string())
            })?
            .to_string()
    };

    let res = db_mod::import_database_sql(
        &state.modules,
        state.webserver.executor().as_ref(),
        &id,
        &sql,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "database.import",
        &format!("id={id} bytes={} success={}", sql.len(), res.success),
    )?;
    Ok(HttpResponse::Ok().json(res))
}

/// `DELETE /api/databases/{id}` — drops the database and removes metadata.
pub async fn delete_database(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let dbs = db_mod::delete_database(
        &state.modules,
        state.webserver.executor().as_ref(),
        &id,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state
        .auth_store
        .append_audit_log(Some(&actor), &ip, "database.delete", &format!("id={id}"))?;
    Ok(HttpResponse::Ok().json(dbs))
}
