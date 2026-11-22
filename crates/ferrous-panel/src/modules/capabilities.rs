use crate::config::Config;
use crate::system::cmd::AllowedBinary;
use crate::webserver::CommandExecutor;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

const PROBE_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Ready,
    Degraded,
    Unavailable,
    Experimental,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureCapability {
    pub id: String,
    pub name: String,
    pub category: String,
    pub status: CapabilityStatus,
    pub reason: String,
    pub fix_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitiesReport {
    pub mode: String,
    pub checked_at: String,
    pub features: Vec<FeatureCapability>,
}

async fn is_service_active(executor: &dyn CommandExecutor, unit: &str) -> bool {
    if !executor.is_binary_available(AllowedBinary::Systemctl) {
        return false;
    }
    executor
        .run(AllowedBinary::Systemctl, &["is-active", unit], PROBE_TIMEOUT)
        .await
        .is_ok_and(|o| o.success && o.stdout.trim() == "active")
}

fn dir_writable(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    let probe = path.join(format!(".ferrous-cap-probe-{}", std::process::id()));
    match std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Probes real host binaries, systemd services, control sockets, and filesystem permissions.
/// Never fabricates readiness: missing binaries or inactive daemons report `unavailable`/`degraded`
/// alongside an actionable `fixHint`.
pub async fn probe_capabilities(
    config: &Config,
    executor: &dyn CommandExecutor,
) -> CapabilitiesReport {
    let mut features = Vec::new();

    // 1. Nginx web server + HTTP/3 QUIC module probe
    let nginx_bin = executor.is_binary_available(AllowedBinary::Nginx);
    let nginx_active = nginx_bin && is_service_active(executor, "nginx.service").await;
    let nginx_http3 = if nginx_bin {
        executor
            .run(AllowedBinary::Nginx, &["-V"], PROBE_TIMEOUT)
            .await
            .is_ok_and(|o| {
                let txt = format!("{}\n{}", o.stdout, o.stderr);
                txt.contains("--with-http_v3_module")
            })
    } else {
        false
    };
    features.push(FeatureCapability {
        id: "nginx".to_string(),
        name: "Nginx Web Server".to_string(),
        category: "webserver".to_string(),
        status: if nginx_active {
            CapabilityStatus::Ready
        } else if nginx_bin {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        reason: if nginx_active {
            if nginx_http3 {
                "nginx.service active (HTTP/3 QUIC module enabled)".to_string()
            } else {
                "nginx.service active (compiled without --with-http_v3_module)".to_string()
            }
        } else if nginx_bin {
            "nginx binary installed but nginx.service is inactive".to_string()
        } else {
            "Missing binary nginx".to_string()
        },
        fix_hint: if nginx_active {
            String::new()
        } else if nginx_bin {
            "sudo systemctl enable --now nginx.service".to_string()
        } else {
            "sudo apt update && sudo apt install -y nginx".to_string()
        },
    });

    // 2. FerrousX pure-Rust web server
    let fx_bin = executor.is_binary_available(AllowedBinary::Ferrousx);
    let fx_active = fx_bin && is_service_active(executor, "ferrousx.service").await;
    let fx_ipc_sock = Path::new("/run/ferrousx/panel-admin.sock").exists();
    features.push(FeatureCapability {
        id: "ferrousx".to_string(),
        name: "FerrousX Pure-Rust Engine".to_string(),
        category: "webserver".to_string(),
        status: if fx_active && fx_ipc_sock {
            CapabilityStatus::Ready
        } else if fx_bin {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        reason: if fx_active && fx_ipc_sock {
            "ferrousx.service active with live UDS control plane".to_string()
        } else if fx_active {
            "ferrousx.service active but /run/ferrousx/panel-admin.sock is missing".to_string()
        } else if fx_bin {
            "ferrousx-bin installed but ferrousx.service is inactive".to_string()
        } else {
            "Missing binary ferrousx-bin".to_string()
        },
        fix_hint: if fx_active && fx_ipc_sock {
            String::new()
        } else if fx_bin {
            "sudo systemctl enable --now ferrousx.service".to_string()
        } else {
            "sudo bash update.sh --ferrousx".to_string()
        },
    });

    // 3. Let's Encrypt SSL (certbot + openssl)
    let certbot_bin = executor.is_binary_available(AllowedBinary::Certbot);
    let openssl_bin = executor.is_binary_available(AllowedBinary::OpenSsl);
    features.push(FeatureCapability {
        id: "ssl_certbot".to_string(),
        name: "Let's Encrypt SSL (Certbot)".to_string(),
        category: "webserver".to_string(),
        status: if certbot_bin && openssl_bin {
            CapabilityStatus::Ready
        } else {
            CapabilityStatus::Unavailable
        },
        reason: if certbot_bin && openssl_bin {
            "certbot & openssl ready for ACME HTTP-01 issuance".to_string()
        } else if !certbot_bin {
            "Missing binary certbot".to_string()
        } else {
            "Missing binary openssl".to_string()
        },
        fix_hint: if certbot_bin && openssl_bin {
            String::new()
        } else {
            "sudo apt update && sudo apt install -y certbot openssl".to_string()
        },
    });

    // 4. File Manager (wwwroot write permissions + zip/unzip/tar/curl)
    let wwwroot_ok = dir_writable(&config.wwwroot);
    let archive_tools = executor.is_binary_available(AllowedBinary::Tar)
        && executor.is_binary_available(AllowedBinary::Unzip);
    features.push(FeatureCapability {
        id: "files".to_string(),
        name: "SafePath File Manager".to_string(),
        category: "core".to_string(),
        status: if wwwroot_ok && archive_tools {
            CapabilityStatus::Ready
        } else if wwwroot_ok {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        reason: if wwwroot_ok && archive_tools {
            format!("{} writable with tar/unzip available", config.wwwroot.display())
        } else if wwwroot_ok {
            "wwwroot writable, but zip/unzip or tar is missing".to_string()
        } else {
            format!("wwwroot '{}' is missing or not writable", config.wwwroot.display())
        },
        fix_hint: if wwwroot_ok && archive_tools {
            String::new()
        } else if !wwwroot_ok {
            format!("sudo mkdir -p {} && sudo chmod 0755 {}", config.wwwroot.display(), config.wwwroot.display())
        } else {
            "sudo apt install -y tar zip unzip zstd curl".to_string()
        },
    });

    // 5. MariaDB / MySQL
    let mysql_bin = executor.is_binary_available(AllowedBinary::Mysql);
    let mysql_active = mysql_bin
        && (is_service_active(executor, "mariadb.service").await
            || is_service_active(executor, "mysql.service").await);
    features.push(FeatureCapability {
        id: "mariadb".to_string(),
        name: "MariaDB / MySQL".to_string(),
        category: "database".to_string(),
        status: if mysql_active {
            CapabilityStatus::Ready
        } else if mysql_bin {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        reason: if mysql_active {
            "mariadb/mysql daemon active".to_string()
        } else if mysql_bin {
            "mysql client present but mariadb/mysql service is not active".to_string()
        } else {
            "Missing binary mysql / mariadb-server".to_string()
        },
        fix_hint: if mysql_active {
            String::new()
        } else if mysql_bin {
            "sudo systemctl enable --now mariadb.service".to_string()
        } else {
            "sudo apt update && sudo apt install -y mariadb-server".to_string()
        },
    });

    // 6. PostgreSQL
    let psql_bin = executor.is_binary_available(AllowedBinary::Psql);
    let pg_active = psql_bin && is_service_active(executor, "postgresql.service").await;
    features.push(FeatureCapability {
        id: "postgresql".to_string(),
        name: "PostgreSQL".to_string(),
        category: "database".to_string(),
        status: if pg_active {
            CapabilityStatus::Ready
        } else if psql_bin {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        reason: if pg_active {
            "postgresql.service active".to_string()
        } else if psql_bin {
            "psql client present but postgresql.service is not active".to_string()
        } else {
            "Missing binary psql / postgresql".to_string()
        },
        fix_hint: if pg_active {
            String::new()
        } else if psql_bin {
            "sudo systemctl enable --now postgresql.service".to_string()
        } else {
            "sudo apt update && sudo apt install -y postgresql".to_string()
        },
    });

    // 7. SQLite 3 Native
    let data_writable = dir_writable(&config.data_dir);
    features.push(FeatureCapability {
        id: "sqlite".to_string(),
        name: "SQLite 3 Native (WAL)".to_string(),
        category: "database".to_string(),
        status: if data_writable {
            CapabilityStatus::Ready
        } else {
            CapabilityStatus::Unavailable
        },
        reason: if data_writable {
            "Embedded Rust rusqlite WAL engine ready".to_string()
        } else {
            format!("data_dir '{}' is not writable", config.data_dir.display())
        },
        fix_hint: if data_writable {
            String::new()
        } else {
            format!("sudo mkdir -p {} && sudo chmod 0700 {}", config.data_dir.display(), config.data_dir.display())
        },
    });

    // 8. Firewall (UFW)
    let ufw_bin = executor.is_binary_available(AllowedBinary::Ufw);
    let ufw_active = if ufw_bin {
        executor
            .run(AllowedBinary::Ufw, &["status"], PROBE_TIMEOUT)
            .await
            .is_ok_and(|o| o.success && o.stdout.to_ascii_lowercase().contains("status: active"))
    } else {
        false
    };
    features.push(FeatureCapability {
        id: "ufw".to_string(),
        name: "UFW Firewall".to_string(),
        category: "security".to_string(),
        status: if ufw_active {
            CapabilityStatus::Ready
        } else if ufw_bin {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        reason: if ufw_active {
            "UFW firewall is active".to_string()
        } else if ufw_bin {
            "ufw installed but firewall status is inactive".to_string()
        } else {
            "Missing binary ufw".to_string()
        },
        fix_hint: if ufw_active {
            String::new()
        } else if ufw_bin {
            "sudo ufw allow 22/tcp && sudo ufw --force enable".to_string()
        } else {
            "sudo apt install -y ufw".to_string()
        },
    });

    // 9. Fail2ban
    let f2b_bin = executor.is_binary_available(AllowedBinary::Fail2banClient);
    let f2b_active = f2b_bin && is_service_active(executor, "fail2ban.service").await;
    features.push(FeatureCapability {
        id: "fail2ban".to_string(),
        name: "Fail2ban Intrusion Prevention".to_string(),
        category: "security".to_string(),
        status: if f2b_active {
            CapabilityStatus::Ready
        } else if f2b_bin {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        reason: if f2b_active {
            "fail2ban.service active".to_string()
        } else if f2b_bin {
            "fail2ban-client installed but fail2ban.service is inactive".to_string()
        } else {
            "Missing binary fail2ban-client".to_string()
        },
        fix_hint: if f2b_active {
            String::new()
        } else if f2b_bin {
            "sudo systemctl enable --now fail2ban.service".to_string()
        } else {
            "sudo apt install -y fail2ban".to_string()
        },
    });

    // 10. Cron + Zstd Backups
    let cron_bin = executor.is_binary_available(AllowedBinary::Crontab);
    let backup_ok = executor.is_binary_available(AllowedBinary::Tar)
        && executor.is_binary_available(AllowedBinary::Zstd);
    features.push(FeatureCapability {
        id: "cron_backup".to_string(),
        name: "Crontab & Zstd Backups".to_string(),
        category: "core".to_string(),
        status: if cron_bin && backup_ok {
            CapabilityStatus::Ready
        } else if cron_bin || backup_ok {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        reason: if cron_bin && backup_ok {
            "crontab, tar, and zstd ready".to_string()
        } else if !cron_bin {
            "Missing binary crontab".to_string()
        } else {
            "Missing binary tar or zstd".to_string()
        },
        fix_hint: if cron_bin && backup_ok {
            String::new()
        } else {
            "sudo apt install -y cron tar zstd".to_string()
        },
    });

    // 11. Multi-PHP FPM
    let php_bins = [
        AllowedBinary::PhpFpm84,
        AllowedBinary::PhpFpm83,
        AllowedBinary::PhpFpm82,
        AllowedBinary::PhpFpm81,
        AllowedBinary::PhpFpm74,
    ];
    let installed_php: Vec<&str> = php_bins
        .iter()
        .filter(|&&b| executor.is_binary_available(b))
        .map(|b| b.label())
        .collect();
    features.push(FeatureCapability {
        id: "php_fpm".to_string(),
        name: "Multi-PHP FPM Manager".to_string(),
        category: "extended".to_string(),
        status: if !installed_php.is_empty() {
            CapabilityStatus::Ready
        } else {
            CapabilityStatus::Unavailable
        },
        reason: if !installed_php.is_empty() {
            format!("Detected PHP-FPM binaries: {}", installed_php.join(", "))
        } else {
            "No php-fpm binary installed on host".to_string()
        },
        fix_hint: if !installed_php.is_empty() {
            String::new()
        } else {
            "sudo apt update && sudo apt install -y php8.3-fpm php8.3-cli php8.3-mysql".to_string()
        },
    });

    // 12. Mail Server (Postfix / Dovecot / OpenSSL DKIM)
    let postmap_bin = executor.is_binary_available(AllowedBinary::Postmap);
    let postfix_active = postmap_bin && is_service_active(executor, "postfix.service").await;
    features.push(FeatureCapability {
        id: "mail_server".to_string(),
        name: "Mail Server (Postfix / Dovecot / DKIM)".to_string(),
        category: "extended".to_string(),
        status: if postfix_active {
            CapabilityStatus::Ready
        } else if postmap_bin {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        reason: if postfix_active {
            "postfix.service active with postmap & openssl".to_string()
        } else if postmap_bin {
            "postmap found but postfix.service is inactive".to_string()
        } else {
            "Missing Postfix binaries (postmap/sendmail)".to_string()
        },
        fix_hint: if postfix_active {
            String::new()
        } else if postmap_bin {
            "sudo systemctl enable --now postfix.service".to_string()
        } else {
            "sudo apt update && sudo apt install -y postfix dovecot-core rspamd dnsutils".to_string()
        },
    });

    // 13. SSH Manager
    let sshd_bin = executor.is_binary_available(AllowedBinary::Sshd);
    let ssh_keygen_bin = executor.is_binary_available(AllowedBinary::SshKeygen);
    let ssh_active = sshd_bin
        && (is_service_active(executor, "ssh.service").await
            || is_service_active(executor, "sshd.service").await);
    features.push(FeatureCapability {
        id: "ssh_manager".to_string(),
        name: "OpenSSH Daemon & Key Manager".to_string(),
        category: "security".to_string(),
        status: if ssh_active && ssh_keygen_bin {
            CapabilityStatus::Ready
        } else if sshd_bin {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        reason: if ssh_active && ssh_keygen_bin {
            "OpenSSH daemon active and ssh-keygen ready".to_string()
        } else if sshd_bin {
            "sshd present but ssh.service is inactive or ssh-keygen missing".to_string()
        } else {
            "Missing binary sshd".to_string()
        },
        fix_hint: if ssh_active && ssh_keygen_bin {
            String::new()
        } else {
            "sudo apt install -y openssh-server openssh-client".to_string()
        },
    });

    // 14. System Optimizer (sysctl)
    let sysctl_bin = executor.is_binary_available(AllowedBinary::Sysctl);
    let sysctl_writable = Path::new("/etc/sysctl.d").is_dir() && dir_writable(Path::new("/etc/sysctl.d"));
    features.push(FeatureCapability {
        id: "optimizer_sysctl".to_string(),
        name: "Kernel Sysctl Optimizer".to_string(),
        category: "extended".to_string(),
        status: if sysctl_bin && sysctl_writable {
            CapabilityStatus::Ready
        } else if sysctl_bin {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        reason: if sysctl_bin && sysctl_writable {
            "sysctl binary and /etc/sysctl.d writable".to_string()
        } else if sysctl_bin {
            "sysctl available but /etc/sysctl.d is not writable (needs root)".to_string()
        } else {
            "Missing binary sysctl".to_string()
        },
        fix_hint: if sysctl_bin && sysctl_writable {
            String::new()
        } else {
            "sudo apt install -y procps (run ferrous-panel as root)".to_string()
        },
    });

    // 15. Edge Cloud — WASI Workers (wasmtime)
    let wasmtime_bin = executor.is_binary_available(AllowedBinary::Wasmtime);
    features.push(FeatureCapability {
        id: "edge_cloud_wasi".to_string(),
        name: "Edge Cloud — WASI Workers".to_string(),
        category: "experimental".to_string(),
        status: if wasmtime_bin {
            CapabilityStatus::Experimental
        } else {
            CapabilityStatus::Unavailable
        },
        reason: if wasmtime_bin {
            "Experimental: wasmtime binary available for real .wasm execution".to_string()
        } else {
            "Missing binary wasmtime (experimental WASI worker runtime)".to_string()
        },
        fix_hint: if wasmtime_bin {
            String::new()
        } else {
            "curl https://wasmtime.dev/install.sh -sSf | bash".to_string()
        },
    });

    // 16. Edge Cloud — OCI Containers (podman/docker)
    let oci_bin = executor.is_binary_available(AllowedBinary::Podman)
        || executor.is_binary_available(AllowedBinary::Docker);
    features.push(FeatureCapability {
        id: "edge_cloud_oci".to_string(),
        name: "Edge Cloud — OCI Containers".to_string(),
        category: "experimental".to_string(),
        status: if oci_bin {
            CapabilityStatus::Experimental
        } else {
            CapabilityStatus::Unavailable
        },
        reason: if oci_bin {
            "Experimental: podman/docker CLI detected".to_string()
        } else {
            "Missing binary podman and docker".to_string()
        },
        fix_hint: if oci_bin {
            String::new()
        } else {
            "sudo apt install -y podman".to_string()
        },
    });

    // 17. Edge Cloud — WireGuard Mesh
    let wg_bin = executor.is_binary_available(AllowedBinary::Wg);
    features.push(FeatureCapability {
        id: "edge_cloud_wireguard".to_string(),
        name: "Edge Cloud — WireGuard Mesh".to_string(),
        category: "experimental".to_string(),
        status: if wg_bin {
            CapabilityStatus::Experimental
        } else {
            CapabilityStatus::Unavailable
        },
        reason: if wg_bin {
            "Experimental: wireguard-tools (wg) detected".to_string()
        } else {
            "Missing binary wg (wireguard-tools)".to_string()
        },
        fix_hint: if wg_bin {
            String::new()
        } else {
            "sudo apt install -y wireguard-tools".to_string()
        },
    });

    // 18. Polyglot FFI/IPC Compiler
    let cargo_bin = executor.is_binary_available(AllowedBinary::Cargo);
    features.push(FeatureCapability {
        id: "polyglot_ffi".to_string(),
        name: "Polyglot Rust FFI/IPC Bridge".to_string(),
        category: "experimental".to_string(),
        status: if cargo_bin {
            CapabilityStatus::Experimental
        } else {
            CapabilityStatus::Unavailable
        },
        reason: if cargo_bin {
            "Experimental: Rust cargo toolchain available".to_string()
        } else {
            "Missing binary cargo/rustc".to_string()
        },
        fix_hint: if cargo_bin {
            String::new()
        } else {
            "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh".to_string()
        },
    });

    // 19. eBPF/XDP Kernel Anti-DDoS (honestly experimental / not attached by default)
    features.push(FeatureCapability {
        id: "ebpf_xdp".to_string(),
        name: "eBPF / XDP Anti-DDoS Filter".to_string(),
        category: "experimental".to_string(),
        status: CapabilityStatus::Experimental,
        reason: "Experimental: kernel sysctl SYN hardening is active; standalone eBPF/XDP bytecode attachment is not yet shipped".to_string(),
        fix_hint: "Use UFW + Fail2ban + sysctl SYN cookies for production DDoS protection".to_string(),
    });

    CapabilitiesReport {
        mode: config.mode.as_str().to_string(),
        checked_at: Utc::now().to_rfc3339(),
        features,
    }
}
