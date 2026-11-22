use crate::state::AppState;
use actix_web::body::{EitherBody, MessageBody};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::header::{HeaderMap, HeaderName, HeaderValue};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenSource {
    Header,
    Cookie,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ForbiddenBody<'a> {
    success: bool,
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

fn is_state_changing_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

/// Inserts baseline HTTP security response headers if not already present.
pub fn attach_security_headers(headers: &mut HeaderMap) {
    let pairs: [(&str, &str); 4] = [
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "SAMEORIGIN"),
        ("referrer-policy", "strict-origin-when-cross-origin"),
        ("permissions-policy", "camera=(), microphone=(), geolocation=()"),
    ];
    for (name, val) in pairs {
        if let (Ok(hdr_name), Ok(hdr_val)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(val),
        ) {
            if !headers.contains_key(&hdr_name) {
                headers.insert(hdr_name, hdr_val);
            }
        }
    }
}

/// Global middleware applying HTTP security headers across panel UI and API responses.
pub async fn security_headers_middleware<B>(
    req: ServiceRequest,
    next: Next<B>,
) -> Result<ServiceResponse<B>, actix_web::Error>
where
    B: MessageBody,
{
    let mut res = next.call(req).await?;
    attach_security_headers(res.headers_mut());
    Ok(res)
}

fn extract_authority_from_url(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    let rest = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))?;
    let authority = rest.split('/').next().unwrap_or("").trim();
    if authority.is_empty() {
        None
    } else {
        Some(authority)
    }
}

fn strip_default_port(authority: &str) -> &str {
    authority
        .strip_suffix(":443")
        .or_else(|| authority.strip_suffix(":80"))
        .unwrap_or(authority)
}

fn origin_matches_host(req: &ServiceRequest) -> bool {
    let Some(host_raw) = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|h| !h.is_empty())
    else {
        return true;
    };

    let candidate_url = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .or_else(|| {
            req.headers()
                .get(header::REFERER)
                .and_then(|v| v.to_str().ok())
                .map(str::trim)
                .filter(|v| !v.is_empty())
        });

    let Some(origin_or_ref) = candidate_url else {
        // Non-browser clients (CLI, curl, integration tests) omit Origin/Referer;
        // browsers always send Origin on cross-site state-changing requests.
        return true;
    };

    if origin_or_ref.eq_ignore_ascii_case("null") {
        return false;
    }

    let Some(origin_authority) = extract_authority_from_url(origin_or_ref) else {
        return false;
    };

    origin_authority.eq_ignore_ascii_case(host_raw)
        || strip_default_port(origin_authority).eq_ignore_ascii_case(strip_default_port(host_raw))
}

fn extract_token_with_source(req: &ServiceRequest) -> Option<(String, TokenSource)> {
    if let Some(auth_hdr) = req.headers().get(header::AUTHORIZATION) {
        if let Ok(raw) = auth_hdr.to_str() {
            let trimmed = raw.trim();
            if let Some(prefix) = trimmed.get(..7) {
                if prefix.eq_ignore_ascii_case("bearer ") {
                    if let Some(tok) = trimmed.get(7..) {
                        let tok = tok.trim();
                        if !tok.is_empty() {
                            return Some((tok.to_string(), TokenSource::Header));
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
                return Some((tok.to_string(), TokenSource::Header));
            }
        }
    }

    if let Some(cookie) = req.cookie(SESSION_COOKIE_NAME) {
        let tok = cookie.value().trim();
        if !tok.is_empty() {
            return Some((tok.to_string(), TokenSource::Cookie));
        }
    }

    None
}

/// Extracts bearer / header / cookie token from an incoming HTTP request.
/// Query-string tokens are intentionally unsupported to keep secrets out of URL logs.
#[must_use]
pub fn extract_token_from_request(req: &ServiceRequest) -> Option<String> {
    extract_token_with_source(req).map(|(tok, _)| tok)
}

fn unauthorized_response<B>(
    req: ServiceRequest,
    message: &str,
) -> ServiceResponse<EitherBody<B>> {
    let mut resp = HttpResponse::Unauthorized().json(UnauthorizedBody {
        success: false,
        authenticated: false,
        expired: true,
        status: 401,
        error: message,
    });
    attach_security_headers(resp.headers_mut());
    req.into_response(resp).map_into_right_body()
}

fn forbidden_response<B>(
    req: ServiceRequest,
    message: &str,
) -> ServiceResponse<EitherBody<B>> {
    let mut resp = HttpResponse::Forbidden().json(ForbiddenBody {
        success: false,
        status: 403,
        error: message,
    });
    attach_security_headers(resp.headers_mut());
    req.into_response(resp).map_into_right_body()
}

/// Middleware enforcing authentication, cookie-auth CSRF Origin defense, security headers,
/// and non-blocking SQLite session lookup across the entire `/api` scope
/// except `GET /api/health` and `POST /api/auth/login`.
pub async fn require_auth<B>(
    req: ServiceRequest,
    next: Next<B>,
) -> Result<ServiceResponse<EitherBody<B>>, actix_web::Error>
where
    B: MessageBody,
{
    if is_public_api_route(req.method(), req.path()) {
        let mut res = next.call(req).await?;
        attach_security_headers(res.headers_mut());
        return Ok(res.map_into_left_body());
    }

    let Some(state) = req.app_data::<web::Data<AppState>>().cloned() else {
        let mut resp = HttpResponse::InternalServerError().json(UnauthorizedBody {
            success: false,
            authenticated: false,
            expired: false,
            status: 500,
            error: "application state unavailable",
        });
        attach_security_headers(resp.headers_mut());
        return Ok(req.into_response(resp).map_into_right_body());
    };

    let Some((token, source)) = extract_token_with_source(&req) else {
        return Ok(unauthorized_response(req, "Authentication token is required"));
    };

    // Defense-in-depth CSRF guard for cookie-authenticated mutations
    if source == TokenSource::Cookie
        && is_state_changing_method(req.method())
        && !origin_matches_host(&req)
    {
        return Ok(forbidden_response(
            req,
            "Cross-origin state-changing request rejected for cookie session",
        ));
    }

    let verify_result = web::block(move || state.auth_store.verify_session(&token)).await;

    match verify_result {
        Ok(Ok(Some(record))) => {
            req.extensions_mut().insert(AuthenticatedUser {
                username: record.username,
            });
            let mut res = next.call(req).await?;
            attach_security_headers(res.headers_mut());
            Ok(res.map_into_left_body())
        }
        Ok(Ok(None)) => Ok(unauthorized_response(
            req,
            "Session token is invalid or has expired",
        )),
        Ok(Err(_)) | Err(_) => {
            let mut resp = HttpResponse::InternalServerError().json(UnauthorizedBody {
                success: false,
                authenticated: false,
                expired: false,
                status: 500,
                error: "session verification error",
            });
            attach_security_headers(resp.headers_mut());
            Ok(req.into_response(resp).map_into_right_body())
        }
    }
}
