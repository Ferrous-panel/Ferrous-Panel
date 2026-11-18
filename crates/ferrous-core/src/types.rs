use crate::error::CoreError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

/// Validated DNS domain name (ASCII FQDN/host, lowercase, 1..=253 bytes).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Domain(String);

impl Domain {
    /// Parses and validates a domain string.
    pub fn parse(raw: &str) -> Result<Self, CoreError> {
        let candidate = raw.trim().trim_end_matches('.').to_ascii_lowercase();
        if candidate.is_empty() || candidate.len() > 253 {
            return Err(CoreError::Validation(
                "domain length must be between 1 and 253 characters".to_string(),
            ));
        }
        if !candidate.contains('.') {
            return Err(CoreError::Validation(
                "domain must include at least one dot label separator".to_string(),
            ));
        }

        for label in candidate.split('.') {
            if label.is_empty() || label.len() > 63 {
                return Err(CoreError::Validation(
                    "domain label length must be between 1 and 63 characters".to_string(),
                ));
            }
            if label.starts_with('-') || label.ends_with('-') {
                return Err(CoreError::Validation(
                    "domain label cannot start or end with '-'".to_string(),
                ));
            }
            if !label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            {
                return Err(CoreError::Validation(
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
    type Err = CoreError;

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

/// Validated systemd / OS service unit name (`[a-z0-9][a-z0-9_.-]{0,63}`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServiceName(String);

impl ServiceName {
    pub fn parse(raw: &str) -> Result<Self, CoreError> {
        let trimmed = raw.trim().to_ascii_lowercase();
        let clean = trimmed.strip_suffix(".service").unwrap_or(&trimmed);
        if clean.is_empty() || clean.len() > 64 {
            return Err(CoreError::Validation(
                "service name length must be between 1 and 64 characters".to_string(),
            ));
        }
        let first_ok = clean
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
        if !first_ok {
            return Err(CoreError::Validation(
                "service name must start with an ASCII letter or digit".to_string(),
            ));
        }
        if clean.contains("..") {
            return Err(CoreError::Validation(
                "service name cannot contain '..'".to_string(),
            ));
        }
        if !clean
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_' || b == b'.')
        {
            return Err(CoreError::Validation(
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
    type Err = CoreError;

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
    pub const fn new(value: u16) -> Result<Self, CoreError> {
        if value == 0 {
            return Err(CoreError::Validation(
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

impl fmt::Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<u16> for Port {
    type Error = CoreError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl FromStr for Port {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parsed = s
            .trim()
            .parse::<u16>()
            .map_err(|_| CoreError::Validation("invalid TCP/UDP port number".to_string()))?;
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

/// Validated filesystem path guaranteed to reject NUL bytes, `..` traversal,
/// and paths escaping a mandatory root prefix.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SafePath(PathBuf);

impl SafePath {
    /// Resolves and validates `candidate` so it stays strictly inside `root`.
    ///
    /// Rejects NUL bytes, any `..` component, and lexical traversal outside `root`.
    /// If both `root` and the target exist on disk, verifies canonical paths to block symlinks
    /// pointing outside `root`.
    pub fn under_root(root: &Path, candidate: &str) -> Result<Self, CoreError> {
        if !root.is_absolute() {
            return Err(CoreError::Validation(
                "safe path root must be absolute".to_string(),
            ));
        }
        if candidate.contains('\0') {
            return Err(CoreError::Forbidden("NUL byte in path is forbidden"));
        }

        let trimmed = candidate.trim();
        let candidate_path = Path::new(trimmed);

        let mut normalized_root = PathBuf::new();
        for comp in root.components() {
            match comp {
                Component::RootDir => normalized_root.push(comp.as_os_str()),
                Component::Normal(seg) => normalized_root.push(seg),
                Component::CurDir => {}
                Component::ParentDir | Component::Prefix(_) => {
                    return Err(CoreError::Validation("invalid root path component".to_string()));
                }
            }
        }

        let mut resolved = if candidate_path.is_absolute() {
            PathBuf::new()
        } else {
            normalized_root.clone()
        };

        for comp in candidate_path.components() {
            match comp {
                Component::RootDir => {
                    if candidate_path.is_absolute() {
                        resolved.push(comp.as_os_str());
                    }
                }
                Component::CurDir => {}
                Component::Normal(seg) => resolved.push(seg),
                Component::ParentDir => {
                    return Err(CoreError::Forbidden("path traversal ('..') is forbidden"));
                }
                Component::Prefix(_) => {
                    return Err(CoreError::Validation("unsupported path prefix".to_string()));
                }
            }
        }

        if resolved != normalized_root && !resolved.starts_with(&normalized_root) {
            return Err(CoreError::Forbidden("path escapes allowed root directory"));
        }

        // Symlink containment check when target and root exist on disk.
        if let Ok(canon_root) = std::fs::canonicalize(&normalized_root) {
            if let Ok(canon_target) = std::fs::canonicalize(&resolved) {
                if canon_target != canon_root && !canon_target.starts_with(&canon_root) {
                    return Err(CoreError::Forbidden(
                        "symlink resolves outside allowed root directory",
                    ));
                }
                return Ok(Self(canon_target));
            }
        }

        Ok(Self(resolved))
    }

    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    #[must_use]
    pub fn as_str_lossy(&self) -> std::borrow::Cow<'_, str> {
        self.0.to_string_lossy()
    }
}

impl fmt::Display for SafePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl Serialize for SafePath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let s = self
            .0
            .to_str()
            .ok_or_else(|| serde::ser::Error::custom("path contains non-UTF-8 characters"))?;
        serializer.serialize_str(s)
    }
}

/// Web server engine selectable per site or panel-wide, matching React UI strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WebServerEngine {
    #[default]
    #[serde(rename = "FerrousX (Rust Web Server)")]
    FerrousX,
    #[serde(rename = "Nginx 1.26")]
    Nginx,
}
