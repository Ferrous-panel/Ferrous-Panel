use crate::auth::middleware::AuthenticatedUser;
use crate::error::PanelError;
use crate::modules::sysops as sysops_mod;
use crate::state::AppState;
use actix_web::{web, HttpMessage, HttpRequest, HttpResponse};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSignalRequest {
    #[serde(default = "default_term_signal")]
    pub signal: String,
}

fn default_term_signal() -> String {
    "TERM".to_string()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigureSwapRequest {
    pub size_mb: u64,
    #[serde(default)]
    pub swappiness: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceActionRequest {
    pub action: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceLogsQuery {
    #[serde(default = "default_log_lines")]
    pub lines: usize,
}

fn default_log_lines() -> usize {
    120
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

/// `GET /api/sysops/overview` — returns real `/proc` processes, `ss` ports, `df` disks, `systemd` services, Swap & sysctl.
pub async fn get_overview(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let overview = sysops_mod::get_sysops_overview(state.webserver.executor().as_ref()).await?;
    Ok(HttpResponse::Ok().json(overview))
}

/// `POST /api/sysops/processes/{pid}/signal` — sends SIGTERM/SIGKILL/SIGHUP with Anti-Lockout & Kernel Guards.
pub async fn signal_process(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<u32>,
    payload: web::Json<ProcessSignalRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let pid = path.into_inner();
    let sig = payload.into_inner().signal;
    let message =
        sysops_mod::signal_process(state.webserver.executor().as_ref(), pid, &sig).await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "sysops.process.signal",
        &format!("pid={pid} signal={sig}"),
    )?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "pid": pid,
        "signal": sig,
        "message": message,
    })))
}

/// `POST /api/sysops/swap` — allocates `/swapfile` (0600), activates `mkswap`+`swapon`, and updates `/etc/fstab`.
pub async fn configure_swap(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: web::Json<ConfigureSwapRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.into_inner();
    let overview = sysops_mod::configure_swap(
        state.webserver.executor().as_ref(),
        body.size_mb,
        body.swappiness,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "sysops.swap.configure",
        &format!("sizeMb={} swappiness={:?}", body.size_mb, body.swappiness),
    )?;
    Ok(HttpResponse::Ok().json(overview))
}

/// `DELETE /api/sysops/swap` — deactivates and deletes `/swapfile` and removes its `/etc/fstab` entry.
pub async fn remove_swap(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let overview = sysops_mod::remove_swap(state.webserver.executor().as_ref()).await?;

    let (actor, ip) = actor_and_ip(&req);
    state
        .auth_store
        .append_audit_log(Some(&actor), &ip, "sysops.swap.remove", "removed /swapfile")?;
    Ok(HttpResponse::Ok().json(overview))
}

/// `POST /api/sysops/services/{unit}/action` — runs `systemctl {start|stop|restart|reload|enable|disable}` with Anti-Lockout Guard.
pub async fn service_action(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: web::Json<ServiceActionRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let unit = path.into_inner();
    let action = payload.into_inner().action;
    let message =
        sysops_mod::service_action(state.webserver.executor().as_ref(), &unit, &action).await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "sysops.service.action",
        &format!("unit={unit} action={action}"),
    )?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "unit": unit,
        "action": action,
        "message": message,
    })))
}

/// `GET /api/sysops/services/{unit}/logs` — reads live `journalctl -u <unit>` logs.
pub async fn get_service_logs(
    state: web::Data<AppState>,
    path: web::Path<String>,
    query: web::Query<ServiceLogsQuery>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let unit = path.into_inner();
    let lines = query.into_inner().lines;
    let logs =
        sysops_mod::get_service_logs(state.webserver.executor().as_ref(), &unit, lines).await?;
    Ok(HttpResponse::Ok().json(logs))
}

/// `PUT /api/sysops/sysctl` — applies allowlisted kernel sysctl parameters live and persists them.
pub async fn update_sysctl(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: web::Json<sysops_mod::SysctlUpdatePatch>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let patch = payload.into_inner();
    let updated = sysops_mod::update_sysctl(state.webserver.executor().as_ref(), patch).await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "sysops.sysctl.update",
        &format!(
            "swappiness={} somaxconn={} cc={} fileMax={}",
            updated.swappiness,
            updated.somaxconn,
            updated.tcp_congestion_control,
            updated.file_max
        ),
    )?;
    Ok(HttpResponse::Ok().json(updated))
}
