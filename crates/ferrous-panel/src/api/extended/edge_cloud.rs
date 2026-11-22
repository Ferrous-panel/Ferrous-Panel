use super::common::*;

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

/// `GET /api/edge-cloud` — loads WASI workers, OCI sandboxes, WireGuard peers, and Zstd/Merkle snapshots.
pub async fn get_edge_cloud(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let data = ext::load_edge_cloud(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
    )
    .await?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/edge-cloud/workers` — compiles and deploys a Wasmtime WASI Preview 2 edge worker.
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
        &state.config,
        state.webserver.executor().as_ref(),
        &name,
        &body.route_pattern,
        &body.target_domain,
        &body.wasi_target,
        body.memory_limit_kb,
        body.fuel_limit,
        &body.kv_namespace,
        &body.rust_source_snippet,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "edge.worker.deploy",
        &format!("name={name}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/edge-cloud/workers/{id}/invoke` — invokes a WASI edge worker and returns real execution telemetry.
pub async fn post_edge_worker_invoke(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let (result, edge_cloud) = ext::invoke_wasi_worker(
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
    let data = ext::toggle_wasi_worker(
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
        "edge.worker.toggle",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `DELETE /api/edge-cloud/workers/{id}` — removes a WASI worker and its build artifacts.
pub async fn delete_edge_worker(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let data = ext::delete_wasi_worker(
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
        "edge.worker.delete",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/edge-cloud/containers` — deploys a Podman/Docker OCI container sandbox.
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
        &state.config,
        state.webserver.executor().as_ref(),
        &name,
        &body.image,
        &body.runtime_engine,
        body.host_port,
        body.container_port,
        body.memory_max_mb,
        body.cpu_quota_percent,
    )
    .await?;

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
    let data = ext::oci_container_action(
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
        "edge.container.action",
        &format!("id={id} action={action}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/edge-cloud/wireguard` — provisions a WireGuard mesh peer with real Curve25519 keys.
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
        &state.config,
        state.webserver.executor().as_ref(),
        &name,
        &body.region,
        &body.endpoint,
        &body.allowed_ips,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "edge.wireguard.add",
        &format!("name={name}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/edge-cloud/wireguard/{id}/toggle` — toggles a WireGuard peer connection state and syncs wg0.conf.
pub async fn post_edge_wireguard_toggle(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let data = ext::toggle_wireguard_peer(
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
        "edge.wireguard.toggle",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `DELETE /api/edge-cloud/wireguard/{id}` — removes a WireGuard mesh peer and syncs wg0.conf.
pub async fn delete_edge_wireguard(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let data = ext::delete_wireguard_peer(
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
        "edge.wireguard.delete",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/edge-cloud/snapshots` — creates a verified Zstd/Gzip snapshot archive of a webroot target.
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
        &state.config,
        state.webserver.executor().as_ref(),
        &name,
        &body.target_resource,
        &body.s3_endpoint,
        &body.bucket,
        body.immutable_lock,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "edge.snapshot.create",
        &format!("name={name}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `DELETE /api/edge-cloud/snapshots/{id}` — deletes a snapshot archive from disk and metadata store.
pub async fn delete_edge_snapshot(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let id = path.into_inner();
    let data = ext::delete_blake3_snapshot(
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
        "edge.snapshot.delete",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}
