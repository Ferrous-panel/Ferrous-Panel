use crate::api::actor_and_ip;
use crate::error::PanelError;
use crate::state::AppState;
use crate::webserver::{EngineKind, SiteSpec};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWebServerRequest {
    #[serde(default, alias = "frontend_engine")]
    pub frontend_engine: Option<EngineKind>,
    #[serde(default, alias = "worker_threads")]
    pub worker_threads: Option<u32>,
    #[serde(default)]
    pub sites: Option<Vec<SiteSpec>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchWebServerRequest {
    #[serde(
        alias = "target_engine",
        alias = "frontendEngine",
        alias = "frontend_engine",
        alias = "engine"
    )]
    pub target_engine: EngineKind,
}

/// `GET /api/webserver` — returns `frontend_engine`, live Nginx/FerrousX status, site specs, and real SSL cert metadata.
pub async fn get_webserver(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let overview = state.webserver.overview().await?;
    Ok(HttpResponse::Ok().json(overview))
}

/// `PUT /api/webserver` — updates settings and site specifications for the configured `frontend_engine`.
pub async fn put_webserver(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: web::Json<UpdateWebServerRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let (actor, ip) = actor_and_ip(&req);
    let body = payload.into_inner();
    let updated = state
        .webserver
        .update_settings(
            body.frontend_engine,
            body.worker_threads,
            body.sites,
            &state.auth_store,
            &actor,
            &ip,
        )
        .await?;

    Ok(HttpResponse::Ok().json(updated))
}

/// `POST /api/webserver/switch` — atomically validates all sites, stops old engine, starts new engine on 80/443,
/// runs HTTP health checks per site, and rolls back on any failure.
pub async fn switch_webserver(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: web::Json<SwitchWebServerRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let (actor, ip) = actor_and_ip(&req);
    let outcome = state
        .webserver
        .switch_engine(payload.target_engine, &state.auth_store, &actor, &ip)
        .await?;

    Ok(HttpResponse::Ok().json(outcome))
}
