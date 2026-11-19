pub mod ferrousx;
pub mod ferrousx_ipc;
pub mod nginx;
pub mod ssl;

use crate::auth::store::AuthStore;
use crate::error::PanelError;
use crate::system::cmd::{self, AllowedBinary, CmdOutput};
use crate::system::paths::{SafePath, DEFAULT_WWWROOT};
use crate::system::validate::{Domain, Port};
use chrono::Utc;
use ferrousx::FerrousxEngine;
use nginx::NginxEngine;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ssl::{SiteSslStatus, SslManager, SslRenewalTimerStatus};
use std::fmt;
use std::future::Future;
use std::io::{Read as _, Write as _};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub const DEFAULT_ACME_WEBROOT: &str = "/www/acme";
pub const DEFAULT_LETSENCRYPT_LIVE: &str = "/etc/letsencrypt/live";

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

// ============================================================================
// Validated Newtypes
// ============================================================================

/// Global active frontend web server engine selector (`nginx | ferrousx`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EngineKind {
    #[default]
    Nginx,
    Ferrousx,
}

impl EngineKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nginx => "nginx",
            Self::Ferrousx => "ferrousx",
        }
    }

    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Nginx => Self::Ferrousx,
            Self::Ferrousx => Self::Nginx,
        }
    }

    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        let lower = raw.trim().to_ascii_lowercase();
        match lower.as_str() {
            "nginx" | "nginx 1.26" => Ok(Self::Nginx),
            "ferrousx" | "ferrousx (rust web server)" => Ok(Self::Ferrousx),
            _ => Err(PanelError::Validation(
                "frontend_engine must be 'nginx' or 'ferrousx'".to_string(),
            )),
        }
    }
}

impl fmt::Display for EngineKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for EngineKind {
    type Err = PanelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Serialize for EngineKind {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for EngineKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Website document root guaranteed to resolve inside `/www/wwwroot`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WwwRootPath(SafePath);

impl WwwRootPath {
    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        let safe = SafePath::in_wwwroot(raw)?;
        Ok(Self(safe))
    }

    pub fn under_root(root: &Path, raw: &str) -> Result<Self, PanelError> {
        let safe = SafePath::under_root(root, raw)?;
        Ok(Self(safe))
    }

    #[must_use]
    pub fn as_path(&self) -> &Path {
        self.0.as_path()
    }
}

impl fmt::Display for WwwRootPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for WwwRootPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for WwwRootPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Safe single filename (e.g. `index.html`, `index.php`) preventing path/directive injection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SafeFilename(String);

impl SafeFilename {
    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.len() > 64 {
            return Err(PanelError::Validation(
                "filename length must be between 1 and 64 characters".to_string(),
            ));
        }
        if trimmed.starts_with('.') || trimmed.contains("..") {
            return Err(PanelError::Validation(
                "filename cannot start with '.' or contain '..'".to_string(),
            ));
        }
        if !trimmed
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
        {
            return Err(PanelError::Validation(
                "filename may only contain [A-Za-z0-9._-]".to_string(),
            ));
        }
        Ok(Self(trimmed.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SafeFilename {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for SafeFilename {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SafeFilename {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Validated reverse-proxy upstream host (IP literal, `localhost`, or DNS hostname).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UpstreamHost(String);

impl UpstreamHost {
    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        let trimmed = raw.trim().to_ascii_lowercase();
        if trimmed.is_empty() || trimmed.len() > 253 {
            return Err(PanelError::Validation(
                "upstream host length must be 1..=253".to_string(),
            ));
        }
        if trimmed == "localhost" {
            return Ok(Self(trimmed));
        }
        if let Ok(ip) = trimmed.parse::<IpAddr>() {
            return Ok(Self(match ip {
                IpAddr::V4(v4) => v4.to_string(),
                IpAddr::V6(v6) => format!("[{v6}]"),
            }));
        }
        let domain = Domain::parse(&trimmed)?;
        Ok(Self(domain.as_str().to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UpstreamHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for UpstreamHost {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for UpstreamHost {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Validated reverse-proxy upstream socket endpoint (`host:port`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpstreamTarget {
    pub host: UpstreamHost,
    pub port: Port,
}

impl UpstreamTarget {
    pub fn new(host: UpstreamHost, port: Port) -> Self {
        Self { host, port }
    }

    #[must_use]
    pub fn http_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }

    #[must_use]
    pub fn authority(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

impl fmt::Display for UpstreamTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

/// Supported PHP-FPM major/minor pools on Ubuntu 22.04/24.04.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PhpFpmVersion {
    #[serde(rename = "8.1")]
    V81,
    #[serde(rename = "8.2")]
    V82,
    #[serde(rename = "8.3")]
    V83,
    #[serde(rename = "8.4")]
    V84,
}

impl PhpFpmVersion {
    #[must_use]
    pub const fn version_str(self) -> &'static str {
        match self {
            Self::V81 => "8.1",
            Self::V82 => "8.2",
            Self::V83 => "8.3",
            Self::V84 => "8.4",
        }
    }

    #[must_use]
    pub const fn socket_path(self) -> &'static str {
        match self {
            Self::V81 => "/run/php/php8.1-fpm.sock",
            Self::V82 => "/run/php/php8.2-fpm.sock",
            Self::V83 => "/run/php/php8.3-fpm.sock",
            Self::V84 => "/run/php/php8.4-fpm.sock",
        }
    }
}

/// Validated ACME notification email address.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AcmeEmail(String);

impl AcmeEmail {
    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.len() > 254 {
            return Err(PanelError::Validation(
                "ACME email length must be 1..=254".to_string(),
            ));
        }
        let Some((local, domain_part)) = trimmed.split_once('@') else {
            return Err(PanelError::Validation(
                "ACME email must contain '@'".to_string(),
            ));
        };
        if local.is_empty()
            || local.len() > 64
            || !local.bytes().all(|b| {
                b.is_ascii_alphanumeric()
                    || matches!(b, b'.' | b'_' | b'%' | b'+' | b'-')
            })
        {
            return Err(PanelError::Validation(
                "ACME email local part contains invalid characters".to_string(),
            ));
        }
        let clean_domain = Domain::parse(domain_part)?;
        Ok(Self(format!("{local}@{clean_domain}")))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AcmeEmail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for AcmeEmail {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for AcmeEmail {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

// ============================================================================
// SiteSpec & Engine Contracts
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SiteMode {
    StaticSpa {
        index_file: SafeFilename,
        #[serde(default)]
        spa_fallback: bool,
    },
    ReverseProxy {
        upstream: UpstreamTarget,
    },
    PhpFpm {
        php_version: PhpFpmVersion,
        index_file: SafeFilename,
    },
    /// Any unrecognized/future site mode triggers HTTP 501 in `render_site`.
    #[serde(other)]
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteSpec {
    pub domain: Domain,
    #[serde(default)]
    pub aliases: Vec<Domain>,
    pub root: WwwRootPath,
    pub mode: SiteMode,
    #[serde(default)]
    pub ssl_enabled: bool,
    #[serde(default)]
    pub redirect_https: bool,
    #[serde(default)]
    pub http3_quic: bool,
    #[serde(default)]
    pub websocket_enabled: bool,
    #[serde(default)]
    pub rate_limit_rps: u32,
}

impl SiteSpec {
    /// Checks site mode and semantic invariants.
    ///
    /// - Unsupported site modes return `501 Not Implemented`.
    /// - Semantically invalid flags (e.g. `redirect_https` without `ssl_enabled`) return `400 Validation`.
    pub fn validate_supported(&self) -> Result<(), PanelError> {
        if matches!(self.mode, SiteMode::Unsupported) {
            return Err(PanelError::NotImplemented(
                "site mode not supported; supported modes: static_spa, reverse_proxy, php_fpm",
            ));
        }
        if self.rate_limit_rps > 1_000_000 {
            return Err(PanelError::Validation(
                "rateLimitRps cannot exceed 1,000,000 requests/sec".to_string(),
            ));
        }
        if self.aliases.len() > 32 {
            return Err(PanelError::Validation(
                "at most 32 domain aliases are allowed per site".to_string(),
            ));
        }
        if self.redirect_https && !self.ssl_enabled {
            return Err(PanelError::Validation(
                "redirectHttps requires sslEnabled = true".to_string(),
            ));
        }
        Ok(())
    }

    /// Deduplicated list of `[domain, aliases...]` with `domain` first.
    #[must_use]
    pub fn all_domains(&self) -> Vec<&Domain> {
        let mut out = vec![&self.domain];
        for alias in &self.aliases {
            if !out.iter().any(|existing| existing.as_str() == alias.as_str()) {
                out.push(alias);
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderedSite {
    pub engine: EngineKind,
    pub domain: Domain,
    pub config_path: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub engine: EngineKind,
    pub valid: bool,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub engine: EngineKind,
    pub installed: bool,
    pub active: bool,
    pub pid: Option<u32>,
    pub sub_state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteHealthResult {
    pub domain: Domain,
    pub status_code: u16,
    pub healthy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchOutcome {
    pub previous_engine: EngineKind,
    pub active_engine: EngineKind,
    pub sites_validated: usize,
    pub health_checks: Vec<SiteHealthResult>,
    pub rolled_back: bool,
    pub message: String,
}

// ============================================================================
// Core Engine Trait
// ============================================================================

/// Unified virtual-host & lifecycle abstraction implemented by `NginxEngine` and `FerrousxEngine`.
pub trait WebServerEngine: fmt::Debug + Send + Sync {
    fn name(&self) -> EngineKind;
    fn is_installed(&self) -> bool;
    fn render_site(&self, spec: &SiteSpec) -> Result<RenderedSite, PanelError>;
    fn write_site<'a>(&'a self, spec: &'a SiteSpec) -> BoxFuture<'a, Result<(), PanelError>>;
    fn remove_site<'a>(&'a self, domain: &'a Domain) -> BoxFuture<'a, Result<(), PanelError>>;
    fn validate(&self) -> BoxFuture<'_, Result<ValidationReport, PanelError>>;
    fn reload(&self) -> BoxFuture<'_, Result<() , PanelError>>;
    fn status(&self) -> BoxFuture<'_, Result<EngineStatus, PanelError>>;
}

// ============================================================================
// System Command & HTTP Probe Abstraction
// ============================================================================

/// Runtime interface for executing allowlisted commands and HTTP health checks.
///
/// Production code always uses `SystemCommandExecutor` (strictly `system::cmd::run`, no shells).
pub trait CommandExecutor: fmt::Debug + Send + Sync {
    fn is_binary_available(&self, bin: AllowedBinary) -> bool;

    fn run<'a>(
        &'a self,
        bin: AllowedBinary,
        args: &'a [&'a str],
        timeout: Duration,
    ) -> BoxFuture<'a, Result<CmdOutput, PanelError>>;

    fn run_with_env<'a>(
        &'a self,
        bin: AllowedBinary,
        args: &'a [&'a str],
        envs: &'a [(&'a str, &'a str)],
        cwd: Option<&'a Path>,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<CmdOutput, PanelError>> {
        let _ = (envs, cwd);
        self.run(bin, args, timeout)
    }

    fn http_probe<'a>(
        &'a self,
        connect_addr: SocketAddr,
        host: &'a Domain,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<u16, PanelError>>;
}

/// Real Ubuntu host executor delegating strictly to `system::cmd::run` and `std::net::TcpStream`.
#[derive(Debug, Default)]
pub struct SystemCommandExecutor;

impl CommandExecutor for SystemCommandExecutor {
    fn is_binary_available(&self, bin: AllowedBinary) -> bool {
        bin.is_available()
    }

    fn run<'a>(
        &'a self,
        bin: AllowedBinary,
        args: &'a [&'a str],
        timeout: Duration,
    ) -> BoxFuture<'a, Result<CmdOutput, PanelError>> {
        Box::pin(cmd::run(bin, args, timeout))
    }

    fn run_with_env<'a>(
        &'a self,
        bin: AllowedBinary,
        args: &'a [&'a str],
        envs: &'a [(&'a str, &'a str)],
        cwd: Option<&'a Path>,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<CmdOutput, PanelError>> {
        Box::pin(cmd::run_with_env(bin, args, envs, cwd, timeout))
    }

    fn http_probe<'a>(
        &'a self,
        connect_addr: SocketAddr,
        host: &'a Domain,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<u16, PanelError>> {
        let host_owned = host.as_str().to_string();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                http_probe_blocking(connect_addr, &host_owned, timeout)
            })
            .await
            .map_err(|_| PanelError::BadGateway("HTTP probe worker cancelled".to_string()))?
        })
    }
}

fn http_probe_blocking(
    connect_addr: SocketAddr,
    host_header: &str,
    timeout: Duration,
) -> Result<u16, PanelError> {
    let mut stream = TcpStream::connect_timeout(&connect_addr, timeout).map_err(|err| {
        PanelError::BadGateway(format!(
            "HTTP health check connect failed for {host_header} on {connect_addr}: {err}"
        ))
    })?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));

    let req = format!(
        "GET / HTTP/1.1\r\nHost: {host_header}\r\nUser-Agent: ferrous-panel-healthcheck/1.0\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).map_err(|err| {
        PanelError::BadGateway(format!(
            "HTTP health check write failed for {host_header}: {err}"
        ))
    })?;

    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).map_err(|err| {
        PanelError::BadGateway(format!(
            "HTTP health check read failed for {host_header}: {err}"
        ))
    })?;
    if n == 0 {
        return Err(PanelError::BadGateway(format!(
            "HTTP health check received empty response for {host_header}"
        )));
    }

    let head = String::from_utf8_lossy(&buf[..n]);
    let status_line = head.lines().next().unwrap_or("");
    let code = status_line
        .strip_prefix("HTTP/")
        .and_then(|rest| rest.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| {
            PanelError::BadGateway(format!(
                "HTTP health check received invalid status line for {host_header}: {status_line}"
            ))
        })?;

    Ok(code)
}

// ============================================================================
// Atomic File & Rollback Helpers
// ============================================================================

/// Writes `content` atomically via same-directory temp file + `sync_all` + `rename`.
pub fn atomic_write_file(target: &Path, content: &[u8], mode: u32) -> Result<(), PanelError> {
    let parent = target.parent().ok_or_else(|| {
        PanelError::Validation(format!("path has no parent directory: {}", target.display()))
    })?;
    std::fs::create_dir_all(parent)?;

    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let file_label = target
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("config");
    let tmp_path = parent.join(format!(".tmp.{file_label}.{}.{seq}", std::process::id()));

    let write_res: Result<(), std::io::Error> = (|| {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&tmp_path)?;
        f.write_all(content)?;
        f.sync_all()?;
        Ok(())
    })();

    if let Err(err) = write_res {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(PanelError::Io(err));
    }

    if let Err(err) = std::fs::rename(&tmp_path, target) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(PanelError::Io(err));
    }
    Ok(())
}

/// Atomically creates or replaces a symlink via temp symlink + `rename`.
pub fn atomic_symlink(target_file: &Path, link_path: &Path) -> Result<(), PanelError> {
    let parent = link_path.parent().ok_or_else(|| {
        PanelError::Validation(format!(
            "symlink path has no parent directory: {}",
            link_path.display()
        ))
    })?;
    std::fs::create_dir_all(parent)?;

    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let file_label = link_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("vhost");
    let tmp_link = parent.join(format!(".tmp.link.{file_label}.{}.{seq}", std::process::id()));

    std::os::unix::fs::symlink(target_file, &tmp_link)?;
    if let Err(err) = std::fs::rename(&tmp_link, link_path) {
        let _ = std::fs::remove_file(&tmp_link);
        return Err(PanelError::Io(err));
    }
    Ok(())
}

/// Snapshot of a regular file + optional symlink so failed validations can restore prior bytes.
#[derive(Debug, Clone)]
pub struct FileSnapshot {
    pub path: PathBuf,
    pub previous_bytes: Option<Vec<u8>>,
}

impl FileSnapshot {
    pub fn capture(path: &Path) -> Result<Self, PanelError> {
        let previous_bytes = match std::fs::symlink_metadata(path) {
            Ok(_) => Some(std::fs::read(path)?),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => return Err(PanelError::Io(err)),
        };
        Ok(Self {
            path: path.to_path_buf(),
            previous_bytes,
        })
    }

    pub fn restore(&self) -> Result<(), PanelError> {
        match &self.previous_bytes {
            Some(bytes) => atomic_write_file(&self.path, bytes, 0o644),
            None => match std::fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(PanelError::Io(err)),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct SymlinkSnapshot {
    pub link_path: PathBuf,
    pub previous_target: Option<PathBuf>,
}

impl SymlinkSnapshot {
    pub fn capture(link_path: &Path) -> Result<Self, PanelError> {
        let previous_target = match std::fs::symlink_metadata(link_path) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    Some(std::fs::read_link(link_path)?)
                } else {
                    None
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => return Err(PanelError::Io(err)),
        };
        Ok(Self {
            link_path: link_path.to_path_buf(),
            previous_target,
        })
    }

    pub fn restore(&self) -> Result<(), PanelError> {
        match &self.previous_target {
            Some(target) => atomic_symlink(target, &self.link_path),
            None => match std::fs::remove_file(&self.link_path) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(PanelError::Io(err)),
            },
        }
    }
}

/// Parses `systemctl show <unit> --property=ActiveState,SubState,MainPID` key=value output.
pub(crate) fn parse_systemctl_status(
    engine: EngineKind,
    installed: bool,
    stdout: &str,
) -> EngineStatus {
    let mut active_state = String::new();
    let mut sub_state = String::new();
    let mut pid: Option<u32> = None;

    for line in stdout.lines() {
        if let Some((k, v)) = line.split_once('=') {
            let val = v.trim();
            match k.trim() {
                "ActiveState" => active_state = val.to_string(),
                "SubState" => sub_state = val.to_string(),
                "MainPID" => {
                    if let Ok(parsed) = val.parse::<u32>() {
                        if parsed > 0 {
                            pid = Some(parsed);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let active = active_state == "active";
    if sub_state.is_empty() {
        sub_state = if installed {
            "unknown".to_string()
        } else {
            "not-installed".to_string()
        };
    }

    EngineStatus {
        engine,
        installed,
        active,
        pid,
        sub_state,
    }
}

// ============================================================================
// SQLite Store (`webserver_settings` & `webserver_sites`)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedWebServerConfig {
    pub frontend_engine: EngineKind,
    pub worker_threads: u32,
}

impl Default for PersistedWebServerConfig {
    fn default() -> Self {
        Self {
            frontend_engine: EngineKind::Nginx,
            worker_threads: 0,
        }
    }
}

#[derive(Debug)]
pub struct WebServerStore {
    conn: Mutex<Connection>,
}

impl WebServerStore {
    pub fn open(db_path: &Path) -> Result<Self, PanelError> {
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS webserver_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS webserver_sites (
                domain TEXT PRIMARY KEY,
                spec_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            ",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, PanelError> {
        self.conn
            .lock()
            .map_err(|_| PanelError::Internal("webserver database lock unavailable".to_string()))
    }

    pub fn load_config(&self) -> Result<PersistedWebServerConfig, PanelError> {
        let conn = self.lock_conn()?;
        let engine_raw: Option<String> = conn
            .query_row(
                "SELECT value FROM webserver_settings WHERE key = 'frontend_engine'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let workers_raw: Option<String> = conn
            .query_row(
                "SELECT value FROM webserver_settings WHERE key = 'worker_threads'",
                [],
                |row| row.get(0),
            )
            .optional()?;

        let frontend_engine = match engine_raw {
            Some(s) => EngineKind::parse(&s)?,
            None => EngineKind::Nginx,
        };
        let worker_threads = workers_raw
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        Ok(PersistedWebServerConfig {
            frontend_engine,
            worker_threads,
        })
    }

    pub fn save_frontend_engine(&self, engine: EngineKind) -> Result<(), PanelError> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO webserver_settings (key, value) VALUES ('frontend_engine', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![engine.as_str()],
        )?;
        Ok(())
    }

    pub fn save_worker_threads(&self, worker_threads: u32) -> Result<(), PanelError> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO webserver_settings (key, value) VALUES ('worker_threads', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![worker_threads.to_string()],
        )?;
        Ok(())
    }

    pub fn list_sites(&self) -> Result<Vec<SiteSpec>, PanelError> {
        let conn = self.lock_conn()?;
        let mut stmt =
            conn.prepare("SELECT spec_json FROM webserver_sites ORDER BY domain ASC")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut sites = Vec::new();
        for row_res in rows {
            let json_str = row_res?;
            let spec: SiteSpec = serde_json::from_str(&json_str)?;
            sites.push(spec);
        }
        Ok(sites)
    }

    pub fn upsert_site(&self, spec: &SiteSpec) -> Result<(), PanelError> {
        let spec_json = serde_json::to_string(spec)?;
        let now = Utc::now().to_rfc3339();
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO webserver_sites (domain, spec_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(domain) DO UPDATE SET spec_json = excluded.spec_json, updated_at = excluded.updated_at",
            params![spec.domain.as_str(), spec_json, now],
        )?;
        Ok(())
    }

    pub fn replace_all_sites(&self, sites: &[SiteSpec]) -> Result<(), PanelError> {
        let now = Utc::now().to_rfc3339();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM webserver_sites", [])?;
        for spec in sites {
            let spec_json = serde_json::to_string(spec)?;
            tx.execute(
                "INSERT INTO webserver_sites (domain, spec_json, updated_at) VALUES (?1, ?2, ?3)",
                params![spec.domain.as_str(), spec_json, now],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn remove_site(&self, domain: &Domain) -> Result<bool, PanelError> {
        let conn = self.lock_conn()?;
        let affected = conn.execute(
            "DELETE FROM webserver_sites WHERE domain = ?1",
            params![domain.as_str()],
        )?;
        Ok(affected > 0)
    }
}

// ============================================================================
// Coordinated WebServerManager (`Nginx` <-> `FerrousX` switch & rollback)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteOverviewDto {
    pub spec: SiteSpec,
    pub ssl: SiteSslStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebServerOverviewDto {
    pub frontend_engine: EngineKind,
    pub worker_threads: u32,
    pub acme_webroot: String,
    pub nginx: EngineStatus,
    pub ferrousx: EngineStatus,
    pub sites_count: usize,
    pub sites: Vec<SiteOverviewDto>,
    pub renewal_timer: SslRenewalTimerStatus,
}

#[derive(Debug)]
enum EngineBackups {
    Nginx(Vec<(FileSnapshot, SymlinkSnapshot)>),
    Ferrousx {
        master: FileSnapshot,
        sites: Vec<FileSnapshot>,
    },
}

impl EngineBackups {
    fn restore_all(&self) -> Result<(), PanelError> {
        match self {
            Self::Nginx(items) => {
                for (conf, link) in items {
                    link.restore()?;
                    conf.restore()?;
                }
            }
            Self::Ferrousx { master, sites } => {
                master.restore()?;
                for site in sites {
                    site.restore()?;
                }
            }
        }
        Ok(())
    }
}

/// Coordinates NginxEngine, FerrousxEngine, SslManager, and zero-overlap engine switches.
#[derive(Debug)]
pub struct WebServerManager {
    pub store: WebServerStore,
    pub nginx: NginxEngine,
    pub ferrousx: FerrousxEngine,
    pub ssl: SslManager,
    executor: Arc<dyn CommandExecutor>,
    probe_addr: SocketAddr,
    switch_lock: tokio::sync::Mutex<()>,
}

impl WebServerManager {
    /// Production constructor bound to real Ubuntu paths (`/etc/nginx`, `/etc/ferrousx`, `/www/acme`, `127.0.0.1:80`).
    pub fn open_system(db_path: &Path, reload_hook_path: PathBuf) -> Result<Self, PanelError> {
        let executor: Arc<dyn CommandExecutor> = Arc::new(SystemCommandExecutor);
        let nginx_paths = nginx::NginxPaths::default_system()?;
        let ferrousx_paths = ferrousx::FerrousxPaths::default_system()?;
        let ssl_paths = ssl::SslPaths::default_system(reload_hook_path)?;
        let store = WebServerStore::open(db_path)?;

        Ok(Self {
            nginx: NginxEngine::new(nginx_paths, Arc::clone(&executor)),
            ferrousx: FerrousxEngine::new(ferrousx_paths, Arc::clone(&executor)),
            ssl: SslManager::new(ssl_paths, Arc::clone(&executor)),
            store,
            executor,
            probe_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80),
            switch_lock: tokio::sync::Mutex::new(()),
        })
    }

    /// Custom constructor for isolated integration tests.
    pub fn with_components(
        store: WebServerStore,
        nginx: NginxEngine,
        ferrousx: FerrousxEngine,
        ssl: SslManager,
        executor: Arc<dyn CommandExecutor>,
        probe_addr: SocketAddr,
    ) -> Self {
        Self {
            store,
            nginx,
            ferrousx,
            ssl,
            executor,
            probe_addr,
            switch_lock: tokio::sync::Mutex::new(()),
        }
    }

    #[must_use]
    pub fn executor(&self) -> &Arc<dyn CommandExecutor> {
        &self.executor
    }

    #[must_use]
    pub const fn probe_addr(&self) -> SocketAddr {
        self.probe_addr
    }

    pub async fn overview(&self) -> Result<WebServerOverviewDto, PanelError> {
        let cfg = self.store.load_config()?;
        let sites_raw = self.store.list_sites()?;
        let nginx_status = self.nginx.status().await?;
        let ferrousx_status = self.ferrousx.status().await?;
        let renewal_timer = self.ssl.renewal_timer_status().await;

        let mut sites = Vec::with_capacity(sites_raw.len());
        for spec in sites_raw {
            let ssl_status = self.ssl.read_cert_status(&spec.domain).await?;
            sites.push(SiteOverviewDto {
                spec,
                ssl: ssl_status,
            });
        }

        Ok(WebServerOverviewDto {
            frontend_engine: cfg.frontend_engine,
            worker_threads: cfg.worker_threads,
            acme_webroot: self.ssl.paths().acme_webroot.as_path().display().to_string(),
            nginx: nginx_status,
            ferrousx: ferrousx_status,
            sites_count: sites.len(),
            sites,
            renewal_timer,
        })
    }

    pub async fn reload_configured_engine(&self) -> Result<(), PanelError> {
        let cfg = self.store.load_config()?;
        match cfg.frontend_engine {
            EngineKind::Nginx => self.nginx.reload().await,
            EngineKind::Ferrousx => self.ferrousx.reload().await,
        }
    }

    pub async fn validate_configured_engine(&self) -> Result<ValidationReport, PanelError> {
        let cfg = self.store.load_config()?;
        self.validate_engine(cfg.frontend_engine).await
    }

    pub async fn upsert_and_apply_site(
        &self,
        spec: &SiteSpec,
        preferred_engine: Option<EngineKind>,
    ) -> Result<RenderedSite, PanelError> {
        spec.validate_supported()?;
        let _guard = self.switch_lock.lock().await;
        let cfg = self.store.load_config()?;
        let engine = preferred_engine.unwrap_or(cfg.frontend_engine);
        let rendered = match engine {
            EngineKind::Nginx => {
                let r = self.nginx.render_site(spec)?;
                self.nginx.write_site(spec).await?;
                if self.nginx.status().await.map(|s| s.active).unwrap_or(false) {
                    self.nginx.reload().await?;
                }
                r
            }
            EngineKind::Ferrousx => {
                let r = self.ferrousx.render_site(spec)?;
                self.ferrousx.write_site(spec).await?;
                if self
                    .ferrousx
                    .status()
                    .await
                    .map(|s| s.active)
                    .unwrap_or(false)
                {
                    self.ferrousx.notify_vhost_add(spec.domain.as_str()).await?;
                }
                r
            }
        };
        self.store.upsert_site(spec)?;
        Ok(rendered)
    }

    pub async fn remove_and_apply_site(&self, domain: &Domain) -> Result<(), PanelError> {
        let _guard = self.switch_lock.lock().await;
        let cfg = self.store.load_config()?;
        if self.nginx.is_installed() {
            let _ = self.nginx.remove_site(domain).await;
            if cfg.frontend_engine == EngineKind::Nginx
                && self.nginx.status().await.map(|s| s.active).unwrap_or(false)
            {
                let _ = self.nginx.reload().await;
            }
        }
        if self.ferrousx.is_installed() {
            let _ = self.ferrousx.remove_site(domain).await;
            if cfg.frontend_engine == EngineKind::Ferrousx
                && self
                    .ferrousx
                    .status()
                    .await
                    .map(|s| s.active)
                    .unwrap_or(false)
            {
                let _ = self.ferrousx.notify_vhost_del(domain.as_str()).await;
            }
        }
        let _ = self.store.remove_site(domain)?;
        Ok(())
    }

    /// Updates global settings and/or site list on the active `frontend_engine` (without switching engines).
    pub async fn update_settings(
        &self,
        desired_engine: Option<EngineKind>,
        worker_threads: Option<u32>,
        sites_update: Option<Vec<SiteSpec>>,
        auth_store: &AuthStore,
        actor: &str,
        ip: &str,
    ) -> Result<WebServerOverviewDto, PanelError> {
        let _guard = self.switch_lock.lock().await;
        let current = self.store.load_config()?;
        let target_engine = desired_engine.unwrap_or(current.frontend_engine);

        if target_engine != current.frontend_engine {
            return Err(PanelError::Conflict(format!(
                "use POST /api/webserver/switch to change active frontend_engine from '{}' to '{}'",
                current.frontend_engine, target_engine
            )));
        }

        if let Some(ref sites) = sites_update {
            for site in sites {
                site.validate_supported()?;
            }
            let engine_installed = match target_engine {
                EngineKind::Nginx => self.nginx.is_installed(),
                EngineKind::Ferrousx => self.ferrousx.is_installed(),
            };
            if !engine_installed {
                return Err(PanelError::Conflict(format!(
                    "{target_engine} is not installed on this server"
                )));
            }

            let threads = worker_threads.unwrap_or(current.worker_threads);
            let backups = self.stage_sites_for(target_engine, sites, threads)?;
            let validation = self.validate_engine(target_engine).await?;
            if !validation.valid {
                let _ = backups.restore_all();
                return Err(PanelError::Validation(format!(
                    "{target_engine} configuration check failed (rolled back): {}",
                    validation.output
                )));
            }
            self.store.replace_all_sites(sites)?;
            if self.engine_is_active(target_engine).await? {
                match target_engine {
                    EngineKind::Nginx => self.nginx.reload().await?,
                    EngineKind::Ferrousx => self.ferrousx.notify_panel_sync().await?,
                }
            }
        }

        if let Some(threads) = worker_threads {
            if target_engine == EngineKind::Ferrousx && self.ferrousx.is_installed() {
                self.ferrousx.ensure_master_config(threads)?;
            }
            self.store.save_worker_threads(threads)?;
        }

        let _ = auth_store.append_audit_log(
            Some(actor),
            ip,
            "WEBSERVER_UPDATE",
            &format!("updated settings for frontend_engine={target_engine}"),
        );

        self.overview().await
    }

    /// Safely switches `frontend_engine` between `nginx` and `ferrousx`:
    ///
    /// 1. Render and validate new configs for ALL sites BEFORE stopping old engine.
    /// 2. Stop old engine and verify it is no longer active (never hold 80/443 simultaneously).
    /// 3. Start new engine on 80/443.
    /// 4. Perform real HTTP health check for EVERY site.
    /// 5. Rollback cleanly on any failure (stop new engine, restore staged files, restart old engine).
    pub async fn switch_engine(
        &self,
        target_engine: EngineKind,
        auth_store: &AuthStore,
        actor: &str,
        ip: &str,
    ) -> Result<SwitchOutcome, PanelError> {
        let _guard = self.switch_lock.lock().await;

        let current_cfg = self.store.load_config()?;
        let previous_engine = current_cfg.frontend_engine;

        if !self.is_engine_installed(target_engine) {
            return Err(PanelError::Conflict(format!(
                "{target_engine} is not installed on this host"
            )));
        }

        let sites = self.store.list_sites()?;
        for site in &sites {
            site.validate_supported()?;
        }

        // Step 1: Stage & validate ALL sites on target engine before touching old engine.
        let staged_backups =
            self.stage_sites_for(target_engine, &sites, current_cfg.worker_threads)?;
        let report = match self.validate_engine(target_engine).await {
            Ok(rep) => rep,
            Err(err) => {
                let _ = staged_backups.restore_all();
                return Err(err);
            }
        };
        if !report.valid {
            let _ = staged_backups.restore_all();
            return Err(PanelError::Validation(format!(
                "{target_engine} validation failed; switch aborted without stopping {previous_engine}: {}",
                report.output
            )));
        }

        let old_engine = target_engine.other();
        let old_was_active = self.engine_is_active(old_engine).await.unwrap_or(false);

        // Step 2: Stop the other engine first so two engines NEVER bind 80/443 at the same time.
        if old_was_active {
            if let Err(err) = self.stop_engine(old_engine).await {
                let _ = staged_backups.restore_all();
                return Err(PanelError::Conflict(format!(
                    "failed to stop {old_engine} before starting {target_engine}: {err}"
                )));
            }
            if self.engine_is_active(old_engine).await.unwrap_or(true) {
                let _ = staged_backups.restore_all();
                return Err(PanelError::Conflict(format!(
                    "{old_engine} remained active after stop; refusing to start {target_engine} on 80/443"
                )));
            }
        }

        // Step 3: Start target engine (or reload if already active in same-engine validation/repair).
        let target_already_active = self.engine_is_active(target_engine).await.unwrap_or(false);
        let start_res = if target_already_active {
            self.reload_engine(target_engine).await
        } else {
            self.start_engine(target_engine).await
        };
        if let Err(start_err) = start_res {
            let _ = self.stop_engine(target_engine).await;
            let _ = staged_backups.restore_all();
            if old_was_active {
                let _ = self.start_engine(old_engine).await;
            }
            return Err(PanelError::BadGateway(format!(
                "failed to start {target_engine} on 80/443 (rolled back to {old_engine}): {start_err}"
            )));
        }

        // Step 4: HTTP health check per site.
        let mut health_checks = Vec::with_capacity(sites.len());
        let mut failed_probe: Option<String> = None;

        for site in &sites {
            match self
                .probe_site_with_retry(&site.domain, Duration::from_secs(2))
                .await
            {
                Ok(code) if (100..500).contains(&code) => {
                    health_checks.push(SiteHealthResult {
                        domain: site.domain.clone(),
                        status_code: code,
                        healthy: true,
                    });
                }
                Ok(code) => {
                    failed_probe = Some(format!(
                        "{} returned unhealthy HTTP status {code}",
                        site.domain
                    ));
                    break;
                }
                Err(err) => {
                    failed_probe = Some(format!("{} probe error: {err}", site.domain));
                    break;
                }
            }
        }

        if let Some(reason) = failed_probe {
            let _ = self.stop_engine(target_engine).await;
            let _ = staged_backups.restore_all();
            if old_was_active {
                let _ = self.start_engine(old_engine).await;
            }
            let _ = auth_store.append_audit_log(
                Some(actor),
                ip,
                "WEBSERVER_SWITCH_ROLLBACK",
                &format!("switch to {target_engine} rolled back: {reason}"),
            );
            return Err(PanelError::BadGateway(format!(
                "HTTP health check failed after starting {target_engine} ({reason}); rolled back cleanly to {old_engine}"
            )));
        }

        // Step 5: Persist active `frontend_engine`.
        self.store.save_frontend_engine(target_engine)?;
        let _ = auth_store.append_audit_log(
            Some(actor),
            ip,
            "WEBSERVER_SWITCH",
            &format!(
                "frontend_engine switched from {previous_engine} to {target_engine} ({} sites verified)",
                sites.len()
            ),
        );

        Ok(SwitchOutcome {
            previous_engine,
            active_engine: target_engine,
            sites_validated: sites.len(),
            health_checks,
            rolled_back: false,
            message: format!(
                "Switched frontend_engine cleanly from {previous_engine} to {target_engine}."
            ),
        })
    }

    async fn probe_site_with_retry(
        &self,
        domain: &Domain,
        per_attempt_timeout: Duration,
    ) -> Result<u16, PanelError> {
        let mut last_err: Option<PanelError> = None;
        for attempt in 0..3 {
            match self
                .executor
                .http_probe(self.probe_addr, domain, per_attempt_timeout)
                .await
            {
                Ok(code) => return Ok(code),
                Err(err) => {
                    last_err = Some(err);
                    if attempt < 2 {
                        tokio::time::sleep(Duration::from_millis(150)).await;
                    }
                }
            }
        }
        Err(last_err.unwrap_or_else(|| {
            PanelError::BadGateway(format!("HTTP probe failed for {domain}"))
        }))
    }

    fn is_engine_installed(&self, engine: EngineKind) -> bool {
        match engine {
            EngineKind::Nginx => self.nginx.is_installed(),
            EngineKind::Ferrousx => self.ferrousx.is_installed(),
        }
    }

    fn stage_sites_for(
        &self,
        engine: EngineKind,
        sites: &[SiteSpec],
        worker_threads: u32,
    ) -> Result<EngineBackups, PanelError> {
        match engine {
            EngineKind::Nginx => {
                let mut backups = Vec::with_capacity(sites.len());
                for site in sites {
                    match self.nginx.stage_site_unvalidated(site) {
                        Ok(pair) => backups.push(pair),
                        Err(err) => {
                            for (conf, link) in backups {
                                let _ = link.restore();
                                let _ = conf.restore();
                            }
                            return Err(err);
                        }
                    }
                }
                Ok(EngineBackups::Nginx(backups))
            }
            EngineKind::Ferrousx => {
                let master_snap = FileSnapshot::capture(&self.ferrousx.paths().master_config)?;
                if let Err(err) = self.ferrousx.ensure_master_config(worker_threads) {
                    let _ = master_snap.restore();
                    return Err(err);
                }
                let mut site_snaps = Vec::with_capacity(sites.len());
                for site in sites {
                    match self.ferrousx.stage_site_unvalidated(site) {
                        Ok(snap) => site_snaps.push(snap),
                        Err(err) => {
                            let _ = master_snap.restore();
                            for s in site_snaps {
                                let _ = s.restore();
                            }
                            return Err(err);
                        }
                    }
                }
                Ok(EngineBackups::Ferrousx {
                    master: master_snap,
                    sites: site_snaps,
                })
            }
        }
    }

    async fn validate_engine(&self, engine: EngineKind) -> Result<ValidationReport, PanelError> {
        match engine {
            EngineKind::Nginx => self.nginx.validate().await,
            EngineKind::Ferrousx => self.ferrousx.validate().await,
        }
    }

    async fn reload_engine(&self, engine: EngineKind) -> Result<(), PanelError> {
        match engine {
            EngineKind::Nginx => self.nginx.reload().await,
            EngineKind::Ferrousx => self.ferrousx.reload().await,
        }
    }

    async fn start_engine(&self, engine: EngineKind) -> Result<(), PanelError> {
        match engine {
            EngineKind::Nginx => self.nginx.start().await,
            EngineKind::Ferrousx => self.ferrousx.start().await,
        }
    }

    async fn stop_engine(&self, engine: EngineKind) -> Result<(), PanelError> {
        match engine {
            EngineKind::Nginx => self.nginx.stop().await,
            EngineKind::Ferrousx => self.ferrousx.stop().await,
        }
    }

    async fn engine_is_active(&self, engine: EngineKind) -> Result<bool, PanelError> {
        let st = match engine {
            EngineKind::Nginx => self.nginx.status().await?,
            EngineKind::Ferrousx => self.ferrousx.status().await?,
        };
        Ok(st.active)
    }
}

/// Ensures path Containment helper for tests or custom docroots.
pub fn wwwroot_base_path() -> &'static Path {
    Path::new(DEFAULT_WWWROOT)
}
