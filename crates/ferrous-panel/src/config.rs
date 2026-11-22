use crate::cli::PanelCommand;
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

/// Explicit runtime mode replacing fragile path-based production detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RuntimeMode {
    Production,
    Test,
}

impl RuntimeMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Test => "test",
        }
    }
}

/// Runtime configuration parsed from CLI flags and environment variables.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "ferrous-panel",
    version,
    about = "FerrousPanel pure-Rust VPS control plane server"
)]
pub struct Config {
    /// Explicit execution mode (`production` enforces real host binaries & verification; `test` is for isolated unit/integration tests).
    #[arg(long, env = "FERROUS_MODE", value_enum, default_value_t = RuntimeMode::Production)]
    pub mode: RuntimeMode,

    /// Listen host address (defaults strictly to loopback 127.0.0.1).
    #[arg(long, env = "FERROUS_BIND", default_value = "127.0.0.1")]
    pub bind: String,

    /// Listen TCP port.
    #[arg(long, env = "FERROUS_PORT", default_value_t = 8443)]
    pub port: u16,

    /// Internal loopback TCP port for built-in Pure-Rust FerrousAdmin (/phpadmin/, 0 to disable).
    #[arg(long, env = "FERROUS_PMA_PORT", default_value_t = 888)]
    pub pma_port: u16,

    /// Persistent state directory (SQLite DB and 0600 credentials file).
    #[arg(long, env = "FERROUS_DATA_DIR", default_value = "/var/lib/ferrous-panel")]
    pub data_dir: PathBuf,

    /// Mandatory website root containment directory for SafePath.
    #[arg(long, env = "FERROUS_WWWROOT", default_value = "/www/wwwroot")]
    pub wwwroot: PathBuf,

    /// Administrator account username.
    #[arg(long, env = "FERROUS_ADMIN_USER", default_value = "admin_ferrous")]
    pub admin_username: String,

    /// Set the Secure flag on session cookies (enable behind HTTPS termination).
    #[arg(long, env = "FERROUS_SECURE_COOKIE", default_value_t = false)]
    pub secure_cookie: bool,

    /// Panel URL entrance path reported to authenticated UI clients.
    #[arg(long, env = "FERROUS_SECURITY_ENTRANCE", default_value = "/panel")]
    pub security_entrance: String,

    /// Optional custom domain configured for this panel instance.
    #[arg(long, env = "FERROUS_CUSTOM_DOMAIN")]
    pub custom_domain: Option<String>,

    /// Reload active frontend web server engine (nginx or ferrousx) and exit (for certbot --deploy-hook).
    #[arg(long, default_value_t = false)]
    pub reload_webserver: bool,

    /// Real archive backup directory (.tar.zst).
    #[arg(long, env = "FERROUS_BACKUPS_DIR", default_value = "/var/backups/ferrous-panel")]
    pub backups_dir: PathBuf,

    /// Systemd unit output directory for managed application services.
    #[arg(long, env = "FERROUS_SYSTEMD_DIR", default_value = "/etc/systemd/system")]
    pub systemd_unit_dir: PathBuf,

    /// Comma-separated reverse proxy IPs trusted to supply X-Forwarded-For / CF-Connecting-IP headers.
    #[arg(long, env = "FERROUS_TRUSTED_PROXIES", default_value = "127.0.0.1,::1")]
    pub trusted_proxies: String,

    /// Optional CLI subcommand (`fp ...` or `serve`). Defaults to serving the panel daemon.
    #[command(subcommand)]
    pub command: Option<PanelCommand>,
}

impl Config {
    #[must_use]
    pub const fn is_production(&self) -> bool {
        matches!(self.mode, RuntimeMode::Production)
    }

    #[must_use]
    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("panel.db")
    }

    #[must_use]
    pub fn credentials_path(&self) -> PathBuf {
        self.data_dir.join("admin.credentials.json")
    }

    /// Returns true if `peer_ip` is explicitly allowlisted as a trusted reverse proxy.
    #[must_use]
    pub fn is_trusted_proxy(&self, peer_ip: &str) -> bool {
        let needle = peer_ip.trim();
        if needle.is_empty() {
            return false;
        }
        self.trusted_proxies
            .split(',')
            .map(str::trim)
            .any(|entry| !entry.is_empty() && entry == needle)
    }

    /// Convenience constructor for integration tests with isolated temp directories.
    #[must_use]
    pub fn for_test(data_dir: PathBuf, wwwroot: PathBuf) -> Self {
        let backups_dir = data_dir.join("backups");
        let systemd_unit_dir = data_dir.join("systemd");
        Self {
            mode: RuntimeMode::Test,
            bind: "127.0.0.1".to_string(),
            port: 8443,
            pma_port: 0,
            data_dir,
            wwwroot,
            admin_username: "admin_ferrous".to_string(),
            secure_cookie: false,
            security_entrance: "/panel".to_string(),
            custom_domain: None,
            reload_webserver: false,
            backups_dir,
            systemd_unit_dir,
            trusted_proxies: "127.0.0.1,::1".to_string(),
            command: None,
        }
    }
}
