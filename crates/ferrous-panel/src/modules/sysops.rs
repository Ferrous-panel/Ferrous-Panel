use crate::error::PanelError;
use crate::system::cmd::AllowedBinary;
use crate::webserver::{atomic_write_file, CommandExecutor};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs::OpenOptions;
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::Duration;

const CMD_TIMEOUT: Duration = Duration::from_secs(15);
const SWAP_CMD_TIMEOUT: Duration = Duration::from_secs(60);
const SWAPFILE_PATH: &str = "/swapfile";
const FSTAB_PATH: &str = "/etc/fstab";
const SYSCTL_DROPIN_PATH: &str = "/etc/sysctl.d/99-ferrous-sysops.conf";

const PROTECTED_PROCESS_NAMES: &[&str] = &[
    "systemd",
    "init",
    "sshd",
    "ferrous-panel",
    "systemd-journal",
    "systemd-journald",
    "systemd-udevd",
    "systemd-logind",
    "dbus-daemon",
    "dbus-broker",
];

const PROTECTED_SYSTEMD_UNITS: &[&str] = &[
    "sshd.service",
    "ssh.service",
    "ferrous-panel.service",
    "systemd-journald.service",
    "systemd-udevd.service",
    "systemd-logind.service",
    "dbus.service",
    "dbus-broker.service",
    "networking.service",
    "systemd-networkd.service",
];

// ============================================================================
// Response & Request Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostDetails {
    pub hostname: String,
    pub kernel: String,
    pub os_pretty: String,
    pub arch: String,
    pub timezone: String,
    pub uptime_secs: u64,
    pub load_avg: [f64; 3],
    pub cpu_cores: usize,
    pub mem_total_mb: u64,
    pub mem_available_mb: u64,
    pub swap_total_mb: u64,
    pub swap_used_mb: u64,
    pub swap_file_active: bool,
    pub swap_file_size_mb: u64,
    pub panel_pid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SysctlTuning {
    pub swappiness: u32,
    pub somaxconn: u32,
    pub tcp_congestion_control: String,
    pub available_congestion_controls: Vec<String>,
    pub file_max: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessItem {
    pub pid: u32,
    pub ppid: u32,
    pub user: String,
    pub name: String,
    pub state: String,
    pub rss_mb: f64,
    pub threads: u32,
    pub command: String,
    pub protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListeningPortItem {
    pub protocol: String,
    pub local_addr: String,
    pub port: u16,
    pub process: String,
    pub pid: Option<u32>,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskMountItem {
    pub device: String,
    pub fs_type: String,
    pub mount_point: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub use_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemdServiceItem {
    pub unit: String,
    pub load_state: String,
    pub active_state: String,
    pub sub_state: String,
    pub description: String,
    pub protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SysopsOverview {
    pub host: HostDetails,
    pub sysctl: SysctlTuning,
    pub processes: Vec<ProcessItem>,
    pub ports: Vec<ListeningPortItem>,
    pub disks: Vec<DiskMountItem>,
    pub services: Vec<SystemdServiceItem>,
    pub collected_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceLogsResponse {
    pub unit: String,
    pub lines: usize,
    pub content: String,
    pub fetched_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SysctlUpdatePatch {
    #[serde(default)]
    pub swappiness: Option<u32>,
    #[serde(default)]
    pub somaxconn: Option<u32>,
    #[serde(default)]
    pub tcp_congestion_control: Option<String>,
    #[serde(default)]
    pub file_max: Option<u64>,
}

// ============================================================================
// Validation & Guard Helpers
// ============================================================================

/// Validates a systemd unit name (`[A-Za-z0-9@_.-]`, supports template units like `php8.3-fpm` or `getty@tty1`).
pub fn validate_systemd_unit(raw: &str) -> Result<String, PanelError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 80 {
        return Err(PanelError::Validation(
            "Tên dịch vụ systemd phải dài từ 1 đến 80 ký tự.".to_string(),
        ));
    }
    if trimmed.starts_with('-')
        || trimmed.contains("..")
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains('\0')
    {
        return Err(PanelError::Forbidden(
            "Tên dịch vụ systemd chứa ký tự không hợp lệ.".to_string(),
        ));
    }
    if !trimmed
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'@' | b'_' | b'.' | b'-'))
    {
        return Err(PanelError::Validation(
            "Tên dịch vụ systemd chỉ được chứa [A-Za-z0-9@_.-].".to_string(),
        ));
    }
    if trimmed.ends_with(".service") {
        Ok(trimmed.to_string())
    } else {
        Ok(format!("{trimmed}.service"))
    }
}

fn is_protected_unit(unit: &str) -> bool {
    let lower = unit.to_ascii_lowercase();
    PROTECTED_SYSTEMD_UNITS.iter().any(|p| *p == lower)
}

fn is_protected_process(pid: u32, ppid: u32, name: &str, panel_pid: u32) -> bool {
    if pid <= 2 || pid == panel_pid || ppid == 2 {
        return true;
    }
    let lower = name.to_ascii_lowercase();
    PROTECTED_PROCESS_NAMES.iter().any(|p| *p == lower)
}

// ============================================================================
// Pure-Rust /proc & Host Telemetry Readers
// ============================================================================

fn read_trimmed(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn read_os_pretty_name() -> String {
    if let Ok(raw) = std::fs::read_to_string("/etc/os-release") {
        for line in raw.lines() {
            if let Some(val) = line.strip_prefix("PRETTY_NAME=") {
                let clean = val.trim().trim_matches('"').trim_matches('\'');
                if !clean.is_empty() {
                    return clean.to_string();
                }
            }
        }
    }
    "Linux".to_string()
}

fn read_timezone() -> String {
    if let Some(tz) = read_trimmed("/etc/timezone") {
        return tz;
    }
    if let Ok(target) = std::fs::read_link("/etc/localtime") {
        let s = target.display().to_string();
        if let Some(idx) = s.find("zoneinfo/") {
            return s[idx + 9..].to_string();
        }
    }
    "UTC".to_string()
}

fn read_kernel_release() -> String {
    if let Some(rel) = read_trimmed("/proc/sys/kernel/osrelease") {
        return rel;
    }
    if let Some(ver) = read_trimmed("/proc/version") {
        return ver
            .split_whitespace()
            .nth(2)
            .unwrap_or("unknown")
            .to_string();
    }
    "unknown".to_string()
}

fn read_uptime_secs() -> u64 {
    read_trimmed("/proc/uptime")
        .and_then(|s| s.split_whitespace().next().map(str::to_string))
        .and_then(|first| first.parse::<f64>().ok())
        .map(|v| v.max(0.0) as u64)
        .unwrap_or(0)
}

fn read_load_avg() -> [f64; 3] {
    let mut out = [0.0, 0.0, 0.0];
    if let Some(raw) = read_trimmed("/proc/loadavg") {
        let parts: Vec<&str> = raw.split_whitespace().collect();
        for i in 0..3 {
            if let Some(tok) = parts.get(i) {
                if let Ok(v) = tok.parse::<f64>() {
                    out[i] = (v * 100.0).round() / 100.0;
                }
            }
        }
    }
    out
}

fn read_mem_and_swap_mb() -> (u64, u64, u64, u64) {
    let mut mem_total_kb = 0u64;
    let mut mem_avail_kb = 0u64;
    let mut swap_total_kb = 0u64;
    let mut swap_free_kb = 0u64;

    if let Ok(raw) = std::fs::read_to_string("/proc/meminfo") {
        for line in raw.lines() {
            let mut parts = line.split_whitespace();
            let key = parts.next().unwrap_or("");
            let val = parts
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            match key {
                "MemTotal:" => mem_total_kb = val,
                "MemAvailable:" => mem_avail_kb = val,
                "SwapTotal:" => swap_total_kb = val,
                "SwapFree:" => swap_free_kb = val,
                _ => {}
            }
        }
    }

    let swap_used_kb = swap_total_kb.saturating_sub(swap_free_kb);
    (
        mem_total_kb / 1024,
        mem_avail_kb / 1024,
        swap_total_kb / 1024,
        swap_used_kb / 1024,
    )
}

fn detect_swapfile_status() -> (bool, u64) {
    let mut active = false;
    let mut size_mb = 0u64;
    if let Ok(raw) = std::fs::read_to_string("/proc/swaps") {
        for line in raw.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.first().copied() == Some(SWAPFILE_PATH) {
                active = true;
                if let Some(kb) = parts.get(2).and_then(|s| s.parse::<u64>().ok()) {
                    size_mb = kb / 1024;
                }
                break;
            }
        }
    }
    if size_mb == 0 {
        if let Ok(meta) = std::fs::metadata(SWAPFILE_PATH) {
            size_mb = meta.len() / (1024 * 1024);
        }
    }
    (active, size_mb)
}

fn read_sysctl_tuning() -> SysctlTuning {
    let swappiness = read_trimmed("/proc/sys/vm/swappiness")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(60);
    let somaxconn = read_trimmed("/proc/sys/net/core/somaxconn")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(4096);
    let tcp_congestion_control =
        read_trimmed("/proc/sys/net/ipv4/tcp_congestion_control").unwrap_or_else(|| "cubic".into());
    let available_congestion_controls =
        read_trimmed("/proc/sys/net/ipv4/tcp_available_congestion_control")
            .map(|s| s.split_whitespace().map(str::to_string).collect())
            .unwrap_or_else(|| vec!["cubic".to_string(), "reno".to_string()]);
    let file_max = read_trimmed("/proc/sys/fs/file-max")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1_048_576);

    SysctlTuning {
        swappiness,
        somaxconn,
        tcp_congestion_control,
        available_congestion_controls,
        file_max,
    }
}

fn load_uid_map() -> HashMap<u32, String> {
    let mut map = HashMap::new();
    if let Ok(raw) = std::fs::read_to_string("/etc/passwd") {
        for line in raw.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 3 {
                if let Ok(uid) = parts[2].parse::<u32>() {
                    map.entry(uid).or_insert_with(|| parts[0].to_string());
                }
            }
        }
    }
    map
}

/// Parses `/proc/<pid>/stat` safely even when process `comm` contains spaces or parentheses.
#[must_use]
pub fn parse_proc_stat_fields(stat: &str) -> Option<(String, String, u32)> {
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    if close <= open {
        return None;
    }
    let name = stat[open + 1..close].to_string();
    let rest = stat.get(close + 2..)?;
    let mut parts = rest.split_whitespace();
    let state = parts.next()?.to_string();
    let ppid = parts.next()?.parse::<u32>().ok()?;
    Some((name, state, ppid))
}

fn list_processes_sync() -> Vec<ProcessItem> {
    let panel_pid = std::process::id();
    let uid_map = load_uid_map();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };

    let mut procs = Vec::new();
    for entry in entries.flatten() {
        let fname = entry.file_name();
        let Some(pid_str) = fname.to_str() else {
            continue;
        };
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };

        let pdir = entry.path();
        let Ok(stat_raw) = std::fs::read_to_string(pdir.join("stat")) else {
            continue;
        };
        let Some((name, state, ppid)) = parse_proc_stat_fields(&stat_raw) else {
            continue;
        };

        let mut uid: Option<u32> = None;
        let mut rss_kb: u64 = 0;
        let mut threads: u32 = 1;
        if let Ok(status_raw) = std::fs::read_to_string(pdir.join("status")) {
            for line in status_raw.lines() {
                if let Some(rest) = line.strip_prefix("Uid:") {
                    uid = rest.split_whitespace().next().and_then(|s| s.parse::<u32>().ok());
                } else if let Some(rest) = line.strip_prefix("VmRSS:") {
                    rss_kb = rest
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                } else if let Some(rest) = line.strip_prefix("Threads:") {
                    threads = rest
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse::<u32>().ok())
                        .unwrap_or(1);
                }
            }
        }

        let cmdline_bytes = std::fs::read(pdir.join("cmdline")).unwrap_or_default();
        let command = if cmdline_bytes.is_empty() {
            format!("[{name}]")
        } else {
            let decoded = cmdline_bytes
                .split(|&b| b == 0)
                .filter(|seg| !seg.is_empty())
                .map(|seg| String::from_utf8_lossy(seg).into_owned())
                .collect::<Vec<_>>()
                .join(" ");
            let trimmed = decoded.trim();
            if trimmed.is_empty() {
                format!("[{name}]")
            } else {
                trimmed.chars().take(180).collect()
            }
        };

        // Hide pure kernel threads with 0 RSS so users see real user-space workloads clearly.
        if rss_kb == 0 && (ppid == 2 || pid == 2) {
            continue;
        }

        let user = uid
            .and_then(|u| uid_map.get(&u).cloned().or_else(|| Some(u.to_string())))
            .unwrap_or_else(|| "root".to_string());
        let rss_mb = ((rss_kb as f64 / 1024.0) * 10.0).round() / 10.0;
        let protected = is_protected_process(pid, ppid, &name, panel_pid);

        procs.push(ProcessItem {
            pid,
            ppid,
            user,
            name,
            state,
            rss_mb,
            threads,
            command,
            protected,
        });
    }

    procs.sort_by(|a, b| {
        b.rss_mb
            .partial_cmp(&a.rss_mb)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.pid.cmp(&b.pid))
    });
    procs.truncate(60);
    procs
}

// ============================================================================
// Parsers for `ss`, `df`, and `systemctl list-units`
// ============================================================================

/// Parses `ss -tulpnH` output into structured listening socket items.
#[must_use]
pub fn parse_ss_listening_ports(stdout: &str) -> Vec<ListeningPortItem> {
    let mut items = Vec::new();
    for raw_line in stdout.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }
        let protocol = parts[0].to_ascii_lowercase();
        let state = parts[1].to_string();
        let local_raw = parts[4];

        let (local_addr, port) = match local_raw.rsplit_once(':') {
            Some((addr, port_s)) => match port_s.parse::<u16>() {
                Ok(p) => (addr.to_string(), p),
                Err(_) => continue,
            },
            None => continue,
        };

        let tail = if parts.len() > 6 {
            parts[6..].join(" ")
        } else {
            String::new()
        };
        let (process, pid) = parse_ss_users_field(&tail);

        items.push(ListeningPortItem {
            protocol,
            local_addr,
            port,
            process,
            pid,
            state,
        });
    }
    items.sort_by_key(|p| (p.port, p.protocol.clone(), p.local_addr.clone()));
    items
}

fn parse_ss_users_field(tail: &str) -> (String, Option<u32>) {
    let mut proc_name = String::new();
    let mut pid = None;

    if let Some(first_q) = tail.find('"') {
        if let Some(end_q) = tail[first_q + 1..].find('"') {
            proc_name = tail[first_q + 1..first_q + 1 + end_q].to_string();
        }
    }
    if let Some(idx) = tail.find("pid=") {
        let digits: String = tail[idx + 4..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        pid = digits.parse::<u32>().ok();
    }
    if proc_name.is_empty() {
        proc_name = "system".to_string();
    }
    (proc_name, pid)
}

/// Parses `df -PT -B1` output into real filesystem mount usage entries.
#[must_use]
pub fn parse_df_mounts(stdout: &str) -> Vec<DiskMountItem> {
    let mut out = Vec::new();
    for line in stdout.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 7 {
            continue;
        }
        let device = parts[0];
        let fs_type = parts[1];
        let mount_point = parts[6..].join(" ");

        if matches!(
            fs_type,
            "tmpfs" | "devtmpfs" | "squashfs" | "proc" | "sysfs" | "cgroup" | "cgroup2" | "efivarfs"
        ) {
            continue;
        }
        if fs_type == "overlay" && mount_point != "/" {
            continue;
        }

        let total_bytes = parts[2].parse::<u64>().unwrap_or(0);
        let used_bytes = parts[3].parse::<u64>().unwrap_or(0);
        let available_bytes = parts[4].parse::<u64>().unwrap_or(0);
        if total_bytes == 0 {
            continue;
        }
        let use_percent = ((used_bytes as f64 / total_bytes as f64) * 1000.0).round() / 10.0;

        out.push(DiskMountItem {
            device: device.to_string(),
            fs_type: fs_type.to_string(),
            mount_point,
            total_bytes,
            used_bytes,
            available_bytes,
            use_percent,
        });
    }
    out
}

/// Parses `systemctl list-units --type=service --all --no-pager --no-legend --plain`.
#[must_use]
pub fn parse_systemd_services(stdout: &str) -> Vec<SystemdServiceItem> {
    let mut services = Vec::new();
    for raw_line in stdout.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(unit) = parts.next() else { continue };
        if !unit.ends_with(".service") {
            continue;
        }
        let load_state = parts.next().unwrap_or("loaded").to_string();
        let active_state = parts.next().unwrap_or("unknown").to_string();
        let sub_state = parts.next().unwrap_or("unknown").to_string();
        let description = parts.collect::<Vec<_>>().join(" ");

        if load_state == "not-found" {
            continue;
        }

        let protected = is_protected_unit(unit);
        services.push(SystemdServiceItem {
            unit: unit.to_string(),
            load_state,
            active_state,
            sub_state,
            description,
            protected,
        });
    }

    services.sort_by(|a, b| {
        let priority = |s: &SystemdServiceItem| -> u8 {
            if s.unit.starts_with("ferrous")
                || s.unit.starts_with("nginx")
                || s.unit.starts_with("php")
                || s.unit.starts_with("mariadb")
                || s.unit.starts_with("mysql")
                || s.unit.starts_with("postgresql")
                || s.unit.starts_with("redis")
                || s.unit.starts_with("postfix")
                || s.unit.starts_with("dovecot")
                || s.unit.starts_with("fail2ban")
                || s.unit.starts_with("ssh")
            {
                0
            } else if s.active_state == "failed" {
                1
            } else if s.active_state == "active" {
                2
            } else {
                3
            }
        };
        priority(a)
            .cmp(&priority(b))
            .then_with(|| a.unit.cmp(&b.unit))
    });
    services.truncate(120);
    services
}

// ============================================================================
// Public Operations
// ============================================================================

pub async fn get_sysops_overview(
    executor: &dyn CommandExecutor,
) -> Result<SysopsOverview, PanelError> {
    let (host, sysctl, processes) = tokio::task::spawn_blocking(|| {
        let (mem_total_mb, mem_available_mb, swap_total_mb, swap_used_mb) = read_mem_and_swap_mb();
        let (swap_file_active, swap_file_size_mb) = detect_swapfile_status();
        let host = HostDetails {
            hostname: read_trimmed("/etc/hostname")
                .or_else(|| read_trimmed("/proc/sys/kernel/hostname"))
                .unwrap_or_else(|| "localhost".to_string()),
            kernel: read_kernel_release(),
            os_pretty: read_os_pretty_name(),
            arch: std::env::consts::ARCH.to_string(),
            timezone: read_timezone(),
            uptime_secs: read_uptime_secs(),
            load_avg: read_load_avg(),
            cpu_cores: std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1),
            mem_total_mb,
            mem_available_mb,
            swap_total_mb,
            swap_used_mb,
            swap_file_active,
            swap_file_size_mb,
            panel_pid: std::process::id(),
        };
        let sysctl = read_sysctl_tuning();
        let processes = list_processes_sync();
        (host, sysctl, processes)
    })
    .await
    .map_err(|_| PanelError::Internal("sysops telemetry worker cancelled".to_string()))?;

    let ports = if executor.is_binary_available(AllowedBinary::Ss) {
        executor
            .run(AllowedBinary::Ss, &["-tulpnH"], CMD_TIMEOUT)
            .await
            .map(|o| parse_ss_listening_ports(&o.stdout))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let disks = if executor.is_binary_available(AllowedBinary::Df) {
        executor
            .run(AllowedBinary::Df, &["-PT", "-B1"], CMD_TIMEOUT)
            .await
            .map(|o| parse_df_mounts(&o.stdout))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let services = if executor.is_binary_available(AllowedBinary::Systemctl) {
        executor
            .run(
                AllowedBinary::Systemctl,
                &[
                    "list-units",
                    "--type=service",
                    "--all",
                    "--no-pager",
                    "--no-legend",
                    "--plain",
                ],
                CMD_TIMEOUT,
            )
            .await
            .map(|o| parse_systemd_services(&o.stdout))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    Ok(SysopsOverview {
        host,
        sysctl,
        processes,
        ports,
        disks,
        services,
        collected_at: Utc::now().to_rfc3339(),
    })
}

pub async fn signal_process(
    executor: &dyn CommandExecutor,
    pid: u32,
    raw_signal: &str,
) -> Result<String, PanelError> {
    let sig = match raw_signal.trim().to_ascii_uppercase().as_str() {
        "TERM" | "SIGTERM" => "TERM",
        "KILL" | "SIGKILL" => "KILL",
        "HUP" | "SIGHUP" => "HUP",
        _ => {
            return Err(PanelError::Validation(
                "Tín hiệu tiến trình chỉ hỗ trợ TERM, KILL hoặc HUP.".to_string(),
            ))
        }
    };

    let panel_pid = std::process::id();
    if pid <= 2 || pid == panel_pid {
        return Err(PanelError::Forbidden(format!(
            "Anti-Lockout Guard: không được gửi tín hiệu tới tiến trình hệ thống cốt lõi (PID {pid})."
        )));
    }

    let stat_path = format!("/proc/{pid}/stat");
    if let Ok(stat_raw) = std::fs::read_to_string(&stat_path) {
        if let Some((name, _state, ppid)) = parse_proc_stat_fields(&stat_raw) {
            if is_protected_process(pid, ppid, &name, panel_pid) {
                return Err(PanelError::Forbidden(format!(
                    "Anti-Lockout Guard: tiến trình '{name}' (PID {pid}) đang được bảo vệ để tránh ngắt SSH/Panel."
                )));
            }
        }
    }

    if !executor.is_binary_available(AllowedBinary::Kill) {
        return Err(PanelError::BinaryUnavailable("kill"));
    }

    let pid_str = pid.to_string();
    let out = executor
        .run(AllowedBinary::Kill, &["-s", sig, &pid_str], CMD_TIMEOUT)
        .await?;
    if !out.success {
        return Err(PanelError::CommandFailed {
            binary: "kill",
            exit_code: out.exit_code,
            stderr: if out.stderr.is_empty() {
                format!("Không thể gửi SIG{sig} tới PID {pid}")
            } else {
                out.stderr
            },
        });
    }

    Ok(format!("Đã gửi tín hiệu SIG{sig} tới PID {pid}."))
}

pub async fn configure_swap(
    executor: &dyn CommandExecutor,
    size_mb: u64,
    swappiness: Option<u32>,
) -> Result<SysopsOverview, PanelError> {
    if !(128..=16_384).contains(&size_mb) {
        return Err(PanelError::Validation(
            "Dung lượng Swap phải từ 128 MB đến 16384 MB (16 GB).".to_string(),
        ));
    }
    if let Some(sw) = swappiness {
        if sw > 100 {
            return Err(PanelError::Validation(
                "vm.swappiness phải nằm trong khoảng 0..=100.".to_string(),
            ));
        }
    }
    for bin in [
        AllowedBinary::Mkswap,
        AllowedBinary::Swapon,
        AllowedBinary::Swapoff,
    ] {
        if !executor.is_binary_available(bin) {
            return Err(PanelError::BinaryUnavailable(bin.label()));
        }
    }

    // Check root filesystem headroom before allocating swapfile.
    if executor.is_binary_available(AllowedBinary::Df) {
        if let Ok(df_out) = executor
            .run(AllowedBinary::Df, &["-PT", "-B1", "/"], CMD_TIMEOUT)
            .await
        {
            if let Some(root_disk) = parse_df_mounts(&df_out.stdout).into_iter().next() {
                let needed = size_mb
                    .saturating_mul(1024 * 1024)
                    .saturating_add(256 * 1024 * 1024);
                let existing_swap = std::fs::metadata(SWAPFILE_PATH)
                    .map(|m| m.len())
                    .unwrap_or(0);
                if root_disk.available_bytes.saturating_add(existing_swap) < needed {
                    return Err(PanelError::Conflict(format!(
                        "Không đủ dung lượng trống trên ổ đĩa gốc '/' (cần {} MB + 256 MB dự phòng, hiện còn {} MB).",
                        size_mb,
                        root_disk.available_bytes / (1024 * 1024)
                    )));
                }
            }
        }
    }

    // Disable existing /swapfile if active.
    let (active_now, _) = detect_swapfile_status();
    if active_now {
        let off = executor
            .run(AllowedBinary::Swapoff, &[SWAPFILE_PATH], SWAP_CMD_TIMEOUT)
            .await?;
        if !off.success {
            return Err(PanelError::CommandFailed {
                binary: "swapoff",
                exit_code: off.exit_code,
                stderr: off.stderr,
            });
        }
    }

    // Pure-Rust zero-fill of /swapfile with strict 0600 permissions (no shell / dd).
    let target_mb = size_mb;
    tokio::task::spawn_blocking(move || -> Result<(), PanelError> {
        let p = Path::new(SWAPFILE_PATH);
        if p.exists() {
            std::fs::remove_file(p)?;
        }
        let mut f = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(p)?;
        let zero_chunk = vec![0u8; 1024 * 1024];
        for _ in 0..target_mb {
            f.write_all(&zero_chunk)?;
        }
        f.sync_all()?;
        Ok(())
    })
    .await
    .map_err(|_| PanelError::Internal("swapfile allocation worker cancelled".to_string()))??;

    let mk = executor
        .run(AllowedBinary::Mkswap, &[SWAPFILE_PATH], SWAP_CMD_TIMEOUT)
        .await?;
    if !mk.success {
        let _ = std::fs::remove_file(SWAPFILE_PATH);
        return Err(PanelError::CommandFailed {
            binary: "mkswap",
            exit_code: mk.exit_code,
            stderr: mk.stderr,
        });
    }

    let on = executor
        .run(AllowedBinary::Swapon, &[SWAPFILE_PATH], SWAP_CMD_TIMEOUT)
        .await?;
    if !on.success {
        let _ = std::fs::remove_file(SWAPFILE_PATH);
        return Err(PanelError::CommandFailed {
            binary: "swapon",
            exit_code: on.exit_code,
            stderr: on.stderr,
        });
    }

    update_fstab_swapfile(true)?;

    if let Some(sw) = swappiness {
        let _ = update_sysctl(
            executor,
            SysctlUpdatePatch {
                swappiness: Some(sw),
                somaxconn: None,
                tcp_congestion_control: None,
                file_max: None,
            },
        )
        .await;
    }

    get_sysops_overview(executor).await
}

pub async fn remove_swap(executor: &dyn CommandExecutor) -> Result<SysopsOverview, PanelError> {
    let (active_now, _) = detect_swapfile_status();
    if active_now {
        if !executor.is_binary_available(AllowedBinary::Swapoff) {
            return Err(PanelError::BinaryUnavailable("swapoff"));
        }
        let off = executor
            .run(AllowedBinary::Swapoff, &[SWAPFILE_PATH], SWAP_CMD_TIMEOUT)
            .await?;
        if !off.success {
            return Err(PanelError::CommandFailed {
                binary: "swapoff",
                exit_code: off.exit_code,
                stderr: off.stderr,
            });
        }
    }

    if Path::new(SWAPFILE_PATH).exists() {
        std::fs::remove_file(SWAPFILE_PATH)?;
    }
    update_fstab_swapfile(false)?;
    get_sysops_overview(executor).await
}

fn update_fstab_swapfile(present: bool) -> Result<(), PanelError> {
    let fstab = Path::new(FSTAB_PATH);
    if !fstab.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(fstab)?;
    let mut lines: Vec<String> = raw
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with(SWAPFILE_PATH)
        })
        .map(str::to_string)
        .collect();
    if present {
        lines.push(format!("{SWAPFILE_PATH} none swap sw 0 0"));
    }
    let mut body = lines.join("\n");
    body.push('\n');
    atomic_write_file(fstab, body.as_bytes(), 0o644)
}

pub async fn service_action(
    executor: &dyn CommandExecutor,
    raw_unit: &str,
    raw_action: &str,
) -> Result<String, PanelError> {
    let unit = validate_systemd_unit(raw_unit)?;
    let action = match raw_action.trim().to_ascii_lowercase().as_str() {
        "start" => "start",
        "stop" => "stop",
        "restart" => "restart",
        "reload" => "reload",
        "enable" => "enable",
        "disable" => "disable",
        _ => {
            return Err(PanelError::Validation(
                "Hành động dịch vụ chỉ hỗ trợ: start, stop, restart, reload, enable, disable."
                    .to_string(),
            ))
        }
    };

    if matches!(action, "stop" | "disable") && is_protected_unit(&unit) {
        return Err(PanelError::Forbidden(format!(
            "Anti-Lockout Guard: không cho phép '{action}' trên dịch vụ hệ thống trọng yếu '{unit}'."
        )));
    }

    if !executor.is_binary_available(AllowedBinary::Systemctl) {
        return Err(PanelError::BinaryUnavailable("systemctl"));
    }

    let out = executor
        .run(AllowedBinary::Systemctl, &[action, &unit], CMD_TIMEOUT)
        .await?;
    if !out.success {
        return Err(PanelError::CommandFailed {
            binary: "systemctl",
            exit_code: out.exit_code,
            stderr: if out.stderr.is_empty() {
                format!("systemctl {action} {unit} thất bại")
            } else {
                out.stderr
            },
        });
    }

    Ok(format!("Đã thực thi `systemctl {action} {unit}` thành công."))
}

pub async fn get_service_logs(
    executor: &dyn CommandExecutor,
    raw_unit: &str,
    lines: usize,
) -> Result<ServiceLogsResponse, PanelError> {
    let unit = validate_systemd_unit(raw_unit)?;
    let clamped = lines.clamp(10, 500);
    if !executor.is_binary_available(AllowedBinary::Journalctl) {
        return Err(PanelError::BinaryUnavailable("journalctl"));
    }
    let lines_arg = clamped.to_string();
    let out = executor
        .run(
            AllowedBinary::Journalctl,
            &["-u", &unit, "-n", &lines_arg, "--no-pager", "-o", "short-iso"],
            CMD_TIMEOUT,
        )
        .await?;
    let content = if !out.stdout.is_empty() {
        out.stdout
    } else if !out.stderr.is_empty() {
        out.stderr
    } else {
        format!("(Chưa có nhật ký journald cho {unit})")
    };

    Ok(ServiceLogsResponse {
        unit,
        lines: clamped,
        content,
        fetched_at: Utc::now().to_rfc3339(),
    })
}

pub async fn update_sysctl(
    executor: &dyn CommandExecutor,
    patch: SysctlUpdatePatch,
) -> Result<SysctlTuning, PanelError> {
    let mut updates: Vec<(&'static str, String)> = Vec::new();

    if let Some(sw) = patch.swappiness {
        if sw > 100 {
            return Err(PanelError::Validation(
                "vm.swappiness phải nằm trong khoảng 0..=100.".to_string(),
            ));
        }
        updates.push(("vm.swappiness", sw.to_string()));
    }
    if let Some(sc) = patch.somaxconn {
        if !(128..=65_535).contains(&sc) {
            return Err(PanelError::Validation(
                "net.core.somaxconn phải nằm trong khoảng 128..=65535.".to_string(),
            ));
        }
        updates.push(("net.core.somaxconn", sc.to_string()));
    }
    if let Some(cc_raw) = patch.tcp_congestion_control {
        let cc = cc_raw.trim().to_ascii_lowercase();
        if !matches!(cc.as_str(), "bbr" | "cubic" | "reno") {
            return Err(PanelError::Validation(
                "net.ipv4.tcp_congestion_control chỉ hỗ trợ: bbr, cubic, reno.".to_string(),
            ));
        }
        updates.push(("net.ipv4.tcp_congestion_control", cc));
    }
    if let Some(fm) = patch.file_max {
        if !(65_536..=10_000_000).contains(&fm) {
            return Err(PanelError::Validation(
                "fs.file-max phải nằm trong khoảng 65536..=10000000.".to_string(),
            ));
        }
        updates.push(("fs.file-max", fm.to_string()));
    }

    if updates.is_empty() {
        return Err(PanelError::Validation(
            "Không có tham số sysctl nào được gửi để cập nhật.".to_string(),
        ));
    }
    if !executor.is_binary_available(AllowedBinary::Sysctl) {
        return Err(PanelError::BinaryUnavailable("sysctl"));
    }

    for (key, val) in &updates {
        let pair = format!("{key}={val}");
        let out = executor
            .run(AllowedBinary::Sysctl, &["-w", &pair], CMD_TIMEOUT)
            .await?;
        if !out.success {
            return Err(PanelError::CommandFailed {
                binary: "sysctl",
                exit_code: out.exit_code,
                stderr: if out.stderr.is_empty() {
                    format!("Không thể áp dụng {pair}")
                } else {
                    out.stderr
                },
            });
        }
    }

    // Persist into /etc/sysctl.d/99-ferrous-sysops.conf so tuning survives reboot.
    let dropin = Path::new(SYSCTL_DROPIN_PATH);
    let mut persisted: BTreeMap<String, String> = BTreeMap::new();
    if let Ok(existing) = std::fs::read_to_string(dropin) {
        for line in existing.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = trimmed.split_once('=') {
                persisted.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    for (k, v) in updates {
        persisted.insert(k.to_string(), v);
    }
    let mut file_body = String::from("# Managed by FerrousPanel SysOps\n");
    for (k, v) in persisted {
        file_body.push_str(&format!("{k} = {v}\n"));
    }
    let _ = atomic_write_file(dropin, file_body.as_bytes(), 0o644);

    Ok(read_sysctl_tuning())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proc_stat_with_spaces_in_comm() {
        let line = "412 (Web Content) S 123 412 412 0 -1 4194304";
        let parsed = parse_proc_stat_fields(line).expect("parsed");
        assert_eq!(parsed.0, "Web Content");
        assert_eq!(parsed.1, "S");
        assert_eq!(parsed.2, 123);
    }

    #[test]
    fn parses_ss_listening_ports_cleanly() {
        let sample = "\
tcp   LISTEN 0 511 0.0.0.0:80 0.0.0.0:* users:((\"nginx\",pid=1024,fd=6))\n\
tcp   LISTEN 0 128 [::]:8443 [::]:* users:((\"ferrous-panel\",pid=512,fd=9))\n";
        let ports = parse_ss_listening_ports(sample);
        assert_eq!(ports.len(), 2);
        assert_eq!(ports[0].port, 80);
        assert_eq!(ports[0].process, "nginx");
        assert_eq!(ports[0].pid, Some(1024));
        assert_eq!(ports[1].port, 8443);
        assert_eq!(ports[1].process, "ferrous-panel");
    }

    #[test]
    fn parses_df_and_skips_tmpfs() {
        let sample = "\
Filesystem     Type     1B-blocks        Used   Available Capacity Mounted on\n\
/dev/vda1      ext4   53687091200 10737418240 42949672960      20% /\n\
tmpfs          tmpfs   1073741824     1048576  1072693248       1% /run\n";
        let mounts = parse_df_mounts(sample);
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0].mount_point, "/");
        assert_eq!(mounts[0].fs_type, "ext4");
    }

    #[test]
    fn validates_systemd_units_and_blocks_traversal() {
        assert_eq!(
            validate_systemd_unit("nginx").expect("valid"),
            "nginx.service"
        );
        assert_eq!(
            validate_systemd_unit("php8.3-fpm.service").expect("valid"),
            "php8.3-fpm.service"
        );
        assert!(validate_systemd_unit("-bad").is_err());
        assert!(validate_systemd_unit("../sshd").is_err());
        assert!(validate_systemd_unit("nginx;id").is_err());
    }
}
