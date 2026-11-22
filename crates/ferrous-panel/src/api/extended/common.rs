pub(super) use crate::api::contract::{OneClickDeployRequest, PolyglotCompileRequest};
pub(super) use crate::error::PanelError;
pub(super) use crate::modules::extended as ext;
pub(super) use crate::state::AppState;
pub(super) use actix_web::{web, HttpRequest, HttpResponse};
pub(super) use serde::Deserialize;

use crate::auth::middleware::AuthenticatedUser;
use crate::auth::rate_limit::extract_client_ip;
use actix_web::HttpMessage;

pub(super) fn actor_and_ip(req: &HttpRequest) -> (String, String) {
    let actor = req
        .extensions()
        .get::<AuthenticatedUser>()
        .map(|u| u.username.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let ip = req
        .app_data::<web::Data<AppState>>()
        .map(|st| extract_client_ip(req, &st.config))
        .unwrap_or_else(|| {
            req.peer_addr()
                .map(|a| a.ip().to_string())
                .unwrap_or_else(|| "127.0.0.1".to_string())
        });
    (actor, ip)
}
