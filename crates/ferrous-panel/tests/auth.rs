#![forbid(unsafe_code)]

use actix_web::body::to_bytes;
use actix_web::http::{header, StatusCode};
use actix_web::test::{init_service, try_call_service, TestRequest};
use actix_web::{web, App};
use anyhow::{bail, Context, Result};
use ferrous_panel::auth::password::{load_or_init_credentials, RedactedSecret};
use ferrous_panel::config::Config;
use ferrous_panel::configure_app;
use ferrous_panel::state::AppState;
use ferrous_panel::system::paths::SafePath;
use rand::rngs::OsRng;
use rand::RngCore;
use std::future::Future;
use std::path::{Path, PathBuf};

fn ensure(cond: bool, msg: &str) -> Result<()> {
    if cond {
        Ok(())
    } else {
        bail!("{msg}")
    }
}

fn run_async<F: Future<Output = Result<()>>>(fut: F) -> Result<()> {
    let rt = actix_web::rt::Runtime::new().context("failed to create runtime")?;
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
        let dir = std::env::temp_dir().join(format!("ferrous-panel-test-{prefix}-{hex}"));
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

fn init_test_env(
    prefix: &str,
) -> Result<(TempWorkspace, Config, AppState, Option<RedactedSecret>)> {
    let ws = TempWorkspace::new(prefix)?;
    let data_dir = ws.path().join("data");
    let wwwroot = ws.path().join("wwwroot");
    std::fs::create_dir_all(&wwwroot)?;
    let cfg = Config::for_test(data_dir, wwwroot);
    let (state, initial_pass) = AppState::init(cfg.clone())?;
    Ok((ws, cfg, state, initial_pass))
}

#[test]
fn api_without_token_returns_401_while_health_stays_public() -> Result<()> {
    run_async(async {
        let (_ws, _cfg, state, _pw) = init_test_env("no-token")?;
        let shared = web::Data::new(state);
        let app = init_service(App::new().configure(|c| configure_app(c, shared.clone()))).await;

        // Public health check must be reachable without token
        let health_req = TestRequest::get().uri("/api/health").to_request();
        let health_resp = try_call_service(&app, health_req)
            .await
            .map_err(|e| anyhow::anyhow!("health call error: {e}"))?;
        ensure(
            health_resp.status() == StatusCode::OK,
            "GET /api/health without token must return 200",
        )?;

        // Protected endpoints without token MUST return 401
        for (method, uri) in [
            ("GET", "/api/telemetry"),
            ("GET", "/api/auth/session"),
            ("POST", "/api/auth/logout"),
            ("GET", "/api/apps"),
            ("GET", "/api/health/extra"),
        ] {
            let builder = if method == "POST" {
                TestRequest::post()
            } else {
                TestRequest::get()
            };
            let req = builder.uri(uri).to_request();
            let resp = try_call_service(&app, req)
                .await
                .map_err(|e| anyhow::anyhow!("call error on {uri}: {e}"))?;
            ensure(
                resp.status() == StatusCode::UNAUTHORIZED,
                &format!("{method} {uri} without token must return 401"),
            )?;
        }

        // Wrong HTTP method on /api/auth/login (GET instead of POST) must NOT bypass middleware
        let wrong_method_req = TestRequest::get().uri("/api/auth/login").to_request();
        let wrong_method_resp = try_call_service(&app, wrong_method_req)
            .await
            .map_err(|e| anyhow::anyhow!("login GET error: {e}"))?;
        ensure(
            wrong_method_resp.status() == StatusCode::UNAUTHORIZED,
            "GET /api/auth/login must require auth (401)",
        )?;

        Ok(())
    })
}

#[test]
fn api_with_fake_token_returns_401() -> Result<()> {
    run_async(async {
        let (_ws, _cfg, state, _pw) = init_test_env("fake-token")?;
        let shared = web::Data::new(state);
        let app = init_service(App::new().configure(|c| configure_app(c, shared.clone()))).await;

        let fake_bearer = TestRequest::get()
            .uri("/api/telemetry")
            .insert_header((header::AUTHORIZATION, "Bearer 00000000000000000000000000000000"))
            .to_request();
        let resp_bearer = try_call_service(&app, fake_bearer)
            .await
            .map_err(|e| anyhow::anyhow!("bearer fake error: {e}"))?;
        ensure(
            resp_bearer.status() == StatusCode::UNAUTHORIZED,
            "fake Bearer token must return 401",
        )?;

        let fake_header = TestRequest::get()
            .uri("/api/auth/session")
            .insert_header(("X-FP-Session", "fp_sess_forged_token_value"))
            .to_request();
        let resp_hdr = try_call_service(&app, fake_header)
            .await
            .map_err(|e| anyhow::anyhow!("header fake error: {e}"))?;
        ensure(
            resp_hdr.status() == StatusCode::UNAUTHORIZED,
            "fake X-FP-Session token must return 401",
        )?;

        let fake_cookie = TestRequest::get()
            .uri("/api/telemetry")
            .insert_header((header::COOKIE, "fp_admin_token=forged_cookie_value"))
            .to_request();
        let resp_cookie = try_call_service(&app, fake_cookie)
            .await
            .map_err(|e| anyhow::anyhow!("cookie fake error: {e}"))?;
        ensure(
            resp_cookie.status() == StatusCode::UNAUTHORIZED,
            "fake cookie token must return 401",
        )?;

        Ok(())
    })
}

#[test]
fn login_session_telemetry_logout_rate_limit_and_audit() -> Result<()> {
    run_async(async {
        let (_ws, cfg, state, initial_pw) = init_test_env("full-auth")?;
        let password = initial_pw
            .context("first initialization must return generated one-time password")?;

        // Verify credentials file is persisted with 0600 mode on Unix and contains hash only
        let cred_raw = std::fs::read_to_string(cfg.credentials_path())?;
        ensure(
            !cred_raw.contains(password.expose()),
            "credentials file must never store plaintext password",
        )?;
        ensure(
            cred_raw.contains("$argon2id$"),
            "credentials file must store Argon2id PHC hash",
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(cfg.credentials_path())?
                .permissions()
                .mode()
                & 0o777;
            ensure(mode == 0o600, "credentials file permission must be 0600")?;
        }

        // Subsequent load must NOT regenerate initial password
        let (_reloaded, second_pw) =
            load_or_init_credentials(&cfg.credentials_path(), &cfg.admin_username)?;
        ensure(
            second_pw.is_none(),
            "subsequent run must not generate a new password",
        )?;

        let shared = web::Data::new(state);
        let app = init_service(App::new().configure(|c| configure_app(c, shared.clone()))).await;

        // Verify rate limiter: 5 bad logins return 401, 6th returns 429
        for attempt in 1..=5 {
            let bad_req = TestRequest::post()
                .uri("/api/auth/login")
                .set_json(serde_json::json!({
                    "username": cfg.admin_username,
                    "password": "WrongPassword!999",
                    "rememberMe": false
                }))
                .to_request();
            let bad_resp = try_call_service(&app, bad_req)
                .await
                .map_err(|e| anyhow::anyhow!("bad login error: {e}"))?;
            ensure(
                bad_resp.status() == StatusCode::UNAUTHORIZED,
                &format!("failed login #{attempt} must return 401"),
            )?;
        }

        let limited_req = TestRequest::post()
            .uri("/api/auth/login")
            .set_json(serde_json::json!({
                "username": cfg.admin_username,
                "password": password.expose(),
                "rememberMe": false
            }))
            .to_request();
        let limited_resp = try_call_service(&app, limited_req)
            .await
            .map_err(|e| anyhow::anyhow!("rate limit check error: {e}"))?;
        ensure(
            limited_resp.status() == StatusCode::TOO_MANY_REQUESTS,
            "6th login attempt within 15m must return 429",
        )?;

        // Clear rate limiter and perform valid login (24h session)
        shared.rate_limiter.clear("127.0.0.1")?;
        let ok_req = TestRequest::post()
            .uri("/api/auth/login")
            .set_json(serde_json::json!({
                "username": cfg.admin_username,
                "password": password.expose(),
                "rememberMe": false
            }))
            .to_request();
        let ok_resp = try_call_service(&app, ok_req)
            .await
            .map_err(|e| anyhow::anyhow!("login ok error: {e}"))?;
        ensure(ok_resp.status() == StatusCode::OK, "valid login must return 200")?;

        let set_cookie_hdr = ok_resp
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|v| v.to_str().ok())
            .context("login must set HttpOnly SameSite=Strict cookie")?
            .to_string();
        ensure(
            set_cookie_hdr.contains("HttpOnly") && set_cookie_hdr.contains("SameSite=Strict"),
            "cookie must enforce HttpOnly and SameSite=Strict",
        )?;
        ensure(
            set_cookie_hdr.contains("Max-Age=86400"),
            "non-rememberMe session cookie must have 24h (86400s) Max-Age",
        )?;

        let body_bytes = to_bytes(ok_resp.into_body())
            .await
            .map_err(|e| anyhow::anyhow!("read body error: {e}"))?;
        let login_json: serde_json::Value = serde_json::from_slice(&body_bytes)?;
        let token = login_json
            .get("token")
            .and_then(|v| v.as_str())
            .context("login JSON must include string token")?
            .to_string();
        ensure(
            token.len() == 64,
            "32-byte OsRng token must serialize as 64 hex chars",
        )?;

        // Authenticated /api/telemetry must succeed with real sysinfo values
        let tel_req = TestRequest::get()
            .uri("/api/telemetry")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .to_request();
        let tel_resp = try_call_service(&app, tel_req)
            .await
            .map_err(|e| anyhow::anyhow!("telemetry call error: {e}"))?;
        ensure(
            tel_resp.status() == StatusCode::OK,
            "authenticated GET /api/telemetry must return 200",
        )?;
        let tel_bytes = to_bytes(tel_resp.into_body())
            .await
            .map_err(|e| anyhow::anyhow!("telemetry body error: {e}"))?;
        let tel_json: serde_json::Value = serde_json::from_slice(&tel_bytes)?;
        let cpu_cores = tel_json
            .get("cpuCores")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let mem_mb = tel_json
            .get("memoryTotalMb")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        ensure(
            cpu_cores > 0 && mem_mb > 0,
            "telemetry must report positive real sysinfo CPU cores and memory",
        )?;

        // Authenticated unimplemented route must return 501 (not fake success)
        let unimpl_req = TestRequest::get()
            .uri("/api/unknown-unimplemented-endpoint")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .to_request();
        let unimpl_resp = try_call_service(&app, unimpl_req)
            .await
            .map_err(|e| anyhow::anyhow!("unimplemented route error: {e}"))?;
        ensure(
            unimpl_resp.status() == StatusCode::NOT_IMPLEMENTED,
            "unimplemented authenticated route must return 501",
        )?;

        // Logout revokes token in SQLite
        let logout_req = TestRequest::post()
            .uri("/api/auth/logout")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .to_request();
        let logout_resp = try_call_service(&app, logout_req)
            .await
            .map_err(|e| anyhow::anyhow!("logout error: {e}"))?;
        ensure(
            logout_resp.status() == StatusCode::OK,
            "authenticated logout must return 200",
        )?;

        let reused_req = TestRequest::get()
            .uri("/api/auth/session")
            .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
            .to_request();
        let reused_resp = try_call_service(&app, reused_req)
            .await
            .map_err(|e| anyhow::anyhow!("post-logout session check error: {e}"))?;
        ensure(
            reused_resp.status() == StatusCode::UNAUTHORIZED,
            "revoked token after logout must return 401",
        )?;

        // Audit logs must have recorded failures, success, and logout in SQLite
        let audit_count = shared.auth_store.audit_log_count()?;
        ensure(audit_count >= 7, "SQLite audit_logs must record auth events")?;

        Ok(())
    })
}

#[test]
fn safe_path_rejects_traversal_and_symlink_escape() -> Result<()> {
    ensure(
        SafePath::in_wwwroot("../../etc/passwd").is_err(),
        "\"../../etc/passwd\" inside wwwroot must be rejected",
    )?;
    ensure(
        SafePath::in_wwwroot("/etc/passwd").is_err(),
        "\"/etc/passwd\" outside /www/wwwroot must be rejected",
    )?;
    ensure(
        SafePath::in_wwwroot("/www/wwwroot-other/evil").is_err(),
        "sibling prefix path must be rejected",
    )?;
    ensure(
        SafePath::in_wwwroot("site\0evil").is_err(),
        "NUL byte path must be rejected",
    )?;

    // Real disk symlink escape test
    let ws = TempWorkspace::new("safepath-symlink")?;
    let root = ws.path().join("wwwroot");
    let outside = ws.path().join("outside");
    std::fs::create_dir_all(&root)?;
    std::fs::create_dir_all(&outside)?;
    std::fs::write(outside.join("secret.txt"), "secret")?;

    let valid_site = root.join("example.com/public");
    std::fs::create_dir_all(&valid_site)?;

    ensure(
        SafePath::under_root(&root, "../../etc/passwd").is_err(),
        "\"../../etc/passwd\" under custom root must be rejected",
    )?;
    ensure(
        SafePath::under_root(&root, "example.com/public/index.html").is_ok(),
        "valid non-existent subpath inside root must succeed",
    )?;

    #[cfg(unix)]
    {
        let link_path = root.join("escape_link");
        std::os::unix::fs::symlink(&outside, &link_path)?;
        ensure(
            SafePath::under_root(&root, "escape_link/secret.txt").is_err(),
            "symlink resolving outside root must be rejected",
        )?;
        ensure(
            SafePath::under_root(&root, "escape_link/future_file.txt").is_err(),
            "non-existent child beneath escaping symlink must be rejected",
        )?;
    }

    Ok(())
}
