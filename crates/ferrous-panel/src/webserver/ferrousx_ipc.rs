//! Unix Domain Socket Admin IPC Client for FerrousX (`/run/ferrousx/panel-admin.sock`)
//! matching the upstream `Ferrous-panel/FerrousX @ 48659720` (`crates/ferrousx-panel/src/api.rs`) protocol:
//! - Request: JSON `IpcRequest { action, payload, auth_token }` delimited by `\n` + write half-close
//! - Response: JSON `IpcResponse { success, message, data, latency_us }` (64 KiB cap, 5s timeout)
//! - Auth: Linux `SO_PEERCRED` (`uid`/`gid`) + 256-bit hex token from `/etc/ferrousx/admin.token` (`0600`)
//! - Env precedence: `Process ENV > $FERROUSX_ENV_FILE > /etc/ferrousx/ferrousx.env > defaults`

use crate::error::PanelError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

pub const DEFAULT_FERROUSX_CONTROL_SOCKET: &str = "/run/ferrousx/panel-admin.sock";
pub const LEGACY_FERROUSX_CONTROL_SOCKET: &str = "/run/ferrousx/admin.sock";
pub const DEFAULT_FERROUSX_ADMIN_TOKEN_FILE: &str = "/etc/ferrousx/admin.token";
pub const DEFAULT_FERROUSX_ENV_FILE: &str = "/etc/ferrousx/ferrousx.env";

const MAX_IPC_RESPONSE_BYTES: usize = 64 * 1024;
const IPC_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const IPC_IO_TIMEOUT: Duration = Duration::from_secs(5);

/// Allowlisted FerrousX Admin Control Plane actions (matches upstream `execute_ipc_command`).
pub const ALLOWED_IPC_ACTIONS: &[&str] = &[
    "status",
    "reload",
    "panel_sync",
    "vhost_add",
    "vhost_del",
    "cache",
    "upgrade",
];

/// Wire-format request matching `Ferrous-panel/FerrousX/crates/ferrousx-panel/src/api.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcRequest {
    pub action: String,
    pub payload: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
}

/// Wire-format response matching `Ferrous-panel/FerrousX/crates/ferrousx-panel/src/api.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub success: bool,
    pub message: String,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
    #[serde(default)]
    pub latency_us: u64,
}

/// Typed snapshot returned by the `"status"` IPC action.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FerrousxLiveStatus {
    pub epoch: u64,
    pub workers: u64,
    pub vhosts_count: u64,
    pub upstreams_count: u64,
    pub quic_enabled: bool,
    pub waf_enabled: bool,
    pub waf_mode: String,
    pub uptime_secs: u64,
    pub total_requests: u64,
    pub active_connections: u64,
    pub waf_blocked: u64,
    pub rate_limited: u64,
    pub upstream_errors: u64,
    pub php_fpm_requests: u64,
    pub static_requests: u64,
    pub http3_requests: u64,
    pub bytes_sent: u64,
    pub xdp_passed_clean: u64,
    pub xdp_dropped_ddos: u64,
}

/// Typed result returned by `"reload"`, `"panel_sync"`, `"vhost_add"`, and `"vhost_del"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FerrousxReloadResult {
    pub action: String,
    pub epoch: u64,
    pub vhosts_count: u64,
    pub latency_us: u64,
    pub message: String,
}

fn load_env_file_map(path: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Ok(raw) = std::fs::read_to_string(path) else {
        return map;
    };
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let cleaned = trimmed.strip_prefix("export ").unwrap_or(trimmed).trim();
        if let Some((k, v)) = cleaned.split_once('=') {
            let key = k.trim();
            if key.is_empty() {
                continue;
            }
            let val = v
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
                .to_string();
            map.insert(key.to_string(), val);
        }
    }
    map
}

fn load_ferrousx_env_map() -> HashMap<String, String> {
    let mut merged = load_env_file_map(Path::new(DEFAULT_FERROUSX_ENV_FILE));
    if let Ok(custom) = std::env::var("FERROUSX_ENV_FILE") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            merged.extend(load_env_file_map(Path::new(trimmed)));
        }
    }
    merged
}

fn resolve_env_var(key: &str, file_env: &HashMap<String, String>) -> Option<String> {
    if let Ok(val) = std::env::var(key) {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    file_env
        .get(key)
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn read_token_file_no_symlink(path: &Path) -> Option<String> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if meta.file_type().is_symlink() {
        return None;
    }
    let raw = std::fs::read_to_string(path).ok()?;
    let trimmed = raw.trim();
    if trimmed.len() >= 32 {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Async client speaking the FerrousX Admin UDS IPC protocol (`/run/ferrousx/panel-admin.sock`).
#[derive(Debug, Clone)]
pub struct FerrousxIpcClient {
    socket_path: PathBuf,
    token_file: PathBuf,
    resolve_from_env: bool,
}

impl FerrousxIpcClient {
    /// Production client resolving `FERROUSX_CONTROL_SOCKET`, `FERROUSX_PANEL_UDS`,
    /// `/run/ferrousx/panel-admin.sock`, `/run/ferrousx/admin.sock`, and `/etc/ferrousx/admin.token`.
    #[must_use]
    pub fn system_default() -> Self {
        Self {
            socket_path: PathBuf::from(DEFAULT_FERROUSX_CONTROL_SOCKET),
            token_file: PathBuf::from(DEFAULT_FERROUSX_ADMIN_TOKEN_FILE),
            resolve_from_env: true,
        }
    }

    /// Isolated client pinned to explicit socket and token paths (never falls back to host paths).
    #[must_use]
    pub fn for_paths(socket_path: PathBuf, token_file: PathBuf) -> Self {
        Self {
            socket_path,
            token_file,
            resolve_from_env: false,
        }
    }

    /// Resolves the effective Unix Domain Socket path.
    #[must_use]
    pub fn resolved_socket_path(&self) -> PathBuf {
        if !self.resolve_from_env {
            return self.socket_path.clone();
        }
        let env_map = load_ferrousx_env_map();
        if let Some(env_sock) = resolve_env_var("FERROUSX_CONTROL_SOCKET", &env_map)
            .or_else(|| resolve_env_var("FERROUSX_PANEL_UDS", &env_map))
        {
            let env_path = PathBuf::from(env_sock);
            if env_path.exists() {
                return env_path;
            }
        }
        if self.socket_path.exists() {
            return self.socket_path.clone();
        }
        let legacy = Path::new(LEGACY_FERROUSX_CONTROL_SOCKET);
        if legacy.exists() {
            return legacy.to_path_buf();
        }
        self.socket_path.clone()
    }

    /// Resolves the effective 256-bit Admin Token file path.
    #[must_use]
    pub fn resolved_token_file(&self) -> PathBuf {
        if !self.resolve_from_env {
            return self.token_file.clone();
        }
        let env_map = load_ferrousx_env_map();
        resolve_env_var("FERROUSX_ADMIN_TOKEN_FILE", &env_map)
            .map(PathBuf::from)
            .unwrap_or_else(|| self.token_file.clone())
    }

    /// Returns `true` if the target Unix Domain Socket exists and is not a symlink.
    #[must_use]
    pub fn socket_available(&self) -> bool {
        let path = self.resolved_socket_path();
        std::fs::symlink_metadata(&path)
            .map(|m| !m.file_type().is_symlink())
            .unwrap_or(false)
    }

    fn resolve_auth_token(&self) -> Option<String> {
        if !self.resolve_from_env {
            return read_token_file_no_symlink(&self.token_file);
        }
        let env_map = load_ferrousx_env_map();
        if let Some(tok) = resolve_env_var("FERROUSX_ADMIN_TOKEN", &env_map) {
            if tok.len() >= 32 {
                return Some(tok);
            }
        }
        let token_path = self.resolved_token_file();
        read_token_file_no_symlink(&token_path)
    }

    /// Sends an authenticated JSON IPC command over the FerrousX Unix Domain Socket.
    pub async fn send_command(
        &self,
        action: &str,
        payload: serde_json::Value,
    ) -> Result<IpcResponse, PanelError> {
        let normalized_action = action.trim().to_ascii_lowercase();
        if !ALLOWED_IPC_ACTIONS.contains(&normalized_action.as_str()) {
            return Err(PanelError::Validation(format!(
                "unsupported FerrousX IPC action '{action}' (allowed: {})",
                ALLOWED_IPC_ACTIONS.join(", ")
            )));
        }

        let socket_path = self.resolved_socket_path();
        let meta = std::fs::symlink_metadata(&socket_path).map_err(|err| {
            PanelError::BadGateway(format!(
                "FerrousX control socket unavailable at {}: {err}",
                socket_path.display()
            ))
        })?;
        if meta.file_type().is_symlink() {
            return Err(PanelError::Forbidden(format!(
                "refusing to connect to symlink FerrousX socket: {}",
                socket_path.display()
            )));
        }

        let auth_token = self.resolve_auth_token();
        let req = IpcRequest {
            action: normalized_action,
            payload,
            auth_token,
        };
        let mut frame = serde_json::to_vec(&req)?;
        if frame.len() >= MAX_IPC_RESPONSE_BYTES {
            return Err(PanelError::Validation(
                "FerrousX IPC request payload exceeds 64 KiB limit".to_string(),
            ));
        }
        frame.push(b'\n');

        let mut stream = tokio::time::timeout(IPC_CONNECT_TIMEOUT, UnixStream::connect(&socket_path))
            .await
            .map_err(|_| {
                PanelError::BadGateway(format!(
                    "timed out connecting to FerrousX control socket {}",
                    socket_path.display()
                ))
            })?
            .map_err(|err| {
                PanelError::BadGateway(format!(
                    "failed to connect to FerrousX control socket {}: {err}",
                    socket_path.display()
                ))
            })?;

        let raw_resp = tokio::time::timeout(IPC_IO_TIMEOUT, async {
            stream.write_all(&frame).await?;
            stream.flush().await?;
            stream.shutdown().await?;

            let mut out = Vec::with_capacity(1024);
            let mut chunk = [0u8; 4096];
            loop {
                let n = stream.read(&mut chunk).await?;
                if n == 0 {
                    break;
                }
                if out.len().saturating_add(n) > MAX_IPC_RESPONSE_BYTES {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "FerrousX IPC response exceeded 64 KiB cap",
                    ));
                }
                out.extend_from_slice(&chunk[..n]);
            }
            Ok::<Vec<u8>, std::io::Error>(out)
        })
        .await
        .map_err(|_| {
            PanelError::BadGateway(format!(
                "timed out waiting for FerrousX IPC response on {}",
                socket_path.display()
            ))
        })?
        .map_err(|err| {
            PanelError::BadGateway(format!(
                "FerrousX IPC I/O error on {}: {err}",
                socket_path.display()
            ))
        })?;

        if raw_resp.is_empty() {
            return Err(PanelError::BadGateway(format!(
                "empty response from FerrousX control socket {}",
                socket_path.display()
            )));
        }

        let resp: IpcResponse = serde_json::from_slice(&raw_resp).map_err(|err| {
            PanelError::BadGateway(format!(
                "invalid JSON response from FerrousX control socket: {err}"
            ))
        })?;
        Ok(resp)
    }

    /// Queries live daemon telemetry via `"status"`.
    pub async fn status(&self) -> Result<(FerrousxLiveStatus, u64, String), PanelError> {
        let resp = self.send_command("status", serde_json::json!({})).await?;
        if !resp.success {
            return Err(PanelError::BadGateway(resp.message));
        }
        let parsed: FerrousxLiveStatus = match resp.data {
            Some(val) => serde_json::from_value(val).unwrap_or_default(),
            None => FerrousxLiveStatus::default(),
        };
        Ok((parsed, resp.latency_us, resp.message))
    }

    async fn send_reload_action(
        &self,
        action: &str,
        payload: serde_json::Value,
    ) -> Result<FerrousxReloadResult, PanelError> {
        let resp = self.send_command(action, payload).await?;
        if !resp.success {
            return Err(PanelError::Validation(format!(
                "FerrousX IPC '{action}' rejected: {}",
                resp.message
            )));
        }
        let epoch = resp
            .data
            .as_ref()
            .and_then(|d| d.get("epoch"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let vhosts_count = resp
            .data
            .as_ref()
            .and_then(|d| d.get("vhosts_count"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        Ok(FerrousxReloadResult {
            action: action.to_string(),
            epoch,
            vhosts_count,
            latency_us: resp.latency_us,
            message: resp.message,
        })
    }

    /// Triggers Zero-Downtime RCU reload via `"reload"`.
    pub async fn reload(&self) -> Result<FerrousxReloadResult, PanelError> {
        self.send_reload_action("reload", serde_json::json!({})).await
    }

    /// Triggers Zero-Downtime panel synchronization via `"panel_sync"`.
    pub async fn panel_sync(
        &self,
        payload: serde_json::Value,
    ) -> Result<FerrousxReloadResult, PanelError> {
        self.send_reload_action("panel_sync", payload).await
    }

    /// Notifies FerrousX of a newly added or updated site via `"vhost_add"`.
    pub async fn vhost_add(&self, domain: &str) -> Result<FerrousxReloadResult, PanelError> {
        self.send_reload_action("vhost_add", serde_json::json!({ "domain": domain }))
            .await
    }

    /// Notifies FerrousX of a removed site via `"vhost_del"`.
    pub async fn vhost_del(&self, domain: &str) -> Result<FerrousxReloadResult, PanelError> {
        self.send_reload_action("vhost_del", serde_json::json!({ "domain": domain }))
            .await
    }

    /// Queries pass-through cache statistics via `"cache"`.
    pub async fn cache_stats(&self) -> Result<IpcResponse, PanelError> {
        self.send_command("cache", serde_json::json!({ "op": "stats" }))
            .await
    }
}
