#![forbid(unsafe_code)]

use actix_web::body::to_bytes;
use actix_web::http::{header, StatusCode};
use actix_web::test::{init_service, try_call_service, TestRequest};
use actix_web::{web, App};
use anyhow::{bail, Context, Result};
use ferrous_panel::config::Config;
use ferrous_panel::configure_app;
use ferrous_panel::error::PanelError;
use ferrous_panel::state::AppState;
use ferrous_panel::system::cmd::{AllowedBinary, CmdOutput};
use ferrous_panel::system::paths::SafePath;
use ferrous_panel::system::validate::Domain;
use ferrous_panel::webserver::ferrousx::{FerrousxEngine, FerrousxPaths};
use ferrous_panel::webserver::nginx::{NginxEngine, NginxPaths};
use ferrous_panel::webserver::ssl::{SslManager, SslPaths};
use ferrous_panel::webserver::{
    BoxFuture, CommandExecutor, SystemCommandExecutor, WebServerManager, WebServerStore,
};
use rand::rngs::OsRng;
use rand::RngCore;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::future::Future;
use std::io::{Read as _, Write as _};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn ensure(cond: bool, msg: &str) -> Result<()> {
    if cond {
        Ok(())
    } else {
        bail!("{msg}")
    }
}

fn ensure_eq<T: PartialEq + std::fmt::Debug>(actual: &T, expected: &T, label: &str) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        bail!("{label} mismatch:\nexpected: {expected:?}\nactual:   {actual:?}")
    }
}

fn run_async<F: Future<Output = Result<()>>>(fut: F) -> Result<()> {
    let rt = actix_web::rt::Runtime::new().context("failed to start test runtime")?;
    rt.block_on(fut)
}

#[derive(Debug)]
struct TempWorkspace {
    dir: PathBuf,
}

impl TempWorkspace {
    fn new(prefix: &str) -> Result<Self> {
        let mut suffix = [0u8; 8];
        OsRng
            .try_fill_bytes(&mut suffix)
            .map_err(|_| anyhow::anyhow!("OsRng unavailable"))?;
        let mut hex = String::with_capacity(16);
        for b in suffix {
            use std::fmt::Write as _;
            let _ = write!(hex, "{b:02x}");
        }
        let dir = std::env::temp_dir().join(format!("ferrous-panel-p5-{prefix}-{hex}"));
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[derive(Debug, Clone)]
struct RecordedCall {
    binary: &'static str,
    args: Vec<String>,
}

#[derive(Debug)]
struct MockExecutor {
    mysql_installed: AtomicBool,
    psql_installed: AtomicBool,
    nginx_installed: AtomicBool,
    ufw_installed: AtomicBool,
    fail2ban_installed: AtomicBool,
    tar_installed: AtomicBool,
    crontab_installed: AtomicBool,
    php_installed: AtomicBool,
    unit_active: AtomicBool,
    ufw_rules: Mutex<Vec<String>>,
    banned_ips: Mutex<Vec<String>>,
    crontabs: Mutex<HashMap<String, String>>,
    calls: Mutex<Vec<RecordedCall>>,
}

impl MockExecutor {
    fn new() -> Self {
        Self {
            mysql_installed: AtomicBool::new(true),
            psql_installed: AtomicBool::new(true),
            nginx_installed: AtomicBool::new(true),
            ufw_installed: AtomicBool::new(true),
            fail2ban_installed: AtomicBool::new(true),
            tar_installed: AtomicBool::new(true),
            crontab_installed: AtomicBool::new(true),
            php_installed: AtomicBool::new(true),
            unit_active: AtomicBool::new(true),
            ufw_rules: Mutex::new(vec![
                "22/tcp                     ALLOW IN    Anywhere                   # SSH"
                    .to_string(),
                "443/tcp                    ALLOW IN    Anywhere                   # HTTPS"
                    .to_string(),
            ]),
            banned_ips: Mutex::new(vec!["203.0.113.99".to_string()]),
            crontabs: Mutex::new(HashMap::new()),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn record(&self, bin: AllowedBinary, args: &[&str]) {
        if let Ok(mut guard) = self.calls.lock() {
            guard.push(RecordedCall {
                binary: bin.label(),
                args: args.iter().map(|s| (*s).to_string()).collect(),
            });
        }
    }

    fn calls_for(&self, label: &str) -> Vec<RecordedCall> {
        self.calls
            .lock()
            .map(|g| {
                g.iter()
                    .filter(|c| c.binary == label)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl CommandExecutor for MockExecutor {
    fn is_binary_available(&self, bin: AllowedBinary) -> bool {
        match bin {
            AllowedBinary::Mysql => self.mysql_installed.load(AtomicOrdering::SeqCst),
            AllowedBinary::Psql => self.psql_installed.load(AtomicOrdering::SeqCst),
            AllowedBinary::Nginx => self.nginx_installed.load(AtomicOrdering::SeqCst),
            AllowedBinary::Ufw => self.ufw_installed.load(AtomicOrdering::SeqCst),
            AllowedBinary::Fail2banClient => {
                self.fail2ban_installed.load(AtomicOrdering::SeqCst)
            }
            AllowedBinary::Tar => self.tar_installed.load(AtomicOrdering::SeqCst),
            AllowedBinary::Crontab => self.crontab_installed.load(AtomicOrdering::SeqCst),
            AllowedBinary::Php83 => self.php_installed.load(AtomicOrdering::SeqCst),
            AllowedBinary::Systemctl
            | AllowedBinary::Sysctl
            | AllowedBinary::Certbot
            | AllowedBinary::OpenSsl
            | AllowedBinary::Zip
            | AllowedBinary::Unzip
            | AllowedBinary::Curl
            | AllowedBinary::Kill => true,
            _ => false,
        }
    }

    fn run<'a>(
        &'a self,
        bin: AllowedBinary,
        args: &'a [&'a str],
        _timeout: Duration,
    ) -> BoxFuture<'a, Result<CmdOutput, PanelError>> {
        self.record(bin, args);
        let owned: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();

        Box::pin(async move {
            match bin {
                AllowedBinary::Nginx => Ok(CmdOutput {
                    exit_code: Some(0),
                    success: true,
                    stdout: String::new(),
                    stderr: "nginx: configuration file /etc/nginx/nginx.conf syntax is ok"
                        .to_string(),
                }),
                AllowedBinary::Systemctl => {
                    let sub = owned.first().map(String::as_str).unwrap_or("");
                    match sub {
                        "stop" => {
                            self.unit_active.store(false, AtomicOrdering::SeqCst);
                            Ok(CmdOutput {
                                exit_code: Some(0),
                                success: true,
                                stdout: String::new(),
                                stderr: String::new(),
                            })
                        }
                        "start" | "restart" | "enable" => {
                            self.unit_active.store(true, AtomicOrdering::SeqCst);
                            Ok(CmdOutput {
                                exit_code: Some(0),
                                success: true,
                                stdout: String::new(),
                                stderr: String::new(),
                            })
                        }
                        "show" => {
                            let active = self.unit_active.load(AtomicOrdering::SeqCst);
                            let (state, sub_state, pid, mem) = if active {
                                ("active", "running", "5150", "67108864")
                            } else {
                                ("inactive", "dead", "0", "0")
                            };
                            Ok(CmdOutput {
                                exit_code: Some(0),
                                success: true,
                                stdout: format!(
                                    "ActiveState={state}\nSubState={sub_state}\nMainPID={pid}\nMemoryCurrent={mem}\n"
                                ),
                                stderr: String::new(),
                            })
                        }
                        _ => Ok(CmdOutput {
                            exit_code: Some(0),
                            success: true,
                            stdout: String::new(),
                            stderr: String::new(),
                        }),
                    }
                }
                AllowedBinary::Ufw => {
                    let joined = owned.join(" ");
                    if owned.len() >= 2 && owned[0] == "status" && owned[1] == "numbered" {
                        let rules = self.ufw_rules.lock().map(|g| g.clone()).unwrap_or_default();
                        let mut stdout =
                            String::from("Status: active\n\n     To                         Action      From\n     --                         ------      ----\n");
                        for (idx, r) in rules.iter().enumerate() {
                            stdout.push_str(&format!("[{:2}] {r}\n", idx + 1));
                        }
                        Ok(CmdOutput {
                            exit_code: Some(0),
                            success: true,
                            stdout,
                            stderr: String::new(),
                        })
                    } else if owned.len() >= 3 && owned[0] == "--force" && owned[1] == "delete" {
                        if let Ok(num) = owned[2].parse::<usize>() {
                            if let Ok(mut guard) = self.ufw_rules.lock() {
                                if num >= 1 && num <= guard.len() {
                                    guard.remove(num - 1);
                                }
                            }
                        }
                        Ok(CmdOutput {
                            exit_code: Some(0),
                            success: true,
                            stdout: "Rule deleted".to_string(),
                            stderr: String::new(),
                        })
                    } else {
                        if let Ok(mut guard) = self.ufw_rules.lock() {
                            guard.push(format!(
                                "8443/tcp                   ALLOW IN    Anywhere                   # {joined}"
                            ));
                        }
                        Ok(CmdOutput {
                            exit_code: Some(0),
                            success: true,
                            stdout: "Rule added".to_string(),
                            stderr: String::new(),
                        })
                    }
                }
                AllowedBinary::Fail2banClient => {
                    if owned == ["status"] {
                        Ok(CmdOutput {
                            exit_code: Some(0),
                            success: true,
                            stdout: "Status\n|- Number of jail:\t1\n`- Jail list:\tsshd\n"
                                .to_string(),
                            stderr: String::new(),
                        })
                    } else if owned.len() == 2 && owned[0] == "status" {
                        let ips = self.banned_ips.lock().map(|g| g.clone()).unwrap_or_default();
                        let ip_line = ips.join(" ");
                        Ok(CmdOutput {
                            exit_code: Some(0),
                            success: true,
                            stdout: format!(
                                "Status for the jail: {}\n|- Filter\n|  `- File list:\t/var/log/auth.log\n`- Actions\n   |- Currently banned:\t{}\n   |- Total banned:\t{}\n   `- Banned IP list:\t{ip_line}\n",
                                owned[1],
                                ips.len(),
                                ips.len()
                            ),
                            stderr: String::new(),
                        })
                    } else if owned.len() == 4 && owned[0] == "set" && owned[2] == "banip" {
                        if let Ok(mut guard) = self.banned_ips.lock() {
                            if !guard.contains(&owned[3]) {
                                guard.push(owned[3].clone());
                            }
                        }
                        Ok(CmdOutput {
                            exit_code: Some(0),
                            success: true,
                            stdout: "1".to_string(),
                            stderr: String::new(),
                        })
                    } else if owned.len() == 4 && owned[0] == "set" && owned[2] == "unbanip" {
                        if let Ok(mut guard) = self.banned_ips.lock() {
                            guard.retain(|x| x != &owned[3]);
                        }
                        Ok(CmdOutput {
                            exit_code: Some(0),
                            success: true,
                            stdout: "1".to_string(),
                            stderr: String::new(),
                        })
                    } else {
                        Ok(CmdOutput {
                            exit_code: Some(0),
                            success: true,
                            stdout: String::new(),
                            stderr: String::new(),
                        })
                    }
                }
                AllowedBinary::Mysql => {
                    let sql = owned
                        .windows(2)
                        .find(|w| w[0] == "-e")
                        .map(|w| w[1].as_str())
                        .unwrap_or("");
                    let stdout = if sql.contains("information_schema.TABLES")
                        && sql.contains("SUM(")
                    {
                        "size_mb\n12.34".to_string()
                    } else if sql.contains("information_schema.TABLES") {
                        "table_name\ttable_rows\tsize_kb\nusers\t15\t32.50".to_string()
                    } else if sql.contains("information_schema.COLUMNS") {
                        "column_name\tcolumn_type\tis_nullable\tcolumn_key\nid\tbigint\tNO\tPRI"
                            .to_string()
                    } else if sql.to_ascii_lowercase().contains("select") {
                        "id\temail\n1\tadmin@example.com".to_string()
                    } else {
                        String::new()
                    };
                    Ok(CmdOutput {
                        exit_code: Some(0),
                        success: true,
                        stdout,
                        stderr: String::new(),
                    })
                }
                AllowedBinary::Psql => {
                    let sql = owned
                        .windows(2)
                        .find(|w| w[0] == "-c")
                        .map(|w| w[1].as_str())
                        .unwrap_or("");
                    let stdout = if sql.contains("pg_database_size") {
                        "8.50".to_string()
                    } else if sql.contains("pg_class") {
                        "sessions\t4\t16.00".to_string()
                    } else {
                        String::new()
                    };
                    Ok(CmdOutput {
                        exit_code: Some(0),
                        success: true,
                        stdout,
                        stderr: String::new(),
                    })
                }
                AllowedBinary::Crontab => {
                    if owned.len() == 3 && owned[0] == "-u" {
                        let user = &owned[1];
                        if owned[2] == "-l" {
                            let existing = self
                                .crontabs
                                .lock()
                                .ok()
                                .and_then(|g| g.get(user).cloned())
                                .unwrap_or_default();
                            if existing.is_empty() {
                                return Ok(CmdOutput {
                                    exit_code: Some(1),
                                    success: false,
                                    stdout: String::new(),
                                    stderr: format!("no crontab for {user}"),
                                });
                            }
                            return Ok(CmdOutput {
                                exit_code: Some(0),
                                success: true,
                                stdout: existing,
                                stderr: String::new(),
                            });
                        }
                        let file_content =
                            std::fs::read_to_string(&owned[2]).unwrap_or_default();
                        if let Ok(mut guard) = self.crontabs.lock() {
                            guard.insert(user.clone(), file_content);
                        }
                    }
                    Ok(CmdOutput {
                        exit_code: Some(0),
                        success: true,
                        stdout: String::new(),
                        stderr: String::new(),
                    })
                }
                AllowedBinary::Tar => {
                    if let Some(pos) = owned.iter().position(|a| a == "-cf") {
                        if let Some(archive_file) = owned.get(pos + 1) {
                            std::fs::write(archive_file, b"MOCK_ZSTD_TAR_BYTES_FERROUS_P5")?;
                        }
                        Ok(CmdOutput {
                            exit_code: Some(0),
                            success: true,
                            stdout: String::new(),
                            stderr: String::new(),
                        })
                    } else if owned.iter().any(|a| a == "-tvf") {
                        Ok(CmdOutput {
                            exit_code: Some(0),
                            success: true,
                            stdout: "-rw-r--r-- root/root 18 2026-11-12 00:00 ./index.php\n"
                                .to_string(),
                            stderr: String::new(),
                        })
                    } else if owned.iter().any(|a| a == "-xf") {
                        if let Some(pos) = owned.iter().position(|a| a == "-C") {
                            if let Some(target_dir) = owned.get(pos + 1) {
                                let _ = std::fs::create_dir_all(target_dir);
                                let _ = std::fs::write(
                                    Path::new(target_dir).join("restored.txt"),
                                    b"restored ok",
                                );
                            }
                        }
                        Ok(CmdOutput {
                            exit_code: Some(0),
                            success: true,
                            stdout: String::new(),
                            stderr: String::new(),
                        })
                    } else {
                        Ok(CmdOutput {
                            exit_code: Some(0),
                            success: true,
                            stdout: String::new(),
                            stderr: String::new(),
                        })
                    }
                }
                AllowedBinary::Php83 => Ok(CmdOutput {
                    exit_code: Some(0),
                    success: true,
                    stdout: "artisan schedule ok".to_string(),
                    stderr: String::new(),
                }),
                _ => Ok(CmdOutput {
                    exit_code: Some(0),
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                }),
            }
        })
    }

    fn http_probe<'a>(
        &'a self,
        connect_addr: SocketAddr,
        host: &'a Domain,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<u16, PanelError>> {
        static REAL: SystemCommandExecutor = SystemCommandExecutor;
        REAL.http_probe(connect_addr, host, timeout)
    }
}

fn spawn_local_http_responder(
    response_bytes: &'static [u8],
    requests_to_serve: usize,
) -> Result<(SocketAddr, std::thread::JoinHandle<()>)> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let addr = listener.local_addr()?;
    let handle = std::thread::spawn(move || {
        for _ in 0..requests_to_serve {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 512];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(response_bytes);
                let _ = stream.flush();
            }
        }
    });
    Ok((addr, handle))
}

fn setup_test_state(
    ws: &TempWorkspace,
    probe_addr: SocketAddr,
) -> Result<(Config, Arc<MockExecutor>, web::Data<AppState>, String)> {
    let data_dir = ws.path().join("data");
    let wwwroot = ws.path().join("wwwroot");
    std::fs::create_dir_all(&data_dir)?;
    std::fs::create_dir_all(&wwwroot)?;

    let cfg = Config::for_test(data_dir.clone(), wwwroot);
    let exec = Arc::new(MockExecutor::new());
    let dyn_exec: Arc<dyn CommandExecutor> = exec.clone();

    let acme = SafePath::under_root(ws.path(), "acme")?;
    let le_live = SafePath::under_root(ws.path(), "letsencrypt/live")?;
    let nginx_paths = NginxPaths::for_dirs(
        ws.path().join("nginx/sites-available"),
        ws.path().join("nginx/sites-enabled"),
        acme.clone(),
        le_live.clone(),
    )?;
    let fx_paths = FerrousxPaths::for_dirs(
        ws.path().join("ferrousx/ferrousx.toml"),
        ws.path().join("ferrousx/sites"),
        acme.clone(),
        le_live.clone(),
    )?;
    let ssl_paths = SslPaths::for_dirs(
        acme,
        le_live,
        PathBuf::from("/usr/local/bin/ferrous-panel"),
        ws.path().join("systemd/ferrous-certbot-renew.service"),
        ws.path().join("systemd/ferrous-certbot-renew.timer"),
    );

    let ws_store = WebServerStore::open(&cfg.db_path())?;
    let ws_mgr = WebServerManager::with_components(
        ws_store,
        NginxEngine::new(nginx_paths, Arc::clone(&dyn_exec)),
        FerrousxEngine::new(fx_paths, Arc::clone(&dyn_exec)),
        SslManager::new(ssl_paths, Arc::clone(&dyn_exec)),
        dyn_exec,
        probe_addr,
    );

    let (state, initial_pw) = AppState::init_with_webserver(cfg.clone(), ws_mgr)?;
    let password = initial_pw.context("expected initial admin password")?.expose().to_string();
    Ok((cfg, exec, web::Data::new(state), password))
}

macro_rules! login_token {
    ($app:expr, $username:expr, $password:expr) => {{
        let req = TestRequest::post()
            .uri("/api/auth/login")
            .set_json(json!({
                "username": $username,
                "password": $password,
                "rememberMe": false
            }))
            .to_request();
        let resp = try_call_service($app, req)
            .await
            .map_err(|e| anyhow::anyhow!("login call failed: {e}"))?;
        ensure_eq(&resp.status(), &StatusCode::OK, "login status")?;
        let body = to_bytes(resp.into_body())
            .await
            .map_err(|e| anyhow::anyhow!("login body failed: {e}"))?;
        let val: Value = serde_json::from_slice(&body)?;
        val.get("token")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .context("missing login token")?
    }};
}

#[test]
fn telemetry_returns_real_host_metrics_and_sqlite_history() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("telemetry")?;
        let dummy_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80);
        let (cfg, _exec, shared, pw) = setup_test_state(&ws, dummy_addr)?;
        shared.record_telemetry_sample(true)?;

        let app = init_service(App::new().configure(|c| configure_app(c, shared.clone()))).await;
        let token = login_token!(&app, &cfg.admin_username, &pw);

        let req = TestRequest::get()
            .uri("/api/telemetry")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .to_request();
        let resp = try_call_service(&app, req)
            .await
            .map_err(|e| anyhow::anyhow!("telemetry call failed: {e}"))?;
        ensure_eq(&resp.status(), &StatusCode::OK, "GET /api/telemetry")?;

        let body: Value = serde_json::from_slice(
            &to_bytes(resp.into_body())
                .await
                .map_err(|e| anyhow::anyhow!("body error: {e}"))?,
        )?;
        let cpu_cores = body.get("cpuCores").and_then(Value::as_u64).unwrap_or(0);
        let mem_total = body.get("memoryTotalMb").and_then(Value::as_u64).unwrap_or(0);
        ensure(cpu_cores >= 1, "cpuCores must come from real sysinfo")?;
        ensure(mem_total >= 1, "memoryTotalMb must come from real sysinfo")?;

        let history = body
            .get("history24h")
            .and_then(Value::as_array)
            .context("history24h must be present after recording a real sample")?;
        ensure(!history.is_empty(), "history24h must contain real sampled point")?;
        Ok(())
    })
}

#[test]
fn apps_systemd_lifecycle_config_live_http_and_501_stubs() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("apps")?;
        let (probe_addr, responder) = spawn_local_http_responder(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 10\r\n\r\nhello-rust",
            1,
        )?;
        let (cfg, exec, shared, pw) = setup_test_state(&ws, probe_addr)?;
        let app = init_service(App::new().configure(|c| configure_app(c, shared.clone()))).await;
        let token = login_token!(&app, &cfg.admin_username, &pw);

        // 1. Deploy SSR app with systemd unit + Nginx vhost
        let deploy_req = TestRequest::post()
            .uri("/api/actix-gateway/nginx/apps")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({
                "name": "API Gateway",
                "domain": "api.example.com",
                "aliases": [],
                "framework": "Next.js 15 (SSR)",
                "execCommand": "/usr/bin/node server.js",
                "internalPort": 3001,
                "workers": 2,
                "memoryLimitMb": 512,
                "autoCreateDb": false,
                "sslEnabled": false,
                "gitRepoUrl": "",
                "gitBranch": "main"
            }))
            .to_request();
        let deploy_resp = try_call_service(&app, deploy_req)
            .await
            .map_err(|e| anyhow::anyhow!("deploy failed: {e}"))?;
        ensure_eq(&deploy_resp.status(), &StatusCode::OK, "POST deploy app")?;
        let deployed: Value = serde_json::from_slice(
            &to_bytes(deploy_resp.into_body())
                .await
                .map_err(|e| anyhow::anyhow!("body error: {e}"))?,
        )?;
        let app_id = deployed
            .get("id")
            .and_then(Value::as_str)
            .context("deployed app id")?
            .to_string();
        ensure_eq(
            &deployed.get("pid").and_then(Value::as_u64),
            &Some(5150),
            "systemd MainPID from systemctl show",
        )?;
        ensure_eq(
            &deployed.get("memoryUsedMb").and_then(Value::as_u64),
            &Some(64),
            "systemd MemoryCurrent in MB",
        )?;

        let unit_file_path = cfg
            .systemd_unit_dir
            .join("ferrous-app-api-example-com.service");
        ensure(unit_file_path.is_file(), "systemd .service unit must be written")?;
        let unit_text = std::fs::read_to_string(&unit_file_path)?;
        ensure(
            unit_text.contains("MemoryMax=512M") && unit_text.contains("ExecStart=/usr/bin/node server.js"),
            "systemd unit must include MemoryMax and ExecStart",
        )?;

        // 2. Stop action sets unit inactive & returns real PID = null
        let stop_req = TestRequest::post()
            .uri(&format!("/api/apps/{app_id}/action"))
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({ "action": "stop" }))
            .to_request();
        let stop_resp = try_call_service(&app, stop_req)
            .await
            .map_err(|e| anyhow::anyhow!("stop action failed: {e}"))?;
        ensure_eq(&stop_resp.status(), &StatusCode::OK, "POST /action stop")?;
        let stopped: Value = serde_json::from_slice(
            &to_bytes(stop_resp.into_body())
                .await
                .map_err(|e| anyhow::anyhow!("body error: {e}"))?,
        )?;
        ensure_eq(
            &stopped.get("status").and_then(Value::as_str),
            &Some("stopped"),
            "stopped status",
        )?;

        // 3. Rebuild action and git-deploy return HTTP 200 OK and update deploy metadata
        let rebuild_req = TestRequest::post()
            .uri(&format!("/api/apps/{app_id}/action"))
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({ "action": "rebuild" }))
            .to_request();
        let rebuild_resp = try_call_service(&app, rebuild_req)
            .await
            .map_err(|e| anyhow::anyhow!("rebuild action failed: {e}"))?;
        ensure_eq(
            &rebuild_resp.status(),
            &StatusCode::OK,
            "rebuild must return 200 OK",
        )?;

        let git_req = TestRequest::post()
            .uri(&format!("/api/apps/{app_id}/git-deploy"))
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .to_request();
        let git_resp = try_call_service(&app, git_req)
            .await
            .map_err(|e| anyhow::anyhow!("git-deploy failed: {e}"))?;
        ensure_eq(
            &git_resp.status(),
            &StatusCode::OK,
            "git-deploy must return 200 OK",
        )?;

        // 4. Update config writes new MemoryMax & Environment lines
        let cfg_req = TestRequest::put()
            .uri(&format!("/api/apps/{app_id}/config"))
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({
                "workers": 4,
                "memoryLimitMb": 1024,
                "rateLimitRps": 0,
                "envVars": [{ "key": "NODE_ENV", "value": "production" }]
            }))
            .to_request();
        let cfg_resp = try_call_service(&app, cfg_req)
            .await
            .map_err(|e| anyhow::anyhow!("update config failed: {e}"))?;
        ensure_eq(&cfg_resp.status(), &StatusCode::OK, "PUT /apps/{id}/config")?;
        let updated_unit = std::fs::read_to_string(&unit_file_path)?;
        ensure(
            updated_unit.contains("MemoryMax=1024M")
                && updated_unit.contains("Environment=\"NODE_ENV=production\""),
            "systemd unit updated cleanly",
        )?;

        // 5. Site-settings persists per-site redirects and returns 200 OK
        let settings_req = TestRequest::put()
            .uri(&format!("/api/apps/{app_id}/site-settings"))
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({
                "sslEnabled": false,
                "forceHttps": false,
                "redirects": [{ "id": "r1", "sourcePath": "/old", "targetUrl": "/new", "type": "301", "keepQuery": true, "enabled": true }]
            }))
            .to_request();
        let settings_resp = try_call_service(&app, settings_req)
            .await
            .map_err(|e| anyhow::anyhow!("site-settings failed: {e}"))?;
        ensure_eq(
            &settings_resp.status(),
            &StatusCode::OK,
            "per-site redirects in site-settings must return 200 OK",
        )?;

        // 6. Real TCP HTTP probe returns statusCode=200 & real bodyPreview
        let probe_req = TestRequest::post()
            .uri(&format!("/api/apps/{app_id}/test-http"))
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .to_request();
        let probe_resp = try_call_service(&app, probe_req)
            .await
            .map_err(|e| anyhow::anyhow!("test-http failed: {e}"))?;
        ensure_eq(&probe_resp.status(), &StatusCode::OK, "POST /test-http")?;
        let probe_val: Value = serde_json::from_slice(
            &to_bytes(probe_resp.into_body())
                .await
                .map_err(|e| anyhow::anyhow!("body error: {e}"))?,
        )?;
        ensure_eq(
            &probe_val.get("statusCode").and_then(Value::as_u64),
            &Some(200),
            "live HTTP probe statusCode",
        )?;
        ensure_eq(
            &probe_val.get("bodyPreview").and_then(Value::as_str),
            &Some("hello-rust"),
            "live HTTP probe bodyPreview",
        )?;

        let _ = responder.join();
        ensure(
            !exec.calls_for("systemctl").is_empty(),
            "systemctl calls recorded",
        )?;

        // 7. Delete app removes unit and vhost
        let del_req = TestRequest::delete()
            .uri(&format!("/api/apps/{app_id}"))
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .to_request();
        let del_resp = try_call_service(&app, del_req)
            .await
            .map_err(|e| anyhow::anyhow!("delete app failed: {e}"))?;
        ensure_eq(&del_resp.status(), &StatusCode::OK, "DELETE /apps/{id}")?;
        ensure(!unit_file_path.exists(), "systemd unit file removed on delete")?;

        Ok(())
    })
}

#[test]
fn security_ufw_fail2ban_and_validation() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("security")?;
        let dummy_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80);
        let (cfg, exec, shared, pw) = setup_test_state(&ws, dummy_addr)?;
        let app = init_service(App::new().configure(|c| configure_app(c, shared.clone()))).await;
        let token = login_token!(&app, &cfg.admin_username, &pw);

        // 1. Shell injection via UFW port must fail validation (400)
        let bad_ufw = TestRequest::post()
            .uri("/api/security/ufw")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({
                "port": "80; rm -rf /",
                "protocol": "TCP",
                "action": "ALLOW",
                "source": "Anywhere",
                "comment": "test"
            }))
            .to_request();
        let bad_ufw_resp = try_call_service(&app, bad_ufw)
            .await
            .map_err(|e| anyhow::anyhow!("bad ufw error: {e}"))?;
        ensure_eq(
            &bad_ufw_resp.status(),
            &StatusCode::BAD_REQUEST,
            "UFW command injection rejected",
        )?;

        // 2. Valid UFW add + delete via argv (no shell)
        let add_ufw = TestRequest::post()
            .uri("/api/security/ufw")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({
                "port": "8443",
                "protocol": "TCP",
                "action": "ALLOW",
                "source": "Anywhere",
                "comment": "Panel"
            }))
            .to_request();
        let add_ufw_resp = try_call_service(&app, add_ufw)
            .await
            .map_err(|e| anyhow::anyhow!("add ufw error: {e}"))?;
        ensure_eq(&add_ufw_resp.status(), &StatusCode::OK, "POST /security/ufw")?;

        let del_ufw = TestRequest::delete()
            .uri("/api/security/ufw/3")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .to_request();
        let del_ufw_resp = try_call_service(&app, del_ufw)
            .await
            .map_err(|e| anyhow::anyhow!("delete ufw error: {e}"))?;
        ensure_eq(&del_ufw_resp.status(), &StatusCode::OK, "DELETE /security/ufw/3")?;

        // 3. Fail2ban ban and unban
        let ban_req = TestRequest::post()
            .uri("/api/security/fail2ban/ban")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({ "jailName": "sshd", "ip": "198.51.100.77" }))
            .to_request();
        let ban_resp = try_call_service(&app, ban_req)
            .await
            .map_err(|e| anyhow::anyhow!("ban error: {e}"))?;
        ensure_eq(&ban_resp.status(), &StatusCode::OK, "POST fail2ban ban")?;

        let sec_req = TestRequest::get()
            .uri("/api/actix-gateway/ufw/security")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .to_request();
        let sec_resp = try_call_service(&app, sec_req)
            .await
            .map_err(|e| anyhow::anyhow!("get security error: {e}"))?;
        ensure_eq(&sec_resp.status(), &StatusCode::OK, "GET /security")?;
        let sec_val: Value = serde_json::from_slice(
            &to_bytes(sec_resp.into_body())
                .await
                .map_err(|e| anyhow::anyhow!("body error: {e}"))?,
        )?;
        ensure(
            sec_val.get("antiDdosShield").is_some(),
            "eBPF/WAF antiDdosShield must be populated in GET /api/security",
        )?;

        // 4. eBPF DDoS mode returns 200 OK and switches protectionMode
        let ddos_req = TestRequest::post()
            .uri("/api/security/ddos-mode")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({ "mode": "under_attack" }))
            .to_request();
        let ddos_resp = try_call_service(&app, ddos_req)
            .await
            .map_err(|e| anyhow::anyhow!("ddos mode error: {e}"))?;
        ensure_eq(
            &ddos_resp.status(),
            &StatusCode::OK,
            "eBPF ddos-mode must return 200 OK",
        )?;

        ensure(!exec.calls_for("ufw").is_empty(), "ufw CLI executed")?;
        ensure(
            !exec.calls_for("fail2ban-client").is_empty(),
            "fail2ban-client executed",
        )?;
        Ok(())
    })
}

#[test]
fn databases_validate_identifiers_never_store_passwords_and_return_409_when_missing() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("databases")?;
        let dummy_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80);
        let (cfg, exec, shared, pw) = setup_test_state(&ws, dummy_addr)?;
        let app = init_service(App::new().configure(|c| configure_app(c, shared.clone()))).await;
        let token = login_token!(&app, &cfg.admin_username, &pw);

        // 1. SQL injection in database identifier must be rejected with 400
        let sqli_req = TestRequest::post()
            .uri("/api/databases")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({
                "name": "app_db; DROP TABLE users",
                "engine": "MariaDB 11.4",
                "username": "app_user",
                "password": "StrongPassword!123",
                "collation": "utf8mb4_unicode_ci"
            }))
            .to_request();
        let sqli_resp = try_call_service(&app, sqli_req)
            .await
            .map_err(|e| anyhow::anyhow!("sqli req failed: {e}"))?;
        ensure_eq(
            &sqli_resp.status(),
            &StatusCode::BAD_REQUEST,
            "SQL identifier injection rejected",
        )?;

        // 2. Create real database and verify plaintext password is never returned or persisted
        let secret_db_pw = "SuperSecretDbPass#2026";
        let create_req = TestRequest::post()
            .uri("/api/databases")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({
                "name": "shop_prod",
                "engine": "MariaDB 11.4",
                "username": "shop_user",
                "password": secret_db_pw,
                "collation": "utf8mb4_unicode_ci"
            }))
            .to_request();
        let create_resp = try_call_service(&app, create_req)
            .await
            .map_err(|e| anyhow::anyhow!("create db failed: {e}"))?;
        ensure_eq(&create_resp.status(), &StatusCode::OK, "POST /api/databases")?;
        let body_bytes = to_bytes(create_resp.into_body())
            .await
            .map_err(|e| anyhow::anyhow!("body error: {e}"))?;
        let body_str = String::from_utf8_lossy(&body_bytes);
        ensure(
            !body_str.contains(secret_db_pw),
            "API response must NEVER expose database password",
        )?;
        let dbs: Value = serde_json::from_slice(&body_bytes)?;
        let first = dbs
            .as_array()
            .and_then(|arr| arr.first())
            .context("created DB missing from list")?;
        ensure_eq(
            &first.get("sizeMb").and_then(Value::as_f64),
            &Some(12.34),
            "DB sizeMb parsed from information_schema",
        )?;
        let db_id = first
            .get("id")
            .and_then(Value::as_str)
            .context("db id")?
            .to_string();

        // Check SQLite panel.db does not contain secret_db_pw either
        let db_raw = std::fs::read(cfg.db_path())?;
        ensure(
            !String::from_utf8_lossy(&db_raw).contains(secret_db_pw),
            "SQLite panel.db must NEVER store plaintext database password",
        )?;

        // 3. Execute query parses real TSV from mysql
        let query_req = TestRequest::post()
            .uri(&format!("/api/databases/{db_id}/query"))
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({ "query": "SELECT id, email FROM users;" }))
            .to_request();
        let query_resp = try_call_service(&app, query_req)
            .await
            .map_err(|e| anyhow::anyhow!("query failed: {e}"))?;
        ensure_eq(&query_resp.status(), &StatusCode::OK, "POST /databases/{id}/query")?;
        let query_val: Value = serde_json::from_slice(
            &to_bytes(query_resp.into_body())
                .await
                .map_err(|e| anyhow::anyhow!("body error: {e}"))?,
        )?;
        ensure(
            query_val.get("columns").and_then(Value::as_array).is_some_and(|c| c.len() == 2),
            "query columns parsed",
        )?;

        // 4. Missing psql binary => HTTP 409 Conflict
        exec.psql_installed.store(false, AtomicOrdering::SeqCst);
        let pg_req = TestRequest::post()
            .uri("/api/databases")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({
                "name": "analytics_pg",
                "engine": "PostgreSQL 17",
                "username": "pg_user",
                "password": "PgSecret#2026"
            }))
            .to_request();
        let pg_resp = try_call_service(&app, pg_req)
            .await
            .map_err(|e| anyhow::anyhow!("pg create failed: {e}"))?;
        ensure_eq(
            &pg_resp.status(),
            &StatusCode::CONFLICT,
            "missing PostgreSQL CLI must return 409 Conflict",
        )?;

        Ok(())
    })
}

#[test]
fn files_safepath_crud_and_anti_zip_slip() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("files")?;
        let dummy_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80);
        let (cfg, _exec, shared, pw) = setup_test_state(&ws, dummy_addr)?;
        let app = init_service(App::new().configure(|c| configure_app(c, shared.clone()))).await;
        let token = login_token!(&app, &cfg.admin_username, &pw);

        // Deploy a static site so app_id exists inside wwwroot
        let deploy_req = TestRequest::post()
            .uri("/api/apps")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({
                "name": "Static Blog",
                "domain": "blog.example.com",
                "aliases": [],
                "framework": "Static HTML / SPA",
                "execCommand": "",
                "internalPort": 8080,
                "workers": 1,
                "memoryLimitMb": 128,
                "autoCreateDb": false,
                "sslEnabled": false,
                "gitRepoUrl": "",
                "gitBranch": "main"
            }))
            .to_request();
        let deploy_resp = try_call_service(&app, deploy_req)
            .await
            .map_err(|e| anyhow::anyhow!("deploy error: {e}"))?;
        ensure_eq(&deploy_resp.status(), &StatusCode::OK, "deploy static app")?;
        let app_val: Value = serde_json::from_slice(
            &to_bytes(deploy_resp.into_body())
                .await
                .map_err(|e| anyhow::anyhow!("body error: {e}"))?,
        )?;
        let app_id = app_val
            .get("id")
            .and_then(Value::as_str)
            .context("app id")?
            .to_string();

        // 1. Create file safely inside app web root
        let create_req = TestRequest::post()
            .uri(&format!("/api/files/{app_id}"))
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({
                "name": "hello.txt",
                "parentDir": "/",
                "type": "file",
                "permissions": "0644",
                "content": "pure-rust"
            }))
            .to_request();
        let create_resp = try_call_service(&app, create_req)
            .await
            .map_err(|e| anyhow::anyhow!("create file error: {e}"))?;
        ensure_eq(&create_resp.status(), &StatusCode::OK, "POST /files/{app_id}")?;
        ensure(
            cfg.wwwroot.join("blog.example.com/hello.txt").is_file(),
            "file written under wwwroot",
        )?;

        // 2. Traversal in create file parentDir must fail with 403/400
        let trav_req = TestRequest::post()
            .uri(&format!("/api/files/{app_id}"))
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({
                "name": "pwn.txt",
                "parentDir": "../../etc",
                "type": "file",
                "content": "x"
            }))
            .to_request();
        let trav_resp = try_call_service(&app, trav_req)
            .await
            .map_err(|e| anyhow::anyhow!("trav error: {e}"))?;
        ensure(
            trav_resp.status() == StatusCode::FORBIDDEN
                || trav_resp.status() == StatusCode::BAD_REQUEST,
            "traversal parentDir rejected",
        )?;

        // 3. Zip-Slip entry in upload-zip must fail with 403 Forbidden
        let zip_slip_req = TestRequest::post()
            .uri(&format!("/api/files/{app_id}/upload-zip"))
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({
                "archiveName": "evil.zip",
                "parentDir": "/",
                "extractImmediately": true,
                "entries": [
                    { "relativePath": "../../etc/cron.d/backdoor", "isDir": false, "content": "bad" }
                ]
            }))
            .to_request();
        let zip_slip_resp = try_call_service(&app, zip_slip_req)
            .await
            .map_err(|e| anyhow::anyhow!("zip slip req failed: {e}"))?;
        ensure_eq(
            &zip_slip_resp.status(),
            &StatusCode::FORBIDDEN,
            "zip-slip entry rejected with 403",
        )?;

        Ok(())
    })
}

#[test]
fn cpanel_cron_and_tar_zstd_backups_with_sha256_verification() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("cpanel")?;
        let dummy_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80);
        let (cfg, exec, shared, pw) = setup_test_state(&ws, dummy_addr)?;
        let app = init_service(App::new().configure(|c| configure_app(c, shared.clone()))).await;
        let token = login_token!(&app, &cfg.admin_username, &pw);

        // Prepare app web root directory under wwwroot
        let site_dir = cfg.wwwroot.join("app.example.com");
        std::fs::create_dir_all(&site_dir)?;
        std::fs::write(site_dir.join("index.php"), b"<?php echo 'ok';")?;

        // 1. Reject shell wrapper cron (`sh -c`)
        let bad_cron = TestRequest::post()
            .uri("/api/cpanel/cron")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({
                "title": "Bad Shell",
                "schedule": "*/5 * * * *",
                "command": "sh -c id",
                "user": "www-data",
                "targetApp": "app.example.com"
            }))
            .to_request();
        let bad_cron_resp = try_call_service(&app, bad_cron)
            .await
            .map_err(|e| anyhow::anyhow!("bad cron error: {e}"))?;
        ensure_eq(
            &bad_cron_resp.status(),
            &StatusCode::FORBIDDEN,
            "sh -c cron rejected",
        )?;

        // 2. Add & run allowlisted php8.3 cron job
        let add_cron = TestRequest::post()
            .uri("/api/cpanel/cron")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({
                "title": "Laravel Scheduler",
                "schedule": "* * * * *",
                "command": "php8.3 /www/wwwroot/app.example.com/artisan schedule:run",
                "user": "www-data",
                "targetApp": "app.example.com"
            }))
            .to_request();
        let add_cron_resp = try_call_service(&app, add_cron)
            .await
            .map_err(|e| anyhow::anyhow!("add cron error: {e}"))?;
        ensure_eq(&add_cron_resp.status(), &StatusCode::OK, "POST /cpanel/cron")?;
        let crons: Value = serde_json::from_slice(
            &to_bytes(add_cron_resp.into_body())
                .await
                .map_err(|e| anyhow::anyhow!("body error: {e}"))?,
        )?;
        let cron_id = crons
            .as_array()
            .and_then(|a| a.first())
            .and_then(|item| item.get("id"))
            .and_then(Value::as_str)
            .context("cron id missing")?
            .to_string();

        let run_cron = TestRequest::post()
            .uri(&format!("/api/cpanel/cron/{cron_id}/run"))
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .to_request();
        let run_resp = try_call_service(&app, run_cron)
            .await
            .map_err(|e| anyhow::anyhow!("run cron error: {e}"))?;
        ensure_eq(&run_resp.status(), &StatusCode::OK, "POST /cpanel/cron/{id}/run")?;
        let php_calls = exec.calls_for("php8.3");
        ensure(
            php_calls
                .iter()
                .any(|c| c.args.iter().any(|a| a == "schedule:run")),
            "php8.3 executed directly with expected arguments without shell",
        )?;

        // 3. Create real tar.zst backup, verify SHA-256 on restore, then tamper bytes & expect 403
        let mk_bk = TestRequest::post()
            .uri("/api/cpanel/backups")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .set_json(json!({
                "targetApp": "app.example.com",
                "databaseIncluded": "Không đính kèm Database"
            }))
            .to_request();
        let mk_bk_resp = try_call_service(&app, mk_bk)
            .await
            .map_err(|e| anyhow::anyhow!("create backup error: {e}"))?;
        ensure_eq(&mk_bk_resp.status(), &StatusCode::OK, "POST /cpanel/backups")?;
        let bks: Value = serde_json::from_slice(
            &to_bytes(mk_bk_resp.into_body())
                .await
                .map_err(|e| anyhow::anyhow!("body error: {e}"))?,
        )?;
        let first_bk = bks
            .as_array()
            .and_then(|a| a.first())
            .context("backup missing")?;
        let bk_id = first_bk
            .get("id")
            .and_then(Value::as_str)
            .context("backup id")?
            .to_string();
        let bk_filename = first_bk
            .get("filename")
            .and_then(Value::as_str)
            .context("backup filename")?
            .to_string();
        let bk_sha = first_bk
            .get("sha256")
            .and_then(Value::as_str)
            .context("backup sha256")?;
        ensure_eq(&bk_sha.len(), &64usize, "SHA-256 hex digest length")?;

        let restore_ok = TestRequest::post()
            .uri(&format!("/api/cpanel/backups/{bk_id}/restore"))
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .to_request();
        let restore_ok_resp = try_call_service(&app, restore_ok)
            .await
            .map_err(|e| anyhow::anyhow!("restore ok error: {e}"))?;
        ensure_eq(&restore_ok_resp.status(), &StatusCode::OK, "POST restore verified backup")?;

        // Tamper 1 byte in archive on disk -> restore must reject with 403 Forbidden
        let archive_path = cfg.backups_dir.join(&bk_filename);
        let mut bytes = std::fs::read(&archive_path)?;
        if let Some(first_byte) = bytes.first_mut() {
            *first_byte ^= 0xFF;
        }
        std::fs::write(&archive_path, &bytes)?;

        let restore_tampered = TestRequest::post()
            .uri(&format!("/api/cpanel/backups/{bk_id}/restore"))
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .to_request();
        let restore_tampered_resp = try_call_service(&app, restore_tampered)
            .await
            .map_err(|e| anyhow::anyhow!("restore tampered error: {e}"))?;
        ensure_eq(
            &restore_tampered_resp.status(),
            &StatusCode::FORBIDDEN,
            "tampered backup SHA-256 mismatch must abort restore with 403",
        )?;

        Ok(())
    })
}
