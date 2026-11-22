use crate::auth::middleware::AuthenticatedUser;
use crate::error::PanelError;
use crate::modules::databases::{self as db_mod, MysqlConfigPatch};
use crate::state::AppState;
use actix_web::{web, HttpMessage, HttpRequest, HttpResponse};
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

fn actor_and_ip(req: &HttpRequest) -> (String, String) {
    let actor = req
        .extensions()
        .get::<AuthenticatedUser>()
        .map(|u| u.username.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let ip = req
        .peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    (actor, ip)
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
pub async fn export_database(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
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
    Ok(HttpResponse::Ok().json(ExportSqlResponse {
        database_id: id,
        sql,
    }))
}

/// `POST /api/databases/{id}/import` — imports a `.sql` script into the database.
pub async fn import_database(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: web::Json<ImportSqlRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let sql = payload.into_inner().sql;
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
        &format!("id={id} success={}", res.success),
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
