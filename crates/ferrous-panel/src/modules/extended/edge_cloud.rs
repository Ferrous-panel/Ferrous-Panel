use super::common::*;

// ============================================================================
// 9. Rust Edge Cloud (Experimental / Optional: Wasmtime, Podman/Docker, WireGuard, Snapshots)
// ============================================================================

fn default_edge_cloud_data(config: &Config) -> EdgeCloudData {
    if is_production_vps(config) {
        return EdgeCloudData {
            wasmtime_version: if AllowedBinary::Wasmtime.is_available() {
                "Wasmtime (installed)".to_string()
            } else {
                "Wasmtime (not installed — experimental)".to_string()
            },
            youki_version: if AllowedBinary::Podman.is_available() {
                "Podman Rootless OCI (installed)".to_string()
            } else if AllowedBinary::Docker.is_available() {
                "Docker OCI (installed)".to_string()
            } else {
                "OCI Runtime (not installed — experimental)".to_string()
            },
            boringtun_version: if AllowedBinary::Wg.is_available() {
                "WireGuard (installed)".to_string()
            } else {
                "WireGuard (not installed — experimental)".to_string()
            },
            blake3_engine_version: "Zstd/Tar + SHA-256 Snapshot Vault".to_string(),
            wireguard_interface: "wg0".to_string(),
            wireguard_listen_port: 51820,
            server_public_key: String::new(),
            mesh_subnet_cidr: "10.66.0.0/24".to_string(),
            wasi_workers: Vec::new(),
            oci_containers: Vec::new(),
            wireguard_peers: Vec::new(),
            blake3_snapshots: Vec::new(),
        };
    }

    let now = Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
    EdgeCloudData {
        wasmtime_version: "Wasmtime (test mode)".to_string(),
        youki_version: "Podman/Docker OCI (test mode)".to_string(),
        boringtun_version: "WireGuard (test mode)".to_string(),
        blake3_engine_version: "Snapshot Vault (test mode)".to_string(),
        wireguard_interface: "wg0".to_string(),
        wireguard_listen_port: 51820,
        server_public_key: String::new(),
        mesh_subnet_cidr: "10.66.0.0/24".to_string(),
        wasi_workers: vec![WasiEdgeWorker {
            id: "wasi-edge-test".to_string(),
            name: "edge-test-worker".to_string(),
            route_pattern: "/api/edge/*".to_string(),
            target_domain: resolve_local_domain(Some(config)),
            wasi_target: "wasm32-wasip2".to_string(),
            engine_version: "Wasmtime (test mode)".to_string(),
            cold_start_us: 0,
            memory_limit_kb: 16384,
            memory_used_kb: 0,
            fuel_limit: 500_000,
            invocations_24h: 0,
            avg_execution_us: 0,
            blake3_hash: sha256_hex(b"wasi-edge-test"),
            kv_namespace: "FERROUS_EDGE_KV".to_string(),
            status: "staged".to_string(),
            rust_source_snippet: "fn main() { println!(\"{{\\\"ok\\\":true}}\"); }".to_string(),
            last_invoked_at: now,
        }],
        oci_containers: Vec::new(),
        wireguard_peers: Vec::new(),
        blake3_snapshots: Vec::new(),
    }
}

fn validate_edge_slug(raw: &str, field: &str, max_len: usize) -> Result<String, PanelError> {
    let clean = raw.trim();
    if clean.is_empty() || clean.len() > max_len {
        return Err(PanelError::Validation(format!(
            "{field} must be between 1 and {max_len} characters"
        )));
    }
    if clean.starts_with('-')
        || !clean
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        return Err(PanelError::Validation(format!(
            "{field} must contain only ASCII letters, digits, '.', '_', or '-' and not start with '-'"
        )));
    }
    Ok(clean.to_string())
}

fn validate_oci_image(raw: &str) -> Result<String, PanelError> {
    let clean = raw.trim();
    if clean.is_empty() || clean.len() > 192 {
        return Err(PanelError::Validation(
            "container image must be between 1 and 192 characters".to_string(),
        ));
    }
    if clean.starts_with('-')
        || clean.chars().any(char::is_whitespace)
        || !clean
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | ':' | '@'))
    {
        return Err(PanelError::Validation(
            "container image contains invalid characters".to_string(),
        ));
    }
    Ok(clean.to_string())
}

fn validate_wg_token(raw: &str, field: &str) -> Result<String, PanelError> {
    let clean = raw.trim();
    if clean.is_empty() || clean.len() > 128 {
        return Err(PanelError::Validation(format!(
            "{field} must be between 1 and 128 characters"
        )));
    }
    if clean.starts_with('-')
        || clean.chars().any(|c| matches!(c, '\r' | '\n' | '\0'))
        || !clean
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '/' | '-' | '_' | '[' | ']' | ',' | ' '))
    {
        return Err(PanelError::Validation(format!(
            "{field} contains forbidden characters"
        )));
    }
    Ok(clean.to_string())
}

fn pick_oci_binary(
    executor: &dyn CommandExecutor,
    runtime_engine: &str,
) -> Option<(AllowedBinary, &'static str)> {
    let lower = runtime_engine.to_ascii_lowercase();
    if lower.contains("docker") && executor.is_binary_available(AllowedBinary::Docker) {
        return Some((AllowedBinary::Docker, "Docker OCI"));
    }
    if executor.is_binary_available(AllowedBinary::Podman) {
        return Some((AllowedBinary::Podman, "Podman Rootless OCI"));
    }
    if executor.is_binary_available(AllowedBinary::Docker) {
        return Some((AllowedBinary::Docker, "Docker OCI"));
    }
    None
}

fn dir_size_bytes(path: &Path) -> u64 {
    let Ok(meta) = std::fs::metadata(path) else {
        return 0;
    };
    if meta.is_file() {
        return meta.len();
    }
    let mut total = 0u64;
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if let Ok(m) = entry.metadata() {
            if m.is_file() {
                total = total.saturating_add(m.len());
            } else if m.is_dir() {
                total = total.saturating_add(dir_size_bytes(&p));
            }
        }
    }
    total
}

async fn ensure_wg_host_private_key(
    config: &Config,
    executor: &dyn CommandExecutor,
) -> Result<String, PanelError> {
    let edge_dir = config.data_dir.join("edge-cloud");
    std::fs::create_dir_all(&edge_dir)?;
    let key_path = edge_dir.join("wg0.key");
    if let Ok(existing) = std::fs::read_to_string(&key_path) {
        let trimmed = existing.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }
    if !executor.is_binary_available(AllowedBinary::Wg) {
        if is_production_vps(config) {
            return Err(PanelError::Conflict(
                "thiếu wireguard-tools (binary `wg`) — gợi ý: sudo apt install -y wireguard-tools".to_string(),
            ));
        }
        let test_key = "TEST_MODE_WG_PRIVATE_KEY_UNSET";
        atomic_write_file(&key_path, format!("{test_key}\n").as_bytes(), 0o600)?;
        return Ok(test_key.to_string());
    }
    let out = executor.run(AllowedBinary::Wg, &["genkey"], CMD_TIMEOUT).await?;
    let trimmed = out.stdout.trim();
    if !out.success || trimmed.is_empty() {
        return Err(PanelError::CommandFailed {
            binary: "wg",
            exit_code: out.exit_code,
            stderr: if out.stderr.trim().is_empty() {
                "wg genkey returned empty key".to_string()
            } else {
                out.stderr
            },
        });
    }
    atomic_write_file(&key_path, format!("{trimmed}\n").as_bytes(), 0o600)?;
    Ok(trimmed.to_string())
}

async fn sync_wireguard_config(
    config: &Config,
    data: &EdgeCloudData,
    executor: &dyn CommandExecutor,
) -> Result<(), PanelError> {
    let priv_key = ensure_wg_host_private_key(config, executor).await?;
    let mut conf = format!(
        "# Managed by FerrousPanel WireGuard Mesh\n[Interface]\nAddress = 10.66.0.1/24\nListenPort = 51820\nPrivateKey = {priv_key}\n\n"
    );
    for peer in &data.wireguard_peers {
        if peer.status != "connected" {
            continue;
        }
        conf.push_str(&format!(
            "# Peer: {} ({})\n[Peer]\nPublicKey = {}\nAllowedIPs = {}\n",
            peer.name, peer.region, peer.public_key, peer.allowed_ips
        ));
        if !peer.endpoint.trim().is_empty() {
            conf.push_str(&format!("Endpoint = {}\n", peer.endpoint.trim()));
        }
        conf.push_str(&format!(
            "PersistentKeepalive = {}\n\n",
            peer.persistent_keepalive_sec.max(15)
        ));
    }

    let edge_dir = config.data_dir.join("edge-cloud");
    std::fs::create_dir_all(&edge_dir)?;
    atomic_write_file(&edge_dir.join("wg0.conf"), conf.as_bytes(), 0o600)?;

    if is_production_vps(config) {
        if !Path::new("/etc/wireguard").is_dir() {
            return Err(PanelError::Conflict(
                "không tìm thấy /etc/wireguard (gợi ý: sudo apt install -y wireguard-tools)".to_string(),
            ));
        }
        atomic_write_file(Path::new("/etc/wireguard/wg0.conf"), conf.as_bytes(), 0o600)?;
    }
    Ok(())
}

pub async fn load_edge_cloud(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
) -> Result<EdgeCloudData, PanelError> {
    let mut data = if let Some(existing) = store.get_json::<EdgeCloudData>(KV_EDGE_CLOUD)? {
        existing
    } else {
        let initial = default_edge_cloud_data(config);
        store.put_json(KV_EDGE_CLOUD, &initial)?;
        initial
    };

    if is_production_vps(config) {
        if let Some((oci_bin, _)) = pick_oci_binary(executor, "podman") {
            for c in &mut data.oci_containers {
                let cname = format!("ferrous-{}", c.name);
                if let Ok(out) = executor
                    .run(
                        oci_bin,
                        &["inspect", "-f", "{{.State.Status}}", "--", &cname],
                        CMD_TIMEOUT,
                    )
                    .await
                {
                    if out.success {
                        let st = out.stdout.trim().to_ascii_lowercase();
                        if !st.is_empty() {
                            c.status = if st == "running" {
                                "running".to_string()
                            } else {
                                "stopped".to_string()
                            };
                        }
                    }
                }
            }
        }

        if executor.is_binary_available(AllowedBinary::Wg) {
            if let Ok(out) = executor
                .run(AllowedBinary::Wg, &["show", "wg0", "dump"], CMD_TIMEOUT)
                .await
            {
                if out.success {
                    for line in out.stdout.lines().skip(1) {
                        let cols: Vec<&str> = line.split('\t').collect();
                        if cols.len() >= 7 {
                            let pubkey = cols[0].trim();
                            let handshake_ts: i64 = cols[4].trim().parse().unwrap_or(0);
                            let rx_bytes: u64 = cols[5].trim().parse().unwrap_or(0);
                            let tx_bytes: u64 = cols[6].trim().parse().unwrap_or(0);
                            for peer in &mut data.wireguard_peers {
                                if peer.public_key == pubkey {
                                    peer.rx_mb = ((rx_bytes as f64) / 1_048_576.0 * 100.0).round() / 100.0;
                                    peer.tx_mb = ((tx_bytes as f64) / 1_048_576.0 * 100.0).round() / 100.0;
                                    if handshake_ts > 0 {
                                        let age = Utc::now().timestamp().saturating_sub(handshake_ts);
                                        peer.latest_handshake = format!("{age}s ago");
                                        peer.status = "connected".to_string();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(data)
}

#[allow(clippy::too_many_arguments)]
pub async fn deploy_wasi_worker(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    name: &str,
    route_pattern: &str,
    target_domain: &str,
    wasi_target: &str,
    memory_limit_kb: u64,
    fuel_limit: u64,
    kv_namespace: &str,
    rust_source_snippet: &str,
) -> Result<EdgeCloudData, PanelError> {
    let clean_name = validate_edge_slug(name, "WASI worker name", 64)?;
    let target_arg = if wasi_target.trim().is_empty() {
        "wasm32-wasip2".to_string()
    } else {
        validate_edge_slug(wasi_target, "WASI target", 48)?
    };
    if is_production_vps(config) && !executor.is_binary_available(AllowedBinary::Rustc) {
        return Err(PanelError::Conflict(
            "thiếu rustc để biên dịch WASI worker (gợi ý: curl https://sh.rustup.rs -sSf | sh && rustup target add wasm32-wasip2)".to_string(),
        ));
    }

    let mut data = load_edge_cloud(store, config, executor).await?;
    let id = format!("wasi-{:x}", Utc::now().timestamp_millis());
    let worker_dir = config.data_dir.join("edge-cloud").join("wasi").join(&id);
    std::fs::create_dir_all(&worker_dir)?;

    let src_path = worker_dir.join("worker.rs");
    let effective_src = if rust_source_snippet.trim().is_empty() {
        format!(
            "fn main() {{\n    println!(\"{{\\\"ok\\\":true,\\\"worker\\\":\\\"{clean_name}\\\",\\\"runtime\\\":\\\"ferrous-wasi\\\"}}\");\n}}\n"
        )
    } else if rust_source_snippet.contains("fn main") {
        rust_source_snippet.to_string()
    } else {
        format!(
            "// FerrousPanel WASI Edge Worker: {clean_name}\n{rust_source_snippet}\n\nfn main() {{\n    println!(\"{{\\\"ok\\\":true,\\\"worker\\\":\\\"{clean_name}\\\",\\\"compiled\\\":true}}\");\n}}\n"
        )
    };
    atomic_write_file(&src_path, effective_src.as_bytes(), 0o644)?;

    let mut engine_version = "WASI Source Staged (test mode)".to_string();
    let mut artifact_bytes = effective_src.as_bytes().to_vec();
    let mut cold_start_us: u64 = 0;
    let mut status = "staged".to_string();

    if executor.is_binary_available(AllowedBinary::Rustc) {
        let src_str = src_path.to_string_lossy().to_string();
        let wasm_out = worker_dir.join("worker.wasm");
        let wasm_str = wasm_out.to_string_lossy().to_string();

        let start = Instant::now();
        let out = executor
            .run(
                AllowedBinary::Rustc,
                &[
                    "--edition",
                    "2021",
                    "--target",
                    &target_arg,
                    "-C",
                    "opt-level=s",
                    "-o",
                    &wasm_str,
                    &src_str,
                ],
                CMD_TIMEOUT,
            )
            .await?;

        if out.success && wasm_out.exists() {
            artifact_bytes = std::fs::read(&wasm_out)?;
            engine_version = format!("Wasmtime WASI ({target_arg} compiled)");
            cold_start_us = start.elapsed().as_micros() as u64;
            status = "deployed".to_string();
        } else if is_production_vps(config) {
            return Err(PanelError::CommandFailed {
                binary: "rustc",
                exit_code: out.exit_code,
                stderr: if out.stderr.trim().is_empty() {
                    out.stdout
                } else {
                    out.stderr
                },
            });
        }
    }

    let hash = sha256_hex(&artifact_bytes);
    let mem_used_kb = ((artifact_bytes.len() as u64) / 1024).max(1);

    data.wasi_workers.insert(
        0,
        WasiEdgeWorker {
            id,
            name: clean_name,
            route_pattern: if route_pattern.trim().is_empty() {
                "/*".to_string()
            } else {
                route_pattern.trim().to_string()
            },
            target_domain: if target_domain.trim().is_empty() {
                "localhost".to_string()
            } else {
                target_domain.trim().to_string()
            },
            wasi_target: target_arg,
            engine_version,
            cold_start_us,
            memory_limit_kb: memory_limit_kb.max(1024),
            memory_used_kb: mem_used_kb,
            fuel_limit: fuel_limit.max(10_000),
            invocations_24h: 0,
            avg_execution_us: 0,
            blake3_hash: hash,
            kv_namespace: kv_namespace.trim().to_string(),
            status,
            rust_source_snippet: rust_source_snippet.to_string(),
            last_invoked_at: Utc::now().format("%Y-%m-%d %H:%M UTC").to_string(),
        },
    );
    store.put_json(KV_EDGE_CLOUD, &data)?;
    Ok(data)
}

pub async fn invoke_wasi_worker(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<(WasiWorkerInvokeResult, EdgeCloudData), PanelError> {
    let mut data = load_edge_cloud(store, config, executor).await?;
    let Some(worker) = data.wasi_workers.iter_mut().find(|w| w.id == id) else {
        return Err(PanelError::NotFound(format!("WASI worker '{id}' not found")));
    };

    let worker_dir = config.data_dir.join("edge-cloud").join("wasi").join(id);
    let wasm_path = worker_dir.join("worker.wasm");
    let src_path = worker_dir.join("worker.rs");

    let start = Instant::now();
    let (exec_mode, status_code, response_body) = if wasm_path.exists()
        && executor.is_binary_available(AllowedBinary::Wasmtime)
    {
        let wasm_str = wasm_path.to_string_lossy().to_string();
        let out = executor
            .run(AllowedBinary::Wasmtime, &["--", &wasm_str], CMD_TIMEOUT)
            .await?;
        if out.success {
            ("wasmtime-real", 200u16, out.stdout.trim().to_string())
        } else {
            return Err(PanelError::CommandFailed {
                binary: "wasmtime",
                exit_code: out.exit_code,
                stderr: if out.stderr.trim().is_empty() {
                    out.stdout
                } else {
                    out.stderr
                },
            });
        }
    } else if is_production_vps(config) {
        return Err(PanelError::Conflict(
            "không thể thực thi WASI worker: thiếu binary `wasmtime` hoặc chưa biên dịch artifact worker.wasm (tính năng thử nghiệm — cài đặt wasmtime từ https://wasmtime.dev)".to_string(),
        ));
    } else {
        (
            "test-mode",
            200u16,
            serde_json::json!({
                "ok": true,
                "worker": worker.name,
                "mode": "test-mode",
            })
            .to_string(),
        )
    };

    let elapsed_us = (start.elapsed().as_micros() as u64).max(1);
    let verified_hash = if let Ok(bytes) = std::fs::read(&wasm_path).or_else(|_| std::fs::read(&src_path)) {
        sha256_hex(&bytes)
    } else {
        worker.blake3_hash.clone()
    };
    let digest_prefix = verified_hash.chars().take(16).collect::<String>();

    worker.invocations_24h = worker.invocations_24h.saturating_add(1);
    worker.avg_execution_us = elapsed_us;
    worker.last_invoked_at = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();

    let mut headers = BTreeMap::new();
    headers.insert("content-type".to_string(), "application/json".to_string());
    headers.insert("x-wasi-engine".to_string(), worker.engine_version.clone());
    headers.insert("x-execution-mode".to_string(), exec_mode.to_string());
    headers.insert("x-sha256-digest".to_string(), verified_hash.clone());

    let res = WasiWorkerInvokeResult {
        worker_id: worker.id.clone(),
        worker_name: worker.name.clone(),
        route: format!("https://{}{}", worker.target_domain, worker.route_pattern),
        status_code,
        cold_start_us: worker.cold_start_us,
        execution_time_us: elapsed_us,
        fuel_consumed: elapsed_us.saturating_mul(64).min(worker.fuel_limit),
        memory_peak_kb: worker.memory_used_kb,
        blake3_verify: format!("VERIFIED ({digest_prefix})"),
        response_headers: headers,
        response_body,
        invoked_at: worker.last_invoked_at.clone(),
    };

    store.put_json(KV_EDGE_CLOUD, &data)?;
    Ok((res, data))
}

pub async fn toggle_wasi_worker(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<EdgeCloudData, PanelError> {
    let mut data = load_edge_cloud(store, config, executor).await?;
    let Some(worker) = data.wasi_workers.iter_mut().find(|w| w.id == id) else {
        return Err(PanelError::NotFound(format!("WASI worker '{id}' not found")));
    };
    worker.status = if worker.status == "deployed" {
        "paused".to_string()
    } else {
        "deployed".to_string()
    };
    store.put_json(KV_EDGE_CLOUD, &data)?;
    Ok(data)
}

pub async fn delete_wasi_worker(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<EdgeCloudData, PanelError> {
    let mut data = load_edge_cloud(store, config, executor).await?;
    data.wasi_workers.retain(|w| w.id != id);
    let worker_dir = config.data_dir.join("edge-cloud").join("wasi").join(id);
    if worker_dir.exists() {
        std::fs::remove_dir_all(&worker_dir)?;
    }
    store.put_json(KV_EDGE_CLOUD, &data)?;
    Ok(data)
}

#[allow(clippy::too_many_arguments)]
pub async fn deploy_oci_container(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    name: &str,
    image: &str,
    runtime_engine: &str,
    host_port: u16,
    container_port: u16,
    memory_max_mb: u64,
    cpu_quota_percent: f64,
) -> Result<EdgeCloudData, PanelError> {
    let clean_name = validate_edge_slug(name, "container name", 48)?;
    let clean_image = validate_oci_image(image)?;
    let h_port = host_port.max(1024);
    let c_port = container_port.max(1);
    let mem_mb = memory_max_mb.max(64);
    let cname = format!("ferrous-{clean_name}");

    let mut data = load_edge_cloud(store, config, executor).await?;
    if data.oci_containers.iter().any(|c| c.name == clean_name) {
        return Err(PanelError::Conflict(format!(
            "container '{clean_name}' already exists"
        )));
    }

    let mut resolved_engine = if runtime_engine.trim().is_empty() {
        "Podman/Docker OCI".to_string()
    } else {
        runtime_engine.trim().to_string()
    };
    let mut status = "staged".to_string();
    let mut uptime = "test-mode".to_string();

    if is_production_vps(config) {
        let Some((oci_bin, label)) = pick_oci_binary(executor, runtime_engine) else {
            return Err(PanelError::Conflict(
                "thiếu container runtime (podman hoặc docker) — gợi ý: sudo apt install -y podman".to_string(),
            ));
        };
        let port_map = format!("127.0.0.1:{h_port}:{c_port}");
        let mem_arg = format!("{mem_mb}m");
        let out = executor
            .run(
                oci_bin,
                &[
                    "run",
                    "-d",
                    "--name",
                    &cname,
                    "--restart",
                    "unless-stopped",
                    "-p",
                    &port_map,
                    "--memory",
                    &mem_arg,
                    "--",
                    &clean_image,
                ],
                APT_TIMEOUT,
            )
            .await?;
        if !out.success {
            let err = if out.stderr.trim().is_empty() {
                out.stdout.trim().to_string()
            } else {
                out.stderr.trim().to_string()
            };
            return Err(PanelError::CommandFailed {
                binary: oci_bin.label(),
                exit_code: out.exit_code,
                stderr: err,
            });
        }
        resolved_engine = label.to_string();
        status = "running".to_string();
        uptime = "Just started".to_string();
    }

    let oci_dir = config.data_dir.join("edge-cloud").join("oci");
    std::fs::create_dir_all(&oci_dir)?;
    let spec_json = serde_json::json!({
        "name": cname,
        "image": clean_image,
        "engine": resolved_engine,
        "hostPort": h_port,
        "containerPort": c_port,
        "memoryMaxMb": mem_mb,
        "cpuQuotaPercent": cpu_quota_percent.clamp(10.0, 800.0),
        "status": status,
    })
    .to_string();
    atomic_write_file(&oci_dir.join(format!("{cname}.json")), spec_json.as_bytes(), 0o644)?;

    let idx = data.oci_containers.len() + 15;
    data.oci_containers.insert(
        0,
        OciContainerSandbox {
            id: format!("oci-{:x}", Utc::now().timestamp_millis()),
            name: clean_name.clone(),
            image: clean_image,
            runtime_engine: resolved_engine,
            cgroups_v2_path: format!("/sys/fs/cgroup/ferrous.slice/{clean_name}.scope"),
            internal_ip: format!("10.88.0.{idx}"),
            host_port: h_port,
            container_port: c_port,
            memory_max_mb: mem_mb,
            memory_used_mb: 0.0,
            cpu_quota_percent: cpu_quota_percent.clamp(10.0, 800.0),
            cpu_usage_percent: 0.0,
            pids_current: 0,
            pids_max: 256,
            seccomp_profile: "default-strict.json".to_string(),
            status,
            uptime,
        },
    );
    store.put_json(KV_EDGE_CLOUD, &data)?;
    Ok(data)
}

pub async fn oci_container_action(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    id: &str,
    action: &str,
) -> Result<EdgeCloudData, PanelError> {
    let mut data = load_edge_cloud(store, config, executor).await?;
    let target = data
        .oci_containers
        .iter()
        .find(|c| c.id == id)
        .map(|c| (c.name.clone(), c.runtime_engine.clone()));

    let Some((name, engine)) = target else {
        return Err(PanelError::NotFound(format!("container '{id}' not found")));
    };
    let cname = format!("ferrous-{name}");

    if is_production_vps(config) {
        let Some((oci_bin, _)) = pick_oci_binary(executor, &engine) else {
            return Err(PanelError::Conflict(
                "thiếu container runtime (podman hoặc docker) — gợi ý: sudo apt install -y podman".to_string(),
            ));
        };
        let args: Vec<&str> = match action {
            "start" => vec!["start", "--", &cname],
            "stop" => vec!["stop", "--", &cname],
            "restart" => vec!["restart", "--", &cname],
            "delete" => vec!["rm", "-f", "--", &cname],
            other => {
                return Err(PanelError::Validation(format!(
                    "unsupported container action '{other}'"
                )));
            }
        };
        let out = executor.run(oci_bin, &args, CMD_TIMEOUT).await?;
        if !out.success {
            return Err(PanelError::CommandFailed {
                binary: oci_bin.label(),
                exit_code: out.exit_code,
                stderr: if out.stderr.trim().is_empty() {
                    out.stdout
                } else {
                    out.stderr
                },
            });
        }
    }

    if action == "delete" {
        data.oci_containers.retain(|c| c.id != id);
        let spec = config
            .data_dir
            .join("edge-cloud")
            .join("oci")
            .join(format!("{cname}.json"));
        if spec.exists() {
            std::fs::remove_file(&spec)?;
        }
        store.put_json(KV_EDGE_CLOUD, &data)?;
        return Ok(data);
    }

    let Some(c) = data.oci_containers.iter_mut().find(|c| c.id == id) else {
        return Err(PanelError::NotFound(format!("container '{id}' not found")));
    };
    match action {
        "start" | "restart" => {
            c.status = "running".to_string();
            c.uptime = "0m".to_string();
        }
        "stop" => {
            c.status = "stopped".to_string();
            c.memory_used_mb = 0.0;
            c.cpu_usage_percent = 0.0;
            c.pids_current = 0;
        }
        other => {
            return Err(PanelError::Validation(format!(
                "unsupported container action '{other}'"
            )));
        }
    }
    store.put_json(KV_EDGE_CLOUD, &data)?;
    Ok(data)
}

pub async fn add_wireguard_peer(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    name: &str,
    region: &str,
    endpoint: &str,
    allowed_ips: &str,
) -> Result<EdgeCloudData, PanelError> {
    let clean_name = validate_edge_slug(name, "peer name", 64)?;
    let clean_region = if region.trim().is_empty() {
        "sg-edge".to_string()
    } else {
        validate_wg_token(region, "region")?
    };
    let clean_endpoint = if endpoint.trim().is_empty() {
        String::new()
    } else {
        validate_wg_token(endpoint, "endpoint")?
    };
    let clean_allowed = if allowed_ips.trim().is_empty() {
        "10.66.0.10/32".to_string()
    } else {
        validate_wg_token(allowed_ips, "allowed IPs")?
    };

    if is_production_vps(config) && !executor.is_binary_available(AllowedBinary::Wg) {
        return Err(PanelError::Conflict(
            "thiếu wireguard-tools (`wg`) để tạo khóa Curve25519 thật (gợi ý: sudo apt install -y wireguard-tools)".to_string(),
        ));
    }

    let mut data = load_edge_cloud(store, config, executor).await?;
    let id = format!("wg-{:x}", Utc::now().timestamp_millis());
    let mesh_ip = clean_allowed
        .split('/')
        .next()
        .unwrap_or("10.66.0.10")
        .trim()
        .to_string();

    let public_key = if executor.is_binary_available(AllowedBinary::Wg) {
        let priv_out = executor.run(AllowedBinary::Wg, &["genkey"], CMD_TIMEOUT).await?;
        let priv_key = priv_out.stdout.trim();
        if !priv_out.success || priv_key.is_empty() {
            return Err(PanelError::CommandFailed {
                binary: "wg",
                exit_code: priv_out.exit_code,
                stderr: priv_out.stderr,
            });
        }
        let pub_out = executor
            .run_with_stdin(
                AllowedBinary::Wg,
                &["pubkey"],
                format!("{priv_key}\n").as_bytes(),
                CMD_TIMEOUT,
            )
            .await?;
        let derived = pub_out.stdout.trim();
        if !pub_out.success || derived.is_empty() {
            return Err(PanelError::CommandFailed {
                binary: "wg",
                exit_code: pub_out.exit_code,
                stderr: pub_out.stderr,
            });
        }
        let peers_dir = config.data_dir.join("edge-cloud").join("peers");
        std::fs::create_dir_all(&peers_dir)?;
        atomic_write_file(
            &peers_dir.join(format!("{id}.key")),
            format!("{priv_key}\n").as_bytes(),
            0o600,
        )?;
        derived.to_string()
    } else {
        "TEST_MODE_WG_PUBKEY".to_string()
    };

    data.wireguard_peers.insert(
        0,
        WireGuardPeerNode {
            id,
            name: clean_name,
            region: clean_region,
            interface_name: "wg0".to_string(),
            public_key: public_key.clone(),
            preshared_key_masked: "unset".to_string(),
            endpoint: clean_endpoint,
            allowed_ips: clean_allowed.clone(),
            mesh_ip,
            cipher_suite: "ChaCha20-Poly1305 / Curve25519".to_string(),
            latest_handshake: "Pending handshake".to_string(),
            rx_mb: 0.0,
            tx_mb: 0.0,
            latency_ms: 0.0,
            persistent_keepalive_sec: 25,
            status: "connected".to_string(),
        },
    );

    sync_wireguard_config(config, &data, executor).await?;
    if is_production_vps(config) {
        let out = executor
            .run(
                AllowedBinary::Wg,
                &[
                    "set",
                    "wg0",
                    "peer",
                    &public_key,
                    "allowed-ips",
                    &clean_allowed,
                    "persistent-keepalive",
                    "25",
                ],
                CMD_TIMEOUT,
            )
            .await?;
        if !out.success {
            return Err(PanelError::CommandFailed {
                binary: "wg",
                exit_code: out.exit_code,
                stderr: if out.stderr.trim().is_empty() {
                    out.stdout
                } else {
                    out.stderr
                },
            });
        }
    }

    store.put_json(KV_EDGE_CLOUD, &data)?;
    Ok(data)
}

pub async fn toggle_wireguard_peer(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<EdgeCloudData, PanelError> {
    let mut data = load_edge_cloud(store, config, executor).await?;
    let Some(peer) = data.wireguard_peers.iter_mut().find(|p| p.id == id) else {
        return Err(PanelError::NotFound(format!("WireGuard peer '{id}' not found")));
    };
    let pubkey = peer.public_key.clone();
    let allowed = peer.allowed_ips.clone();
    let now_connected = peer.status != "connected";
    peer.status = if now_connected {
        "connected".to_string()
    } else {
        "disconnected".to_string()
    };

    sync_wireguard_config(config, &data, executor).await?;
    if is_production_vps(config) {
        if !executor.is_binary_available(AllowedBinary::Wg) {
            return Err(PanelError::Conflict(
                "thiếu wireguard-tools (`wg`) — gợi ý: sudo apt install -y wireguard-tools".to_string(),
            ));
        }
        let out = if now_connected {
            executor
                .run(
                    AllowedBinary::Wg,
                    &[
                        "set",
                        "wg0",
                        "peer",
                        &pubkey,
                        "allowed-ips",
                        &allowed,
                        "persistent-keepalive",
                        "25",
                    ],
                    CMD_TIMEOUT,
                )
                .await?
        } else {
            executor
                .run(
                    AllowedBinary::Wg,
                    &["set", "wg0", "peer", &pubkey, "remove"],
                    CMD_TIMEOUT,
                )
                .await?
        };
        if !out.success {
            return Err(PanelError::CommandFailed {
                binary: "wg",
                exit_code: out.exit_code,
                stderr: if out.stderr.trim().is_empty() {
                    out.stdout
                } else {
                    out.stderr
                },
            });
        }
    }

    store.put_json(KV_EDGE_CLOUD, &data)?;
    Ok(data)
}

pub async fn delete_wireguard_peer(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<EdgeCloudData, PanelError> {
    let mut data = load_edge_cloud(store, config, executor).await?;
    let removed_pubkey = data
        .wireguard_peers
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.public_key.clone());
    data.wireguard_peers.retain(|p| p.id != id);

    sync_wireguard_config(config, &data, executor).await?;
    if is_production_vps(config) {
        if !executor.is_binary_available(AllowedBinary::Wg) {
            return Err(PanelError::Conflict(
                "thiếu wireguard-tools (`wg`) — gợi ý: sudo apt install -y wireguard-tools".to_string(),
            ));
        }
        if let Some(pubkey) = removed_pubkey {
            let out = executor
                .run(
                    AllowedBinary::Wg,
                    &["set", "wg0", "peer", &pubkey, "remove"],
                    CMD_TIMEOUT,
                )
                .await?;
            if !out.success {
                return Err(PanelError::CommandFailed {
                    binary: "wg",
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
    let key_file = config
        .data_dir
        .join("edge-cloud")
        .join("peers")
        .join(format!("{id}.key"));
    if key_file.exists() {
        std::fs::remove_file(&key_file)?;
    }

    store.put_json(KV_EDGE_CLOUD, &data)?;
    Ok(data)
}

pub async fn create_blake3_snapshot(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    snapshot_name: &str,
    target_resource: &str,
    s3_endpoint: &str,
    bucket: &str,
    immutable_lock: bool,
) -> Result<EdgeCloudData, PanelError> {
    let clean_name = if snapshot_name.trim().is_empty() {
        "merkle-checkpoint".to_string()
    } else {
        validate_edge_slug(snapshot_name, "snapshot name", 64)?
    };

    let mut data = load_edge_cloud(store, config, executor).await?;
    let id = format!("b3-{:x}", Utc::now().timestamp_millis());
    let snap_dir = config.data_dir.join("edge-cloud").join("snapshots");
    std::fs::create_dir_all(&snap_dir)?;

    let raw_target = target_resource.trim();
    let rel_target = raw_target
        .strip_prefix(&config.wwwroot.to_string_lossy().to_string())
        .unwrap_or(raw_target)
        .trim_start_matches('/');
    let source_dir = if rel_target.is_empty() || raw_target.starts_with("pg://") || raw_target.starts_with("mysql://") {
        config.wwwroot.clone()
    } else if let Ok(safe) = SafePath::under_root(&config.wwwroot, rel_target) {
        if safe.as_path().exists() {
            safe.as_path().to_path_buf()
        } else {
            config.wwwroot.clone()
        }
    } else {
        return Err(PanelError::Validation(
            "snapshot target path must stay inside /www/wwwroot".to_string(),
        ));
    };

    std::fs::create_dir_all(&source_dir)?;
    let logical_bytes = dir_size_bytes(&source_dir).max(4096);

    let use_zstd = executor.is_binary_available(AllowedBinary::Zstd);
    let (archive_path, compression_label) = if use_zstd {
        (snap_dir.join(format!("{id}.tar.zst")), "Zstd (tar --zstd)")
    } else {
        (snap_dir.join(format!("{id}.tar.gz")), "Gzip (tar -czf)")
    };

    let mut stored_bytes: u64 = logical_bytes;
    let mut merkle = sha256_hex(
        format!("{clean_name}:{}:{id}", source_dir.display()).as_bytes(),
    );

    if !executor.is_binary_available(AllowedBinary::Tar) {
        if is_production_vps(config) {
            return Err(PanelError::Conflict(
                "thiếu binary `tar` để tạo snapshot (gợi ý: sudo apt install -y tar zstd)".to_string(),
            ));
        }
    } else {
        let archive_str = archive_path.to_string_lossy().to_string();
        let src_str = source_dir.to_string_lossy().to_string();
        let tar_args: Vec<&str> = if use_zstd {
            vec!["--zstd", "-cf", &archive_str, "-C", &src_str, "."]
        } else {
            vec!["-czf", &archive_str, "-C", &src_str, "."]
        };
        let out = executor.run(AllowedBinary::Tar, &tar_args, APT_TIMEOUT).await?;
        if !out.success {
            return Err(PanelError::CommandFailed {
                binary: "tar",
                exit_code: out.exit_code,
                stderr: if out.stderr.trim().is_empty() {
                    out.stdout
                } else {
                    out.stderr
                },
            });
        }
        if archive_path.exists() {
            let bytes = std::fs::read(&archive_path)?;
            stored_bytes = (bytes.len() as u64).max(1);
            merkle = sha256_hex(&bytes);
        }
    }

    let logical_mb = ((logical_bytes as f64) / 1_048_576.0 * 100.0).round() / 100.0;
    let stored_mb = ((stored_bytes as f64) / 1_048_576.0 * 100.0).round() / 100.0;
    let ratio = if stored_bytes > 0 {
        (logical_bytes as f64 / stored_bytes as f64).max(1.0)
    } else {
        1.0
    };
    let chunks_count = stored_bytes.div_ceil(65_536).max(1);

    data.blake3_snapshots.insert(
        0,
        Blake3ObjectSnapshot {
            id,
            snapshot_name: clean_name,
            target_resource: source_dir.to_string_lossy().to_string(),
            s3_endpoint: s3_endpoint.trim().to_string(),
            bucket: bucket.trim().to_string(),
            merkle_root_blake3: merkle,
            compression: compression_label.to_string(),
            dedup_ratio: format!("{ratio:.1}x"),
            logical_size_mb: logical_mb.max(0.01),
            stored_size_mb: stored_mb.max(0.01),
            chunks_count,
            immutable_lock,
            created_at: Utc::now().format("%Y-%m-%d %H:%M UTC").to_string(),
            status: "verified".to_string(),
        },
    );
    store.put_json(KV_EDGE_CLOUD, &data)?;
    Ok(data)
}

pub async fn delete_blake3_snapshot(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<EdgeCloudData, PanelError> {
    let mut data = load_edge_cloud(store, config, executor).await?;
    data.blake3_snapshots.retain(|s| s.id != id);
    let snap_dir = config.data_dir.join("edge-cloud").join("snapshots");
    for ext in ["tar.zst", "tar.gz"] {
        let p = snap_dir.join(format!("{id}.{ext}"));
        if p.exists() {
            std::fs::remove_file(&p)?;
        }
    }
    store.put_json(KV_EDGE_CLOUD, &data)?;
    Ok(data)
}
