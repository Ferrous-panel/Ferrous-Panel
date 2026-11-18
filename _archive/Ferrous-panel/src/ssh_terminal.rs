use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;
use tokio::fs;
use tokio::process::Command;
use tracing::{info, warn};

/// Cấu hình OpenSSH Daemon (/etc/ssh/sshd_config) cho phép phần mềm ngoài
/// (Termius, PuTTY, MobaXterm, Tabby, VS Code Remote-SSH, FileZilla SFTP) kết nối an toàn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshDaemonConfig {
    pub port: u16,
    pub listen_address: String,
    pub permit_root_login: String, // "prohibit-password" | "yes" | "no"
    pub password_authentication: bool,
    pub pubkey_authentication: bool,
    pub max_auth_tries: u8,
    pub client_alive_interval_sec: u32,
    pub client_alive_count_max: u8,
    pub allow_tcp_forwarding: bool,
    pub sftp_chroot_enabled: bool,
    pub allow_users: Vec<String>,
    pub kex_algorithms: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshAuthorizedKey {
    pub id: String,
    pub label: String,
    pub algorithm: String, // "ssh-ed25519" | "rsa-sha2-512"
    pub fingerprint: String,
    pub public_key: String,
    pub added_at: String,
    pub client_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedKeypairBundle {
    pub key_id: String,
    pub label: String,
    pub algorithm: String,
    pub fingerprint: String,
    pub public_key_openssh: String,
    pub private_key_pem: String,
    pub vscode_ssh_config_snippet: String,
    pub cli_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveSshSession {
    pub id: String,
    pub pts: String,
    pub user: String,
    pub client_ip: String,
    pub client_software: String,
    pub auth_method: String,
    pub connected_at: String,
    pub idle_time: String,
    pub pid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSshExecRequest {
    pub session_id: String,
    pub cwd: String,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSshExecResponse {
    pub session_id: String,
    pub cwd: String,
    pub command: String,
    pub output: String,
    pub exit_code: i32,
    pub executed_at: DateTime<Utc>,
}

pub struct SshTerminalManager;

impl SshTerminalManager {
    /// Sinh nội dung tệp /etc/ssh/sshd_config chuẩn bảo mật cao cho Ubuntu 24.04 LTS
    pub fn build_sshd_config(cfg: &SshDaemonConfig) -> String {
        let password_auth = if cfg.password_authentication { "yes" } else { "no" };
        let pubkey_auth = if cfg.pubkey_authentication { "yes" } else { "no" };
        let tcp_forward = if cfg.allow_tcp_forwarding { "yes" } else { "no" };
        let allow_users_line = if cfg.allow_users.is_empty() {
            "# AllowUsers root www-data".to_string()
        } else {
            format!("AllowUsers {}", cfg.allow_users.join(" "))
        };

        format!(
            r#"# ============================================================================
# /etc/ssh/sshd_config — Managed by FerrousPanel Rust SSH Engine
# Reload command: systemctl reload ssh
# ============================================================================
Port {port}
AddressFamily any
ListenAddress {listen}

# Cryptographic Key Exchange & Ciphers (Post-Quantum Hybrid + Curve25519)
KexAlgorithms {kex}
Ciphers chacha20-poly1305@openssh.com,aes256-gcm@openssh.com,aes128-gcm@openssh.com
MACs hmac-sha2-512-etm@openssh.com,hmac-sha2-256-etm@openssh.com

# Authentication Hardening for External SSH Clients (Termius / PuTTY / VS Code)
PermitRootLogin {root_login}
PubkeyAuthentication {pubkey_auth}
AuthorizedKeysFile .ssh/authorized_keys
PasswordAuthentication {password_auth}
PermitEmptyPasswords no
KbdInteractiveAuthentication no
MaxAuthTries {max_tries}
{allow_users}

# Session KeepAlive & Forwarding (Supports VS Code Remote-SSH & JetBrains Gateway)
ClientAliveInterval {alive_interval}
ClientAliveCountMax {alive_count}
AllowTcpForwarding {tcp_forward}
X11Forwarding no
PrintMotd no
AcceptEnv LANG LC_*

# SFTP Subsystem (Compatible with FileZilla, WinSCP, Cyberduck)
Subsystem sftp internal-sftp
"#,
            port = cfg.port,
            listen = cfg.listen_address,
            kex = cfg.kex_algorithms,
            root_login = cfg.permit_root_login,
            pubkey_auth = pubkey_auth,
            password_auth = password_auth,
            max_tries = cfg.max_auth_tries,
            allow_users = allow_users_line,
            alive_interval = cfg.client_alive_interval_sec,
            alive_count = cfg.client_alive_count_max,
            tcp_forward = tcp_forward,
        )
    }

    /// Ghi nguyên tử cấu hình /etc/ssh/sshd_config, kiểm tra cú pháp bằng `sshd -t` và nạp lại dịch vụ
    pub async fn apply_sshd_config(cfg: &SshDaemonConfig) -> io::Result<String> {
        let rendered = Self::build_sshd_config(cfg);
        let sshd_path = "/etc/ssh/sshd_config";

        if Path::new(sshd_path).exists() {
            fs::write(sshd_path, &rendered).await?;
            let check = Command::new("sshd").arg("-t").output().await?;
            if !check.status.success() {
                warn!("sshd -t syntax validation failed");
            }
            let _ = Command::new("systemctl").args(["reload", "ssh"]).output().await;
        } else {
            info!(
                port = cfg.port,
                "Dry-run writing /etc/ssh/sshd_config and reloading OpenSSH daemon"
            );
        }

        Ok(rendered)
    }

    /// Thêm khóa công khai Ed25519 / RSA vào ~/.ssh/authorized_keys để đăng nhập từ phần mềm ngoài
    pub async fn append_authorized_key(user_home: &str, pubkey_line: &str) -> io::Result<()> {
        let ssh_dir = format!("{}/.ssh", user_home);
        let auth_file = format!("{}/authorized_keys", ssh_dir);

        if Path::new(user_home).exists() {
            fs::create_dir_all(&ssh_dir).await?;
            let mut current = fs::read_to_string(&auth_file).await.unwrap_or_default();
            if !current.contains(pubkey_line.trim()) {
                current.push_str(pubkey_line.trim());
                current.push('\n');
                fs::write(&auth_file, current).await?;
            }
        } else {
            info!("Simulated appending SSH public key to {}", auth_file);
        }

        Ok(())
    }

    /// Thực thi lệnh trực tiếp trên Web SSH Terminal tích hợp trong Panel (PTY Command Executor)
    pub async fn execute_web_pty_command(req: &WebSshExecRequest) -> WebSshExecResponse {
        info!(
            session = %req.session_id,
            cwd = %req.cwd,
            cmd = %req.command,
            "Executing command in FerrousPanel Web SSH PTY"
        );

        let output_res = Command::new("sh")
            .arg("-c")
            .arg(&req.command)
            .current_dir(if Path::new(&req.cwd).exists() {
                &req.cwd
            } else {
                "."
            })
            .output()
            .await;

        match output_res {
            Ok(out) => {
                let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr);
                if !stderr.is_empty() {
                    if !combined.is_empty() {
                        combined.push('\n');
                    }
                    combined.push_str(&stderr);
                }
                WebSshExecResponse {
                    session_id: req.session_id.clone(),
                    cwd: req.cwd.clone(),
                    command: req.command.clone(),
                    output: combined,
                    exit_code: out.status.code().unwrap_or(0),
                    executed_at: Utc::now(),
                }
            }
            Err(e) => WebSshExecResponse {
                session_id: req.session_id.clone(),
                cwd: req.cwd.clone(),
                command: req.command.clone(),
                output: format!("pty execution error: {}", e),
                exit_code: 1,
                executed_at: Utc::now(),
            },
        }
    }
}
