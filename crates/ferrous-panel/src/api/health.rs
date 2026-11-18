use crate::state::AppState;
use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
    timestamp: String,
}

/// `GET /api/health` — public health check endpoint.
pub async fn health(state: web::Data<AppState>) -> HttpResponse {
    state.telemetry.record_request();
    HttpResponse::Ok().json(HealthResponse {
        status: "ok",
        service: "ferrous-panel",
        version: env!("CARGO_PKG_VERSION"),
        timestamp: Utc::now().to_rfc3339(),
    })
}
