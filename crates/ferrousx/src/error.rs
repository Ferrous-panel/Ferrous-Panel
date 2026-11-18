use std::fmt;

/// Unified error type for `ferrousx` (zero `thiserror`/`anyhow`/`unwrap`/`panic`).
#[derive(Debug)]
pub enum ServerError {
    Config(String),
    Tls(String),
    Io(std::io::Error),
    Toml(toml::de::Error),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(msg) => write!(f, "configuration error: {msg}"),
            Self::Tls(msg) => write!(f, "TLS error: {msg}"),
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Toml(err) => write!(f, "TOML parse error: {err}"),
        }
    }
}

impl std::error::Error for ServerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Toml(err) => Some(err),
            Self::Config(_) | Self::Tls(_) => None,
        }
    }
}

impl From<std::io::Error> for ServerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<toml::de::Error> for ServerError {
    fn from(value: toml::de::Error) -> Self {
        Self::Toml(value)
    }
}
