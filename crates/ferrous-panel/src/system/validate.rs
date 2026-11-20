use crate::error::PanelError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// Validated DNS domain name (ASCII FQDN/host, lowercase, 1..=253 bytes).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Domain(String);

impl Domain {
    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        let candidate = raw.trim().trim_end_matches('.').to_ascii_lowercase();
        if candidate.is_empty() || candidate.len() > 253 {
            return Err(PanelError::Validation(
                "domain length must be between 1 and 253 characters".to_string(),
            ));
        }
        if !candidate.contains('.') {
            return Err(PanelError::Validation(
                "domain must include at least one dot label separator".to_string(),
            ));
        }

        for label in candidate.split('.') {
            if label.is_empty() || label.len() > 63 {
                return Err(PanelError::Validation(
                    "domain label length must be between 1 and 63 characters".to_string(),
                ));
            }
            if label.starts_with('-') || label.ends_with('-') {
                return Err(PanelError::Validation(
                    "domain label cannot start or end with '-'".to_string(),
                ));
            }
            if !label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            {
                return Err(PanelError::Validation(
                    "domain may only contain ASCII letters, digits, '-' and '.'".to_string(),
                ));
            }
        }

        Ok(Self(candidate))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Domain {
    type Err = PanelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Serialize for Domain {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Domain {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Validated systemd service unit name (`[a-z0-9][a-z0-9._-]{0,63}`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServiceName(String);

impl ServiceName {
    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        let trimmed = raw.trim().to_ascii_lowercase();
        let clean = trimmed.strip_suffix(".service").unwrap_or(&trimmed);
        if clean.is_empty() || clean.len() > 64 {
            return Err(PanelError::Validation(
                "service name length must be between 1 and 64 characters".to_string(),
            ));
        }
        let first_ok = clean
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
        if !first_ok {
            return Err(PanelError::Validation(
                "service name must start with an ASCII letter or digit".to_string(),
            ));
        }
        if clean.contains("..") {
            return Err(PanelError::Validation(
                "service name cannot contain '..'".to_string(),
            ));
        }
        if !clean
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_' || b == b'.')
        {
            return Err(PanelError::Validation(
                "service name may only contain [a-z0-9._-]".to_string(),
            ));
        }

        Ok(Self(clean.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn unit_file_name(&self) -> String {
        format!("{}.service", self.0)
    }
}

impl fmt::Display for ServiceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for ServiceName {
    type Err = PanelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Serialize for ServiceName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ServiceName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Validated non-zero TCP/UDP port (`1..=65535`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Port(u16);

impl Port {
    pub fn new(value: u16) -> Result<Self, PanelError> {
        if value == 0 {
            return Err(PanelError::Validation(
                "port 0 is not allowed; must be 1..=65535".to_string(),
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn is_privileged(self) -> bool {
        self.0 < 1024
    }
}

/// Default TCP port for the FerrousPanel web control plane.
pub const DEFAULT_PANEL_PORT: u16 = 8443;

/// Returns `true` if `port` is strictly reserved for websites served by Nginx/FerrousX (`80` or `443`).
#[must_use]
pub const fn is_reserved_website_port(port: u16) -> bool {
    matches!(port, 80 | 443)
}

/// Validates a TCP port dedicated to the FerrousPanel control plane.
/// Ports `80` and `443` are strictly reserved for websites managed by Nginx/FerrousX.
pub fn validate_panel_port(value: u16) -> Result<Port, PanelError> {
    let port = Port::new(value)?;
    if is_reserved_website_port(port.get()) {
        return Err(PanelError::Validation(format!(
            "port {} is reserved exclusively for websites (Nginx/FerrousX); choose a dedicated panel port (default: {DEFAULT_PANEL_PORT})",
            port.get()
        )));
    }
    Ok(port)
}

impl fmt::Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<u16> for Port {
    type Error = PanelError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl FromStr for Port {
    type Err = PanelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parsed = s
            .trim()
            .parse::<u16>()
            .map_err(|_| PanelError::Validation("invalid TCP/UDP port number".to_string()))?;
        Self::new(parsed)
    }
}

impl Serialize for Port {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u16(self.0)
    }
}

impl<'de> Deserialize<'de> for Port {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = u16::deserialize(deserializer)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

/// Selectable web server engine matching UI JSON labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WebServerEngine {
    #[default]
    #[serde(rename = "FerrousX (Rust Web Server)")]
    FerrousX,
    #[serde(rename = "Nginx 1.26")]
    Nginx,
}

/// Validated SQL identifier (`[A-Za-z_][A-Za-z0-9_]{0,63}`) safe for backtick/double-quote escaping.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SqlIdentifier(String);

impl SqlIdentifier {
    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.len() > 63 {
            return Err(PanelError::Validation(
                "SQL identifier length must be between 1 and 63 characters".to_string(),
            ));
        }
        let mut bytes = trimmed.bytes();
        let Some(first) = bytes.next() else {
            return Err(PanelError::Validation(
                "SQL identifier cannot be empty".to_string(),
            ));
        };
        if !(first.is_ascii_alphabetic() || first == b'_') {
            return Err(PanelError::Validation(
                "SQL identifier must start with an ASCII letter or '_'".to_string(),
            ));
        }
        if !bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err(PanelError::Validation(
                "SQL identifier may only contain [A-Za-z0-9_]".to_string(),
            ));
        }
        let upper = trimmed.to_ascii_uppercase();
        if matches!(
            upper.as_str(),
            "MYSQL" | "INFORMATION_SCHEMA" | "PERFORMANCE_SCHEMA" | "SYS" | "POSTGRES" | "TEMPLATE0" | "TEMPLATE1"
        ) {
            return Err(PanelError::Forbidden(format!(
                "system identifier '{trimmed}' cannot be modified"
            )));
        }
        Ok(Self(trimmed.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn quote_mysql(&self) -> String {
        format!("`{}`", self.0.replace('`', "``"))
    }

    #[must_use]
    pub fn quote_postgres(&self) -> String {
        format!("\"{}\"", self.0.replace('"', "\"\""))
    }
}

impl fmt::Display for SqlIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Validated SQL collation / charset string (`[A-Za-z0-9_-]{1,64}`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CollationName(String);

impl CollationName {
    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.len() > 64 {
            return Err(PanelError::Validation(
                "collation length must be 1..=64".to_string(),
            ));
        }
        if !trimmed
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
        {
            return Err(PanelError::Validation(
                "collation may only contain [A-Za-z0-9_.-]".to_string(),
            ));
        }
        Ok(Self(trimmed.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Escapes an arbitrary literal string for MySQL single-quoted SQL literals.
pub fn escape_mysql_literal(raw: &str) -> Result<String, PanelError> {
    if raw.contains('\0') {
        return Err(PanelError::Forbidden(
            "NUL byte in SQL literal is forbidden".to_string(),
        ));
    }
    let escaped = raw.replace('\\', "\\\\").replace('\'', "''");
    Ok(format!("'{escaped}'"))
}

/// Escapes an arbitrary literal string for PostgreSQL single-quoted literals.
pub fn escape_postgres_literal(raw: &str) -> Result<String, PanelError> {
    if raw.contains('\0') {
        return Err(PanelError::Forbidden(
            "NUL byte in SQL literal is forbidden".to_string(),
        ));
    }
    let escaped = raw.replace('\'', "''");
    Ok(format!("'{escaped}'"))
}

/// Validated port (`80`) or port range (`8000:8010`) for UFW rules.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PortOrRange(String);

impl PortOrRange {
    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        let trimmed = raw.trim();
        if let Some((start_s, end_s)) = trimmed.split_once(':') {
            let start = Port::from_str(start_s)?;
            let end = Port::from_str(end_s)?;
            if start.get() >= end.get() {
                return Err(PanelError::Validation(
                    "port range start must be less than end".to_string(),
                ));
            }
            Ok(Self(format!("{}:{}", start.get(), end.get())))
        } else {
            let port = Port::from_str(trimmed)?;
            Ok(Self(port.get().to_string()))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated 5-field cron schedule (`min hour dom month dow`) with zero newlines or shell metachars.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CronSchedule(String);

impl CronSchedule {
    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        let normalized = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.is_empty() || normalized.len() > 128 {
            return Err(PanelError::Validation(
                "cron schedule length must be 1..=128".to_string(),
            ));
        }
        let fields: Vec<&str> = normalized.split(' ').collect();
        if fields.len() != 5 {
            return Err(PanelError::Validation(
                "cron schedule must have exactly 5 whitespace-separated fields".to_string(),
            ));
        }
        for field in fields {
            if field.is_empty()
                || !field
                    .bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'*' | b'/' | b'-' | b','))
            {
                return Err(PanelError::Validation(
                    "cron schedule field may only contain [0-9*/,-]".to_string(),
                ));
            }
        }
        Ok(Self(normalized))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated POSIX account name (`[a-z_][a-z0-9_-]{0,31}`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SafeLinuxUser(String);

impl SafeLinuxUser {
    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.len() > 32 {
            return Err(PanelError::Validation(
                "linux username length must be 1..=32".to_string(),
            ));
        }
        let mut bytes = trimmed.bytes();
        let Some(first) = bytes.next() else {
            return Err(PanelError::Validation(
                "linux username cannot be empty".to_string(),
            ));
        };
        if !(first.is_ascii_lowercase() || first == b'_') {
            return Err(PanelError::Validation(
                "linux username must start with [a-z_]".to_string(),
            ));
        }
        if !bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-') {
            return Err(PanelError::Validation(
                "linux username may only contain [a-z0-9_-]".to_string(),
            ));
        }
        Ok(Self(trimmed.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated URL-safe resource identifier (`[A-Za-z0-9._-]{1,64}`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SafeId(String);

impl SafeId {
    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.len() > 64 {
            return Err(PanelError::Validation(
                "identifier length must be 1..=64".to_string(),
            ));
        }
        if trimmed.contains("..") {
            return Err(PanelError::Validation(
                "identifier cannot contain '..'".to_string(),
            ));
        }
        if !trimmed
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
        {
            return Err(PanelError::Validation(
                "identifier may only contain [A-Za-z0-9._-]".to_string(),
            ));
        }
        Ok(Self(trimmed.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated POSIX permission mode (`0600`..=`0775`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OctalMode(u32);

impl OctalMode {
    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        let trimmed = raw.trim().trim_start_matches("0o");
        if trimmed.len() < 3 || trimmed.len() > 4 {
            return Err(PanelError::Validation(
                "file permissions must be 3 or 4 octal digits (e.g. 0644 or 0755)".to_string(),
            ));
        }
        let parsed = u32::from_str_radix(trimmed, 8).map_err(|_| {
            PanelError::Validation("invalid octal file permissions".to_string())
        })?;
        if (parsed & !0o777) != 0 {
            return Err(PanelError::Forbidden(
                "setuid/setgid/sticky permission bits are forbidden".to_string(),
            ));
        }
        Ok(Self(parsed))
    }

    #[must_use]
    pub const fn mode(self) -> u32 {
        self.0
    }
}

