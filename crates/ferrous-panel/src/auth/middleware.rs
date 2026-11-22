use crate::state::AppState;
use actix_web::body::{EitherBody, MessageBody};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::{header, Method};
use actix_web::middleware::Next;
use actix_web::{web, HttpMessage, HttpResponse};
use serde::Serialize;

pub const SESSION_COOKIE_NAME: &str = "fp_admin_token";

/// Authenticated administrator identity attached to request extensions after verification.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub username: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UnauthorizedBody<'a> {
    success: bool,
    authenticated: bool,
    expired: bool,
    status: u16,
    error: &'a str,
}

fn is_public_api_route(method: &Method, path: &str) -> bool {
    (method == Method::GET
        && matches!(
            path,
            "/api/health" | "/phpadmin/api/health" | "/phpmyadmin/api/health"
        ))
        || (method == Method::POST
            && matches!(
                path,
                "/api/auth/login" | "/phpadmin/api/auth/login" | "/phpmyadmin/api/auth/login"
            ))
}

/// Extracts bearer / header / cookie token from an incoming HTTP request.
/// Query-string tokens are intentionally unsupported to keep secrets out of URL logs.
#[must_use]
pub fn extract_token_from_request(req: &ServiceRequest) -> Option<String> {
    if let Some(auth_hdr) = req.headers().get(header::AUTHORIZATION) {
        if let Ok(raw) = auth_hdr.to_str() {
            let trimmed = raw.trim();
            if let Some(prefix) = trimmed.get(..7) {
                if prefix.eq_ignore_ascii_case("bearer ") {
                    if let Some(tok) = trimmed.get(7..) {
                        let tok = tok.trim();
                        if !tok.is_empty() {
                            return Some(tok.to_string());
                        }
                    }
                }
            }
        }
    }

    if let Some(session_hdr) = req.headers().get("X-FP-Session") {
        if let Ok(raw) = session_hdr.to_str() {
            let tok = raw.trim();
            if !tok.is_empty() {
                return Some(tok.to_string());
            }
        }
    }

    if let Some(cookie) = req.cookie(SESSION_COOKIE_NAME) {
        let tok = cookie.value().trim();
        if !tok.is_empty() {
            return Some(tok.to_string());
        }
    }

    None
}

fn unauthorized_response<B>(
    req: ServiceRequest,
    message: &str,
) -> ServiceResponse<EitherBody<B>> {
    let resp = HttpResponse::Unauthorized().json(UnauthorizedBody {
        success: false,
        authenticated: false,
        expired: true,
        status: 401,
        error: message,
    });
    req.into_response(resp).map_into_right_body()
}

/// Middleware enforcing authentication across the entire `/api` scope
/// except `GET /api/health` and `POST /api/auth/login`.
pub async fn require_auth<B>(
    req: ServiceRequest,
    next: Next<B>,
) -> Result<ServiceResponse<EitherBody<B>>, actix_web::Error>
where
    B: MessageBody,
{
    if is_public_api_route(req.method(), req.path()) {
        return Ok(next.call(req).await?.map_into_left_body());
    }

    let Some(state) = req.app_data::<web::Data<AppState>>() else {
        let resp = HttpResponse::InternalServerError().json(UnauthorizedBody {
            success: false,
            authenticated: false,
            expired: false,
            status: 500,
            error: "application state unavailable",
        });
        return Ok(req.into_response(resp).map_into_right_body());
    };

    let Some(token) = extract_token_from_request(&req) else {
        return Ok(unauthorized_response(req, "Authentication token is required"));
    };

    match state.auth_store.verify_session(&token) {
        Ok(Some(record)) => {
            req.extensions_mut().insert(AuthenticatedUser {
                username: record.username,
            });
            Ok(next.call(req).await?.map_into_left_body())
        }
        Ok(None) => Ok(unauthorized_response(
            req,
            "Session token is invalid or has expired",
        )),
        Err(_) => {
            let resp = HttpResponse::InternalServerError().json(UnauthorizedBody {
                success: false,
                authenticated: false,
                expired: false,
                status: 500,
                error: "session verification error",
            });
            Ok(req.into_response(resp).map_into_right_body())
        }
    }
}
