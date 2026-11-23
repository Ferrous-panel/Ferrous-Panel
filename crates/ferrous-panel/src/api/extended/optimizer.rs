use super::common::*;

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

    let data = ext::load_optimizer(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
    )
    .await?;
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
        .unwrap_or_else(|| "profile-high-concurrency".to_string());
    let data = ext::apply_optimizer_profile(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
        &profile_id,
    )
    .await?;

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
    let data = ext::toggle_optimizer_sysctl(
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
        "optimizer.sysctl.toggle",
        &format!("id={id}"),
    )?;
    Ok(HttpResponse::Ok().json(data))
}

/// `POST /api/optimizer/reclaim-memory` — flushes Linux kernel page caches and records memory reclamation event.
pub async fn post_optimizer_reclaim_memory(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let (freed_mb, message, optimizer) = ext::reclaim_memory(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
    )
    .await?;

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

    let data = ext::watchdog_heal(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
    )
    .await?;

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
        &state.config,
        state.webserver.executor().as_ref(),
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
    )
    .await?;

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
    let (message, optimizer) = ext::emergency_mitigate(
        &state.modules,
        &state.config,
        state.webserver.executor().as_ref(),
        &action,
    )
    .await?;

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

