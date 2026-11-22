use super::common::*;

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
    #[serde(default)]
    pub raw_php_ini: Option<String>,
    #[serde(default)]
    pub raw_fpm_pool_conf: Option<String>,
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

    let list = ext::list_php_versions(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
    )
    .await?;
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
        &state.config,
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
        state.webserver.executor().as_ref(),
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
            raw_php_ini: body.raw_php_ini,
            raw_fpm_pool_conf: body.raw_fpm_pool_conf,
        },
    )
    .await?;

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
    let list = ext::toggle_php_extension(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
        &version,
        &ext_name,
    )
    .await?;

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
        let php_versions = ext::list_php_versions(
            &state.modules,
            &state.config,
            state.webserver.executor().as_ref(),
        )
        .await?;
        return Ok(HttpResponse::Ok().json(serde_json::json!({
            "phpVersions": php_versions,
        })));
    };
    if body.app_id.trim().is_empty() {
        let php_versions = ext::list_php_versions(
            &state.modules,
            &state.config,
            state.webserver.executor().as_ref(),
        )
        .await?;
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

