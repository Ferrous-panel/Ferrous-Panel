use crate::auth::middleware::{AuthenticatedUser, SESSION_COOKIE_NAME};
use crate::auth::password::{constant_time_eq, verify_password};
use crate::error::PanelError;
use crate::state::AppState;
use actix_web::cookie::time::Duration as CookieDuration;
use actix_web::cookie::{Cookie, SameSite};
use actix_web::http::header;
use actix_web::{web, HttpMessage, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub remember_me: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogoutRequest {
    pub token: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UserProfileDto {
    username: String,
    panel_port: u16,
    security_entrance: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    custom_domain: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginResponse {
    success: bool,
    token: String,
    user: UserProfileDto,
    message: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionResponse {
    authenticated: bool,
    user: UserProfileDto,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LogoutResponse {
    success: bool,
    message: &'static str,
}

fn client_ip(req: &HttpRequest) -> String {
    req.peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string())
}

fn user_agent(req: &HttpRequest) -> String {
    req.headers()
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(200).collect())
        .unwrap_or_else(|| "unknown".to_string())
}

/// `POST /api/auth/login`
pub async fn login(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: web::Json<LoginRequest>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let ip = client_ip(&req);
    let ua = user_agent(&req);
    state.rate_limiter.check(&ip)?;

    let clean_user = payload.username.trim();
    let clean_pass = payload.password.trim();
    if clean_user.is_empty() || clean_pass.is_empty() {
        return Err(PanelError::Validation(
            "username and password are required".to_string(),
        ));
    }

    let (stored_user, stored_hash) = {
        let guard = state
            .credentials
            .read()
            .map_err(|_| PanelError::Internal("credentials lock unavailable".to_string()))?;
        (guard.username.clone(), guard.password_hash.clone())
    };

    let user_matches = constant_time_eq(clean_user.as_bytes(), stored_user.as_bytes());
    let pass_matches = verify_password(clean_pass, &stored_hash);

    if !user_matches || !pass_matches {
        state.rate_limiter.record_failure(&ip)?;
        let _ = state.auth_store.append_audit_log(
            Some(clean_user),
            &ip,
            "AUTH_LOGIN_FAILED",
            "invalid credentials",
        );
        return Err(PanelError::Unauthorized(
            "invalid username or password".to_string(),
        ));
    }

    state.rate_limiter.clear(&ip)?;
    let (token, ttl_secs) =
        state
            .auth_store
            .create_session(&stored_user, payload.remember_me, &ip, &ua)?;
    state.auth_store.append_audit_log(
        Some(&stored_user),
        &ip,
        "AUTH_LOGIN_SUCCESS",
        if payload.remember_me {
            "session issued (30d rememberMe)"
        } else {
            "session issued (24h)"
        },
    )?;

    let cookie = Cookie::build(SESSION_COOKIE_NAME, token.expose().to_string())
        .path("/")
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(state.config.secure_cookie)
        .max_age(CookieDuration::seconds(ttl_secs))
        .finish();

    Ok(HttpResponse::Ok().cookie(cookie).json(LoginResponse {
        success: true,
        token: token.expose().to_string(),
        user: UserProfileDto {
            username: stored_user,
            panel_port: state.config.port,
            security_entrance: state.config.security_entrance.clone(),
            custom_domain: state.config.custom_domain.clone(),
        },
        message: "Signed in to FerrousPanel.",
    }))
}

/// `GET /api/auth/session` — requires valid session via auth middleware.
pub async fn session(req: HttpRequest, state: web::Data<AppState>) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let username = req
        .extensions()
        .get::<AuthenticatedUser>()
        .map(|u| u.username.clone())
        .ok_or_else(|| PanelError::Unauthorized("session missing".to_string()))?;

    Ok(HttpResponse::Ok().json(SessionResponse {
        authenticated: true,
        user: UserProfileDto {
            username,
            panel_port: state.config.port,
            security_entrance: state.config.security_entrance.clone(),
            custom_domain: state.config.custom_domain.clone(),
        },
    }))
}

/// `POST /api/auth/logout` — requires valid session via auth middleware; revokes token in SQLite.
pub async fn logout(
    req: HttpRequest,
    state: web::Data<AppState>,
    payload: Option<web::Json<LogoutRequest>>,
) -> Result<HttpResponse, PanelError> {
    state.telemetry.record_request();
    let _conn_guard = state.telemetry.connection_guard();

    let ip = client_ip(&req);
    let username = req
        .extensions()
        .get::<AuthenticatedUser>()
        .map(|u| u.username.clone());

    let mut token = extract_token_from_http(&req);
    if token.is_none() {
        if let Some(body) = payload {
            token = body
                .into_inner()
                .token
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty());
        }
    }

    if let Some(ref tok) = token {
        state.auth_store.revoke_session(tok)?;
    }

    state.auth_store.append_audit_log(
        username.as_deref(),
        &ip,
        "AUTH_LOGOUT",
        "session revoked",
    )?;

    let cleared = Cookie::build(SESSION_COOKIE_NAME, "")
        .path("/")
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(state.config.secure_cookie)
        .max_age(CookieDuration::ZERO)
        .finish();

    Ok(HttpResponse::Ok().cookie(cleared).json(LogoutResponse {
        success: true,
        message: "Signed out safely.",
    }))
}

fn extract_token_from_http(req: &HttpRequest) -> Option<String> {
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
    if let Some(hdr) = req.headers().get("X-FP-Session") {
        if let Ok(raw) = hdr.to_str() {
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
