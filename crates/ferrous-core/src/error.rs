use thiserror::Error;

/// Unified error type for `ferrous-core`.
///
/// Maps cleanly to HTTP status codes without leaking internal secrets or paths.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid input: {0}")]
    Validation(String),

    #[error("authentication error: {0}")]
    Auth(&'static str),

    #[error("access forbidden: {0}")]
    Forbidden(&'static str),

    #[error("resource conflict: {0}")]
    Conflict(String),

    #[error("resource not found: {0}")]
    NotFound(String),

    #[error("not implemented: {0}")]
    NotImplemented(&'static str),

    #[error("command timed out after {timeout_ms}ms: {binary}")]
    CommandTimeout { binary: &'static str, timeout_ms: u64 },

    #[error("command failed ({binary}, exit={exit_code:?}): {stderr}")]
    CommandFailed {
        binary: &'static str,
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("binary not found on host: {0}")]
    BinaryUnavailable(&'static str),

    #[error("system I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

impl CoreError {
    /// Returns the HTTP status code corresponding to this error variant.
    #[must_use]
    pub const fn http_status(&self) -> u16 {
        match self {
            Self::Validation(_) => 400,
            Self::Auth(_) => 401,
            Self::Forbidden(_) => 403,
            Self::NotFound(_) => 404,
            Self::Conflict(_) => 409,
            Self::NotImplemented(_) => 501,
            Self::CommandTimeout { .. } => 504,
            Self::CommandFailed { .. }
            | Self::BinaryUnavailable(_)
            | Self::Io(_)
            | Self::Json(_) => 500,
        }
    }
}
