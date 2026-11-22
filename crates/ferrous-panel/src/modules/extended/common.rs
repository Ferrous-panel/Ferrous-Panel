pub(super) use crate::api::contract::{
    AntiCrashGuardState, BackendWatchdogState, Blake3ObjectSnapshot, DeployWebsiteRequest,
    DiagnosticCheckItem, EdgeCloudData, FerrousXLibDep, FerrousXLibraryCompatibility,
    FerrousXSyncCrate, FerrousXSyncState, KernelSysctlParam, MailDomainItem, MailForwarder,
    MailMessageItem, MailServerData, MailboxAccount, ManagedApp, OciContainerSandbox,
    OneClickDeployRequest, OneClickTemplate, OomImmunityProcessItem, OptimizationProfilePreset,
    PanelAdminAccount, PhpExtensionItem, PhpVersionInstance, PhpVersionTag, PolyglotCompileRequest,
    PolyglotCompileResult, PolyglotToolchainSpec, RustModuleConversionItem, RustSourceFile,
    ServerPackageItem, SshActiveSession, SshAuthorizedKey, SshDaemonConfig, SshGeneratedKeypair,
    SshPanelData, SystemOptimizerData, WasiEdgeWorker, WasiWorkerInvokeResult, WatchdogEventItem,
    WebSshCommandOutput, WebsiteRuntimeType, WireGuardPeerNode,
};
pub(super) use crate::auth::password::{
    load_or_init_credentials, update_admin_password, update_admin_username,
};
pub(super) use crate::auth::store::AuthStore;
pub(super) use crate::cli::upsert_env_var;
pub(super) use crate::config::Config;
pub(super) use crate::error::PanelError;
pub(super) use crate::modules::ModuleStore;
pub(super) use crate::system::cmd::AllowedBinary;
pub(super) use crate::system::paths::SafePath;
pub(super) use crate::system::validate::{
    is_reserved_website_port, validate_panel_port, Domain, DEFAULT_PANEL_PORT,
};
pub(super) use crate::webserver::{
    atomic_write_file, CommandExecutor, PhpFpmVersion, SafeFilename, WebServerEngine,
    WebServerManager,
};
pub(super) use chrono::Utc;
use sha2::{Digest, Sha256};
pub(super) use std::collections::BTreeMap;
pub(super) use std::path::{Path, PathBuf};
pub(super) use std::time::{Duration, Instant};

pub(super) const CMD_TIMEOUT: Duration = Duration::from_secs(15);
pub(super) const APT_TIMEOUT: Duration = Duration::from_secs(180);

pub(super) const KV_PHP_VERSIONS: &str = "extended.php_versions";
pub(super) const KV_SOFTWARE_PKGS: &str = "extended.software_packages";
pub(super) const KV_MAIL_SERVER: &str = "extended.mail_server";
pub(super) const KV_OPTIMIZER: &str = "extended.system_optimizer";
pub(super) const KV_SSH_PANEL: &str = "extended.ssh_panel";
pub(super) const KV_PANEL_ADMIN: &str = "extended.panel_admin";
pub(super) const KV_EDGE_CLOUD: &str = "extended.edge_cloud";

pub(super) fn sha256_hex(input: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input);
    format!("{:x}", hasher.finalize())
}

pub(super) fn is_production_vps(config: &Config) -> bool {
    config.data_dir == Path::new("/var/lib/ferrous-panel")
}

pub(super) fn read_env_file_value(path: &Path, key: &str) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            if k.trim() == key {
                let val = v.trim().trim_matches('"').trim_matches('\'').trim();
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

pub(super) fn detect_vps_server_host(config: &Config) -> String {
    if let Some(ref dom) = config.custom_domain {
        let trimmed = dom.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let env_file = resolve_panel_env_path(config);
    if let Some(dom) = read_env_file_value(&env_file, "FERROUS_CUSTOM_DOMAIN") {
        return dom;
    }
    for env_key in ["FERROUS_PUBLIC_IP", "FERROUSX_SERVER_IP"] {
        if let Ok(val) = std::env::var(env_key) {
            let trimmed = val.trim();
            if !trimmed.is_empty()
                && trimmed != "127.0.0.1"
                && trimmed != "0.0.0.0"
                && !trimmed.eq_ignore_ascii_case("localhost")
            {
                return trimmed.to_string();
            }
        }
    }
    for (file_path, key) in [
        (env_file.as_path(), "FERROUS_PUBLIC_IP"),
        (Path::new("/etc/ferrousx/ferrousx.env"), "FERROUSX_SERVER_IP"),
    ] {
        if let Some(val) = read_env_file_value(file_path, key) {
            if val != "127.0.0.1" && val != "0.0.0.0" && !val.eq_ignore_ascii_case("localhost") {
                return val;
            }
        }
    }
    let nets = sysinfo::Networks::new_with_refreshed_list();
    let mut fallback_v4: Option<String> = None;
    for (iface_name, data) in &nets {
        let lower = iface_name.to_ascii_lowercase();
        let is_virtual = lower == "lo"
            || lower.starts_with("docker")
            || lower.starts_with("br-")
            || lower.starts_with("veth")
            || lower.starts_with("cni")
            || lower.starts_with("virbr")
            || lower.starts_with("wg")
            || lower.starts_with("tailscale");
        if is_virtual {
            continue;
        }
        for net in data.ip_networks() {
            if let std::net::IpAddr::V4(v4) = net.addr {
                if v4.is_loopback() || v4.is_unspecified() || v4.is_link_local() {
                    continue;
                }
                if !v4.is_private() {
                    return v4.to_string();
                }
                if fallback_v4.is_none() {
                    fallback_v4 = Some(v4.to_string());
                }
            }
        }
    }
    fallback_v4.unwrap_or_else(|| "127.0.0.1".to_string())
}

pub(super) fn read_proc_mem_available_kb() -> Option<u64> {
    let raw = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("MemAvailable:") {
            return rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<u64>().ok());
        }
    }
    None
}

pub(super) fn read_proc_sysctl(key: &str) -> Option<String> {
    if key.is_empty()
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
    {
        return None;
    }
    let rel = key.replace('.', "/");
    let path = Path::new("/proc/sys").join(rel);
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub(super) fn is_safe_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/' | b':' | b',' | b'@' | b'=' | b'+'))
}

pub(super) fn read_process_rss_mb() -> f64 {
    let Ok(statm) = std::fs::read_to_string("/proc/self/statm") else {
        return 14.8;
    };
    let pages = statm
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(3800);
    let mb = (pages.saturating_mul(4096) as f64) / (1024.0 * 1024.0);
    ((mb * 100.0).round() / 100.0).max(1.0)
}

pub(super) fn resolve_local_domain(config: Option<&Config>) -> String {
    if let Some(custom) = config.and_then(|c| c.custom_domain.as_deref()) {
        if let Ok(parsed) = Domain::parse(custom) {
            return parsed.as_str().to_string();
        }
    }
    if let Ok(hostname) = std::fs::read_to_string("/etc/hostname") {
        let trimmed = hostname.trim();
        if let Ok(parsed) = Domain::parse(trimmed) {
            return parsed.as_str().to_string();
        }
    }
    "vps.local".to_string()
}


pub(super) fn resolve_panel_env_path(config: &Config) -> PathBuf {
    if let Ok(explicit) = std::env::var("FERROUS_ENV_FILE") {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if config.data_dir == Path::new("/var/lib/ferrous-panel") {
        PathBuf::from("/etc/ferrous-panel/panel.env")
    } else {
        config.data_dir.join("panel.env")
    }
}

