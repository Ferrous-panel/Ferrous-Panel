use crate::error::PanelError;
use crate::modules::capabilities;
use crate::state::AppState;
use actix_web::{web, HttpResponse};

/// `GET /api/capabilities` — returns real host feature availability (ready / degraded / unavailable / experimental).
pub async fn get_capabilities(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let report =
        capabilities::probe_capabilities(&state.config, state.webserver.executor().as_ref()).await;
    Ok(HttpResponse::Ok().json(report))
}
