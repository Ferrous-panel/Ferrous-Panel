use crate::error::PanelError;
use crate::system::cmd::AllowedBinary;
use crate::system::paths::SafePath;
use crate::system::validate::{Domain, ServiceName};
use crate::webserver::{
    atomic_symlink, atomic_write_file, parse_systemctl_status, write_site_htpasswd_file, BoxFuture,
    CommandExecutor, EngineKind, EngineStatus, FileSnapshot, RenderedSite, SiteMode, SiteSpec,
    SymlinkSnapshot, ValidationReport, WebServerEngine, DEFAULT_ACME_WEBROOT,
    DEFAULT_LETSENCRYPT_LIVE,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

const CMD_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub struct NginxPaths {
    pub sites_available: PathBuf,
    pub sites_enabled: PathBuf,
    pub acme_webroot: SafePath,
    pub letsencrypt_live: SafePath,
    pub service_name: ServiceName,
}

impl NginxPaths {
    pub fn default_system() -> Result<Self, PanelError> {
        Ok(Self {
            sites_available: PathBuf::from("/etc/nginx/sites-available"),
            sites_enabled: PathBuf::from("/etc/nginx/sites-enabled"),
            acme_webroot: SafePath::under_root(Path::new("/www"), "acme")?,
            letsencrypt_live: SafePath::under_root(Path::new("/etc/letsencrypt"), "live")?,
            service_name: ServiceName::parse("nginx")?,
        })
    }

    pub fn for_dirs(
        sites_available: PathBuf,
        sites_enabled: PathBuf,
        acme_webroot: SafePath,
        letsencrypt_live: SafePath,
    ) -> Result<Self, PanelError> {
        Ok(Self {
            sites_available,
            sites_enabled,
            acme_webroot,
            letsencrypt_live,
            service_name: ServiceName::parse("nginx")?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct NginxEngine {
    paths: NginxPaths,
    executor: Arc<dyn CommandExecutor>,
    http3_supported: Arc<AtomicBool>,
}

impl NginxEngine {
    #[must_use]
    pub fn new(paths: NginxPaths, executor: Arc<dyn CommandExecutor>) -> Self {
        Self {
            paths,
            executor,
            http3_supported: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Probes `nginx -V` to detect `--with-http_v3_module` and updates cached capability flag.
    pub async fn detect_http3_support(&self) -> bool {
        if !self.is_installed() {
            self.http3_supported.store(false, Ordering::Relaxed);
            return false;
        }
        let supported = match self
            .executor
            .run(AllowedBinary::Nginx, &["-V"], Duration::from_secs(5))
            .await
        {
            Ok(out) => {
                out.stderr.contains("http_v3_module") || out.stdout.contains("http_v3_module")
            }
            Err(_) => false,
        };
        self.http3_supported.store(supported, Ordering::Relaxed);
        supported
    }

    #[must_use]
    pub fn paths(&self) -> &NginxPaths {
        &self.paths
    }

    fn vhost_available_path(&self, domain: &Domain) -> PathBuf {
        self.paths
            .sites_available
            .join(format!("{}.conf", domain.as_str()))
    }

    fn vhost_enabled_path(&self, domain: &Domain) -> PathBuf {
        self.paths
            .sites_enabled
            .join(format!("{}.conf", domain.as_str()))
    }

    fn htpasswd_path(&self, domain: &Domain) -> PathBuf {
        self.paths
            .sites_available
            .parent()
            .unwrap_or_else(|| Path::new("/etc/nginx"))
            .join("htpasswd")
            .join(format!("{}.htpasswd", domain.as_str()))
    }

    fn ensure_installed(&self) -> Result<(), PanelError> {
        if !self.is_installed() {
            return Err(PanelError::Conflict(
                "Nginx is not installed on this server (/usr/sbin/nginx missing)".to_string(),
            ));
        }
        Ok(())
    }

    /// Writes site config atomically via tmp+rename + symlink without validating yet,
    /// returning snapshots so callers (`write_site` / `switch`) can rollback if validation fails.
    pub fn stage_site_unvalidated(
        &self,
        spec: &SiteSpec,
    ) -> Result<(FileSnapshot, SymlinkSnapshot), PanelError> {
        let rendered = self.render_site(spec)?;
        if spec.directory_auth.iter().any(|d| d.enabled) {
            let htpasswd_file = self.htpasswd_path(&spec.domain);
            write_site_htpasswd_file(&htpasswd_file, &spec.directory_auth)?;
        }
        let avail_path = self.vhost_available_path(&spec.domain);
        let enabled_path = self.vhost_enabled_path(&spec.domain);

        let file_snap = FileSnapshot::capture(&avail_path)?;
        let link_snap = SymlinkSnapshot::capture(&enabled_path)?;

        if let Err(err) = atomic_write_file(&avail_path, rendered.content.as_bytes(), 0o644) {
            let _ = link_snap.restore();
            let _ = file_snap.restore();
            return Err(err);
        }

        if let Err(err) = atomic_symlink(&avail_path, &enabled_path) {
            let _ = link_snap.restore();
            let _ = file_snap.restore();
            return Err(err);
        }

        Ok((file_snap, link_snap))
    }

    pub fn start(&self) -> BoxFuture<'_, Result<(), PanelError>> {
        Box::pin(async move {
            self.ensure_installed()?;
            let unit = self.paths.service_name.as_str();
            let out = self
                .executor
                .run(AllowedBinary::Systemctl, &["start", unit], CMD_TIMEOUT)
                .await?;
            if !out.success {
                return Err(PanelError::CommandFailed {
                    binary: AllowedBinary::Systemctl.label(),
                    exit_code: out.exit_code,
                    stderr: out.stderr,
                });
            }
            Ok(())
        })
    }

    pub fn stop(&self) -> BoxFuture<'_, Result<(), PanelError>> {
        Box::pin(async move {
            if !self.is_installed() {
                return Ok(());
            }
            let unit = self.paths.service_name.as_str();
            let out = self
                .executor
                .run(AllowedBinary::Systemctl, &["stop", unit], CMD_TIMEOUT)
                .await?;
            if !out.success {
                return Err(PanelError::CommandFailed {
                    binary: AllowedBinary::Systemctl.label(),
                    exit_code: out.exit_code,
                    stderr: out.stderr,
                });
            }
            Ok(())
        })
    }

    pub fn enable(&self) -> BoxFuture<'_, Result<(), PanelError>> {
        Box::pin(async move {
            if !self.is_installed() {
                return Ok(());
            }
            let unit = self.paths.service_name.as_str();
            let out = self
                .executor
                .run(AllowedBinary::Systemctl, &["enable", unit], CMD_TIMEOUT)
                .await?;
            if !out.success {
                return Err(PanelError::CommandFailed {
                    binary: AllowedBinary::Systemctl.label(),
                    exit_code: out.exit_code,
                    stderr: out.stderr,
                });
            }
            Ok(())
        })
    }

    pub fn disable(&self) -> BoxFuture<'_, Result<(), PanelError>> {
        Box::pin(async move {
            if !self.is_installed() {
                return Ok(());
            }
            let unit = self.paths.service_name.as_str();
            let out = self
                .executor
                .run(AllowedBinary::Systemctl, &["disable", unit], CMD_TIMEOUT)
                .await?;
            if !out.success {
                return Err(PanelError::CommandFailed {
                    binary: AllowedBinary::Systemctl.label(),
                    exit_code: out.exit_code,
                    stderr: out.stderr,
                });
            }
            Ok(())
        })
    }
}

impl WebServerEngine for NginxEngine {
    fn name(&self) -> EngineKind {
        EngineKind::Nginx
    }

    fn is_installed(&self) -> bool {
        self.executor.is_binary_available(AllowedBinary::Nginx)
    }

    fn render_site(&self, spec: &SiteSpec) -> Result<RenderedSite, PanelError> {
        spec.validate_supported()?;
        if spec.http3_quic && !self.http3_supported.load(Ordering::Relaxed) {
            return Err(PanelError::Conflict(
                "HTTP/3 QUIC is not supported by the installed Nginx binary (missing --with-http_v3_module, e.g. Ubuntu 24.04 stock nginx 1.24). Disable http3Quic or build Nginx with http_v3_module."
                    .to_string(),
            ));
        }

        let all_domains = spec.all_domains();
        let server_names = all_domains
            .iter()
            .map(|d| d.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let doc_root = spec.root.as_path().display();
        let acme_root = self.paths.acme_webroot.as_path().display();
        let acme_str = if acme_root.to_string().is_empty() {
            DEFAULT_ACME_WEBROOT.to_string()
        } else {
            acme_root.to_string()
        };
        let live_root = self.paths.letsencrypt_live.as_path().display();
        let live_str = if live_root.to_string().is_empty() {
            DEFAULT_LETSENCRYPT_LIVE.to_string()
        } else {
            live_root.to_string()
        };

        let zone_slug: String = spec
            .domain
            .as_str()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let (rate_zone_header, rate_limit_directive) = if spec.rate_limit_rps > 0 {
            let burst = spec.rate_limit_rps.saturating_mul(2).max(5);
            (
                format!(
                    "limit_req_zone $binary_remote_addr zone=fp_{zone_slug}:10m rate={}r/s;\n\n",
                    spec.rate_limit_rps
                ),
                format!(
                    "    limit_req zone=fp_{zone_slug} burst={burst} nodelay;\n    limit_req_status 429;\n"
                ),
            )
        } else {
            (String::new(), String::new())
        };

        let mut extra_locations = String::new();
        for rule in spec.redirects.iter().filter(|r| r.enabled) {
            let src = rule.source_path.trim();
            let dst = rule.target_url.trim();
            let matcher = if src.ends_with('/') { "^~" } else { "=" };
            let suffix = if rule.keep_query { "$is_args$args" } else { "" };
            extra_locations.push_str(&format!(
                "    location {matcher} {src} {{\n        return {} {dst}{suffix};\n    }}\n\n",
                rule.status_code
            ));
        }

        if let Some(ref hl) = spec.hotlink_protection {
            if hl.enabled {
                let exts: Vec<String> = hl
                    .protected_extensions
                    .split(|c: char| c == ',' || c == '|' || c.is_ascii_whitespace())
                    .map(|s| s.trim().trim_start_matches('.').to_ascii_lowercase())
                    .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric()))
                    .collect();
                if !exts.is_empty() {
                    let mut referers = String::new();
                    if !hl.block_empty_referer {
                        referers.push_str("none ");
                    }
                    referers.push_str("blocked server_names");
                    for dom in &hl.allowed_domains {
                        let clean = dom.trim();
                        if !clean.is_empty()
                            && clean.bytes().all(|b| {
                                b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'*')
                            })
                        {
                            referers.push(' ');
                            referers.push_str(clean);
                        }
                    }
                    extra_locations.push_str(&format!(
                        "    location ~* \\.({})$ {{\n        valid_referers {referers};\n        if ($invalid_referer) {{\n            return 403;\n        }}\n        try_files $uri =404;\n    }}\n\n",
                        exts.join("|")
                    ));
                }
            }
        }

        let htpasswd_file = self.htpasswd_path(&spec.domain);
        for auth in spec.directory_auth.iter().filter(|d| d.enabled) {
            let clean_path = auth.path.trim();
            let realm = if auth.label.trim().is_empty() {
                "Restricted Area".to_string()
            } else {
                auth.label.trim().replace('"', "'")
            };
            extra_locations.push_str(&format!(
                "    location ^~ {clean_path} {{\n        auth_basic \"{realm}\";\n        auth_basic_user_file {};\n        try_files $uri $uri/ =404;\n    }}\n\n",
                htpasswd_file.display()
            ));
        }

        let route_block = render_nginx_route_block(
            &spec.mode,
            &doc_root.to_string(),
            spec.websocket_enabled,
        );
        let acme_block = format!(
            "    location ^~ /.well-known/acme-challenge/ {{\n        root {acme_str};\n        default_type \"text/plain\";\n        try_files $uri =404;\n    }}"
        );
        let pma_block = concat!(
            "    location = /phpadmin {\n",
            "        return 301 $scheme://$http_host/phpadmin/;\n",
            "    }\n\n",
            "    location ^~ /phpadmin/ {\n",
            "        proxy_pass http://127.0.0.1:888;\n",
            "        proxy_set_header Host $http_host;\n",
            "        proxy_set_header X-Real-IP $remote_addr;\n",
            "        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n",
            "        proxy_set_header X-Forwarded-Proto $scheme;\n",
            "    }\n\n",
            "    location = /phpmyadmin {\n",
            "        return 301 $scheme://$http_host/phpmyadmin/;\n",
            "    }\n\n",
            "    location ^~ /phpmyadmin/ {\n",
            "        proxy_pass http://127.0.0.1:888;\n",
            "        proxy_set_header Host $http_host;\n",
            "        proxy_set_header X-Real-IP $remote_addr;\n",
            "        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n",
            "        proxy_set_header X-Forwarded-Proto $scheme;\n",
            "    }"
        );

        let quic_directives = if spec.http3_quic {
            "    listen 443 quic;\n    listen [::]:443 quic;\n    add_header Alt-Svc 'h3=\":443\"; ma=86400' always;\n"
        } else {
            ""
        };

        let content = if spec.ssl_enabled {
            let cert_dir = format!("{live_str}/{}", spec.domain.as_str());
            let http_body = if spec.redirect_https {
                "    location / {\n        return 301 https://$host$request_uri;\n    }".to_string()
            } else {
                format!("{pma_block}\n\n{rate_limit_directive}{extra_locations}{route_block}")
            };

            format!(
                "# Managed by FerrousPanel (NginxEngine) — {domain}\n{rate_zone_header}server {{\n    listen 80;\n    listen [::]:80;\n    server_name {server_names};\n\n{acme_block}\n\n{http_body}\n}}\n\nserver {{\n    listen 443 ssl http2;\n    listen [::]:443 ssl http2;\n{quic_directives}    server_name {server_names};\n\n    ssl_certificate {cert_dir}/fullchain.pem;\n    ssl_certificate_key {cert_dir}/privkey.pem;\n    ssl_protocols TLSv1.2 TLSv1.3;\n    ssl_prefer_server_ciphers on;\n\n    root {doc_root};\n    server_tokens off;\n\n{pma_block}\n\n{rate_limit_directive}{extra_locations}{route_block}\n}}\n",
                domain = spec.domain.as_str(),
            )
        } else {
            format!(
                "# Managed by FerrousPanel (NginxEngine) — {domain}\n{rate_zone_header}server {{\n    listen 80;\n    listen [::]:80;\n    server_name {server_names};\n\n    root {doc_root};\n    server_tokens off;\n\n{acme_block}\n\n{pma_block}\n\n{rate_limit_directive}{extra_locations}{route_block}\n}}\n",
                domain = spec.domain.as_str(),
            )
        };

        Ok(RenderedSite {
            engine: EngineKind::Nginx,
            domain: spec.domain.clone(),
            config_path: self.vhost_available_path(&spec.domain),
            content,
        })
    }

    fn write_site<'a>(&'a self, spec: &'a SiteSpec) -> BoxFuture<'a, Result<(), PanelError>> {
        Box::pin(async move {
            self.ensure_installed()?;
            if spec.http3_quic && !self.detect_http3_support().await {
                return Err(PanelError::Conflict(
                    "HTTP/3 QUIC is not supported by the installed Nginx binary (missing --with-http_v3_module). Disable http3Quic."
                        .to_string(),
                ));
            }
            if matches!(spec.mode, SiteMode::PhpFpm { .. }) {
                if let Some(nginx_root) = self.paths.sites_available.parent() {
                    let snippet_path = nginx_root.join("snippets/fastcgi-php.conf");
                    if !snippet_path.is_file() && nginx_root == Path::new("/etc/nginx") {
                        let snippet_body = "# Managed by FerrousPanel — fallback FastCGI PHP snippet\n\
fastcgi_split_path_info ^(.+?\\.php)(/.*)$;\n\
try_files $fastcgi_script_name =404;\n\
set $path_info $fastcgi_path_info;\n\
fastcgi_param PATH_INFO $path_info;\n\
fastcgi_index index.php;\n\
include fastcgi.conf;\n";
                        let _ = atomic_write_file(&snippet_path, snippet_body.as_bytes(), 0o644);
                    }
                }
            }
            let (file_snap, link_snap) = self.stage_site_unvalidated(spec)?;

            let validation = match self.validate().await {
                Ok(rep) => rep,
                Err(err) => {
                    let _ = link_snap.restore();
                    let _ = file_snap.restore();
                    return Err(err);
                }
            };

            if !validation.valid {
                let _ = link_snap.restore();
                let _ = file_snap.restore();
                return Err(PanelError::Validation(format!(
                    "nginx -t failed; previous vhost config restored: {}",
                    validation.output
                )));
            }

            Ok(())
        })
    }

    fn remove_site<'a>(&'a self, domain: &'a Domain) -> BoxFuture<'a, Result<(), PanelError>> {
        Box::pin(async move {
            self.ensure_installed()?;
            let avail_path = self.vhost_available_path(domain);
            let enabled_path = self.vhost_enabled_path(domain);

            let file_snap = FileSnapshot::capture(&avail_path)?;
            let link_snap = SymlinkSnapshot::capture(&enabled_path)?;

            if std::fs::symlink_metadata(&enabled_path).is_ok() {
                std::fs::remove_file(&enabled_path)?;
            }
            if std::fs::symlink_metadata(&avail_path).is_ok() {
                std::fs::remove_file(&avail_path)?;
            }

            let validation = match self.validate().await {
                Ok(rep) => rep,
                Err(err) => {
                    let _ = file_snap.restore();
                    let _ = link_snap.restore();
                    return Err(err);
                }
            };

            if !validation.valid {
                let _ = file_snap.restore();
                let _ = link_snap.restore();
                return Err(PanelError::Validation(format!(
                    "nginx -t failed after site removal; restored: {}",
                    validation.output
                )));
            }

            Ok(())
        })
    }

    fn validate(&self) -> BoxFuture<'_, Result<ValidationReport, PanelError>> {
        Box::pin(async move {
            self.ensure_installed()?;
            let out = self
                .executor
                .run(AllowedBinary::Nginx, &["-t"], CMD_TIMEOUT)
                .await?;
            let combined = match (out.stdout.is_empty(), out.stderr.is_empty()) {
                (false, false) => format!("{}\n{}", out.stdout, out.stderr),
                (false, true) => out.stdout,
                (true, false) => out.stderr,
                (true, true) => {
                    if out.success {
                        "nginx: syntax is ok".to_string()
                    } else {
                        "nginx -t failed without output".to_string()
                    }
                }
            };
            Ok(ValidationReport {
                engine: EngineKind::Nginx,
                valid: out.success,
                output: combined,
            })
        })
    }

    fn reload(&self) -> BoxFuture<'_, Result<(), PanelError>> {
        Box::pin(async move {
            self.ensure_installed()?;
            let unit = self.paths.service_name.as_str();
            let out = self
                .executor
                .run(AllowedBinary::Systemctl, &["reload", unit], CMD_TIMEOUT)
                .await?;
            if !out.success {
                return Err(PanelError::CommandFailed {
                    binary: AllowedBinary::Systemctl.label(),
                    exit_code: out.exit_code,
                    stderr: out.stderr,
                });
            }
            Ok(())
        })
    }

    fn status(&self) -> BoxFuture<'_, Result<EngineStatus, PanelError>> {
        Box::pin(async move {
            let installed = self.is_installed();
            if !installed {
                return Ok(EngineStatus {
                    engine: EngineKind::Nginx,
                    installed: false,
                    active: false,
                    pid: None,
                    sub_state: "not-installed".to_string(),
                });
            }
            let unit = self.paths.service_name.as_str();
            let out = self
                .executor
                .run(
                    AllowedBinary::Systemctl,
                    &[
                        "show",
                        unit,
                        "--property=ActiveState,SubState,MainPID",
                        "--no-pager",
                    ],
                    CMD_TIMEOUT,
                )
                .await?;
            Ok(parse_systemctl_status(
                EngineKind::Nginx,
                true,
                &out.stdout,
            ))
        })
    }
}

fn render_nginx_route_block(mode: &SiteMode, doc_root: &str, websocket_enabled: bool) -> String {
    match mode {
        SiteMode::StaticSpa {
            index_file,
            spa_fallback,
        } => {
            let idx = index_file.as_str();
            let try_rule = if *spa_fallback {
                format!("try_files $uri $uri.html $uri/ /{idx};")
            } else {
                "try_files $uri $uri.html $uri/ =404;".to_string()
            };
            format!(
                "    index {idx};\n    location ~* \\.(?:mjs|js|css|wasm|woff2|woff|svg|webp|avif)$ {{\n        expires 30d;\n        add_header Cache-Control \"public, max-age=2592000\";\n        add_header X-Content-Type-Options \"nosniff\" always;\n        try_files $uri =404;\n    }}\n    location / {{\n        {try_rule}\n    }}"
            )
        }
        SiteMode::ReverseProxy { upstream } => {
            let url = upstream.http_url();
            let ws_block = if websocket_enabled {
                "\n        proxy_set_header Upgrade $http_upgrade;\n        proxy_set_header Connection \"upgrade\";\n        proxy_read_timeout 3600s;"
            } else {
                ""
            };
            format!(
                "    location / {{\n        proxy_pass {url};\n        proxy_http_version 1.1;\n        proxy_set_header Host $host;\n        proxy_set_header X-Real-IP $remote_addr;\n        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n        proxy_set_header X-Forwarded-Proto $scheme;{ws_block}\n    }}"
            )
        }
        SiteMode::PhpFpm {
            php_version,
            index_file,
        } => {
            let idx = index_file.as_str();
            let sock = php_version.socket_path();
            format!(
                "    index {idx} index.html;\n    location / {{\n        try_files $uri $uri/ /{idx}?$query_string;\n    }}\n    location ~ \\.php$ {{\n        include snippets/fastcgi-php.conf;\n        fastcgi_param SCRIPT_FILENAME {doc_root}$fastcgi_script_name;\n        fastcgi_pass unix:{sock};\n    }}\n    location ~ /\\.ht {{\n        deny all;\n    }}"
            )
        }
        SiteMode::Unsupported => String::new(),
    }
}
