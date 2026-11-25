#![forbid(unsafe_code)]

use actix_web::body::to_bytes;
use actix_web::http::{header, StatusCode};
use actix_web::test::{init_service, try_call_service, TestRequest};
use actix_web::App;
use anyhow::{bail, Context, Result};
use chrono::{TimeZone, Utc};
use ferrous_panel::auth::store::AuthStore;
use ferrous_panel::config::Config;
use ferrous_panel::configure_app;
use ferrous_panel::error::PanelError;
use ferrous_panel::state::AppState;
use ferrous_panel::system::cmd::{AllowedBinary, CmdOutput};
use ferrous_panel::system::paths::SafePath;
use ferrous_panel::system::validate::{Domain, Port};
use ferrous_panel::webserver::ferrousx::{FerrousxEngine, FerrousxPaths};
use ferrous_panel::webserver::nginx::{NginxEngine, NginxPaths};
use ferrous_panel::webserver::ssl::{
    parse_openssl_x509_output, SslCertState, SslManager, SslPaths,
};
use ferrous_panel::webserver::{
    AcmeEmail, BoxFuture, CommandExecutor, EngineKind, PhpFpmVersion, SafeFilename, SiteMode,
    SiteSpec, SystemCommandExecutor, UpstreamHost, UpstreamTarget, WebServerEngine,
    WebServerManager, WebServerStore, WwwRootPath,
};
use rand::rngs::OsRng;
use rand::RngCore;
use serde_json::{json, Value};
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
        let dir = std::env::temp_dir().join(format!("ferrous-panel-ws-{prefix}-{hex}"));
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
struct FakeExecutor {
    nginx_installed: AtomicBool,
    ferrousx_installed: AtomicBool,
    nginx_active: AtomicBool,
    ferrousx_active: AtomicBool,
    nginx_validates: AtomicBool,
    ferrousx_validates: AtomicBool,
    ferrousx_pid: u32,
    probe_use_real_tcp: AtomicBool,
    probe_fixed_status: Mutex<Option<u16>>,
    calls: Mutex<Vec<RecordedCall>>,
}

impl FakeExecutor {
    fn new(nginx_installed: bool, ferrousx_installed: bool) -> Self {
        Self {
            nginx_installed: AtomicBool::new(nginx_installed),
            ferrousx_installed: AtomicBool::new(ferrousx_installed),
            nginx_active: AtomicBool::new(true),
            ferrousx_active: AtomicBool::new(false),
            nginx_validates: AtomicBool::new(true),
            ferrousx_validates: AtomicBool::new(true),
            ferrousx_pid: 4242,
            probe_use_real_tcp: AtomicBool::new(true),
            probe_fixed_status: Mutex::new(None),
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

    fn recorded_calls(&self) -> Vec<RecordedCall> {
        self.calls.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

impl CommandExecutor for FakeExecutor {
    fn is_binary_available(&self, bin: AllowedBinary) -> bool {
        match bin {
            AllowedBinary::Nginx => self.nginx_installed.load(AtomicOrdering::SeqCst),
            AllowedBinary::Ferrousx => self.ferrousx_installed.load(AtomicOrdering::SeqCst),
            AllowedBinary::Certbot | AllowedBinary::OpenSsl | AllowedBinary::Systemctl | AllowedBinary::Kill => true,
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
        let owned_args: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();

        Box::pin(async move {
            match bin {
                AllowedBinary::Nginx => {
                    let valid = self.nginx_validates.load(AtomicOrdering::SeqCst);
                    Ok(CmdOutput {
                        exit_code: Some(if valid { 0 } else { 1 }),
                        success: valid,
                        stdout: String::new(),
                        stderr: if valid {
                            "nginx: the configuration file /etc/nginx/nginx.conf syntax is ok"
                                .to_string()
                        } else {
                            "nginx: [emerg] unexpected \"}\" in /etc/nginx/sites-enabled/bad.conf:12"
                                .to_string()
                        },
                    })
                }
                AllowedBinary::Ferrousx => {
                    let valid = self.ferrousx_validates.load(AtomicOrdering::SeqCst);
                    Ok(CmdOutput {
                        exit_code: Some(if valid { 0 } else { 1 }),
                        success: valid,
                        stdout: if valid {
                            "ferrousx-bin: config syntax OK".to_string()
                        } else {
                            String::new()
                        },
                        stderr: if valid {
                            String::new()
                        } else {
                            "ferrousx-bin: invalid TOML directive in sites/example.com.toml"
                                .to_string()
                        },
                    })
                }
                AllowedBinary::Kill => Ok(CmdOutput {
                    exit_code: Some(0),
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                }),
                AllowedBinary::Systemctl => {
                    let sub = owned_args.first().map(String::as_str).unwrap_or("");
                    match sub {
                        "start" => {
                            if let Some(unit) = owned_args.get(1) {
                                if unit == "nginx" {
                                    self.nginx_active.store(true, AtomicOrdering::SeqCst);
                                } else if unit == "ferrousx" {
                                    self.ferrousx_active.store(true, AtomicOrdering::SeqCst);
                                }
                            }
                            Ok(CmdOutput {
                                exit_code: Some(0),
                                success: true,
                                stdout: String::new(),
                                stderr: String::new(),
                            })
                        }
                        "stop" => {
                            if let Some(unit) = owned_args.get(1) {
                                if unit == "nginx" {
                                    self.nginx_active.store(false, AtomicOrdering::SeqCst);
                                } else if unit == "ferrousx" {
                                    self.ferrousx_active.store(false, AtomicOrdering::SeqCst);
                                }
                            }
                            Ok(CmdOutput {
                                exit_code: Some(0),
                                success: true,
                                stdout: String::new(),
                                stderr: String::new(),
                            })
                        }
                        "reload" => Ok(CmdOutput {
                            exit_code: Some(0),
                            success: true,
                            stdout: String::new(),
                            stderr: String::new(),
                        }),
                        "show" => {
                            let is_fx = owned_args.iter().any(|a| a == "ferrousx");
                            let is_nginx = owned_args.iter().any(|a| a == "nginx");
                            let wants_value = owned_args.iter().any(|a| a == "--value");
                            if wants_value && is_fx {
                                let pid = if self.ferrousx_active.load(AtomicOrdering::SeqCst) {
                                    self.ferrousx_pid
                                } else {
                                    0
                                };
                                return Ok(CmdOutput {
                                    exit_code: Some(0),
                                    success: true,
                                    stdout: pid.to_string(),
                                    stderr: String::new(),
                                });
                            }
                            let active = if is_fx {
                                self.ferrousx_active.load(AtomicOrdering::SeqCst)
                            } else if is_nginx {
                                self.nginx_active.load(AtomicOrdering::SeqCst)
                            } else {
                                false
                            };
                            let (state, sub_s, pid) = if active {
                                (
                                    "active",
                                    "running",
                                    if is_fx { self.ferrousx_pid } else { 1111 },
                                )
                            } else {
                                ("inactive", "dead", 0)
                            };
                            Ok(CmdOutput {
                                exit_code: Some(0),
                                success: true,
                                stdout: format!(
                                    "ActiveState={state}\nSubState={sub_s}\nMainPID={pid}\nLoadState=loaded\nUnitFileState=enabled\n"
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
        if self.probe_use_real_tcp.load(AtomicOrdering::SeqCst) {
            static REAL: SystemCommandExecutor = SystemCommandExecutor;
            REAL.http_probe(connect_addr, host, timeout)
        } else {
            let status_opt = self.probe_fixed_status.lock().ok().and_then(|g| *g);
            Box::pin(async move {
                match status_opt {
                    Some(code) => Ok(code),
                    None => Err(PanelError::BadGateway(
                        "simulated probe failure".to_string(),
                    )),
                }
            })
        }
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

fn sample_static_site() -> Result<SiteSpec> {
    Ok(SiteSpec {
        domain: Domain::parse("spa.example.com")?,
        aliases: vec![Domain::parse("www.spa.example.com")?],
        root: WwwRootPath::parse("/www/wwwroot/spa.example.com")?,
        mode: SiteMode::StaticSpa {
            index_file: SafeFilename::parse("index.html")?,
            spa_fallback: true,
        },
        ssl_enabled: false,
        redirect_https: false,
        http3_quic: false,
        websocket_enabled: false,
        rate_limit_rps: 0,
        redirects: Vec::new(),
        hotlink_protection: None,
        directory_auth: Vec::new(),
    })
}

fn sample_proxy_ssl_site() -> Result<SiteSpec> {
    Ok(SiteSpec {
        domain: Domain::parse("api.example.com")?,
        aliases: vec![],
        root: WwwRootPath::parse("/www/wwwroot/api.example.com")?,
        mode: SiteMode::ReverseProxy {
            upstream: UpstreamTarget::new(UpstreamHost::parse("127.0.0.1")?, Port::new(3000)?),
        },
        ssl_enabled: true,
        redirect_https: true,
        http3_quic: false,
        websocket_enabled: false,
        rate_limit_rps: 0,
        redirects: Vec::new(),
        hotlink_protection: None,
        directory_auth: Vec::new(),
    })
}

fn sample_php_site() -> Result<SiteSpec> {
    Ok(SiteSpec {
        domain: Domain::parse("wp.example.com")?,
        aliases: vec![],
        root: WwwRootPath::parse("/www/wwwroot/wp.example.com")?,
        mode: SiteMode::PhpFpm {
            php_version: PhpFpmVersion::V83,
            index_file: SafeFilename::parse("index.php")?,
        },
        ssl_enabled: false,
        redirect_https: false,
        http3_quic: false,
        websocket_enabled: false,
        rate_limit_rps: 0,
        redirects: Vec::new(),
        hotlink_protection: None,
        directory_auth: Vec::new(),
    })
}

fn build_test_manager(
    ws: &TempWorkspace,
    exec: Arc<FakeExecutor>,
    probe_addr: SocketAddr,
) -> Result<WebServerManager> {
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

    let dyn_exec: Arc<dyn CommandExecutor> = exec;
    let store = WebServerStore::open(&ws.path().join("panel.db"))?;
    let nginx = NginxEngine::new(nginx_paths, Arc::clone(&dyn_exec));
    let ferrousx = FerrousxEngine::new(fx_paths, Arc::clone(&dyn_exec));
    let ssl = SslManager::new(ssl_paths, Arc::clone(&dyn_exec));

    Ok(WebServerManager::with_components(
        store, nginx, ferrousx, ssl, dyn_exec, probe_addr,
    ))
}

#[test]
fn render_site_snapshots_for_nginx_and_ferrousx_match_contract() -> Result<()> {
    let exec: Arc<dyn CommandExecutor> = Arc::new(FakeExecutor::new(true, true));
    let nginx = NginxEngine::new(NginxPaths::default_system()?, Arc::clone(&exec)).with_ipv6(true);
    let ferrousx = FerrousxEngine::new(FerrousxPaths::default_system()?, exec);

    // 1. Nginx static/SPA snapshot
    let static_site = sample_static_site()?;
    let nginx_spa = nginx.render_site(&static_site)?;
    let expected_nginx_spa = "\
# Managed by FerrousPanel (NginxEngine) — spa.example.com
server {
    listen 80;
    listen [::]:80;
    server_name spa.example.com www.spa.example.com;

    root /www/wwwroot/spa.example.com;
    server_tokens off;

    location ^~ /.well-known/acme-challenge/ {
        root /www/acme;
        default_type \"text/plain\";
        try_files $uri =404;
    }

    location = /phpadmin {
        return 301 $scheme://$http_host/phpadmin/;
    }

    location ^~ /phpadmin/ {
        proxy_pass http://127.0.0.1:888;
        proxy_set_header Host $http_host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    location = /phpmyadmin {
        return 301 $scheme://$http_host/phpmyadmin/;
    }

    location ^~ /phpmyadmin/ {
        proxy_pass http://127.0.0.1:888;
        proxy_set_header Host $http_host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    index index.html;
    location ~* \\.(?:mjs|js|css|wasm|woff2|woff|svg|webp|avif)$ {
        expires 30d;
        add_header Cache-Control \"public, max-age=2592000\";
        add_header X-Content-Type-Options \"nosniff\" always;
        try_files $uri =404;
    }
    location / {
        try_files $uri $uri.html $uri/ /index.html;
    }
}
";
    ensure_eq(&nginx_spa.content, &expected_nginx_spa.to_string(), "nginx static/SPA snapshot")?;
    ensure_eq(
        &nginx_spa.config_path,
        &PathBuf::from("/etc/nginx/sites-available/spa.example.com.conf"),
        "nginx config_path",
    )?;

    // 2. Nginx reverse_proxy + SSL + redirect_https snapshot (ACME challenge served BEFORE 301 redirect)
    let proxy_site = sample_proxy_ssl_site()?;
    let nginx_proxy = nginx.render_site(&proxy_site)?;
    let expected_nginx_proxy = "\
# Managed by FerrousPanel (NginxEngine) — api.example.com
server {
    listen 80;
    listen [::]:80;
    server_name api.example.com;

    location ^~ /.well-known/acme-challenge/ {
        root /www/acme;
        default_type \"text/plain\";
        try_files $uri =404;
    }

    location / {
        return 301 https://$host$request_uri;
    }
}

server {
    listen 443 ssl http2;
    listen [::]:443 ssl http2;
    server_name api.example.com;

    ssl_certificate /etc/letsencrypt/live/api.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/api.example.com/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_prefer_server_ciphers on;
    ssl_session_cache shared:FerrousSSL:10m;
    ssl_session_timeout 1d;
    ssl_session_tickets off;

    root /www/wwwroot/api.example.com;
    server_tokens off;

    location ^~ /.well-known/acme-challenge/ {
        root /www/acme;
        default_type \"text/plain\";
        try_files $uri =404;
    }

    location = /phpadmin {
        return 301 $scheme://$http_host/phpadmin/;
    }

    location ^~ /phpadmin/ {
        proxy_pass http://127.0.0.1:888;
        proxy_set_header Host $http_host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    location = /phpmyadmin {
        return 301 $scheme://$http_host/phpmyadmin/;
    }

    location ^~ /phpmyadmin/ {
        proxy_pass http://127.0.0.1:888;
        proxy_set_header Host $http_host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    location / {
        client_max_body_size 128m;
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
";
    ensure_eq(
        &nginx_proxy.content,
        &expected_nginx_proxy.to_string(),
        "nginx reverse_proxy + ssl snapshot",
    )?;

    // 3. Nginx PHP-FPM snapshot
    let php_site = sample_php_site()?;
    let nginx_php = nginx.render_site(&php_site)?;
    ensure(
        nginx_php.content.contains("fastcgi_pass unix:/run/php/php8.3-fpm.sock;"),
        "nginx php_fpm snapshot missing php8.3 socket",
    )?;

    // 4. FerrousX master TOML + site TOML snapshots
    let master_toml = ferrousx.render_master_config(4);
    let expected_master = "\
# Managed by FerrousPanel — FerrousX master configuration
worker_threads = 4
bind_http = \"0.0.0.0:80\"
bind_https = \"0.0.0.0:443\"
sites_dir = \"/etc/ferrousx/sites\"
acme_webroot = \"/www/acme\"
acme_challenge_prefix = \"/.well-known/acme-challenge/\"
enable_phpmyadmin_route = true
phpmyadmin_upstream = \"127.0.0.1:888\"
server_tokens = false
";
    ensure_eq(&master_toml, &expected_master.to_string(), "ferrousx master.toml snapshot")?;

    let fx_proxy = ferrousx.render_site(&proxy_site)?;
    let expected_fx_proxy = "\
# Managed by FerrousPanel (FerrousxEngine) — api.example.com
domain = \"api.example.com\"
server_names = [\"api.example.com\"]
root = \"/www/wwwroot/api.example.com\"
rate_limit_rps = 0

[acme]
prefix = \"/.well-known/acme-challenge/\"
webroot = \"/www/acme\"
serve_before_redirect = true

[ssl]
enabled = true
redirect_https = true
cert_path = \"/etc/letsencrypt/live/api.example.com/fullchain.pem\"
key_path = \"/etc/letsencrypt/live/api.example.com/privkey.pem\"

[handler]
kind = \"reverse_proxy\"
upstream = \"http://127.0.0.1:3000\"
pass_host_header = true
forwarded_headers = true
";
    ensure_eq(
        &fx_proxy.content,
        &expected_fx_proxy.to_string(),
        "ferrousx reverse_proxy + ssl snapshot",
    )?;
    ensure_eq(
        &fx_proxy.config_path,
        &PathBuf::from("/etc/ferrousx/sites/api.example.com.toml"),
        "ferrousx site path",
    )?;

    let fx_php = ferrousx.render_site(&php_site)?;
    ensure(
        fx_php.content.contains("fastcgi_socket = \"/run/php/php8.3-fpm.sock\""),
        "ferrousx php_fpm snapshot missing socket path",
    )?;

    Ok(())
}

#[test]
fn unsupported_site_modes_return_501_and_invalid_ssl_redirect_returns_400() -> Result<()> {
    let exec: Arc<dyn CommandExecutor> = Arc::new(FakeExecutor::new(true, true));
    let nginx = NginxEngine::new(NginxPaths::default_system()?, exec);

    // Unsupported SiteMode serialized from JSON => SiteMode::Unsupported => 501
    let mut unsupported_mode: SiteSpec = serde_json::from_value(json!({
        "domain": "edge.example.com",
        "root": "/www/wwwroot/edge.example.com",
        "mode": { "kind": "wasi_worker" }
    }))?;
    ensure(
        matches!(
            nginx.render_site(&unsupported_mode),
            Err(PanelError::NotImplemented(_))
        ),
        "unsupported SiteMode must return NotImplemented (501)",
    )?;

    // Unsupported HTTP/3 QUIC on stock Nginx (without --with-http_v3_module) => 409 Conflict
    unsupported_mode.mode = SiteMode::StaticSpa {
        index_file: SafeFilename::parse("index.html")?,
        spa_fallback: false,
    };
    unsupported_mode.http3_quic = true;
    ensure(
        matches!(nginx.render_site(&unsupported_mode), Err(PanelError::Conflict(_))),
        "http3Quic without http_v3_module support must return Conflict (409)",
    )?;

    // Invalid redirectHttps=true with sslEnabled=false => 400 Validation
    let mut invalid_ssl = sample_static_site()?;
    invalid_ssl.redirect_https = true;
    ensure(
        matches!(nginx.render_site(&invalid_ssl), Err(PanelError::Validation(_))),
        "redirectHttps without sslEnabled must fail with Validation (400)",
    )?;

    // Newtype validation checks on invalid inputs
    ensure(Domain::parse("bad domain; rm -rf /").is_err(), "domain must reject spaces/semicolons")?;
    ensure(WwwRootPath::parse("../../etc/passwd").is_err(), "WwwRootPath must reject traversal")?;
    ensure(WwwRootPath::parse("/etc/nginx/sites-available").is_err(), "WwwRootPath must reject paths outside /www/wwwroot")?;
    ensure(UpstreamHost::parse("127.0.0.1;\n").is_err(), "UpstreamHost must reject newline/semicolon")?;
    ensure(SafeFilename::parse("../secret").is_err(), "SafeFilename must reject traversal")?;
    ensure(AcmeEmail::parse("bad email@example.com").is_err(), "AcmeEmail must reject whitespace")?;

    Ok(())
}

#[test]
fn write_site_rolls_back_previous_config_and_symlink_when_validate_fails() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("write-rollback")?;
        let exec = Arc::new(FakeExecutor::new(true, true));
        let mgr = build_test_manager(&ws, Arc::clone(&exec), SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80))?;

        // 1. Nginx: existing file + symlink must be restored byte-for-byte when `nginx -t` fails
        let site = sample_static_site()?;
        let avail = mgr.nginx.paths().sites_available.join("spa.example.com.conf");
        let enabled = mgr.nginx.paths().sites_enabled.join("spa.example.com.conf");
        std::fs::create_dir_all(&mgr.nginx.paths().sites_available)?;
        std::fs::create_dir_all(&mgr.nginx.paths().sites_enabled)?;
        let original_nginx = b"# PREVIOUS VALID NGINX VHOST\nserver { listen 80; server_name spa.example.com; }\n";
        std::fs::write(&avail, original_nginx)?;
        std::os::unix::fs::symlink(&avail, &enabled)?;

        exec.nginx_validates.store(false, AtomicOrdering::SeqCst);
        let write_err = mgr.nginx.write_site(&site).await;
        ensure(write_err.is_err(), "nginx write_site must fail when nginx -t fails")?;

        let restored_nginx = std::fs::read(&avail)?;
        ensure_eq(&restored_nginx, &original_nginx.to_vec(), "restored nginx config bytes")?;
        let restored_target = std::fs::read_link(&enabled)?;
        ensure_eq(&restored_target, &avail, "restored nginx symlink")?;

        // 2. Nginx: brand-new file must be deleted cleanly (not left half-written) when `nginx -t` fails
        let new_site = sample_php_site()?;
        let new_avail = mgr.nginx.paths().sites_available.join("wp.example.com.conf");
        let new_enabled = mgr.nginx.paths().sites_enabled.join("wp.example.com.conf");
        ensure(mgr.nginx.write_site(&new_site).await.is_err(), "new nginx site write must fail validation")?;
        ensure(!new_avail.exists(), "failed new nginx vhost file must be removed")?;
        ensure(std::fs::symlink_metadata(&new_enabled).is_err(), "failed new nginx symlink must be removed")?;

        // 3. FerrousX: existing file restored byte-for-byte when `ferrousx-bin --check-config` fails
        let fx_site_path = mgr.ferrousx.paths().sites_dir.join("spa.example.com.toml");
        std::fs::create_dir_all(&mgr.ferrousx.paths().sites_dir)?;
        let original_fx = b"# PREVIOUS VALID FERROUSX TOML\ndomain = \"spa.example.com\"\n";
        std::fs::write(&fx_site_path, original_fx)?;

        exec.ferrousx_validates.store(false, AtomicOrdering::SeqCst);
        let fx_err = mgr.ferrousx.write_site(&site).await;
        ensure(fx_err.is_err(), "ferrousx write_site must fail when --check-config fails")?;
        let restored_fx = std::fs::read(&fx_site_path)?;
        ensure_eq(&restored_fx, &original_fx.to_vec(), "restored ferrousx site TOML")?;

        // 4. FerrousX reload uses MainPID from systemctl show + kill -HUP
        exec.ferrousx_active.store(true, AtomicOrdering::SeqCst);
        mgr.ferrousx.reload().await?;
        let calls = exec.recorded_calls();
        let sent_sighup = calls.iter().any(|c| {
            c.binary == "kill" && c.args == vec!["-HUP".to_string(), "4242".to_string()]
        });
        ensure(sent_sighup, "FerrousxEngine::reload must send kill -HUP to MainPID from systemctl show")?;

        Ok(())
    })
}

#[test]
fn switch_aborts_before_stopping_nginx_on_validation_error_and_rolls_back_on_health_failure() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("switch-rollback")?;
        let exec = Arc::new(FakeExecutor::new(true, false));

        let addr_unreach = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1);
        let mgr = build_test_manager(&ws, Arc::clone(&exec), addr_unreach)?;
        let auth_store = AuthStore::open(&ws.path().join("panel.db"))?;

        let site = sample_static_site()?;
        mgr.store.upsert_site(&site)?;

        // Case A: FerrousX not installed => HTTP 409 Conflict
        let res_not_installed = mgr
            .switch_engine(EngineKind::Ferrousx, &auth_store, "admin_ferrous", "127.0.0.1")
            .await;
        ensure(
            matches!(res_not_installed, Err(PanelError::Conflict(_))),
            "switching to uninstalled ferrousx must return Conflict (409)",
        )?;

        // Case B: FerrousX installed, but --check-config fails => Nginx MUST NOT be stopped!
        exec.ferrousx_installed.store(true, AtomicOrdering::SeqCst);
        exec.ferrousx_validates.store(false, AtomicOrdering::SeqCst);
        exec.calls.lock().map(|mut g| g.clear()).ok();

        let res_bad_cfg = mgr
            .switch_engine(EngineKind::Ferrousx, &auth_store, "admin_ferrous", "127.0.0.1")
            .await;
        ensure(res_bad_cfg.is_err(), "switch must fail when target validation fails")?;
        ensure(
            exec.nginx_active.load(AtomicOrdering::SeqCst),
            "nginx must stay running when target validate fails in Step 1",
        )?;
        let stopped_nginx = exec
            .recorded_calls()
            .iter()
            .any(|c| c.binary == "systemctl" && c.args == vec!["stop".to_string(), "nginx".to_string()]);
        ensure(!stopped_nginx, "systemctl stop nginx must never run if validation failed")?;

        // Case C: Validation succeeds, old stops, new starts, HTTP health check FAILS => rollback restarts nginx!
        exec.ferrousx_validates.store(true, AtomicOrdering::SeqCst);
        exec.probe_use_real_tcp.store(false, AtomicOrdering::SeqCst);
        if let Ok(mut st) = exec.probe_fixed_status.lock() {
            *st = Some(502);
        }
        exec.calls.lock().map(|mut g| g.clear()).ok();

        let res_health_fail = mgr
            .switch_engine(EngineKind::Ferrousx, &auth_store, "admin_ferrous", "127.0.0.1")
            .await;
        ensure(
            matches!(res_health_fail, Err(PanelError::BadGateway(_))),
            "switch must return BadGateway and rollback when site returns 502",
        )?;
        ensure(
            !exec.ferrousx_active.load(AtomicOrdering::SeqCst),
            "ferrousx must be stopped during rollback",
        )?;
        ensure(
            exec.nginx_active.load(AtomicOrdering::SeqCst),
            "nginx must be restarted during rollback",
        )?;
        ensure_eq(
            &mgr.store.load_config()?.frontend_engine,
            &EngineKind::Nginx,
            "persisted frontend_engine after rollback",
        )?;

        // Case D: Real local HTTP server responds 200 OK => switch succeeds cleanly & never holds both ports!
        let (probe_addr, join_srv) = spawn_local_http_responder(
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
            1,
        )?;
        let exec_ok = Arc::new(FakeExecutor::new(true, true));
        let mgr_ok = build_test_manager(&ws, Arc::clone(&exec_ok), probe_addr)?;

        let outcome = mgr_ok
            .switch_engine(EngineKind::Ferrousx, &auth_store, "admin_ferrous", "127.0.0.1")
            .await?;
        let _ = join_srv.join();

        ensure_eq(&outcome.previous_engine, &EngineKind::Nginx, "previous engine")?;
        ensure_eq(&outcome.active_engine, &EngineKind::Ferrousx, "active engine")?;
        ensure(!outcome.rolled_back, "switch should not report rollback on success")?;
        ensure_eq(&outcome.sites_validated, &1usize, "sites validated count")?;
        ensure(
            !exec_ok.nginx_active.load(AtomicOrdering::SeqCst)
                && exec_ok.ferrousx_active.load(AtomicOrdering::SeqCst),
            "only ferrousx must be active after switch",
        )?;

        let calls = exec_ok.recorded_calls();
        let stop_idx = calls
            .iter()
            .position(|c| c.binary == "systemctl" && c.args == vec!["stop", "nginx"])
            .ok_or_else(|| anyhow::anyhow!("missing stop nginx call"))?;
        let start_idx = calls
            .iter()
            .position(|c| c.binary == "systemctl" && c.args == vec!["start", "ferrousx"])
            .ok_or_else(|| anyhow::anyhow!("missing start ferrousx call"))?;
        ensure(
            stop_idx < start_idx,
            "old engine must stop strictly before new engine starts to avoid holding 80/443 simultaneously",
        )?;

        ensure_eq(
            &mgr_ok.store.load_config()?.frontend_engine,
            &EngineKind::Ferrousx,
            "persisted frontend_engine after switch",
        )?;

        Ok(())
    })
}

#[test]
fn ssl_real_cert_parser_certbot_args_and_renewal_timer_units() -> Result<()> {
    let ws = TempWorkspace::new("ssl")?;
    let exec = Arc::new(FakeExecutor::new(true, true));
    let acme = SafePath::under_root(ws.path(), "acme")?;
    let le_live = SafePath::under_root(ws.path(), "letsencrypt/live")?;
    let ssl = SslManager::new(
        SslPaths::for_dirs(
            acme,
            le_live,
            PathBuf::from("/usr/local/bin/ferrous-panel"),
            ws.path().join("ferrous-certbot-renew.service"),
            ws.path().join("ferrous-certbot-renew.timer"),
        ),
        exec,
    );

    let domain = Domain::parse("example.com")?;
    let alias = Domain::parse("www.example.com")?;
    let email = AcmeEmail::parse("ops@example.com")?;

    let args = ssl.build_certbot_args(&domain, &[alias], Some(&email))?;
    ensure(args.starts_with(&["certonly".to_string()]), "must use certbot certonly")?;
    ensure(args.contains(&"--webroot".to_string()), "must use --webroot")?;
    ensure(args.contains(&"-w".to_string()), "must pass -w webroot flag")?;
    ensure(args.contains(&"--deploy-hook".to_string()), "must include --deploy-hook")?;

    let unit_service = ssl.render_renewal_systemd_service();
    ensure(
        unit_service.contains("ExecStart=/usr/bin/certbot renew --quiet --deploy-hook"),
        "renewal service must invoke certbot renew with --deploy-hook",
    )?;
    let unit_timer = ssl.render_renewal_systemd_timer();
    ensure(
        unit_timer.contains("OnCalendar=*-*-* 00,12:00:00"),
        "renewal timer must schedule twice daily",
    )?;

    // Parse real openssl x509 output for Valid, ExpiringSoon, and Expired certs
    let now = Utc
        .with_ymd_and_hms(2026, 11, 12, 12, 0, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid reference time"))?;

    let valid_out = "\
notBefore=Oct  1 00:00:00 2026 GMT
notAfter=Dec 30 00:00:00 2026 GMT
issuer=C = US, O = Let's Encrypt, CN = R10
subject=CN = example.com
";
    let parsed_valid = parse_openssl_x509_output(
        &domain,
        "/etc/letsencrypt/live/example.com/fullchain.pem",
        "/etc/letsencrypt/live/example.com/privkey.pem",
        valid_out,
        now,
    )?;
    ensure_eq(&parsed_valid.status, &SslCertState::Valid, "valid cert state")?;
    ensure_eq(&parsed_valid.days_remaining, &Some(47), "valid cert remaining days")?;

    let soon_out = "\
notBefore=Sep  1 00:00:00 2026 GMT
notAfter=Nov 20 12:00:00 2026 GMT
issuer=C = US, O = Let's Encrypt, CN = R10
subject=CN = example.com
";
    let parsed_soon = parse_openssl_x509_output(
        &domain,
        "/etc/letsencrypt/live/example.com/fullchain.pem",
        "/etc/letsencrypt/live/example.com/privkey.pem",
        soon_out,
        now,
    )?;
    ensure_eq(&parsed_soon.status, &SslCertState::ExpiringSoon, "expiring soon cert state")?;

    let expired_out = "\
notBefore=Jul  1 00:00:00 2026 GMT
notAfter=Nov  1 00:00:00 2026 GMT
issuer=C = US, O = Let's Encrypt, CN = R10
subject=CN = example.com
";
    let parsed_expired = parse_openssl_x509_output(
        &domain,
        "/etc/letsencrypt/live/example.com/fullchain.pem",
        "/etc/letsencrypt/live/example.com/privkey.pem",
        expired_out,
        now,
    )?;
    ensure_eq(&parsed_expired.status, &SslCertState::Expired, "expired cert state")?;

    // Verify Certbot lineage reconciliation (`archive/<domain>/fullchainN.pem` and `live/<domain>-0001/`)
    let arch_dir = ws.path().join("letsencrypt/archive/wp.example.com");
    std::fs::create_dir_all(&arch_dir)?;
    std::fs::write(arch_dir.join("fullchain1.pem"), b"CERT_V1")?;
    std::fs::write(arch_dir.join("privkey1.pem"), b"KEY_V1")?;
    std::fs::write(arch_dir.join("fullchain2.pem"), b"CERT_V2")?;
    std::fs::write(arch_dir.join("privkey2.pem"), b"KEY_V2")?;
    let wp_dom = Domain::parse("wp.example.com")?;
    ensure(
        ssl.site_cert_ready(&wp_dom),
        "site_cert_ready must reconcile archive/<domain>/fullchain2.pem into live/<domain>",
    )?;
    let (wp_cert, wp_key) = ssl.site_cert_paths(&wp_dom)?;
    ensure_eq(
        &std::fs::read(&wp_cert)?,
        &b"CERT_V2".to_vec(),
        "reconciled highest-numbered archive cert",
    )?;
    ensure_eq(
        &std::fs::read(&wp_key)?,
        &b"KEY_V2".to_vec(),
        "reconciled highest-numbered archive key",
    )?;

    let suffixed_live = ws.path().join("letsencrypt/live/blog.example.com-0001");
    std::fs::create_dir_all(&suffixed_live)?;
    std::fs::write(suffixed_live.join("fullchain.pem"), b"CERT_0001")?;
    std::fs::write(suffixed_live.join("privkey.pem"), b"KEY_0001")?;
    let blog_dom = Domain::parse("blog.example.com")?;
    ensure(
        ssl.site_cert_ready(&blog_dom),
        "site_cert_ready must reconcile live/<domain>-0001 into live/<domain>",
    )?;

    Ok(())
}

#[test]
fn api_webserver_endpoints_enforce_auth_validation_501_and_409() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("api-webserver")?;
        let wwwroot = ws.path().join("wwwroot");
        std::fs::create_dir_all(&wwwroot)?;
        let cfg = Config::for_test(ws.path().to_path_buf(), wwwroot);

        let (probe_addr, join_srv) = spawn_local_http_responder(
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
            1,
        )?;
        let exec = Arc::new(FakeExecutor::new(true, false));
        let mgr = build_test_manager(&ws, Arc::clone(&exec), probe_addr)?;

        let (state, initial_secret) = AppState::init_with_webserver(cfg, mgr)?;
        let password = initial_secret
            .ok_or_else(|| anyhow::anyhow!("missing first-run password"))?
            .expose()
            .to_string();

        let shared_state = actix_web::web::Data::new(state);
        let app = init_service(App::new().configure(|c| configure_app(c, shared_state.clone()))).await;

        // 1. Unauthenticated GET /api/webserver => 401
        let unauth = try_call_service(&app, TestRequest::get().uri("/api/webserver").to_request())
            .await
            .map_err(|e| anyhow::anyhow!("GET /api/webserver failed: {e}"))?;
        ensure_eq(&unauth.status(), &StatusCode::UNAUTHORIZED, "unauthenticated GET /api/webserver")?;

        // Login to get bearer token
        let login_res = try_call_service(
            &app,
            TestRequest::post()
                .uri("/api/auth/login")
                .set_json(json!({ "username": "admin_ferrous", "password": password }))
                .to_request(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("login failed: {e}"))?;
        ensure_eq(&login_res.status(), &StatusCode::OK, "login status")?;
        let login_bytes = to_bytes(login_res.into_body())
            .await
            .map_err(|e| anyhow::anyhow!("read login body failed: {e}"))?;
        let login_json: Value = serde_json::from_slice(&login_bytes)?;
        let token = login_json["token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing token"))?
            .to_string();
        let bearer = format!("Bearer {token}");

        // 2. PUT /api/webserver with traversal root => 400 validation error from WwwRootPath newtype
        let bad_path_res = try_call_service(
            &app,
            TestRequest::put()
                .uri("/api/webserver")
                .insert_header((header::AUTHORIZATION, bearer.clone()))
                .set_json(json!({
                    "sites": [{
                        "domain": "app.example.com",
                        "root": "../../etc/passwd",
                        "mode": { "kind": "static_spa", "index_file": "index.html", "spa_fallback": true }
                    }]
                }))
                .to_request(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("PUT bad root error: {e}"))?;
        ensure_eq(&bad_path_res.status(), &StatusCode::BAD_REQUEST, "traversal root status")?;

        // 3. PUT /api/webserver with unsupported mode => 501 Not Implemented
        let unsupported_res = try_call_service(
            &app,
            TestRequest::put()
                .uri("/api/webserver")
                .insert_header((header::AUTHORIZATION, bearer.clone()))
                .set_json(json!({
                    "sites": [{
                        "domain": "app.example.com",
                        "root": "/www/wwwroot/app.example.com",
                        "mode": { "kind": "cgi_legacy" }
                    }]
                }))
                .to_request(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("PUT unsupported mode error: {e}"))?;
        ensure_eq(&unsupported_res.status(), &StatusCode::NOT_IMPLEMENTED, "unsupported mode 501")?;

        // 4. PUT /api/webserver with valid StaticSpa site => 200 OK & persisted
        let put_ok_res = try_call_service(
            &app,
            TestRequest::put()
                .uri("/api/webserver")
                .insert_header((header::AUTHORIZATION, bearer.clone()))
                .set_json(json!({
                    "frontendEngine": "nginx",
                    "workerThreads": 4,
                    "sites": [{
                        "domain": "app.example.com",
                        "root": "/www/wwwroot/app.example.com",
                        "mode": { "kind": "static_spa", "index_file": "index.html", "spa_fallback": true },
                        "sslEnabled": false,
                        "redirectHttps": false
                    }]
                }))
                .to_request(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("PUT valid sites error: {e}"))?;
        ensure_eq(&put_ok_res.status(), &StatusCode::OK, "PUT /api/webserver ok")?;
        let put_bytes = to_bytes(put_ok_res.into_body())
            .await
            .map_err(|e| anyhow::anyhow!("read PUT body failed: {e}"))?;
        let put_json: Value = serde_json::from_slice(&put_bytes)?;
        ensure_eq(&put_json["frontendEngine"], &json!("nginx"), "frontendEngine default nginx")?;
        ensure_eq(&put_json["sitesCount"], &json!(1), "sitesCount updated")?;

        // 5. POST /api/webserver/switch when ferrousx is NOT installed => 409 Conflict
        let switch_409 = try_call_service(
            &app,
            TestRequest::post()
                .uri("/api/webserver/switch")
                .insert_header((header::AUTHORIZATION, bearer.clone()))
                .set_json(json!({ "targetEngine": "ferrousx" }))
                .to_request(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("POST switch 409 error: {e}"))?;
        ensure_eq(&switch_409.status(), &StatusCode::CONFLICT, "switch to uninstalled ferrousx 409")?;

        // 6. Enable ferrousx binary => POST /api/webserver/switch => 200 OK
        exec.ferrousx_installed.store(true, AtomicOrdering::SeqCst);
        let switch_ok = try_call_service(
            &app,
            TestRequest::post()
                .uri("/api/webserver/switch")
                .insert_header((header::AUTHORIZATION, bearer.clone()))
                .set_json(json!({ "targetEngine": "ferrousx" }))
                .to_request(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("POST switch ok error: {e}"))?;
        let _ = join_srv.join();
        ensure_eq(&switch_ok.status(), &StatusCode::OK, "switch ok status")?;
        let switch_bytes = to_bytes(switch_ok.into_body())
            .await
            .map_err(|e| anyhow::anyhow!("read switch body failed: {e}"))?;
        let switch_json: Value = serde_json::from_slice(&switch_bytes)?;
        ensure_eq(&switch_json["activeEngine"], &json!("ferrousx"), "switched activeEngine")?;

        // 7. GET /api/webserver reflects persisted frontendEngine = "ferrousx"
        let get_after = try_call_service(
            &app,
            TestRequest::get()
                .uri("/api/webserver")
                .insert_header((header::AUTHORIZATION, bearer))
                .to_request(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("GET after switch error: {e}"))?;
        ensure_eq(&get_after.status(), &StatusCode::OK, "GET /api/webserver after switch")?;
        let get_bytes = to_bytes(get_after.into_body())
            .await
            .map_err(|e| anyhow::anyhow!("read GET body failed: {e}"))?;
        let get_json: Value = serde_json::from_slice(&get_bytes)?;
        ensure_eq(&get_json["frontendEngine"], &json!("ferrousx"), "GET persisted frontendEngine")?;

        Ok(())
    })
}

#[test]
fn ferrousx_uds_ipc_client_and_engine_reload_follow_upstream_protocol() -> Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixListener;

    run_async(async {
        let ws = TempWorkspace::new("fx-uds-ipc")?;
        let exec = Arc::new(FakeExecutor::new(true, true));
        let mgr = build_test_manager(
            &ws,
            Arc::clone(&exec),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80),
        )?;

        let sock_path = mgr.ferrousx.paths().control_socket.clone();
        let token_path = mgr.ferrousx.paths().admin_token_file.clone();
        if let Some(parent) = sock_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let expected_token =
            "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
        std::fs::write(&token_path, format!("{expected_token}\n"))?;

        let listener = UnixListener::bind(&sock_path)?;
        let seen_actions: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let seen_clone = Arc::clone(&seen_actions);
        let token_owned = expected_token.to_string();

        let server_task = tokio::spawn(async move {
            let mut epoch = 4u64;
            for _ in 0..4 {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let mut buf = Vec::new();
                let _ = stream.read_to_end(&mut buf).await;
                let trimmed = String::from_utf8_lossy(&buf).trim().to_string();
                let Ok(req) = serde_json::from_str::<Value>(&trimmed) else {
                    break;
                };
                let action = req["action"].as_str().unwrap_or("").to_string();
                let auth_ok = req["auth_token"].as_str() == Some(token_owned.as_str());
                if let Ok(mut guard) = seen_clone.lock() {
                    guard.push(action.clone());
                }

                let resp = if !auth_ok {
                    json!({
                        "success": false,
                        "message": "403 Unauthorized: invalid token",
                        "data": null,
                        "latency_us": 5
                    })
                } else if action == "status" {
                    json!({
                        "success": true,
                        "message": "FerrousX online",
                        "data": {
                            "epoch": epoch,
                            "workers": 4,
                            "vhosts_count": 3,
                            "upstreams_count": 1,
                            "quic_enabled": true,
                            "waf_enabled": true,
                            "waf_mode": "Block",
                            "uptime_secs": 120,
                            "total_requests": 500,
                            "active_connections": 2
                        },
                        "latency_us": 18
                    })
                } else {
                    epoch += 1;
                    json!({
                        "success": true,
                        "message": format!("RCU reload OK ({action})"),
                        "data": {
                            "action": action,
                            "epoch": epoch,
                            "vhosts_count": 3
                        },
                        "latency_us": 31
                    })
                };
                let out = serde_json::to_vec(&resp).unwrap_or_default();
                let _ = stream.write_all(&out).await;
                let _ = stream.flush().await;
                let _ = stream.shutdown().await;
            }
        });

        let client = mgr.ferrousx.ipc_client();
        ensure(client.socket_available(), "UDS socket must be detected as available")?;

        // 1. status query
        let (live, latency_us, _msg) = client.status().await?;
        ensure_eq(&live.epoch, &4u64, "live status epoch")?;
        ensure_eq(&live.vhosts_count, &3u64, "live status vhosts_count")?;
        ensure_eq(&latency_us, &18u64, "live status latency_us")?;

        // 2. FerrousxEngine::reload uses UDS IPC rather than kill -HUP when socket is live
        mgr.ferrousx.reload().await?;
        let kill_called = exec.recorded_calls().iter().any(|c| c.binary == "kill");
        ensure(!kill_called, "FerrousxEngine::reload should use UDS IPC instead of SIGHUP when socket is live")?;

        // 3. vhost_add and panel_sync actions
        mgr.ferrousx.notify_vhost_add("spa.example.com").await?;
        let sync_res = client
            .panel_sync(json!({ "source": "ferrous-panel" }))
            .await?;
        ensure_eq(&sync_res.epoch, &7u64, "epoch after reload + vhost_add + panel_sync")?;

        // 4. Disallowed IPC action rejected client-side before connecting
        let bad_action = client.send_command("rm_rf", json!({})).await;
        ensure(
            matches!(bad_action, Err(PanelError::Validation(_))),
            "disallowed IPC action must fail with Validation error",
        )?;

        let _ = server_task.await;
        let recorded = seen_actions.lock().map(|g| g.clone()).unwrap_or_default();
        ensure_eq(
            &recorded,
            &vec![
                "status".to_string(),
                "reload".to_string(),
                "vhost_add".to_string(),
                "panel_sync".to_string(),
            ],
            "ordered IPC actions received over panel-admin.sock",
        )?;

        Ok(())
    })
}

