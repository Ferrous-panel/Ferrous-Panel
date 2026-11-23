use crate::auth::password::{
    generate_initial_password, load_or_init_credentials, update_admin_password,
    update_admin_username,
};
use crate::config::Config;
use crate::error::PanelError;
use crate::system::cmd::{self, AllowedBinary};
use crate::system::validate::{
    is_reserved_website_port, validate_panel_port, Domain, DEFAULT_PANEL_PORT,
};
use crate::webserver::{atomic_write_file, EngineKind, WebServerManager};
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

const CLI_CMD_TIMEOUT: Duration = Duration::from_secs(20);
const PANEL_SERVICE: &str = "ferrous-panel.service";
const DEFAULT_ENV_FILE: &str = "/etc/ferrous-panel/panel.env";

/// Top-level subcommands for the `ferrous-panel` binary.
#[derive(Debug, Clone, Subcommand)]
pub enum PanelCommand {
    /// Run the FerrousPanel HTTP daemon (default when no subcommand is provided)
    Serve,
    /// Terminal administration CLI (`fp` / `ferrous`)
    Fp(FpArgs),
}

/// CLI entrypoint when invoked directly as `fp` or `ferrous`.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "fp",
    version,
    about = "FerrousPanel VPS terminal CLI toolbox (replaces legacy ferrous-cli.sh)"
)]
pub struct FpCli {
    /// Persistent state directory (SQLite DB and 0600 credentials file)
    #[arg(long, env = "FERROUS_DATA_DIR", default_value = "/var/lib/ferrous-panel")]
    pub data_dir: PathBuf,

    /// Mandatory website root containment directory
    #[arg(long, env = "FERROUS_WWWROOT", default_value = "/www/wwwroot")]
    pub wwwroot: PathBuf,

    /// Administrator username
    #[arg(long, env = "FERROUS_ADMIN_USER", default_value = "admin_ferrous")]
    pub admin_username: String,

    /// Environment file read by systemd (`panel.env`)
    #[arg(long, env = "FERROUS_ENV_FILE", default_value = DEFAULT_ENV_FILE)]
    pub env_file: PathBuf,

    #[command(subcommand)]
    pub command: Option<FpCommand>,
}

/// Subcommand wrapper for `ferrous-panel fp <SUBCOMMAND>`.
#[derive(Debug, Clone, Parser)]
pub struct FpArgs {
    /// Environment file read by systemd (`panel.env`)
    #[arg(long, env = "FERROUS_ENV_FILE", default_value = DEFAULT_ENV_FILE)]
    pub env_file: PathBuf,

    #[command(subcommand)]
    pub command: Option<FpCommand>,
}

/// Supported `fp` management operations (`fp <command>` or numeric `fp <1..19>`).
#[derive(Debug, Clone, Subcommand)]
pub enum FpCommand {
    /// (1) Restart ferrous-panel.service
    #[command(alias = "1")]
    Restart,
    /// (2) Stop ferrous-panel.service
    #[command(alias = "2")]
    Stop,
    /// (3) Start ferrous-panel.service
    #[command(alias = "3")]
    Start,
    /// (4) Reload configured frontend web server engine and panel service
    #[command(alias = "4")]
    Reload,
    /// (5) Change Web Panel admin password (Argon2id hash; prompted interactively or --generate)
    #[command(alias = "5")]
    Passwd {
        /// Generate a random 24-character password using kernel OsRng instead of reading stdin
        #[arg(long, default_value_t = false)]
        generate: bool,
    },
    /// (6) Change Web Panel admin username
    #[command(alias = "6")]
    Username {
        /// New admin username (prompted interactively if omitted)
        username: Option<String>,
    },
    /// (7) Change Web Panel listening port in panel.env and open UFW rule
    #[command(alias = "7")]
    Port {
        /// Port number (1..=65535)
        port: Option<u16>,
    },
    /// (8) Change security entrance path in panel.env
    #[command(alias = "8")]
    Entrance {
        /// URL path prefix (e.g. /panel or /)
        path: Option<String>,
    },
    /// (9) Reset security entrance path back to '/'
    #[command(alias = "9", name = "clear-entrance")]
    ClearEntrance,
    /// (10) Set or change Web Panel custom domain in panel.env
    #[command(alias = "10")]
    Domain {
        /// Custom domain (e.g. panel.example.com; prompted interactively if omitted)
        domain: Option<String>,
    },
    /// (11) Validate active web server config, ensure /www/acme permissions, and reload engine
    #[command(alias = "11")]
    Repair,
    /// (12) Clear Web Panel custom domain binding in panel.env
    #[command(alias = "12", name = "clear-domain")]
    ClearDomain,
    /// (13) Change Web Panel listen bind IP (0.0.0.0 for public IP or 127.0.0.1 for local-only)
    #[command(alias = "13")]
    Bind {
        /// Bind IP address (0.0.0.0 or 127.0.0.1; prompted interactively if omitted)
        ip: Option<String>,
    },
    /// (14) Show panel URL, active engine, credential file path, and admin username (never prints stored password)
    #[command(alias = "14", alias = "default", alias = "info")]
    Status,
    /// (15) Check real service states (ferrous-panel, nginx/ferrousx, mariadb, postgresql, php8.3-fpm)
    #[command(alias = "15")]
    Check,
    /// (16) Show recent journalctl logs for ferrous-panel.service
    #[command(alias = "16")]
    Logs {
        /// Number of log lines to display
        #[arg(short = 'n', long, default_value_t = 40)]
        lines: u16,
    },
    /// (17) Fast update or rollback FerrousPanel (incremental VPS profile, prebuilt release, or rollback)
    #[command(alias = "17", alias = "upgrade")]
    Update {
        /// Check for available updates without modifying binaries or restarting
        #[arg(long, default_value_t = false)]
        check: bool,
        /// Download SHA-256 verified prebuilt GitHub Release binary (~3s, skips Cargo build)
        #[arg(long, default_value_t = false)]
        prebuilt: bool,
        /// Force rebuild/reinstall even if already on the latest commit
        #[arg(long, default_value_t = false)]
        force: bool,
        /// Also sync & update FerrousX engine
        #[arg(long, default_value_t = false)]
        ferrousx: bool,
        /// Rollback immediately to the previously installed working binary
        #[arg(long, default_value_t = false)]
        rollback: bool,
    },
    /// (19) View or switch active frontend web server engine (nginx | ferrousx)
    #[command(alias = "19")]
    Webserver {
        /// Optional target engine to switch to ('nginx' or 'ferrousx')
        engine: Option<String>,
    },
    /// First-run setup helper: initialize 0600 Argon2id credentials & engine choice (for install.sh)
    Init {
        /// Frontend web server engine to record ('nginx' or 'ferrousx')
        #[arg(long, default_value = "nginx")]
        engine: String,
    },
}

fn prompt_line(prompt: &str) -> Result<String> {
    let mut stdout = io::stdout();
    write!(stdout, "{prompt}")?;
    stdout.flush()?;
    let mut buf = String::new();
    let bytes = io::stdin().lock().read_line(&mut buf)?;
    if bytes == 0 {
        bail!("end of input while reading prompt");
    }
    Ok(buf.trim().to_string())
}

async fn run_systemctl(args: &[&str]) -> Result<()> {
    if !AllowedBinary::Systemctl.is_available() {
        bail!("systemctl binary is not available on this host");
    }
    let out = cmd::run(AllowedBinary::Systemctl, args, CLI_CMD_TIMEOUT)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    if !out.success {
        bail!(
            "systemctl {} failed (exit {:?}): {}",
            args.join(" "),
            out.exit_code,
            out.stderr
        );
    }
    Ok(())
}

async fn ensure_clean_panel_systemd_unit(env_file: &Path) -> Result<()> {
    let unit_path = Path::new("/etc/systemd/system/ferrous-panel.service");
    if !unit_path.is_file() {
        return Ok(());
    }
    let Ok(raw) = std::fs::read_to_string(unit_path) else {
        return Ok(());
    };

    let env_line = format!("EnvironmentFile=-{}", env_file.display());
    let mut has_env_file = false;
    let mut changed = false;
    let mut out_lines: Vec<String> = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed == env_line {
            has_env_file = true;
            out_lines.push(line.to_string());
            continue;
        }
        if trimmed.starts_with("Environment=") && trimmed.contains("FERROUS_PORT=") {
            changed = true;
            continue;
        }
        if trimmed.starts_with("ExecStart=") && trimmed.contains("--port") {
            let value = trimmed.trim_start_matches("ExecStart=").trim();
            let mut filtered: Vec<&str> = Vec::new();
            let mut skip_next = false;
            for tok in value.split_whitespace() {
                if skip_next {
                    skip_next = false;
                    continue;
                }
                if tok == "--port" {
                    skip_next = true;
                    continue;
                }
                if tok.starts_with("--port=") {
                    continue;
                }
                filtered.push(tok);
            }
            out_lines.push(format!("ExecStart={}", filtered.join(" ")));
            changed = true;
            continue;
        }
        if !has_env_file && trimmed.starts_with("ExecStart=") {
            out_lines.push(env_line.clone());
            has_env_file = true;
            changed = true;
        }
        out_lines.push(line.to_string());
    }

    if !has_env_file {
        out_lines.push(env_line);
        changed = true;
    }

    let mut dropin_changed = false;
    let dropin_dir = Path::new("/etc/systemd/system/ferrous-panel.service.d");
    if dropin_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(dropin_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) != Some("conf") {
                    continue;
                }
                let Ok(drop_raw) = std::fs::read_to_string(&p) else {
                    continue;
                };
                if drop_raw.contains("FERROUS_PORT=") || drop_raw.contains("--port") {
                    let filtered_lines: Vec<&str> = drop_raw
                        .lines()
                        .filter(|l| {
                            let t = l.trim();
                            !(t.starts_with("Environment=") && t.contains("FERROUS_PORT="))
                                && !(t.starts_with("ExecStart=") && t.contains("--port"))
                        })
                        .collect();
                    let has_directive = filtered_lines.iter().any(|l| {
                        let t = l.trim();
                        !t.is_empty() && !t.starts_with('#') && !t.starts_with('[')
                    });
                    if has_directive {
                        let mut next_drop = filtered_lines.join("\n");
                        next_drop.push('\n');
                        let _ = atomic_write_file(&p, next_drop.as_bytes(), 0o644);
                    } else {
                        let _ = std::fs::remove_file(&p);
                    }
                    dropin_changed = true;
                }
            }
        }
    }

    if changed {
        let mut next_content = out_lines.join("\n");
        if !next_content.is_empty() {
            next_content.push('\n');
        }
        atomic_write_file(unit_path, next_content.as_bytes(), 0o644)
            .map_err(|e| anyhow::anyhow!("failed updating {}: {e}", unit_path.display()))?;
    }
    if (changed || dropin_changed) && AllowedBinary::Systemctl.is_available() {
        run_systemctl(&["daemon-reload"]).await?;
    }
    Ok(())
}

async fn wait_panel_ready(bind: &str, port: u16) -> bool {
    let probe_host = match bind.trim() {
        "" | "0.0.0.0" | "::" | "[::]" => "127.0.0.1",
        other => other,
    };
    for _ in 0..24 {
        if tokio::net::TcpStream::connect((probe_host, port))
            .await
            .is_ok()
        {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    false
}

fn read_env_key(env_file: &Path, key: &str) -> Option<String> {
    let raw = std::fs::read_to_string(env_file).ok()?;
    let mut found: Option<String> = None;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            let clean_key = k.trim().trim_start_matches("export ").trim();
            if clean_key == key {
                let val = v.trim().trim_matches('"').trim_matches('\'').trim();
                if !val.is_empty() {
                    found = Some(val.to_string());
                } else {
                    found = None;
                }
            }
        }
    }
    found
}

fn detect_display_host(env_file: &Path) -> String {
    if let Some(ip) = read_env_key(env_file, "FERROUS_PUBLIC_IP") {
        if ip != "127.0.0.1" && ip != "0.0.0.0" {
            return ip;
        }
    }
    if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("1.1.1.1:80").is_ok() {
            if let Ok(addr) = sock.local_addr() {
                let ip = addr.ip().to_string();
                if !ip.starts_with("127.") && ip != "0.0.0.0" {
                    return ip;
                }
            }
        }
    }
    "<VPS-IP>".to_string()
}

fn cli_flag_provided(flag: &str) -> bool {
    let prefix = format!("{flag}=");
    std::env::args().skip(1).any(|arg| arg == flag || arg.starts_with(&prefix))
}

/// Populates `Config` fields from a systemd `EnvironmentFile` (`panel.env` or `.env`) when present.
/// Explicit CLI flags always take precedence over values in the environment file.
pub fn apply_env_file(cfg: &mut Config, env_path: &Path) {
    let Ok(raw) = std::fs::read_to_string(env_path) else {
        return;
    };
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((k, v)) = trimmed.split_once('=') else {
            continue;
        };
        let key = k.trim().trim_start_matches("export ").trim();
        let val = v.trim().trim_matches('"').trim_matches('\'').trim();
        match key {
            "FERROUS_BIND" if !val.is_empty() && !cli_flag_provided("--bind") => {
                cfg.bind = val.to_string();
            }
            "FERROUS_PORT" if !cli_flag_provided("--port") => {
                if let Ok(p) = val.parse::<u16>() {
                    cfg.port = if p == 0 || is_reserved_website_port(p) {
                        DEFAULT_PANEL_PORT
                    } else {
                        p
                    };
                }
            }
            "FERROUS_DATA_DIR" if !val.is_empty() && !cli_flag_provided("--data-dir") => {
                cfg.data_dir = PathBuf::from(val);
            }
            "FERROUS_WWWROOT" if !val.is_empty() && !cli_flag_provided("--wwwroot") => {
                cfg.wwwroot = PathBuf::from(val);
            }
            "FERROUS_BACKUPS_DIR" if !val.is_empty() && !cli_flag_provided("--backups-dir") => {
                cfg.backups_dir = PathBuf::from(val);
            }
            "FERROUS_TRUSTED_PROXIES"
                if !val.is_empty() && !cli_flag_provided("--trusted-proxies") =>
            {
                cfg.trusted_proxies = val.to_string();
            }
            "FERROUS_SYSTEMD_DIR"
                if !val.is_empty() && !cli_flag_provided("--systemd-unit-dir") =>
            {
                cfg.systemd_unit_dir = PathBuf::from(val);
            }
            "FERROUS_ADMIN_USER" if !val.is_empty() && !cli_flag_provided("--admin-username") => {
                cfg.admin_username = val.to_string();
            }
            "FERROUS_SECURITY_ENTRANCE"
                if !val.is_empty() && !cli_flag_provided("--security-entrance") =>
            {
                cfg.security_entrance = val.to_string();
            }
            "FERROUS_CUSTOM_DOMAIN" if !cli_flag_provided("--custom-domain") => {
                cfg.custom_domain = if val.is_empty() {
                    None
                } else {
                    Some(val.to_string())
                };
            }
            "FERROUS_SECURE_COOKIE" if !cli_flag_provided("--secure-cookie") => {
                cfg.secure_cookie =
                    matches!(val.to_ascii_lowercase().as_str(), "1" | "true" | "yes");
            }
            _ => {}
        }
    }
}

/// Automatically loads `/etc/ferrous-panel/panel.env` (or `$FERROUS_ENV_FILE`) and local `.env`
/// for any `FERROUS_*` keys not already set via explicit CLI flags or the process environment.
pub fn apply_default_env_files(cfg: &mut Config) {
    let primary = std::env::var_os("FERROUS_ENV_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_ENV_FILE));
    for candidate in [PathBuf::from(".env"), primary] {
        let Ok(raw) = std::fs::read_to_string(&candidate) else {
            continue;
        };
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let Some((k, v)) = trimmed.split_once('=') else {
                continue;
            };
            let key = k.trim().trim_start_matches("export ").trim();
            if std::env::var_os(key).is_some() {
                continue;
            }
            let val = v.trim().trim_matches('"').trim_matches('\'').trim();
            match key {
                "FERROUS_BIND" if !val.is_empty() && !cli_flag_provided("--bind") => {
                    cfg.bind = val.to_string();
                }
                "FERROUS_PORT" if !cli_flag_provided("--port") => {
                    if let Ok(p) = val.parse::<u16>() {
                        cfg.port = if p == 0 || is_reserved_website_port(p) {
                            DEFAULT_PANEL_PORT
                        } else {
                            p
                        };
                    }
                }
                "FERROUS_DATA_DIR" if !val.is_empty() && !cli_flag_provided("--data-dir") => {
                    cfg.data_dir = PathBuf::from(val);
                }
                "FERROUS_WWWROOT" if !val.is_empty() && !cli_flag_provided("--wwwroot") => {
                    cfg.wwwroot = PathBuf::from(val);
                }
                "FERROUS_BACKUPS_DIR"
                    if !val.is_empty() && !cli_flag_provided("--backups-dir") =>
                {
                    cfg.backups_dir = PathBuf::from(val);
                }
                "FERROUS_SYSTEMD_DIR"
                    if !val.is_empty() && !cli_flag_provided("--systemd-unit-dir") =>
                {
                    cfg.systemd_unit_dir = PathBuf::from(val);
                }
                "FERROUS_ADMIN_USER"
                    if !val.is_empty() && !cli_flag_provided("--admin-username") =>
                {
                    cfg.admin_username = val.to_string();
                }
                "FERROUS_SECURITY_ENTRANCE"
                    if !val.is_empty() && !cli_flag_provided("--security-entrance") =>
                {
                    cfg.security_entrance = val.to_string();
                }
                "FERROUS_CUSTOM_DOMAIN" if !cli_flag_provided("--custom-domain") => {
                    cfg.custom_domain = if val.is_empty() {
                        None
                    } else {
                        Some(val.to_string())
                    };
                }
                "FERROUS_SECURE_COOKIE" if !cli_flag_provided("--secure-cookie") => {
                    cfg.secure_cookie =
                        matches!(val.to_ascii_lowercase().as_str(), "1" | "true" | "yes");
                }
                _ => {}
            }
        }
    }
}

/// Ensures `env_path` (`/etc/ferrous-panel/panel.env`) exists (mode `0600`) and contains all
/// required `FERROUS_*` environment variables, preserving any existing custom keys.
pub fn ensure_default_env_file(env_path: &Path, cfg: &Config) -> Result<()> {
    let existing_keys = if env_path.is_file() {
        let raw = std::fs::read_to_string(env_path).unwrap_or_default();
        raw.lines()
            .filter_map(|l| {
                let t = l.trim();
                if t.is_empty() || t.starts_with('#') {
                    None
                } else {
                    t.split_once('=').map(|(k, _)| k.trim().to_string())
                }
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let defaults: [(&str, String); 11] = [
        ("FERROUS_BIND", cfg.bind.clone()),
        ("FERROUS_PORT", cfg.port.to_string()),
        ("FERROUS_DATA_DIR", cfg.data_dir.display().to_string()),
        ("FERROUS_WWWROOT", cfg.wwwroot.display().to_string()),
        ("FERROUS_BACKUPS_DIR", cfg.backups_dir.display().to_string()),
        (
            "FERROUS_SYSTEMD_DIR",
            cfg.systemd_unit_dir.display().to_string(),
        ),
        ("FERROUS_ADMIN_USER", cfg.admin_username.clone()),
        ("FERROUS_SECURITY_ENTRANCE", cfg.security_entrance.clone()),
        (
            "FERROUS_CUSTOM_DOMAIN",
            cfg.custom_domain.clone().unwrap_or_default(),
        ),
        (
            "FERROUS_SECURE_COOKIE",
            if cfg.secure_cookie { "true" } else { "false" }.to_string(),
        ),
        ("RUST_LOG", "info,actix_web=info".to_string()),
    ];

    for (key, val) in defaults {
        if !existing_keys.iter().any(|k| k == key) {
            upsert_env_var(env_path, key, &val)?;
        }
    }
    Ok(())
}

pub fn upsert_env_var(env_path: &Path, key: &str, value: &str) -> Result<()> {
    if value.contains('\n') || value.contains('\r') || value.contains('\0') {
        bail!("invalid control character in environment value");
    }
    let existing = if env_path.is_file() {
        std::fs::read_to_string(env_path)
            .with_context(|| format!("failed to read {}", env_path.display()))?
    } else {
        String::new()
    };

    let mut ordered_lines = Vec::new();
    let mut kv_seen = BTreeMap::new();
    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            ordered_lines.push(line.to_string());
            continue;
        }
        if let Some((k, _)) = trimmed.split_once('=') {
            let clean_key = k.trim().trim_start_matches("export ").trim();
            if clean_key == key {
                if !kv_seen.contains_key(key) {
                    ordered_lines.push(format!("{key}={value}"));
                    kv_seen.insert(key.to_string(), true);
                }
                continue;
            }
        }
        ordered_lines.push(line.to_string());
    }
    if !kv_seen.contains_key(key) {
        ordered_lines.push(format!("{key}={value}"));
    }

    let mut content = ordered_lines.join("\n");
    if !content.is_empty() {
        content.push('\n');
    }
    atomic_write_file(env_path, content.as_bytes(), 0o600)
        .map_err(|e| anyhow::anyhow!("failed writing {}: {e}", env_path.display()))
}

fn normalize_entrance(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("security entrance path cannot be empty");
    }
    if trimmed.contains('\n') || trimmed.contains('\r') || trimmed.contains(' ') {
        bail!("security entrance path must not contain spaces or newlines");
    }
    if trimmed.starts_with('/') {
        Ok(trimmed.to_string())
    } else {
        Ok(format!("/{trimmed}"))
    }
}

async fn print_status(config: &Config, env_file: &Path) -> Result<()> {
    let creds_path = config.credentials_path();
    let (username, has_creds) = if creds_path.is_file() {
        let raw = std::fs::read_to_string(&creds_path)?;
        let parsed: crate::auth::password::AdminCredentialsFile = serde_json::from_str(&raw)?;
        (parsed.username, true)
    } else {
        (config.admin_username.clone(), false)
    };

    let hook_exe =
        std::env::current_exe().unwrap_or_else(|_| PathBuf::from("/usr/local/bin/ferrous-panel"));
    let manager = WebServerManager::open_system(&config.db_path(), hook_exe)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let ws_cfg = manager
        .store
        .load_config()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let active_port = if config.port == 0 || is_reserved_website_port(config.port) {
        DEFAULT_PANEL_PORT
    } else {
        config.port
    };
    let display_host = detect_display_host(env_file);
    let probe_host = match config.bind.trim() {
        "" | "0.0.0.0" | "::" | "[::]" => "127.0.0.1",
        other => other,
    };
    let listening = tokio::net::TcpStream::connect((probe_host, active_port))
        .await
        .is_ok();

    println!("======================================================================");
    println!("  FERROUS-PANEL — RUNTIME STATUS (100% Pure Rust)");
    println!("======================================================================");
    println!(
        "  Bind Address     : {}:{} ({})",
        config.bind,
        active_port,
        if listening {
            "LISTENING"
        } else {
            "NOT LISTENING — run `sudo fp restart`"
        }
    );
    println!("  Security Entrance: {}", config.security_entrance);
    if config.bind == "127.0.0.1" || config.bind == "::1" {
        println!(
            "  Panel URL (Local): http://{}:{}{}",
            config.bind, active_port, config.security_entrance
        );
    } else {
        println!(
            "  Panel URL (IP)   : http://{display_host}:{active_port}{}",
            config.security_entrance
        );
    }
    if let Some(ref dom) = config.custom_domain {
        if !dom.trim().is_empty() {
            println!("  Custom Domain    : {}", dom.trim());
            println!(
                "  Panel URL (Domain): http://{}:{active_port}{}",
                dom.trim(),
                config.security_entrance
            );
            println!(
                "  Cloudflare Note  : Set DNS record '{}' to DNS-Only (Gray Cloud) for HTTP :{active_port}; ports 80/443 are reserved strictly for websites.",
                dom.trim()
            );
        }
    }
    println!("  Admin Username   : {username}");
    println!(
        "  Credentials File : {} ({}, mode 0600, Argon2id — plaintext never stored)",
        creds_path.display(),
        if has_creds { "initialized" } else { "not initialized" }
    );
    println!("  SQLite Store     : {}", config.db_path().display());
    println!("  Web Root         : {}", config.wwwroot.display());
    println!(
        "  Web Server Engine: {} (exclusively owns ports 80 & 443 for websites)",
        ws_cfg.frontend_engine
    );
    println!("======================================================================");
    Ok(())
}

async fn check_services(config: &Config) -> Result<()> {
    let hook_exe =
        std::env::current_exe().unwrap_or_else(|_| PathBuf::from("/usr/local/bin/ferrous-panel"));
    let manager = WebServerManager::open_system(&config.db_path(), hook_exe)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let ws_cfg = manager
        .store
        .load_config()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let engine_unit = match ws_cfg.frontend_engine {
        EngineKind::Nginx => "nginx.service",
        EngineKind::Ferrousx => "ferrousx.service",
    };

    println!("FerrousPanel System Check (active engine: {}):", ws_cfg.frontend_engine);
    if AllowedBinary::Systemctl.is_available() {
        for unit in [
            PANEL_SERVICE,
            engine_unit,
            "mariadb.service",
            "postgresql.service",
            "php8.3-fpm.service",
        ] {
            let out = cmd::run(AllowedBinary::Systemctl, &["is-active", unit], CLI_CMD_TIMEOUT)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let state = if out.stdout.is_empty() {
                "unknown"
            } else {
                out.stdout.as_str()
            };
            println!("  - {unit:<22} : {state}");
        }
    } else {
        println!("  - systemctl not found on host");
    }

    let validation = manager
        .validate_configured_engine()
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!(
        "  - Config validation ({}) : {} ({})",
        validation.engine,
        if validation.valid { "OK" } else { "FAILED" },
        validation.output
    );
    println!("Zero-Node.js JS/CSS & Rust Compiler Toolchain:");
    println!(
        "  - bun (JS/TS frameworks)  : {}",
        AllowedBinary::Bun
            .resolved_path_str()
            .unwrap_or("not installed (run update.sh or Software Store)")
    );
    let node_shim_status = match std::fs::read_link("/usr/local/bin/node") {
        Ok(target) => format!("/usr/local/bin/node -> {} (Zero-Node.js compat)", target.display()),
        Err(_) if Path::new("/usr/bin/node").is_file() => "/usr/bin/node (system)".to_string(),
        Err(_) => "not linked (using `bun --bun` directly)".to_string(),
    };
    println!("  - node compat shim        : {node_shim_status}");
    println!(
        "  - tailwindcss (CSS v4)    : {}",
        AllowedBinary::TailwindCli
            .resolved_path_str()
            .unwrap_or("not installed (run update.sh or Software Store)")
    );
    println!(
        "  - deno (optional Rust V8) : {}",
        AllowedBinary::Deno
            .resolved_path_str()
            .unwrap_or("not installed (optional)")
    );
    println!(
        "  - rustc (Rust & FFI ABI)  : {}",
        AllowedBinary::Rustc
            .resolved_path_str()
            .unwrap_or("not installed")
    );
    println!(
        "  - cargo (Rust frameworks) : {}",
        AllowedBinary::Cargo
            .resolved_path_str()
            .unwrap_or("not installed")
    );
    Ok(())
}

/// Executes a single `fp` subcommand cleanly using allowlisted binaries and atomic config writes.
pub async fn run_fp_command(
    command: FpCommand,
    config: &Config,
    env_file: &Path,
) -> Result<()> {
    match command {
        FpCommand::Restart => {
            run_systemctl(&["restart", PANEL_SERVICE]).await?;
            println!("Restarted {PANEL_SERVICE}.");
        }
        FpCommand::Stop => {
            run_systemctl(&["stop", PANEL_SERVICE]).await?;
            println!("Stopped {PANEL_SERVICE}.");
        }
        FpCommand::Start => {
            run_systemctl(&["start", PANEL_SERVICE]).await?;
            println!("Started {PANEL_SERVICE}.");
        }
        FpCommand::Reload => {
            let hook_exe = std::env::current_exe()
                .unwrap_or_else(|_| PathBuf::from("/usr/local/bin/ferrous-panel"));
            let manager = WebServerManager::open_system(&config.db_path(), hook_exe)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            manager
                .reload_or_start_configured_engine()
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if AllowedBinary::Systemctl.is_available() {
                let _ = run_systemctl(&["reload-or-restart", PANEL_SERVICE]).await;
            }
            println!("Reloaded configured web server engine and panel service.");
        }
        FpCommand::Passwd { generate } => {
            let creds_path = config.credentials_path();
            let plain_secret = if generate {
                generate_initial_password().map_err(|e| anyhow::anyhow!("{e}"))?
            } else {
                let mut accepted: Option<String> = None;
                for attempt in 1..=3 {
                    let input = prompt_line(
                        "Enter new Admin password (8..=256 chars; input read from stdin): ",
                    )?;
                    if !(8..=256).contains(&input.len()) {
                        eprintln!(
                            "Password length must be 8..=256 chars (got {} chars). Attempt {attempt}/3.",
                            input.len()
                        );
                        continue;
                    }
                    let confirm = prompt_line("Confirm new Admin password: ")?;
                    if input != confirm {
                        eprintln!(
                            "Passwords do not match (enter the exact same password twice). Attempt {attempt}/3."
                        );
                        continue;
                    }
                    accepted = Some(input);
                    break;
                }
                let Some(valid_pw) = accepted else {
                    bail!("password update aborted after 3 invalid/mismatched attempts");
                };
                crate::auth::password::RedactedSecret::new(valid_pw)
            };
            let updated =
                update_admin_password(&creds_path, &config.admin_username, plain_secret.expose())
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
            if let Ok(auth_store) = crate::auth::store::AuthStore::open(&config.db_path()) {
                let _ = auth_store.revoke_all_sessions();
            }
            if AllowedBinary::Systemctl.is_available() {
                let _ = run_systemctl(&["restart", PANEL_SERVICE]).await;
            }
            println!(
                "Updated Argon2id password hash for '{}' in {} (mode 0600) and reloaded panel service.",
                updated.username,
                creds_path.display()
            );
            if generate {
                println!(
                    "Generated one-time password (NOT saved anywhere): {}",
                    plain_secret.expose()
                );
            }
        }
        FpCommand::Username { username } => {
            let new_user = match username {
                Some(u) => u,
                None => prompt_line("Enter new Admin username (3..=64 chars): ")?,
            };
            let creds_path = config.credentials_path();
            let updated =
                update_admin_username(&creds_path, &config.admin_username, &new_user)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
            let _ = upsert_env_var(env_file, "FERROUS_ADMIN_USER", &updated.username);
            if let Ok(auth_store) = crate::auth::store::AuthStore::open(&config.db_path()) {
                let _ = auth_store.revoke_all_sessions();
            }
            if AllowedBinary::Systemctl.is_available() {
                let _ = run_systemctl(&["restart", PANEL_SERVICE]).await;
            }
            println!(
                "Updated Admin username to '{}' in {} and reloaded panel service.",
                updated.username,
                creds_path.display()
            );
        }
        FpCommand::Port { port } => {
            let current_port = if config.port == 0 || is_reserved_website_port(config.port) {
                DEFAULT_PANEL_PORT
            } else {
                config.port
            };
            let port_num = match port {
                Some(p) => p,
                None => {
                    let prompt = format!(
                        "Enter new panel port (1..=65535, excluding website ports 80/443; current {current_port}, default {DEFAULT_PANEL_PORT}): "
                    );
                    let raw = prompt_line(&prompt)?;
                    let cleaned = raw.trim().trim_start_matches(':');
                    if cleaned.is_empty() {
                        println!("Port unchanged (still {current_port}).");
                        return Ok(());
                    }
                    if cleaned.eq_ignore_ascii_case("default") {
                        DEFAULT_PANEL_PORT
                    } else {
                        cleaned
                            .parse::<u16>()
                            .context("invalid port integer (expected 1..=65535, excluding 80/443)")?
                    }
                }
            };
            let valid_port = validate_panel_port(port_num).map_err(|e| anyhow::anyhow!("{e}"))?;
            let prev_port = current_port;

            ensure_default_env_file(env_file, config)?;
            upsert_env_var(env_file, "FERROUS_PORT", &valid_port.to_string())?;
            ensure_clean_panel_systemd_unit(env_file).await?;

            let effective_bind = read_env_key(env_file, "FERROUS_BIND")
                .unwrap_or_else(|| config.bind.clone());

            if effective_bind != "127.0.0.1"
                && effective_bind != "::1"
                && AllowedBinary::Ufw.is_available()
            {
                let rule = format!("{valid_port}/tcp");
                let _ = cmd::run(
                    AllowedBinary::Ufw,
                    &["allow", &rule, "comment", "FerrousPanel"],
                    CLI_CMD_TIMEOUT,
                )
                .await;
            }

            if AllowedBinary::Systemctl.is_available() {
                if let Err(err) = run_systemctl(&["restart", PANEL_SERVICE]).await {
                    if prev_port != valid_port.get() {
                        let _ = upsert_env_var(env_file, "FERROUS_PORT", &prev_port.to_string());
                        let _ = run_systemctl(&["restart", PANEL_SERVICE]).await;
                    }
                    return Err(err).with_context(|| {
                        format!(
                            "failed to restart {PANEL_SERVICE} on port {valid_port}; reverted FERROUS_PORT to {prev_port}"
                        )
                    });
                }
                if !wait_panel_ready(&effective_bind, valid_port.get()).await {
                    if prev_port != valid_port.get() {
                        let _ = upsert_env_var(env_file, "FERROUS_PORT", &prev_port.to_string());
                        let _ = run_systemctl(&["restart", PANEL_SERVICE]).await;
                    }
                    bail!(
                        "{PANEL_SERVICE} is not listening on {effective_bind}:{valid_port} (port may already be in use by another service); reverted FERROUS_PORT to {prev_port}. Check `sudo fp logs`."
                    );
                }
            }

            println!(
                "Updated FERROUS_PORT={valid_port} in {} and verified {PANEL_SERVICE} is listening on {effective_bind}:{valid_port}.",
                env_file.display()
            );
            if effective_bind == "127.0.0.1" || effective_bind == "::1" {
                println!(
                    "  Panel URL (Local) : http://{effective_bind}:{valid_port}{}",
                    config.security_entrance
                );
                println!("  Note: bind IP is local-only; run `sudo fp bind 0.0.0.0` to allow public access.");
            } else {
                let host = detect_display_host(env_file);
                println!(
                    "  Panel URL (IP)    : http://{host}:{valid_port}{}",
                    config.security_entrance
                );
                if let Some(ref dom) = config.custom_domain {
                    if !dom.trim().is_empty() {
                        println!(
                            "  Panel URL (Domain): http://{}:{valid_port}{}",
                            dom.trim(),
                            config.security_entrance
                        );
                        println!(
                            "  Cloudflare Note   : Keep '{}' on DNS-Only (Gray Cloud) to reach HTTP :{valid_port}. Ports 80/443 are reserved strictly for websites.",
                            dom.trim()
                        );
                    }
                }
            }
        }
        FpCommand::Entrance { path } => {
            let raw = match path {
                Some(p) => p,
                None => prompt_line("Enter new security entrance path (e.g. /panel): ")?,
            };
            let clean = normalize_entrance(&raw)?;
            upsert_env_var(env_file, "FERROUS_SECURITY_ENTRANCE", &clean)?;
            if AllowedBinary::Systemctl.is_available() {
                run_systemctl(&["restart", PANEL_SERVICE]).await?;
            }
            println!(
                "Updated FERROUS_SECURITY_ENTRANCE={clean} in {} and restarted {PANEL_SERVICE}.",
                env_file.display()
            );
        }
        FpCommand::ClearEntrance => {
            upsert_env_var(env_file, "FERROUS_SECURITY_ENTRANCE", "/")?;
            if AllowedBinary::Systemctl.is_available() {
                run_systemctl(&["restart", PANEL_SERVICE]).await?;
            }
            println!(
                "Reset FERROUS_SECURITY_ENTRANCE=/ in {} and restarted {PANEL_SERVICE}.",
                env_file.display()
            );
        }
        FpCommand::Domain { domain } => {
            let raw = match domain {
                Some(d) => d,
                None => prompt_line(
                    "Enter panel custom domain (e.g. panel.example.com, or 'clear' to remove): ",
                )?,
            };
            let trimmed = raw.trim();
            if trimmed.is_empty()
                || trimmed.eq_ignore_ascii_case("clear")
                || trimmed.eq_ignore_ascii_case("none")
            {
                upsert_env_var(env_file, "FERROUS_CUSTOM_DOMAIN", "")?;
                if AllowedBinary::Systemctl.is_available() {
                    run_systemctl(&["restart", PANEL_SERVICE]).await?;
                }
                println!(
                    "Cleared FERROUS_CUSTOM_DOMAIN in {} and restarted {PANEL_SERVICE}.",
                    env_file.display()
                );
            } else {
                let valid_domain = Domain::parse(trimmed).map_err(|e| anyhow::anyhow!("{e}"))?;
                upsert_env_var(env_file, "FERROUS_CUSTOM_DOMAIN", valid_domain.as_str())?;
                if AllowedBinary::Systemctl.is_available() {
                    run_systemctl(&["restart", PANEL_SERVICE]).await?;
                }
                let active_port = if config.port == 0 || is_reserved_website_port(config.port) {
                    DEFAULT_PANEL_PORT
                } else {
                    config.port
                };
                println!(
                    "Updated FERROUS_CUSTOM_DOMAIN={} in {} and restarted {PANEL_SERVICE}.",
                    valid_domain.as_str(),
                    env_file.display()
                );
                println!(
                    "  Panel URL: http://{}:{active_port}{}",
                    valid_domain.as_str(),
                    config.security_entrance
                );
                println!(
                    "  Note     : Keep '{}' on DNS-Only (Gray Cloud) if using Cloudflare; ports 80/443 are reserved strictly for websites.",
                    valid_domain.as_str()
                );
            }
        }
        FpCommand::ClearDomain => {
            upsert_env_var(env_file, "FERROUS_CUSTOM_DOMAIN", "")?;
            if AllowedBinary::Systemctl.is_available() {
                run_systemctl(&["restart", PANEL_SERVICE]).await?;
            }
            println!(
                "Cleared FERROUS_CUSTOM_DOMAIN in {} and restarted {PANEL_SERVICE}.",
                env_file.display()
            );
        }
        FpCommand::Bind { ip } => {
            let raw = match ip {
                Some(v) => v,
                None => prompt_line(
                    "Enter listen bind IP (0.0.0.0 for public VPS IP, 127.0.0.1 for local-only): ",
                )?,
            };
            let trimmed = raw.trim();
            let parsed_ip: std::net::IpAddr = trimmed
                .parse()
                .with_context(|| format!("invalid IP address '{trimmed}'"))?;
            let ip_str = parsed_ip.to_string();
            upsert_env_var(env_file, "FERROUS_BIND", &ip_str)?;
            ensure_clean_panel_systemd_unit(env_file).await?;
            if !parsed_ip.is_loopback() && AllowedBinary::Ufw.is_available() {
                let rule = format!("{}/tcp", config.port);
                let _ = cmd::run(
                    AllowedBinary::Ufw,
                    &["allow", &rule, "comment", "FerrousPanel"],
                    CLI_CMD_TIMEOUT,
                )
                .await;
            }
            if AllowedBinary::Systemctl.is_available() {
                run_systemctl(&["restart", PANEL_SERVICE]).await?;
            }
            println!(
                "Updated FERROUS_BIND={ip_str} in {} and restarted {PANEL_SERVICE}.",
                env_file.display()
            );
            let public_host = detect_display_host(env_file);
            if parsed_ip.is_loopback() {
                println!(
                    "Local URL : http://127.0.0.1:{}{}",
                    config.port, config.security_entrance
                );
                println!(
                    "SSH tunnel: ssh -L {}:127.0.0.1:{} root@{}",
                    config.port, config.port, public_host
                );
            } else {
                if !config.secure_cookie {
                    eprintln!(
                        "[WARNING] CANH BAO BAO MAT: Panel dang chay HTTP khong ma hoa tren {ip_str}:{} (FERROUS_SECURE_COOKIE=false). Nen cau hinh HTTPS reverse proxy hoac quay lai 127.0.0.1.",
                        config.port
                    );
                }
                println!(
                    "Direct URL: http://{}:{}{}",
                    public_host, config.port, config.security_entrance
                );
                println!(
                    "Note: Ensure TCP port {} is also allowed in your cloud provider firewall / security group.",
                    config.port
                );
            }
        }
        FpCommand::Repair => {
            std::fs::create_dir_all(&config.wwwroot)?;
            std::fs::create_dir_all("/www/acme")?;
            let hook_exe = std::env::current_exe()
                .unwrap_or_else(|_| PathBuf::from("/usr/local/bin/ferrous-panel"));
            let manager = WebServerManager::open_system(&config.db_path(), hook_exe)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let report = manager
                .validate_configured_engine()
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if !report.valid {
                bail!("engine validation failed: {}", report.output);
            }
            manager
                .reload_or_start_configured_engine()
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!(
                "Verified and reloaded configured engine ({}).",
                report.engine
            );
        }
        FpCommand::Status => {
            print_status(config, env_file).await?;
        }
        FpCommand::Check => {
            check_services(config).await?;
        }
        FpCommand::Logs { lines } => {
            if !AllowedBinary::Journalctl.is_available() {
                bail!("journalctl binary is not available on this host");
            }
            let n_str = lines.clamp(1, 500).to_string();
            let out = cmd::run(
                AllowedBinary::Journalctl,
                &["-u", PANEL_SERVICE, "-n", &n_str, "--no-pager"],
                CLI_CMD_TIMEOUT,
            )
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
            if !out.stdout.is_empty() {
                println!("{}", out.stdout);
            }
            if !out.success {
                bail!(
                    "journalctl failed (exit {:?}): {}",
                    out.exit_code,
                    out.stderr
                );
            }
        }
        FpCommand::Webserver { engine } => {
            let hook_exe = std::env::current_exe()
                .unwrap_or_else(|_| PathBuf::from("/usr/local/bin/ferrous-panel"));
            let manager = WebServerManager::open_system(&config.db_path(), hook_exe)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            match engine {
                Some(target_raw) => {
                    let target =
                        EngineKind::parse(&target_raw).map_err(|e| anyhow::anyhow!("{e}"))?;
                    let auth_store = crate::auth::store::AuthStore::open(&config.db_path())
                        .map_err(|e| anyhow::anyhow!("{e}"))?;
                    let res = manager
                        .switch_engine(target, &auth_store, &config.admin_username, "127.0.0.1")
                        .await
                        .map_err(|e| anyhow::anyhow!("{e}"))?;
                    println!(
                        "Switched active frontend web server to '{}' (health-checked {} sites).",
                        res.active_engine,
                        res.health_checks.len()
                    );
                }
                None => {
                    let overview = manager.overview().await.map_err(|e| anyhow::anyhow!("{e}"))?;
                    println!("Active Web Server Engine: {}", overview.frontend_engine);
                    println!(
                        "  nginx    : installed={}, active={}, sub_state={}",
                        overview.nginx.installed, overview.nginx.active, overview.nginx.sub_state
                    );
                    println!(
                        "  ferrousx : installed={}, active={}, sub_state={}",
                        overview.ferrousx.installed,
                        overview.ferrousx.active,
                        overview.ferrousx.sub_state
                    );
                }
            }
        }
        FpCommand::Update {
            check,
            prebuilt,
            force,
            ferrousx,
            rollback,
        } => {
            let src_dir = PathBuf::from(
                std::env::var("FERROUS_SRC_DIR")
                    .unwrap_or_else(|_| "/usr/local/src/ferrous-panel".to_string()),
            );
            let branch = std::env::var("FERROUS_BRANCH").unwrap_or_else(|_| "main".to_string());
            let repo = std::env::var("FERROUS_REPO")
                .unwrap_or_else(|_| "Ferrous-panel/Ferrous-Panel".to_string());

            // Refresh update.sh from upstream before executing so updater fixes take effect immediately.
            let local_script = src_dir.join("update.sh");
            let temp_script = PathBuf::from(format!("/tmp/ferrous-update-{}.sh", std::process::id()));
            let script_to_run = if !rollback && src_dir.join(".git").is_dir() && AllowedBinary::Git.is_available() {
                let src_str = src_dir.display().to_string();
                let _ = cmd::run(
                    AllowedBinary::Git,
                    &["-C", &src_str, "fetch", "--depth", "1", "--quiet", "origin", &branch],
                    Duration::from_secs(30),
                )
                .await;
                let refspec = format!("origin/{branch}:update.sh");
                match cmd::run(
                    AllowedBinary::Git,
                    &["-C", &src_str, "show", &refspec],
                    Duration::from_secs(15),
                )
                .await
                {
                    Ok(out) if out.success && out.stdout.starts_with("#!/") => {
                        if std::fs::write(&temp_script, &out.stdout).is_ok() {
                            temp_script.clone()
                        } else {
                            local_script.clone()
                        }
                    }
                    _ => local_script.clone(),
                }
            } else if local_script.is_file() {
                local_script.clone()
            } else if AllowedBinary::Curl.is_available() {
                let url = format!("https://raw.githubusercontent.com/{repo}/{branch}/update.sh");
                let dst_str = temp_script.display().to_string();
                let dl = cmd::run(
                    AllowedBinary::Curl,
                    &["-fsSL", "--connect-timeout", "10", "-o", &dst_str, &url],
                    Duration::from_secs(30),
                )
                .await
                .map_err(|e| anyhow::anyhow!("failed to download update.sh: {e}"))?;
                if !dl.success {
                    bail!("failed to fetch update.sh from {url}: {}", dl.stderr.trim());
                }
                temp_script.clone()
            } else {
                bail!("update.sh not found at {} and curl is unavailable", local_script.display());
            };

            let mut args: Vec<String> = Vec::new();
            if check {
                args.push("--check".to_string());
            }
            if prebuilt {
                args.push("--prebuilt".to_string());
            }
            if force {
                args.push("--force".to_string());
            }
            if ferrousx {
                args.push("--ferrousx".to_string());
            }
            if rollback {
                args.push("--rollback".to_string());
            }

            let status = tokio::process::Command::new("/usr/bin/env")
                .arg("bash")
                .arg(&script_to_run)
                .args(&args)
                .status()
                .await
                .with_context(|| format!("failed to execute {}", script_to_run.display()))?;

            if temp_script.exists() {
                let _ = std::fs::remove_file(&temp_script);
            }

            if !status.success() {
                bail!(
                    "update.sh exited with status {}",
                    status.code().map(|c| c.to_string()).unwrap_or_else(|| "signal".to_string())
                );
            }
        }
        FpCommand::Init { engine } => {
            let parsed_engine =
                EngineKind::parse(&engine).map_err(|e| anyhow::anyhow!("{e}"))?;
            std::fs::create_dir_all(&config.data_dir)?;
            std::fs::create_dir_all(&config.wwwroot)?;
            ensure_default_env_file(env_file, config)?;
            let (creds, maybe_secret) =
                load_or_init_credentials(&config.credentials_path(), &config.admin_username)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
            let hook_exe = std::env::current_exe()
                .unwrap_or_else(|_| PathBuf::from("/usr/local/bin/ferrous-panel"));
            let manager = WebServerManager::open_system(&config.db_path(), hook_exe)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            manager
                .store
                .save_frontend_engine(parsed_engine)
                .map_err(|e| anyhow::anyhow!("{e}"))?;

            println!("Initialized FerrousPanel state at {}", config.data_dir.display());
            println!("  Env File   : {} (mode 0600)", env_file.display());
            println!("  Web Engine : {parsed_engine}");
            println!("  Username   : {}", creds.username);
            if let Some(secret) = maybe_secret {
                println!(
                    "  Password   : {} (SHOWN ONCE ONLY — stored solely as Argon2id hash in {})",
                    secret.expose(),
                    config.credentials_path().display()
                );
            } else {
                println!(
                    "  Password   : (already initialized in {}; use `fp passwd` to reset)",
                    config.credentials_path().display()
                );
            }
        }
    }
    Ok(())
}

/// Interactive `fp` menu displayed when invoked without subcommands.
pub async fn run_interactive_menu(config: &Config, env_file: &Path) -> Result<()> {
    println!("======================================================================");
    println!("  FERROUS-PANEL CLI (`fp`) — Pure Rust VPS Management Toolbox");
    println!("======================================================================");
    println!("  (1)  Restart panel service       (10) Change panel custom domain");
    println!("  (2)  Stop panel service          (11) Validate & reload web engine");
    println!("  (3)  Start panel service         (12) Clear panel custom domain");
    println!("  (4)  Reload web engine + panel   (13) Change bind IP (0.0.0.0/local)");
    println!("  (5)  Change Admin password       (14) Show panel status & endpoints");
    println!("  (6)  Change Admin username       (15) Check system service health");
    println!("  (7)  Change panel listening port (16) Show panel systemd logs");
    println!("  (8)  Change security entrance    (17) Update FerrousPanel (fast)");
    println!("  (9)  Clear security entrance '/' (19) Show web engine status");
    println!("  (0)  Exit");
    println!("======================================================================");

    let choice = prompt_line("Select action [0-19]: ")?;
    let cmd = match choice.as_str() {
        "0" | "" | "exit" | "quit" => return Ok(()),
        "1" => FpCommand::Restart,
        "2" => FpCommand::Stop,
        "3" => FpCommand::Start,
        "4" => FpCommand::Reload,
        "5" => FpCommand::Passwd { generate: false },
        "6" => FpCommand::Username { username: None },
        "7" => FpCommand::Port { port: None },
        "8" => FpCommand::Entrance { path: None },
        "9" => FpCommand::ClearEntrance,
        "10" => FpCommand::Domain { domain: None },
        "11" => FpCommand::Repair,
        "12" => FpCommand::ClearDomain,
        "13" => FpCommand::Bind { ip: None },
        "14" => FpCommand::Status,
        "15" => FpCommand::Check,
        "16" => FpCommand::Logs { lines: 40 },
        "17" => FpCommand::Update {
            check: false,
            prebuilt: false,
            force: false,
            ferrousx: false,
            rollback: false,
        },
        "19" => FpCommand::Webserver { engine: None },
        other => {
            return Err(anyhow::anyhow!(
                PanelError::Validation(format!("unknown menu selection '{other}'"))
            ));
        }
    };
    run_fp_command(cmd, config, env_file).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use rand::rngs::OsRng;
    use rand::RngCore;

    fn ensure(cond: bool, msg: &str) -> Result<()> {
        if cond {
            Ok(())
        } else {
            bail!("{msg}")
        }
    }

    fn temp_dir(prefix: &str) -> Result<PathBuf> {
        let mut buf = [0u8; 6];
        OsRng
            .try_fill_bytes(&mut buf)
            .map_err(|_| anyhow::anyhow!("OsRng failed"))?;
        let mut hex = String::with_capacity(12);
        for b in buf {
            use std::fmt::Write as _;
            let _ = write!(hex, "{b:02x}");
        }
        let dir = std::env::temp_dir().join(format!("ferrous-cli-{prefix}-{hex}"));
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    #[test]
    fn fp_cli_parses_subcommands_and_numeric_aliases() -> Result<()> {
        let parsed = FpCli::try_parse_from(["fp", "14"])?;
        ensure(
            matches!(parsed.command, Some(FpCommand::Status)),
            "fp 14 must map to Status",
        )?;

        let restart = FpCli::try_parse_from(["fp", "1"])?;
        ensure(
            matches!(restart.command, Some(FpCommand::Restart)),
            "fp 1 must map to Restart",
        )?;

        let passwd_gen = FpCli::try_parse_from(["fp", "passwd", "--generate"])?;
        ensure(
            matches!(passwd_gen.command, Some(FpCommand::Passwd { generate: true })),
            "fp passwd --generate must parse cleanly",
        )?;

        let full = Config::try_parse_from(["ferrous-panel", "fp", "check"])?;
        ensure(
            matches!(
                full.command,
                Some(PanelCommand::Fp(FpArgs {
                    command: Some(FpCommand::Check),
                    ..
                }))
            ),
            "ferrous-panel fp check must parse cleanly",
        )?;

        let domain_cmd = FpCli::try_parse_from(["fp", "domain", "panel.example.com"])?;
        ensure(
            matches!(
                domain_cmd.command,
                Some(FpCommand::Domain {
                    domain: Some(ref d)
                }) if d == "panel.example.com"
            ),
            "fp domain panel.example.com must parse cleanly",
        )?;

        let clear_dom = FpCli::try_parse_from(["fp", "12"])?;
        ensure(
            matches!(clear_dom.command, Some(FpCommand::ClearDomain)),
            "fp 12 must map to ClearDomain",
        )?;

        let bind_cmd = FpCli::try_parse_from(["fp", "bind", "0.0.0.0"])?;
        ensure(
            matches!(
                bind_cmd.command,
                Some(FpCommand::Bind { ip: Some(ref ip) }) if ip == "0.0.0.0"
            ),
            "fp bind 0.0.0.0 must parse cleanly",
        )?;

        Ok(())
    }

    #[test]
    fn upsert_env_var_preserves_comments_and_updates_atomically() -> Result<()> {
        let dir = temp_dir("env")?;
        let env_file = dir.join("panel.env");
        std::fs::write(
            &env_file,
            "# FerrousPanel env\nFERROUS_BIND=127.0.0.1\nFERROUS_PORT=8443\n",
        )?;

        upsert_env_var(&env_file, "FERROUS_PORT", "9443")?;
        upsert_env_var(&env_file, "FERROUS_SECURITY_ENTRANCE", "/panel-sec")?;

        let content = std::fs::read_to_string(&env_file)?;
        ensure(
            content.contains("# FerrousPanel env"),
            "comment line preserved",
        )?;
        ensure(
            content.contains("FERROUS_PORT=9443") && !content.contains("FERROUS_PORT=8443"),
            "FERROUS_PORT updated",
        )?;
        ensure(
            content.contains("FERROUS_SECURITY_ENTRANCE=/panel-sec"),
            "FERROUS_SECURITY_ENTRANCE appended",
        )?;

        let mut cfg = Config::for_test(dir.join("data"), dir.join("wwwroot"));
        ensure_default_env_file(&env_file, &cfg)?;
        apply_env_file(&mut cfg, &env_file);
        ensure(cfg.port == 9443, "apply_env_file loaded FERROUS_PORT=9443")?;
        ensure(
            cfg.security_entrance == "/panel-sec",
            "apply_env_file loaded FERROUS_SECURITY_ENTRANCE",
        )?;

        // Duplicate keys must be deduplicated and reserved website ports 80/443 must fall back to 8443.
        std::fs::write(
            &env_file,
            "FERROUS_PORT=443\nexport FERROUS_PORT=80\nFERROUS_BIND=0.0.0.0\n",
        )?;
        let mut reserved_cfg = Config::for_test(dir.join("data"), dir.join("wwwroot"));
        apply_env_file(&mut reserved_cfg, &env_file);
        ensure(
            reserved_cfg.port == DEFAULT_PANEL_PORT,
            "apply_env_file must never bind reserved website ports 80/443",
        )?;
        upsert_env_var(&env_file, "FERROUS_PORT", "8443")?;
        let deduped = std::fs::read_to_string(&env_file)?;
        ensure(
            deduped.matches("FERROUS_PORT=").count() == 1 && deduped.contains("FERROUS_PORT=8443"),
            "upsert_env_var must deduplicate FERROUS_PORT entries",
        )?;
        ensure(
            validate_panel_port(80).is_err() && validate_panel_port(443).is_err(),
            "validate_panel_port must reject ports 80 and 443",
        )?;

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}
