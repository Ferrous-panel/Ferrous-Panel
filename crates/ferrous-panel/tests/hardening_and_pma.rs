#![forbid(unsafe_code)]

use actix_web::body::to_bytes;
use actix_web::http::{header, StatusCode};
use actix_web::middleware::from_fn;
use actix_web::test::{init_service, try_call_service, TestRequest};
use actix_web::{web, App};
use anyhow::{bail, Context, Result};
use ferrous_panel::auth::middleware::{security_headers_middleware, SESSION_COOKIE_NAME};
use ferrous_panel::config::Config;
use ferrous_panel::configure_app;
use ferrous_panel::error::PanelError;
use ferrous_panel::state::AppState;
use ferrous_panel::system::cmd::{AllowedBinary, CmdOutput};
use ferrous_panel::system::paths::SafePath;
use ferrous_panel::system::validate::{validate_panel_port, Domain};
use ferrous_panel::webserver::ferrousx::{FerrousxEngine, FerrousxPaths};
use ferrous_panel::webserver::nginx::{NginxEngine, NginxPaths};
use ferrous_panel::webserver::ssl::{SslManager, SslPaths};
use ferrous_panel::webserver::{
    BoxFuture, CommandExecutor, SystemCommandExecutor, WebServerManager, WebServerStore,
};
use rand::rngs::OsRng;
use rand::RngCore;
use serde_json::{json, Value};
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
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

async fn read_body_bytes<B>(body: B) -> Result<Vec<u8>>
where
    B: actix_web::body::MessageBody,
    B::Error: std::fmt::Display,
{
    let b = to_bytes(body)
        .await
        .map_err(|e| anyhow::anyhow!("failed to read HTTP body: {e}"))?;
    Ok(b.to_vec())
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
        let dir = std::env::temp_dir().join(format!("ferrous-panel-harden-{prefix}-{hex}"));
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

#[derive(Debug, Default)]
struct MinimalMockExecutor;

impl CommandExecutor for MinimalMockExecutor {
    fn is_binary_available(&self, bin: AllowedBinary) -> bool {
        matches!(
            bin,
            AllowedBinary::Nginx
                | AllowedBinary::Systemctl
                | AllowedBinary::Tar
                | AllowedBinary::Certbot
        )
    }

    fn run<'a>(
        &'a self,
        bin: AllowedBinary,
        _args: &'a [&'a str],
        _timeout: Duration,
    ) -> BoxFuture<'a, Result<CmdOutput, PanelError>> {
        Box::pin(async move {
            let stdout = match bin {
                AllowedBinary::Systemctl => {
                    "ActiveState=active\nSubState=running\nMainPID=4242\nMemoryCurrent=33554432\n"
                        .to_string()
                }
                _ => String::new(),
            };
            Ok(CmdOutput {
                exit_code: Some(0),
                success: true,
                stdout,
                stderr: String::new(),
            })
        })
    }

    fn http_probe<'a>(
        &'a self,
        connect_addr: SocketAddr,
        host: &'a Domain,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<u16, PanelError>> {
        let real = SystemCommandExecutor;
        real.http_probe(connect_addr, host, timeout)
    }
}

fn setup_test_state(ws: &TempWorkspace) -> Result<(Config, web::Data<AppState>, String)> {
    let data_dir = ws.path().join("data");
    let wwwroot = ws.path().join("wwwroot");
    std::fs::create_dir_all(&data_dir)?;
    std::fs::create_dir_all(&wwwroot)?;

    let cfg = Config::for_test(data_dir, wwwroot);
    let dyn_exec: Arc<dyn CommandExecutor> = Arc::new(MinimalMockExecutor);
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
    let probe_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 18080);
    let ws_mgr = WebServerManager::with_components(
        ws_store,
        NginxEngine::new(nginx_paths, Arc::clone(&dyn_exec)),
        FerrousxEngine::new(fx_paths, Arc::clone(&dyn_exec)),
        SslManager::new(ssl_paths, Arc::clone(&dyn_exec)),
        dyn_exec,
        probe_addr,
    );

    let (state, initial_pw) = AppState::init_with_webserver(cfg.clone(), ws_mgr)?;
    let pw = initial_pw
        .map(|s| s.expose().to_string())
        .context("expected initial admin password on fresh test state")?;
    Ok((cfg, web::Data::new(state), pw))
}

#[test]
fn pure_rust_sqlite_and_phpadmin_api_roundtrip() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("sqlite-pma")?;
        let (_cfg, state, password) = setup_test_state(&ws)?;
        let app = init_service(App::new().configure(move |cfg| configure_app(cfg, state))).await;

        // 1. Login via /phpadmin/api/auth/login
        let login_req = TestRequest::post()
            .uri("/phpadmin/api/auth/login")
            .set_json(json!({
                "username": "admin_ferrous",
                "password": password,
                "rememberMe": false
            }))
            .to_request();
        let login_resp = try_call_service(&app, login_req)
            .await
            .map_err(|e| anyhow::anyhow!("login failed: {e}"))?;
        ensure_eq(&login_resp.status(), &StatusCode::OK, "pma login status")?;
        let login_body: Value = serde_json::from_slice(&read_body_bytes(login_resp.into_body()).await?)?;
        let token = login_body["token"]
            .as_str()
            .context("missing session token")?
            .to_string();
        let auth_hdr = format!("Bearer {token}");

        // 2. Embedded Pure-Rust FerrousAdmin UI renders on /phpadmin/
        let ui_req = TestRequest::get().uri("/phpadmin/").to_request();
        let ui_resp = try_call_service(&app, ui_req)
            .await
            .map_err(|e| anyhow::anyhow!("phpadmin ui failed: {e}"))?;
        ensure_eq(&ui_resp.status(), &StatusCode::OK, "phpadmin html status")?;
        let ui_html = String::from_utf8_lossy(&read_body_bytes(ui_resp.into_body()).await?).to_string();
        ensure(
            ui_html.contains("FerrousAdmin"),
            "expected FerrousAdmin HTML UI on /phpadmin/",
        )?;

        // 3. Create Pure-Rust SQLite 3 database via /phpadmin/api/databases
        let create_req = TestRequest::post()
            .uri("/phpadmin/api/databases")
            .insert_header((header::AUTHORIZATION, auth_hdr.clone()))
            .set_json(json!({
                "name": "app_store_db",
                "engine": "SQLite 3 (WAL · Pure Rust)",
                "username": "app_usr",
                "collation": "UTF-8",
                "linkedAppId": ""
            }))
            .to_request();
        let create_resp = try_call_service(&app, create_req)
            .await
            .map_err(|e| anyhow::anyhow!("create sqlite db failed: {e}"))?;
        ensure_eq(
            &create_resp.status(),
            &StatusCode::OK,
            "create sqlite status",
        )?;
        let dbs: Value = serde_json::from_slice(&read_body_bytes(create_resp.into_body()).await?)?;
        let db_id = dbs
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|d| d["id"].as_str())
            .context("expected created sqlite database id")?
            .to_string();

        // 4. Execute SQL DDL + INSERT via /phpadmin/api/databases/{id}/query
        let ddl_req = TestRequest::post()
            .uri(&format!("/phpadmin/api/databases/{db_id}/query"))
            .insert_header((header::AUTHORIZATION, auth_hdr.clone()))
            .set_json(json!({
                "query": "CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT NOT NULL); INSERT INTO users (email) VALUES ('alice@ferrous.local');"
            }))
            .to_request();
        let ddl_resp = try_call_service(&app, ddl_req)
            .await
            .map_err(|e| anyhow::anyhow!("ddl query failed: {e}"))?;
        ensure_eq(&ddl_resp.status(), &StatusCode::OK, "ddl status")?;
        let ddl_json: Value = serde_json::from_slice(&read_body_bytes(ddl_resp.into_body()).await?)?;
        ensure_eq(&ddl_json["success"], &Value::Bool(true), "ddl success")?;

        // 5. Browse paginated rows via /phpadmin/api/databases/{id}/tables/users/rows
        let browse_req = TestRequest::get()
            .uri(&format!("/phpadmin/api/databases/{db_id}/tables/users/rows?limit=50&offset=0"))
            .insert_header((header::AUTHORIZATION, auth_hdr.clone()))
            .to_request();
        let browse_resp = try_call_service(&app, browse_req)
            .await
            .map_err(|e| anyhow::anyhow!("browse rows failed: {e}"))?;
        ensure_eq(&browse_resp.status(), &StatusCode::OK, "browse rows status")?;
        let browse_json: Value = serde_json::from_slice(&read_body_bytes(browse_resp.into_body()).await?)?;
        ensure_eq(
            &browse_json["rows"][0]["email"],
            &Value::String("alice@ferrous.local".to_string()),
            "browsed email row",
        )?;

        // 6. Export raw SQL attachment via ?download=1
        let export_req = TestRequest::get()
            .uri(&format!("/phpadmin/api/databases/{db_id}/export?download=1"))
            .insert_header((header::AUTHORIZATION, auth_hdr.clone()))
            .to_request();
        let export_resp = try_call_service(&app, export_req)
            .await
            .map_err(|e| anyhow::anyhow!("export download failed: {e}"))?;
        ensure_eq(&export_resp.status(), &StatusCode::OK, "export status")?;
        let disp = export_resp
            .headers()
            .get(header::CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        ensure(
            disp.contains("attachment; filename="),
            "expected Content-Disposition attachment header",
        )?;
        let dump_sql = String::from_utf8_lossy(&read_body_bytes(export_resp.into_body()).await?).to_string();
        ensure(
            dump_sql.contains("alice@ferrous.local"),
            "expected dumped SQL to contain inserted row",
        )?;

        // 7. Import raw UTF-8 SQL directly (non-JSON body) via /phpadmin/api/databases/{id}/import
        let import_req = TestRequest::post()
            .uri(&format!("/phpadmin/api/databases/{db_id}/import"))
            .insert_header((header::AUTHORIZATION, auth_hdr))
            .insert_header((header::CONTENT_TYPE, "application/sql"))
            .set_payload("INSERT INTO users (email) VALUES ('bob@ferrous.local');")
            .to_request();
        let import_resp = try_call_service(&app, import_req)
            .await
            .map_err(|e| anyhow::anyhow!("raw sql import failed: {e}"))?;
        ensure_eq(&import_resp.status(), &StatusCode::OK, "import status")?;
        let import_json: Value = serde_json::from_slice(&read_body_bytes(import_resp.into_body()).await?)?;
        ensure_eq(&import_json["success"], &Value::Bool(true), "import success")?;

        Ok(())
    })
}

#[test]
fn security_headers_csrf_guard_audit_logs_and_port_protection() -> Result<()> {
    // Panel port 80 and 443 must ALWAYS be rejected (reserved for websites)
    ensure(
        validate_panel_port(80).is_err(),
        "port 80 must be rejected as panel port",
    )?;
    ensure(
        validate_panel_port(443).is_err(),
        "port 443 must be rejected as panel port",
    )?;
    ensure(
        validate_panel_port(8443).is_ok(),
        "port 8443 must be accepted as default panel port",
    )?;

    run_async(async {
        let ws = TempWorkspace::new("sec-audit")?;
        let (_cfg, state, password) = setup_test_state(&ws)?;
        let app = init_service(
            App::new()
                .wrap(from_fn(security_headers_middleware))
                .configure(move |cfg| configure_app(cfg, state)),
        )
        .await;

        // 1. Public /api/health returns security headers
        let health_req = TestRequest::get().uri("/api/health").to_request();
        let health_resp = try_call_service(&app, health_req)
            .await
            .map_err(|e| anyhow::anyhow!("health call failed: {e}"))?;
        ensure_eq(&health_resp.status(), &StatusCode::OK, "health status")?;
        let nosniff = health_resp
            .headers()
            .get("x-content-type-options")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        ensure_eq(&nosniff, &"nosniff", "x-content-type-options header")?;

        // 2. Login to get session token
        let login_req = TestRequest::post()
            .uri("/api/auth/login")
            .set_json(json!({
                "username": "admin_ferrous",
                "password": password,
                "rememberMe": false
            }))
            .to_request();
        let login_resp = try_call_service(&app, login_req)
            .await
            .map_err(|e| anyhow::anyhow!("login failed: {e}"))?;
        ensure_eq(&login_resp.status(), &StatusCode::OK, "login status")?;
        let login_body: Value = serde_json::from_slice(&read_body_bytes(login_resp.into_body()).await?)?;
        let token = login_body["token"]
            .as_str()
            .context("missing token")?
            .to_string();

        // 3. Cookie-authenticated cross-origin POST must be blocked with 403 Forbidden
        let csrf_req = TestRequest::post()
            .uri("/api/jobs/cpanel/backups")
            .insert_header((header::HOST, "panel.example.com:8443"))
            .insert_header((header::ORIGIN, "https://attacker.invalid"))
            .insert_header((header::COOKIE, format!("{SESSION_COOKIE_NAME}={token}")))
            .set_json(json!({ "targetApp": "demo.example.com", "databaseIncluded": "" }))
            .to_request();
        let csrf_resp = try_call_service(&app, csrf_req)
            .await
            .map_err(|e| anyhow::anyhow!("csrf probe failed: {e}"))?;
        ensure_eq(
            &csrf_resp.status(),
            &StatusCode::FORBIDDEN,
            "cross-origin cookie POST must return 403",
        )?;

        // 4. GET /api/security/audit-logs returns recorded AUTH_LOGIN_SUCCESS entry
        let audit_req = TestRequest::get()
            .uri("/api/security/audit-logs?limit=25")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .to_request();
        let audit_resp = try_call_service(&app, audit_req)
            .await
            .map_err(|e| anyhow::anyhow!("audit logs request failed: {e}"))?;
        ensure_eq(&audit_resp.status(), &StatusCode::OK, "audit logs status")?;
        let logs: Value = serde_json::from_slice(&read_body_bytes(audit_resp.into_body()).await?)?;
        let has_login_log = logs
            .as_array()
            .map(|arr| arr.iter().any(|row| row["action"] == "AUTH_LOGIN_SUCCESS"))
            .unwrap_or(false);
        ensure(has_login_log, "expected AUTH_LOGIN_SUCCESS in audit logs")?;

        Ok(())
    })
}

#[test]
fn background_jobs_and_raw_file_upload_flow() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("jobs-upload")?;
        let (cfg, state, password) = setup_test_state(&ws)?;
        let app = init_service(App::new().configure(move |cfg| configure_app(cfg, state))).await;

        let login_req = TestRequest::post()
            .uri("/api/auth/login")
            .set_json(json!({
                "username": "admin_ferrous",
                "password": password,
                "rememberMe": false
            }))
            .to_request();
        let login_resp = try_call_service(&app, login_req)
            .await
            .map_err(|e| anyhow::anyhow!("login failed: {e}"))?;
        let login_body: Value = serde_json::from_slice(&read_body_bytes(login_resp.into_body()).await?)?;
        let token = login_body["token"]
            .as_str()
            .context("missing token")?
            .to_string();
        let auth_hdr = format!("Bearer {token}");

        // 1. Unknown app ID on async job returns 404 immediately
        let missing_job_req = TestRequest::post()
            .uri("/api/jobs/apps/does-not-exist/rebuild")
            .insert_header((header::AUTHORIZATION, auth_hdr.clone()))
            .to_request();
        let missing_job_resp = try_call_service(&app, missing_job_req)
            .await
            .map_err(|e| anyhow::anyhow!("missing job request failed: {e}"))?;
        ensure_eq(
            &missing_job_resp.status(),
            &StatusCode::NOT_FOUND,
            "unknown app rebuild job must return 404",
        )?;

        // 2. Deploy a test website so we have a real app_id and wwwroot directory
        let deploy_req = TestRequest::post()
            .uri("/api/apps")
            .insert_header((header::AUTHORIZATION, auth_hdr.clone()))
            .set_json(json!({
                "name": "Jobs Demo",
                "framework": "Static HTML / SPA",
                "domain": "jobs-demo.example.com",
                "aliases": [],
                "internalPort": 8080,
                "workingDir": "",
                "runSubDir": "",
                "execCommand": "",
                "workers": 1,
                "memoryLimitMb": 128,
                "sslEnabled": false,
                "http3Quic": false,
                "websocketEnabled": false,
                "rateLimitRps": 0,
                "createDatabase": false,
                "dbEngine": "",
                "dbName": "",
                "dbUser": "",
                "dbPassword": "",
                "createFtp": false,
                "ftpUser": "",
                "gitRepoUrl": "",
                "gitBranch": "main"
            }))
            .to_request();
        let deploy_resp = try_call_service(&app, deploy_req)
            .await
            .map_err(|e| anyhow::anyhow!("deploy app failed: {e}"))?;
        ensure_eq(&deploy_resp.status(), &StatusCode::OK, "deploy app status")?;
        let app_val: Value = serde_json::from_slice(&read_body_bytes(deploy_resp.into_body()).await?)?;
        let app_id = app_val["id"]
            .as_str()
            .context("expected deployed app id")?
            .to_string();

        // 3. Queue background rebuild job -> 202 Accepted + poll to completion
        let start_job_req = TestRequest::post()
            .uri(&format!("/api/jobs/apps/{app_id}/rebuild"))
            .insert_header((header::AUTHORIZATION, auth_hdr.clone()))
            .to_request();
        let start_job_resp = try_call_service(&app, start_job_req)
            .await
            .map_err(|e| anyhow::anyhow!("start rebuild job failed: {e}"))?;
        ensure_eq(
            &start_job_resp.status(),
            &StatusCode::ACCEPTED,
            "rebuild job must return 202 Accepted",
        )?;
        let job_val: Value = serde_json::from_slice(&read_body_bytes(start_job_resp.into_body()).await?)?;
        let job_id = job_val["id"]
            .as_str()
            .context("missing job id")?
            .to_string();
        ensure(job_id.starts_with("job-"), "job id must have job- prefix")?;

        let mut offset = 0u64;
        let mut job_done = false;
        for _ in 0..8 {
            let poll_req = TestRequest::get()
                .uri(&format!("/api/jobs/{job_id}/poll?offset={offset}&waitMs=3000"))
                .insert_header((header::AUTHORIZATION, auth_hdr.clone()))
                .to_request();
            let poll_resp = try_call_service(&app, poll_req)
                .await
                .map_err(|e| anyhow::anyhow!("poll job failed: {e}"))?;
            ensure_eq(&poll_resp.status(), &StatusCode::OK, "poll status")?;
            let poll_json: Value =
                serde_json::from_slice(&read_body_bytes(poll_resp.into_body()).await?)?;
            offset = poll_json["nextOffset"].as_u64().unwrap_or(offset);
            if poll_json["done"] == Value::Bool(true) {
                job_done = true;
                break;
            }
        }
        ensure(job_done, "polled job should reach terminal done=true state")?;

        // 4. Raw binary file upload inside SafePath + traversal rejection
        let upload_req = TestRequest::post()
            .uri(&format!(
                "/api/files/{app_id}/upload-raw?parentDir=/&fileName=hello.txt"
            ))
            .insert_header((header::AUTHORIZATION, auth_hdr.clone()))
            .set_payload("hello-ferrous-raw-upload")
            .to_request();
        let upload_resp = try_call_service(&app, upload_req)
            .await
            .map_err(|e| anyhow::anyhow!("raw upload failed: {e}"))?;
        ensure_eq(&upload_resp.status(), &StatusCode::OK, "raw upload status")?;
        let uploaded_path = cfg.wwwroot.join("jobs-demo.example.com").join("hello.txt");
        let saved = std::fs::read_to_string(&uploaded_path)?;
        ensure_eq(&saved, &"hello-ferrous-raw-upload".to_string(), "uploaded bytes")?;

        let bad_upload_req = TestRequest::post()
            .uri(&format!(
                "/api/files/{app_id}/upload-raw?parentDir=/&fileName=..%2Fescape.txt"
            ))
            .insert_header((header::AUTHORIZATION, auth_hdr))
            .set_payload("blocked")
            .to_request();
        let bad_upload_resp = try_call_service(&app, bad_upload_req)
            .await
            .map_err(|e| anyhow::anyhow!("traversal upload probe failed: {e}"))?;
        ensure(
            bad_upload_resp.status().is_client_error(),
            "path traversal filename must be rejected with 4xx",
        )?;

        Ok(())
    })
}

#[test]
fn pure_rust_native_panel_ui_serves_without_nodejs_dist() -> Result<()> {
    run_async(async {
        let ws = TempWorkspace::new("native-ui")?;
        let (_cfg, state, _password) = setup_test_state(&ws)?;
        let app = init_service(App::new().configure(move |cfg| configure_app(cfg, state))).await;

        for path in ["/", "/panel"] {
            let req = TestRequest::get().uri(path).to_request();
            let resp = try_call_service(&app, req)
                .await
                .map_err(|e| anyhow::anyhow!("GET {path} failed: {e}"))?;
            ensure_eq(&resp.status(), &StatusCode::OK, path)?;
            let ui_hdr = resp
                .headers()
                .get("x-ferrous-ui")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            ensure_eq(&ui_hdr, &"pure-rust-native", "x-ferrous-ui header")?;
            let html = String::from_utf8_lossy(&read_body_bytes(resp.into_body()).await?).to_string();
            ensure(
                html.contains("FerrousPanel") && html.contains("/ui/app.js"),
                "expected Pure-Rust HTML shell referencing /ui/app.js",
            )?;
            ensure(
                html.contains("pure-rust-native"),
                "expected embedded boot metadata in HTML shell",
            )?;
        }

        for (asset_path, expected_ct) in [
            ("/ui/panel.css", "text/css"),
            ("/ui/favicon.svg", "image/svg+xml"),
            ("/ui/app.js", "application/javascript"),
            ("/ui/views-core.js", "application/javascript"),
            ("/ui/views-infra.js", "application/javascript"),
            ("/ui/views-security.js", "application/javascript"),
        ] {
            let req = TestRequest::get().uri(asset_path).to_request();
            let resp = try_call_service(&app, req)
                .await
                .map_err(|e| anyhow::anyhow!("GET {asset_path} failed: {e}"))?;
            ensure_eq(&resp.status(), &StatusCode::OK, asset_path)?;
            let ct = resp
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            ensure(
                ct.starts_with(expected_ct),
                &format!("unexpected content-type '{ct}' for {asset_path}"),
            )?;
            let body = read_body_bytes(resp.into_body()).await?;
            ensure(!body.is_empty(), &format!("empty asset body for {asset_path}"))?;
        }

        // Unknown asset under /ui/* must return 404 (no HTML fallback)
        let missing = TestRequest::get().uri("/ui/does-not-exist.js").to_request();
        let missing_resp = try_call_service(&app, missing)
            .await
            .map_err(|e| anyhow::anyhow!("missing ui asset probe failed: {e}"))?;
        ensure_eq(
            &missing_resp.status(),
            &StatusCode::NOT_FOUND,
            "missing /ui/* asset must return 404",
        )?;

        Ok(())
    })
}

