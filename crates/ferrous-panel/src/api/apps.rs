use crate::api::contract::{
    AppActionRequest, AppConfigUpdateRequest, DeployWebsiteRequest, DirectoryPrivacyItem,
    HotlinkProtectionConfig, RedirectRuleItem,
};
use crate::auth::middleware::AuthenticatedUser;
use crate::error::PanelError;
use crate::modules::apps as apps_mod;
use crate::state::AppState;
use actix_web::{web, HttpMessage, HttpRequest, HttpResponse};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteSettingsRequest {
    #[serde(default)]
    pub ssl_enabled: Option<bool>,
    #[serde(default)]
    pub force_https: Option<bool>,
    #[serde(default)]
    pub http3_quic: Option<bool>,
    #[serde(default)]
    pub websocket_enabled: Option<bool>,
    #[serde(default)]
    pub rate_limit_rps: Option<u32>,
    #[serde(default)]
    pub redirects: Option<Vec<RedirectRuleItem>>,
    #[serde(default)]
    pub hotlink_protection: Option<HotlinkProtectionConfig>,
    #[serde(default)]
    pub directory_auth: Option<Vec<DirectoryPrivacyItem>>,
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

/// `GET /api/apps` — lists managed websites enriched with real systemd MainPID/MemoryCurrent, disk usage, and SSL cert status.
pub async fn list_apps(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let list = apps_mod::list_apps(&state.modules, &state.config, &state.webserver).await?;
    Ok(HttpResponse::Ok().json(list))
}

/// `POST /api/apps` — creates vhost, systemd unit (if runtime service), optional database, and persists metadata.
pub async fn deploy_app(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: web::Json<DeployWebsiteRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let (actor, ip) = actor_and_ip(&req);
    let body = payload.into_inner();
    let domain_label = body.domain.clone();
    let created = apps_mod::deploy_app(
        body,
        &state.modules,
        &state.config,
        &state.webserver,
    )
    .await?;

    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "app.deploy",
        &format!("deployed domain={domain_label} unit={}", created.systemd_unit),
    )?;
    Ok(HttpResponse::Ok().json(created))
}

/// `POST /api/apps/{id}/action` — executes real `systemctl {start|stop|restart}` on the app unit.
pub async fn app_action(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: web::Json<AppActionRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let action = payload.into_inner().action;
    let updated = apps_mod::app_action(
        &id,
        &action,
        &state.modules,
        &state.config,
        &state.webserver,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "app.action",
        &format!("id={id} action={action}"),
    )?;
    Ok(HttpResponse::Ok().json(updated))
}

/// `PUT /api/apps/{id}/config` — updates systemd `MemoryMax` + `Environment=` and reloads daemon.
pub async fn update_app_config(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: web::Json<AppConfigUpdateRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let body = payload.into_inner();
    let updated = apps_mod::update_app_config(
        &id,
        body.workers,
        body.memory_limit_mb,
        body.rate_limit_rps,
        body.ssl_enabled,
        body.http3_quic,
        body.websocket_enabled,
        body.env_vars,
        &state.modules,
        &state.config,
        &state.webserver,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "app.config.update",
        &format!("id={id} memoryLimitMb={}", updated.memory_limit_mb),
    )?;
    Ok(HttpResponse::Ok().json(updated))
}

/// `PUT /api/apps/{id}/site-settings` — applies supported `sslEnabled`/`forceHttps` site vhost updates.
pub async fn update_site_settings(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: web::Json<SiteSettingsRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let body = payload.into_inner();
    if body.http3_quic.unwrap_or(false)
        || body.websocket_enabled.unwrap_or(false)
        || body.rate_limit_rps.unwrap_or(0) > 0
        || body.redirects.as_ref().is_some_and(|r| !r.is_empty())
        || body
            .hotlink_protection
            .as_ref()
            .is_some_and(|h| h.enabled)
        || body.directory_auth.as_ref().is_some_and(|d| !d.is_empty())
    {
        return Err(PanelError::NotImplemented(
            "redirects, hotlinkProtection, directoryAuth, http3Quic, websocketEnabled, and rateLimitRps per-site rules are not supported yet",
        ));
    }

    let updated = apps_mod::update_site_settings(
        &id,
        body.ssl_enabled,
        body.force_https,
        &state.modules,
        &state.config,
        &state.webserver,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "app.site_settings.update",
        &format!(
            "id={id} sslEnabled={} forceHttps={}",
            updated.ssl_enabled, updated.force_https
        ),
    )?;
    Ok(HttpResponse::Ok().json(updated))
}

/// `POST /api/apps/{id}/test-http` — performs a real TCP HTTP `GET /` probe with `Host: <domain>`.
pub async fn test_http_live(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let result = apps_mod::test_http_live(&id, &state.modules, &state.webserver).await?;
    Ok(HttpResponse::Ok().json(result))
}

/// `POST /api/apps/{id}/renew-ssl` — runs real Certbot renewal and reloads the active engine.
pub async fn renew_ssl(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let updated = apps_mod::renew_ssl(
        &id,
        &state.modules,
        &state.config,
        &state.webserver,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "app.ssl.renew",
        &format!("id={id} domain={}", updated.domain),
    )?;
    Ok(HttpResponse::Ok().json(updated))
}

/// `DELETE /api/apps/{id}` — stops/disables systemd unit, removes vhost, and deletes metadata.
pub async fn delete_app(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let remaining = apps_mod::delete_app(
        &id,
        &state.modules,
        &state.config,
        &state.webserver,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state
        .auth_store
        .append_audit_log(Some(&actor), &ip, "app.delete", &format!("id={id}"))?;
    Ok(HttpResponse::Ok().json(remaining))
}
