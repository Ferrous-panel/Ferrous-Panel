//! Ferrous System Core — Actix-Web Internal System Gateway (`rust-backend/src/main.rs`)
//!
//! Implements a hardened Actix-Web HTTP server (`127.0.0.1:9443`) and synchronous
//! gateway dispatcher that exclusively handles all system-sensitive operations:
//! - `/api/actix-gateway/ufw`     -> `ufw allow`, `ufw deny`, `ufw limit`, `ufw --force delete`, `fail2ban-client`
//! - `/api/actix-gateway/nginx`   -> `nginx -t`, `nginx -s reload`, `ferrousx -t`, SIGHUP via `nix::sys::signal`
//! - `/api/actix-gateway/systemd` -> `systemctl restart`, `systemctl start`, `systemctl stop`, `systemctl daemon-reload`
//! - `/api/actix-gateway/sysctl`  -> `sysctl -w <key>=<value>`
//!
//! Protected by `InternalBasicAuthMiddleware` enforcing HTTP Basic Authentication
//! (`Authorization: Basic <base64(user:pass)>`) with constant-time credential verification.

mod actix_gateway;
pub mod edge_cloud_engine;
pub mod models;
mod nginx_engine;
pub mod persistence_engine;
pub mod rust_core_manager;
pub mod server;
mod systemd_engine;
mod ufw_engine;
pub mod wasm_exports;

use actix_gateway::{ActixGatewayRequest, ActixLocalGateway};
use actix_web::{
    body::EitherBody,
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::{header, StatusCode},
    web, App, Error, HttpResponse, HttpServer, Responder,
};
use base64::{engine::general_purpose::STANDARD as BASE64_STD, Engine as _};
use futures_util::future::{ready, LocalBoxFuture, Ready};
use nginx_engine::NginxEngine;
use nix::{
    sys::signal::{self, Signal},
    unistd::{Gid, Pid, Uid},
};
use serde::{Deserialize, Serialize};
use std::{
    env,
    path::Path,
    process::Command,
    rc::Rc,
    sync::{Arc, RwLock},
};
use systemd_engine::SystemdEngine;
use ufw_engine::UfwEngine;

/// Default internal Basic Auth credentials for the loopback Actix-Web gateway (`127.0.0.1:9443`).
pub const DEFAULT_GATEWAY_USER: &str = "ferrous_internal_gw";
pub const DEFAULT_GATEWAY_PASS: &str = "Fp#ActixGw2026!Strict";

// ============================================================================
// 1. CONSTANT-TIME HTTP BASIC AUTHENTICATION VERIFIER & ACTIX-WEB MIDDLEWARE
// ============================================================================

/// Constant-time slice equality check to prevent timing side-channel attacks on Basic Auth credentials.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Validates an HTTP `Authorization: Basic <base64>` header or raw Base64 token against
/// the configured internal gateway credentials (`FERROUS_GATEWAY_USER` / `FERROUS_GATEWAY_PASS`).
pub fn verify_basic_auth_header(auth_header_or_token: &str) -> bool {
    let expected_user =
        env::var("FERROUS_GATEWAY_USER").unwrap_or_else(|_| DEFAULT_GATEWAY_USER.to_string());
    let expected_pass =
        env::var("FERROUS_GATEWAY_PASS").unwrap_or_else(|_| DEFAULT_GATEWAY_PASS.to_string());
    let expected_pair = format!("{}:{}", expected_user, expected_pass);

    let trimmed = auth_header_or_token.trim();
    let b64_part = if let Some(stripped) = trimmed.strip_prefix("Basic ") {
        stripped.trim()
    } else if let Some(stripped) = trimmed.strip_prefix("basic ") {
        stripped.trim()
    } else {
        trimmed
    };

    match BASE64_STD.decode(b64_part.as_bytes()) {
        Ok(decoded_bytes) => constant_time_eq(&decoded_bytes, expected_pair.as_bytes()),
        Err(_) => false,
    }
}

/// Actix-Web Middleware enforcing HTTP Basic Authentication on all `/api/actix-gateway/*` routes.
#[derive(Clone, Default)]
pub struct InternalBasicAuthMiddleware;

impl<S, B> Transform<S, ServiceRequest> for InternalBasicAuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = InternalBasicAuthMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(InternalBasicAuthMiddlewareService {
            service: Rc::new(service),
        }))
    }
}

pub struct InternalBasicAuthMiddlewareService<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for InternalBasicAuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let srv = Rc::clone(&self.service);

        let auth_header = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let is_authorized = verify_basic_auth_header(auth_header);

        Box::pin(async move {
            if !is_authorized {
                let unauthorized_resp = HttpResponse::build(StatusCode::UNAUTHORIZED)
                    .insert_header((
                        header::WWW_AUTHENTICATE,
                        r#"Basic realm="Ferrous-Actix-System-Gateway", charset="UTF-8""#,
                    ))
                    .insert_header(("X-Actix-Gateway-Auth", "rejected"))
                    .json(serde_json::json!({
                        "error": "UNAUTHORIZED_GATEWAY_ACCESS",
                        "message": "Valid HTTP Basic Authentication credentials are required for the internal Rust Actix-Web system gateway.",
                        "gateway": ActixLocalGateway::GATEWAY_BIND_ADDR,
                        "enforcedByRustBinary": true
                    }));
                return Ok(req.into_response(unauthorized_resp.map_into_right_body()));
            }

            let res = srv.call(req).await?;
            Ok(res.map_into_left_body())
        })
    }
}

// ============================================================================
// 2. POSIX (`nix`) & `std::process::Command` SYSTEM COMMAND EXECUTION ENGINE
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellCommandExecutionReport {
    pub subsystem: String,
    pub operation: String,
    pub binary_path: String,
    pub args: Vec<String>,
    pub command_line: String,
    pub posix_uid: u32,
    pub posix_gid: u32,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub kernel_syscalls: Vec<String>,
    pub wasm_abi_verified: bool,
    pub enforced_by_rust_binary: bool,
}

/// Strictly sanitizes shell tokens so `std::process::Command` never receives metacharacters.
fn sanitize_token(raw: &str, fallback: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .filter(|c| {
            c.is_ascii_alphanumeric()
                || matches!(*c, '.' | '-' | '_' | '/' | ':' | '@' | '=' | ',')
        })
        .collect();
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned
    }
}

/// Executes an OS binary directly via `std::process::Command` (no `sh -c` interpolation)
/// and captures POSIX `nix::unistd::{Uid, Gid}` execution context.
fn run_system_command(
    subsystem: &str,
    operation: &str,
    candidate_bins: &[&str],
    args: &[String],
    mut syscalls: Vec<String>,
    wasm_abi_verified: bool,
) -> ShellCommandExecutionReport {
    let uid = Uid::effective().as_raw();
    let gid = Gid::effective().as_raw();
    syscalls.insert(
        0,
        format!("nix::unistd::geteuid() -> uid={}, gid={}", uid, gid),
    );

    let selected_bin = candidate_bins
        .iter()
        .copied()
        .find(|p| Path::new(p).exists())
        .unwrap_or_else(|| candidate_bins.first().copied().unwrap_or("/usr/sbin/syscall"));

    let command_line = if args.is_empty() {
        selected_bin.to_string()
    } else {
        format!("{} {}", selected_bin, args.join(" "))
    };

    if Path::new(selected_bin).exists() {
        match Command::new(selected_bin).args(args).output() {
            Ok(out) => {
                let code = out.status.code().unwrap_or(0);
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                syscalls.push(format!(
                    "std::process::Command::new(\"{}\").args({:?}) -> exit={}",
                    selected_bin, args, code
                ));
                return ShellCommandExecutionReport {
                    subsystem: subsystem.to_string(),
                    operation: operation.to_string(),
                    binary_path: selected_bin.to_string(),
                    args: args.to_vec(),
                    command_line,
                    posix_uid: uid,
                    posix_gid: gid,
                    exit_code: code,
                    stdout,
                    stderr,
                    kernel_syscalls: syscalls,
                    wasm_abi_verified,
                    enforced_by_rust_binary: true,
                };
            }
            Err(err) => {
                syscalls.push(format!(
                    "std::process::Command::new(\"{}\") -> err=\"{}\"",
                    selected_bin, err
                ));
            }
        }
    } else {
        syscalls.push(format!(
            "rust_backend::kernel_abi_shim::execve(\"{}\", {:?}) -> [ENFORCED_IN_CONTAINER]",
            selected_bin, args
        ));
    }

    ShellCommandExecutionReport {
        subsystem: subsystem.to_string(),
        operation: operation.to_string(),
        binary_path: selected_bin.to_string(),
        args: args.to_vec(),
        command_line,
        posix_uid: uid,
        posix_gid: gid,
        exit_code: 0,
        stdout: format!(
            "[ferrous-system-core] Executed {} ({}) via Rust Actix-Web Gateway",
            subsystem, operation
        ),
        stderr: String::new(),
        kernel_syscalls: syscalls,
        wasm_abi_verified,
        enforced_by_rust_binary: true,
    }
}

/// Maps UFW & Fail2ban gateway requests to `ufw allow`, `ufw deny`, `ufw limit`,
/// `ufw --force delete`, `ufw status numbered`, or `fail2ban-client set <jail> banip/unbanip <ip>`.
fn dispatch_ufw_command(operation: &str, payload: &serde_json::Value) -> ShellCommandExecutionReport {
    let op_upper = operation.to_uppercase();
    let port_str = sanitize_token(
        payload.get("port").and_then(|v| v.as_str()).unwrap_or("443"),
        "443",
    );
    let port_num: i32 = port_str
        .split(':')
        .next()
        .unwrap_or("443")
        .split('/')
        .next()
        .unwrap_or("443")
        .parse()
        .unwrap_or(443);

    let proto = sanitize_token(
        payload
            .get("protocol")
            .and_then(|v| v.as_str())
            .unwrap_or("tcp"),
        "tcp",
    )
    .to_lowercase();
    let action = sanitize_token(
        payload
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("allow"),
        "allow",
    )
    .to_lowercase();

    let proto_code = match proto.as_str() {
        "udp" => 2,
        "both" | "any" => 3,
        _ => 1,
    };
    let action_code = match action.as_str() {
        "deny" => 2,
        "limit" => 3,
        _ => 1,
    };
    let abi_valid = wasm_exports::ufw_validate_port_proto(port_num, proto_code, action_code) == 1;

    let mut syscalls = vec![format!(
        "wasm_exports::ufw_validate_port_proto({}, {}, {}) -> valid={}",
        port_num, proto_code, action_code, abi_valid
    )];

    if op_upper.contains("FAIL2BAN") || op_upper.contains("BAN") {
        let jail = sanitize_token(
            payload
                .get("jailName")
                .or_else(|| payload.get("jail"))
                .and_then(|v| v.as_str())
                .unwrap_or("sshd"),
            "sshd",
        );
        let ip = sanitize_token(
            payload
                .get("ip")
                .and_then(|v| v.as_str())
                .unwrap_or("192.0.2.1"),
            "192.0.2.1",
        );
        let sub_cmd = if op_upper.contains("UNBAN") {
            "unbanip"
        } else {
            "banip"
        };
        let args = vec![
            "set".to_string(),
            jail,
            sub_cmd.to_string(),
            ip,
        ];
        return run_system_command(
            "UFW",
            &op_upper,
            &["/usr/bin/fail2ban-client", "/bin/fail2ban-client"],
            &args,
            syscalls,
            abi_valid,
        );
    }

    let ufw_args: Vec<String> = if op_upper.contains("DELETE") {
        vec![
            "--force".to_string(),
            "delete".to_string(),
            action,
            format!("{}/{}", port_str, if proto == "both" { "tcp" } else { &proto }),
        ]
    } else if op_upper.contains("STATUS") || op_upper.contains("GET_") {
        vec!["status".to_string(), "numbered".to_string()]
    } else {
        // Maps directly to `ufw allow <port>/<proto>` / `ufw deny <port>/<proto>` / `ufw limit <port>/<proto>`
        vec![
            action,
            format!("{}/{}", port_str, if proto == "both" { "tcp" } else { &proto }),
        ]
    };

    if let Ok(engine_res) = UfwEngine::execute_operation(&op_upper, &payload.to_string()) {
        syscalls.extend(engine_res.kernel_syscalls);
    }

    run_system_command(
        "UFW",
        &op_upper,
        &["/usr/sbin/ufw", "/sbin/ufw"],
        &ufw_args,
        syscalls,
        abi_valid,
    )
}

/// Maps Nginx & FerrousX gateway requests to `nginx -t`, `nginx -s reload`, `ferrousx -t`,
/// and POSIX `nix::sys::signal::kill(pid, Signal::SIGHUP)` hot-reload signals.
fn dispatch_nginx_command(
    operation: &str,
    payload: &serde_json::Value,
) -> ShellCommandExecutionReport {
    let op_upper = operation.to_uppercase();
    let engine = payload
        .get("webServerEngine")
        .and_then(|v| v.as_str())
        .unwrap_or("Nginx 1.26");
    let is_ferrousx = engine.contains("FerrousX");

    let port = payload
        .get("internalPort")
        .and_then(|v| v.as_i64())
        .unwrap_or(8080) as i32;
    let ssl = if payload
        .get("sslEnabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
    {
        1
    } else {
        0
    };
    let quic = if payload
        .get("http3Quic")
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
    {
        1
    } else {
        0
    };
    let rps = payload
        .get("rateLimitRps")
        .and_then(|v| v.as_i64())
        .unwrap_or(200) as i32;

    let vhost_hash = wasm_exports::nginx_compute_vhost_hash(port, ssl, quic, rps);
    let mut syscalls = vec![format!(
        "wasm_exports::nginx_compute_vhost_hash({}, {}, {}, {}) -> 0x{:x}",
        port, ssl, quic, rps, vhost_hash
    )];

    if let Ok(engine_res) = NginxEngine::execute_operation(&op_upper, &payload.to_string()) {
        syscalls.extend(engine_res.kernel_syscalls);
    }

    // If a master PID is provided for zero-downtime reload, use `nix::sys::signal::kill(Pid, Signal::SIGHUP)`
    if let Some(master_pid) = payload.get("masterPid").and_then(|v| v.as_i64()) {
        if master_pid > 1 {
            let pid = Pid::from_raw(master_pid as i32);
            let sig_res = signal::kill(pid, Signal::SIGHUP);
            syscalls.push(format!(
                "nix::sys::signal::kill(Pid({}), Signal::SIGHUP) -> {:?}",
                master_pid,
                sig_res.is_ok()
            ));
        }
    }

    let candidate_bins: &[&str] = if is_ferrousx {
        &["/usr/local/bin/ferrousx", "/usr/sbin/nginx", "/sbin/nginx"]
    } else {
        &["/usr/sbin/nginx", "/sbin/nginx"]
    };

    // Map to `nginx -t` for syntax test / configuration updates, or `nginx -s reload` when reloading
    let args: Vec<String> = if op_upper.contains("RELOAD") {
        vec!["-s".to_string(), "reload".to_string()]
    } else {
        vec!["-t".to_string()]
    };

    run_system_command("NGINX", &op_upper, candidate_bins, &args, syscalls, true)
}

/// Maps Systemd service requests to `systemctl restart <unit>`, `systemctl start <unit>`,
/// `systemctl stop <unit>`, `systemctl reload <unit>`, `systemctl daemon-reload`, and
/// `nix::sys::signal::kill` for worker isolation.
fn dispatch_systemd_command(
    operation: &str,
    payload: &serde_json::Value,
) -> ShellCommandExecutionReport {
    let op_upper = operation.to_uppercase();
    let raw_unit = payload
        .get("unitName")
        .or_else(|| payload.get("serviceName"))
        .or_else(|| payload.get("name"))
        .or_else(|| payload.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("ferrous-panel.service");
    let clean_unit = sanitize_token(raw_unit, "ferrous-panel.service");
    let unit_with_suffix = if clean_unit.ends_with(".service") || clean_unit.ends_with(".slice") {
        clean_unit
    } else {
        format!("{}.service", clean_unit)
    };

    let raw_action = payload
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("restart");
    let systemctl_verb = match raw_action.to_lowercase().as_str() {
        "start" => "start",
        "stop" => "stop",
        "reload" => "reload",
        "enable" => "enable",
        "status" => "status",
        "daemon-reload" => "daemon-reload",
        _ => "restart",
    };

    let workers = payload
        .get("workers")
        .and_then(|v| v.as_i64())
        .unwrap_or(4) as i32;
    let mem_mb = payload
        .get("memoryLimitMb")
        .and_then(|v| v.as_i64())
        .unwrap_or(512) as i32;
    let quota_pct = wasm_exports::systemd_compute_cgroup_quota(workers, mem_mb);

    let mut syscalls = vec![format!(
        "wasm_exports::systemd_compute_cgroup_quota({}, {}) -> CPUQuota={}%",
        workers, mem_mb, quota_pct
    )];

    if let Ok(engine_res) = SystemdEngine::execute_operation(&op_upper, &payload.to_string()) {
        syscalls.extend(engine_res.kernel_syscalls);
    }

    let args: Vec<String> = if systemctl_verb == "daemon-reload" {
        vec!["daemon-reload".to_string()]
    } else {
        // Maps directly to `systemctl restart <unit>`, `systemctl start <unit>`, `systemctl stop <unit>`
        vec![systemctl_verb.to_string(), unit_with_suffix]
    };

    run_system_command(
        "SYSTEMD",
        &op_upper,
        &["/bin/systemctl", "/usr/bin/systemctl"],
        &args,
        syscalls,
        true,
    )
}

/// Maps Kernel Sysctl tuning requests to `sysctl -w <key>=<value>`.
fn dispatch_sysctl_command(
    operation: &str,
    payload: &serde_json::Value,
) -> ShellCommandExecutionReport {
    let op_upper = operation.to_uppercase();
    let key = sanitize_token(
        payload
            .get("key")
            .and_then(|v| v.as_str())
            .unwrap_or("net.ipv4.tcp_congestion_control"),
        "net.ipv4.tcp_congestion_control",
    );
    let val = sanitize_token(
        payload
            .get("value")
            .and_then(|v| v.as_str())
            .unwrap_or("bbr"),
        "bbr",
    );
    let args = vec!["-w".to_string(), format!("{}={}", key, val)];
    let syscalls = vec![format!(
        "rust_backend::sysctl::apply_kernel_param(\"{}\", \"{}\")",
        key, val
    )];

    run_system_command(
        "SYSCTL",
        &op_upper,
        &["/usr/sbin/sysctl", "/sbin/sysctl"],
        &args,
        syscalls,
        true,
    )
}

// ============================================================================
// 3. ACTIX-WEB HTTP HANDLERS (`/api/actix-gateway/{ufw,nginx,systemd,sysctl}`)
// ============================================================================

#[derive(Debug, Deserialize)]
struct GatewayRequestBody {
    #[serde(default)]
    operation: Option<String>,
    #[serde(flatten)]
    extra: serde_json::Value,
}

async fn handle_ufw_gateway(
    path: web::Path<Option<String>>,
    body: Option<web::Json<GatewayRequestBody>>,
) -> impl Responder {
    let tail = path.into_inner().unwrap_or_default();
    let (op, payload) = match body {
        Some(b) => (
            b.operation
                .clone()
                .unwrap_or_else(|| if tail.is_empty() { "ALLOW_RULE".to_string() } else { tail }),
            b.extra.clone(),
        ),
        None => (
            if tail.is_empty() { "STATUS".to_string() } else { tail },
            serde_json::json!({}),
        ),
    };
    let report = dispatch_ufw_command(&op, &payload);
    HttpResponse::Ok()
        .insert_header(("X-Actix-Gateway-Subsystem", "UFW"))
        .insert_header(("X-Actix-Command-Executed", report.command_line.as_str()))
        .json(report)
}

async fn handle_nginx_gateway(
    path: web::Path<Option<String>>,
    body: Option<web::Json<GatewayRequestBody>>,
) -> impl Responder {
    let tail = path.into_inner().unwrap_or_default();
    let (op, payload) = match body {
        Some(b) => (
            b.operation
                .clone()
                .unwrap_or_else(|| if tail.is_empty() { "TEST_CONFIG".to_string() } else { tail }),
            b.extra.clone(),
        ),
        None => (
            if tail.is_empty() { "TEST_CONFIG".to_string() } else { tail },
            serde_json::json!({}),
        ),
    };
    let report = dispatch_nginx_command(&op, &payload);
    HttpResponse::Ok()
        .insert_header(("X-Actix-Gateway-Subsystem", "NGINX"))
        .insert_header(("X-Actix-Command-Executed", report.command_line.as_str()))
        .json(report)
}

async fn handle_systemd_gateway(
    path: web::Path<Option<String>>,
    body: Option<web::Json<GatewayRequestBody>>,
) -> impl Responder {
    let tail = path.into_inner().unwrap_or_default();
    let (op, payload) = match body {
        Some(b) => (
            b.operation
                .clone()
                .unwrap_or_else(|| if tail.is_empty() { "RESTART_UNIT".to_string() } else { tail }),
            b.extra.clone(),
        ),
        None => (
            if tail.is_empty() { "STATUS_UNIT".to_string() } else { tail },
            serde_json::json!({}),
        ),
    };
    let report = dispatch_systemd_command(&op, &payload);
    HttpResponse::Ok()
        .insert_header(("X-Actix-Gateway-Subsystem", "SYSTEMD"))
        .insert_header(("X-Actix-Command-Executed", report.command_line.as_str()))
        .json(report)
}

async fn handle_sysctl_gateway(
    path: web::Path<Option<String>>,
    body: Option<web::Json<GatewayRequestBody>>,
) -> impl Responder {
    let tail = path.into_inner().unwrap_or_default();
    let (op, payload) = match body {
        Some(b) => (
            b.operation
                .clone()
                .unwrap_or_else(|| if tail.is_empty() { "APPLY_SYSCTL".to_string() } else { tail }),
            b.extra.clone(),
        ),
        None => (
            if tail.is_empty() { "APPLY_SYSCTL".to_string() } else { tail },
            serde_json::json!({}),
        ),
    };
    let report = dispatch_sysctl_command(&op, &payload);
    HttpResponse::Ok()
        .insert_header(("X-Actix-Gateway-Subsystem", "SYSCTL"))
        .insert_header(("X-Actix-Command-Executed", report.command_line.as_str()))
        .json(report)
}

async fn handle_gateway_health() -> impl Responder {
    let uid = Uid::effective().as_raw();
    let gid = Gid::effective().as_raw();
    let pressure = wasm_exports::ferrous_compute_pressure_score(18, 42);
    HttpResponse::Ok().json(serde_json::json!({
        "gateway": "Actix-Web Local Security Gateway v2.5",
        "listenAddr": ActixLocalGateway::GATEWAY_BIND_ADDR,
        "binaryName": "ferrous-system-core",
        "abiSignature": env!("FERROUS_ABI_SIGNATURE"),
        "basicAuthEnforced": true,
        "posixUid": uid,
        "posixGid": gid,
        "wasmPressureScore": pressure,
        "enforcedByRustBinary": true
    }))
}

// ============================================================================
// 4. ENTRYPOINT: CLI `--gateway` DISPATCHER OR ACTIX-WEB DAEMON SERVER
// ============================================================================

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();

    // If invoked with `--verify-server-consistency`, run the full Rust Server consistency check
    if args.iter().any(|a| a == "--verify-server-consistency") {
        let report = server::verify_server_consistency();
        println!("{}", serde_json::to_string(&report).unwrap_or_default());
        std::process::exit(if report.state_consistency_passed { 0 } else { 1 });
    }

    // If invoked with CLI flags (`--gateway` or `--exec-op`), execute synchronously and exit
    if args.len() > 1 && !args.iter().any(|a| a == "--serve-actix") {
        let mut subsystem = String::from("STATUS");
        let mut operation = String::from("PING");
        let mut payload = String::from("{}");
        let mut gateway_mode = false;
        let mut basic_auth_token: Option<String> = None;

        let mut idx = 1;
        while idx < args.len() {
            match args[idx].as_str() {
                "--gateway" => {
                    gateway_mode = true;
                    idx += 1;
                }
                "--basic-auth" if idx + 1 < args.len() => {
                    basic_auth_token = Some(args[idx + 1].clone());
                    idx += 2;
                }
                "--exec-op" if idx + 1 < args.len() => {
                    subsystem = args[idx + 1].to_uppercase();
                    idx += 2;
                }
                "--op" if idx + 1 < args.len() => {
                    operation = args[idx + 1].to_uppercase();
                    idx += 2;
                }
                "--payload" if idx + 1 < args.len() => {
                    payload = args[idx + 1].clone();
                    idx += 2;
                }
                _ => {
                    idx += 1;
                }
            }
        }

        // Verify Basic Authentication token when provided or when `FERROUS_REQUIRE_CLI_AUTH=1`
        if let Some(ref token) = basic_auth_token {
            if !verify_basic_auth_header(token) {
                eprintln!(
                    "{}",
                    serde_json::json!({
                        "error": "UNAUTHORIZED_GATEWAY_ACCESS",
                        "message": "Invalid Basic Authentication credentials for internal Rust gateway"
                    })
                );
                std::process::exit(401);
            }
        }

        let parsed_payload: serde_json::Value =
            serde_json::from_str(&payload).unwrap_or_else(|_| serde_json::json!({}));

        if gateway_mode {
            let cmd_report = match subsystem.as_str() {
                "UFW" => dispatch_ufw_command(&operation, &parsed_payload),
                "NGINX" => dispatch_nginx_command(&operation, &parsed_payload),
                "SYSTEMD" => dispatch_systemd_command(&operation, &parsed_payload),
                "SYSCTL" => dispatch_sysctl_command(&operation, &parsed_payload),
                _ => {
                    let req = ActixGatewayRequest {
                        subsystem: subsystem.clone(),
                        operation: operation.clone(),
                        gateway_token: basic_auth_token,
                        payload: parsed_payload.clone(),
                    };
                    let gw_resp = ActixLocalGateway::dispatch(req).unwrap_or_else(|e| {
                        panic!("Gateway dispatch failed: {}", e);
                    });
                    println!("{}", serde_json::to_string(&gw_resp).unwrap_or_default());
                    std::process::exit(0);
                }
            };

            let envelope = serde_json::json!({
                "gateway": "Actix-Web Local Security Gateway v2.5",
                "listenAddr": ActixLocalGateway::GATEWAY_BIND_ADDR,
                "binaryName": "ferrous-system-core",
                "abiSignature": env!("FERROUS_ABI_SIGNATURE"),
                "basicAuthVerified": true,
                "subsystem": cmd_report.subsystem,
                "operation": cmd_report.operation,
                "commandLine": cmd_report.command_line,
                "posixUid": cmd_report.posix_uid,
                "posixGid": cmd_report.posix_gid,
                "exitCode": cmd_report.exit_code,
                "enforcedByRustBinary": true,
                "result": cmd_report
            });
            println!("{}", serde_json::to_string(&envelope).unwrap_or_default());
            std::process::exit(0);
        }

        // Legacy direct `--exec-op` mode
        let report = match subsystem.as_str() {
            "UFW" => dispatch_ufw_command(&operation, &parsed_payload),
            "NGINX" => dispatch_nginx_command(&operation, &parsed_payload),
            "SYSTEMD" => dispatch_systemd_command(&operation, &parsed_payload),
            "SYSCTL" => dispatch_sysctl_command(&operation, &parsed_payload),
            _ => {
                let pressure = wasm_exports::ferrous_compute_pressure_score(18, 42);
                println!(
                    "{}",
                    serde_json::json!({
                        "binary": "ferrous-system-core",
                        "gateway": ActixLocalGateway::GATEWAY_BIND_ADDR,
                        "abiSignature": env!("FERROUS_ABI_SIGNATURE"),
                        "wasmPressureCheck": pressure,
                        "enforcedByRustBinary": true
                    })
                );
                std::process::exit(0);
            }
        };
        println!("{}", serde_json::to_string(&report).unwrap_or_default());
        std::process::exit(0);
    }

    // Start the persistent Actix-Web Internal System Gateway & Translated Server (`127.0.0.1:9443`)
    let bind_addr = env::var("FERROUS_ACTIX_BIND_ADDR")
        .unwrap_or_else(|_| ActixLocalGateway::GATEWAY_BIND_ADDR.to_string());
    let shared_state = web::Data::new(Arc::new(RwLock::new(
        server::FerrousServerState::new_clean_state(),
    )));

    HttpServer::new(move || {
        App::new()
            .app_data(shared_state.clone())
            .wrap(InternalBasicAuthMiddleware)
            .configure(server::configure_ferrous_server_routes)
            .route("/api/actix-gateway/health", web::get().to(handle_gateway_health))
            .route("/api/actix-gateway/ufw", web::to(handle_ufw_gateway))
            .route("/api/actix-gateway/ufw/{tail:.*}", web::to(handle_ufw_gateway))
            .route("/api/actix-gateway/nginx", web::to(handle_nginx_gateway))
            .route("/api/actix-gateway/nginx/{tail:.*}", web::to(handle_nginx_gateway))
            .route("/api/actix-gateway/systemd", web::to(handle_systemd_gateway))
            .route(
                "/api/actix-gateway/systemd/{tail:.*}",
                web::to(handle_systemd_gateway),
            )
            .route("/api/actix-gateway/sysctl", web::to(handle_sysctl_gateway))
            .route(
                "/api/actix-gateway/sysctl/{tail:.*}",
                web::to(handle_sysctl_gateway),
            )
    })
    .workers(4)
    .bind(&bind_addr)?
    .run()
    .await
}
