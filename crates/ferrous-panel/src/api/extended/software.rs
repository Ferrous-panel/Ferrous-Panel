use super::common::*;

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

    let list = ext::list_software_packages(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
    )
    .await?;
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
        &state.config,
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
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "software.config",
        &format!("id={id} bytes={}", content.len()),
    )?;
    Ok(HttpResponse::Ok().json(list))
}

