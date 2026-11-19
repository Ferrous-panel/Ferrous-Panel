use crate::error::PanelError;
use crate::system::cmd::AllowedBinary;
use crate::system::paths::SafePath;
use crate::system::validate::{Domain, ServiceName};
use crate::webserver::{
    atomic_symlink, atomic_write_file, parse_systemctl_status, BoxFuture, CommandExecutor,
    EngineKind, EngineStatus, FileSnapshot, RenderedSite, SiteMode, SiteSpec, SymlinkSnapshot,
    ValidationReport, WebServerEngine, DEFAULT_ACME_WEBROOT, DEFAULT_LETSENCRYPT_LIVE,
};
use std::path::{Path, PathBuf};
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
}

impl NginxEngine {
    #[must_use]
    pub fn new(paths: NginxPaths, executor: Arc<dyn CommandExecutor>) -> Self {
        Self { paths, executor }
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
            let _ = self
                .executor
                .run(AllowedBinary::Systemctl, &["enable", unit], CMD_TIMEOUT)
                .await;
            Ok(())
        })
    }

    pub fn disable(&self) -> BoxFuture<'_, Result<(), PanelError>> {
        Box::pin(async move {
            if !self.is_installed() {
                return Ok(());
            }
            let unit = self.paths.service_name.as_str();
            let _ = self
                .executor
                .run(AllowedBinary::Systemctl, &["disable", unit], CMD_TIMEOUT)
                .await;
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

        let route_block = render_nginx_route_block(
            &spec.mode,
            &doc_root.to_string(),
            spec.websocket_enabled,
        );
        let acme_block = format!(
            "    location ^~ /.well-known/acme-challenge/ {{\n        root {acme_str};\n        default_type \"text/plain\";\n        try_files $uri =404;\n    }}"
        );

        let content = if spec.ssl_enabled {
            let cert_dir = format!("{live_str}/{}", spec.domain.as_str());
            let http_body = if spec.redirect_https {
                "    location / {\n        return 301 https://$host$request_uri;\n    }".to_string()
            } else {
                route_block.clone()
            };

            format!(
                "# Managed by FerrousPanel (NginxEngine) — {domain}\n\
server {{\n\
    listen 80;\n\
    listen [::]:80;\n\
    server_name {server_names};\n\
\n\
{acme_block}\n\
\n\
{http_body}\n\
}}\n\
\n\
server {{\n\
    listen 443 ssl http2;\n\
    listen [::]:443 ssl http2;\n\
    server_name {server_names};\n\
\n\
    ssl_certificate {cert_dir}/fullchain.pem;\n\
    ssl_certificate_key {cert_dir}/privkey.pem;\n\
    ssl_protocols TLSv1.2 TLSv1.3;\n\
    ssl_prefer_server_ciphers on;\n\
\n\
    root {doc_root};\n\
    server_tokens off;\n\
\n\
{route_block}\n\
}}\n",
                domain = spec.domain.as_str(),
            )
        } else {
            format!(
                "# Managed by FerrousPanel (NginxEngine) — {domain}\n\
server {{\n\
    listen 80;\n\
    listen [::]:80;\n\
    server_name {server_names};\n\
\n\
    root {doc_root};\n\
    server_tokens off;\n\
\n\
{acme_block}\n\
\n\
{route_block}\n\
}}\n",
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
                format!("try_files $uri $uri/ /{idx};")
            } else {
                "try_files $uri $uri/ =404;".to_string()
            };
            format!(
                "    index {idx};\n\
    location / {{\n\
        {try_rule}\n\
    }}"
            )
        }
        SiteMode::ReverseProxy { upstream } => {
            let url = upstream.http_url();
            let ws_block = if websocket_enabled {
                "\n        proxy_set_header Upgrade $http_upgrade;\n\
        proxy_set_header Connection \"upgrade\";\n\
        proxy_read_timeout 3600s;"
            } else {
                ""
            };
            format!(
                "    location / {{\n\
        proxy_pass {url};\n\
        proxy_http_version 1.1;\n\
        proxy_set_header Host $host;\n\
        proxy_set_header X-Real-IP $remote_addr;\n\
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n\
        proxy_set_header X-Forwarded-Proto $scheme;{ws_block}\n\
    }}"
            )
        }
        SiteMode::PhpFpm {
            php_version,
            index_file,
        } => {
            let idx = index_file.as_str();
            let sock = php_version.socket_path();
            format!(
                "    index {idx} index.html;\n\
    location / {{\n\
        try_files $uri $uri/ /{idx}?$query_string;\n\
    }}\n\
    location ~ \\.php$ {{\n\
        include snippets/fastcgi-php.conf;\n\
        fastcgi_param SCRIPT_FILENAME {doc_root}$fastcgi_script_name;\n\
        fastcgi_pass unix:{sock};\n\
    }}\n\
    location ~ /\\.ht {{\n\
        deny all;\n\
    }}"
            )
        }
        SiteMode::Unsupported => String::new(),
    }
}
