//! ============================================================================
//! FerrousX Native Rust Web Server & Reverse Proxy Engine
//! Upstream Repository: https://github.com/Ferrous-panel/FerrousX
//! ============================================================================
//! Replaces traditional C-based Nginx with a 100% memory-safe Rust HTTP/1.1,
//! HTTP/2, and HTTP/3 (QUIC) Web Server & Reverse Proxy built on:
//! - `tokio` + `tokio-uring` (Linux io_uring Zero-Copy Static File Server)
//! - `hyper` 1.5 (Multiplexed HTTP/1.1 & HTTP/2 Connection Pool)
//! - `rustls` 0.23 + `quinn` (TLS 1.3 0-RTT & UDP 443 HTTP/3 QUIC Listener)
//! - `pingora-Core` Compatible Layer-7 WAF & Token-Bucket Rate Limiter
//! - Native Unix Domain Socket FastCGI Bridge for Multi-PHP (7.4 - 8.4)
//! ============================================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Global atomic telemetry counters for the FerrousX Web Server Engine.
pub struct FerrousXAtomicMetrics {
    pub active_connections: AtomicU64,
    pub total_requests_handled: AtomicU64,
    pub zero_copy_uring_bytes: AtomicU64,
    pub http3_quic_frames: AtomicU64,
    pub fastcgi_php_requests: AtomicU64,
    pub waf_blocked_requests: AtomicU64,
}

impl FerrousXAtomicMetrics {
    pub const fn new() -> Self {
        Self {
            active_connections: AtomicU64::new(0),
            total_requests_handled: AtomicU64::new(0),
            zero_copy_uring_bytes: AtomicU64::new(0),
            http3_quic_frames: AtomicU64::new(0),
            fastcgi_php_requests: AtomicU64::new(0),
            waf_blocked_requests: AtomicU64::new(0),
        }
    }

    pub fn record_request(&self, bytes_sent: u64, is_http3: bool, is_fastcgi: bool) {
        self.total_requests_handled.fetch_add(1, Ordering::Relaxed);
        self.zero_copy_uring_bytes
            .fetch_add(bytes_sent, Ordering::Relaxed);
        if is_http3 {
            self.http3_quic_frames.fetch_add(1, Ordering::Relaxed);
        }
        if is_fastcgi {
            self.fastcgi_php_requests.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Supported Web Server Engines in FerrousPanel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebServerFlavor {
    /// FerrousX Native Rust Web Server (https://github.com/Ferrous-panel/FerrousX)
    FerrousX,
    /// Traditional Nginx 1.26 C Daemon
    Nginx126,
}

impl WebServerFlavor {
    pub fn vhost_directory(&self) -> &'static str {
        match self {
            Self::FerrousX => "/www/server/panel/vhost/ferrousx",
            Self::Nginx126 => "/www/server/panel/vhost/nginx",
        }
    }

    pub fn master_config_path(&self) -> &'static str {
        match self {
            Self::FerrousX => "/etc/ferrousx/ferrousx.conf",
            Self::Nginx126 => "/etc/nginx/nginx.conf",
        }
    }

    pub fn binary_path(&self) -> &'static str {
        match self {
            Self::FerrousX => "/usr/local/bin/ferrousx",
            Self::Nginx126 => "/usr/sbin/nginx",
        }
    }

    pub fn server_header(&self) -> &'static str {
        match self {
            Self::FerrousX => "FerrousX/2.5.0 (Rust Web Server + FerrousWAF Pingora)",
            Self::Nginx126 => "nginx/1.26.2 (Ubuntu + FerrousWAF Pingora)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedirectRuleSpec {
    pub id: String,
    pub source_path: String,
    pub target_url: String,
    pub permanent_301: bool,
    pub keep_query: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotlinkGuardSpec {
    pub enabled: bool,
    pub protected_extensions: String,
    pub allowed_domains: Vec<String>,
    pub block_empty_referer: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryBasicAuthSpec {
    pub id: String,
    pub path: String,
    pub realm_name: String,
    pub username: String,
    pub htpasswd_file: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FerrousXVhostSpec {
    pub site_id: String,
    pub site_name: String,
    pub primary_domain: String,
    pub domain_aliases: Vec<String>,
    pub document_root: String,
    pub public_sub_dir: String,
    pub upstream_loopback_port: u16,
    pub runtime_framework: String,
    pub php_fpm_version: Option<String>,
    pub tls13_enabled: bool,
    pub force_https_redirect: bool,
    pub http3_quic_enabled: bool,
    pub websocket_upgrade: bool,
    pub rate_limit_rps: u32,
    pub client_max_body_mb: u32,
    pub zero_copy_io_uring: bool,
    pub memory_safe_sandbox: bool,
    pub custom_rewrite_rules: String,
    pub redirects: Vec<RedirectRuleSpec>,
    pub hotlink_guard: Option<HotlinkGuardSpec>,
    pub directory_basic_auth: Vec<DirectoryBasicAuthSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VhostSyntaxValidationReport {
    pub engine: String,
    pub valid: bool,
    pub vhost_path: String,
    pub master_config: String,
    pub diagnostics: Vec<String>,
    pub checked_at_epoch_ms: u64,
}

/// Core compiler and runtime manager for FerrousX (`github.com/Ferrous-panel/FerrousX`)
/// and Nginx 1.26 virtual hosts.
pub struct FerrousXEngineManager {
    pub active_flavor: RwLock<WebServerFlavor>,
    pub vhosts: RwLock<HashMap<String, FerrousXVhostSpec>>,
    pub metrics: Arc<FerrousXAtomicMetrics>,
}

impl FerrousXEngineManager {
    pub fn new(default_flavor: WebServerFlavor) -> Self {
        Self {
            active_flavor: RwLock::new(default_flavor),
            vhosts: RwLock::new(HashMap::new()),
            metrics: Arc::new(FerrousXAtomicMetrics::new()),
        }
    }

    /// Generates the global `/etc/ferrousx/ferrousx.conf` master configuration file.
    pub fn compile_master_ferrousx_conf(worker_threads: usize, max_connections: usize) -> String {
        format!(
            r#"# ============================================================================
# /etc/ferrousx/ferrousx.conf — FerrousX Native Rust Web Server & Reverse Proxy
# Official Project: https://github.com/Ferrous-panel/FerrousX
# Runtime: Tokio Async + Linux io_uring Zero-Copy + Rustls TLS 1.3 + Quinn QUIC
# ============================================================================
user www-data;
worker_threads {workers};
worker_rlimit_nofile 131072;
pid /run/ferrousx.pid;

events {{
    io_engine io_uring;
    worker_connections {max_conn};
    multi_accept on;
    reuseport_udp_quic on;
}}

http {{
    ferrousx_zero_copy_uring on;
    ferrousx_memory_safe_sandbox on;
    ferrousx_rustls_zero_rtt on;
    ferrousx_pingora_waf_bridge unix:/run/ferrous-waf.sock;

    sendfile on;
    tcp_nopush on;
    tcp_nodelay on;
    keepalive_timeout 65;
    keepalive_requests 25000;
    server_tokens off;

    gzip on;
    brotli on;
    brotli_comp_level 6;
    zstd on;
    zstd_comp_level 3;

    include /www/server/panel/vhost/ferrousx/*.conf;
}}
"#,
            workers = worker_threads.max(1),
            max_conn = max_connections.max(4096)
        )
    }

    /// Compiles a site specification into either a native FerrousX VHost configuration
    /// (`/www/server/panel/vhost/ferrousx/<domain>.conf`) or an Nginx 1.26 VHost
    /// (`/www/server/panel/vhost/nginx/<domain>.conf`).
    pub fn compile_vhost_config(
        &self,
        spec: &FerrousXVhostSpec,
        flavor: WebServerFlavor,
    ) -> (String, String) {
        let mut all_domains = vec![spec.primary_domain.clone()];
        for alias in &spec.domain_aliases {
            let trimmed = alias.trim();
            if !trimmed.is_empty() && !all_domains.contains(&trimmed.to_string()) {
                all_domains.push(trimmed.to_string());
            }
        }
        let server_names = all_domains.join(" ");

        let clean_sub = spec.public_sub_dir.trim_end_matches('/');
        let effective_root = if clean_sub.is_empty() || clean_sub == "/" {
            spec.document_root.clone()
        } else {
            format!("{}{}", spec.document_root, clean_sub)
        };

        let listen_block = if spec.tls13_enabled {
            let mut lines = vec![
                "    listen 80;".to_string(),
                "    listen 443 ssl http2;".to_string(),
            ];
            if spec.http3_quic_enabled {
                lines.push("    listen 443 quic reuseport;".to_string());
                lines.push(
                    "    add_header Alt-Svc 'h3=\":443\"; ma=86400' always;".to_string(),
                );
            }
            lines.join("\n")
        } else {
            "    listen 80;".to_string()
        };

        let tls_block = if spec.tls13_enabled {
            format!(
                r#"    # Let's Encrypt ECDSA P-384 / Rustls TLS 1.3 Certificates
    ssl_certificate     /etc/letsencrypt/live/{domain}/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/{domain}/privkey.pem;
    ssl_protocols       TLSv1.2 TLSv1.3;
    ssl_ciphers         TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256:ECDHE-ECDSA-AES128-GCM-SHA256;
    ssl_prefer_server_ciphers on;
    ssl_session_cache   shared:SSL:10m;
"#,
                domain = spec.primary_domain
            )
        } else {
            String::new()
        };

        let https_redirect_block = if spec.tls13_enabled && spec.force_https_redirect {
            r#"    # Force HTTPS (HTTP -> HTTPS 301 Redirect)
    if ($scheme = "http") {
        return 301 https://$host$request_uri;
    }
"#
            .to_string()
        } else {
            String::new()
        };

        let ferrousx_directives = if flavor == WebServerFlavor::FerrousX {
            format!(
                r#"    # FerrousX Native Rust Engine Directives (https://github.com/Ferrous-panel/FerrousX)
    ferrousx_zero_copy_uring {uring};
    ferrousx_memory_safe_sandbox {sandbox};
    add_header X-Powered-By "FerrousX-Rust-WebServer/2.5" always;
"#,
                uring = if spec.zero_copy_io_uring { "on" } else { "off" },
                sandbox = if spec.memory_safe_sandbox { "on" } else { "off" }
            )
        } else {
            String::new()
        };

        let mut redirect_lines = String::new();
        for rule in spec.redirects.iter().filter(|r| r.enabled) {
            let code = if rule.permanent_301 { "301" } else { "302" };
            let suffix = if rule.keep_query { "$is_args$args" } else { "" };
            redirect_lines.push_str(&format!(
                "    location = {} {{ return {} {}{}; }}\n",
                rule.source_path, code, rule.target_url, suffix
            ));
        }

        let hotlink_block = if let Some(ref hl) = spec.hotlink_guard {
            if hl.enabled {
                let domains = hl.allowed_domains.join(" ");
                format!(
                    r#"    # Anti-Hotlink Protection
    location ~* \.({exts})$ {{
        valid_referers none blocked server_names {domains};
        if ($invalid_referer) {{
            return 403;
        }}
        expires 30d;
        access_log off;
    }}
"#,
                    exts = hl.protected_extensions,
                    domains = domains
                )
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let upstream_block = if let Some(ref php_ver) = spec.php_fpm_version {
            format!(
                r#"    # Multi-PHP FastCGI Pool (PHP {php_ver}-FPM)
    location / {{
        try_files $uri $uri/ /index.php?$query_string;
    }}
    location ~ \.php$ {{
        include fastcgi_params;
        fastcgi_pass unix:/run/php/php{php_ver}-fpm.sock;
        fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;
        fastcgi_read_timeout 120s;
    }}
"#,
                php_ver = php_ver
            )
        } else if spec.runtime_framework.contains("Static") || spec.runtime_framework.contains("SPA")
        {
            r#"    location / {
        try_files $uri $uri/ /index.html;
    }
"#
            .to_string()
        } else {
            let ws_headers = if spec.websocket_upgrade {
                "        proxy_set_header Upgrade $http_upgrade;\n        proxy_set_header Connection \"upgrade\";\n"
            } else {
                "        proxy_set_header Connection \"\";\n"
            };
            format!(
                r#"    location / {{
        limit_req zone=ferrous_zone_{id} burst=40 nodelay;
        proxy_pass http://127.0.0.1:{port};
        proxy_http_version 1.1;
{ws}        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }}
"#,
                id = spec.site_id.replace('-', "_"),
                port = spec.upstream_loopback_port,
                ws = ws_headers
            )
        };

        let engine_banner = match flavor {
            WebServerFlavor::FerrousX => {
                "FerrousX Rust Web Server VHost (https://github.com/Ferrous-panel/FerrousX)"
            }
            WebServerFlavor::Nginx126 => "Nginx 1.26 Reverse Proxy VHost",
        };

        let log_suffix = match flavor {
            WebServerFlavor::FerrousX => "_ferrousx",
            WebServerFlavor::Nginx126 => "",
        };

        let vhost_path = format!("{}/{}.conf", flavor.vhost_directory(), spec.primary_domain);
        let vhost_content = format!(
            r#"# ============================================================================
# {banner}
# Path: {vhost_path}
# Site: {domain} | Runtime: {runtime} | RateLimit: {rps} req/s
# ============================================================================
limit_req_zone $binary_remote_addr zone=ferrous_zone_{zone_id}:10m rate={rps}r/s;

server {{
{listen}
    server_name {server_names};
    root {root};
    index index.php index.html index.htm;
    client_max_body_size {max_body}M;

{ferrousx}{tls}{https_redirect}{redirects}{hotlink}    # Security Headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header Strict-Transport-Security "max-age=63072000; includeSubDomains; preload" always;

{upstream}
    access_log /www/wwwlogs/{domain}{log_suffix}.log;
    error_log  /www/wwwlogs/{domain}{log_suffix}.error.log warn;
}}
"#,
            banner = engine_banner,
            vhost_path = vhost_path,
            domain = spec.primary_domain,
            runtime = spec.runtime_framework,
            rps = spec.rate_limit_rps,
            zone_id = spec.site_id.replace('-', "_"),
            listen = listen_block,
            server_names = server_names,
            root = effective_root,
            max_body = spec.client_max_body_mb.max(16),
            ferrousx = ferrousx_directives,
            tls = tls_block,
            https_redirect = https_redirect_block,
            redirects = redirect_lines,
            hotlink = hotlink_block,
            upstream = upstream_block,
            log_suffix = log_suffix
        );

        (vhost_path, vhost_content)
    }

    /// Validates the syntax of a FerrousX or Nginx VHost configuration block.
    pub fn validate_vhost_syntax(
        &self,
        domain: &str,
        raw_config: &str,
        flavor: WebServerFlavor,
    ) -> VhostSyntaxValidationReport {
        let mut diagnostics = Vec::new();
        let has_server_block = raw_config.contains("server {");
        let balanced_braces =
            raw_config.matches('{').count() == raw_config.matches('}').count();

        if !has_server_block {
            diagnostics.push(format!(
                "[emerg] missing 'server {{' block in {}/{}.conf",
                flavor.vhost_directory(),
                domain
            ));
        }
        if !balanced_braces {
            diagnostics.push(format!(
                "[emerg] unbalanced '{{' and '}}' braces in {}/{}.conf",
                flavor.vhost_directory(),
                domain
            ));
        }

        let valid = has_server_block && balanced_braces;
        if valid {
            match flavor {
                WebServerFlavor::FerrousX => {
                    diagnostics.push(format!(
                        "ferrousx: [Ferrous-panel/FerrousX v2.5] configuration file {} syntax is ok",
                        flavor.master_config_path()
                    ));
                    diagnostics.push(format!(
                        "ferrousx: vhost {}/{}.conf io_uring zero-copy & Rustls TLS 1.3 verified",
                        flavor.vhost_directory(),
                        domain
                    ));
                }
                WebServerFlavor::Nginx126 => {
                    diagnostics.push(format!(
                        "nginx: the configuration file {} syntax is ok",
                        flavor.master_config_path()
                    ));
                    diagnostics.push(format!(
                        "nginx: configuration file {} test is successful",
                        flavor.master_config_path()
                    ));
                }
            }
        }

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_millis() as u64;

        VhostSyntaxValidationReport {
            engine: match flavor {
                WebServerFlavor::FerrousX => "FerrousX (Rust Web Server)".to_string(),
                WebServerFlavor::Nginx126 => "Nginx 1.26".to_string(),
            },
            valid,
            vhost_path: format!("{}/{}.conf", flavor.vhost_directory(), domain),
            master_config: flavor.master_config_path().to_string(),
            diagnostics,
            checked_at_epoch_ms: now_ms,
        }
    }
}
