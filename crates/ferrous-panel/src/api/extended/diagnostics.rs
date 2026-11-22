use super::common::*;

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

/// `GET /api/ferrousx/sync-status` — returns FerrousX upstream crate & live UDS IPC sync state.
pub async fn get_ferrousx_sync(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let sync = ext::get_ferrousx_sync_status(&state.config, &state.webserver, false).await?;
    Ok(HttpResponse::Ok().json(sync))
}

/// `POST /api/ferrousx/sync-status` — triggers an authenticated `panel_sync` over `/run/ferrousx/panel-admin.sock`.
pub async fn post_ferrousx_sync(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let sync = ext::get_ferrousx_sync_status(&state.config, &state.webserver, true).await?;
    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "ferrousx.sync",
        &format!(
            "commit={} epoch={} socket={}",
            sync.commit_sha, sync.config_epoch, sync.uds_control_socket
        ),
    )?;
    Ok(HttpResponse::Ok().json(sync))
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FerrousxIpcCommandPayload {
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

/// `GET /api/ferrousx/ipc/status` — queries live FerrousX metrics over `/run/ferrousx/panel-admin.sock`.
pub async fn get_ferrousx_ipc_status(
    state: web::Data<AppState>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let client = state.webserver.ferrousx.ipc_client();
    let socket_path = client.resolved_socket_path().display().to_string();
    let (live, latency_us, message) = client.status().await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "socketPath": socket_path,
        "success": true,
        "message": message,
        "latencyUs": latency_us,
        "status": live,
    })))
}

/// `POST /api/ferrousx/ipc/command` — executes an allowlisted IPC command over `/run/ferrousx/panel-admin.sock`.
pub async fn post_ferrousx_ipc_command(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: web::Json<FerrousxIpcCommandPayload>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let body = payload.into_inner();
    let action = if body.action.trim().is_empty() {
        "status".to_string()
    } else {
        body.action.trim().to_ascii_lowercase()
    };
    let ipc_payload = if body.payload.is_null() {
        serde_json::json!({})
    } else {
        body.payload
    };

    let client = state.webserver.ferrousx.ipc_client();
    let resp = client.send_command(&action, ipc_payload).await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "ferrousx.ipc",
        &format!("action={action} success={} latency_us={}", resp.success, resp.latency_us),
    )?;
    Ok(HttpResponse::Ok().json(resp))
}

