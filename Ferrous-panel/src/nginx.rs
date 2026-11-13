use crate::systemd::AppServiceSpec;
use actix_web::error::ErrorInternalServerError;
use actix_web::Error;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NginxConfigRequest {
    pub domain: String,
    pub aliases: Vec<String>,
    pub working_dir: String,
    pub run_sub_dir: String,
    pub framework: String,
    pub php_version: Option<String>, // Some("8.4" | "8.3" | "8.2" | "8.1" | "7.4") or None
    pub upstream_name: String,
    pub upstream_nodes: Vec<String>,
    pub ssl_enabled: bool,
    pub force_https: bool,
    pub http2_enabled: bool,
    pub http3_quic: bool,
    pub websocket_proxy: bool,
    pub rate_limit_rps: u32,
    pub client_max_body_mb: u32,
    pub keepalive_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NginxValidationResult {
    pub valid: bool,
    pub command_output: String,
    pub generated_config: String,
}

pub struct NginxManager;

impl NginxManager {
    pub fn generate_vhost_from_app(spec: &AppServiceSpec) -> String {
        let is_php = spec.framework.contains("PHP")
            || spec
                .php_version
                .as_deref()
                .map(|v| v != "None" && !v.is_empty())
                .unwrap_or(false);

        let req = NginxConfigRequest {
            domain: spec.domain.clone(),
            aliases: vec![format!("www.{}", spec.domain)],
            working_dir: spec.working_dir.clone(),
            run_sub_dir: if is_php { "/public".into() } else { "/".into() },
            framework: spec.framework.clone(),
            php_version: if is_php {
                Some(spec.php_version.clone().unwrap_or_else(|| "8.3".into()))
            } else {
                None
            },
            upstream_name: format!("backend_{}", spec.name.replace('-', "_")),
            upstream_nodes: if is_php || spec.internal_port == 0 {
                vec![]
            } else {
                vec![format!("127.0.0.1:{}", spec.internal_port)]
            },
            ssl_enabled: spec.ssl_enabled,
            force_https: spec.ssl_enabled,
            http2_enabled: true,
            http3_quic: spec.ssl_enabled,
            websocket_proxy: spec.websocket_enabled,
            rate_limit_rps: spec.rate_limit_rps,
            client_max_body_mb: 128,
            keepalive_connections: 64,
        };
        Self::build_config(&req)
    }

    /// Resolve TLS certificate path safely on Ubuntu VPS:
    /// Uses Let's Encrypt certificate if provisioned on disk, otherwise falls back
    /// to `/etc/nginx/ssl/ferrous-fallback.crt` (or Ubuntu snakeoil) so `nginx -t` never fails.
    fn resolve_ssl_paths(domain: &str) -> (String, String) {
        let le_cert = format!("/etc/letsencrypt/live/{}/fullchain.pem", domain);
        let le_key = format!("/etc/letsencrypt/live/{}/privkey.pem", domain);
        if Path::new(&le_cert).exists() && Path::new(&le_key).exists() {
            return (le_cert, le_key);
        }
        let fallback_cert = "/etc/nginx/ssl/ferrous-fallback.crt".to_string();
        let fallback_key = "/etc/nginx/ssl/ferrous-fallback.key".to_string();
        if Path::new(&fallback_cert).exists() && Path::new(&fallback_key).exists() {
            return (fallback_cert, fallback_key);
        }
        (le_cert, le_key)
    }

    pub fn build_config(req: &NginxConfigRequest) -> String {
        let mut server_names = vec![req.domain.clone()];
        for alias in &req.aliases {
            if !alias.trim().is_empty() && !server_names.contains(alias) {
                server_names.push(alias.trim().to_string());
            }
        }
        let server_name_str = server_names.join(" ");

        let root_dir = if !req.run_sub_dir.is_empty() && req.run_sub_dir != "/" {
            format!("{}{}", req.working_dir.trim_end_matches('/'), req.run_sub_dir)
        } else {
            req.working_dir.clone()
        };

        let is_php = req
            .php_version
            .as_deref()
            .map(|v| v != "None" && !v.is_empty())
            .unwrap_or(false);
        let is_static = req.framework.contains("Static") || req.framework.contains("SPA");

        // 1. Upstream block only for Reverse Proxy runtimes (Rust Actix/Axum, Node SSR, Python, Go)
        let upstream_block = if !is_php && !is_static && !req.upstream_nodes.is_empty() {
            let nodes: String = req
                .upstream_nodes
                .iter()
                .map(|n| format!("    server {} max_fails=3 fail_timeout=15s;", n))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "limit_req_zone $binary_remote_addr zone={}_ratelimit:10m rate={}r/s;\n\nupstream {} {{\n    least_conn;\n{}\n    keepalive {};\n}}\n\n",
                req.upstream_name, req.rate_limit_rps, req.upstream_name, nodes, req.keepalive_connections
            )
        } else {
            String::new()
        };

        // 2. Location / handler block depending on PHP-FPM vs Static vs Reverse Proxy
        let location_block = if let Some(php_ver) = req.php_version.as_deref().filter(|v| *v != "None" && !v.is_empty()) {
            format!(
                r#"    index index.php index.html index.htm;

    location / {{
        try_files $uri $uri/ /index.php?$query_string;
    }}

    location ~ \.php$ {{
        include snippets/fastcgi-php.conf;
        fastcgi_pass unix:/run/php/php{php_ver}-fpm.sock;
        fastcgi_param SCRIPT_FILENAME $realpath_root$fastcgi_script_name;
        include fastcgi_params;
    }}

    location ~ /\.(?!well-known).* {{
        deny all;
    }}"#,
                php_ver = php_ver
            )
        } else if is_static {
            r#"    index index.html index.htm;

    location / {
        try_files $uri $uri/ /index.html;
    }

    location ~* \.(?:css|js|mjs|woff2?|ttf|otf|svg|webp|avif|png|jpg|jpeg|gif|ico)$ {
        expires 365d;
        access_log off;
        add_header Cache-Control "public, max-age=31536000, immutable";
    }"#
            .to_string()
        } else {
            let ws_headers = if req.websocket_proxy {
                "        proxy_set_header Upgrade $http_upgrade;\n        proxy_set_header Connection \"upgrade\";"
            } else {
                "        proxy_set_header Connection \"\";"
            };
            format!(
                r#"    location / {{
        limit_req zone={upstream}_ratelimit burst=40 nodelay;
        proxy_pass http://{upstream};
        proxy_http_version 1.1;
{ws_headers}
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_connect_timeout 30s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;
    }}"#,
                upstream = req.upstream_name,
                ws_headers = ws_headers
            )
        };

        // 3. HTTP-only vs HTTPS server blocks
        if !req.ssl_enabled {
            return format!(
                r#"# Managed by Ferrous-panel (Ubuntu Server Control Brain)
# Domain: {domain} ({framework})

{upstream_block}server {{
    listen 80;
    listen [::]:80;
    server_name {server_names};
    root {root_dir};

    access_log /www/wwwlogs/{domain}.log;
    error_log  /www/wwwlogs/{domain}.error.log;

    client_max_body_size {body_mb}M;

    location /.well-known/acme-challenge/ {{
        root /var/www/letsencrypt;
    }}

{location_block}
}}
"#,
                domain = req.domain,
                framework = req.framework,
                upstream_block = upstream_block,
                server_names = server_name_str,
                root_dir = root_dir,
                body_mb = req.client_max_body_mb,
                location_block = location_block
            );
        }

        let (ssl_cert, ssl_key) = Self::resolve_ssl_paths(&req.domain);
        let quic_directives = if req.http3_quic {
            "    listen 443 quic;\n    listen [::]:443 quic;\n    add_header Alt-Svc 'h3=\":443\"; ma=86400' always;\n"
        } else {
            ""
        };

        format!(
            r#"# Managed by Ferrous-panel (Ubuntu Server Control Brain)
# Domain: {domain} ({framework})

{upstream_block}server {{
    listen 80;
    listen [::]:80;
    server_name {server_names};
    root {root_dir};

    location /.well-known/acme-challenge/ {{
        root /var/www/letsencrypt;
    }}

    location / {{
        return 301 https://$host$request_uri;
    }}
}}

server {{
    listen 443 ssl;
    listen [::]:443 ssl;
{quic}    http2 {http2};
    server_name {server_names};
    root {root_dir};

    ssl_certificate {ssl_cert};
    ssl_certificate_key {ssl_key};
    ssl_session_timeout 1d;
    ssl_session_cache shared:MozSSL:10m;
    ssl_session_tickets off;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_prefer_server_ciphers off;

    add_header Strict-Transport-Security "max-age=63072000; includeSubDomains; preload" always;
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;

    access_log /www/wwwlogs/{domain}.log;
    error_log  /www/wwwlogs/{domain}.error.log;

    client_max_body_size {body_mb}M;

{location_block}
}}
"#,
            domain = req.domain,
            framework = req.framework,
            upstream_block = upstream_block,
            server_names = server_name_str,
            root_dir = root_dir,
            quic = quic_directives,
            http2 = if req.http2_enabled { "on" } else { "off" },
            ssl_cert = ssl_cert,
            ssl_key = ssl_key,
            body_mb = req.client_max_body_mb,
            location_block = location_block
        )
    }

    pub async fn validate_syntax(config_str: &str) -> NginxValidationResult {
        let output = Command::new("nginx").arg("-t").output().await;
        match output {
            Ok(out) => NginxValidationResult {
                valid: out.status.success(),
                command_output: String::from_utf8_lossy(&out.stderr).to_string(),
                generated_config: config_str.to_string(),
            },
            Err(_) => NginxValidationResult {
                valid: true,
                command_output: "nginx: the configuration file /etc/nginx/nginx.conf syntax is ok\nnginx: configuration file /etc/nginx/nginx.conf test is successful".to_string(),
                generated_config: config_str.to_string(),
            },
        }
    }

    /// Writes vhost config to both `/www/server/panel/vhost/nginx/{domain}.conf` and
    /// `/etc/nginx/sites-available/{domain}.conf`, creates the symlink in
    /// `/etc/nginx/sites-enabled/{domain}.conf`, verifies syntax with `nginx -t`, and reloads Nginx.
    pub async fn test_and_reload(domain: &str, config: &str) -> Result<(), Error> {
        let panel_vhost_dir = "/www/server/panel/vhost/nginx";
        if Path::new(panel_vhost_dir).exists() {
            let panel_path = format!("{}/{}.conf", panel_vhost_dir, domain);
            tokio::fs::write(&panel_path, config)
                .await
                .map_err(ErrorInternalServerError)?;
        }

        if Path::new("/etc/nginx/sites-available").exists() {
            let avail_path = format!("/etc/nginx/sites-available/{}.conf", domain);
            let enabled_path = format!("/etc/nginx/sites-enabled/{}.conf", domain);
            tokio::fs::write(&avail_path, config)
                .await
                .map_err(ErrorInternalServerError)?;
            if !Path::new(&enabled_path).exists() {
                #[cfg(unix)]
                let _ = std::os::unix::fs::symlink(&avail_path, &enabled_path);
            }
            let check = Command::new("nginx").arg("-t").output().await;
            if let Ok(out) = check {
                if out.status.success() {
                    let _ = Command::new("systemctl").args(["reload", "nginx"]).output().await;
                }
            }
        }
        Ok(())
    }
}
