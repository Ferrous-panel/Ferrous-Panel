use crate::api::actor_and_ip;
use crate::error::PanelError;
use crate::modules::security as sec_mod;
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddUfwRuleRequest {
    pub port: String,
    pub protocol: String,
    pub action: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub comment: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BanIpRequest {
    pub jail_name: String,
    pub ip: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnbanIpRequest {
    pub jail_name: String,
    pub ip: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogsQuery {
    #[serde(default = "default_audit_limit")]
    pub limit: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteUfwQuery {
    #[serde(default)]
    pub force: bool,
}

fn default_audit_limit() -> usize {
    100
}

/// `GET /api/security` — loads real `ufw status numbered`, `fail2ban-client status`, `sysctl` directives, and Anti-DDoS WAF/FIM state.
pub async fn get_security(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let data = sec_mod::load_security_overview(
        &state.modules,
        state.webserver.executor().as_ref(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/security/ufw` — adds a UFW rule using validated `PortOrRange` and argv `ufw` execution.
pub async fn add_ufw_rule(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: web::Json<AddUfwRuleRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.into_inner();
    let rules = sec_mod::add_ufw_rule(
        state.webserver.executor().as_ref(),
        &body.port,
        &body.protocol,
        &body.action,
        &body.source,
        &body.comment,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "security.ufw.add",
        &format!("port={} proto={} action={}", body.port, body.protocol, body.action),
    )?;
    Ok(HttpResponse::Ok().json(rules))
}

/// `PUT /api/security/ufw/{id}/toggle` — toggles per-rule enabled state.
pub async fn toggle_ufw_rule(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let rules = sec_mod::toggle_ufw_rule(
        &state.modules,
        state.webserver.executor().as_ref(),
        &id,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "security.ufw.toggle",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(rules))
}

/// `DELETE /api/security/ufw/{id}` — removes UFW numbered rule (`ufw --force delete <num>`),
/// guarding active SSH and panel ports unless `?force=true` is explicitly provided.
pub async fn delete_ufw_rule(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    query: web::Query<DeleteUfwQuery>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let force = query.into_inner().force;
    let ssh_port = crate::modules::extended::ssh_admin::load_ssh_panel(
        &state.modules,
        &state.config,
        &state.auth_store,
        state.webserver.executor().as_ref(),
    )
    .await
    .map(|o| o.config.port)
    .unwrap_or(22);
    let protected_ports = [ssh_port, state.config.port];
    let rules = sec_mod::delete_ufw_rule_guarded(
        state.webserver.executor().as_ref(),
        &id,
        &protected_ports,
        force,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "security.ufw.delete",
        &format!("id={id} force={force}"),
    )?;
    Ok(HttpResponse::Ok().json(rules))
}

/// `POST /api/security/fail2ban/ban` — bans an IP in the specified Fail2ban jail.
pub async fn ban_ip(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: web::Json<BanIpRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.into_inner();
    let jails = sec_mod::fail2ban_ban_unban(
        state.webserver.executor().as_ref(),
        &body.jail_name,
        &body.ip,
        true,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "security.fail2ban.ban",
        &format!("jail={} targetIp={}", body.jail_name, body.ip),
    )?;
    Ok(HttpResponse::Ok().json(jails))
}

/// `POST /api/security/fail2ban/unban` — unbans an IP in the specified Fail2ban jail.
pub async fn unban_ip(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: web::Json<UnbanIpRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.into_inner();
    let jails = sec_mod::fail2ban_ban_unban(
        state.webserver.executor().as_ref(),
        &body.jail_name,
        &body.ip,
        false,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "security.fail2ban.unban",
        &format!("jail={} targetIp={}", body.jail_name, body.ip),
    )?;
    Ok(HttpResponse::Ok().json(jails))
}

/// `PUT /api/security/hardening/{id}` — toggles a sysctl kernel hardening parameter via `sysctl -w`.
pub async fn toggle_hardening(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let directives =
        sec_mod::toggle_hardening(state.webserver.executor().as_ref(), &id).await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "security.hardening.toggle",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(directives))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DdosModeRequest {
    #[serde(default)]
    pub mode: String,
}

/// `POST /api/security/ddos-mode` — switches Anti-DDoS shield protection mode (`normal`, `elevated`, `under_attack`).
pub async fn set_ddos_mode(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<DdosModeRequest>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let mode = payload
        .map(|p| p.into_inner().mode)
        .unwrap_or_else(|| "under_attack".to_string());
    let shield = sec_mod::set_ddos_mode(
        &state.modules,
        state.webserver.executor().as_ref(),
        &mode,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "security.ddos_mode",
        &format!("mode={}", shield.protection_mode),
    )?;
    Ok(HttpResponse::Ok().json(shield))
}

/// `PUT /api/security/waf/{id}/toggle` — enables/disables a Layer-7 WAF rule.
pub async fn toggle_waf_rule(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let shield = sec_mod::toggle_waf_rule(&state.modules, &id)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "security.waf.toggle",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(shield))
}

/// `POST /api/security/fim-scan` — executes a real SHA-256 File Integrity Monitoring scan on a blocking worker thread.
pub async fn run_fim_scan(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let st = state.clone();
    let (scanned_files, sha256_root, shield) =
        web::block(move || sec_mod::run_fim_scan(&st.modules, &st.config))
            .await
            .map_err(|_| PanelError::Internal("FIM scan worker failed".to_string()))??;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "security.fim.scan",
        &format!("scannedFiles={scanned_files} sha256={sha256_root}"),
    )?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "verified",
        "scannedFiles": scanned_files,
        "sha256Root": sha256_root,
        "antiDdos": shield,
        "antiDdosShield": shield,
    })))
}

/// `GET /api/security/audit-logs?limit=100` — lists newest persisted security audit log entries.
pub async fn get_audit_logs(
    state: web::Data<AppState>,
    query: web::Query<AuditLogsQuery>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let limit = query.into_inner().limit;
    let st = state.clone();
    let logs = web::block(move || st.auth_store.list_audit_logs(limit))
        .await
        .map_err(|_| PanelError::Internal("audit log query worker failed".to_string()))??;
    Ok(HttpResponse::Ok().json(logs))
}
