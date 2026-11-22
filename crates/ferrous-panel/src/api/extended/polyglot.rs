use super::common::*;

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
            target_language: "Python (PyO3 / FastAPI)".to_string(),
            module_name: "ferrous_bridge".to_string(),
            optimization_level: "release-lto".to_string(),
            enable_zero_copy_ipc: true,
            expose_sys_metrics: true,
            expose_sql_bridge: false,
            custom_functions: Vec::new(),
        });
    let result = ext::compile_polyglot_bridge(
        &state.config,
        state.webserver.executor().as_ref(),
        &body,
    )
    .await?;

    let (actor, ip) = actor_and_ip(&req);
    state.auth_store.append_audit_log(
        Some(&actor),
        &ip,
        "polyglot.compile",
        &format!(
            "lang={} artifact={}",
            result.target_language, result.binary_artifact_path
        ),
    )?;
    Ok(HttpResponse::Ok().json(result))
}

