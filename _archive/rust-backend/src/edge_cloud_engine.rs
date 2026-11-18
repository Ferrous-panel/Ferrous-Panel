//! Ferrous System Core — Rust Edge Cloud, WASI 0.2, Youki OCI & BoringTun Engine (`rust-backend/src/edge_cloud_engine.rs`)
//!
//! Native Rust implementation for:
//! 1. Wasmtime WASI 0.2 (`wasm32-wasip2`) Serverless Edge Workers with fuel metering
//! 2. Rootless Youki OCI Containers & Firecracker KVM MicroVMs with Linux Cgroups v2 isolation
//! 3. BoringTun Userspace WireGuard (`wg0`) Zero-Trust Mesh (`ChaCha20-Poly1305` / `X25519`)
//! 4. BLAKE3 Merkle-Tree + Zstandard Incremental Object Storage Snapshots

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WasiEdgeWorker {
    pub id: String,
    pub name: String,
    pub route_pattern: String,
    pub target_domain: String,
    pub wasi_target: String,
    pub engine_version: String,
    pub cold_start_us: u64,
    pub memory_limit_kb: u64,
    pub memory_used_kb: u64,
    pub fuel_limit: u64,
    pub invocations24h: u64,
    pub avg_execution_us: u64,
    pub blake3_hash: String,
    pub kv_namespace: String,
    pub status: String,
    pub rust_source_snippet: String,
    pub last_invoked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WasiWorkerInvokeResult {
    pub worker_id: String,
    pub worker_name: String,
    pub route: String,
    pub status_code: u16,
    pub cold_start_us: u64,
    pub execution_time_us: u64,
    pub fuel_consumed: u64,
    pub memory_peak_kb: u64,
    pub blake3_verify: String,
    pub response_headers: HashMap<String, String>,
    pub response_body: String,
    pub invoked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OciContainerSandbox {
    pub id: String,
    pub name: String,
    pub image: String,
    pub runtime_engine: String,
    pub cgroups_v2_path: String,
    pub internal_ip: String,
    pub host_port: u16,
    pub container_port: u16,
    pub memory_max_mb: u64,
    pub memory_used_mb: u64,
    pub cpu_quota_percent: u32,
    pub cpu_usage_percent: f64,
    pub pids_current: u32,
    pub pids_max: u32,
    pub seccomp_profile: String,
    pub status: String,
    pub uptime: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireGuardPeerNode {
    pub id: String,
    pub name: String,
    pub region: String,
    pub interface_name: String,
    pub public_key: String,
    pub preshared_key_masked: String,
    pub endpoint: String,
    pub allowed_ips: String,
    pub mesh_ip: String,
    pub cipher_suite: String,
    pub latest_handshake: String,
    pub rx_mb: f64,
    pub tx_mb: f64,
    pub latency_ms: f64,
    pub persistent_keepalive_sec: u32,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Blake3ObjectSnapshot {
    pub id: String,
    pub snapshot_name: String,
    pub target_resource: String,
    pub s3_endpoint: String,
    pub bucket: String,
    pub merkle_root_blake3: String,
    pub compression: String,
    pub dedup_ratio: String,
    pub logical_size_mb: u64,
    pub stored_size_mb: u64,
    pub chunks_count: u64,
    pub immutable_lock: bool,
    pub created_at: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeCloudData {
    pub wasmtime_version: String,
    pub youki_version: String,
    pub boringtun_version: String,
    pub blake3_engine_version: String,
    pub wireguard_interface: String,
    pub wireguard_listen_port: u16,
    pub server_public_key: String,
    pub mesh_subnet_cidr: String,
    pub wasi_workers: Vec<WasiEdgeWorker>,
    pub oci_containers: Vec<OciContainerSandbox>,
    pub wireguard_peers: Vec<WireGuardPeerNode>,
    pub blake3_snapshots: Vec<Blake3ObjectSnapshot>,
}
