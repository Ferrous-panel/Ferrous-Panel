use crate::state::AppState;
use crate::system::validate::{is_reserved_website_port, DEFAULT_PANEL_PORT};
use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
    panel_port: u16,
    security_entrance: String,
    timestamp: String,
}

/// `GET /api/health` — public health check endpoint.
pub async fn health(state: web::Data<AppState>) -> HttpResponse {
    state.telemetry.record_request();
    let panel_port = if state.config.port == 0 || is_reserved_website_port(state.config.port) {
        DEFAULT_PANEL_PORT
    } else {
        state.config.port
    };
    HttpResponse::Ok().json(HealthResponse {
        status: "ok",
        service: "ferrous-panel",
        version: env!("CARGO_PKG_VERSION"),
        panel_port,
        security_entrance: state.config.security_entrance.clone(),
        timestamp: Utc::now().to_rfc3339(),
    })
}
