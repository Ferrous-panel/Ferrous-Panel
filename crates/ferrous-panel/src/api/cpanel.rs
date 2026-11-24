use crate::api::actor_and_ip;
use crate::error::PanelError;
use crate::modules::cpanel as cpanel_mod;
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddDnsRequest {
    pub domain: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub host: String,
    pub value: String,
    #[serde(default = "default_dns_ttl")]
    pub ttl: u32,
    #[serde(default)]
    pub priority: Option<u16>,
}

const fn default_dns_ttl() -> u32 {
    300
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddSftpRequest {
    pub username: String,
    #[serde(default)]
    pub target_app: String,
    #[serde(default)]
    pub auth_method: String,
    #[serde(default = "default_sftp_quota")]
    pub quota_mb: u64,
}

const fn default_sftp_quota() -> u64 {
    2048
}

/// `POST /api/cpanel/dns` — creates a DNS zone record in SQLite and writes RFC 1035 zone file.
pub async fn add_dns(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<AddDnsRequest>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let Some(body) = payload.map(web::Json::into_inner) else {
        let records = cpanel_mod::list_dns_records(&state.modules)?;
        return Ok(HttpResponse::Ok().json(records));
    };
    let records = cpanel_mod::add_dns_record(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
        &body.domain,
        &body.record_type,
        &body.host,
        &body.value,
        body.ttl,
        body.priority,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "cpanel.dns.add",
        &format!("domain={} type={} host={}", body.domain, body.record_type, body.host),
    )?;
    Ok(HttpResponse::Ok().json(records))
}

/// `DELETE /api/cpanel/dns/{id}` — removes a DNS zone record and re-writes RFC 1035 zone file.
pub async fn delete_dns(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let records = cpanel_mod::delete_dns_record(
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
        "cpanel.dns.delete",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(records))
}

/// `POST /api/cpanel/sftp` — creates a chrooted SFTP account under `wwwroot`.
pub async fn add_sftp(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<AddSftpRequest>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let Some(body) = payload.map(web::Json::into_inner) else {
        let accounts = cpanel_mod::list_sftp_accounts(&state.modules, &state.config)?;
        return Ok(HttpResponse::Ok().json(accounts));
    };
    let accounts = cpanel_mod::add_sftp_account(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
        &body.username,
        &body.target_app,
        &body.auth_method,
        body.quota_mb,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "cpanel.sftp.add",
        &format!("username={} targetApp={}", body.username, body.target_app),
    )?;
    Ok(HttpResponse::Ok().json(accounts))
}

/// `DELETE /api/cpanel/sftp/{id}` — deletes an SFTP chroot account.
pub async fn delete_sftp(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let accounts = cpanel_mod::delete_sftp_account(
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
        "cpanel.sftp.delete",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(accounts))
}
