use super::common::*;

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
    payload: web::Json<OneClickDeployRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.into_inner();
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

