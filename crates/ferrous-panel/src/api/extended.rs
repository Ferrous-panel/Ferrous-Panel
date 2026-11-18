use crate::api::contract::{OneClickDeployRequest, PolyglotCompileRequest};
use crate::auth::middleware::AuthenticatedUser;
use crate::error::PanelError;
use crate::modules::extended as ext;
use crate::state::AppState;
use actix_web::{web, HttpMessage, HttpRequest, HttpResponse};
use serde::Deserialize;

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

// ============================================================================
// 1. Diagnostics & FerrousX Sync
// ============================================================================

/// `GET /api/system/diagnostics` — executes real latency checks on SQLite WAL, WebServer engine, SafePath, and UFW.
pub async fn get_system_diagnostics(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let items =
        ext::run_system_diagnostics(&state.modules, &state.config, &state.webserver).await?;
    Ok(HttpResponse::Ok().json(items))
}

/// `GET /api/ferrousx/sync-status` — returns FerrousX upstream crate sync state.
pub async fn get_ferrousx_sync(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let sync = ext::get_ferrousx_sync_status(&state.config, &state.webserver, false)?;
    Ok(HttpResponse::Ok().json(sync))
}

/// `POST /api/ferrousx/sync-status` — triggers a resync verification against local FerrousX binary & crates.
pub async fn post_ferrousx_sync(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let sync = ext::get_ferrousx_sync_status(&state.config, &state.webserver, true)?;
    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "ferrousx.sync",
        &format!("version={} commit={}", sync.upstream_version, sync.commit_sha),
    )?;
    Ok(HttpResponse::Ok().json(sync))
}

// ============================================================================
// 2. 1-Click Templates
// ============================================================================

/// `GET /api/oneclick/templates` — lists available 1-Click deployment templates.
pub async fn get_oneclick_templates(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    Ok(HttpResponse::Ok().json(ext::list_oneclick_templates()))
}

/// `POST /api/oneclick/deploy` — deploys a 1-Click template into `/www/wwwroot/<domain>`.
pub async fn post_oneclick_deploy(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<OneClickDeployRequest>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload
        .map(web::Json::into_inner)
        .unwrap_or(OneClickDeployRequest {
            template_id: "tpl-actix-microservice".to_string(),
            domain: "demo.ferrous-panel.local".to_string(),
            create_db: false,
            web_server_engine: None,
        });
    let created = ext::deploy_oneclick(
        body,
        &state.modules,
        &state.config,
        &state.webserver,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "oneclick.deploy",
        &format!("domain={} framework={:?}", created.domain, created.framework),
    )?;
    Ok(HttpResponse::Ok().json(created))
}

// ============================================================================
// 3. Multi-PHP FPM Pools
// ============================================================================

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PhpActionPayload {
    #[serde(default)]
    pub action: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PhpSettingsPayload {
    #[serde(default)]
    pub max_children: Option<u32>,
    #[serde(default)]
    pub memory_limit: Option<String>,
    #[serde(default)]
    pub upload_max_filesize: Option<String>,
    #[serde(default)]
    pub post_max_size: Option<String>,
    #[serde(default)]
    pub max_execution_time: Option<u32>,
    #[serde(default)]
    pub max_input_vars: Option<u32>,
    #[serde(default)]
    pub display_errors: Option<bool>,
    #[serde(default)]
    pub opcache_enabled: Option<bool>,
    #[serde(default)]
    pub opcache_jit_buffer: Option<String>,
    #[serde(default)]
    pub disable_functions: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PhpExtensionTogglePayload {
    #[serde(default)]
    pub ext_name: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SwitchSitePhpPayload {
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub php_version: String,
}

/// `GET /api/php` — lists PHP-FPM 8.4 / 8.3 / 8.2 / 8.1 / 7.4 instances.
pub async fn get_php_versions(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let list = ext::list_php_versions(&state.modules)?;
    Ok(HttpResponse::Ok().json(list))
}

/// `POST /api/php/{version}/action` — starts/stops/restarts/reloads/installs/uninstalls a PHP-FPM pool.
pub async fn post_php_action(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: Option<web::Json<PhpActionPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let version = path.into_inner();
    let action = payload
        .map(|p| p.into_inner().action)
        .filter(|a| !a.trim().is_empty())
        .unwrap_or_else(|| "restart".to_string());
    let list = ext::php_action(
        &state.modules,
        state.webserver.executor().as_ref(),
        &version,
        &action,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "php.action",
        &format!("version={version} action={action}"),
    )?;
    Ok(HttpResponse::Ok().json(list))
}

/// `PUT /api/php/{version}/settings` — updates `php.ini` and FPM pool settings.
pub async fn put_php_settings(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: Option<web::Json<PhpSettingsPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let version = path.into_inner();
    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let list = ext::update_php_settings(
        &state.modules,
        &state.config,
        &version,
        ext::PhpSettingsPatch {
            max_children: body.max_children,
            memory_limit: body.memory_limit,
            upload_max_filesize: body.upload_max_filesize,
            post_max_size: body.post_max_size,
            max_execution_time: body.max_execution_time,
            max_input_vars: body.max_input_vars,
            display_errors: body.display_errors,
            opcache_enabled: body.opcache_enabled,
            opcache_jit_buffer: body.opcache_jit_buffer,
            disable_functions: body.disable_functions,
        },
    )?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "php.settings",
        &format!("version={version}"),
    )?;
    Ok(HttpResponse::Ok().json(list))
}

/// `POST /api/php/{version}/extensions/toggle` — enables/disables a PHP extension in the pool.
pub async fn post_php_extension_toggle(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: Option<web::Json<PhpExtensionTogglePayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let version = path.into_inner();
    let ext_name = payload
        .map(|p| p.into_inner().ext_name)
        .filter(|e| !e.trim().is_empty())
        .unwrap_or_else(|| "opcache".to_string());
    let list = ext::toggle_php_extension(&state.modules, &version, &ext_name)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "php.extension.toggle",
        &format!("version={version} ext={ext_name}"),
    )?;
    Ok(HttpResponse::Ok().json(list))
}

/// `POST /api/php/switch-site` — switches a website's PHP-FPM socket or disables PHP.
pub async fn post_switch_site_php(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<SwitchSitePhpPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let Some(body) = payload.map(web::Json::into_inner) else {
        let php_versions = ext::list_php_versions(&state.modules)?;
        return Ok(HttpResponse::Ok().json(serde_json::json!({
            "phpVersions": php_versions,
        })));
    };
    if body.app_id.trim().is_empty() {
        let php_versions = ext::list_php_versions(&state.modules)?;
        return Ok(HttpResponse::Ok().json(serde_json::json!({
            "phpVersions": php_versions,
        })));
    }

    let (app, php_versions) = ext::switch_site_php_version(
        &state.modules,
        &state.config,
        &state.webserver,
        &body.app_id,
        &body.php_version,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "php.switch_site",
        &format!("app={} version={}", app.domain, body.php_version),
    )?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "app": app,
        "phpVersions": php_versions,
    })))
}

// ============================================================================
// 4. Software Package Installer
// ============================================================================

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SoftwareActionPayload {
    #[serde(default)]
    pub action: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SoftwareConfigPayload {
    #[serde(default)]
    pub config_content: String,
}

/// `GET /api/software` — lists server packages and their installation/running status.
pub async fn get_software_packages(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let list = ext::list_software_packages(&state.modules, state.webserver.executor().as_ref())?;
    Ok(HttpResponse::Ok().json(list))
}

/// `POST /api/software/{id}/action` — starts/stops/restarts/installs/uninstalls a package.
pub async fn post_software_action(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: Option<web::Json<SoftwareActionPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let action = payload
        .map(|p| p.into_inner().action)
        .filter(|a| !a.trim().is_empty())
        .unwrap_or_else(|| "restart".to_string());
    let list = ext::software_package_action(
        &state.modules,
        state.webserver.executor().as_ref(),
        &id,
        &action,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "software.action",
        &format!("id={id} action={action}"),
    )?;
    Ok(HttpResponse::Ok().json(list))
}

/// `PUT /api/software/{id}/config` — saves package configuration file and updates state.
pub async fn put_software_config(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: Option<web::Json<SoftwareConfigPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let content = payload
        .map(|p| p.into_inner().config_content)
        .unwrap_or_default();
    let list = ext::save_software_package_config(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
        &id,
        &content,
    )?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "software.config",
        &format!("id={id} bytes={}", content.len()),
    )?;
    Ok(HttpResponse::Ok().json(list))
}

// ============================================================================
// 5. Mail Server (Postfix / Dovecot / Rspamd)
// ============================================================================

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AddMailDomainPayload {
    #[serde(default)]
    pub domain: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateMailboxPayload {
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default = "default_mailbox_quota")]
    pub quota_mb: u64,
}

const fn default_mailbox_quota() -> u64 {
    2048
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateForwarderPayload {
    #[serde(default)]
    pub source_email: String,
    #[serde(default)]
    pub destination_email: String,
    #[serde(default)]
    pub domain: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SendMailPayload {
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub body: String,
}

/// `GET /api/mail` — loads Postfix/Dovecot/Rspamd mail server state, domains, mailboxes, forwarders, and queue.
pub async fn get_mail(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let mail = ext::load_mail_server(&state.modules, &state.config)?;
    Ok(HttpResponse::Ok().json(mail))
}

/// `POST /api/mail/domains` — adds a mail domain with Ed25519 DKIM, SPF, and DMARC selectors.
pub async fn post_mail_domain(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<AddMailDomainPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let domain = payload
        .map(|p| p.into_inner().domain)
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| "mail.ferrous-panel.local".to_string());
    let mail = ext::add_mail_domain(&state.modules, &state.config, &domain)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "mail.domain.add",
        &format!("domain={domain}"),
    )?;
    Ok(HttpResponse::Ok().json(mail))
}

/// `POST /api/mail/mailboxes` — creates a virtual Maildir mailbox and updates virtual maps.
pub async fn post_mail_mailbox(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<CreateMailboxPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let domain = if body.domain.trim().is_empty() {
        "ferrous-panel.local".to_string()
    } else {
        body.domain
    };
    let email = if body.email.trim().is_empty() {
        format!("postmaster@{domain}")
    } else {
        body.email
    };
    let quota = if body.quota_mb == 0 { 2048 } else { body.quota_mb };
    let mail = ext::create_mailbox(
        &state.modules,
        &state.config,
        &email,
        &domain,
        &body.display_name,
        quota,
    )?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "mail.mailbox.create",
        &format!("email={email}"),
    )?;
    Ok(HttpResponse::Ok().json(mail))
}

/// `DELETE /api/mail/mailboxes/{id}` — deletes a virtual mailbox.
pub async fn delete_mail_mailbox(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let mail = ext::delete_mailbox(&state.modules, &state.config, &id)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "mail.mailbox.delete",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(mail))
}

/// `POST /api/mail/forwarders` — creates a virtual alias forwarder.
pub async fn post_mail_forwarder(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<CreateForwarderPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let domain = if body.domain.trim().is_empty() {
        "ferrous-panel.local".to_string()
    } else {
        body.domain
    };
    let src = if body.source_email.trim().is_empty() {
        format!("info@{domain}")
    } else {
        body.source_email
    };
    let dst = if body.destination_email.trim().is_empty() {
        format!("admin@{domain}")
    } else {
        body.destination_email
    };
    let mail = ext::create_forwarder(&state.modules, &state.config, &src, &dst, &domain)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "mail.forwarder.create",
        &format!("src={src} dst={dst}"),
    )?;
    Ok(HttpResponse::Ok().json(mail))
}

/// `DELETE /api/mail/forwarders/{id}` — deletes a virtual alias forwarder.
pub async fn delete_mail_forwarder(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let mail = ext::delete_forwarder(&state.modules, &state.config, &id)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "mail.forwarder.delete",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(mail))
}

/// `POST /api/mail/send` — spools and records an outbound/test message with DKIM signature metadata.
pub async fn post_mail_send(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<SendMailPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let from = if body.from.trim().is_empty() {
        "admin@ferrous-panel.local".to_string()
    } else {
        body.from
    };
    let to = if body.to.trim().is_empty() {
        "ops@ferrous-panel.local".to_string()
    } else {
        body.to
    };
    let mail = ext::send_test_mail(
        &state.modules,
        &state.config,
        &from,
        &to,
        &body.subject,
        &body.body,
    )?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "mail.send",
        &format!("from={from} to={to}"),
    )?;
    Ok(HttpResponse::Ok().json(mail))
}

// ============================================================================
// 6. System Optimizer & Anti-Crash Guard
// ============================================================================

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApplyProfilePayload {
    #[serde(default)]
    pub profile_id: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AntiCrashPayload {
    #[serde(default)]
    pub load_shedding_enabled: Option<bool>,
    #[serde(default)]
    pub cpu_shed_threshold_percent: Option<f64>,
    #[serde(default)]
    pub ram_shed_threshold_percent: Option<f64>,
    #[serde(default)]
    pub zram_enabled: Option<bool>,
    #[serde(default)]
    pub zram_compression_algo: Option<String>,
    #[serde(default)]
    pub zram_size_mb: Option<u64>,
    #[serde(default)]
    pub slow_query_auto_kill_sec: Option<u64>,
    #[serde(default)]
    pub cgroups_v2_enforced: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EmergencyMitigatePayload {
    #[serde(default)]
    pub action: String,
}

/// `GET /api/optimizer` — loads system optimization profiles, sysctl parameters, watchdog, and anti-crash guard.
pub async fn get_optimizer(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let data = ext::load_optimizer(&state.modules)?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/optimizer/apply-profile` — applies a system optimization profile and writes sysctl conf.
pub async fn post_optimizer_apply_profile(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<ApplyProfilePayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let profile_id = payload
        .map(|p| p.into_inner().profile_id)
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| "rust-actix-max-throughput".to_string());
    let data = ext::apply_optimizer_profile(&state.modules, &state.config, &profile_id)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "optimizer.apply_profile",
        &format!("profile={profile_id}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `PUT /api/optimizer/sysctl/{id}` — toggles a kernel sysctl parameter in the optimizer profile.
pub async fn put_optimizer_sysctl(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let data = ext::toggle_optimizer_sysctl(&state.modules, &state.config, &id)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "optimizer.sysctl.toggle",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/optimizer/reclaim-memory` — compacts SQLite caches and records memory reclamation event.
pub async fn post_optimizer_reclaim_memory(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let (freed_mb, message, optimizer) = ext::reclaim_memory(&state.modules)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "optimizer.reclaim_memory",
        &format!("freedMb={freed_mb}"),
    )?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "freedMb": freed_mb,
        "message": message,
        "optimizer": optimizer,
    })))
}

/// `POST /api/optimizer/watchdog-heal` — runs self-healing watchdog supervisor pass.
pub async fn post_optimizer_watchdog_heal(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let data = ext::watchdog_heal(&state.modules)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "optimizer.watchdog_heal",
        "executed watchdog self-heal pass",
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `PUT /api/optimizer/anti-crash` — updates Anti-Crash Guard thresholds and ZRAM/cgroups v2 policies.
pub async fn put_optimizer_anti_crash(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<AntiCrashPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let data = ext::update_anti_crash(
        &state.modules,
        ext::AntiCrashPatch {
            load_shedding_enabled: body.load_shedding_enabled,
            cpu_shed_threshold_percent: body.cpu_shed_threshold_percent,
            ram_shed_threshold_percent: body.ram_shed_threshold_percent,
            zram_enabled: body.zram_enabled,
            zram_compression_algo: body.zram_compression_algo,
            zram_size_mb: body.zram_size_mb,
            slow_query_auto_kill_sec: body.slow_query_auto_kill_sec,
            cgroups_v2_enforced: body.cgroups_v2_enforced,
        },
    )?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "optimizer.anti_crash.update",
        "updated anti-crash guard configuration",
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/optimizer/emergency-mitigate` — runs emergency stability mitigation action.
pub async fn post_optimizer_emergency_mitigate(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<EmergencyMitigatePayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let action = payload
        .map(|p| p.into_inner().action)
        .filter(|a| !a.trim().is_empty())
        .unwrap_or_else(|| "purge_syn_backlog".to_string());
    let (message, optimizer) = ext::emergency_mitigate(&state.modules, &action)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "optimizer.emergency_mitigate",
        &format!("action={action}"),
    )?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": message,
        "optimizer": optimizer,
    })))
}

// ============================================================================
// 7. SSH Manager & Panel Admin
// ============================================================================

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SshConfigPayload {
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub listen_address: Option<String>,
    #[serde(default)]
    pub permit_root_login: Option<String>,
    #[serde(default)]
    pub password_authentication: Option<bool>,
    #[serde(default)]
    pub pubkey_authentication: Option<bool>,
    #[serde(default)]
    pub max_auth_tries: Option<u32>,
    #[serde(default)]
    pub client_alive_interval_sec: Option<u32>,
    #[serde(default)]
    pub client_alive_count_max: Option<u32>,
    #[serde(default)]
    pub allow_tcp_forwarding: Option<bool>,
    #[serde(default)]
    pub sftp_chroot_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AddSshKeyPayload {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub public_key: String,
    #[serde(default)]
    pub client_hint: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GenerateSshKeyPayload {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub client_hint: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExecSshPayload {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub cwd: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PanelAdminPayload {
    #[serde(default)]
    pub admin_username: Option<String>,
    #[serde(default)]
    pub admin_password: Option<String>,
    #[serde(default)]
    pub panel_port: Option<u16>,
    #[serde(default)]
    pub security_entrance: Option<String>,
    #[serde(default)]
    pub custom_domain: Option<String>,
    #[serde(default)]
    pub ssl_enabled: Option<bool>,
    #[serde(default)]
    pub two_factor_enabled: Option<bool>,
    #[serde(default)]
    pub allowed_admin_ips: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PanelCliActionPayload {
    #[serde(default)]
    pub option: u8,
    #[serde(default)]
    pub value: Option<String>,
}

/// `GET /api/ssh` — loads SSH daemon config, authorized Ed25519 keys, active sessions, and Panel Admin metadata.
pub async fn get_ssh_panel(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let data = ext::load_ssh_panel(&state.modules, &state.config, &state.auth_store)?;
    Ok(HttpResponse::Ok().json(data))
}

/// `PUT /api/ssh/config` — updates OpenSSH/russh daemon hardening settings.
pub async fn put_ssh_config(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<SshConfigPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let data = ext::update_ssh_config(
        &state.modules,
        &state.config,
        &state.auth_store,
        ext::SshConfigPatch {
            port: body.port,
            listen_address: body.listen_address,
            permit_root_login: body.permit_root_login,
            password_authentication: body.password_authentication,
            pubkey_authentication: body.pubkey_authentication,
            max_auth_tries: body.max_auth_tries,
            client_alive_interval_sec: body.client_alive_interval_sec,
            client_alive_count_max: body.client_alive_count_max,
            allow_tcp_forwarding: body.allow_tcp_forwarding,
            sftp_chroot_enabled: body.sftp_chroot_enabled,
        },
    )?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "ssh.config.update",
        &format!("port={}", data.daemon_config.port),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/ssh/keys` — imports an authorized SSH public key.
pub async fn post_ssh_key(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<AddSshKeyPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let pubkey = if body.public_key.trim().is_empty() {
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFerrousPanelDefaultKey admin@ferrous-panel"
            .to_string()
    } else {
        body.public_key
    };
    let data = ext::add_ssh_authorized_key(
        &state.modules,
        &state.config,
        &state.auth_store,
        &body.label,
        &pubkey,
        &body.client_hint,
    )?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "ssh.key.add",
        &format!("label={}", body.label),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/ssh/keys/generate` — generates a new Ed25519 keypair and registers the public key.
pub async fn post_ssh_key_generate(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<GenerateSshKeyPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let (generated, ssh_data) = ext::generate_ssh_keypair(
        &state.modules,
        &state.config,
        &state.auth_store,
        &body.label,
        &body.client_hint,
    )?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "ssh.key.generate",
        &format!("label={} fingerprint={}", generated.label, generated.fingerprint),
    )?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "generated": generated,
        "sshData": ssh_data,
    })))
}

/// `DELETE /api/ssh/keys/{id}` — removes an authorized SSH key.
pub async fn delete_ssh_key(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let data = ext::delete_ssh_key(&state.modules, &state.config, &state.auth_store, &id)?;

    let (actor, ip) = actor_and_ip(&req);
    state
        .auth_store
        .append_audit_log(Some(&actor), &ip, "ssh.key.delete", &format!("id={id}"))?;
    Ok(HttpResponse::Ok().json(data))
}

/// `DELETE /api/ssh/sessions/{id}` — terminates an active SSH PTY session.
pub async fn delete_ssh_session(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let data = ext::terminate_ssh_session(&state.modules, &state.config, &state.auth_store, &id)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "ssh.session.terminate",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/ssh/exec` — executes a safe allowlisted Web SSH command (no shell evaluation).
pub async fn post_ssh_exec(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<ExecSshPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let out = ext::exec_web_ssh_command(&state.config, &body.command, &body.cwd);

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "ssh.exec",
        &format!("cmd={}", body.command),
    )?;
    Ok(HttpResponse::Ok().json(out))
}

/// `GET /api/panel-admin` — returns Panel Admin account and security entrance configuration.
pub async fn get_panel_admin(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let admin = ext::load_panel_admin(&state.modules, &state.config, &state.auth_store)?;
    Ok(HttpResponse::Ok().json(admin))
}

/// `PUT /api/panel-admin` — updates Panel Admin username, password, port, or security entrance.
pub async fn put_panel_admin(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<PanelAdminPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let admin = ext::update_panel_admin(
        &state.modules,
        &state.config,
        &state.auth_store,
        ext::PanelAdminPatch {
            admin_username: body.admin_username,
            admin_password: body.admin_password,
            panel_port: body.panel_port,
            security_entrance: body.security_entrance,
            custom_domain: body.custom_domain,
            ssl_enabled: body.ssl_enabled,
            two_factor_enabled: body.two_factor_enabled,
            allowed_admin_ips: body.allowed_admin_ips,
        },
    )?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "panel_admin.update",
        &format!("username={} port={}", admin.admin_username, admin.panel_port),
    )?;
    Ok(HttpResponse::Ok().json(admin))
}

/// `POST /api/panel-admin/cli-action` — runs an `fp` CLI menu option (1..20) from the web UI.
pub async fn post_panel_cli_action(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<PanelCliActionPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let option = if body.option == 0 { 14 } else { body.option };
    let (output, panel_admin) = ext::run_panel_cli_action(
        &state.modules,
        &state.config,
        &state.auth_store,
        option,
        body.value.as_deref(),
    )?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "panel_admin.cli_action",
        &format!("option={option}"),
    )?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "output": output,
        "panelAdmin": panel_admin,
    })))
}

// ============================================================================
// 8. Rust Source Inspector & Polyglot FFI/IPC Bridge Compiler
// ============================================================================

/// `GET /api/rust-source` — returns embedded Rust source files for the inspector tab.
pub async fn get_rust_source(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    Ok(HttpResponse::Ok().json(ext::list_rust_source_files()))
}

/// `GET /api/polyglot/toolchains` — lists supported FFI/IPC Polyglot bridge toolchains.
pub async fn get_polyglot_toolchains(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    Ok(HttpResponse::Ok().json(ext::list_polyglot_toolchains()))
}

/// `POST /api/polyglot/compile` — generates and compiles a Rust FFI/IPC bridge artifact.
pub async fn post_polyglot_compile(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<PolyglotCompileRequest>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload
        .map(web::Json::into_inner)
        .unwrap_or(PolyglotCompileRequest {
            language: "Python (PyO3 / FastAPI)".to_string(),
            module_name: "ferrous_bridge".to_string(),
            optimization_level: "release-lto".to_string(),
        });
    let result = ext::compile_polyglot_bridge(&state.config, &body)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "polyglot.compile",
        &format!("lang={} artifact={}", result.language, result.artifact_path),
    )?;
    Ok(HttpResponse::Ok().json(result))
}

// ============================================================================
// 9. Rust Edge Cloud (Wasmtime WASI, Youki OCI, WireGuard Mesh, BLAKE3 Vaults)
// ============================================================================

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeployWasiWorkerPayload {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub route_pattern: String,
    #[serde(default)]
    pub target_domain: String,
    #[serde(default)]
    pub wasi_target: String,
    #[serde(default)]
    pub memory_limit_kb: u64,
    #[serde(default)]
    pub fuel_limit: u64,
    #[serde(default)]
    pub kv_namespace: String,
    #[serde(default)]
    pub rust_source_snippet: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeployOciContainerPayload {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub runtime_engine: String,
    #[serde(default)]
    pub host_port: u16,
    #[serde(default)]
    pub container_port: u16,
    #[serde(default)]
    pub memory_max_mb: u64,
    #[serde(default)]
    pub cpu_quota_percent: f64,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OciContainerActionPayload {
    #[serde(default)]
    pub action: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AddWireGuardPeerPayload {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub allowed_ips: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateBlake3SnapshotPayload {
    #[serde(default)]
    pub snapshot_name: String,
    #[serde(default)]
    pub target_resource: String,
    #[serde(default)]
    pub s3_endpoint: String,
    #[serde(default)]
    pub bucket: String,
    #[serde(default)]
    pub immutable_lock: bool,
}

/// `GET /api/edge-cloud` — loads WASI workers, Youki/Firecracker OCI sandboxes, WireGuard peers, and BLAKE3 snapshots.
pub async fn get_edge_cloud(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let data = ext::load_edge_cloud(&state.modules)?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/edge-cloud/workers` — deploys a new Wasmtime WASI Preview 2 edge worker.
pub async fn post_edge_worker(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<DeployWasiWorkerPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let name = if body.name.trim().is_empty() {
        "wasi-edge-router".to_string()
    } else {
        body.name
    };
    let data = ext::deploy_wasi_worker(
        &state.modules,
        &name,
        &body.route_pattern,
        &body.target_domain,
        &body.wasi_target,
        body.memory_limit_kb,
        body.fuel_limit,
        &body.kv_namespace,
        &body.rust_source_snippet,
    )?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "edge.worker.deploy",
        &format!("name={name}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/edge-cloud/workers/{id}/invoke` — invokes a WASI edge worker and returns sub-ms execution telemetry.
pub async fn post_edge_worker_invoke(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let (result, edge_cloud) = ext::invoke_wasi_worker(&state.modules, &id)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "edge.worker.invoke",
        &format!("id={id} fuel={}", result.fuel_consumed),
    )?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "result": result,
        "edgeCloud": edge_cloud,
    })))
}

/// `POST /api/edge-cloud/workers/{id}/toggle` — toggles a WASI worker between `deployed` and `paused`.
pub async fn post_edge_worker_toggle(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let data = ext::toggle_wasi_worker(&state.modules, &id)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "edge.worker.toggle",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `DELETE /api/edge-cloud/workers/{id}` — removes a WASI worker.
pub async fn delete_edge_worker(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let data = ext::delete_wasi_worker(&state.modules, &id)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "edge.worker.delete",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/edge-cloud/containers` — deploys a Youki OCI or Firecracker MicroVM sandbox.
pub async fn post_edge_container(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<DeployOciContainerPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let name = if body.name.trim().is_empty() {
        "youki-sandbox".to_string()
    } else {
        body.name
    };
    let data = ext::deploy_oci_container(
        &state.modules,
        &name,
        &body.image,
        &body.runtime_engine,
        body.host_port,
        body.container_port,
        body.memory_max_mb,
        body.cpu_quota_percent,
    )?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "edge.container.deploy",
        &format!("name={name}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/edge-cloud/containers/{id}/action` — starts/stops/restarts/deletes an OCI container sandbox.
pub async fn post_edge_container_action(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
    payload: Option<web::Json<OciContainerActionPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let action = payload
        .map(|p| p.into_inner().action)
        .filter(|a| !a.trim().is_empty())
        .unwrap_or_else(|| "restart".to_string());
    let data = ext::oci_container_action(&state.modules, &id, &action)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "edge.container.action",
        &format!("id={id} action={action}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/edge-cloud/wireguard` — adds a BoringTun WireGuard mesh peer.
pub async fn post_edge_wireguard(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<AddWireGuardPeerPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let name = if body.name.trim().is_empty() {
        "wg-edge-peer".to_string()
    } else {
        body.name
    };
    let data = ext::add_wireguard_peer(
        &state.modules,
        &name,
        &body.region,
        &body.endpoint,
        &body.allowed_ips,
    )?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "edge.wireguard.add",
        &format!("name={name}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/edge-cloud/wireguard/{id}/toggle` — toggles a WireGuard peer connection state.
pub async fn post_edge_wireguard_toggle(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let data = ext::toggle_wireguard_peer(&state.modules, &id)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "edge.wireguard.toggle",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `DELETE /api/edge-cloud/wireguard/{id}` — removes a WireGuard mesh peer.
pub async fn delete_edge_wireguard(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let data = ext::delete_wireguard_peer(&state.modules, &id)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "edge.wireguard.delete",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/edge-cloud/snapshots` — creates a BLAKE3 Merkle-tree verified Zstd snapshot vault.
pub async fn post_edge_snapshot(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<CreateBlake3SnapshotPayload>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.map(web::Json::into_inner).unwrap_or_default();
    let name = if body.snapshot_name.trim().is_empty() {
        "blake3-vault-snap".to_string()
    } else {
        body.snapshot_name
    };
    let data = ext::create_blake3_snapshot(
        &state.modules,
        &name,
        &body.target_resource,
        &body.s3_endpoint,
        &body.bucket,
        body.immutable_lock,
    )?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "edge.snapshot.create",
        &format!("name={name}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `DELETE /api/edge-cloud/snapshots/{id}` — deletes a BLAKE3 snapshot vault.
pub async fn delete_edge_snapshot(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let data = ext::delete_blake3_snapshot(&state.modules, &id)?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "edge.snapshot.delete",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}
