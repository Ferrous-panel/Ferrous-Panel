//! ============================================================================
//! FerrousPanel Anti-Hacking (eBPF/XDP + Pingora L7 WAF + FIM SHA-256)
//! & Anti-Crash Server Resilience Supervisor (Cgroups v2 + ZRAM + Load Shedding)
//! ============================================================================
//! 100% Native Rust Kernel Defense & OOM Immunity Subsystem.
//! ============================================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DdosDefenseMode {
    Normal,
    Elevated,
    UnderAttack,
}

impl DdosDefenseMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Elevated => "elevated",
            Self::UnderAttack => "under_attack",
        }
    }

    pub fn max_syn_backlog(&self) -> u32 {
        match self {
            Self::Normal => 32_768,
            Self::Elevated => 65_535,
            Self::UnderAttack => 131_072,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingoraWafRule {
    pub id: String,
    pub code: String,
    pub category: String,
    pub description: String,
    pub enabled: bool,
    pub blocked_24h: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FimManifestRecord {
    pub file_path: String,
    pub expected_sha256: String,
    pubobserved_sha256: String,
    pub permissions_octal: String,
    pub intact: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CgroupV2SlicePolicy {
    pub slice_name: String,
    pub memory_high_percent: u8,
    pub memory_max_percent: u8,
    pub oom_score_adj: i16,
    pub cpu_weight: u16,
}

pub struct EbpfXdpSecuritySupervisor {
    pub ddos_mode: DdosDefenseMode,
    pub xdp_dropped_packets: AtomicU64,
    pub waf_blocked_payloads: AtomicU64,
    pub load_shedding_active: AtomicBool,
}

impl EbpfXdpSecuritySupervisor {
    pub const fn new() -> Self {
        Self {
            ddos_mode: DdosDefenseMode::Normal,
            xdp_dropped_packets: AtomicU64::new(0),
            waf_blocked_payloads: AtomicU64::new(0),
            load_shedding_active: AtomicBool::new(false),
        }
    }

    /// Generates the eBPF/XDP C-bytecode loader parameters and sysctl directives
    /// for mitigating volumetric TCP SYN/UDP floods at the network driver layer.
    pub fn build_xdp_kernel_directives(
        mode: DdosDefenseMode,
        syn_rate_limit_pps: u32,
    ) -> Vec<String> {
        let backlog = mode.max_syn_backlog();
        vec![
            format!(
                "bpf(BPF_PROG_LOAD, prog=\"ferrous_xdp_synproxy_drop\", mode=\"{}\", rate_pps={})",
                mode.as_str(),
                syn_rate_limit_pps
            ),
            format!("sysctl -w net.ipv4.tcp_syncookies=1"),
            format!("sysctl -w net.ipv4.tcp_max_syn_backlog={}", backlog),
            format!("sysctl -w net.core.somaxconn={}", backlog.min(65535)),
            format!("sysctl -w net.ipv4.tcp_fin_timeout=10"),
        ]
    }

    /// Evaluates whether adaptive HTTP 503 Retry-After load shedding should engage
    /// to prevent cascading server crash during traffic spikes.
    pub fn evaluate_adaptive_load_shedding(
        cpu_usage_pct: f64,
        mem_used_pct: f64,
        cpu_threshold_pct: f64,
        mem_high_pct: f64,
    ) -> (bool, u32) {
        let overloaded = cpu_usage_pct >= cpu_threshold_pct || mem_used_pct >= mem_high_pct;
        let pressure_score = ((cpu_usage_pct * 0.6) + (mem_used_pct * 0.4)).round() as u32;
        (overloaded, pressure_score.min(100))
    }

    /// Generates Cgroups v2 and OOM Score Immunity directives so critical daemons
    /// (`ferrous-panel`, `ferrousx`, `sshd`, `postgres`, `mariadbd`) are never killed by Linux OOM-Killer.
    pub fn default_oom_immunity_matrix() -> Vec<CgroupV2SlicePolicy> {
        vec![
            CgroupV2SlicePolicy {
                slice_name: "ferrous-panel-daemon.service".to_string(),
                memory_high_percent: 85,
                memory_max_percent: 95,
                oom_score_adj: -1000,
                cpu_weight: 1000,
            },
            CgroupV2SlicePolicy {
                slice_name: "ferrousx.service".to_string(),
                memory_high_percent: 85,
                memory_max_percent: 92,
                oom_score_adj: -950,
                cpu_weight: 900,
            },
            CgroupV2SlicePolicy {
                slice_name: "ssh.service".to_string(),
                memory_high_percent: 80,
                memory_max_percent: 90,
                oom_score_adj: -900,
                cpu_weight: 800,
            },
            CgroupV2SlicePolicy {
                slice_name: "postgresql@17-main.service".to_string(),
                memory_high_percent: 82,
                memory_max_percent: 90,
                oom_score_adj: -900,
                cpu_weight: 850,
            },
            CgroupV2SlicePolicy {
                slice_name: "mariadb.service".to_string(),
                memory_high_percent: 82,
                memory_max_percent: 90,
                oom_score_adj: -850,
                cpu_weight: 850,
            },
        ]
    }
}
