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
    #[serde(default)]
    pub label: String,
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
    #[serde(rename = "capabilities", alias = "features")]
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
    let push_cap = |list: &mut Vec<FeatureCapability>,
                    id: &str,
                    name: &str,
                    category: &str,
                    status: CapabilityStatus,
                    reason: String,
                    fix_hint: String| {
        list.push(FeatureCapability {
            id: id.to_string(),
            name: name.to_string(),
            label: name.to_string(),
            category: category.to_string(),
            status,
            reason,
            fix_hint,
        });
    };

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
    push_cap(
        &mut features,
        "nginx",
        "Nginx Web Server",
        "webserver",
        if nginx_active {
            CapabilityStatus::Ready
        } else if nginx_bin {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        if nginx_active {
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
        if nginx_active {
            String::new()
        } else if nginx_bin {
            "sudo systemctl enable --now nginx.service".to_string()
        } else {
            "sudo apt update && sudo apt install -y nginx".to_string()
        },
    );

    // 2. FerrousX pure-Rust web server
    let fx_bin = executor.is_binary_available(AllowedBinary::Ferrousx);
    let fx_active = fx_bin && is_service_active(executor, "ferrousx.service").await;
    let fx_ipc_sock = Path::new("/run/ferrousx/panel-admin.sock").exists();
    push_cap(
        &mut features,
        "ferrousx",
        "FerrousX Pure-Rust Engine",
        "webserver",
        if fx_active && fx_ipc_sock {
            CapabilityStatus::Ready
        } else if fx_bin {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        if fx_active && fx_ipc_sock {
            "ferrousx.service active with live UDS control plane".to_string()
        } else if fx_active {
            "ferrousx.service active but /run/ferrousx/panel-admin.sock is missing".to_string()
        } else if fx_bin {
            "ferrousx-bin installed but ferrousx.service is inactive".to_string()
        } else {
            "Missing binary ferrousx-bin".to_string()
        },
        if fx_active && fx_ipc_sock {
            String::new()
        } else if fx_bin {
            "sudo systemctl enable --now ferrousx.service".to_string()
        } else {
            "sudo bash update.sh --ferrousx".to_string()
        },
    );

    // 3. SSL / HTTPS (Let's Encrypt Certbot + OpenSSL X.509)
    let certbot_bin = executor.is_binary_available(AllowedBinary::Certbot);
    let openssl_bin = executor.is_binary_available(AllowedBinary::OpenSsl);
    push_cap(
        &mut features,
        "ssl_certbot",
        "SSL / HTTPS (Let's Encrypt & OpenSSL)",
        "webserver",
        if openssl_bin || certbot_bin {
            CapabilityStatus::Ready
        } else {
            CapabilityStatus::Unavailable
        },
        if certbot_bin && openssl_bin {
            "certbot & openssl ready for ACME HTTP-01 and X.509 SSL".to_string()
        } else if openssl_bin {
            "openssl ready for HTTPS (certbot will be auto-installed on Let's Encrypt issuance)".to_string()
        } else if certbot_bin {
            "certbot ready".to_string()
        } else {
            "Missing binary openssl / certbot".to_string()
        },
        if openssl_bin || certbot_bin {
            String::new()
        } else {
            "sudo apt update && sudo apt install -y certbot openssl".to_string()
        },
    );

    // 4. File Manager (wwwroot write permissions + zip/unzip/tar/curl)
    let wwwroot_ok = dir_writable(&config.wwwroot);
    let archive_tools = executor.is_binary_available(AllowedBinary::Tar)
        && executor.is_binary_available(AllowedBinary::Unzip);
    push_cap(
        &mut features,
        "files",
        "SafePath File Manager",
        "core",
        if wwwroot_ok && archive_tools {
            CapabilityStatus::Ready
        } else if wwwroot_ok {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        if wwwroot_ok && archive_tools {
            format!("{} writable with tar/unzip available", config.wwwroot.display())
        } else if wwwroot_ok {
            "wwwroot writable, but zip/unzip or tar is missing".to_string()
        } else {
            format!("wwwroot '{}' is missing or not writable", config.wwwroot.display())
        },
        if wwwroot_ok && archive_tools {
            String::new()
        } else if !wwwroot_ok {
            format!("sudo mkdir -p {} && sudo chmod 0755 {}", config.wwwroot.display(), config.wwwroot.display())
        } else {
            "sudo apt install -y tar zip unzip zstd curl".to_string()
        },
    );

    // 5. MariaDB / MySQL
    let mysql_bin = executor.is_binary_available(AllowedBinary::Mysql);
    let mysql_active = mysql_bin
        && (is_service_active(executor, "mariadb.service").await
            || is_service_active(executor, "mysql.service").await);
    push_cap(
        &mut features,
        "mariadb",
        "MariaDB / MySQL",
        "database",
        if mysql_active {
            CapabilityStatus::Ready
        } else if mysql_bin {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        if mysql_active {
            "mariadb/mysql daemon active".to_string()
        } else if mysql_bin {
            "mysql client present but mariadb/mysql service is not active".to_string()
        } else {
            "Missing binary mysql / mariadb-server".to_string()
        },
        if mysql_active {
            String::new()
        } else if mysql_bin {
            "sudo systemctl enable --now mariadb.service".to_string()
        } else {
            "sudo apt update && sudo apt install -y mariadb-server".to_string()
        },
    );

    // 6. PostgreSQL
    let psql_bin = executor.is_binary_available(AllowedBinary::Psql);
    let pg_active = psql_bin && is_service_active(executor, "postgresql.service").await;
    push_cap(
        &mut features,
        "postgresql",
        "PostgreSQL",
        "database",
        if pg_active {
            CapabilityStatus::Ready
        } else if psql_bin {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        if pg_active {
            "postgresql.service active".to_string()
        } else if psql_bin {
            "psql client present but postgresql.service is not active".to_string()
        } else {
            "Missing binary psql / postgresql".to_string()
        },
        if pg_active {
            String::new()
        } else if psql_bin {
            "sudo systemctl enable --now postgresql.service".to_string()
        } else {
            "sudo apt update && sudo apt install -y postgresql".to_string()
        },
    );

    // 7. SQLite 3 Native
    let data_writable = dir_writable(&config.data_dir);
    push_cap(
        &mut features,
        "sqlite",
        "SQLite 3 Native (WAL)",
        "database",
        if data_writable {
            CapabilityStatus::Ready
        } else {
            CapabilityStatus::Unavailable
        },
        if data_writable {
            "Embedded Rust rusqlite WAL engine ready".to_string()
        } else {
            format!("data_dir '{}' is not writable", config.data_dir.display())
        },
        if data_writable {
            String::new()
        } else {
            format!("sudo mkdir -p {} && sudo chmod 0700 {}", config.data_dir.display(), config.data_dir.display())
        },
    );

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
    push_cap(
        &mut features,
        "ufw",
        "UFW Firewall",
        "security",
        if ufw_active {
            CapabilityStatus::Ready
        } else if ufw_bin {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        if ufw_active {
            "UFW firewall is active".to_string()
        } else if ufw_bin {
            "ufw installed but firewall status is inactive".to_string()
        } else {
            "Missing binary ufw".to_string()
        },
        if ufw_active {
            String::new()
        } else if ufw_bin {
            "sudo ufw allow 22/tcp && sudo ufw --force enable".to_string()
        } else {
            "sudo apt install -y ufw".to_string()
        },
    );

    // 9. Fail2ban
    let f2b_bin = executor.is_binary_available(AllowedBinary::Fail2banClient);
    let f2b_active = f2b_bin && is_service_active(executor, "fail2ban.service").await;
    push_cap(
        &mut features,
        "fail2ban",
        "Fail2ban Intrusion Prevention",
        "security",
        if f2b_active {
            CapabilityStatus::Ready
        } else if f2b_bin {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        if f2b_active {
            "fail2ban.service active".to_string()
        } else if f2b_bin {
            "fail2ban-client installed but fail2ban.service is inactive".to_string()
        } else {
            "Missing binary fail2ban-client".to_string()
        },
        if f2b_active {
            String::new()
        } else if f2b_bin {
            "sudo systemctl enable --now fail2ban.service".to_string()
        } else {
            "sudo apt install -y fail2ban".to_string()
        },
    );

    // 10. Cron + Zstd Backups
    let cron_bin = executor.is_binary_available(AllowedBinary::Crontab);
    let backup_ok = executor.is_binary_available(AllowedBinary::Tar)
        && executor.is_binary_available(AllowedBinary::Zstd);
    push_cap(
        &mut features,
        "cron_backup",
        "Crontab & Zstd Backups",
        "core",
        if cron_bin && backup_ok {
            CapabilityStatus::Ready
        } else if cron_bin || backup_ok {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        if cron_bin && backup_ok {
            "crontab, tar, and zstd ready".to_string()
        } else if !cron_bin {
            "Missing binary crontab".to_string()
        } else {
            "Missing binary tar or zstd".to_string()
        },
        if cron_bin && backup_ok {
            String::new()
        } else {
            "sudo apt install -y cron tar zstd".to_string()
        },
    );

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
    push_cap(
        &mut features,
        "php_fpm",
        "Multi-PHP FPM Manager",
        "extended",
        if !installed_php.is_empty() {
            CapabilityStatus::Ready
        } else {
            CapabilityStatus::Unavailable
        },
        if !installed_php.is_empty() {
            format!("Detected PHP-FPM binaries: {}", installed_php.join(", "))
        } else {
            "No php-fpm binary installed on host".to_string()
        },
        if !installed_php.is_empty() {
            String::new()
        } else {
            "sudo apt update && sudo apt install -y php8.3-fpm php8.3-cli php8.3-mysql".to_string()
        },
    );

    // 12. Mail Server (Postfix / Dovecot / OpenSSL DKIM)
    let postmap_bin = executor.is_binary_available(AllowedBinary::Postmap);
    let postfix_active = postmap_bin && is_service_active(executor, "postfix.service").await;
    push_cap(
        &mut features,
        "mail_server",
        "Mail Server (Postfix / Dovecot / DKIM)",
        "extended",
        if postfix_active {
            CapabilityStatus::Ready
        } else if postmap_bin {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        if postfix_active {
            "postfix.service active with postmap & openssl".to_string()
        } else if postmap_bin {
            "postmap found but postfix.service is inactive".to_string()
        } else {
            "Missing Postfix binaries (postmap/sendmail)".to_string()
        },
        if postfix_active {
            String::new()
        } else if postmap_bin {
            "sudo systemctl enable --now postfix.service".to_string()
        } else {
            "sudo apt update && sudo apt install -y postfix dovecot-core rspamd dnsutils".to_string()
        },
    );

    // 13. SSH Manager
    let sshd_bin = executor.is_binary_available(AllowedBinary::Sshd);
    let ssh_keygen_bin = executor.is_binary_available(AllowedBinary::SshKeygen);
    let ssh_active = sshd_bin
        && (is_service_active(executor, "ssh.service").await
            || is_service_active(executor, "sshd.service").await);
    push_cap(
        &mut features,
        "ssh_manager",
        "OpenSSH Daemon & Key Manager",
        "security",
        if ssh_active && ssh_keygen_bin {
            CapabilityStatus::Ready
        } else if sshd_bin {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        if ssh_active && ssh_keygen_bin {
            "OpenSSH daemon active and ssh-keygen ready".to_string()
        } else if sshd_bin {
            "sshd present but ssh.service is inactive or ssh-keygen missing".to_string()
        } else {
            "Missing binary sshd".to_string()
        },
        if ssh_active && ssh_keygen_bin {
            String::new()
        } else {
            "sudo apt install -y openssh-server openssh-client".to_string()
        },
    );

    // 14. System Optimizer (sysctl)
    let sysctl_bin = executor.is_binary_available(AllowedBinary::Sysctl);
    let sysctl_writable = Path::new("/etc/sysctl.d").is_dir() && dir_writable(Path::new("/etc/sysctl.d"));
    push_cap(
        &mut features,
        "optimizer_sysctl",
        "Kernel Sysctl Optimizer",
        "extended",
        if sysctl_bin && sysctl_writable {
            CapabilityStatus::Ready
        } else if sysctl_bin {
            CapabilityStatus::Degraded
        } else {
            CapabilityStatus::Unavailable
        },
        if sysctl_bin && sysctl_writable {
            "sysctl binary and /etc/sysctl.d writable".to_string()
        } else if sysctl_bin {
            "sysctl available but /etc/sysctl.d is not writable (needs root)".to_string()
        } else {
            "Missing binary sysctl".to_string()
        },
        if sysctl_bin && sysctl_writable {
            String::new()
        } else {
            "sudo apt install -y procps (run ferrous-panel as root)".to_string()
        },
    );

    // 15. Edge Cloud — WASI Workers (wasmtime)
    let wasmtime_bin = executor.is_binary_available(AllowedBinary::Wasmtime);
    push_cap(
        &mut features,
        "edge_cloud_wasi",
        "Edge Cloud — WASI Workers",
        "experimental",
        if wasmtime_bin {
            CapabilityStatus::Experimental
        } else {
            CapabilityStatus::Unavailable
        },
        if wasmtime_bin {
            "Experimental: wasmtime binary available for real .wasm execution".to_string()
        } else {
            "Missing binary wasmtime (experimental WASI worker runtime)".to_string()
        },
        if wasmtime_bin {
            String::new()
        } else {
            "curl https://wasmtime.dev/install.sh -sSf | bash".to_string()
        },
    );

    // 16. Edge Cloud — OCI Containers (podman/docker)
    let oci_bin = executor.is_binary_available(AllowedBinary::Podman)
        || executor.is_binary_available(AllowedBinary::Docker);
    push_cap(
        &mut features,
        "edge_cloud_oci",
        "Edge Cloud — OCI Containers",
        "experimental",
        if oci_bin {
            CapabilityStatus::Experimental
        } else {
            CapabilityStatus::Unavailable
        },
        if oci_bin {
            "Experimental: podman/docker CLI detected".to_string()
        } else {
            "Missing binary podman and docker".to_string()
        },
        if oci_bin {
            String::new()
        } else {
            "sudo apt install -y podman".to_string()
        },
    );

    // 17. Edge Cloud — WireGuard Mesh
    let wg_bin = executor.is_binary_available(AllowedBinary::Wg);
    push_cap(
        &mut features,
        "edge_cloud_wireguard",
        "Edge Cloud — WireGuard Mesh",
        "experimental",
        if wg_bin {
            CapabilityStatus::Experimental
        } else {
            CapabilityStatus::Unavailable
        },
        if wg_bin {
            "Experimental: wireguard-tools (wg) detected".to_string()
        } else {
            "Missing binary wg (wireguard-tools)".to_string()
        },
        if wg_bin {
            String::new()
        } else {
            "sudo apt install -y wireguard-tools".to_string()
        },
    );

    // 18. Polyglot FFI/IPC Compiler
    let cargo_bin = executor.is_binary_available(AllowedBinary::Cargo);
    push_cap(
        &mut features,
        "polyglot_ffi",
        "Polyglot Rust FFI/IPC Bridge",
        "experimental",
        if cargo_bin {
            CapabilityStatus::Experimental
        } else {
            CapabilityStatus::Unavailable
        },
        if cargo_bin {
            "Experimental: Rust cargo toolchain available".to_string()
        } else {
            "Missing binary cargo/rustc".to_string()
        },
        if cargo_bin {
            String::new()
        } else {
            "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh".to_string()
        },
    );

    // 19. eBPF/XDP Kernel Anti-DDoS (honestly experimental / not attached by default)
    push_cap(
        &mut features,
        "ebpf_xdp",
        "eBPF / XDP Anti-DDoS Filter",
        "experimental",
        CapabilityStatus::Experimental,
        "Experimental: kernel sysctl SYN hardening is active; standalone eBPF/XDP bytecode attachment is not yet shipped".to_string(),
        "Use UFW + Fail2ban + sysctl SYN cookies for production DDoS protection".to_string(),
    );

    CapabilitiesReport {
        mode: config.mode.as_str().to_string(),
        checked_at: Utc::now().to_rfc3339(),
        features,
    }
}
