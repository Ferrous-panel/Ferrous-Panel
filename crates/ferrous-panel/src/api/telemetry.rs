use crate::error::PanelError;
use crate::state::AppState;
use actix_web::{web, HttpResponse};

/// `GET /api/telemetry` — authenticated real host metrics from `/proc` + `sysinfo` and persisted SQLite history.
pub async fn get_telemetry(state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let snapshot = state.snapshot_with_real_history()?;
    Ok(HttpResponse::Ok().json(snapshot))
}
