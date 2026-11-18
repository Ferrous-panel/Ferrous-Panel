use crate::auth::middleware::AuthenticatedUser;
use crate::error::PanelError;
use crate::modules::databases as db_mod;
use crate::state::AppState;
use actix_web::{web, HttpMessage, HttpRequest, HttpResponse};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDatabaseRequest {
    pub name: String,
    pub engine: String,
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
pub struct UpdateMysqlConfigRequest {
    pub my_cnf_raw: String,
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

/// `GET /api/databases` — lists managed databases with real sizes from `information_schema` / `pg_database_size`.
pub async fn list_databases(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let dbs =
        db_mod::list_databases(&state.modules, state.webserver.executor().as_ref()).await?;
    Ok(HttpResponse::Ok().json(dbs))
}

/// `POST /api/databases` — creates a database and user via `mysql`/`psql` CLI (never stores or logs password).
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

/// `GET /api/databases/mysql-config` — reads real MariaDB/MySQL configuration.
pub async fn get_mysql_config(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let cfg =
        db_mod::get_mysql_config(state.webserver.executor().as_ref(), None).await?;
    Ok(HttpResponse::Ok().json(cfg))
}

/// `PUT /api/databases/mysql-config` — atomically writes `50-server.cnf` and reloads `mariadb`.
pub async fn update_mysql_config(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: web::Json<UpdateMysqlConfigRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.into_inner();
    let cfg = db_mod::update_mysql_config(
        state.webserver.executor().as_ref(),
        &body.my_cnf_raw,
        None,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "database.mysql_config.update",
        "updated MariaDB server configuration",
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

/// `POST /api/databases/{id}/query` — executes a SQL query via `mysql`/`psql` and parses real TSV results.
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
