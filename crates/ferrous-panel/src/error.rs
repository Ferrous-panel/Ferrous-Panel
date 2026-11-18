use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError};
use serde::Serialize;
use thiserror::Error;

/// Unified application error type for `ferrous-panel`.
#[derive(Debug, Error)]
pub enum PanelError {
    #[error("invalid input: {0}")]
    Validation(String),

    #[error("authentication failed: {0}")]
    Unauthorized(String),

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("too many failed login attempts; retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: i64 },

    #[error("not implemented: {0}")]
    NotImplemented(&'static str),

    #[error("upstream or engine check failed: {0}")]
    BadGateway(String),

    #[error("command timed out after {timeout_ms}ms: {binary}")]
    CommandTimeout { binary: &'static str, timeout_ms: u64 },

    #[error("command failed ({binary}, exit={exit_code:?}): {stderr}")]
    CommandFailed {
        binary: &'static str,
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("system binary unavailable: {0}")]
    BinaryUnavailable(&'static str),

    #[error("system I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("database error")]
    Sqlite(#[from] rusqlite::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("internal server error: {0}")]
    Internal(String),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorEnvelope {
    success: bool,
    status: u16,
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    authenticated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expired: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_after_secs: Option<i64>,
}

impl ResponseError for PanelError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Validation(_) | Self::Json(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::NotImplemented(_) => StatusCode::NOT_IMPLEMENTED,
            Self::BadGateway(_) => StatusCode::BAD_GATEWAY,
            Self::CommandTimeout { .. } => StatusCode::GATEWAY_TIMEOUT,
            Self::CommandFailed { .. }
            | Self::BinaryUnavailable(_)
            | Self::Io(_)
            | Self::Sqlite(_)
            | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let code = self.status_code();
        let is_unauth = matches!(self, Self::Unauthorized(_));
        let retry_after_secs = match self {
            Self::RateLimited { retry_after_secs } => Some(*retry_after_secs),
            _ => None,
        };

        let safe_message = match self {
            Self::Sqlite(_) => "internal storage error".to_string(),
            Self::Internal(_) => "internal server error".to_string(),
            other => other.to_string(),
        };

        let mut builder = HttpResponse::build(code);
        if let Some(retry) = retry_after_secs {
            builder.insert_header(("Retry-After", retry.to_string()));
        }

        builder.json(ErrorEnvelope {
            success: false,
            status: code.as_u16(),
            error: safe_message,
            authenticated: if is_unauth { Some(false) } else { None },
            expired: if is_unauth { Some(true) } else { None },
            retry_after_secs,
        })
    }
}
