use crate::error::ServerError;
use crate::tls::load_certified_key;
use clap::Parser;
use rustls::sign::CertifiedKey;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

pub const DEFAULT_MASTER_CONFIG: &str = "/etc/ferrousx/ferrousx.toml";
pub const DEFAULT_FERROUSX_ENV: &str = "/etc/ferrousx/ferrousx.env";
pub const DEFAULT_SITES_DIR: &str = "/etc/ferrousx/sites";
pub const DEFAULT_ACME_WEBROOT: &str = "/www/acme";
pub const DEFAULT_CONTROL_SOCKET: &str = "/run/ferrousx/panel-admin.sock";
pub const DEFAULT_ADMIN_TOKEN_FILE: &str = "/etc/ferrousx/admin.token";

/// Loads key/value pairs from a `.env` / `ferrousx.env` file safely without shell evaluation.
#[must_use]
pub fn load_env_file_map(path: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Ok(raw) = std::fs::read_to_string(path) else {
        return map;
    };
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let cleaned = trimmed.strip_prefix("export ").unwrap_or(trimmed).trim();
        if let Some((k, v)) = cleaned.split_once('=') {
            let key = k.trim();
            if key.is_empty() {
                continue;
            }
            let val = v
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
                .to_string();
            map.insert(key.to_string(), val);
        }
    }
    map
}

/// Loads merged environment variables following upstream FerrousX precedence:
/// `Process ENV > $FERROUSX_ENV_FILE > ./.env > /etc/ferrousx/ferrousx.env > ferrousx.toml`.
#[must_use]
pub fn load_default_env_map() -> HashMap<String, String> {
    let mut merged = load_env_file_map(Path::new(DEFAULT_FERROUSX_ENV));
    let local_env = Path::new(".env");
    if local_env.is_file() {
        merged.extend(load_env_file_map(local_env));
    }
    if let Ok(custom_path) = std::env::var("FERROUSX_ENV_FILE") {
        let trimmed = custom_path.trim();
        if !trimmed.is_empty() {
            merged.extend(load_env_file_map(Path::new(trimmed)));
        }
    }
    merged
}

/// Safely resolves an environment variable (process env takes priority over `ferrousx.env`).
#[must_use]
pub fn resolve_env_var(key: &str, file_env: &HashMap<String, String>) -> Option<String> {
    if let Ok(val) = std::env::var(key) {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    file_env
        .get(key)
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn parse_env_bool(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// CLI flags for `ferrousx-bin` (compatible with upstream Ferrous-panel/FerrousX).
#[derive(Debug, Clone, Parser)]
#[command(
    name = "ferrousx-bin",
    version,
    about = "FerrousX pure-Rust HTTP/TLS web server & reverse proxy"
)]
pub struct CliArgs {
    /// Path to master ferrousx.toml configuration.
    #[arg(short = 'c', long, env = "FERROUSX_CONFIG", default_value = DEFAULT_MASTER_CONFIG)]
    pub config: PathBuf,

    /// Validate master config, sites/*.toml, and TLS certificates then exit (0 = valid, 1 = invalid).
    #[arg(short = 't', long = "check-config", alias = "test", default_value_t = false)]
    pub check_config: bool,
}

// ============================================================================
// TOML Deserialization Models
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MasterConfigToml {
    #[serde(alias = "workers")]
    pub worker_threads: usize,
    #[serde(alias = "bind_http_addr")]
    pub bind_http: String,
    #[serde(alias = "bind_https_addr")]
    pub bind_https: String,
    pub sites_dir: PathBuf,
    pub acme_webroot: PathBuf,
    pub acme_challenge_prefix: String,
    pub server_tokens: bool,
    #[serde(alias = "client_header_timeout_ms")]
    pub header_read_timeout_ms: u64,
    #[serde(alias = "proxy_read_timeout_ms")]
    pub request_timeout_ms: u64,
    #[serde(alias = "proxy_connect_timeout_ms")]
    pub upstream_connect_timeout_ms: u64,
    pub max_header_bytes: usize,
    #[serde(alias = "client_max_body_size")]
    pub max_body_bytes: usize,
    #[serde(alias = "panel_uds")]
    pub control_socket: Option<PathBuf>,
    pub admin_token_file: PathBuf,
    pub allow_read_group_gid: Option<u32>,
    pub require_peer_uid_auth: bool,
    pub max_failed_attempts: u32,
    pub brute_force_lockout_secs: u64,
    pub waf_mode: String,
    #[serde(default = "default_true")]
    pub enable_phpmyadmin_route: bool,
    #[serde(default = "default_phpmyadmin_upstream")]
    pub phpmyadmin_upstream: String,
}

fn default_phpmyadmin_upstream() -> String {
    "127.0.0.1:888".to_string()
}

impl Default for MasterConfigToml {
    fn default() -> Self {
        Self {
            worker_threads: 0,
            bind_http: "0.0.0.0:80".to_string(),
            bind_https: "0.0.0.0:443".to_string(),
            sites_dir: PathBuf::from(DEFAULT_SITES_DIR),
            acme_webroot: PathBuf::from(DEFAULT_ACME_WEBROOT),
            acme_challenge_prefix: "/.well-known/acme-challenge/".to_string(),
            server_tokens: false,
            header_read_timeout_ms: 10_000,
            request_timeout_ms: 30_000,
            upstream_connect_timeout_ms: 5_000,
            max_header_bytes: 16 * 1024,
            max_body_bytes: 10 * 1024 * 1024,
            control_socket: None,
            admin_token_file: PathBuf::from(DEFAULT_ADMIN_TOKEN_FILE),
            allow_read_group_gid: None,
            require_peer_uid_auth: true,
            max_failed_attempts: 5,
            brute_force_lockout_secs: 60,
            waf_mode: "block".to_string(),
            enable_phpmyadmin_route: true,
            phpmyadmin_upstream: default_phpmyadmin_upstream(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SiteConfigToml {
    pub domain: String,
    #[serde(default)]
    pub server_names: Vec<String>,
    pub root: PathBuf,
    #[serde(default)]
    pub acme: AcmeSectionToml,
    #[serde(default)]
    pub ssl: SslSectionToml,
    pub handler: HandlerSectionToml,
    #[serde(default)]
    pub waf_mode: Option<String>,
    #[serde(default)]
    pub rate_limit_rps: u32,
    #[serde(default)]
    pub http3_quic: bool,
    #[serde(default)]
    pub redirects: Vec<RedirectRuleToml>,
    #[serde(default)]
    pub hotlink: Option<HotlinkSectionToml>,
    #[serde(default)]
    pub directory_auth: Vec<DirectoryAuthToml>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedirectRuleToml {
    pub source_path: String,
    pub target_url: String,
    #[serde(default = "default_redirect_status")]
    pub status_code: u16,
    #[serde(default = "default_true")]
    pub keep_query: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct HotlinkSectionToml {
    pub enabled: bool,
    pub protected_extensions: Vec<String>,
    pub allowed_domains: Vec<String>,
    pub block_empty_referer: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DirectoryAuthToml {
    pub path: String,
    #[serde(default)]
    pub label: String,
    pub htpasswd_file: PathBuf,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AcmeSectionToml {
    pub prefix: String,
    pub webroot: PathBuf,
    pub serve_before_redirect: bool,
}

impl Default for AcmeSectionToml {
    fn default() -> Self {
        Self {
            prefix: "/.well-known/acme-challenge/".to_string(),
            webroot: PathBuf::from(DEFAULT_ACME_WEBROOT),
            serve_before_redirect: true,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SslSectionToml {
    pub enabled: bool,
    pub redirect_https: bool,
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HandlerSectionToml {
    StaticSpa {
        #[serde(default = "default_index_html")]
        index_file: String,
        #[serde(default)]
        spa_fallback: bool,
        #[serde(default = "default_true")]
        clean_urls: bool,
        #[serde(default = "default_true")]
        precompressed: bool,
        #[serde(default = "default_true")]
        immutable_assets: bool,
    },
    ReverseProxy {
        upstream: String,
        #[serde(default = "default_true")]
        pass_host_header: bool,
        #[serde(default = "default_true")]
        forwarded_headers: bool,
    },
    PhpFpm {
        #[serde(default)]
        php_version: String,
        fastcgi_socket: PathBuf,
        #[serde(default = "default_index_php")]
        index_file: String,
    },
    RedirectHttps {
        #[serde(default = "default_redirect_status")]
        status_code: u16,
    },
}

fn default_index_html() -> String {
    "index.html".to_string()
}

fn default_index_php() -> String {
    "index.php".to_string()
}

const fn default_true() -> bool {
    true
}

const fn default_redirect_status() -> u16 {
    301
}

// ============================================================================
// Validated Runtime Models
// ============================================================================

/// Validated DNS domain or hostname label sequence (lowercase ASCII).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HostLabel(String);

impl HostLabel {
    pub fn parse(raw: &str) -> Result<Self, ServerError> {
        let trimmed = raw.trim().trim_end_matches('.').to_ascii_lowercase();
        if trimmed.is_empty() || trimmed.len() > 253 {
            return Err(ServerError::Config(format!(
                "invalid hostname '{raw}': length must be 1..=253"
            )));
        }
        if !trimmed.contains('.') && trimmed != "localhost" {
            return Err(ServerError::Config(format!(
                "invalid hostname '{raw}': must contain a dot or be 'localhost'"
            )));
        }
        for label in trimmed.split('.') {
            if label.is_empty() || label.len() > 63 {
                return Err(ServerError::Config(format!(
                    "invalid hostname label in '{raw}'"
                )));
            }
            if label.starts_with('-') || label.ends_with('-') {
                return Err(ServerError::Config(format!(
                    "hostname label cannot start or end with '-' in '{raw}'"
                )));
            }
            if !label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            {
                return Err(ServerError::Config(format!(
                    "hostname contains invalid characters in '{raw}'"
                )));
            }
        }
        Ok(Self(trimmed))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HostLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Validated single filename (e.g. `index.html`, `index.php`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeFileName(String);

impl SafeFileName {
    pub fn parse(raw: &str) -> Result<Self, ServerError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.len() > 64 {
            return Err(ServerError::Config(format!(
                "invalid filename '{raw}': length must be 1..=64"
            )));
        }
        if trimmed.starts_with('.') || trimmed.contains("..") {
            return Err(ServerError::Config(format!(
                "invalid filename '{raw}': cannot start with '.' or contain '..'"
            )));
        }
        if !trimmed
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        {
            return Err(ServerError::Config(format!(
                "invalid filename '{raw}': disallowed characters"
            )));
        }
        Ok(Self(trimmed.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct UpstreamOrigin {
    pub scheme: String,
    pub authority: String,
    pub connect_addr: String,
}

impl UpstreamOrigin {
    pub fn parse(raw: &str) -> Result<Self, ServerError> {
        let trimmed = raw.trim().trim_end_matches('/');
        let (scheme, rest) = trimmed
            .split_once("://")
            .ok_or_else(|| ServerError::Config(format!("upstream '{raw}' must include scheme http://")))?;
        if !scheme.eq_ignore_ascii_case("http") {
            return Err(ServerError::Config(format!(
                "upstream '{raw}' scheme must be 'http'"
            )));
        }
        if rest.is_empty() || rest.contains('/') || rest.contains('?') || rest.contains('#') {
            return Err(ServerError::Config(format!(
                "upstream '{raw}' cannot contain paths or query strings"
            )));
        }
        if rest.bytes().any(|b| b.is_ascii_whitespace() || b == b'\0') {
            return Err(ServerError::Config(format!(
                "upstream '{raw}' contains illegal whitespace/NUL"
            )));
        }
        let (_host, port_str) = rest.rsplit_once(':').ok_or_else(|| {
            ServerError::Config(format!("upstream '{raw}' must include an explicit :port"))
        })?;
        let port: u16 = port_str.parse().map_err(|_| {
            ServerError::Config(format!("upstream '{raw}' has invalid port '{port_str}'"))
        })?;
        if port == 0 {
            return Err(ServerError::Config(format!(
                "upstream '{raw}' port cannot be 0"
            )));
        }

        Ok(Self {
            scheme: "http".to_string(),
            authority: rest.to_string(),
            connect_addr: rest.to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub enum SiteHandler {
    StaticSpa {
        index_file: SafeFileName,
        spa_fallback: bool,
        clean_urls: bool,
        precompressed: bool,
        immutable_assets: bool,
    },
    ReverseProxy {
        upstream: UpstreamOrigin,
        pass_host_header: bool,
        forwarded_headers: bool,
    },
    PhpFpm {
        fastcgi_socket: PathBuf,
        index_file: SafeFileName,
    },
    RedirectHttps {
        status_code: u16,
    },
}

#[derive(Debug, Clone)]
pub struct ValidatedRedirectRule {
    pub source_path: String,
    pub target_url: String,
    pub status_code: u16,
    pub keep_query: bool,
}

#[derive(Debug, Clone)]
pub struct ValidatedHotlinkConfig {
    pub protected_extensions: Vec<String>,
    pub allowed_domains: Vec<String>,
    pub block_empty_referer: bool,
}

#[derive(Debug, Clone)]
pub struct ValidatedDirectoryAuth {
    pub path: String,
    pub label: String,
    pub htpasswd_file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ValidatedSite {
    pub domain: HostLabel,
    pub server_names: Vec<HostLabel>,
    pub root: PathBuf,
    pub acme_prefix: String,
    pub acme_webroot: PathBuf,
    pub acme_serve_before_redirect: bool,
    pub ssl_enabled: bool,
    pub redirect_https: bool,
    pub handler: SiteHandler,
    pub tls_key: Option<Arc<CertifiedKey>>,
    pub rate_limit_rps: u32,
    pub redirects: Vec<ValidatedRedirectRule>,
    pub hotlink: Option<ValidatedHotlinkConfig>,
    pub directory_auth: Vec<ValidatedDirectoryAuth>,
}

#[derive(Debug, Clone)]
pub struct ValidatedMasterConfig {
    pub worker_threads: usize,
    pub bind_http: SocketAddr,
    pub bind_https: SocketAddr,
    pub sites_dir: PathBuf,
    pub acme_webroot: PathBuf,
    pub acme_challenge_prefix: String,
    pub server_tokens: bool,
    pub header_read_timeout: Duration,
    pub request_timeout: Duration,
    pub upstream_connect_timeout: Duration,
    pub max_header_bytes: usize,
    pub max_body_bytes: usize,
    pub control_socket: Option<PathBuf>,
    pub admin_token_file: PathBuf,
    pub allow_read_group_gid: Option<u32>,
    pub require_peer_uid_auth: bool,
    pub max_failed_attempts: u32,
    pub brute_force_lockout_secs: u64,
    pub waf_mode: String,
    pub enable_phpmyadmin_route: bool,
    pub phpmyadmin_upstream: Option<UpstreamOrigin>,
}

#[derive(Debug)]
pub struct LoadedBundle {
    pub master: ValidatedMasterConfig,
    pub sites: Vec<ValidatedSite>,
    pub host_index: HashMap<String, usize>,
    pub sni_certs: HashMap<String, Arc<CertifiedKey>>,
}

pub fn validate_absolute_clean_path(path: &Path, label: &str) -> Result<PathBuf, ServerError> {
    if !path.is_absolute() {
        return Err(ServerError::Config(format!(
            "{label} must be an absolute path: {}",
            path.display()
        )));
    }
    let raw_str = path.to_string_lossy();
    if raw_str.contains('\0') {
        return Err(ServerError::Config(format!(
            "{label} contains NUL byte"
        )));
    }
    let mut normalized = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::RootDir => normalized.push(comp.as_os_str()),
            Component::Normal(seg) => normalized.push(seg),
            Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) => {
                return Err(ServerError::Config(format!(
                    "{label} cannot contain '..' traversal: {}",
                    path.display()
                )));
            }
        }
    }
    Ok(normalized)
}

/// Loads and strictly validates `ferrousx.toml`, all `sites/*.toml`, and configured TLS certs/keys.
pub fn load_bundle(master_path: &Path) -> Result<LoadedBundle, ServerError> {
    let raw_master = std::fs::read_to_string(master_path).map_err(|err| {
        ServerError::Config(format!(
            "cannot read master config {}: {err}",
            master_path.display()
        ))
    })?;
    let mut parsed_master: MasterConfigToml = toml::from_str(&raw_master)?;
    let env_map = load_default_env_map();
    if let Some(v) = resolve_env_var("FERROUSX_BIND_HTTP", &env_map) {
        parsed_master.bind_http = v;
    }
    if let Some(v) = resolve_env_var("FERROUSX_BIND_HTTPS", &env_map) {
        parsed_master.bind_https = v;
    }
    if let Some(v) = resolve_env_var("FERROUSX_WORKERS", &env_map) {
        if let Ok(parsed) = v.parse::<usize>() {
            parsed_master.worker_threads = parsed;
        }
    }
    if let Some(v) = resolve_env_var("FERROUSX_SITES_DIR", &env_map) {
        parsed_master.sites_dir = PathBuf::from(v);
    }
    if let Some(v) = resolve_env_var("FERROUSX_ACME_WEBROOT", &env_map) {
        parsed_master.acme_webroot = PathBuf::from(v);
    }
    if let Some(v) = resolve_env_var("FERROUSX_SERVER_TOKENS", &env_map).and_then(|s| parse_env_bool(&s)) {
        parsed_master.server_tokens = v;
    }
    if let Some(v) = resolve_env_var("FERROUSX_CONTROL_SOCKET", &env_map)
        .or_else(|| resolve_env_var("FERROUSX_PANEL_UDS", &env_map))
    {
        parsed_master.control_socket = Some(PathBuf::from(v));
    }
    if let Some(v) = resolve_env_var("FERROUSX_ADMIN_TOKEN_FILE", &env_map) {
        parsed_master.admin_token_file = PathBuf::from(v);
    }
    if let Some(v) = resolve_env_var("FERROUSX_WAF_MODE", &env_map) {
        parsed_master.waf_mode = v;
    }
    if let Some(v) = resolve_env_var("FERROUSX_ENABLE_PHPMYADMIN", &env_map).and_then(|s| parse_env_bool(&s)) {
        parsed_master.enable_phpmyadmin_route = v;
    }
    if let Some(v) = resolve_env_var("FERROUSX_PHPMYADMIN_UPSTREAM", &env_map) {
        parsed_master.phpmyadmin_upstream = v;
    }

    let bind_http: SocketAddr = parsed_master.bind_http.trim().parse().map_err(|err| {
        ServerError::Config(format!(
            "invalid bind_http '{}': {err}",
            parsed_master.bind_http
        ))
    })?;
    let bind_https: SocketAddr = parsed_master.bind_https.trim().parse().map_err(|err| {
        ServerError::Config(format!(
            "invalid bind_https '{}': {err}",
            parsed_master.bind_https
        ))
    })?;

    if parsed_master.header_read_timeout_ms == 0
        || parsed_master.request_timeout_ms == 0
        || parsed_master.upstream_connect_timeout_ms == 0
    {
        return Err(ServerError::Config(
            "all timeout_ms values must be greater than zero".to_string(),
        ));
    }
    if parsed_master.max_header_bytes < 1024 || parsed_master.max_body_bytes == 0 {
        return Err(ServerError::Config(
            "max_header_bytes must be >= 1024 and max_body_bytes > 0".to_string(),
        ));
    }

    let sites_dir = validate_absolute_clean_path(&parsed_master.sites_dir, "sites_dir")?;
    let acme_webroot = validate_absolute_clean_path(&parsed_master.acme_webroot, "acme_webroot")?;
    let acme_prefix = parsed_master.acme_challenge_prefix.trim().to_string();
    if !acme_prefix.starts_with('/') {
        return Err(ServerError::Config(
            "acme_challenge_prefix must start with '/'".to_string(),
        ));
    }
    let control_socket = match parsed_master.control_socket {
        Some(ref sock) => Some(validate_absolute_clean_path(sock, "control_socket")?),
        None => None,
    };
    let admin_token_file =
        validate_absolute_clean_path(&parsed_master.admin_token_file, "admin_token_file")?;

    let phpmyadmin_upstream = if parsed_master.enable_phpmyadmin_route {
        let raw_pma = parsed_master.phpmyadmin_upstream.trim();
        if raw_pma.is_empty() {
            Some(UpstreamOrigin::parse("http://127.0.0.1:888")?)
        } else if raw_pma.starts_with("http://") {
            Some(UpstreamOrigin::parse(raw_pma)?)
        } else {
            Some(UpstreamOrigin::parse(&format!("http://{raw_pma}"))?)
        }
    } else {
        None
    };

    let master = ValidatedMasterConfig {
        worker_threads: parsed_master.worker_threads,
        bind_http,
        bind_https,
        sites_dir: sites_dir.clone(),
        acme_webroot,
        acme_challenge_prefix: acme_prefix,
        server_tokens: parsed_master.server_tokens,
        header_read_timeout: Duration::from_millis(parsed_master.header_read_timeout_ms),
        request_timeout: Duration::from_millis(parsed_master.request_timeout_ms),
        upstream_connect_timeout: Duration::from_millis(parsed_master.upstream_connect_timeout_ms),
        max_header_bytes: parsed_master.max_header_bytes,
        max_body_bytes: parsed_master.max_body_bytes,
        control_socket,
        admin_token_file,
        allow_read_group_gid: parsed_master.allow_read_group_gid,
        require_peer_uid_auth: parsed_master.require_peer_uid_auth,
        max_failed_attempts: parsed_master.max_failed_attempts.max(3),
        brute_force_lockout_secs: parsed_master.brute_force_lockout_secs.max(30),
        waf_mode: parsed_master.waf_mode,
        enable_phpmyadmin_route: parsed_master.enable_phpmyadmin_route,
        phpmyadmin_upstream,
    };

    let mut site_files: Vec<PathBuf> = Vec::new();
    if sites_dir.exists() {
        for entry_res in std::fs::read_dir(&sites_dir)? {
            let entry = entry_res?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("toml") {
                site_files.push(path);
            }
        }
        site_files.sort();
    }

    let mut sites = Vec::with_capacity(site_files.len());
    let mut host_index: HashMap<String, usize> = HashMap::new();
    let mut sni_certs: HashMap<String, Arc<CertifiedKey>> = HashMap::new();

    for site_file in site_files {
        let raw_site = std::fs::read_to_string(&site_file)?;
        let parsed_site: SiteConfigToml = toml::from_str(&raw_site).map_err(|err| {
            ServerError::Config(format!("failed to parse {}: {err}", site_file.display()))
        })?;

        let validated = validate_site(parsed_site, &site_file, &master)?;
        let site_idx = sites.len();

        for host in &validated.server_names {
            let key = host.as_str().to_string();
            if host_index.contains_key(&key) {
                return Err(ServerError::Config(format!(
                    "duplicate server_name '{key}' declared in {}",
                    site_file.display()
                )));
            }
            host_index.insert(key.clone(), site_idx);
            if let Some(ref cert) = validated.tls_key {
                sni_certs.insert(key, Arc::clone(cert));
            }
        }

        sites.push(validated);
    }

    Ok(LoadedBundle {
        master,
        sites,
        host_index,
        sni_certs,
    })
}

fn validate_site(
    raw: SiteConfigToml,
    source_file: &Path,
    master: &ValidatedMasterConfig,
) -> Result<ValidatedSite, ServerError> {
    let domain = HostLabel::parse(&raw.domain)?;
    let mut seen = HashSet::new();
    let mut server_names = Vec::new();

    seen.insert(domain.as_str().to_string());
    server_names.push(domain.clone());
    for alias in &raw.server_names {
        let parsed = HostLabel::parse(alias)?;
        if seen.insert(parsed.as_str().to_string()) {
            server_names.push(parsed);
        }
    }

    let root = validate_absolute_clean_path(&raw.root, "site root")?;
    let site_acme_root = if raw.acme.webroot.as_os_str().is_empty() {
        master.acme_webroot.clone()
    } else {
        validate_absolute_clean_path(&raw.acme.webroot, "acme webroot")?
    };
    let acme_prefix = if raw.acme.prefix.trim().is_empty() {
        master.acme_challenge_prefix.clone()
    } else {
        let p = raw.acme.prefix.trim().to_string();
        if !p.starts_with('/') {
            return Err(ServerError::Config(format!(
                "invalid acme.prefix in {}",
                source_file.display()
            )));
        }
        p
    };

    if raw.ssl.redirect_https && !raw.ssl.enabled {
        return Err(ServerError::Config(format!(
            "ssl.redirect_https = true requires ssl.enabled = true in {}",
            source_file.display()
        )));
    }

    let tls_key = if raw.ssl.enabled {
        let cert_path = raw.ssl.cert_path.as_ref().ok_or_else(|| {
            ServerError::Config(format!(
                "ssl.cert_path required when ssl.enabled = true in {}",
                source_file.display()
            ))
        })?;
        let key_path = raw.ssl.key_path.as_ref().ok_or_else(|| {
            ServerError::Config(format!(
                "ssl.key_path required when ssl.enabled = true in {}",
                source_file.display()
            ))
        })?;
        let clean_cert = validate_absolute_clean_path(cert_path, "ssl.cert_path")?;
        let clean_key = validate_absolute_clean_path(key_path, "ssl.key_path")?;
        Some(load_certified_key(&clean_cert, &clean_key)?)
    } else {
        None
    };

    let handler = match raw.handler {
        HandlerSectionToml::StaticSpa {
            index_file,
            spa_fallback,
            clean_urls,
            precompressed,
            immutable_assets,
        } => SiteHandler::StaticSpa {
            index_file: SafeFileName::parse(&index_file)?,
            spa_fallback,
            clean_urls,
            precompressed,
            immutable_assets,
        },
        HandlerSectionToml::ReverseProxy {
            upstream,
            pass_host_header,
            forwarded_headers,
        } => SiteHandler::ReverseProxy {
            upstream: UpstreamOrigin::parse(&upstream)?,
            pass_host_header,
            forwarded_headers,
        },
        HandlerSectionToml::PhpFpm {
            php_version: _,
            fastcgi_socket,
            index_file,
        } => {
            let clean_sock = validate_absolute_clean_path(&fastcgi_socket, "fastcgi_socket")?;
            SiteHandler::PhpFpm {
                fastcgi_socket: clean_sock,
                index_file: SafeFileName::parse(&index_file)?,
            }
        }
        HandlerSectionToml::RedirectHttps { status_code } => {
            if !matches!(status_code, 301 | 302 | 307 | 308) {
                return Err(ServerError::Config(format!(
                    "redirect_https status_code must be 301, 302, 307, or 308 in {}",
                    source_file.display()
                )));
            }
            SiteHandler::RedirectHttps { status_code }
        }
    };

    if raw.http3_quic {
        return Err(ServerError::Config(format!(
            "http3_quic = true is not supported by FerrousX UDP/QUIC listener yet in {}",
            source_file.display()
        )));
    }

    let mut redirects = Vec::new();
    for rule in raw.redirects {
        if !rule.enabled {
            continue;
        }
        let src = rule.source_path.trim().to_string();
        let target = rule.target_url.trim().to_string();
        if !src.starts_with('/') || src.contains('\r') || src.contains('\n') {
            return Err(ServerError::Config(format!(
                "invalid redirect source_path '{}' in {}",
                rule.source_path,
                source_file.display()
            )));
        }
        if target.is_empty() || target.contains('\r') || target.contains('\n') {
            return Err(ServerError::Config(format!(
                "invalid redirect target_url '{}' in {}",
                rule.target_url,
                source_file.display()
            )));
        }
        if !matches!(rule.status_code, 301 | 302 | 307 | 308) {
            return Err(ServerError::Config(format!(
                "redirect status_code must be 301, 302, 307, or 308 in {}",
                source_file.display()
            )));
        }
        redirects.push(ValidatedRedirectRule {
            source_path: src,
            target_url: target,
            status_code: rule.status_code,
            keep_query: rule.keep_query,
        });
    }

    let hotlink = raw.hotlink.and_then(|hl| {
        if !hl.enabled {
            return None;
        }
        let protected_extensions: Vec<String> = hl
            .protected_extensions
            .into_iter()
            .map(|e| e.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|e| !e.is_empty())
            .collect();
        if protected_extensions.is_empty() {
            return None;
        }
        let allowed_domains: Vec<String> = hl
            .allowed_domains
            .into_iter()
            .map(|d| d.trim().to_ascii_lowercase())
            .filter(|d| !d.is_empty())
            .collect();
        Some(ValidatedHotlinkConfig {
            protected_extensions,
            allowed_domains,
            block_empty_referer: hl.block_empty_referer,
        })
    });

    let mut directory_auth = Vec::new();
    for auth in raw.directory_auth {
        if !auth.enabled {
            continue;
        }
        let clean_path = auth.path.trim().to_string();
        if !clean_path.starts_with('/') || clean_path.contains('\r') || clean_path.contains('\n') {
            return Err(ServerError::Config(format!(
                "invalid directory_auth.path '{}' in {}",
                auth.path,
                source_file.display()
            )));
        }
        let htpasswd_file =
            validate_absolute_clean_path(&auth.htpasswd_file, "directory_auth.htpasswd_file")?;
        let label = if auth.label.trim().is_empty() {
            "Restricted Area".to_string()
        } else {
            auth.label.trim().replace('"', "'")
        };
        directory_auth.push(ValidatedDirectoryAuth {
            path: clean_path,
            label,
            htpasswd_file,
        });
    }

    Ok(ValidatedSite {
        domain,
        server_names,
        root,
        acme_prefix,
        acme_webroot: site_acme_root,
        acme_serve_before_redirect: raw.acme.serve_before_redirect,
        ssl_enabled: raw.ssl.enabled,
        redirect_https: raw.ssl.redirect_https,
        handler,
        tls_key,
        rate_limit_rps: raw.rate_limit_rps,
        redirects,
        hotlink,
        directory_auth,
    })
}
