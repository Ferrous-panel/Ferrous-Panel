//! Upstream-compatible Unix Domain Socket Admin Control Plane (`/run/ferrousx/panel-admin.sock`)
//! matching `Ferrous-panel/FerrousX @ 48659720` (`crates/ferrousx-panel/src/api.rs`):
//! - Anti-symlink parent & socket directory verification (`0o750`)
//! - Atomic staged socket bind (`0o700` staging dir -> `chmod 0o600`/`0o660` -> atomic `rename`)
//! - `SO_PEERCRED` (`uid`/`gid`) + 256-bit Admin Token (`/etc/ferrousx/admin.token`, `0o600`, constant-time)
//! - RBAC (`FullAdmin` vs `ReadOnly` restricted to `"status"`) + per-UID brute-force lockout guard
//! - 64 KiB payload cap, 5s read timeout, newline-or-EOF JSON framing (`IpcRequest` / `IpcResponse`)

use crate::config::{load_default_env_map, resolve_env_var, SiteHandler};
use crate::error::ServerError;
use crate::handlers::AppRuntime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Read as _;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::watch;

/// Maximum IPC JSON request frame size (64 KiB, matches upstream FerrousX FX-005).
pub const MAX_IPC_PAYLOAD_BYTES: usize = 64 * 1024;
/// Maximum time allowed to read one IPC request frame.
pub const IPC_READ_TIMEOUT: Duration = Duration::from_secs(5);

/// RBAC role granted to an authenticated Unix Domain Socket peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminRole {
    FullAdmin,
    ReadOnly,
}

/// Wire-format JSON request sent over `/run/ferrousx/panel-admin.sock`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcRequest {
    pub action: String,
    #[serde(default)]
    pub payload: serde_json::Value,
    #[serde(default)]
    pub auth_token: Option<String>,
}

/// Wire-format JSON response returned over `/run/ferrousx/panel-admin.sock`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
    pub latency_us: u64,
}

/// Lock-free runtime telemetry counters exposed by the `"status"` and `"cache"` IPC actions.
#[derive(Debug)]
pub struct EngineRuntimeMetrics {
    started_at: Instant,
    epoch: AtomicU64,
    total_requests: AtomicU64,
    active_connections: AtomicU64,
    waf_blocked: AtomicU64,
    rate_limited: AtomicU64,
    upstream_errors: AtomicU64,
    php_fpm_requests: AtomicU64,
    static_requests: AtomicU64,
    http3_requests: AtomicU64,
    bytes_sent: AtomicU64,
    xdp_passed_clean: AtomicU64,
    xdp_dropped_ddos: AtomicU64,
}

impl Default for EngineRuntimeMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineRuntimeMetrics {
    #[must_use]
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            epoch: AtomicU64::new(1),
            total_requests: AtomicU64::new(0),
            active_connections: AtomicU64::new(0),
            waf_blocked: AtomicU64::new(0),
            rate_limited: AtomicU64::new(0),
            upstream_errors: AtomicU64::new(0),
            php_fpm_requests: AtomicU64::new(0),
            static_requests: AtomicU64::new(0),
            http3_requests: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            xdp_passed_clean: AtomicU64::new(0),
            xdp_dropped_ddos: AtomicU64::new(0),
        }
    }

    #[must_use]
    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    pub fn bump_epoch(&self) -> u64 {
        self.epoch.fetch_add(1, Ordering::AcqRel).saturating_add(1)
    }

    #[must_use]
    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    pub fn record_request(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.xdp_passed_clean.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_static(&self) {
        self.static_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_php_fpm(&self) {
        self.php_fpm_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_upstream_error(&self) {
        self.upstream_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_bytes_sent(&self, bytes: u64) {
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn conn_open(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn conn_close(&self) {
        let _ = self
            .active_connections
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |cur| {
                Some(cur.saturating_sub(1))
            });
    }
}

/// Per-UID failed token tracker preventing brute-force guessing over UDS.
#[derive(Debug)]
pub struct BruteForceGuard {
    failures: Mutex<HashMap<u32, (u32, Instant)>>,
    max_attempts: u32,
    lockout_duration: Duration,
}

impl BruteForceGuard {
    #[must_use]
    pub fn new(max_attempts: u32, lockout_secs: u64) -> Self {
        Self {
            failures: Mutex::new(HashMap::new()),
            max_attempts: max_attempts.max(3),
            lockout_duration: Duration::from_secs(lockout_secs.max(30)),
        }
    }

    #[must_use]
    pub fn is_locked_out(&self, uid: u32) -> bool {
        let Ok(mut map) = self.failures.lock() else {
            return false;
        };
        if let Some((count, last_fail)) = map.get(&uid) {
            if *count >= self.max_attempts {
                if last_fail.elapsed() < self.lockout_duration {
                    return true;
                }
                map.remove(&uid);
            }
        }
        false
    }

    pub fn record_failure(&self, uid: u32) {
        if let Ok(mut map) = self.failures.lock() {
            let entry = map.entry(uid).or_insert((0, Instant::now()));
            entry.0 = entry.0.saturating_add(1);
            entry.1 = Instant::now();
        }
    }

    pub fn record_success(&self, uid: u32) {
        if let Ok(mut map) = self.failures.lock() {
            map.remove(&uid);
        }
    }
}

/// Constant-time byte slice equality check resistant to timing attacks.
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.is_empty() || a.len() != b.len() {
        return false;
    }
    bool::from(a.ct_eq(b))
}

/// Loads or initializes the 256-bit hex admin token (`0o600`), rejecting symlinks.
pub fn load_or_init_admin_token(token_path: &Path) -> Result<String, ServerError> {
    let file_env = load_default_env_map();
    if let Some(env_tok) = resolve_env_var("FERROUSX_ADMIN_TOKEN", &file_env) {
        if env_tok.len() >= 32 {
            return Ok(env_tok);
        }
    }

    let effective_path = resolve_env_var("FERROUSX_ADMIN_TOKEN_FILE", &file_env)
        .map(PathBuf::from)
        .unwrap_or_else(|| token_path.to_path_buf());

    if effective_path.exists() {
        let meta = std::fs::symlink_metadata(&effective_path)?;
        if meta.file_type().is_symlink() {
            return Err(ServerError::Config(format!(
                "refusing to read symlink admin token file: {}",
                effective_path.display()
            )));
        }
        let _ = std::fs::set_permissions(&effective_path, std::fs::Permissions::from_mode(0o600));
        let content = std::fs::read_to_string(&effective_path)?;
        let token = content.trim().to_string();
        if token.len() >= 32 {
            return Ok(token);
        }
    }

    let mut random_bytes = [0u8; 32];
    let mut urandom = std::fs::File::open("/dev/urandom")?;
    urandom.read_exact(&mut random_bytes)?;
    let mut hex_token = String::with_capacity(64);
    for b in random_bytes {
        let _ = write!(&mut hex_token, "{b:02x}");
    }

    if let Some(parent) = effective_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&effective_path)
    {
        use std::io::Write as _;
        let _ = std::fs::set_permissions(&effective_path, std::fs::Permissions::from_mode(0o600));
        let _ = writeln!(file, "{hex_token}");
    }
    Ok(hex_token)
}

/// Live-reads the active admin token so `fx token --rotate` takes effect without daemon restart.
#[must_use]
pub fn read_current_admin_token(token_path: &Path, fallback: &str) -> String {
    let file_env = load_default_env_map();
    if let Some(env_tok) = resolve_env_var("FERROUSX_ADMIN_TOKEN", &file_env) {
        if env_tok.len() >= 32 {
            return env_tok;
        }
    }
    let effective_path = resolve_env_var("FERROUSX_ADMIN_TOKEN_FILE", &file_env)
        .map(PathBuf::from)
        .unwrap_or_else(|| token_path.to_path_buf());
    if let Ok(meta) = std::fs::symlink_metadata(&effective_path) {
        if !meta.file_type().is_symlink() {
            if let Ok(content) = std::fs::read_to_string(&effective_path) {
                let tok = content.trim();
                if tok.len() >= 32 {
                    return tok.to_string();
                }
            }
        }
    }
    fallback.to_string()
}

/// Validates and hardens the Unix Domain Socket parent directory against symlink attacks.
fn prepare_secure_socket_directory(socket_path: &Path) -> Result<(), ServerError> {
    if let Some(parent) = socket_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
        let meta = std::fs::symlink_metadata(parent)?;
        if meta.file_type().is_symlink() {
            return Err(ServerError::Config(format!(
                "refusing symlink parent directory for admin IPC socket: {}",
                parent.display()
            )));
        }
        let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o750));
    }

    if let Ok(meta) = std::fs::symlink_metadata(socket_path) {
        if meta.file_type().is_symlink() {
            return Err(ServerError::Config(format!(
                "refusing symlink at admin IPC socket path: {}",
                socket_path.display()
            )));
        }
        std::fs::remove_file(socket_path)?;
    }

    Ok(())
}

/// Binds a UnixListener inside a private `0o700` staging directory, applies `sock_mode`, then
/// atomically renames the socket into `socket_path` so there is zero permission-race window.
pub fn bind_secure_unix_listener(
    socket_path: &Path,
    sock_mode: u32,
) -> Result<UnixListener, ServerError> {
    prepare_secure_socket_directory(socket_path)?;
    let parent = socket_path.parent().ok_or_else(|| {
        ServerError::Config(format!(
            "admin IPC socket path has no parent directory: {}",
            socket_path.display()
        ))
    })?;
    let staging_dir = parent.join(format!(".staging-{}", std::process::id()));
    if staging_dir.exists() {
        let _ = std::fs::remove_dir_all(&staging_dir);
    }
    std::fs::create_dir_all(&staging_dir)?;
    std::fs::set_permissions(&staging_dir, std::fs::Permissions::from_mode(0o700))?;

    let staged_socket = staging_dir.join("admin.sock");
    let listener = UnixListener::bind(&staged_socket)?;
    std::fs::set_permissions(&staged_socket, std::fs::Permissions::from_mode(sock_mode))?;
    std::fs::rename(&staged_socket, socket_path)?;
    let _ = std::fs::remove_dir_all(&staging_dir);
    Ok(listener)
}

/// Authenticates an IPC peer using Linux `SO_PEERCRED` (`uid`/`gid`) and 256-bit `auth_token`.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_ipc_peer(
    peer_uid: u32,
    peer_gid: u32,
    process_uid: u32,
    provided_token: Option<&str>,
    expected_token: &str,
    allow_read_gid: Option<u32>,
    require_peer_uid_auth: bool,
    brute_guard: &BruteForceGuard,
) -> Result<AdminRole, &'static str> {
    if brute_guard.is_locked_out(peer_uid) {
        return Err("UID temporarily locked out after repeated invalid IPC tokens");
    }

    if require_peer_uid_auth && (peer_uid == 0 || peer_uid == process_uid) {
        return Ok(AdminRole::FullAdmin);
    }

    if let Some(tok) = provided_token {
        if !expected_token.is_empty()
            && constant_time_eq(tok.trim().as_bytes(), expected_token.as_bytes())
        {
            brute_guard.record_success(peer_uid);
            return Ok(AdminRole::FullAdmin);
        }
        brute_guard.record_failure(peer_uid);
        return Err("invalid admin IPC authentication token");
    }

    if let Some(allowed_gid) = allow_read_gid {
        if peer_gid == allowed_gid {
            return Ok(AdminRole::ReadOnly);
        }
    }

    Err("IPC access denied: root/owner peer credential or valid 256-bit admin token required")
}

#[derive(Debug)]
enum IpcReadError {
    PayloadTooLarge,
    Timeout,
    Io(std::io::Error),
}

async fn read_ipc_frame(stream: &mut UnixStream) -> Result<Vec<u8>, IpcReadError> {
    let mut buffer = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];
    let read_res = tokio::time::timeout(IPC_READ_TIMEOUT, async {
        loop {
            let n = stream.read(&mut chunk).await.map_err(IpcReadError::Io)?;
            if n == 0 {
                break;
            }
            if let Some(nl_pos) = chunk[..n].iter().position(|&b| b == b'\n') {
                if buffer.len().saturating_add(nl_pos) > MAX_IPC_PAYLOAD_BYTES {
                    return Err(IpcReadError::PayloadTooLarge);
                }
                buffer.extend_from_slice(&chunk[..nl_pos]);
                break;
            }
            if buffer.len().saturating_add(n) > MAX_IPC_PAYLOAD_BYTES {
                return Err(IpcReadError::PayloadTooLarge);
            }
            buffer.extend_from_slice(&chunk[..n]);
        }
        Ok(buffer)
    })
    .await;

    match read_res {
        Ok(res) => res,
        Err(_) => Err(IpcReadError::Timeout),
    }
}

/// Callback invoked by IPC `"reload"` / `"panel_sync"` / `"vhost_add"` / `"vhost_del"` actions.
pub type ReloadFn = Arc<dyn Fn() -> Result<(u64, usize), ServerError> + Send + Sync>;

fn elapsed_us(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn execute_ipc_command(
    req: &IpcRequest,
    role: AdminRole,
    runtime: &AppRuntime,
    reload_fn: &ReloadFn,
    start_time: Instant,
) -> IpcResponse {
    let action = req.action.trim().to_ascii_lowercase();
    if role == AdminRole::ReadOnly && action != "status" {
        tracing::warn!(action = %action, "blocked mutating IPC command from ReadOnly peer");
        return IpcResponse {
            success: false,
            message: "403 Forbidden: ReadOnly role is restricted to 'status'".to_string(),
            data: None,
            latency_us: elapsed_us(start_time),
        };
    }

    match action.as_str() {
        "reload" | "panel_sync" | "vhost_add" | "vhost_del" => match reload_fn() {
            Ok((new_epoch, vhosts_count)) => IpcResponse {
                success: true,
                message: format!(
                    "Synchronized '{action}' and hot-reloaded config + SNI (Zero-Downtime RCU). Epoch: v{new_epoch}"
                ),
                data: Some(serde_json::json!({
                    "action": action,
                    "epoch": new_epoch,
                    "vhosts_count": vhosts_count,
                })),
                latency_us: elapsed_us(start_time),
            },
            Err(err) => IpcResponse {
                success: false,
                message: format!("Rejected invalid reload configuration (previous config retained): {err}"),
                data: None,
                latency_us: elapsed_us(start_time),
            },
        },
        "status" => {
            let bundle = runtime.current_bundle();
            let vhosts_count = bundle.as_ref().map(|b| b.sites.len()).unwrap_or(0);
            let upstreams_count = bundle
                .as_ref()
                .map(|b| {
                    b.sites
                        .iter()
                        .filter(|s| matches!(s.handler, SiteHandler::ReverseProxy { .. }))
                        .count()
                })
                .unwrap_or(0);
            let workers = bundle
                .as_ref()
                .map(|b| {
                    if b.master.worker_threads == 0 {
                        std::thread::available_parallelism()
                            .map(std::num::NonZeroUsize::get)
                            .unwrap_or(1)
                    } else {
                        b.master.worker_threads
                    }
                })
                .unwrap_or(1);
            let waf_mode = bundle
                .as_ref()
                .map(|b| b.master.waf_mode.clone())
                .unwrap_or_else(|| "block".to_string());
            let waf_enabled = !waf_mode.eq_ignore_ascii_case("off");
            let m = &runtime.metrics;

            IpcResponse {
                success: true,
                message: "FerrousX daemon is online and healthy".to_string(),
                data: Some(serde_json::json!({
                    "epoch": m.epoch(),
                    "workers": workers,
                    "vhosts_count": vhosts_count,
                    "upstreams_count": upstreams_count,
                    "quic_enabled": false,
                    "waf_enabled": waf_enabled,
                    "waf_mode": waf_mode,
                    "uptime_secs": m.uptime_secs(),
                    "total_requests": m.total_requests.load(Ordering::Relaxed),
                    "active_connections": m.active_connections.load(Ordering::Relaxed),
                    "waf_blocked": m.waf_blocked.load(Ordering::Relaxed),
                    "rate_limited": m.rate_limited.load(Ordering::Relaxed),
                    "upstream_errors": m.upstream_errors.load(Ordering::Relaxed),
                    "php_fpm_requests": m.php_fpm_requests.load(Ordering::Relaxed),
                    "static_requests": m.static_requests.load(Ordering::Relaxed),
                    "http3_requests": m.http3_requests.load(Ordering::Relaxed),
                    "bytes_sent": m.bytes_sent.load(Ordering::Relaxed),
                    "xdp_passed_clean": m.xdp_passed_clean.load(Ordering::Relaxed),
                    "xdp_dropped_ddos": m.xdp_dropped_ddos.load(Ordering::Relaxed),
                })),
                latency_us: elapsed_us(start_time),
            }
        }
        "cache" => {
            let m = &runtime.metrics;
            IpcResponse {
                success: true,
                message: "FerrousX is operating in direct pass-through/FastCGI mode; send 'reload' to refresh VHosts".to_string(),
                data: Some(serde_json::json!({
                    "mode": "direct_pass_through",
                    "static_requests": m.static_requests.load(Ordering::Relaxed),
                    "php_fpm_requests": m.php_fpm_requests.load(Ordering::Relaxed),
                    "bytes_sent": m.bytes_sent.load(Ordering::Relaxed),
                })),
                latency_us: elapsed_us(start_time),
            }
        }
        "upgrade" => match req.payload.get("binary").and_then(|v| v.as_str()) {
            Some(raw_path) => {
                let p = Path::new(raw_path);
                if !p.is_file() {
                    IpcResponse {
                        success: false,
                        message: format!("Binary path not found: {raw_path}"),
                        data: None,
                        latency_us: elapsed_us(start_time),
                    }
                } else {
                    IpcResponse {
                        success: false,
                        message: format!(
                            "Validate with '{} --check-config' then run `systemctl restart ferrousx.service`.",
                            p.display()
                        ),
                        data: Some(serde_json::json!({ "binary": p.display().to_string() })),
                        latency_us: elapsed_us(start_time),
                    }
                }
            }
            None => IpcResponse {
                success: false,
                message: "Missing required 'binary' parameter for upgrade action".to_string(),
                data: None,
                latency_us: elapsed_us(start_time),
            },
        },
        unknown => {
            tracing::warn!(action = %unknown, "received unknown IPC command");
            IpcResponse {
                success: false,
                message: format!("Unknown IPC action: {unknown}"),
                data: None,
                latency_us: elapsed_us(start_time),
            }
        }
    }
}

async fn write_ipc_response(stream: &mut UnixStream, resp: &IpcResponse) {
    if let Ok(bytes) = serde_json::to_vec(resp) {
        let _ = stream.write_all(&bytes).await;
        let _ = stream.flush().await;
    }
    let _ = stream.shutdown().await;
}

async fn handle_ipc_connection(
    mut stream: UnixStream,
    runtime: Arc<AppRuntime>,
    fallback_token: Arc<String>,
    brute_guard: Arc<BruteForceGuard>,
    process_uid: u32,
    reload_fn: ReloadFn,
) {
    let start_time = Instant::now();
    let (peer_uid, peer_gid) = match stream.peer_cred() {
        Ok(cred) => (cred.uid(), cred.gid()),
        Err(_) => (u32::MAX, u32::MAX),
    };

    let buffer = match read_ipc_frame(&mut stream).await {
        Ok(b) => b,
        Err(IpcReadError::PayloadTooLarge) => {
            let resp = IpcResponse {
                success: false,
                message: "413 Payload Too Large: IPC request exceeds 64 KiB".to_string(),
                data: None,
                latency_us: elapsed_us(start_time),
            };
            write_ipc_response(&mut stream, &resp).await;
            return;
        }
        Err(IpcReadError::Timeout) => {
            let resp = IpcResponse {
                success: false,
                message: "408 Request Timeout: IPC frame read timed out".to_string(),
                data: None,
                latency_us: elapsed_us(start_time),
            };
            write_ipc_response(&mut stream, &resp).await;
            return;
        }
        Err(IpcReadError::Io(_)) => return,
    };

    if buffer.is_empty() {
        return;
    }

    let req: IpcRequest = match serde_json::from_slice(&buffer) {
        Ok(r) => r,
        Err(err) => {
            let resp = IpcResponse {
                success: false,
                message: format!("Invalid JSON IPC request: {err}"),
                data: None,
                latency_us: elapsed_us(start_time),
            };
            write_ipc_response(&mut stream, &resp).await;
            return;
        }
    };

    let (token_file, allow_read_gid, require_peer_uid_auth) = runtime
        .current_bundle()
        .map(|b| {
            (
                b.master.admin_token_file.clone(),
                b.master.allow_read_group_gid,
                b.master.require_peer_uid_auth,
            )
        })
        .unwrap_or_else(|| {
            (
                PathBuf::from(crate::config::DEFAULT_ADMIN_TOKEN_FILE),
                None,
                true,
            )
        });

    let expected_token = read_current_admin_token(&token_file, &fallback_token);
    let role = match authenticate_ipc_peer(
        peer_uid,
        peer_gid,
        process_uid,
        req.auth_token.as_deref(),
        &expected_token,
        allow_read_gid,
        require_peer_uid_auth,
        &brute_guard,
    ) {
        Ok(role) => role,
        Err(reason) => {
            tracing::warn!(
                peer_uid,
                peer_gid,
                action = %req.action,
                "blocked unauthorized IPC connection: {reason}"
            );
            let resp = IpcResponse {
                success: false,
                message: format!("403 Unauthorized: {reason}"),
                data: None,
                latency_us: elapsed_us(start_time),
            };
            write_ipc_response(&mut stream, &resp).await;
            return;
        }
    };

    let response = execute_ipc_command(&req, role, &runtime, &reload_fn, start_time);
    write_ipc_response(&mut stream, &response).await;
}

/// Starts the Unix Domain Socket Admin Control Plane accept loop until shutdown signal.
pub async fn run_admin_ipc_loop(
    listener: UnixListener,
    socket_path: PathBuf,
    runtime: Arc<AppRuntime>,
    fallback_token: Arc<String>,
    brute_guard: Arc<BruteForceGuard>,
    reload_fn: ReloadFn,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let process_uid = std::fs::metadata("/proc/self")
        .map(|m| m.uid())
        .unwrap_or(0);

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    break;
                }
            }
            accept_res = listener.accept() => {
                let Ok((stream, _addr)) = accept_res else {
                    continue;
                };
                let rt = Arc::clone(&runtime);
                let tok = Arc::clone(&fallback_token);
                let guard = Arc::clone(&brute_guard);
                let rfn = Arc::clone(&reload_fn);
                tokio::spawn(async move {
                    handle_ipc_connection(stream, rt, tok, guard, process_uid, rfn).await;
                });
            }
        }
    }

    let _ = std::fs::remove_file(&socket_path);
}
