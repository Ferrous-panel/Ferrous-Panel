#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use ferrousx::config::{
    load_bundle, load_default_env_map, resolve_env_var, DEFAULT_ADMIN_TOKEN_FILE,
    DEFAULT_CONTROL_SOCKET, DEFAULT_MASTER_CONFIG,
};
use ferrousx::ipc::{
    load_or_init_admin_token, read_current_admin_token, IpcRequest, IpcResponse,
    MAX_IPC_PAYLOAD_BYTES,
};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const IO_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Parser)]
#[command(
    name = "fx",
    version,
    about = "FerrousX Admin Control Plane CLI (speaks /run/ferrousx/panel-admin.sock UDS IPC)"
)]
struct FxCli {
    /// Override FerrousX Unix Domain Socket path
    #[arg(long)]
    socket: Option<PathBuf>,

    /// Override FerrousX master config path for `fx test`
    #[arg(long, default_value = DEFAULT_MASTER_CONFIG)]
    config: PathBuf,

    #[command(subcommand)]
    command: Option<FxCommand>,
}

#[derive(Debug, Subcommand)]
enum FxCommand {
    /// Query live FerrousX daemon metrics over UDS IPC
    Status,
    /// Trigger Zero-Downtime RCU configuration + SNI hot-reload
    Reload,
    /// Trigger panel_sync over UDS IPC
    Sync,
    /// Notify FerrousX of an added or updated vhost domain
    VhostAdd { domain: String },
    /// Notify FerrousX of a removed vhost domain
    VhostDel { domain: String },
    /// Query pass-through/FastCGI cache statistics
    Cache,
    /// Validate ferrousx.toml and sites/*.toml syntax locally
    Test,
    /// Inspect or rotate the 256-bit admin IPC token (/etc/ferrousx/admin.token)
    Token {
        /// Generate and write a fresh 256-bit hex admin token (mode 0600)
        #[arg(long, default_value_t = false)]
        rotate: bool,
    },
}

fn resolve_socket_path(override_path: Option<PathBuf>) -> PathBuf {
    if let Some(p) = override_path {
        return p;
    }
    let env_map = load_default_env_map();
    if let Some(sock) = resolve_env_var("FERROUSX_CONTROL_SOCKET", &env_map)
        .or_else(|| resolve_env_var("FERROUSX_PANEL_UDS", &env_map))
    {
        return PathBuf::from(sock);
    }
    let default = PathBuf::from(DEFAULT_CONTROL_SOCKET);
    if default.exists() {
        return default;
    }
    let legacy = PathBuf::from("/run/ferrousx/admin.sock");
    if legacy.exists() {
        return legacy;
    }
    default
}

fn resolve_token_file() -> PathBuf {
    let env_map = load_default_env_map();
    resolve_env_var("FERROUSX_ADMIN_TOKEN_FILE", &env_map)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_ADMIN_TOKEN_FILE))
}

async fn send_ipc(
    socket_path: &Path,
    action: &str,
    payload: serde_json::Value,
) -> Result<IpcResponse, String> {
    let meta = std::fs::symlink_metadata(socket_path)
        .map_err(|e| format!("control socket unavailable at {}: {e}", socket_path.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!(
            "refusing to connect to symlink socket {}",
            socket_path.display()
        ));
    }

    let token_file = resolve_token_file();
    let token = read_current_admin_token(&token_file, "");
    let auth_token = if token.len() >= 32 { Some(token) } else { None };

    let req = IpcRequest {
        action: action.to_string(),
        payload,
        auth_token,
    };
    let mut frame = serde_json::to_vec(&req).map_err(|e| format!("serialize error: {e}"))?;
    frame.push(b'\n');

    let mut stream = tokio::time::timeout(CONNECT_TIMEOUT, UnixStream::connect(socket_path))
        .await
        .map_err(|_| format!("connect timed out: {}", socket_path.display()))?
        .map_err(|e| format!("connect failed ({}): {e}", socket_path.display()))?;

    let raw = tokio::time::timeout(IO_TIMEOUT, async {
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
            if out.len().saturating_add(n) > MAX_IPC_PAYLOAD_BYTES {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "IPC response exceeded 64 KiB cap",
                ));
            }
            out.extend_from_slice(&chunk[..n]);
        }
        Ok::<Vec<u8>, std::io::Error>(out)
    })
    .await
    .map_err(|_| "IPC request timed out".to_string())?
    .map_err(|e| format!("IPC I/O error: {e}"))?;

    serde_json::from_slice::<IpcResponse>(&raw)
        .map_err(|e| format!("invalid JSON IPC response: {e}"))
}

fn main() -> ExitCode {
    let cli = FxCli::parse();
    let cmd = cli.command.unwrap_or(FxCommand::Status);

    match cmd {
        FxCommand::Test => match load_bundle(&cli.config) {
            Ok(bundle) => {
                println!(
                    "ferrousx: config syntax OK ({} sites, {} SNI certificates)",
                    bundle.sites.len(),
                    bundle.sni_certs.len()
                );
                return ExitCode::SUCCESS;
            }
            Err(err) => {
                eprintln!("ferrousx: config check failed: {err}");
                return ExitCode::FAILURE;
            }
        },
        FxCommand::Token { rotate } => {
            let token_file = resolve_token_file();
            if rotate && token_file.exists() {
                if let Err(err) = std::fs::remove_file(&token_file) {
                    eprintln!("ferrousx-ctl: failed to rotate {}: {err}", token_file.display());
                    return ExitCode::FAILURE;
                }
            }
            match load_or_init_admin_token(&token_file) {
                Ok(tok) => {
                    println!(
                        "Admin token file: {} ({} hex chars, mode 0600)",
                        token_file.display(),
                        tok.len()
                    );
                    if rotate {
                        println!("Rotated 256-bit admin IPC token cleanly (live-reloaded per request).");
                    }
                    return ExitCode::SUCCESS;
                }
                Err(err) => {
                    eprintln!("ferrousx-ctl: token error: {err}");
                    return ExitCode::FAILURE;
                }
            }
        }
        ipc_cmd => {
            let socket_path = resolve_socket_path(cli.socket);
            let (action, payload) = match ipc_cmd {
                FxCommand::Status => ("status", serde_json::json!({})),
                FxCommand::Reload => ("reload", serde_json::json!({})),
                FxCommand::Sync => ("panel_sync", serde_json::json!({ "source": "ferrousx-ctl" })),
                FxCommand::VhostAdd { domain } => ("vhost_add", serde_json::json!({ "domain": domain })),
                FxCommand::VhostDel { domain } => ("vhost_del", serde_json::json!({ "domain": domain })),
                FxCommand::Cache => ("cache", serde_json::json!({ "op": "stats" })),
                FxCommand::Test | FxCommand::Token { .. } => return ExitCode::SUCCESS,
            };

            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(err) => {
                    eprintln!("ferrousx-ctl: runtime init error: {err}");
                    return ExitCode::FAILURE;
                }
            };

            match rt.block_on(send_ipc(&socket_path, action, payload)) {
                Ok(resp) => {
                    match serde_json::to_string_pretty(&resp) {
                        Ok(pretty) => println!("{pretty}"),
                        Err(_) => println!("{}", resp.message),
                    }
                    if resp.success {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::FAILURE
                    }
                }
                Err(err) => {
                    eprintln!("ferrousx-ctl: {err}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}
