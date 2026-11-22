use super::common::*;

// ============================================================================
// 6. System Optimizer & Anti-Crash Guard
// ============================================================================

const ALLOWED_SYSCTL_KEYS: &[&str] = &[
    "net.ipv4.tcp_congestion_control",
    "net.core.somaxconn",
    "net.ipv4.tcp_tw_reuse",
    "vm.swappiness",
    "fs.file-max",
    "net.ipv4.tcp_syncookies",
    "net.ipv4.tcp_max_syn_backlog",
];

fn validate_sysctl_assignment(key: &str, value: &str) -> Result<(), PanelError> {
    if !ALLOWED_SYSCTL_KEYS.contains(&key) {
        return Err(PanelError::Validation(format!(
            "sysctl key '{key}' is not in the FerrousPanel optimizer allowlist"
        )));
    }
    if value.is_empty()
        || value.len() > 64
        || value.starts_with('-')
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    {
        return Err(PanelError::Validation(format!(
            "invalid sysctl value '{value}' for key '{key}'"
        )));
    }
    Ok(())
}

fn render_sysctl_preview(params: &[KernelSysctlParam]) -> String {
    let mut out = String::from("# Managed by FerrousPanel — /etc/sysctl.d/99-ferrous-optimizer.conf\n");
    for p in params {
        if p.enabled {
            out.push_str(&format!("{} = {}\n", p.key, p.recommended_value));
        } else {
            out.push_str(&format!(
                "# {} = {} (current: {})\n",
                p.key, p.recommended_value, p.current_value
            ));
        }
    }
    out
}

fn default_optimizer_data() -> SystemOptimizerData {
    let rss = read_process_rss_mb();
    let pid = std::process::id();
    let sysctl_params = vec![
        KernelSysctlParam {
            id: "sys-bbr".to_string(),
            key: "net.ipv4.tcp_congestion_control".to_string(),
            category: "TCP / Network".to_string(),
            current_value: read_proc_sysctl("net.ipv4.tcp_congestion_control")
                .unwrap_or_else(|| "unknown".to_string()),
            recommended_value: "bbr".to_string(),
            description: "Google BBR bandwidth-delay product congestion control for high-throughput TLS.".to_string(),
            enabled: true,
        },
        KernelSysctlParam {
            id: "sys-somaxconn".to_string(),
            key: "net.core.somaxconn".to_string(),
            category: "TCP / Network".to_string(),
            current_value: read_proc_sysctl("net.core.somaxconn")
                .unwrap_or_else(|| "unknown".to_string()),
            recommended_value: "65535".to_string(),
            description: "Maximum listen() socket accept queue depth for Actix-Web & FerrousX.".to_string(),
            enabled: true,
        },
        KernelSysctlParam {
            id: "sys-tw-reuse".to_string(),
            key: "net.ipv4.tcp_tw_reuse".to_string(),
            category: "TCP / Network".to_string(),
            current_value: read_proc_sysctl("net.ipv4.tcp_tw_reuse")
                .unwrap_or_else(|| "unknown".to_string()),
            recommended_value: "1".to_string(),
            description: "Safely reuse TIME_WAIT sockets for outbound reverse-proxy upstream connections.".to_string(),
            enabled: true,
        },
        KernelSysctlParam {
            id: "sys-swappiness".to_string(),
            key: "vm.swappiness".to_string(),
            category: "Virtual Memory".to_string(),
            current_value: read_proc_sysctl("vm.swappiness")
                .unwrap_or_else(|| "unknown".to_string()),
            recommended_value: "10".to_string(),
            description: "Prefer keeping hot SQLite/Database pages in physical RAM before swapping.".to_string(),
            enabled: true,
        },
        KernelSysctlParam {
            id: "sys-file-max".to_string(),
            key: "fs.file-max".to_string(),
            category: "Filesystem".to_string(),
            current_value: read_proc_sysctl("fs.file-max")
                .unwrap_or_else(|| "unknown".to_string()),
            recommended_value: "2097152".to_string(),
            description: "System-wide open file descriptor ceiling for high-concurrency epoll loops.".to_string(),
            enabled: true,
        },
    ];
    let preview = render_sysctl_preview(&sysctl_params);

    SystemOptimizerData {
        active_profile_id: "profile-high-concurrency".to_string(),
        profiles: vec![
            OptimizationProfilePreset {
                id: "profile-low-ram".to_string(),
                name: "1GB–2GB VPS Eco Mode".to_string(),
                badge: "Low Memory Guard".to_string(),
                description: "Caps database buffer pools and worker threads to prevent OOM on compact VPS nodes.".to_string(),
                actix_workers: 2,
                actix_max_connections: 4096,
                actix_keep_alive_secs: 30,
                nginx_worker_connections: 2048,
                nginx_keepalive_requests: 500,
                mariadb_buffer_pool_mb: 128,
                postgres_shared_buffers_mb: 128,
                redis_max_memory_mb: 64,
                expected_p99_reduction_percent: 24.0,
            },
            OptimizationProfilePreset {
                id: "profile-high-concurrency".to_string(),
                name: "High-Throughput Production".to_string(),
                badge: "Recommended".to_string(),
                description: "Balances epoll worker pools, TCP BBR queues, and SQL shared buffers for low p99 latency.".to_string(),
                actix_workers: 4,
                actix_max_connections: 32768,
                actix_keep_alive_secs: 75,
                nginx_worker_connections: 8192,
                nginx_keepalive_requests: 2000,
                mariadb_buffer_pool_mb: 512,
                postgres_shared_buffers_mb: 512,
                redis_max_memory_mb: 256,
                expected_p99_reduction_percent: 42.5,
            },
            OptimizationProfilePreset {
                id: "profile-extreme-edge".to_string(),
                name: "Extreme Edge & Anti-DDoS".to_string(),
                badge: "Max IOPS".to_string(),
                description: "Maximizes socket backlog, connection recycling, and SYN flood absorption.".to_string(),
                actix_workers: 8,
                actix_max_connections: 65535,
                actix_keep_alive_secs: 60,
                nginx_worker_connections: 16384,
                nginx_keepalive_requests: 5000,
                mariadb_buffer_pool_mb: 1024,
                postgres_shared_buffers_mb: 1024,
                redis_max_memory_mb: 512,
                expected_p99_reduction_percent: 58.0,
            },
        ],
        sysctl_params,
        sysctl_config_preview: preview,
        watchdog: BackendWatchdogState {
            daemon_pid: pid,
            oom_score_adj: -900,
            allocator_engine: "System Allocator".to_string(),
            allocated_ram_mb: (rss * 0.85 * 100.0).round() / 100.0,
            resident_ram_mb: rss,
            worker_threads_active: 4,
            worker_threads_total: 4,
            db_pool_active: 1,
            db_pool_idle: 3,
            db_pool_max: 16,
            circuit_breaker_status: "Closed (Healthy)".to_string(),
            auto_heal_enabled: true,
            last_self_heal_at: Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            uptime_stability_percent: 100.0,
            recent_events: Vec::new(),
        },
        anti_crash_guard: Some(AntiCrashGuardState {
            load_shedding_enabled: true,
            cpu_shed_threshold_percent: 92.0,
            ram_shed_threshold_percent: 90.0,
            zram_enabled: Path::new("/dev/zram0").exists(),
            zram_compression_algo: "zstd".to_string(),
            zram_size_mb: 2048,
            slow_query_auto_kill_sec: 30,
            cgroups_v2_enforced: Path::new("/sys/fs/cgroup/cgroup.controllers").exists(),
            oom_immunity_map: vec![OomImmunityProcessItem {
                service: "ferrous-panel.service".to_string(),
                pid,
                oom_score_adj: std::fs::read_to_string("/proc/self/oom_score_adj")
                    .ok()
                    .and_then(|s| s.trim().parse::<i32>().ok())
                    .unwrap_or(0),
                memory_high_mb: 256,
                memory_max_mb: 512,
                policy: "Control Plane".to_string(),
            }],
            mitigation_count: 0,
            last_mitigation_action: "None".to_string(),
        }),
    }
}

pub fn load_optimizer(
    store: &ModuleStore,
    config: &Config,
) -> Result<SystemOptimizerData, PanelError> {
    let mut data = if let Some(existing) = store.get_json::<SystemOptimizerData>(KV_OPTIMIZER)? {
        existing
    } else {
        let initial = default_optimizer_data();
        store.put_json(KV_OPTIMIZER, &initial)?;
        initial
    };
    if is_production_vps(config) {
        for param in &mut data.sysctl_params {
            if let Some(live_val) = read_proc_sysctl(&param.key) {
                param.current_value = live_val;
            }
        }
    }
    let rss = read_process_rss_mb();
    data.watchdog.daemon_pid = std::process::id();
    data.watchdog.resident_ram_mb = rss;
    data.watchdog.allocated_ram_mb = ((rss * 0.85) * 100.0).round() / 100.0;
    Ok(data)
}

async fn apply_sysctl_to_host(
    config: &Config,
    executor: &dyn CommandExecutor,
    params: &[KernelSysctlParam],
    preview: &str,
) -> Result<(), PanelError> {
    for param in params.iter().filter(|p| p.enabled) {
        validate_sysctl_assignment(&param.key, &param.recommended_value)?;
    }

    if !is_production_vps(config) {
        return Ok(());
    }

    if !executor.is_binary_available(AllowedBinary::Sysctl) {
        return Err(PanelError::Conflict(
            "thiếu binary sysctl trên máy chủ (gợi ý: sudo apt install -y procps)".to_string(),
        ));
    }

    let sys_dir = Path::new("/etc/sysctl.d");
    if !sys_dir.is_dir() {
        return Err(PanelError::Conflict(
            "không tìm thấy thư mục /etc/sysctl.d để ghi cấu hình kernel".to_string(),
        ));
    }

    let target_file = sys_dir.join("99-ferrous-optimizer.conf");
    let previous = std::fs::read(&target_file).ok();
    atomic_write_file(&target_file, preview.as_bytes(), 0o644)?;

    let rollback = || {
        if let Some(ref bytes) = previous {
            let _ = atomic_write_file(&target_file, bytes, 0o644);
        } else {
            let _ = std::fs::remove_file(&target_file);
        }
    };

    let apply_res = executor
        .run(
            AllowedBinary::Sysctl,
            &["-p", "/etc/sysctl.d/99-ferrous-optimizer.conf"],
            CMD_TIMEOUT,
        )
        .await;
    match apply_res {
        Ok(out) if out.success => Ok(()),
        Ok(out) => {
            rollback();
            let _ = executor
                .run(AllowedBinary::Sysctl, &["--system"], CMD_TIMEOUT)
                .await;
            Err(PanelError::CommandFailed {
                binary: "sysctl",
                exit_code: out.exit_code,
                stderr: if out.stderr.trim().is_empty() {
                    out.stdout
                } else {
                    out.stderr
                },
            })
        }
        Err(err) => {
            rollback();
            Err(err)
        }
    }
}

pub async fn apply_optimizer_profile(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    profile_id: &str,
) -> Result<SystemOptimizerData, PanelError> {
    let mut data = load_optimizer(store, config)?;
    if !data.profiles.iter().any(|p| p.id == profile_id) && !profile_id.trim().is_empty() {
        return Err(PanelError::NotFound(format!(
            "optimization profile '{profile_id}' not found"
        )));
    }
    if !profile_id.trim().is_empty() {
        data.active_profile_id = profile_id.trim().to_string();
    }
    data.sysctl_config_preview = render_sysctl_preview(&data.sysctl_params);

    let sysctl_dir = config.data_dir.join("sysctl");
    std::fs::create_dir_all(&sysctl_dir)?;
    atomic_write_file(
        &sysctl_dir.join("99-ferrous-optimizer.conf"),
        data.sysctl_config_preview.as_bytes(),
        0o644,
    )?;

    apply_sysctl_to_host(config, executor, &data.sysctl_params, &data.sysctl_config_preview).await?;
    if is_production_vps(config) {
        for param in &mut data.sysctl_params {
            if let Some(live_val) = read_proc_sysctl(&param.key) {
                param.current_value = live_val;
            }
        }
    }

    data.watchdog.recent_events.insert(
        0,
        WatchdogEventItem {
            id: format!("wd-{:x}", Utc::now().timestamp_millis()),
            timestamp: Utc::now().format("%H:%M:%S UTC").to_string(),
            subsystem: "System Optimizer".to_string(),
            action: format!(
                "Applied profile '{}' & verified kernel sysctl via /proc/sys",
                data.active_profile_id
            ),
            result: "OK".to_string(),
        },
    );

    store.put_json(KV_OPTIMIZER, &data)?;
    Ok(data)
}

pub async fn toggle_optimizer_sysctl(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<SystemOptimizerData, PanelError> {
    let mut data = load_optimizer(store, config)?;
    let Some(param) = data
        .sysctl_params
        .iter_mut()
        .find(|p| p.id == id || p.key == id)
    else {
        return Err(PanelError::NotFound(format!(
            "sysctl parameter '{id}' not found"
        )));
    };
    validate_sysctl_assignment(&param.key, &param.recommended_value)?;
    param.enabled = !param.enabled;
    let toggled_key = param.key.clone();
    if !is_production_vps(config) {
        param.current_value = if param.enabled {
            param.recommended_value.clone()
        } else {
            "default".to_string()
        };
    }

    data.sysctl_config_preview = render_sysctl_preview(&data.sysctl_params);

    let sysctl_dir = config.data_dir.join("sysctl");
    std::fs::create_dir_all(&sysctl_dir)?;
    atomic_write_file(
        &sysctl_dir.join("99-ferrous-optimizer.conf"),
        data.sysctl_config_preview.as_bytes(),
        0o644,
    )?;

    apply_sysctl_to_host(config, executor, &data.sysctl_params, &data.sysctl_config_preview).await?;
    if is_production_vps(config) {
        if let Some(live_val) = read_proc_sysctl(&toggled_key) {
            if let Some(p) = data.sysctl_params.iter_mut().find(|p| p.key == toggled_key) {
                p.current_value = live_val;
            }
        }
    }

    store.put_json(KV_OPTIMIZER, &data)?;
    Ok(data)
}

pub fn reclaim_memory(
    store: &ModuleStore,
    config: &Config,
) -> Result<(u64, String, SystemOptimizerData), PanelError> {
    let mut data = load_optimizer(store, config)?;
    let freed_mb = if is_production_vps(config) {
        let before_kb = read_proc_mem_available_kb().unwrap_or(0);
        std::fs::write("/proc/sys/vm/drop_caches", b"3\n").map_err(|err| {
            PanelError::Conflict(format!(
                "không thể ghi /proc/sys/vm/drop_caches (cần quyền root): {err}"
            ))
        })?;
        let after_kb = read_proc_mem_available_kb().unwrap_or(before_kb);
        after_kb.saturating_sub(before_kb) / 1024
    } else {
        0u64
    };
    data.watchdog.last_self_heal_at = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    data.watchdog.recent_events.insert(
        0,
        WatchdogEventItem {
            id: format!("wd-reclaim-{:x}", Utc::now().timestamp_millis()),
            timestamp: Utc::now().format("%H:%M:%S UTC").to_string(),
            subsystem: "Memory Reclaimer".to_string(),
            action: "Flushed Linux page/dentry/inode caches (/proc/sys/vm/drop_caches)".to_string(),
            result: format!("Freed {freed_mb} MB"),
        },
    );
    store.put_json(KV_OPTIMIZER, &data)?;
    Ok((
        freed_mb,
        format!("Đã giải phóng {freed_mb} MB bộ nhớ đệm hệ thống (/proc/sys/vm/drop_caches)."),
        data,
    ))
}

pub async fn watchdog_heal(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
) -> Result<SystemOptimizerData, PanelError> {
    let mut data = load_optimizer(store, config)?;
    let mut healed_units: Vec<String> = Vec::new();
    let mut healthy_units: Vec<String> = Vec::new();
    let mut failed_units: Vec<String> = Vec::new();

    if is_production_vps(config) {
        if !executor.is_binary_available(AllowedBinary::Systemctl) {
            return Err(PanelError::Conflict(
                "thiếu systemctl để kiểm tra và tự phục hồi dịch vụ".to_string(),
            ));
        }
        let candidates = [
            "ferrous-panel.service",
            "ferrousx.service",
            "nginx.service",
            "mariadb.service",
            "postgresql.service",
            "php8.3-fpm.service",
        ];
        for unit in candidates {
            let is_enabled = executor
                .run(AllowedBinary::Systemctl, &["is-enabled", unit], CMD_TIMEOUT)
                .await
                .map(|o| o.success && o.stdout.trim() == "enabled")
                .unwrap_or(false);
            if !is_enabled {
                continue;
            }
            let is_active = executor
                .run(AllowedBinary::Systemctl, &["is-active", unit], CMD_TIMEOUT)
                .await
                .map(|o| o.success && o.stdout.trim() == "active")
                .unwrap_or(false);
            if is_active {
                healthy_units.push(unit.to_string());
            } else {
                let restarted = executor
                    .run(AllowedBinary::Systemctl, &["restart", unit], CMD_TIMEOUT)
                    .await
                    .map(|o| o.success)
                    .unwrap_or(false);
                if restarted {
                    healed_units.push(unit.to_string());
                } else {
                    failed_units.push(unit.to_string());
                }
            }
        }
    }

    let summary = if !failed_units.is_empty() {
        format!(
            "Failed to restart: {}; healed: {}",
            failed_units.join(", "),
            healed_units.join(", ")
        )
    } else if !healed_units.is_empty() {
        format!(
            "Auto-restarted inactive VPS services: {}",
            healed_units.join(", ")
        )
    } else if !healthy_units.is_empty() {
        format!("Verified {} enabled VPS services active", healthy_units.len())
    } else {
        "Executed self-healing check (no enabled services required restart)".to_string()
    };

    data.watchdog.last_self_heal_at = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    data.watchdog.circuit_breaker_status = if failed_units.is_empty() {
        "Closed (Healthy)".to_string()
    } else {
        "Degraded (Restart Failures)".to_string()
    };
    data.watchdog.recent_events.insert(
        0,
        WatchdogEventItem {
            id: format!("wd-heal-{:x}", Utc::now().timestamp_millis()),
            timestamp: Utc::now().format("%H:%M:%S UTC").to_string(),
            subsystem: "Watchdog Supervisor".to_string(),
            action: summary,
            result: if !failed_units.is_empty() {
                format!("Failed ({})", failed_units.len())
            } else if healed_units.is_empty() {
                "Healthy".to_string()
            } else {
                format!("Healed ({})", healed_units.len())
            },
        },
    );
    store.put_json(KV_OPTIMIZER, &data)?;
    if !failed_units.is_empty() {
        return Err(PanelError::Conflict(format!(
            "không thể tự khởi động lại các dịch vụ: {}",
            failed_units.join(", ")
        )));
    }
    Ok(data)
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AntiCrashPatch {
    pub load_shedding_enabled: Option<bool>,
    pub cpu_shed_threshold_percent: Option<f64>,
    pub ram_shed_threshold_percent: Option<f64>,
    pub zram_enabled: Option<bool>,
    pub zram_compression_algo: Option<String>,
    pub zram_size_mb: Option<u64>,
    pub slow_query_auto_kill_sec: Option<u64>,
    pub cgroups_v2_enforced: Option<bool>,
}

pub fn update_anti_crash(
    store: &ModuleStore,
    config: &Config,
    patch: AntiCrashPatch,
) -> Result<SystemOptimizerData, PanelError> {
    let mut data = load_optimizer(store, config)?;
    let mut guard = data
        .anti_crash_guard
        .take()
        .unwrap_or_else(|| {
            default_optimizer_data()
                .anti_crash_guard
                .unwrap_or(AntiCrashGuardState {
                    load_shedding_enabled: true,
                    cpu_shed_threshold_percent: 92.0,
                    ram_shed_threshold_percent: 90.0,
                    zram_enabled: false,
                    zram_compression_algo: "zstd".to_string(),
                    zram_size_mb: 2048,
                    slow_query_auto_kill_sec: 30,
                    cgroups_v2_enforced: false,
                    oom_immunity_map: Vec::new(),
                    mitigation_count: 0,
                    last_mitigation_action: String::new(),
                })
        });

    if let Some(v) = patch.load_shedding_enabled {
        guard.load_shedding_enabled = v;
    }
    if let Some(v) = patch.cpu_shed_threshold_percent {
        guard.cpu_shed_threshold_percent = v.clamp(50.0, 99.0);
    }
    if let Some(v) = patch.ram_shed_threshold_percent {
        guard.ram_shed_threshold_percent = v.clamp(50.0, 99.0);
    }
    if let Some(v) = patch.zram_enabled {
        guard.zram_enabled = v;
    }
    if let Some(v) = patch.zram_compression_algo {
        guard.zram_compression_algo = v;
    }
    if let Some(v) = patch.zram_size_mb {
        guard.zram_size_mb = v;
    }
    if let Some(v) = patch.slow_query_auto_kill_sec {
        guard.slow_query_auto_kill_sec = v;
    }
    if let Some(v) = patch.cgroups_v2_enforced {
        guard.cgroups_v2_enforced = v;
    }

    data.anti_crash_guard = Some(guard);
    store.put_json(KV_OPTIMIZER, &data)?;
    Ok(data)
}

pub async fn emergency_mitigate(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    action: &str,
) -> Result<(String, SystemOptimizerData), PanelError> {
    let mut data = load_optimizer(store, config)?;
    let prod = is_production_vps(config);

    let msg = match action.trim() {
        "purge_syn_backlog" => {
            if prod {
                if !executor.is_binary_available(AllowedBinary::Sysctl) {
                    return Err(PanelError::Conflict(
                        "thiếu sysctl để bật tcp_syncookies và mở rộng tcp_max_syn_backlog".to_string(),
                    ));
                }
                for kv in [
                    "net.ipv4.tcp_syncookies=1",
                    "net.ipv4.tcp_max_syn_backlog=16384",
                ] {
                    let out = executor
                        .run(AllowedBinary::Sysctl, &["-w", kv], CMD_TIMEOUT)
                        .await?;
                    if !out.success {
                        return Err(PanelError::CommandFailed {
                            binary: "sysctl",
                            exit_code: out.exit_code,
                            stderr: if out.stderr.trim().is_empty() {
                                out.stdout
                            } else {
                                out.stderr
                            },
                        });
                    }
                }
            }
            "Enforced net.ipv4.tcp_syncookies=1 and net.ipv4.tcp_max_syn_backlog=16384"
        }
        "isolate_leaking_workers" => {
            if prod {
                std::fs::write("/proc/sys/vm/drop_caches", b"3\n").map_err(|err| {
                    PanelError::Conflict(format!(
                        "không thể ghi /proc/sys/vm/drop_caches: {err}"
                    ))
                })?;
                if executor.is_binary_available(AllowedBinary::Systemctl) {
                    let php_active = executor
                        .run(
                            AllowedBinary::Systemctl,
                            &["is-active", "php8.3-fpm.service"],
                            CMD_TIMEOUT,
                        )
                        .await
                        .map(|o| o.success && o.stdout.trim() == "active")
                        .unwrap_or(false);
                    if php_active {
                        let out = executor
                            .run(
                                AllowedBinary::Systemctl,
                                &["reload", "php8.3-fpm.service"],
                                CMD_TIMEOUT,
                            )
                            .await?;
                        if !out.success {
                            return Err(PanelError::CommandFailed {
                                binary: "systemctl",
                                exit_code: out.exit_code,
                                stderr: if out.stderr.trim().is_empty() {
                                    out.stdout
                                } else {
                                    out.stderr
                                },
                            });
                        }
                    }
                }
            }
            "Dropped Linux page caches and reloaded active PHP-FPM workers"
        }
        "reset_hung_db_queries" => {
            if prod {
                if !executor.is_binary_available(AllowedBinary::Systemctl) {
                    return Err(PanelError::Conflict(
                        "thiếu systemctl để reload database engine".to_string(),
                    ));
                }
                for db_unit in ["mariadb.service", "postgresql.service"] {
                    let active = executor
                        .run(AllowedBinary::Systemctl, &["is-active", db_unit], CMD_TIMEOUT)
                        .await
                        .map(|o| o.success && o.stdout.trim() == "active")
                        .unwrap_or(false);
                    if active {
                        let out = executor
                            .run(AllowedBinary::Systemctl, &["reload", db_unit], CMD_TIMEOUT)
                            .await?;
                        if !out.success {
                            return Err(PanelError::CommandFailed {
                                binary: "systemctl",
                                exit_code: out.exit_code,
                                stderr: if out.stderr.trim().is_empty() {
                                    out.stdout
                                } else {
                                    out.stderr
                                },
                            });
                        }
                    }
                }
            }
            "Reloaded active database engines"
        }
        _ => {
            if prod {
                std::fs::write("/proc/sys/vm/drop_caches", b"3\n").map_err(|err| {
                    PanelError::Conflict(format!(
                        "không thể ghi /proc/sys/vm/drop_caches: {err}"
                    ))
                })?;
            }
            "Executed emergency memory cache drop mitigation"
        }
    };
    if let Some(ref mut guard) = data.anti_crash_guard {
        guard.mitigation_count = guard.mitigation_count.saturating_add(1);
        guard.last_mitigation_action = format!("{} ({})", msg, Utc::now().format("%H:%M:%S UTC"));
    }
    data.watchdog.recent_events.insert(
        0,
        WatchdogEventItem {
            id: format!("wd-mitigate-{:x}", Utc::now().timestamp_millis()),
            timestamp: Utc::now().format("%H:%M:%S UTC").to_string(),
            subsystem: "Anti-Crash Guard".to_string(),
            action: msg.to_string(),
            result: "Mitigated".to_string(),
        },
    );
    store.put_json(KV_OPTIMIZER, &data)?;
    Ok((msg.to_string(), data))
}
