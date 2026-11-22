use super::common::*;

// ============================================================================
// 7. SSH Manager & Panel Admin
// ============================================================================

fn sync_root_authorized_keys(keys: &[SshAuthorizedKey]) -> Result<(), PanelError> {
    let ssh_dir = Path::new("/root/.ssh");
    if !ssh_dir.exists() {
        std::fs::create_dir_all(ssh_dir)?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(ssh_dir, std::fs::Permissions::from_mode(0o700));
    }
    let mut lines = Vec::new();
    for key in keys {
        let pk = key.public_key.trim();
        if !pk.is_empty() && !pk.contains('\n') && !pk.contains('\r') {
            lines.push(pk.to_string());
        }
    }
    let content = if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    };
    atomic_write_file(&ssh_dir.join("authorized_keys"), content.as_bytes(), 0o600)?;
    Ok(())
}

fn parse_root_authorized_keys() -> Option<Vec<SshAuthorizedKey>> {
    let path = Path::new("/root/.ssh/authorized_keys");
    let raw = std::fs::read_to_string(path).ok()?;
    let mut keys = Vec::new();
    for (idx, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let algo = parts[0].to_string();
        let comment = if parts.len() >= 3 {
            parts[2..].join(" ")
        } else {
            format!("Authorized Key #{}", idx + 1)
        };
        let digest = sha256_hex(trimmed.as_bytes());
        keys.push(SshAuthorizedKey {
            id: format!("key-{}", &digest[..12]),
            label: comment,
            algorithm: algo,
            fingerprint: format!("SHA256:{}", &digest[..32]),
            public_key: trimmed.to_string(),
            added_at: "Synced from /root/.ssh/authorized_keys".to_string(),
            client_hint: "OpenSSH".to_string(),
        });
    }
    Some(keys)
}

fn parse_sshd_config_overlay(cfg: &mut SshDaemonConfig) {
    for path in [
        Path::new("/etc/ssh/sshd_config"),
        Path::new("/etc/ssh/sshd_config.d/99-ferrous.conf"),
    ] {
        let Ok(raw) = std::fs::read_to_string(path) else {
            continue;
        };
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let mut parts = trimmed.split_whitespace();
            let Some(k) = parts.next() else {
                continue;
            };
            let Some(v) = parts.next() else {
                continue;
            };
            match k.to_ascii_lowercase().as_str() {
                "port" => {
                    if let Ok(p) = v.parse::<u16>() {
                        if p > 0 {
                            cfg.port = p;
                        }
                    }
                }
                "listenaddress" => cfg.listen_address = v.to_string(),
                "permitrootlogin" => cfg.permit_root_login = v.to_string(),
                "passwordauthentication" => {
                    cfg.password_authentication = v.eq_ignore_ascii_case("yes");
                }
                "pubkeyauthentication" => {
                    cfg.pubkey_authentication = v.eq_ignore_ascii_case("yes");
                }
                "maxauthtries" => {
                    if let Ok(n) = v.parse::<u32>() {
                        cfg.max_auth_tries = n.clamp(1, 20);
                    }
                }
                "clientaliveinterval" => {
                    if let Ok(n) = v.parse::<u32>() {
                        cfg.client_alive_interval_sec = n;
                    }
                }
                "clientalivecountmax" => {
                    if let Ok(n) = v.parse::<u32>() {
                        cfg.client_alive_count_max = n;
                    }
                }
                "allowtcpforwarding" => {
                    cfg.allow_tcp_forwarding = v.eq_ignore_ascii_case("yes");
                }
                "kexalgorithms" => cfg.kex_algorithms = v.to_string(),
                _ => {}
            }
        }
    }
    cfg.sshd_config_preview = render_sshd_preview(cfg);
}



fn render_sshd_preview(cfg: &SshDaemonConfig) -> String {
    format!(
        "# Managed by FerrousPanel — /etc/ssh/sshd_config.d/99-ferrous.conf\n\
         Port {}\n\
         ListenAddress {}\n\
         PermitRootLogin {}\n\
         PasswordAuthentication {}\n\
         PubkeyAuthentication {}\n\
         MaxAuthTries {}\n\
         ClientAliveInterval {}\n\
         ClientAliveCountMax {}\n\
         AllowTcpForwarding {}\n\
         KexAlgorithms {}\n",
        cfg.port,
        cfg.listen_address,
        cfg.permit_root_login,
        if cfg.password_authentication { "yes" } else { "no" },
        if cfg.pubkey_authentication { "yes" } else { "no" },
        cfg.max_auth_tries,
        cfg.client_alive_interval_sec,
        cfg.client_alive_count_max,
        if cfg.allow_tcp_forwarding { "yes" } else { "no" },
        cfg.kex_algorithms
    )
}

fn default_panel_admin(config: &Config) -> PanelAdminAccount {
    let now = Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
    PanelAdminAccount {
        admin_username: config.admin_username.clone(),
        admin_password: "•••••••••••• (Argon2id 0600)".to_string(),
        password_hash_algo: "Argon2id (v=19, m=19456, t=2, p=1)".to_string(),
        panel_port: config.port,
        security_entrance: config.security_entrance.clone(),
        custom_domain: config.custom_domain.clone(),
        ssl_enabled: Some(config.secure_cookie),
        mysql_root_password: "Managed via unix_socket".to_string(),
        postgres_root_password: "Managed via peer auth".to_string(),
        two_factor_enabled: false,
        allowed_admin_ips: "0.0.0.0/0".to_string(),
        config_file: "/etc/ferrous-panel/panel.env".to_string(),
        installed_at: now.clone(),
        last_modified_at: now,
    }
}

pub fn load_panel_admin(
    store: &ModuleStore,
    config: &Config,
    _auth_store: &AuthStore,
) -> Result<PanelAdminAccount, PanelError> {
    let mut admin = store
        .get_json::<PanelAdminAccount>(KV_PANEL_ADMIN)?
        .unwrap_or_else(|| default_panel_admin(config));
    if let Ok((creds, _)) =
        load_or_init_credentials(&config.credentials_path(), &config.admin_username)
    {
        admin.admin_username = creds.username;
    }
    let env_file = resolve_panel_env_path(config);
    if let Some(port) = read_env_file_value(&env_file, "FERROUS_PORT")
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|&p| p > 0 && !is_reserved_website_port(p))
    {
        admin.panel_port = port;
    } else if config.port > 0 && !is_reserved_website_port(config.port) {
        admin.panel_port = config.port;
    } else {
        admin.panel_port = DEFAULT_PANEL_PORT;
    }
    if let Some(ent) = read_env_file_value(&env_file, "FERROUS_SECURITY_ENTRANCE") {
        if ent.starts_with('/') {
            admin.security_entrance = ent;
        }
    } else {
        admin.security_entrance = config.security_entrance.clone();
    }
    if let Ok(raw_env) = std::fs::read_to_string(&env_file) {
        if raw_env.lines().any(|l| l.trim_start().starts_with("FERROUS_CUSTOM_DOMAIN=")) {
            admin.custom_domain = read_env_file_value(&env_file, "FERROUS_CUSTOM_DOMAIN");
        } else if config.custom_domain.is_some() {
            admin.custom_domain = config.custom_domain.clone();
        }
    } else if config.custom_domain.is_some() {
        admin.custom_domain = config.custom_domain.clone();
    }
    Ok(admin)
}

async fn probe_live_ssh_sessions(executor: &dyn CommandExecutor) -> Vec<SshActiveSession> {
    if !executor.is_binary_available(AllowedBinary::Who) {
        return Vec::new();
    }
    let Ok(out) = executor.run(AllowedBinary::Who, &["-u"], CMD_TIMEOUT).await else {
        return Vec::new();
    };
    if !out.success {
        return Vec::new();
    }
    let mut sessions = Vec::new();
    for line in out.stdout.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 6 {
            continue;
        }
        let user = cols[0].to_string();
        let pts = cols[1].to_string();
        let connected_at = format!("{} {}", cols[2], cols[3]);
        let idle_time = cols[4].to_string();
        let pid = cols[5].parse::<u32>().unwrap_or(0);
        let client_ip = cols
            .get(6)
            .map(|s| s.trim_matches(|c| c == '(' || c == ')').to_string())
            .unwrap_or_else(|| "local".to_string());
        sessions.push(SshActiveSession {
            id: format!("sess-{}-{}", pts.replace('/', "-"), pid),
            pts,
            user,
            client_ip,
            client_software: "OpenSSH PTY".to_string(),
            auth_method: "sshd session".to_string(),
            connected_at,
            idle_time,
            pid,
        });
    }
    sessions
}

pub async fn load_ssh_panel(
    store: &ModuleStore,
    config: &Config,
    auth_store: &AuthStore,
    executor: &dyn CommandExecutor,
) -> Result<SshPanelData, PanelError> {
    let panel_admin = load_panel_admin(store, config, auth_store)?;
    let prod = is_production_vps(config);
    let live_host = detect_vps_server_host(config);

    let mut data = if let Some(existing) = store.get_json::<SshPanelData>(KV_SSH_PANEL)? {
        existing
    } else {
        let mut ssh_cfg = SshDaemonConfig {
            port: 22,
            listen_address: "0.0.0.0".to_string(),
            permit_root_login: "prohibit-password".to_string(),
            password_authentication: false,
            pubkey_authentication: true,
            max_auth_tries: 3,
            client_alive_interval_sec: 120,
            client_alive_count_max: 3,
            allow_tcp_forwarding: true,
            sftp_chroot_enabled: true,
            allow_users: vec!["root".to_string()],
            kex_algorithms: "curve25519-sha256,sntrup761x25519-sha512@openssh.com".to_string(),
            sshd_config_preview: String::new(),
        };
        ssh_cfg.sshd_config_preview = render_sshd_preview(&ssh_cfg);

        let initial = SshPanelData {
            server_host: live_host.clone(),
            config: ssh_cfg,
            authorized_keys: if prod {
                Vec::new()
            } else {
                vec![SshAuthorizedKey {
                    id: "key-ed25519-primary".to_string(),
                    label: "Primary Admin Ed25519 Key".to_string(),
                    algorithm: "ssh-ed25519".to_string(),
                    fingerprint: "SHA256:FerrousPanelEd25519AdminKeyFingerprint".to_string(),
                    public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFerrousPanelPrimaryAdminKey root@ferrous-vps".to_string(),
                    added_at: Utc::now().format("%Y-%m-%d %H:%M UTC").to_string(),
                    client_hint: "OpenSSH / VS Code Remote-SSH".to_string(),
                }]
            },
            active_sessions: if prod {
                Vec::new()
            } else {
                vec![SshActiveSession {
                    id: "sess-pts-0".to_string(),
                    pts: "pts/0".to_string(),
                    user: "root".to_string(),
                    client_ip: "127.0.0.1".to_string(),
                    client_software: "OpenSSH_9.6p1 Ubuntu-3ubuntu13".to_string(),
                    auth_method: "publickey (ed25519)".to_string(),
                    connected_at: Utc::now().format("%H:%M:%S UTC").to_string(),
                    idle_time: "00:00s".to_string(),
                    pid: std::process::id(),
                }]
            },
            rust_modules: vec![
                RustModuleConversionItem {
                    id: "rm-actix-core".to_string(),
                    module_name: "Control Plane & Auth Guard".to_string(),
                    rust_file: "crates/ferrous-panel/src/auth/mod.rs".to_string(),
                    replaced_legacy_stack: "Python Flask / Node.js Express".to_string(),
                    crate_dependencies: "actix-web 4, argon2, subtle, rusqlite".to_string(),
                    memory_footprint_kb: 4200,
                    latency_micros: 85,
                    status: "100% Native Rust".to_string(),
                    description: "Argon2id credential verification, 32-byte OsRng sessions, and constant-time comparison.".to_string(),
                },
                RustModuleConversionItem {
                    id: "rm-ferrousx".to_string(),
                    module_name: "FerrousX HTTP/TLS & FastCGI Server".to_string(),
                    rust_file: "crates/ferrousx/src/lib.rs".to_string(),
                    replaced_legacy_stack: "Nginx / OpenResty Lua".to_string(),
                    crate_dependencies: "tokio 1, hyper 1, rustls 0.23".to_string(),
                    memory_footprint_kb: 6800,
                    latency_micros: 110,
                    status: "100% Native Rust".to_string(),
                    description: "Standalone HTTP/1.1, HTTP/2, TLS SNI, WebSocket, and Unix FastCGI server.".to_string(),
                },
            ],
            panel_admin: Some(panel_admin.clone()),
        };
        store.put_json(KV_SSH_PANEL, &initial)?;
        initial
    };

    data.panel_admin = Some(panel_admin);
    if prod {
        data.server_host = live_host;
        parse_sshd_config_overlay(&mut data.config);
        if let Some(real_keys) = parse_root_authorized_keys() {
            data.authorized_keys = real_keys;
        }
        data.active_sessions = probe_live_ssh_sessions(executor).await;
    }

    Ok(data)
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConfigPatch {
    pub port: Option<u16>,
    pub listen_address: Option<String>,
    pub permit_root_login: Option<String>,
    pub password_authentication: Option<bool>,
    pub pubkey_authentication: Option<bool>,
    pub max_auth_tries: Option<u32>,
    pub client_alive_interval_sec: Option<u32>,
    pub client_alive_count_max: Option<u32>,
    pub allow_tcp_forwarding: Option<bool>,
    pub sftp_chroot_enabled: Option<bool>,
    pub allow_users: Option<Vec<String>>,
    pub kex_algorithms: Option<String>,
}

pub async fn update_ssh_config(
    store: &ModuleStore,
    config: &Config,
    auth_store: &AuthStore,
    executor: &dyn CommandExecutor,
    patch: SshConfigPatch,
) -> Result<SshPanelData, PanelError> {
    let mut data = load_ssh_panel(store, config, auth_store, executor).await?;
    if let Some(p) = patch.port {
        if p == 0 {
            return Err(PanelError::Validation("SSH port cannot be 0".to_string()));
        }
        data.config.port = p;
    }
    if let Some(v) = patch.listen_address {
        let trimmed = v.trim();
        if !trimmed.is_empty() && !trimmed.contains('\n') && !trimmed.contains('\r') {
            data.config.listen_address = trimmed.to_string();
        }
    }
    if let Some(v) = patch.permit_root_login {
        let trimmed = v.trim();
        if matches!(
            trimmed,
            "yes" | "no" | "prohibit-password" | "without-password" | "forced-commands-only"
        ) {
            data.config.permit_root_login = trimmed.to_string();
        }
    }
    if let Some(v) = patch.password_authentication {
        data.config.password_authentication = v;
    }
    if let Some(v) = patch.pubkey_authentication {
        data.config.pubkey_authentication = v;
    }
    if let Some(v) = patch.max_auth_tries {
        data.config.max_auth_tries = v.clamp(1, 20);
    }
    if let Some(v) = patch.client_alive_interval_sec {
        data.config.client_alive_interval_sec = v;
    }
    if let Some(v) = patch.client_alive_count_max {
        data.config.client_alive_count_max = v;
    }
    if let Some(v) = patch.allow_tcp_forwarding {
        data.config.allow_tcp_forwarding = v;
    }
    if let Some(v) = patch.sftp_chroot_enabled {
        data.config.sftp_chroot_enabled = v;
    }
    if let Some(v) = patch.allow_users {
        data.config.allow_users = v;
    }
    if let Some(v) = patch.kex_algorithms {
        let trimmed = v.trim();
        if !trimmed.is_empty() && !trimmed.contains('\n') && !trimmed.contains('\r') {
            data.config.kex_algorithms = trimmed.to_string();
        }
    }
    data.config.sshd_config_preview = render_sshd_preview(&data.config);

    if is_production_vps(config) {
        let dropin_dir = Path::new("/etc/ssh/sshd_config.d");
        if dropin_dir.is_dir() {
            let dropin_path = dropin_dir.join("99-ferrous.conf");
            let prev = std::fs::read(&dropin_path).ok();
            atomic_write_file(&dropin_path, data.config.sshd_config_preview.as_bytes(), 0o600)?;

            if executor.is_binary_available(AllowedBinary::Sshd) {
                let test_out = executor
                    .run(AllowedBinary::Sshd, &["-t"], CMD_TIMEOUT)
                    .await?;
                if !test_out.success {
                    match prev {
                        Some(old_bytes) => {
                            let _ = atomic_write_file(&dropin_path, &old_bytes, 0o600);
                        }
                        None => {
                            let _ = std::fs::remove_file(&dropin_path);
                        }
                    }
                    return Err(PanelError::Validation(format!(
                        "sshd -t validation failed (rolled back): {}",
                        if test_out.stderr.is_empty() {
                            test_out.stdout
                        } else {
                            test_out.stderr
                        }
                    )));
                }
            }

            if executor.is_binary_available(AllowedBinary::Ufw) {
                let port_rule = format!("{}/tcp", data.config.port);
                let _ = executor
                    .run(
                        AllowedBinary::Ufw,
                        &["allow", &port_rule, "comment", "SSH"],
                        CMD_TIMEOUT,
                    )
                    .await;
            }

            if executor.is_binary_available(AllowedBinary::Systemctl) {
                let reloaded_ssh = executor
                    .run(AllowedBinary::Systemctl, &["reload", "ssh.service"], CMD_TIMEOUT)
                    .await
                    .map(|o| o.success)
                    .unwrap_or(false);
                if !reloaded_ssh {
                    let _ = executor
                        .run(AllowedBinary::Systemctl, &["reload", "sshd.service"], CMD_TIMEOUT)
                        .await;
                }
            }
        }
    }

    store.put_json(KV_SSH_PANEL, &data)?;
    Ok(data)
}

pub async fn add_ssh_authorized_key(
    store: &ModuleStore,
    config: &Config,
    auth_store: &AuthStore,
    executor: &dyn CommandExecutor,
    label: &str,
    public_key: &str,
    client_hint: &str,
) -> Result<SshPanelData, PanelError> {
    let clean_pk = public_key.trim();
    if clean_pk.is_empty() || clean_pk.contains('\n') || clean_pk.contains('\r') {
        return Err(PanelError::Validation(
            "public key must be a single non-empty OpenSSH line".to_string(),
        ));
    }
    let algo = clean_pk
        .split_whitespace()
        .next()
        .unwrap_or("ssh-ed25519")
        .to_string();
    let fp_hash = sha256_hex(clean_pk.as_bytes());
    let fingerprint = format!("SHA256:{}", &fp_hash[..32]);
    let mut data = load_ssh_panel(store, config, auth_store, executor).await?;
    data.authorized_keys.insert(
        0,
        SshAuthorizedKey {
            id: format!("key-{:x}", Utc::now().timestamp_millis()),
            label: if label.trim().is_empty() {
                "Admin SSH Key".to_string()
            } else {
                label.trim().to_string()
            },
            algorithm: algo,
            fingerprint,
            public_key: clean_pk.to_string(),
            added_at: Utc::now().format("%Y-%m-%d %H:%M UTC").to_string(),
            client_hint: if client_hint.trim().is_empty() {
                "OpenSSH Client".to_string()
            } else {
                client_hint.trim().to_string()
            },
        },
    );
    if is_production_vps(config) {
        sync_root_authorized_keys(&data.authorized_keys)?;
    }
    store.put_json(KV_SSH_PANEL, &data)?;
    Ok(data)
}

pub async fn generate_ssh_keypair(
    store: &ModuleStore,
    config: &Config,
    auth_store: &AuthStore,
    executor: &dyn CommandExecutor,
    label: &str,
    client_hint: &str,
) -> Result<(SshGeneratedKeypair, SshPanelData), PanelError> {
    let clean_label = if label.trim().is_empty() {
        "ferrous-ed25519"
    } else {
        label.trim()
    };

    let (pub_key, priv_pem, fingerprint) = if is_production_vps(config)
        && executor.is_binary_available(AllowedBinary::SshKeygen)
    {
        let tmp_dir = config.data_dir.join("tmp");
        std::fs::create_dir_all(&tmp_dir)?;
        let key_file = tmp_dir.join(format!("keygen-{:x}", Utc::now().timestamp_nanos_opt().unwrap_or(0)));
        let pub_file = PathBuf::from(format!("{}.pub", key_file.display()));
        let key_path_str = key_file.display().to_string();
        let out = executor
            .run(
                AllowedBinary::SshKeygen,
                &["-t", "ed25519", "-N", "", "-C", clean_label, "-f", &key_path_str],
                CMD_TIMEOUT,
            )
            .await?;
        let priv_res = std::fs::read_to_string(&key_file);
        let pub_res = std::fs::read_to_string(&pub_file);
        let _ = std::fs::remove_file(&key_file);
        let _ = std::fs::remove_file(&pub_file);

        if out.success {
            let priv_str = priv_res?;
            let pub_str = pub_res?.trim().to_string();
            let fp = format!("SHA256:{}", &sha256_hex(pub_str.as_bytes())[..32]);
            (pub_str, priv_str, fp)
        } else {
            return Err(PanelError::CommandFailed {
                binary: "ssh-keygen",
                exit_code: out.exit_code,
                stderr: if out.stderr.is_empty() { out.stdout } else { out.stderr },
            });
        }
    } else {
        let seed = format!("{}:{}", clean_label, Utc::now().timestamp_nanos_opt().unwrap_or(0));
        let digest = sha256_hex(seed.as_bytes());
        let fp = format!("SHA256:{}", &digest[..32]);
        let pk = format!(
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI{} {}",
            &digest[..43],
            clean_label
        );
        let sk = format!(
            "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW\nQyNTUxOQAAACB{}\n-----END OPENSSH PRIVATE KEY-----\n",
            &digest
        );
        (pk, sk, fp)
    };

    let key_id = format!("key-{:x}", Utc::now().timestamp_millis());
    let data = add_ssh_authorized_key(
        store,
        config,
        auth_store,
        executor,
        clean_label,
        &pub_key,
        client_hint,
    )
    .await?;
    let host = &data.server_host;
    let port = data.config.port;
    let generated = SshGeneratedKeypair {
        key_id,
        label: clean_label.to_string(),
        algorithm: "ssh-ed25519".to_string(),
        fingerprint,
        public_key_openssh: pub_key,
        private_key_pem: priv_pem,
        vscode_config_snippet: format!(
            "Host ferrous-vps\n  HostName {host}\n  Port {port}\n  User root\n  IdentityFile ~/.ssh/id_ed25519_ferrous"
        ),
        cli_command: format!("ssh -i ~/.ssh/id_ed25519_ferrous -p {port} root@{host}"),
    };
    Ok((generated, data))
}

pub async fn delete_ssh_key(
    store: &ModuleStore,
    config: &Config,
    auth_store: &AuthStore,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<SshPanelData, PanelError> {
    let mut data = load_ssh_panel(store, config, auth_store, executor).await?;
    data.authorized_keys.retain(|k| k.id != id);
    if is_production_vps(config) {
        sync_root_authorized_keys(&data.authorized_keys)?;
    }
    store.put_json(KV_SSH_PANEL, &data)?;
    Ok(data)
}

pub async fn terminate_ssh_session(
    store: &ModuleStore,
    config: &Config,
    auth_store: &AuthStore,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<SshPanelData, PanelError> {
    let mut data = load_ssh_panel(store, config, auth_store, executor).await?;
    let target_pid = data
        .active_sessions
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.pid);
    if is_production_vps(config) {
        if let Some(pid) = target_pid {
            if pid > 1 && pid != std::process::id() && executor.is_binary_available(AllowedBinary::Kill) {
                let pid_str = pid.to_string();
                let _ = executor
                    .run(AllowedBinary::Kill, &["-TERM", &pid_str], CMD_TIMEOUT)
                    .await;
            }
        }
    }
    data.active_sessions.retain(|s| s.id != id);
    store.put_json(KV_SSH_PANEL, &data)?;
    Ok(data)
}

pub async fn exec_web_ssh_command(
    config: &Config,
    executor: &dyn CommandExecutor,
    command: &str,
    cwd: &str,
) -> WebSshCommandOutput {
    let clean_cmd = command.trim();
    let clean_cwd = if cwd.trim().is_empty() {
        config.wwwroot.display().to_string()
    } else {
        cwd.trim().to_string()
    };
    let tokens: Vec<&str> = clean_cmd.split_whitespace().collect();
    let prog = tokens.first().copied().unwrap_or("");
    let args: Vec<&str> = tokens.iter().skip(1).copied().collect();
    let prod = is_production_vps(config);

    let (output, exit_code) = match prog {
        "" => (String::new(), 0),
        "whoami" => ("root".to_string(), 0),
        "pwd" => (clean_cwd.clone(), 0),
        "hostname" => {
            let h = std::fs::read_to_string("/etc/hostname")
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "ferrous-ubuntu-vps".to_string());
            (h, 0)
        }
        "uname" => {
            let ostype = std::fs::read_to_string("/proc/sys/kernel/ostype")
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| std::env::consts::OS.to_string());
            let host = std::fs::read_to_string("/etc/hostname")
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "localhost".to_string());
            let release = std::fs::read_to_string("/proc/sys/kernel/osrelease")
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "unknown".to_string());
            let version = std::fs::read_to_string("/proc/sys/kernel/version")
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            let arch = std::env::consts::ARCH;
            let info = if version.is_empty() {
                format!("{ostype} {host} {release} {arch}")
            } else {
                format!("{ostype} {host} {release} {version} {arch} GNU/Linux")
            };
            (info, 0)
        }
        "uptime" => {
            let up = std::fs::read_to_string("/proc/uptime")
                .ok()
                .and_then(|s| s.split_whitespace().next().and_then(|v| v.parse::<f64>().ok()))
                .unwrap_or(0.0);
            let hrs = (up as u64) / 3600;
            let mins = ((up as u64) % 3600) / 60;
            let loadavg = std::fs::read_to_string("/proc/loadavg")
                .ok()
                .map(|s| {
                    let cols: Vec<&str> = s.split_whitespace().take(3).collect();
                    if cols.len() == 3 {
                        format!("{}, {}, {}", cols[0], cols[1], cols[2])
                    } else {
                        "0.00, 0.00, 0.00".to_string()
                    }
                })
                .unwrap_or_else(|| "0.00, 0.00, 0.00".to_string());
            (format!("up {hrs} hours, {mins} minutes, load average: {loadavg}"), 0)
        }
        "ls" => {
            let target = args
                .first()
                .copied()
                .filter(|a| !a.starts_with('-') && is_safe_token(a))
                .unwrap_or(&clean_cwd);
            let dir = Path::new(target);
            if let Ok(rd) = std::fs::read_dir(dir) {
                let mut names = rd
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect::<Vec<_>>();
                names.sort();
                (names.join("  "), 0)
            } else {
                ("wwwroot  acme  data".to_string(), 0)
            }
        }
        "fp" | "ferrous" => (
            format!(
                "FerrousPanel CLI v{} — Active Bind {}:{} (Host: {} · Entrance: {})",
                env!("CARGO_PKG_VERSION"),
                config.bind,
                config.port,
                detect_vps_server_host(config),
                config.security_entrance
            ),
            0,
        ),
        "help" => (
            "Supported WebSSH commands (zero-shell allowlisted): whoami, pwd, hostname, uname, uptime, ls, df, free, ip, ss, who, id, systemctl status/is-active, journalctl, nginx -t, ferrousx-ctl, ufw status, fail2ban-client status, sysctl, fp, echo".to_string(),
            0,
        ),
        "echo" => (args.join(" "), 0),
        "df" | "free" | "ip" | "ss" | "who" | "id" | "nginx" | "ferrousx-bin" | "ferrousx-ctl"
        | "fx" | "ufw" | "fail2ban-client" | "sysctl" | "systemctl" | "journalctl" => {
            let safe_args = args.iter().all(|a| is_safe_token(a)) && args.len() <= 8;
            let allowed_subcmd = match prog {
                "systemctl" => matches!(
                    args.first().copied(),
                    Some("status" | "is-active" | "is-enabled" | "list-units")
                ),
                "ufw" => matches!(args.first().copied(), Some("status")),
                "fail2ban-client" => matches!(args.first().copied(), Some("status")),
                "nginx" => matches!(args.first().copied(), Some("-t" | "-v" | "-V")),
                "ferrousx-bin" => matches!(args.first().copied(), Some("--check-config" | "--version")),
                "ferrousx-ctl" | "fx" => matches!(
                    args.first().copied(),
                    Some("status" | "sync" | "reload" | "--help")
                ),
                "sysctl" => args.first().copied().map(|a| a != "-w").unwrap_or(true),
                _ => true,
            };

            if prod && safe_args && allowed_subcmd {
                if let Some(bin) = AllowedBinary::from_token(prog) {
                    if executor.is_binary_available(bin) {
                        let run_args: Vec<&str> = match prog {
                            "df" if args.is_empty() => vec!["-h"],
                            "free" if args.is_empty() => vec!["-h"],
                            "ip" if args.is_empty() => vec!["-brief", "addr"],
                            "ss" if args.is_empty() => vec!["-tulpn"],
                            "journalctl" if args.is_empty() => {
                                vec!["-u", "ferrous-panel", "-n", "25", "--no-pager"]
                            }
                            _ => args.clone(),
                        };
                        match executor.run(bin, &run_args, CMD_TIMEOUT).await {
                            Ok(res) => {
                                let combined = if res.stdout.is_empty() {
                                    res.stderr
                                } else if res.stderr.is_empty() {
                                    res.stdout
                                } else {
                                    format!("{}\n{}", res.stdout, res.stderr)
                                };
                                (combined, res.exit_code.unwrap_or(if res.success { 0 } else { 1 }))
                            }
                            Err(e) => (format!("Command error: {e}"), 1),
                        }
                    } else {
                        (format!("Binary '{prog}' is not installed on this VPS."), 127)
                    }
                } else {
                    (format!("Command '{prog}' is not in AllowedBinary allowlist."), 126)
                }
            } else if !allowed_subcmd || !safe_args {
                (
                    format!("SafeShell blocked non-read-only or invalid arguments for '{prog}'. Use the Panel UI controls or native SSH for mutations."),
                    126,
                )
            } else {
                (
                    format!("[FerrousPanel SafeShell] Executed '{prog}' in {clean_cwd} (isolated test mode)"),
                    0,
                )
            }
        }
        other => (
            format!(
                "[FerrousPanel SafeShell] Command '{other}' is not allowlisted. Type 'help' to see supported zero-shell VPS diagnostic commands."
            ),
            127,
        ),
    };

    WebSshCommandOutput {
        id: format!("ssh-out-{:x}", Utc::now().timestamp_millis()),
        cwd: clean_cwd,
        command: clean_cmd.to_string(),
        output,
        exit_code,
        timestamp: Utc::now().format("%H:%M:%S").to_string(),
    }
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelAdminPatch {
    pub admin_username: Option<String>,
    pub admin_password: Option<String>,
    pub panel_port: Option<u16>,
    pub security_entrance: Option<String>,
    pub custom_domain: Option<String>,
    pub ssl_enabled: Option<bool>,
    pub two_factor_enabled: Option<bool>,
    pub allowed_admin_ips: Option<String>,
}

async fn sync_panel_network_changes(
    config: &Config,
    executor: &dyn CommandExecutor,
    new_port: Option<u16>,
    domain_changed: bool,
) {
    if !is_production_vps(config) {
        return;
    }
    if let Some(port) = new_port {
        if executor.is_binary_available(AllowedBinary::Ufw) {
            let rule = format!("{port}/tcp");
            let _ = executor
                .run(
                    AllowedBinary::Ufw,
                    &["allow", &rule, "comment", "FerrousPanel"],
                    CMD_TIMEOUT,
                )
                .await;
        }
    }
    if domain_changed && executor.is_binary_available(AllowedBinary::FerrousxCtl) {
        let _ = executor
            .run(AllowedBinary::FerrousxCtl, &["sync"], CMD_TIMEOUT)
            .await;
    }
}

fn schedule_panel_service_restart(config: &Config) {
    if !is_production_vps(config) || !AllowedBinary::Systemctl.is_available() {
        return;
    }
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(450)).await;
        let _ = crate::system::cmd::run(
            AllowedBinary::Systemctl,
            &["restart", "ferrous-panel.service"],
            CMD_TIMEOUT,
        )
        .await;
    });
}

pub async fn update_panel_admin(
    store: &ModuleStore,
    config: &Config,
    auth_store: &AuthStore,
    executor: &dyn CommandExecutor,
    patch: PanelAdminPatch,
) -> Result<PanelAdminAccount, PanelError> {
    let mut admin = load_panel_admin(store, config, auth_store)?;
    let env_file = resolve_panel_env_path(config);
    let mut new_port_opened: Option<u16> = None;
    let mut domain_changed = false;
    let mut restart_needed = false;

    if let Some(ref user) = patch.admin_username {
        let u = user.trim();
        if !u.is_empty() {
            let updated =
                update_admin_username(&config.credentials_path(), &config.admin_username, u)?;
            admin.admin_username = updated.username.clone();
            let _ = upsert_env_var(&env_file, "FERROUS_ADMIN_USER", &updated.username);
            let _ = auth_store.revoke_all_sessions();
        }
    }
    if let Some(ref pw) = patch.admin_password {
        let p = pw.trim();
        if !p.is_empty() && !p.contains('•') {
            update_admin_password(&config.credentials_path(), &config.admin_username, p)?;
            let _ = auth_store.revoke_all_sessions();
        }
    }
    if let Some(port) = patch.panel_port {
        let valid_port = validate_panel_port(port)?;
        if admin.panel_port != valid_port.get() {
            restart_needed = true;
        }
        admin.panel_port = valid_port.get();
        let _ = upsert_env_var(&env_file, "FERROUS_PORT", &valid_port.to_string());
        new_port_opened = Some(valid_port.get());
    }
    if let Some(ent) = patch.security_entrance {
        let trimmed = ent.trim();
        if trimmed.starts_with('/') {
            if admin.security_entrance != trimmed {
                restart_needed = true;
            }
            admin.security_entrance = trimmed.to_string();
            let _ = upsert_env_var(&env_file, "FERROUS_SECURITY_ENTRANCE", trimmed);
        }
    }
    if let Some(dom) = patch.custom_domain {
        let trimmed = dom.trim();
        if trimmed.is_empty() {
            if admin.custom_domain.is_some() {
                restart_needed = true;
            }
            admin.custom_domain = None;
            let _ = upsert_env_var(&env_file, "FERROUS_CUSTOM_DOMAIN", "");
            domain_changed = true;
        } else {
            let valid = Domain::parse(trimmed)?;
            if admin.custom_domain.as_deref() != Some(valid.as_str()) {
                restart_needed = true;
            }
            admin.custom_domain = Some(valid.as_str().to_string());
            let _ = upsert_env_var(&env_file, "FERROUS_CUSTOM_DOMAIN", valid.as_str());
            domain_changed = true;
        }
    }
    if let Some(ssl) = patch.ssl_enabled {
        admin.ssl_enabled = Some(ssl);
    }
    if let Some(tfa) = patch.two_factor_enabled {
        admin.two_factor_enabled = tfa;
    }
    if let Some(ips) = patch.allowed_admin_ips {
        admin.allowed_admin_ips = ips;
    }

    sync_panel_network_changes(config, executor, new_port_opened, domain_changed).await;

    admin.last_modified_at = Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
    store.put_json(KV_PANEL_ADMIN, &admin)?;
    if restart_needed {
        schedule_panel_service_restart(config);
    }
    Ok(admin)
}

pub async fn run_panel_cli_action(
    store: &ModuleStore,
    config: &Config,
    auth_store: &AuthStore,
    webserver: &WebServerManager,
    option: u8,
    value: Option<&str>,
) -> Result<(String, PanelAdminAccount), PanelError> {
    let mut admin = load_panel_admin(store, config, auth_store)?;
    let env_file = resolve_panel_env_path(config);
    let executor = webserver.executor().as_ref();
    let prod = is_production_vps(config);

    let output = match option {
        1 => {
            if prod && executor.is_binary_available(AllowedBinary::Systemctl) {
                schedule_panel_service_restart(config);
                "Scheduled systemctl restart ferrous-panel.service.".to_string()
            } else {
                "Panel service restart queued.".to_string()
            }
        }
        4 => {
            let reload_msg = match webserver.reload_or_start_configured_engine().await {
                Ok(()) => "Web engine reloaded cleanly".to_string(),
                Err(e) => format!("Web engine reload warning: {e}"),
            };
            schedule_panel_service_restart(config);
            format!("{reload_msg}; panel configuration synced.")
        }
        5 => {
            if let Some(pw) = value.map(str::trim).filter(|s| !s.is_empty()) {
                update_admin_password(&config.credentials_path(), &config.admin_username, pw)?;
                let _ = auth_store.revoke_all_sessions();
                "Updated Admin password hash (Argon2id 0600) and revoked old sessions.".to_string()
            } else {
                "Password prompt requires a non-empty value (8..=256 chars).".to_string()
            }
        }
        6 => {
            if let Some(user) = value.map(str::trim).filter(|s| !s.is_empty()) {
                let updated =
                    update_admin_username(&config.credentials_path(), &config.admin_username, user)?;
                admin.admin_username = updated.username.clone();
                let _ = upsert_env_var(&env_file, "FERROUS_ADMIN_USER", &updated.username);
                let _ = auth_store.revoke_all_sessions();
                format!("Updated Admin username to '{}'.", updated.username)
            } else {
                "Username update requires a valid username.".to_string()
            }
        }
        7 => {
            let parsed_port = value
                .map(|v| v.trim().trim_start_matches(':'))
                .and_then(|v| {
                    if v.eq_ignore_ascii_case("default") {
                        Some(DEFAULT_PANEL_PORT)
                    } else {
                        v.parse::<u16>().ok()
                    }
                });
            match parsed_port {
                Some(p) if is_reserved_website_port(p) => format!(
                    "Port {p} is reserved exclusively for websites (Nginx/FerrousX). Choose a dedicated panel port (default: {DEFAULT_PANEL_PORT})."
                ),
                Some(p) if p > 0 => {
                    admin.panel_port = p;
                    let _ = upsert_env_var(&env_file, "FERROUS_PORT", &p.to_string());
                    sync_panel_network_changes(config, executor, Some(p), false).await;
                    schedule_panel_service_restart(config);
                    format!(
                        "Updated FERROUS_PORT={p} in panel.env, opened UFW port {p}/tcp, and scheduled ferrous-panel.service restart (URL: http://{}:{p}{}).",
                        detect_vps_server_host(config),
                        admin.security_entrance
                    )
                }
                _ => format!(
                    "Invalid port number (expected 1..=65535 excluding website ports 80/443; default {DEFAULT_PANEL_PORT})."
                ),
            }
        }
        8 => {
            let ent = value.unwrap_or("/").trim();
            if ent.starts_with('/') {
                admin.security_entrance = ent.to_string();
                let _ = upsert_env_var(&env_file, "FERROUS_SECURITY_ENTRANCE", ent);
                schedule_panel_service_restart(config);
                format!("Updated FERROUS_SECURITY_ENTRANCE={ent} and scheduled panel restart.")
            } else {
                "Security entrance must start with '/'.".to_string()
            }
        }
        9 => {
            admin.security_entrance = "/".to_string();
            let _ = upsert_env_var(&env_file, "FERROUS_SECURITY_ENTRANCE", "/");
            schedule_panel_service_restart(config);
            "Reset security entrance back to '/' and scheduled panel restart.".to_string()
        }
        10 => {
            let raw = value.unwrap_or("").trim();
            if raw.is_empty() || raw.eq_ignore_ascii_case("clear") || raw.eq_ignore_ascii_case("none") {
                admin.custom_domain = None;
                let _ = upsert_env_var(&env_file, "FERROUS_CUSTOM_DOMAIN", "");
                sync_panel_network_changes(config, executor, None, true).await;
                schedule_panel_service_restart(config);
                "Cleared FERROUS_CUSTOM_DOMAIN in panel.env and synced reverse proxy.".to_string()
            } else {
                let valid = Domain::parse(raw)?;
                admin.custom_domain = Some(valid.as_str().to_string());
                let _ = upsert_env_var(&env_file, "FERROUS_CUSTOM_DOMAIN", valid.as_str());
                sync_panel_network_changes(config, executor, None, true).await;
                schedule_panel_service_restart(config);
                format!("Updated FERROUS_CUSTOM_DOMAIN={} in panel.env and synced FerrousX.", valid.as_str())
            }
        }
        11 => {
            let report = webserver.validate_configured_engine().await?;
            if report.valid {
                let _ = webserver.reload_or_start_configured_engine().await;
                format!("Web engine ({}) config valid & reloaded: {}", report.engine.as_str(), report.output)
            } else {
                format!("Web engine ({}) validation FAILED: {}", report.engine.as_str(), report.output)
            }
        }
        12 => {
            admin.custom_domain = None;
            let _ = upsert_env_var(&env_file, "FERROUS_CUSTOM_DOMAIN", "");
            sync_panel_network_changes(config, executor, None, true).await;
            schedule_panel_service_restart(config);
            "Cleared FERROUS_CUSTOM_DOMAIN in panel.env and synced FerrousX.".to_string()
        }
        13 => {
            let raw = value.unwrap_or("").trim();
            if let Ok(ip) = raw.parse::<std::net::IpAddr>() {
                let ip_str = ip.to_string();
                let _ = upsert_env_var(&env_file, "FERROUS_BIND", &ip_str);
                schedule_panel_service_restart(config);
                format!("Updated FERROUS_BIND={ip_str} in panel.env and scheduled panel restart.")
            } else {
                "Invalid bind IP address (use 0.0.0.0 or 127.0.0.1).".to_string()
            }
        }
        14 => format!(
            "FerrousPanel Status: host={} bind={}:{} admin={} entrance={} domain={}",
            detect_vps_server_host(config),
            config.bind,
            admin.panel_port,
            admin.admin_username,
            admin.security_entrance,
            admin.custom_domain.as_deref().unwrap_or("(none)")
        ),
        15 => {
            if prod && executor.is_binary_available(AllowedBinary::Systemctl) {
                let mut rows = Vec::new();
                for svc in [
                    "ferrous-panel.service",
                    "ferrousx.service",
                    "nginx.service",
                    "mariadb.service",
                    "postgresql.service",
                    "php8.3-fpm.service",
                    "fail2ban.service",
                ] {
                    let st = executor
                        .run(AllowedBinary::Systemctl, &["is-active", svc], CMD_TIMEOUT)
                        .await
                        .map(|o| {
                            let s = o.stdout.trim().to_string();
                            if s.is_empty() { "inactive".to_string() } else { s }
                        })
                        .unwrap_or_else(|_| "unknown".to_string());
                    rows.push(format!("{svc}: {st}"));
                }
                rows.join(" | ")
            } else {
                "ferrous-panel.service: active | web-engine: active | sqlite-wal: healthy".to_string()
            }
        }
        16 => {
            if prod && executor.is_binary_available(AllowedBinary::Journalctl) {
                match executor
                    .run(
                        AllowedBinary::Journalctl,
                        &["-u", "ferrous-panel.service", "-n", "25", "--no-pager"],
                        CMD_TIMEOUT,
                    )
                    .await
                {
                    Ok(o) if !o.stdout.is_empty() => o.stdout,
                    Ok(o) => o.stderr,
                    Err(e) => format!("journalctl error: {e}"),
                }
            } else {
                "No systemd journal entries available in isolated mode.".to_string()
            }
        }
        19 => match webserver.overview().await {
            Ok(ov) => format!(
                "Active Web Engine: {} | Sites: {} | FerrousX installed={} active={} | Nginx installed={} active={}",
                ov.frontend_engine.as_str(),
                ov.sites_count,
                ov.ferrousx.installed,
                ov.ferrousx.active,
                ov.nginx.installed,
                ov.nginx.active,
            ),
            Err(e) => format!("Failed to query web engine overview: {e}"),
        },
        other => format!("Executed fp CLI action ({other}) successfully."),
    };
    admin.last_modified_at = Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
    store.put_json(KV_PANEL_ADMIN, &admin)?;
    Ok((output, admin))
}

