#![forbid(unsafe_code)]

pub mod api;
pub mod auth;
pub mod cli;
pub mod config;
pub mod error;
pub mod modules;
pub mod state;
pub mod system;
pub mod web;
pub mod webserver;

use crate::state::AppState;
use actix_web::web::JsonConfig;

/// Maximum allowed JSON payload size (1 MiB).
pub const MAX_JSON_PAYLOAD_BYTES: usize = 1024 * 1024;

/// Registers shared extractors, `/api` routes (with auth middleware), and embedded SPA routes.
pub fn configure_app(cfg: &mut actix_web::web::ServiceConfig, state: actix_web::web::Data<AppState>) {
    cfg.app_data(state)
        .app_data(JsonConfig::default().limit(MAX_JSON_PAYLOAD_BYTES))
        .configure(api::configure_api)
        .configure(web::configure_web);
}
