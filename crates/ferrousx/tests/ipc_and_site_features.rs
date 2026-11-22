#![forbid(unsafe_code)]

use ferrousx::handlers::{compute_apr1_hash, verify_htpasswd_hash};
use ferrousx::ipc::{IpcRequest, IpcResponse};
use ferrousx::server::FerrousxServer;
use std::fs;
use std::future::Future;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UnixStream};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

static TEST_SEQ: AtomicU64 = AtomicU64::new(1);

fn ensure(cond: bool, msg: &str) -> TestResult {
    if cond {
        Ok(())
    } else {
        Err(msg.into())
    }
}

fn ensure_eq<T: PartialEq + std::fmt::Debug>(actual: &T, expected: &T, label: &str) -> TestResult {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{label} mismatch:\nexpected: {expected:?}\nactual:   {actual:?}").into())
    }
}

fn run_async<F: Future<Output = TestResult>>(fut: F) -> TestResult {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()?;
    rt.block_on(fut)
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> TestResult<Self> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let seq = TEST_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "ferrousx-e2e-{prefix}-{}-{nanos}-{seq}",
            std::process::id()
        ));
        fs::create_dir_all(&dir)?;
        Ok(Self { path: dir })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

async fn send_http(
    addr: SocketAddr,
    host: &str,
    path: &str,
    extra_headers: &[(&str, &str)],
) -> TestResult<(u16, String, String)> {
    let mut stream = TcpStream::connect(addr).await?;
    let mut req = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");
    for (k, v) in extra_headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    stream.write_all(req.as_bytes()).await?;
    stream.flush().await?;

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await?;
    let text = String::from_utf8_lossy(&raw);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or("malformed HTTP response")?;
    let status_line = head.lines().next().unwrap_or("");
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or("missing HTTP status code")?;
    Ok((status_code, head.to_string(), body.to_string()))
}

async fn send_ipc(socket_path: &Path, req: &IpcRequest) -> TestResult<IpcResponse> {
    let mut stream = UnixStream::connect(socket_path).await?;
    let mut payload = serde_json::to_vec(req)?;
    payload.push(b'\n');
    stream.write_all(&payload).await?;
    stream.flush().await?;

    let mut resp_bytes = Vec::new();
    stream.read_to_end(&mut resp_bytes).await?;
    let resp: IpcResponse = serde_json::from_slice(&resp_bytes)?;
    Ok(resp)
}

#[test]
fn apr1_htpasswd_matches_openssl_reference_vector_and_rejects_wrong_password() -> TestResult {
    // Verified against `openssl passwd -apr1 -salt 12345678 password`
    let expected = "$apr1$12345678$9pHAGSBYtlmFtid2xxNog0";
    let actual = compute_apr1_hash("password", "12345678");
    ensure_eq(&actual, &expected.to_string(), "APR1 reference vector")?;
    ensure(
        verify_htpasswd_hash("password", expected),
        "verify_htpasswd_hash must accept matching APR1 password",
    )?;
    ensure(
        !verify_htpasswd_hash("wrongpass", expected),
        "verify_htpasswd_hash must reject wrong password",
    )?;
    Ok(())
}

#[test]
fn site_rate_limit_redirects_hotlink_and_basic_auth_work_over_real_http() -> TestResult {
    run_async(async {
        let tmp = TempDir::new("site-features")?;
        let sites_dir = tmp.path().join("sites");
        let acme_dir = tmp.path().join("acme");
        let site_root = tmp.path().join("wwwroot/site.local");
        let secret_dir = site_root.join("secret");
        fs::create_dir_all(&sites_dir)?;
        fs::create_dir_all(&acme_dir)?;
        fs::create_dir_all(&secret_dir)?;

        fs::write(site_root.join("index.html"), "hello-ferrousx")?;
        fs::write(site_root.join("logo.png"), "PNGDATA")?;
        fs::write(secret_dir.join("index.html"), "top-secret-area")?;

        let htpasswd_path = tmp.path().join("site.htpasswd");
        let apr1 = compute_apr1_hash("password", "12345678");
        fs::write(&htpasswd_path, format!("admin:{apr1}\n"))?;

        let master_path = tmp.path().join("ferrousx.toml");
        fs::write(
            &master_path,
            format!(
                "worker_threads = 2\nbind_http = \"127.0.0.1:0\"\nbind_https = \"127.0.0.1:0\"\nsites_dir = \"{}\"\nacme_webroot = \"{}\"\nserver_tokens = false\n",
                sites_dir.display(),
                acme_dir.display()
            ),
        )?;

        // Site 1: redirects + hotlink + directory_auth (rate_limit_rps = 0 so feature checks never throttle)
        fs::write(
            sites_dir.join("site.local.toml"),
            format!(
                r#"domain = "site.local"
server_names = ["site.local"]
root = "{}"
rate_limit_rps = 0

[handler]
kind = "static_spa"
index_file = "index.html"
spa_fallback = false

[[redirects]]
source_path = "/old"
target_url = "https://example.com/new"
status_code = 301
keep_query = true
enabled = true

[hotlink]
enabled = true
protected_extensions = ["png", "jpg"]
allowed_domains = ["partner.example"]
block_empty_referer = true

[[directory_auth]]
path = "/secret"
label = "Vault"
htpasswd_file = "{}"
enabled = true
"#,
                site_root.display(),
                htpasswd_path.display()
            ),
        )?;

        // Site 2: tight per-IP token-bucket rate limit (rate_limit_rps = 2)
        fs::write(
            sites_dir.join("limited.local.toml"),
            format!(
                r#"domain = "limited.local"
server_names = ["limited.local"]
root = "{}"
rate_limit_rps = 2

[handler]
kind = "static_spa"
index_file = "index.html"
spa_fallback = false
"#,
                site_root.display()
            ),
        )?;

        let server = FerrousxServer::start(&master_path).await?;
        let http_addr = server.http_addr;

        // 1. Redirect rule (/old?x=1 -> 301 https://example.com/new?x=1)
        let (code, head, _) = send_http(http_addr, "site.local", "/old?x=1", &[]).await?;
        ensure_eq(&code, &301u16, "redirect status")?;
        ensure(
            head.to_ascii_lowercase()
                .contains("location: https://example.com/new?x=1"),
            &format!("redirect Location header mismatch: {head}"),
        )?;

        // 2. Directory Basic Auth (/secret/index.html)
        let (unauth_code, unauth_head, _) =
            send_http(http_addr, "site.local", "/secret/index.html", &[]).await?;
        ensure_eq(&unauth_code, &401u16, "directory auth unauthenticated status")?;
        ensure(
            unauth_head.to_ascii_lowercase().contains("www-authenticate: basic"),
            "missing WWW-Authenticate Basic challenge",
        )?;

        // Base64("admin:password") == "YWRtaW46cGFzc3dvcmQ="
        let (auth_code, _, auth_body) = send_http(
            http_addr,
            "site.local",
            "/secret/index.html",
            &[("Authorization", "Basic YWRtaW46cGFzc3dvcmQ=")],
        )
        .await?;
        ensure_eq(&auth_code, &200u16, "directory auth valid credentials status")?;
        ensure_eq(&auth_body, &"top-secret-area".to_string(), "directory auth body")?;

        // 3. Hotlink protection (/logo.png)
        let (hl_blocked, _, _) = send_http(
            http_addr,
            "site.local",
            "/logo.png",
            &[("Referer", "https://thief.invalid/page.html")],
        )
        .await?;
        ensure_eq(&hl_blocked, &403u16, "hotlink blocked status")?;

        let (hl_allowed, _, hl_body) = send_http(
            http_addr,
            "site.local",
            "/logo.png",
            &[("Referer", "https://partner.example/gallery")],
        )
        .await?;
        ensure_eq(&hl_allowed, &200u16, "hotlink allowed partner status")?;
        ensure_eq(&hl_body, &"PNGDATA".to_string(), "hotlink allowed body")?;

        // 4. Token-bucket rate limiter (burst > 2 hits 429 Too Many Requests)
        let mut saw_429 = false;
        for _ in 0..6 {
            let (st, hdrs, _) = send_http(http_addr, "limited.local", "/index.html", &[]).await?;
            if st == 429 {
                ensure(
                    hdrs.to_ascii_lowercase().contains("retry-after:"),
                    "429 response must include Retry-After header",
                )?;
                saw_429 = true;
                break;
            }
        }
        ensure(saw_429, "rate_limit_rps=2 must return HTTP 429 under burst")?;

        server.shutdown().await;
        Ok(())
    })
}

#[test]
fn uds_admin_ipc_end_to_end_status_reload_vhost_add_and_del() -> TestResult {
    run_async(async {
        let tmp = TempDir::new("uds-ipc")?;
        let sites_dir = tmp.path().join("sites");
        let acme_dir = tmp.path().join("acme");
        let run_dir = tmp.path().join("run");
        let site1_root = tmp.path().join("wwwroot/alpha.local");
        let site2_root = tmp.path().join("wwwroot/beta.local");
        fs::create_dir_all(&sites_dir)?;
        fs::create_dir_all(&acme_dir)?;
        fs::create_dir_all(&run_dir)?;
        fs::create_dir_all(&site1_root)?;
        fs::create_dir_all(&site2_root)?;

        fs::write(site1_root.join("index.html"), "alpha-live")?;
        fs::write(site2_root.join("index.html"), "beta-live")?;

        let sock_path = run_dir.join("panel-admin.sock");
        let token_path = tmp.path().join("admin.token");
        let master_path = tmp.path().join("ferrousx.toml");
        fs::write(
            &master_path,
            format!(
                "worker_threads = 2\nbind_http = \"127.0.0.1:0\"\nbind_https = \"127.0.0.1:0\"\nsites_dir = \"{}\"\nacme_webroot = \"{}\"\nadmin_token_file = \"{}\"\nserver_tokens = false\n",
                sites_dir.display(),
                acme_dir.display(),
                token_path.display()
            ),
        )?;

        fs::write(
            sites_dir.join("alpha.local.toml"),
            format!(
                "domain = \"alpha.local\"\nserver_names = [\"alpha.local\"]\nroot = \"{}\"\n\n[handler]\nkind = \"static_spa\"\nindex_file = \"index.html\"\nspa_fallback = false\n",
                site1_root.display()
            ),
        )?;

        let server = FerrousxServer::start_with_ipc(&master_path, Some(sock_path.clone())).await?;
        let http_addr = server.http_addr;
        let token = fs::read_to_string(&token_path)?.trim().to_string();
        ensure(token.len() >= 32, "admin token must be initialized with >= 32 hex chars")?;

        // 1. status over UDS
        let st_resp = send_ipc(
            &sock_path,
            &IpcRequest {
                action: "status".to_string(),
                payload: serde_json::json!({}),
                auth_token: Some(token.clone()),
            },
        )
        .await?;
        ensure(st_resp.success, "IPC status must succeed")?;
        let st_data = st_resp.data.ok_or("missing status data")?;
        ensure_eq(&st_data["vhosts_count"], &serde_json::json!(1), "initial vhosts_count")?;

        // 2. Write beta.local.toml and trigger vhost_add over UDS
        let beta_toml = sites_dir.join("beta.local.toml");
        fs::write(
            &beta_toml,
            format!(
                "domain = \"beta.local\"\nserver_names = [\"beta.local\"]\nroot = \"{}\"\n\n[handler]\nkind = \"static_spa\"\nindex_file = \"index.html\"\nspa_fallback = false\n",
                site2_root.display()
            ),
        )?;
        let add_resp = send_ipc(
            &sock_path,
            &IpcRequest {
                action: "vhost_add".to_string(),
                payload: serde_json::json!({ "domain": "beta.local" }),
                auth_token: Some(token.clone()),
            },
        )
        .await?;
        ensure(add_resp.success, "IPC vhost_add must succeed")?;
        ensure_eq(
            &add_resp.data.as_ref().and_then(|d| d.get("vhosts_count")).cloned(),
            &Some(serde_json::json!(2)),
            "vhosts_count after vhost_add",
        )?;

        // Verify beta.local serves live traffic immediately after RCU reload
        let (beta_code, _, beta_body) = send_http(http_addr, "beta.local", "/index.html", &[]).await?;
        ensure_eq(&beta_code, &200u16, "beta.local HTTP 200 after vhost_add")?;
        ensure_eq(&beta_body, &"beta-live".to_string(), "beta.local body")?;

        // 3. Invalid config reload must fail gracefully and retain alpha.local + beta.local
        let broken_toml = sites_dir.join("broken.local.toml");
        fs::write(&broken_toml, "domain = \"broken.local\"\nhttp3_quic = true\n[handler]\nkind = \"static_spa\"\n")?;
        let bad_reload = send_ipc(
            &sock_path,
            &IpcRequest {
                action: "reload".to_string(),
                payload: serde_json::json!({}),
                auth_token: Some(token.clone()),
            },
        )
        .await?;
        ensure(!bad_reload.success, "IPC reload must reject invalid site config")?;
        let _ = fs::remove_file(&broken_toml);

        // Previous config still serves alpha.local
        let (alpha_still, _, alpha_body) = send_http(http_addr, "alpha.local", "/index.html", &[]).await?;
        ensure_eq(&alpha_still, &200u16, "alpha.local still live after rejected reload")?;
        ensure_eq(&alpha_body, &"alpha-live".to_string(), "alpha.local body")?;

        // 4. Remove beta.local.toml and trigger vhost_del over UDS
        fs::remove_file(&beta_toml)?;
        let del_resp = send_ipc(
            &sock_path,
            &IpcRequest {
                action: "vhost_del".to_string(),
                payload: serde_json::json!({ "domain": "beta.local" }),
                auth_token: Some(token),
            },
        )
        .await?;
        ensure(del_resp.success, "IPC vhost_del must succeed")?;
        ensure_eq(
            &del_resp.data.as_ref().and_then(|d| d.get("vhosts_count")).cloned(),
            &Some(serde_json::json!(1)),
            "vhosts_count after vhost_del",
        )?;

        let (beta_gone, _, _) = send_http(http_addr, "beta.local", "/index.html", &[]).await?;
        ensure_eq(&beta_gone, &404u16, "beta.local must return 404 after vhost_del")?;

        server.shutdown().await;
        Ok(())
    })
}
