use crate::api::contract::TelemetryHistoryPoint;
use crate::auth::password::{load_or_init_credentials, AdminCredentialsFile, RedactedSecret};
use crate::auth::rate_limit::LoginRateLimiter;
use crate::auth::store::AuthStore;
use crate::config::Config;
use crate::error::PanelError;
use crate::modules::ModuleStore;
use crate::system::cmd::AllowedBinary;
use crate::webserver::{CommandExecutor, WebServerManager};
use chrono::{Local, Utc};
use serde::Serialize;
use std::collections::HashSet;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};
use std::time::Instant;
use sysinfo::{Disks, Networks, System};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreLoadDto {
    pub core: usize,
    pub model: String,
    pub usage_percent: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySnapshot {
    pub hostname: String,
    pub public_ip: String,
    pub os_version: String,
    pub kernel_version: String,
    pub rust_daemon_version: String,
    pub nginx_version: String,
    pub uptime_seconds: u64,
    pub cpu_brand: String,
    pub cpu_cores: usize,
    pub cpu_usage_percent: f64,
    pub core_loads: Vec<CoreLoadDto>,
    pub load_average: [f64; 3],
    pub memory_total_mb: u64,
    pub memory_used_mb: u64,
    pub memory_percent: f64,
    pub swap_total_mb: u64,
    pub swap_used_mb: u64,
    pub disk_total_gb: f64,
    pub disk_used_gb: f64,
    pub network_rx_mbps: f64,
    pub network_tx_mbps: f64,
    pub total_requests_per_sec: u64,
    pub active_connections: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_24h: Option<Vec<TelemetryHistoryPoint>>,
    pub timestamp: String,
}

#[derive(Debug)]
struct TelemetryInner {
    system: System,
    networks: Networks,
    last_sample: Instant,
    last_req_count: u64,
}

/// Real host telemetry collector backed by `sysinfo` (zero fabricated metrics).
#[derive(Debug)]
pub struct TelemetryCollector {
    inner: Mutex<TelemetryInner>,
    total_requests: AtomicU64,
    active_connections: AtomicU64,
    nginx_version: RwLock<String>,
}

impl TelemetryCollector {
    #[must_use]
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_cpu_usage();
        system.refresh_memory();
        let networks = Networks::new_with_refreshed_list();
        let initial_nginx = if AllowedBinary::Nginx.is_available() {
            "installed".to_string()
        } else {
            "not installed".to_string()
        };
        Self {
            inner: Mutex::new(TelemetryInner {
                system,
                networks,
                last_sample: Instant::now(),
                last_req_count: 0,
            }),
            total_requests: AtomicU64::new(0),
            active_connections: AtomicU64::new(0),
            nginx_version: RwLock::new(initial_nginx),
        }
    }

    /// Asynchronously probes the Nginx version via the allowlisted `CommandExecutor` (never blocking).
    pub async fn refresh_nginx_version(&self, executor: &dyn CommandExecutor) {
        if !executor.is_binary_available(AllowedBinary::Nginx) {
            if let Ok(mut guard) = self.nginx_version.write() {
                *guard = "not installed".to_string();
            }
            return;
        }
        if let Ok(out) = executor
            .run(
                AllowedBinary::Nginx,
                &["-v"],
                std::time::Duration::from_secs(3),
            )
            .await
        {
            let raw = if !out.stderr.is_empty() {
                out.stderr
            } else {
                out.stdout
            };
            let parsed = raw
                .lines()
                .next()
                .map(str::trim)
                .and_then(|line| line.strip_prefix("nginx version:"))
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| "installed".to_string());
            if let Ok(mut guard) = self.nginx_version.write() {
                *guard = parsed;
            }
        }
    }

    pub fn record_request(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    #[must_use]
    pub fn connection_guard(&self) -> ConnectionGuard<'_> {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
        ConnectionGuard {
            counter: &self.active_connections,
        }
    }

    pub fn snapshot(&self, config: &Config) -> Result<TelemetrySnapshot, PanelError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| PanelError::Internal("telemetry state unavailable".to_string()))?;

        let elapsed_secs = guard.last_sample.elapsed().as_secs_f64();
        guard.last_sample = Instant::now();

        guard.system.refresh_cpu_usage();
        guard.system.refresh_memory();
        guard.networks.refresh(true);

        let hostname = System::host_name().unwrap_or_else(|| "unknown".to_string());
        let os_version = System::long_os_version().unwrap_or_else(|| "Linux".to_string());
        let kernel_version = System::kernel_version().unwrap_or_else(|| "unknown".to_string());
        let uptime_seconds = System::uptime();
        let load_average = read_proc_loadavg().unwrap_or_else(|| {
            let load = System::load_average();
            [round2(load.one), round2(load.five), round2(load.fifteen)]
        });

        let cpus = guard.system.cpus();
        let cpu_cores = cpus.len();
        let cpu_brand = cpus
            .first()
            .map(|c| c.brand().trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        let cpu_usage_percent = round1(f64::from(guard.system.global_cpu_usage()));
        let core_loads = cpus
            .iter()
            .enumerate()
            .map(|(core, cpu)| CoreLoadDto {
                core,
                model: cpu.brand().trim().to_string(),
                usage_percent: round1(f64::from(cpu.cpu_usage())),
            })
            .collect();

        let memory_total_mb = guard.system.total_memory() / (1024 * 1024);
        let memory_used_mb = guard.system.used_memory() / (1024 * 1024);
        let memory_percent = if memory_total_mb > 0 {
            round1((memory_used_mb as f64 / memory_total_mb as f64) * 100.0)
        } else {
            0.0
        };
        let swap_total_mb = guard.system.total_swap() / (1024 * 1024);
        let swap_used_mb = guard.system.used_swap() / (1024 * 1024);

        let disks = Disks::new_with_refreshed_list();
        let (disk_total_bytes, disk_used_bytes) = collect_disk_bytes(&disks);
        let disk_total_gb = round2(disk_total_bytes as f64 / 1_073_741_824.0);
        let disk_used_gb = round2(disk_used_bytes as f64 / 1_073_741_824.0);

        let mut rx_bytes: u64 = 0;
        let mut tx_bytes: u64 = 0;
        let mut public_v4: Option<String> = None;
        let mut fallback_v4: Option<String> = None;

        for (iface, data) in &guard.networks {
            if iface == "lo" {
                continue;
            }
            rx_bytes = rx_bytes.saturating_add(data.received());
            tx_bytes = tx_bytes.saturating_add(data.transmitted());
            let is_virtual = iface.starts_with("docker")
                || iface.starts_with("br-")
                || iface.starts_with("veth")
                || iface.starts_with("cni")
                || iface.starts_with("virbr")
                || iface.starts_with("wg")
                || iface.starts_with("tailscale");
            if !is_virtual {
                for net in data.ip_networks() {
                    if let IpAddr::V4(v4) = net.addr {
                        if !v4.is_loopback() && !v4.is_unspecified() && !v4.is_link_local() {
                            if !v4.is_private() && public_v4.is_none() {
                                public_v4 = Some(v4.to_string());
                            } else if fallback_v4.is_none() {
                                fallback_v4 = Some(v4.to_string());
                            }
                        }
                    }
                }
            }
        }

        let (network_rx_mbps, network_tx_mbps) = if elapsed_secs > 0.05 {
            let rx_mbps = (rx_bytes as f64 * 8.0) / (elapsed_secs * 1_000_000.0);
            let tx_mbps = (tx_bytes as f64 * 8.0) / (elapsed_secs * 1_000_000.0);
            (round2(rx_mbps), round2(tx_mbps))
        } else {
            (0.0, 0.0)
        };

        let cur_total_reqs = self.total_requests.load(Ordering::Relaxed);
        let delta_reqs = cur_total_reqs.saturating_sub(guard.last_req_count);
        guard.last_req_count = cur_total_reqs;
        let total_requests_per_sec = if elapsed_secs >= 1.0 {
            ((delta_reqs as f64) / elapsed_secs).round() as u64
        } else {
            delta_reqs
        };

        let env_public_ip = ["FERROUS_PUBLIC_IP", "FERROUSX_SERVER_IP"]
            .into_iter()
            .find_map(|k| std::env::var(k).ok())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty() && v != "127.0.0.1" && v != "0.0.0.0" && v != "localhost")
            .or_else(|| {
                std::fs::read_to_string("/etc/ferrousx/ferrousx.env")
                    .ok()
                    .and_then(|raw| {
                        raw.lines().find_map(|line| {
                            line.trim()
                                .strip_prefix("FERROUSX_SERVER_IP=")
                                .map(|v| v.trim().trim_matches('"').to_string())
                                .filter(|v| !v.is_empty() && v != "127.0.0.1" && v != "0.0.0.0")
                        })
                    })
            });

        let public_ip = env_public_ip
            .or(public_v4)
            .or(fallback_v4)
            .unwrap_or_else(|| config.bind.clone());
        let nginx_version = self
            .nginx_version
            .read()
            .map(|g| g.clone())
            .unwrap_or_else(|_| {
                if AllowedBinary::Nginx.is_available() {
                    "installed".to_string()
                } else {
                    "not installed".to_string()
                }
            });

        Ok(TelemetrySnapshot {
            hostname,
            public_ip,
            os_version,
            kernel_version,
            rust_daemon_version: format!("ferrous-panel v{}", env!("CARGO_PKG_VERSION")),
            nginx_version,
            uptime_seconds,
            cpu_brand,
            cpu_cores,
            cpu_usage_percent,
            core_loads,
            load_average,
            memory_total_mb,
            memory_used_mb,
            memory_percent,
            swap_total_mb,
            swap_used_mb,
            disk_total_gb,
            disk_used_gb,
            network_rx_mbps,
            network_tx_mbps,
            total_requests_per_sec,
            active_connections: self.active_connections.load(Ordering::Relaxed),
            history_24h: None,
            timestamp: Utc::now().to_rfc3339(),
        })
    }
}

impl Default for TelemetryCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct ConnectionGuard<'a> {
    counter: &'a AtomicU64,
}

impl Drop for ConnectionGuard<'_> {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Relaxed);
    }
}

fn collect_disk_bytes(disks: &Disks) -> (u64, u64) {
    let mut seen_devices: HashSet<String> = HashSet::new();
    let mut total: u64 = 0;
    let mut used: u64 = 0;

    for disk in disks {
        let fs = disk.file_system().to_string_lossy();
        if matches!(
            fs.as_ref(),
            "tmpfs" | "devtmpfs" | "squashfs" | "proc" | "sysfs" | "cgroup" | "cgroup2"
        ) {
            continue;
        }
        let dev_name = disk.name().to_string_lossy().to_string();
        if !seen_devices.insert(dev_name) {
            continue;
        }
        let d_total = disk.total_space();
        let d_used = d_total.saturating_sub(disk.available_space());
        total = total.saturating_add(d_total);
        used = used.saturating_add(d_used);
    }

    if total == 0 {
        for disk in disks {
            if disk.mount_point() == std::path::Path::new("/") {
                let d_total = disk.total_space();
                let d_used = d_total.saturating_sub(disk.available_space());
                return (d_total, d_used);
            }
        }
    }

    (total, used)
}



fn round1(v: f64) -> f64 {
    if v.is_finite() {
        (v * 10.0).round() / 10.0
    } else {
        0.0
    }
}

fn round2(v: f64) -> f64 {
    if v.is_finite() {
        (v * 100.0).round() / 100.0
    } else {
        0.0
    }
}

fn read_proc_loadavg() -> Option<[f64; 3]> {
    let raw = std::fs::read_to_string("/proc/loadavg").ok()?;
    let mut parts = raw.split_whitespace();
    let one = parts.next()?.parse::<f64>().ok()?;
    let five = parts.next()?.parse::<f64>().ok()?;
    let fifteen = parts.next()?.parse::<f64>().ok()?;
    Some([round2(one), round2(five), round2(fifteen)])
}

/// Shared application state injected into Actix-Web workers.
#[derive(Debug)]
pub struct AppState {
    pub config: Config,
    pub credentials: RwLock<AdminCredentialsFile>,
    pub auth_store: AuthStore,
    pub rate_limiter: LoginRateLimiter,
    pub telemetry: TelemetryCollector,
    pub webserver: WebServerManager,
    pub modules: ModuleStore,
    last_history_sample_ts: Mutex<Option<i64>>,
}

impl AppState {
    /// Initializes SQLite auth/audit storage, credentials (`0600`), telemetry collector, web server manager, and P5 module store.
    pub fn init(config: Config) -> Result<(Self, Option<RedactedSecret>), PanelError> {
        let (creds, initial_password) =
            load_or_init_credentials(&config.credentials_path(), &config.admin_username)?;
        let auth_store = AuthStore::open(&config.db_path())?;
        let modules = ModuleStore::open(&config.db_path())?;
        let hook_exe = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("/usr/local/bin/ferrous-panel"));
        let webserver = WebServerManager::open_system(&config.db_path(), hook_exe)?;
        let state = Self {
            config,
            credentials: RwLock::new(creds),
            auth_store,
            rate_limiter: LoginRateLimiter::new(),
            telemetry: TelemetryCollector::new(),
            webserver,
            modules,
            last_history_sample_ts: Mutex::new(None),
        };
        Ok((state, initial_password))
    }

    /// Initializes `AppState` with a custom `WebServerManager` (for isolated tests).
    pub fn init_with_webserver(
        config: Config,
        webserver: WebServerManager,
    ) -> Result<(Self, Option<RedactedSecret>), PanelError> {
        let (creds, initial_password) =
            load_or_init_credentials(&config.credentials_path(), &config.admin_username)?;
        let auth_store = AuthStore::open(&config.db_path())?;
        let modules = ModuleStore::open(&config.db_path())?;
        if config.is_production() {
            crate::modules::default_pages::ensure_system_default_landing_pages(&config.wwwroot);
        }
        let state = Self {
            config,
            credentials: RwLock::new(creds),
            auth_store,
            rate_limiter: LoginRateLimiter::new(),
            telemetry: TelemetryCollector::new(),
            webserver,
            modules,
            last_history_sample_ts: Mutex::new(None),
        };
        Ok((state, initial_password))
    }

    /// Samples real host telemetry into SQLite `telemetry_history` (at most once per 60s unless `force = true`).
    pub fn record_telemetry_sample(&self, force: bool) -> Result<(), PanelError> {
        let now_ts = Utc::now().timestamp();
        {
            let guard = self
                .last_history_sample_ts
                .lock()
                .map_err(|_| PanelError::Internal("telemetry history lock unavailable".to_string()))?;
            if !force {
                if let Some(prev) = *guard {
                    if now_ts.saturating_sub(prev) < 60 {
                        return Ok(());
                    }
                }
            }
        }

        let snap = self.telemetry.snapshot(&self.config)?;
        let point = TelemetryHistoryPoint {
            time: Local::now().format("%H:%M").to_string(),
            cpu_percent: snap.cpu_usage_percent,
            memory_percent: snap.memory_percent,
            memory_used_mb: snap.memory_used_mb,
            requests_per_sec: Some(snap.total_requests_per_sec),
        };
        self.modules.insert_telemetry_point(&point)?;

        let mut guard = self
            .last_history_sample_ts
            .lock()
            .map_err(|_| PanelError::Internal("telemetry history lock unavailable".to_string()))?;
        *guard = Some(now_ts);
        Ok(())
    }

    /// Returns current real host telemetry plus persisted real SQLite history points.
    pub fn snapshot_with_real_history(&self) -> Result<TelemetrySnapshot, PanelError> {
        let _ = self.record_telemetry_sample(false);
        let mut snap = self.telemetry.snapshot(&self.config)?;
        let history = self.modules.load_telemetry_history()?;
        if !history.is_empty() {
            snap.history_24h = Some(history);
        }
        Ok(snap)
    }
}
