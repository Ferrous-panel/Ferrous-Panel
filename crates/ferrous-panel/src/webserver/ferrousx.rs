use crate::error::PanelError;
use crate::system::cmd::AllowedBinary;
use crate::system::paths::SafePath;
use crate::system::validate::{Domain, ServiceName};
use crate::webserver::ferrousx_ipc::{
    FerrousxIpcClient, DEFAULT_FERROUSX_ADMIN_TOKEN_FILE, DEFAULT_FERROUSX_CONTROL_SOCKET,
};
use crate::webserver::{
    atomic_write_file, parse_systemctl_status, BoxFuture, CommandExecutor, EngineKind,
    EngineStatus, FileSnapshot, RenderedSite, SiteMode, SiteSpec, ValidationReport,
    WebServerEngine, DEFAULT_ACME_WEBROOT, DEFAULT_LETSENCRYPT_LIVE,
};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const CMD_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub struct FerrousxPaths {
    pub master_config: PathBuf,
    pub sites_dir: PathBuf,
    pub acme_webroot: SafePath,
    pub letsencrypt_live: SafePath,
    pub service_name: ServiceName,
    pub control_socket: PathBuf,
    pub admin_token_file: PathBuf,
    use_system_ipc_env: bool,
}

impl FerrousxPaths {
    pub fn default_system() -> Result<Self, PanelError> {
        Ok(Self {
            master_config: PathBuf::from("/etc/ferrousx/ferrousx.toml"),
            sites_dir: PathBuf::from("/etc/ferrousx/sites"),
            acme_webroot: SafePath::under_root(Path::new("/www"), "acme")?,
            letsencrypt_live: SafePath::under_root(Path::new("/etc/letsencrypt"), "live")?,
            service_name: ServiceName::parse("ferrousx")?,
            control_socket: PathBuf::from(DEFAULT_FERROUSX_CONTROL_SOCKET),
            admin_token_file: PathBuf::from(DEFAULT_FERROUSX_ADMIN_TOKEN_FILE),
            use_system_ipc_env: true,
        })
    }

    pub fn for_dirs(
        master_config: PathBuf,
        sites_dir: PathBuf,
        acme_webroot: SafePath,
        letsencrypt_live: SafePath,
    ) -> Result<Self, PanelError> {
        let base_dir = master_config
            .parent()
            .unwrap_or_else(|| Path::new("/tmp"))
            .to_path_buf();
        Ok(Self {
            master_config,
            sites_dir,
            acme_webroot,
            letsencrypt_live,
            service_name: ServiceName::parse("ferrousx")?,
            control_socket: base_dir.join("panel-admin.sock"),
            admin_token_file: base_dir.join("admin.token"),
            use_system_ipc_env: false,
        })
    }

    #[must_use]
    pub fn with_ipc_paths(mut self, control_socket: PathBuf, admin_token_file: PathBuf) -> Self {
        self.control_socket = control_socket;
        self.admin_token_file = admin_token_file;
        self.use_system_ipc_env = false;
        self
    }
}

#[derive(Debug, Clone)]
pub struct FerrousxEngine {
    paths: FerrousxPaths,
    executor: Arc<dyn CommandExecutor>,
}

impl FerrousxEngine {
    #[must_use]
    pub fn new(paths: FerrousxPaths, executor: Arc<dyn CommandExecutor>) -> Self {
        Self { paths, executor }
    }

    #[must_use]
    pub fn paths(&self) -> &FerrousxPaths {
        &self.paths
    }

    #[must_use]
    pub fn ipc_client(&self) -> FerrousxIpcClient {
        if self.paths.use_system_ipc_env {
            FerrousxIpcClient::system_default()
        } else {
            FerrousxIpcClient::for_paths(
                self.paths.control_socket.clone(),
                self.paths.admin_token_file.clone(),
            )
        }
    }

    async fn reload_via_sighup(&self) -> Result<(), PanelError> {
        let unit = self.paths.service_name.as_str();
        let pid_out = self
            .executor
            .run(
                AllowedBinary::Systemctl,
                &["show", "--property=MainPID", "--value", unit],
                CMD_TIMEOUT,
            )
            .await?;
        if !pid_out.success {
            return Err(PanelError::CommandFailed {
                binary: AllowedBinary::Systemctl.label(),
                exit_code: pid_out.exit_code,
                stderr: pid_out.stderr,
            });
        }

        let pid: u32 = pid_out.stdout.trim().parse().map_err(|_| {
            PanelError::Conflict(format!(
                "invalid MainPID returned by systemctl show for {unit}: '{}'",
                pid_out.stdout.trim()
            ))
        })?;

        if pid == 0 {
            return Err(PanelError::Conflict(format!(
                "{unit} service is not running (MainPID=0); cannot send SIGHUP"
            )));
        }

        let pid_str = pid.to_string();
        let kill_out = self
            .executor
            .run(AllowedBinary::Kill, &["-HUP", &pid_str], CMD_TIMEOUT)
            .await?;
        if !kill_out.success {
            return Err(PanelError::CommandFailed {
                binary: AllowedBinary::Kill.label(),
                exit_code: kill_out.exit_code,
                stderr: kill_out.stderr,
            });
        }
        Ok(())
    }

    /// Sends an authenticated UDS IPC reload action (`reload` / `panel_sync` / `vhost_add` / `vhost_del`)
    /// over `/run/ferrousx/panel-admin.sock` when available, falling back cleanly to `SIGHUP`.
    pub async fn notify_ipc_reload(
        &self,
        action: &str,
        payload: serde_json::Value,
    ) -> Result<(), PanelError> {
        self.ensure_installed()?;
        let client = self.ipc_client();
        if client.socket_available() {
            let resp = client.send_command(action, payload).await?;
            if resp.success {
                tracing::info!(
                    action = %action,
                    latency_us = resp.latency_us,
                    "ferrousx UDS IPC action completed"
                );
                return Ok(());
            }
            return Err(PanelError::Validation(format!(
                "ferrousx IPC '{action}' failed: {}",
                resp.message
            )));
        }
        self.reload_via_sighup().await
    }

    pub async fn notify_panel_sync(&self) -> Result<(), PanelError> {
        self.notify_ipc_reload("panel_sync", serde_json::json!({ "source": "ferrous-panel" }))
            .await
    }

    pub async fn notify_vhost_add(&self, domain: &str) -> Result<(), PanelError> {
        self.notify_ipc_reload("vhost_add", serde_json::json!({ "domain": domain }))
            .await
    }

    pub async fn notify_vhost_del(&self, domain: &str) -> Result<(), PanelError> {
        self.notify_ipc_reload("vhost_del", serde_json::json!({ "domain": domain }))
            .await
    }

    fn site_config_path(&self, domain: &Domain) -> PathBuf {
        self.paths.sites_dir.join(format!("{}.toml", domain.as_str()))
    }

    fn ensure_installed(&self) -> Result<(), PanelError> {
        if !self.is_installed() {
            return Err(PanelError::Conflict(
                "FerrousX is not installed on this server (/usr/local/bin/ferrousx-bin missing)"
                    .to_string(),
            ));
        }
        Ok(())
    }

    /// Renders deterministic global `/etc/ferrousx/ferrousx.toml`.
    #[must_use]
    pub fn render_master_config(&self, worker_threads: u32) -> String {
        let sites_dir = self.paths.sites_dir.display();
        let acme = self.paths.acme_webroot.as_path().display();
        format!(
            "# Managed by FerrousPanel — FerrousX master configuration\n\
worker_threads = {worker_threads}\n\
bind_http = \"0.0.0.0:80\"\n\
bind_https = \"0.0.0.0:443\"\n\
sites_dir = \"{sites_dir}\"\n\
acme_webroot = \"{acme}\"\n\
acme_challenge_prefix = \"/.well-known/acme-challenge/\"\n\
server_tokens = false\n"
        )
    }

    /// Writes `/etc/ferrousx/ferrousx.toml` atomically if missing or updating worker count.
    pub fn ensure_master_config(&self, worker_threads: u32) -> Result<(), PanelError> {
        self.ensure_installed()?;
        std::fs::create_dir_all(&self.paths.sites_dir)?;
        let content = self.render_master_config(worker_threads);
        atomic_write_file(&self.paths.master_config, content.as_bytes(), 0o644)
    }

    /// Atomically stages `sites/<domain>.toml` via tmp+rename without running `--check-config` yet.
    pub fn stage_site_unvalidated(&self, spec: &SiteSpec) -> Result<FileSnapshot, PanelError> {
        let rendered = self.render_site(spec)?;
        let site_path = self.site_config_path(&spec.domain);
        let snap = FileSnapshot::capture(&site_path)?;
        if let Err(err) = atomic_write_file(&site_path, rendered.content.as_bytes(), 0o644) {
            let _ = snap.restore();
            return Err(err);
        }
        Ok(snap)
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
}

impl WebServerEngine for FerrousxEngine {
    fn name(&self) -> EngineKind {
        EngineKind::Ferrousx
    }

    fn is_installed(&self) -> bool {
        self.executor.is_binary_available(AllowedBinary::Ferrousx)
    }

    fn render_site(&self, spec: &SiteSpec) -> Result<RenderedSite, PanelError> {
        spec.validate_supported()?;

        let all_domains = spec.all_domains();
        let mut domains_toml = String::from("[");
        for (idx, d) in all_domains.iter().enumerate() {
            if idx > 0 {
                domains_toml.push_str(", ");
            }
            let _ = write!(domains_toml, "\"{}\"", d.as_str());
        }
        domains_toml.push(']');

        let doc_root = spec.root.as_path().display();
        let acme_raw = self.paths.acme_webroot.as_path().display().to_string();
        let acme_root = if acme_raw.is_empty() {
            DEFAULT_ACME_WEBROOT.to_string()
        } else {
            acme_raw
        };
        let live_raw = self.paths.letsencrypt_live.as_path().display().to_string();
        let live_root = if live_raw.is_empty() {
            DEFAULT_LETSENCRYPT_LIVE.to_string()
        } else {
            live_raw
        };

        let cert_path = format!("{live_root}/{}/fullchain.pem", spec.domain.as_str());
        let key_path = format!("{live_root}/{}/privkey.pem", spec.domain.as_str());

        let mode_section = match &spec.mode {
            SiteMode::StaticSpa {
                index_file,
                spa_fallback,
            } => format!(
                "[handler]\n\
kind = \"static_spa\"\n\
index_file = \"{}\"\n\
spa_fallback = {spa_fallback}\n",
                index_file.as_str()
            ),
            SiteMode::ReverseProxy { upstream } => format!(
                "[handler]\n\
kind = \"reverse_proxy\"\n\
upstream = \"{}\"\n\
pass_host_header = true\n\
forwarded_headers = true\n",
                upstream.http_url()
            ),
            SiteMode::PhpFpm {
                php_version,
                index_file,
            } => format!(
                "[handler]\n\
kind = \"php_fpm\"\n\
php_version = \"{}\"\n\
fastcgi_socket = \"{}\"\n\
index_file = \"{}\"\n",
                php_version.version_str(),
                php_version.socket_path(),
                index_file.as_str()
            ),
            SiteMode::Unsupported => String::new(),
        };

        let content = format!(
            "# Managed by FerrousPanel (FerrousxEngine) — {domain}\n\
domain = \"{domain}\"\n\
server_names = {domains_toml}\n\
root = \"{doc_root}\"\n\
\n\
[acme]\n\
prefix = \"/.well-known/acme-challenge/\"\n\
webroot = \"{acme_root}\"\n\
serve_before_redirect = true\n\
\n\
[ssl]\n\
enabled = {ssl_enabled}\n\
redirect_https = {redirect_https}\n\
cert_path = \"{cert_path}\"\n\
key_path = \"{key_path}\"\n\
\n\
{mode_section}",
            domain = spec.domain.as_str(),
            ssl_enabled = spec.ssl_enabled,
            redirect_https = spec.redirect_https,
        );

        Ok(RenderedSite {
            engine: EngineKind::Ferrousx,
            domain: spec.domain.clone(),
            config_path: self.site_config_path(&spec.domain),
            content,
        })
    }

    fn write_site<'a>(&'a self, spec: &'a SiteSpec) -> BoxFuture<'a, Result<(), PanelError>> {
        Box::pin(async move {
            self.ensure_installed()?;
            let master_snap = FileSnapshot::capture(&self.paths.master_config)?;
            if !self.paths.master_config.exists() {
                if let Err(err) = self.ensure_master_config(0) {
                    let _ = master_snap.restore();
                    return Err(err);
                }
            }

            let site_snap = match self.stage_site_unvalidated(spec) {
                Ok(snap) => snap,
                Err(err) => {
                    let _ = master_snap.restore();
                    return Err(err);
                }
            };

            let validation = match self.validate().await {
                Ok(rep) => rep,
                Err(err) => {
                    let _ = site_snap.restore();
                    let _ = master_snap.restore();
                    return Err(err);
                }
            };

            if !validation.valid {
                let _ = site_snap.restore();
                let _ = master_snap.restore();
                return Err(PanelError::Validation(format!(
                    "ferrousx-bin --check-config failed; previous config restored: {}",
                    validation.output
                )));
            }

            Ok(())
        })
    }

    fn remove_site<'a>(&'a self, domain: &'a Domain) -> BoxFuture<'a, Result<(), PanelError>> {
        Box::pin(async move {
            self.ensure_installed()?;
            let site_path = self.site_config_path(domain);
            let snap = FileSnapshot::capture(&site_path)?;

            if std::fs::symlink_metadata(&site_path).is_ok() {
                std::fs::remove_file(&site_path)?;
            }

            let validation = match self.validate().await {
                Ok(rep) => rep,
                Err(err) => {
                    let _ = snap.restore();
                    return Err(err);
                }
            };

            if !validation.valid {
                let _ = snap.restore();
                return Err(PanelError::Validation(format!(
                    "ferrousx-bin --check-config failed after removal; restored: {}",
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
                .run(AllowedBinary::Ferrousx, &["--check-config"], CMD_TIMEOUT)
                .await?;
            let combined = match (out.stdout.is_empty(), out.stderr.is_empty()) {
                (false, false) => format!("{}\n{}", out.stdout, out.stderr),
                (false, true) => out.stdout,
                (true, false) => out.stderr,
                (true, true) => {
                    if out.success {
                        "ferrousx configuration check ok".to_string()
                    } else {
                        "ferrousx-bin --check-config failed without output".to_string()
                    }
                }
            };
            Ok(ValidationReport {
                engine: EngineKind::Ferrousx,
                valid: out.success,
                output: combined,
            })
        })
    }

    fn reload(&self) -> BoxFuture<'_, Result<(), PanelError>> {
        Box::pin(async move {
            self.notify_ipc_reload("reload", serde_json::json!({})).await
        })
    }

    fn status(&self) -> BoxFuture<'_, Result<EngineStatus, PanelError>> {
        Box::pin(async move {
            let installed = self.is_installed();
            if !installed {
                return Ok(EngineStatus {
                    engine: EngineKind::Ferrousx,
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
                EngineKind::Ferrousx,
                true,
                &out.stdout,
            ))
        })
    }
}
