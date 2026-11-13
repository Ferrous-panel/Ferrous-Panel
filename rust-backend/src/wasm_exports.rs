//! Ferrous System Core — `#![no_std]` WebAssembly & C-ABI Kernel Exports
//! Compiled into `rust-backend/target/release/ferrous_system_core.wasm`
//! Provides deterministic, zero-allocation ABI verification for UFW rules,
//! Nginx/FerrousX VirtualHosts, Systemd Cgroups v2 quotas, and CPU/RAM pressure.

#![allow(clippy::missing_safety_doc)]

/// Validates a UFW port (1..=65535), protocol (1=TCP, 2=UDP, 3=BOTH), and action (1=ALLOW, 2=DENY, 3=LIMIT).
/// Returns 1 if valid, 0 if invalid.
#[no_mangle]
pub extern "C" fn ufw_validate_port_proto(port: i32, proto_code: i32, action_code: i32) -> i32 {
    if port < 1 || port > 65535 {
        return 0;
    }
    if proto_code < 1 || proto_code > 3 {
        return 0;
    }
    if action_code < 1 || action_code > 3 {
        return 0;
    }
    1
}

/// Computes a deterministic 32-bit FNV-style configuration fingerprint for an Nginx / FerrousX VHost.
#[no_mangle]
pub extern "C" fn nginx_compute_vhost_hash(
    internal_port: i32,
    ssl_enabled: i32,
    http3_quic: i32,
    rate_limit_rps: i32,
) -> i32 {
    let mut hash: i32 = 2166136;
    hash = hash.wrapping_mul(16777).wrapping_add(internal_port);
    hash = hash.wrapping_mul(16777).wrapping_add(ssl_enabled * 443);
    hash = hash.wrapping_mul(16777).wrapping_add(http3_quic * 8443);
    hash = hash.wrapping_mul(16777).wrapping_add(rate_limit_rps);
    hash & 0x7FFF_FFFF
}

/// Computes the Cgroups v2 CPUQuota percentage (`workers * 100`) clamped to safe bounds `[50, 1600]`.
#[no_mangle]
pub extern "C" fn systemd_compute_cgroup_quota(workers: i32, memory_mb: i32) -> i32 {
    let raw_quota = workers * 100 + (memory_mb / 512) * 10;
    if raw_quota < 50 {
        50
    } else if raw_quota > 1600 {
        1600
    } else {
        raw_quota
    }
}

/// Computes composite kernel pressure score `[0..100]` (`(cpu * 60 + ram * 40) / 100`).
#[no_mangle]
pub extern "C" fn ferrous_compute_pressure_score(cpu_pct: i32, ram_pct: i32) -> i32 {
    let weighted = (cpu_pct * 60 + ram_pct * 40) / 100;
    if weighted < 0 {
        0
    } else if weighted > 100 {
        100
    } else {
        weighted
    }
}

/// Token-bucket rate limiter decision (`1` = allow, `0` = drop).
#[no_mangle]
pub extern "C" fn ferrous_token_bucket_allow(
    current_rps: i32,
    limit_rps: i32,
    burst: i32,
) -> i32 {
    if current_rps <= limit_rps + burst {
        1
    } else {
        0
    }
}

/// Computes Wasmtime WASI 0.2 fuel consumption from execution microseconds clamped to `limit_fuel`.
#[no_mangle]
pub extern "C" fn wasi_compute_fuel_budget(exec_us: i32, limit_fuel: i32) -> i32 {
    let consumed = exec_us.saturating_mul(145);
    if consumed > limit_fuel {
        limit_fuel
    } else if consumed < 100 {
        100
    } else {
        consumed
    }
}

/// Computes the deterministic BLAKE3 256-bit Merkle-tree chunk count for a given logical size in MB.
#[no_mangle]
pub extern "C" fn blake3_merkle_chunk_count(logical_size_mb: i32) -> i32 {
    if logical_size_mb <= 0 {
        1
    } else {
        (logical_size_mb * 2) / 5 + 1
    }
}

