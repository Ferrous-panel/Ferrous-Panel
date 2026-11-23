#![forbid(unsafe_code)]

use actix_web::body::to_bytes;
use actix_web::http::{header, StatusCode};
use actix_web::test::{init_service, try_call_service, TestRequest};
use actix_web::App;
use anyhow::{bail, Context, Result};
use ferrous_panel::api::contract::OneClickDeployRequest;
use ferrous_panel::config::{Config, RuntimeMode};
use ferrous_panel::configure_app;
use ferrous_panel::error::PanelError;
use ferrous_panel::modules::extended::{mail, oneclick, software, ssh_admin};
use ferrous_panel::modules::{databases, files, security, ModuleStore};
use ferrous_panel::state::AppState;
use ferrous_panel::system::cmd::{validate_no_flag_injection, AllowedBinary, CmdOutput};
use ferrous_panel::system::paths::SafePath;
use ferrous_panel::system::validate::{Domain, Port};
use ferrous_panel::webserver::ferrousx::{FerrousxEngine, FerrousxPaths};
use ferrous_panel::webserver::nginx::{NginxEngine, NginxPaths};
use ferrous_panel::webserver::ssl::{SslManager, SslPaths};
use ferrous_panel::webserver::{
    BoxFuture, CommandExecutor, SafeFilename, SiteDirectoryAuth, SiteHotlinkProtection, SiteMode,
    SiteRedirectRule, SiteSpec, UpstreamHost, UpstreamTarget, WebServerEngine, WebServerManager,
    WebServerStore, WwwRootPath,
};
use rand::rngs::OsRng;
use rand::RngCore;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
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
        let dir = std::env::temp_dir().join(format!("ferrous-sec-ops-{prefix}-{hex}"));
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
struct RecordedCommand {
    binary: &'static str,
    args: Vec<String>,
    stdin_utf8: String,
}

type MockResponder = Box<dyn Fn(AllowedBinary, &[String], &[u8], usize) -> CmdOutput + Send + Sync>;

struct MockExecutor {
    available: Mutex<HashSet<&'static str>>,
    calls: Mutex<Vec<RecordedCommand>>,
    call_seq: AtomicUsize,
    responder: MockResponder,
}

impl std::fmt::Debug for MockExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockExecutor")
            .field("call_count", &self.call_seq.load(Ordering::SeqCst))
            .finish()
    }
}

impl MockExecutor {
    fn new<F>(available_bins: &[AllowedBinary], responder: F) -> Self
    where
        F: Fn(AllowedBinary, &[String], &[u8], usize) -> CmdOutput + Send + Sync + 'static,
    {
        let mut set = HashSet::new();
        for b in available_bins {
            set.insert(b.label());
        }
        Self {
            available: Mutex::new(set),
            calls: Mutex::new(Vec::new()),
            call_seq: AtomicUsize::new(0),
            responder: Box::new(responder),
        }
    }

    fn recorded(&self) -> Vec<RecordedCommand> {
        self.calls.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

impl CommandExecutor for MockExecutor {
    fn is_binary_available(&self, bin: AllowedBinary) -> bool {
        self.available
            .lock()
            .map(|g| g.contains(bin.label()))
            .unwrap_or(false)
    }

    fn run<'a>(
        &'a self,
        bin: AllowedBinary,
        args: &'a [&'a str],
        timeout: Duration,
    ) -> BoxFuture<'a, Result<CmdOutput, PanelError>> {
        self.run_with_stdin(bin, args, &[], timeout)
    }

    fn run_with_stdin<'a>(
        &'a self,
        bin: AllowedBinary,
        args: &'a [&'a str],
        stdin_bytes: &'a [u8],
        _timeout: Duration,
    ) -> BoxFuture<'a, Result<CmdOutput, PanelError>> {
        let owned_args: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
        let owned_stdin = stdin_bytes.to_vec();
        let seq = self.call_seq.fetch_add(1, Ordering::SeqCst);

        if let Ok(mut g) = self.calls.lock() {
            g.push(RecordedCommand {
                binary: bin.label(),
                args: owned_args.clone(),
                stdin_utf8: String::from_utf8_lossy(&owned_stdin).to_string(),
            });
        }

        let out = (self.responder)(bin, &owned_args, &owned_stdin, seq);
        Box::pin(async move { Ok(out) })
    }
}

fn ok_out(stdout: &str) -> CmdOutput {
    CmdOutput {
        exit_code: Some(0),
        success: true,
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}

fn fail_out(code: i32, stderr: &str) -> CmdOutput {
    CmdOutput {
        exit_code: Some(code),
        success: false,
        stdout: String::new(),
        stderr: stderr.to_string(),
    }
}

// ============================================================================
// 1.3 — WebSSH Allowlist & Zero Fake Output / Isolated Test Mode
// ============================================================================

#[test]
fn web_ssh_enforces_readonly_allowlist_and_rejects_missing_binary_with_409() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("webssh")?;
        let wwwroot = ws.path().join("wwwroot");
        std::fs::create_dir_all(&wwwroot)?;
        let mut cfg = Config::for_test(ws.path().to_path_buf(), wwwroot.clone());
        cfg.mode = RuntimeMode::Production;

        let exec = MockExecutor::new(
            &[
                AllowedBinary::Sysctl,
                AllowedBinary::Ip,
                AllowedBinary::Ss,
                AllowedBinary::Journalctl,
                AllowedBinary::Id,
                AllowedBinary::Ls,
            ],
            |bin, args, _, _| match bin {
                AllowedBinary::Id => ok_out("root\n"),
                AllowedBinary::Sysctl => ok_out(&format!("{} = 4096\n", args.join(" "))),
                AllowedBinary::Ip => ok_out("lo               UNKNOWN        127.0.0.1/8\n"),
                _ => ok_out("ok\n"),
            },
        );

        let forbidden_cmds = [
            "sysctl -w net.ipv4.ip_forward=1",
            "sysctl net.ipv4.ip_forward=1",
            "sysctl -p /etc/sysctl.conf",
            "sysctl --system",
            "sysctl --load=/tmp/bad.conf",
            "ip link set lo down",
            "ip addr add 10.0.0.1/24 dev eth0",
            "ip route del default",
            "ip netns list",
            "ss -K dst 1.2.3.4",
            "ss --kill",
            "journalctl --vacuum-time=1s",
            "journalctl --rotate",
            "journalctl --flush",
            "ls /etc",
            "ls /root",
        ];
        for bad in forbidden_cmds {
            let res = ssh_admin::exec_web_ssh_command(&cfg, &exec, bad, "/").await;
            ensure(
                res.is_err(),
                &format!("WebSSH must reject dangerous command: {bad}"),
            )?;
        }

        // Allowed read-only commands invoke real binaries
        let who = ssh_admin::exec_web_ssh_command(&cfg, &exec, "whoami", "/").await?;
        ensure_eq(&who.exit_code, &0i32, "whoami exit code")?;
        ensure(who.output.contains("root"), "whoami must return id -un output")?;

        let sc = ssh_admin::exec_web_ssh_command(&cfg, &exec, "sysctl net.core.somaxconn", "/").await?;
        ensure_eq(&sc.exit_code, &0i32, "sysctl read exit code")?;

        let ip = ssh_admin::exec_web_ssh_command(&cfg, &exec, "ip -brief addr", "/").await?;
        ensure_eq(&ip.exit_code, &0i32, "ip -brief addr exit code")?;

        // Missing binary MUST return HTTP 409 Conflict, NEVER fake output or exit 0
        let empty_exec = MockExecutor::new(&[], |_, _, _, _| ok_out(""));
        let missing = ssh_admin::exec_web_ssh_command(&cfg, &empty_exec, "whoami", "/").await;
        ensure(
            matches!(missing, Err(PanelError::Conflict(_))),
            "missing binary in WebSSH must return Conflict (409), not fake output",
        )?;

        Ok(())
    })
}

// ============================================================================
// 1.4 — SSRF Protection (Private/CGNAT/Mapped IPv6 + Obfuscated Numeric Hosts)
// ============================================================================

#[test]
fn ssrf_guard_blocks_private_cgnat_mapped_ipv6_and_obfuscated_hosts() -> Result<()> {
    let blocked_ips = [
        "127.0.0.1",
        "10.0.0.1",
        "172.16.5.1",
        "192.168.1.1",
        "100.64.0.1",
        "100.127.255.254",
        "0.0.0.1",
        "192.0.0.1",
        "198.18.0.1",
        "198.19.255.254",
        "169.254.169.254",
        "::1",
        "::ffff:127.0.0.1",
        "::ffff:169.254.169.254",
        "::ffff:100.64.1.2",
        "fe80::1",
        "fc00::1",
    ];
    for raw in blocked_ips {
        let ip: IpAddr = raw.parse().with_context(|| format!("invalid test ip {raw}"))?;
        ensure(
            files::is_forbidden_ssrf_ip(ip),
            &format!("SSRF guard must block IP {raw}"),
        )?;
    }

    let public_ip: IpAddr = "93.184.216.34".parse()?;
    ensure(
        !files::is_forbidden_ssrf_ip(public_ip),
        "public IP 93.184.216.34 must be allowed",
    )?;

    let blocked_urls = [
        "http://2130706433/",
        "http://0x7f.1/",
        "http://0x7f000001/",
        "http://0177.0.0.1/",
        "http://127.1/",
        "http://169.254.169.254/latest/meta-data/",
        "http://[::ffff:127.0.0.1]/",
        "http://metadata.google.internal/",
        "http://localhost:8443/api/health",
        "file:///etc/passwd",
        "gopher://127.0.0.1:6379/",
    ];
    for bad_url in blocked_urls {
        let parsed = files::parse_and_validate_remote_url(bad_url);
        ensure(
            parsed.is_err(),
            &format!("parse_and_validate_remote_url must reject SSRF URL: {bad_url}"),
        )?;
    }

    let good = files::parse_and_validate_remote_url("https://example.com/release/app.tar.gz")?;
    ensure_eq(&good.host, &"example.com".to_string(), "validated host")?;
    ensure_eq(&good.port, &443u16, "validated default port")?;

    Ok(())
}

// ============================================================================
// 1.5 — Tar & Zip Archive Safety (Symlinks & Spaces in Filenames)
// ============================================================================

#[test]
fn archive_validators_reject_symlinks_and_handle_spaces_in_filenames() -> Result<()> {
    // Tar listing with spaces in filenames passes when no symlink/traversal exists
    let tar_names = "docs/my report 2026.txt\nassets/hero banner.png\n";
    let tar_verbose = "\
-rw-r--r-- root/root 1024 2026-01-01 00:00 docs/my report 2026.txt
-rw-r--r-- root/root 4096 2026-01-01 00:00 assets/hero banner.png
";
    let tar_count = files::validate_tar_listings(tar_names, tar_verbose)?;
    ensure_eq(&tar_count, &2usize, "tar clean entries count")?;

    // Tar symlink rejected
    let tar_sym_verbose = "lrwxrwxrwx root/root 0 2026-01-01 00:00 leak -> /etc/passwd\n";
    ensure(
        files::validate_tar_listings("leak\n", tar_sym_verbose).is_err(),
        "tar validator must reject symlink entries",
    )?;

    // Tar path traversal rejected
    ensure(
        files::validate_tar_listings("../etc/passwd\n", "-rw-r--r-- root/root 12 2026-01-01 00:00 ../etc/passwd\n")
            .is_err(),
        "tar validator must reject ../ traversal",
    )?;

    // Zip listing clean
    let zip_info = "\
Archive:  bundle.zip
Zip file size: 2048 bytes, number of entries: 2
-rw-r--r--  3.0 unx      512 tx defN 26-Jan-01 00:00 folder/my file.txt
-rw-r--r--  3.0 unx      256 tx defN 26-Jan-01 00:00 index.html
2 files, 768 bytes uncompressed, 400 bytes compressed:  47.9%
";
    let zip_names = "folder/my file.txt\nindex.html\n";
    let zip_count = files::validate_zip_listings(zip_info, zip_names)?;
    ensure_eq(&zip_count, &2usize, "zip clean entries count")?;

    // Zip symlink rejected
    let zip_sym_info = "\
Archive:  evil.zip
lrwxrwxrwx  3.0 unx       11 tx stor 26-Jan-01 00:00 pwn_link
1 file, 11 bytes uncompressed
";
    ensure(
        files::validate_zip_listings(zip_sym_info, "pwn_link\n").is_err(),
        "zip validator must reject symlink entries",
    )?;

    Ok(())
}

// ============================================================================
// 1.6 — Command Flag Injection Guard
// ============================================================================

#[test]
fn flag_injection_validator_blocks_leading_dashes_and_git_exec_options() -> Result<()> {
    let dangerous = [
        "-c",
        "--config=/etc/passwd",
        "--exec=id",
        "--to-command=sh",
        "--checkpoint-action=exec=sh",
        "core.sshCommand=evil",
        "--upload-pack=id",
    ];
    for arg in dangerous {
        ensure(
            validate_no_flag_injection(arg, "test_arg").is_err(),
            &format!("flag injection guard must reject '{arg}'"),
        )?;
    }

    ensure(
        validate_no_flag_injection("main", "branch").is_ok(),
        "clean branch name must pass",
    )?;
    ensure(
        validate_no_flag_injection("https://github.com/Ferrous-panel/Ferrous-Panel.git", "repo").is_ok(),
        "clean git URL must pass",
    )?;
    Ok(())
}

// ============================================================================
// 2.1 & 2.3 — RuntimeMode & GET /api/capabilities Feature Registry
// ============================================================================

#[test]
fn capabilities_endpoint_and_runtime_mode_report_honest_host_state() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("caps")?;
        let wwwroot = ws.path().join("wwwroot");
        std::fs::create_dir_all(&wwwroot)?;
        let cfg = Config::for_test(ws.path().to_path_buf(), wwwroot);
        ensure(!cfg.is_production(), "Config::for_test must default to RuntimeMode::Test")?;
        ensure_eq(&cfg.mode, &RuntimeMode::Test, "test mode enum")?;

        let (state, secret) = AppState::init(cfg)?;
        let password = secret
            .ok_or_else(|| anyhow::anyhow!("missing first-run secret"))?
            .expose()
            .to_string();

        let shared = actix_web::web::Data::new(state);
        let app = init_service(App::new().configure(|c| configure_app(c, shared.clone()))).await;

        // Must require authentication
        let unauth = try_call_service(&app, TestRequest::get().uri("/api/capabilities").to_request())
            .await
            .map_err(|e| anyhow::anyhow!("unauth capabilities error: {e}"))?;
        ensure_eq(&unauth.status(), &StatusCode::UNAUTHORIZED, "GET /api/capabilities unauth")?;

        let login = try_call_service(
            &app,
            TestRequest::post()
                .uri("/api/auth/login")
                .set_json(json!({ "username": "admin_ferrous", "password": password }))
                .to_request(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("login error: {e}"))?;
        let login_json: Value = serde_json::from_slice(&to_bytes(login.into_body()).await?)?;
        let token = login_json["token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing token"))?;

        let res = try_call_service(
            &app,
            TestRequest::get()
                .uri("/api/capabilities")
                .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
                .to_request(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("GET /api/capabilities error: {e}"))?;
        ensure_eq(&res.status(), &StatusCode::OK, "GET /api/capabilities ok")?;

        let body: Value = serde_json::from_slice(&to_bytes(res.into_body()).await?)?;
        ensure_eq(&body["mode"], &json!("test"), "capabilities runtime mode")?;
        let caps = body["capabilities"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("capabilities must be JSON array"))?;
        ensure(caps.len() >= 10, "capabilities registry must list core and extended features")?;

        let ids: HashSet<&str> = caps
            .iter()
            .filter_map(|c| c.get("id").and_then(Value::as_str))
            .collect();
        for required_id in ["nginx", "ferrousx", "sqlite", "files", "mail_server", "ebpf_xdp"] {
            ensure(
                ids.contains(required_id),
                &format!("capabilities report missing expected id '{required_id}'"),
            )?;
        }
        Ok(())
    })
}

// ============================================================================
// 3.1 — Per-Site Settings Rendering (Both Engines) + htpasswd + HTTP/3 409
// ============================================================================

#[test]
fn per_site_settings_render_for_both_engines_and_reject_unsupported_http3() -> Result<()> {
    let ws = TempWorkspace::new("site-settings")?;
    let acme = SafePath::under_root(ws.path(), "acme")?;
    let le_live = SafePath::under_root(ws.path(), "letsencrypt/live")?;

    let exec: Arc<dyn CommandExecutor> = Arc::new(MockExecutor::new(
        &[AllowedBinary::Nginx, AllowedBinary::Ferrousx],
        |_, _, _, _| ok_out("ok"),
    ));

    let nginx = NginxEngine::new(
        NginxPaths::for_dirs(
            ws.path().join("nginx/sites-available"),
            ws.path().join("nginx/sites-enabled"),
            acme.clone(),
            le_live.clone(),
        )?,
        Arc::clone(&exec),
    );
    let ferrousx = FerrousxEngine::new(
        FerrousxPaths::for_dirs(
            ws.path().join("ferrousx/ferrousx.toml"),
            ws.path().join("ferrousx/sites"),
            acme,
            le_live,
        )?,
        exec,
    );

    let spec = SiteSpec {
        domain: Domain::parse("shop.example.com")?,
        aliases: vec![],
        root: WwwRootPath::parse("/www/wwwroot/shop.example.com")?,
        mode: SiteMode::ReverseProxy {
            upstream: UpstreamTarget::new(UpstreamHost::parse("127.0.0.1")?, Port::new(4000)?),
        },
        ssl_enabled: false,
        redirect_https: false,
        http3_quic: false,
        websocket_enabled: true,
        rate_limit_rps: 25,
        redirects: vec![SiteRedirectRule {
            source_path: "/legacy".to_string(),
            target_url: "https://shop.example.com/v2".to_string(),
            status_code: 301,
            keep_query: true,
            enabled: true,
        }],
        hotlink_protection: Some(SiteHotlinkProtection {
            enabled: true,
            protected_extensions: "jpg,png,webp".to_string(),
            allowed_domains: vec!["cdn.example.com".to_string()],
            block_empty_referer: true,
        }),
        directory_auth: vec![SiteDirectoryAuth {
            path: "/admin".to_string(),
            username: "ops".to_string(),
            password: Some("StrongPass#2026".to_string()),
            label: "Admin Area".to_string(),
            enabled: true,
        }],
    };

    let ng = nginx.render_site(&spec)?;
    ensure(ng.content.contains("limit_req_zone $binary_remote_addr"), "nginx must render limit_req_zone")?;
    ensure(ng.content.contains("limit_req zone=fp_"), "nginx must render limit_req")?;
    ensure(ng.content.contains("return 301 https://shop.example.com/v2$is_args$args;"), "nginx must render redirect")?;
    ensure(ng.content.contains("valid_referers blocked server_names cdn.example.com;"), "nginx must render hotlink valid_referers")?;
    ensure(ng.content.contains("auth_basic \"Admin Area\";"), "nginx must render directoryAuth")?;
    ensure(ng.content.contains("auth_basic_user_file"), "nginx must point to htpasswd file")?;
    ensure(ng.content.contains("proxy_set_header Upgrade $http_upgrade;"), "nginx must render websocket headers")?;

    let fx = ferrousx.render_site(&spec)?;
    ensure(fx.content.contains("rate_limit_rps = 25"), "ferrousx must render rate_limit_rps")?;
    ensure(fx.content.contains("[[redirects]]"), "ferrousx must render [[redirects]]")?;
    ensure(fx.content.contains("[hotlink]"), "ferrousx must render [hotlink]")?;
    ensure(fx.content.contains("[[directory_auth]]"), "ferrousx must render [[directory_auth]]")?;
    ensure(fx.content.contains("websocket = true"), "ferrousx must render websocket = true")?;

    // Staging writes real $apr1$ htpasswd file readable by both engines
    let _ = nginx.stage_site_unvalidated(&spec)?;
    let htpasswd_path = ws.path().join("nginx/htpasswd/shop.example.com.htpasswd");
    let ht_body = std::fs::read_to_string(&htpasswd_path)?;
    ensure(
        ht_body.starts_with("ops:$apr1$"),
        &format!("htpasswd file must contain $apr1$ entry for user ops, got: {ht_body}"),
    )?;

    // Unsupported http3_quic must return Conflict (409) on both stock Nginx and FerrousX
    let mut quic_spec = spec;
    quic_spec.http3_quic = true;
    ensure(
        matches!(nginx.render_site(&quic_spec), Err(PanelError::Conflict(_))),
        "stock nginx must return 409 Conflict for http3_quic=true",
    )?;
    ensure(
        matches!(ferrousx.render_site(&quic_spec), Err(PanelError::Conflict(_))),
        "ferrousx must return 409 Conflict for http3_quic=true",
    )?;

    Ok(())
}

// ============================================================================
// 3.5 — Database Creation Pipes Password via STDIN (Zero Password in Argv)
// ============================================================================

#[test]
fn database_creation_passes_password_via_stdin_never_in_argv() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("db-stdin")?;
        let store = ModuleStore::open(&ws.path().join("panel.db"))?;
        let secret = "S3cr3tDbPass!99";

        let exec = MockExecutor::new(
            &[AllowedBinary::Mysql, AllowedBinary::Psql],
            |_, _, _, _| ok_out("0.01\n"),
        );

        let _ = databases::create_database(
            &store,
            &exec,
            "app_mariadb",
            "MariaDB 11.4",
            "app_user",
            Some(secret),
            "utf8mb4_unicode_ci",
            "",
        )
        .await?;

        let _ = databases::create_database(
            &store,
            &exec,
            "app_pg",
            "PostgreSQL 17",
            "pg_user",
            Some(secret),
            "UTF8",
            "",
        )
        .await?;

        let calls = exec.recorded();
        ensure(!calls.is_empty(), "expected mysql and psql invocations")?;

        let mut saw_mysql_stdin = false;
        let mut saw_psql_stdin = false;
        for call in &calls {
            for arg in &call.args {
                ensure(
                    !arg.contains(secret),
                    &format!("DB secret leaked into command argv for {}: {:?}", call.binary, call.args),
                )?;
            }
            if call.binary == "mysql" && call.stdin_utf8.contains(secret) {
                saw_mysql_stdin = true;
            }
            if call.binary == "psql" && call.stdin_utf8.contains(secret) {
                saw_psql_stdin = true;
            }
        }
        ensure(saw_mysql_stdin, "mysql CREATE USER statement must be delivered via stdin")?;
        ensure(saw_psql_stdin, "psql CREATE ROLE statement must be delivered via stdin")?;

        Ok(())
    })
}

// ============================================================================
// 3.6 — UFW Anti-Lockout Guard Protects SSH & Panel Ports Unless force=true
// ============================================================================

#[test]
fn ufw_anti_lockout_guard_requires_force_for_ssh_and_panel_ports() -> Result<()> {
    run_async(async {
        let numbered_output = "\
Status: active

     To                         Action      From
     --                         ------      ----
[ 1] 22/tcp                     ALLOW IN    Anywhere                   # SSH
[ 2] 8443/tcp                   ALLOW IN    Anywhere                   # Panel
[ 3] 8080/tcp                   ALLOW IN    Anywhere                   # App
";
        let exec = MockExecutor::new(&[AllowedBinary::Ufw], move |_, args, _, _| {
            if args.first().map(String::as_str) == Some("status") {
                ok_out(numbered_output)
            } else {
                ok_out("Rule deleted\n")
            }
        });

        // Blocking SSH port 22 without force => 409 Conflict
        let err_ssh = security::delete_ufw_rule_guarded(&exec, "1", &[22, 8443], false).await;
        ensure(
            matches!(err_ssh, Err(PanelError::Conflict(_))),
            "deleting SSH UFW rule without force must return Conflict (409)",
        )?;

        // Blocking Panel port 8443 without force => 409 Conflict
        let err_panel = security::delete_ufw_rule_guarded(&exec, "2", &[22, 8443], false).await;
        ensure(
            matches!(err_panel, Err(PanelError::Conflict(_))),
            "deleting Panel UFW rule without force must return Conflict (409)",
        )?;

        // Ensure `ufw --force delete` was NEVER executed on blocked calls
        let calls_before = exec.recorded();
        ensure(
            !calls_before.iter().any(|c| c.args.iter().any(|a| a == "delete")),
            "ufw delete must not run when anti-lockout guard blocks deletion",
        )?;

        // Deleting non-protected port 8080 succeeds immediately
        let _ = security::delete_ufw_rule_guarded(&exec, "3", &[22, 8443], false).await?;

        // Deleting protected port 22 WITH force=true succeeds
        let _ = security::delete_ufw_rule_guarded(&exec, "1", &[22, 8443], true).await?;
        let calls_after = exec.recorded();
        ensure(
            calls_after
                .iter()
                .any(|c| c.binary == "ufw" && c.args == vec!["--force", "delete", "1"]),
            "ufw --force delete 1 must run when force=true",
        )?;

        Ok(())
    })
}

// ============================================================================
// 4.2 — Mail Domain Uses Real OpenSSL + Dig Without Fake Verified Status
// ============================================================================

#[test]
fn mail_domain_uses_real_openssl_and_dig_without_fake_verified_status() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("mail-real")?;
        let wwwroot = ws.path().join("wwwroot");
        std::fs::create_dir_all(&wwwroot)?;
        let mut cfg = Config::for_test(ws.path().to_path_buf(), wwwroot);
        cfg.mode = RuntimeMode::Production;
        let store = ModuleStore::open(&cfg.db_path())?;

        let exec = MockExecutor::new(
            &[
                AllowedBinary::OpenSsl,
                AllowedBinary::Dig,
                AllowedBinary::Systemctl,
            ],
            |bin, args, _, _| match bin {
                AllowedBinary::OpenSsl => {
                    if args.first().map(String::as_str) == Some("genpkey") {
                        if let Some(out_idx) = args.iter().position(|a| a == "-out") {
                            if let Some(path) = args.get(out_idx + 1) {
                                let _ = std::fs::write(
                                    path,
                                    "-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQCtest\n-----END PRIVATE KEY-----\n",
                                );
                            }
                        }
                        ok_out("")
                    } else {
                        ok_out(
                            "-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAtestpubkey12345\n-----END PUBLIC KEY-----\n",
                        )
                    }
                }
                AllowedBinary::Dig => ok_out(""), // Empty DNS TXT -> must be "missing", NOT "verified"!
                AllowedBinary::Systemctl => ok_out("inactive\n"),
                _ => ok_out(""),
            },
        );

        let mail_data = mail::add_mail_domain(&store, &cfg, &exec, "mailtest.example.com").await?;
        let dom = mail_data
            .domains
            .iter()
            .find(|d| d.domain == "mailtest.example.com")
            .ok_or_else(|| anyhow::anyhow!("mail domain was not added"))?;

        ensure_eq(&dom.spf_status, &"missing".to_string(), "SPF status when dig empty")?;
        ensure_eq(&dom.dkim_status, &"missing".to_string(), "DKIM status when dig empty")?;
        ensure_eq(&dom.dmarc_status, &"missing".to_string(), "DMARC status when dig empty")?;
        ensure(
            dom.dkim_public_key.contains("p=MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAtestpubkey12345"),
            "DKIM public key must come from real openssl pkey -pubout output",
        )?;

        let calls = exec.recorded();
        ensure(
            calls.iter().any(|c| c.binary == "openssl" && c.args.first().map(String::as_str) == Some("genpkey")),
            "mail domain creation must call openssl genpkey",
        )?;
        ensure(
            calls.iter().any(|c| c.binary == "dig"),
            "mail domain creation must query real DNS via dig",
        )?;

        Ok(())
    })
}

// ============================================================================
// 6.2 — MockExecutor Sequence & Atomic Rollback Verification When Step N Fails
// ============================================================================

#[test]
fn mock_executor_verifies_atomic_rollback_when_command_n_fails() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("rollback-seq")?;
        let acme = SafePath::under_root(ws.path(), "acme")?;
        let le_live = SafePath::under_root(ws.path(), "letsencrypt/live")?;
        let sites_avail = ws.path().join("nginx/sites-available");
        let sites_enabled = ws.path().join("nginx/sites-enabled");
        std::fs::create_dir_all(&sites_avail)?;
        std::fs::create_dir_all(&sites_enabled)?;

        let existing_conf = sites_avail.join("rollback.example.com.conf");
        let existing_link = sites_enabled.join("rollback.example.com.conf");
        let original_bytes = b"# ORIGINAL VALID CONFIG\nserver { listen 80; server_name rollback.example.com; }\n";
        std::fs::write(&existing_conf, original_bytes)?;
        std::os::unix::fs::symlink(&existing_conf, &existing_link)?;

        // Fail specifically on command #0 (`nginx -t`) to assert exact sequence & byte-for-byte rollback
        let exec = Arc::new(MockExecutor::new(
            &[AllowedBinary::Nginx, AllowedBinary::Systemctl],
            |bin, args, _, seq| {
                if bin == AllowedBinary::Nginx && args.first().map(String::as_str) == Some("-t") && seq == 0 {
                    fail_out(1, "nginx: [emerg] simulated syntax failure on step 0")
                } else {
                    ok_out("ok")
                }
            },
        ));
        let dyn_exec: Arc<dyn CommandExecutor> = exec.clone();

        let nginx = NginxEngine::new(
            NginxPaths::for_dirs(sites_avail, sites_enabled, acme, le_live)?,
            dyn_exec,
        );

        let updated_spec = SiteSpec {
            domain: Domain::parse("rollback.example.com")?,
            aliases: vec![],
            root: WwwRootPath::parse("/www/wwwroot/rollback.example.com")?,
            mode: SiteMode::StaticSpa {
                index_file: SafeFilename::parse("index.html")?,
                spa_fallback: true,
            },
            ssl_enabled: false,
            redirect_https: false,
            http3_quic: false,
            websocket_enabled: false,
            rate_limit_rps: 50,
            redirects: Vec::new(),
            hotlink_protection: None,
            directory_auth: Vec::new(),
        };

        let res = nginx.write_site(&updated_spec).await;
        ensure(res.is_err(), "write_site must fail when nginx -t fails on command #0")?;

        let restored = std::fs::read(&existing_conf)?;
        ensure_eq(
            &restored,
            &original_bytes.to_vec(),
            "vhost file must be rolled back byte-for-byte on failure",
        )?;

        let calls = exec.recorded();
        ensure_eq(&calls.len(), &1usize, "only nginx -t should run before rollback")?;
        ensure_eq(&calls[0].binary, &"nginx", "first command binary")?;
        ensure_eq(&calls[0].args, &vec!["-t".to_string()], "first command args")?;

        Ok(())
    })
}

// ============================================================================
// One-Click WordPress Payload Contract, 400 Strict Validation & 409 Pre-Flight
// ============================================================================

#[test]
fn oneclick_request_accepts_create_db_and_legacy_auto_create_db_alias() -> Result<()> {
    let modern: OneClickDeployRequest = serde_json::from_value(json!({
        "templateId": "tpl-wordpress-fpm",
        "domain": "wordpress.example.com",
        "siteTitle": "My Blog",
        "adminEmail": "admin@example.com",
        "createDb": true,
        "enableSsl": true
    }))?;
    ensure_eq(&modern.template_id, &"tpl-wordpress-fpm".to_string(), "template_id")?;
    ensure_eq(&modern.domain, &"wordpress.example.com".to_string(), "domain")?;
    ensure_eq(&modern.create_db, &true, "createDb true")?;
    ensure_eq(&modern.enable_ssl, &true, "enableSsl true")?;
    ensure_eq(&modern.site_title, &Some("My Blog".to_string()), "siteTitle")?;
    ensure_eq(&modern.admin_email, &Some("admin@example.com".to_string()), "adminEmail")?;

    let legacy: OneClickDeployRequest = serde_json::from_value(json!({
        "templateId": "tpl-wordpress-fpm",
        "domain": "blog.example.com",
        "autoCreateDb": true
    }))?;
    ensure_eq(&legacy.create_db, &true, "autoCreateDb alias must map to create_db")?;
    ensure_eq(&legacy.enable_ssl, &false, "enableSsl defaults to false")?;

    let minimal: OneClickDeployRequest = serde_json::from_value(json!({
        "templateId": "tpl-tailwind-static",
        "domain": "static.example.com"
    }))?;
    ensure_eq(&minimal.create_db, &false, "create_db defaults to false")?;

    Ok(())
}

#[test]
fn oneclick_wordpress_preflight_returns_409_and_api_rejects_invalid_json_with_400() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("oneclick-wp")?;
        let wwwroot = ws.path().join("wwwroot");
        std::fs::create_dir_all(&wwwroot)?;
        let cfg = Config::for_test(ws.path().to_path_buf(), wwwroot.clone());

        let (state, secret) = AppState::init(cfg.clone())?;
        let password = secret
            .ok_or_else(|| anyhow::anyhow!("missing first-run secret"))?
            .expose()
            .to_string();

        let shared = actix_web::web::Data::new(state);
        let app = init_service(App::new().configure(|c| configure_app(c, shared.clone()))).await;

        let login = try_call_service(
            &app,
            TestRequest::post()
                .uri("/api/auth/login")
                .set_json(json!({ "username": "admin_ferrous", "password": password }))
                .to_request(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("login error: {e}"))?;
        let login_json: Value = serde_json::from_slice(&to_bytes(login.into_body()).await?)?;
        let token = login_json["token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing token"))?;

        // Invalid/incomplete JSON MUST return 400 Bad Request, never silently deploy demo.ferrous-panel.local
        let bad_req = try_call_service(
            &app,
            TestRequest::post()
                .uri("/api/oneclick/deploy")
                .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
                .set_json(json!({}))
                .to_request(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("POST /api/oneclick/deploy invalid json error: {e}"))?;
        ensure_eq(
            &bad_req.status(),
            &StatusCode::BAD_REQUEST,
            "POST /api/oneclick/deploy empty JSON must return 400",
        )?;

        // Unknown templateId MUST return 404 Not Found
        let unknown_tpl = try_call_service(
            &app,
            TestRequest::post()
                .uri("/api/oneclick/deploy")
                .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
                .set_json(json!({
                    "templateId": "tpl-does-not-exist",
                    "domain": "missing.example.com",
                    "createDb": false
                }))
                .to_request(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("POST /api/oneclick/deploy unknown template error: {e}"))?;
        ensure_eq(
            &unknown_tpl.status(),
            &StatusCode::NOT_FOUND,
            "unknown 1-Click template must return 404",
        )?;

        // Production WordPress preflight without PHP-FPM/MariaDB MUST return 409 Conflict (never 502)
        let mut prod_cfg = cfg;
        prod_cfg.mode = RuntimeMode::Production;
        let mod_store = ModuleStore::open(&ws.path().join("oneclick-prod.db"))?;
        let acme = SafePath::under_root(ws.path(), "acme-oc")?;
        let le_live = SafePath::under_root(ws.path(), "le-oc")?;
        let empty_exec: Arc<dyn CommandExecutor> =
            Arc::new(MockExecutor::new(&[], |_, _, _, _| fail_out(1, "missing")));
        let ws_mgr = WebServerManager::with_components(
            WebServerStore::open(&ws.path().join("oneclick-ws.db"))?,
            NginxEngine::new(
                NginxPaths::for_dirs(
                    ws.path().join("oc-nginx/avail"),
                    ws.path().join("oc-nginx/enabled"),
                    acme.clone(),
                    le_live.clone(),
                )?,
                Arc::clone(&empty_exec),
            ),
            FerrousxEngine::new(
                FerrousxPaths::for_dirs(
                    ws.path().join("oc-fx/ferrousx.toml"),
                    ws.path().join("oc-fx/sites"),
                    acme.clone(),
                    le_live.clone(),
                )?,
                Arc::clone(&empty_exec),
            ),
            SslManager::new(
                SslPaths::for_dirs(
                    acme,
                    le_live,
                    PathBuf::from("/usr/local/bin/ferrous-panel"),
                    ws.path().join("oc-systemd/renew.service"),
                    ws.path().join("oc-systemd/renew.timer"),
                ),
                Arc::clone(&empty_exec),
            ),
            empty_exec,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80),
        );

        let wp_req = OneClickDeployRequest {
            template_id: "tpl-wordpress-fpm".to_string(),
            domain: "wordpress.example.com".to_string(),
            site_title: Some("WP Test".to_string()),
            admin_email: None,
            create_db: true,
            enable_ssl: false,
            web_server_engine: None,
        };
        let wp_res = oneclick::deploy_oneclick(wp_req, &mod_store, &prod_cfg, &ws_mgr).await;
        ensure(
            matches!(wp_res, Err(PanelError::Conflict(_))),
            &format!(
                "WordPress 1-Click missing prerequisites must return 409 Conflict, got: {wp_res:?}"
            ),
        )?;

        Ok(())
    })
}

// ============================================================================
// FerrousX Restart Pre-Check & Automatic Competing Web Server Rollback
// ============================================================================

#[test]
fn ferrousx_restart_aborts_before_stopping_nginx_when_config_check_fails() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("fx-precheck")?;
        let wwwroot = ws.path().join("wwwroot");
        std::fs::create_dir_all(&wwwroot)?;
        let mut cfg = Config::for_test(ws.path().to_path_buf(), wwwroot);
        cfg.mode = RuntimeMode::Production;
        let store = ModuleStore::open(&ws.path().join("panel.db"))?;

        let exec = MockExecutor::new(
            &[
                AllowedBinary::Ferrousx,
                AllowedBinary::Nginx,
                AllowedBinary::Systemctl,
            ],
            |bin, args, _, _| match bin {
                AllowedBinary::Ferrousx => {
                    fail_out(1, "ferrousx: config check failed: missing /etc/ferrousx/ferrousx.toml")
                }
                AllowedBinary::Systemctl
                    if args.first().map(String::as_str) == Some("is-active")
                        && args.get(1).map(String::as_str) == Some("nginx.service") =>
                {
                    ok_out("active\n")
                }
                _ => ok_out("inactive\n"),
            },
        );

        let res = software::software_package_action(&store, &cfg, &exec, "pkg-ferrousx", "restart")
            .await;
        ensure(
            matches!(res, Err(PanelError::Conflict(_))),
            &format!("expected 409 Conflict on invalid ferrousx config, got: {res:?}"),
        )?;

        let calls = exec.recorded();
        ensure(
            !calls
                .iter()
                .any(|c| c.binary == "systemctl" && c.args == vec!["stop", "nginx.service"]),
            "nginx.service must NEVER be stopped when ferrousx --check-config fails",
        )?;

        Ok(())
    })
}

#[test]
fn ferrousx_restart_rolls_back_to_nginx_when_ferrousx_fails_to_stay_active() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("fx-rollback")?;
        let wwwroot = ws.path().join("wwwroot");
        std::fs::create_dir_all(&wwwroot)?;
        let mut cfg = Config::for_test(ws.path().to_path_buf(), wwwroot);
        cfg.mode = RuntimeMode::Production;
        let store = ModuleStore::open(&ws.path().join("panel.db"))?;

        let exec = MockExecutor::new(
            &[
                AllowedBinary::Ferrousx,
                AllowedBinary::Nginx,
                AllowedBinary::Systemctl,
                AllowedBinary::Journalctl,
            ],
            |bin, args, _, _| match bin {
                AllowedBinary::Ferrousx => ok_out("ferrousx: config syntax OK\n"),
                AllowedBinary::Systemctl => {
                    let sub = args.first().map(String::as_str).unwrap_or("");
                    let unit = args.get(1).map(String::as_str).unwrap_or("");
                    match (sub, unit) {
                        ("cat", "ferrousx.service") => ok_out("[Unit]\nDescription=FerrousX\n"),
                        ("is-active", "nginx.service") => ok_out("active\n"),
                        ("is-active", "ferrousx.service") => fail_out(3, "failed\n"),
                        ("restart", "ferrousx.service") => fail_out(1, "Job for ferrousx.service failed"),
                        ("stop", _) | ("start", "nginx.service") => ok_out(""),
                        _ => ok_out("inactive\n"),
                    }
                }
                AllowedBinary::Journalctl => ok_out("ferrousx: bind failed"),
                _ => ok_out(""),
            },
        );

        let res = software::software_package_action(&store, &cfg, &exec, "pkg-ferrousx", "restart")
            .await;
        ensure(
            matches!(res, Err(PanelError::Conflict(_))),
            &format!("expected 409 Conflict with rollback on ferrousx failure, got: {res:?}"),
        )?;

        let calls = exec.recorded();
        ensure(
            calls
                .iter()
                .any(|c| c.binary == "systemctl" && c.args == vec!["start", "nginx.service"]),
            "nginx.service must be automatically restarted on ferrousx restart failure rollback",
        )?;

        Ok(())
    })
}
