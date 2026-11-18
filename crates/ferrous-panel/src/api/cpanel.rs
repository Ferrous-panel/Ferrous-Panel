use crate::auth::middleware::AuthenticatedUser;
use crate::error::PanelError;
use crate::modules::cpanel as cpanel_mod;
use crate::state::AppState;
use actix_web::{web, HttpMessage, HttpRequest, HttpResponse};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddCronRequest {
    pub title: String,
    pub schedule: String,
    pub command: String,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub target_app: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBackupRequest {
    pub target_app: String,
    #[serde(default)]
    pub database_included: String,
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

/// `GET /api/cpanel` — loads real `crontab` jobs and verified `tar.zst` backups (DNS/SFTP empty & 501).
pub async fn get_cpanel(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let data = cpanel_mod::load_cpanel_overview(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/cpanel/cron` — validates schedule + allowlisted binary and installs entry into user crontab.
pub async fn add_cron(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: web::Json<AddCronRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.into_inner();
    let jobs = cpanel_mod::add_cron_job(
        &state.modules,
        state.webserver.executor().as_ref(),
        &body.title,
        &body.schedule,
        &body.command,
        &body.user,
        &body.target_app,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "cpanel.cron.add",
        &format!("title={} user={} schedule={}", body.title, body.user, body.schedule),
    )?;
    Ok(HttpResponse::Ok().json(jobs))
}

/// `POST /api/cpanel/cron/{id}/run` — immediately executes the allowlisted cron command (no `sh -c`).
pub async fn run_cron(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let jobs = cpanel_mod::run_cron_job_now(
        &state.modules,
        state.webserver.executor().as_ref(),
        &id,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "cpanel.cron.run",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(jobs))
}

/// `PUT /api/cpanel/cron/{id}/toggle` — enables/disables a managed entry inside crontab.
pub async fn toggle_cron(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let jobs = cpanel_mod::toggle_cron_job(
        &state.modules,
        state.webserver.executor().as_ref(),
        &id,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "cpanel.cron.toggle",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(jobs))
}

/// `DELETE /api/cpanel/cron/{id}` — removes a managed entry from crontab.
pub async fn delete_cron(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let jobs = cpanel_mod::delete_cron_job(
        &state.modules,
        state.webserver.executor().as_ref(),
        &id,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "cpanel.cron.delete",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(jobs))
}

/// `POST /api/cpanel/backups` — creates real `tar --zstd` archive (+ optional SQL dump) with real SHA-256 and disk size.
pub async fn create_backup(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: web::Json<CreateBackupRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.into_inner();
    let backups = cpanel_mod::create_backup(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
        &body.target_app,
        &body.database_included,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "cpanel.backup.create",
        &format!("targetApp={} db={}", body.target_app, body.database_included),
    )?;
    Ok(HttpResponse::Ok().json(backups))
}

/// `POST /api/cpanel/backups/{id}/restore` — verifies SHA-256 + traversal-safe tar listing and extracts archive.
pub async fn restore_backup(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let backups = cpanel_mod::restore_backup(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
        &id,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "cpanel.backup.restore",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(backups))
}

/// `DELETE /api/cpanel/backups/{id}` — removes backup archive from disk and SQLite.
pub async fn delete_backup(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let backups = cpanel_mod::delete_backup(&state.modules, &state.config, &id)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "cpanel.backup.delete",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(backups))
}
