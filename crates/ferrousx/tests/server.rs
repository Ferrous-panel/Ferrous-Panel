#![forbid(unsafe_code)]

use ferrousx::config::load_bundle;
use ferrousx::server::FerrousxServer;
use rustls::pki_types::{CertificateDer, ServerName};
use std::fs;
use std::future::Future;
use std::io::BufReader;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener as AsyncTcpListener, TcpStream as AsyncTcpStream};
use tokio_rustls::TlsConnector;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

static WS_SEQ: AtomicU64 = AtomicU64::new(1);

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
        let seq = WS_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "ferrousx-test-{prefix}-{}-{nanos}-{seq}",
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

async fn spawn_http_echo_upstream() -> TestResult<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let listener = AsyncTcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let addr = listener.local_addr()?;

    let task = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let Ok(n) = stream.read(&mut buf).await else {
                    return;
                };
                if n == 0 {
                    return;
                }
                let raw = String::from_utf8_lossy(&buf[..n]);
                let lower = raw.to_ascii_lowercase();
                if lower.contains("upgrade: websocket") {
                    let res = b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: test-accept\r\n\r\n";
                    if stream.write_all(res).await.is_err() {
                        return;
                    }
                    let _ = stream.flush().await;
                    let mut ping = [0u8; 64];
                    if let Ok(m) = stream.read(&mut ping).await {
                        if m > 0 {
                            let mut reply = b"echo:".to_vec();
                            reply.extend_from_slice(&ping[..m]);
                            let _ = stream.write_all(&reply).await;
                            let _ = stream.flush().await;
                        }
                    }
                } else {
                    let first_line = raw.lines().next().unwrap_or("");
                    let body = format!("proxied:{first_line}");
                    let res = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(res.as_bytes()).await;
                    let _ = stream.flush().await;
                }
            });
        }
    });

    Ok((addr, task))
}

async fn send_http_request(
    addr: SocketAddr,
    host: &str,
    method: &str,
    path: &str,
    extra_headers: &[(&str, &str)],
    body: &[u8],
) -> TestResult<(u16, String, String)> {
    let mut stream = AsyncTcpStream::connect(addr).await?;
    let mut req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (k, v) in extra_headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");

    stream.write_all(req.as_bytes()).await?;
    if !body.is_empty() {
        stream.write_all(body).await?;
    }
    stream.flush().await?;

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await?;
    parse_raw_http_response(&raw)
}

fn parse_raw_http_response(raw: &[u8]) -> TestResult<(u16, String, String)> {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or("missing HTTP header terminator")?;
    let status_line = head.lines().next().unwrap_or("");
    let code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .ok_or("missing HTTP status code")?
        .parse()?;
    Ok((code, head.to_string(), body.to_string()))
}

async fn read_one_http_response(stream: &mut AsyncTcpStream) -> TestResult<(u16, String, String)> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 2048];
    let head_end;
    loop {
        let n = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut chunk)).await??;
        ensure(n > 0, "unexpected EOF waiting for HTTP headers")?;
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            head_end = pos + 4;
            break;
        }
    }

    let head = String::from_utf8_lossy(&buf[..head_end - 4]).to_string();
    let content_len: usize = head
        .lines()
        .find_map(|line| {
            let (k, v) = line.split_once(':')?;
            if k.trim().eq_ignore_ascii_case("content-length") {
                v.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);

    while buf.len() < head_end + content_len {
        let n = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut chunk)).await??;
        ensure(n > 0, "unexpected EOF waiting for HTTP body")?;
        buf.extend_from_slice(&chunk[..n]);
    }

    let status_line = head.lines().next().unwrap_or("");
    let code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .ok_or("missing HTTP status code")?
        .parse()?;
    let body = String::from_utf8_lossy(&buf[head_end..head_end + content_len]).to_string();
    Ok((code, head, body))
}

#[test]
fn ferrousx_random_port_static_proxy_host_404_traversal_and_hot_reload() -> TestResult {
    run_async(async {
        let ws = TempDir::new("e2e")?;
        let sites_dir = ws.path().join("sites");
        let acme_dir = ws.path().join("acme");
        let static_root = ws.path().join("wwwroot/static");
        let redirect_root = ws.path().join("wwwroot/redirect");
        let outside_secret = ws.path().join("secret.txt");

        fs::create_dir_all(&sites_dir)?;
        fs::create_dir_all(acme_dir.join(".well-known/acme-challenge"))?;
        fs::create_dir_all(&static_root)?;
        fs::create_dir_all(&redirect_root)?;
        fs::write(static_root.join("index.html"), "<h1>SPA v1</h1>")?;
        fs::write(static_root.join("app.js"), "console.log('ok');")?;
        fs::write(&outside_secret, "TOP_SECRET_OUTSIDE_ROOT")?;
        fs::write(
            acme_dir.join(".well-known/acme-challenge/token123"),
            "acme-proof-token123",
        )?;

        #[cfg(unix)]
        {
            let _ = std::os::unix::fs::symlink(&outside_secret, static_root.join("escape.txt"));
        }

        let (upstream_addr, upstream_task) = spawn_http_echo_upstream().await?;

        let master_path = ws.path().join("ferrousx.toml");
        fs::write(
            &master_path,
            format!(
                "worker_threads = 2\n\
                 bind_http = \"127.0.0.1:0\"\n\
                 bind_https = \"127.0.0.1:0\"\n\
                 sites_dir = \"{}\"\n\
                 acme_webroot = \"{}\"\n\
                 max_header_bytes = 4096\n\
                 max_body_bytes = 64\n",
                sites_dir.display(),
                acme_dir.display()
            ),
        )?;

        fs::write(
            sites_dir.join("static.example.test.toml"),
            format!(
                "domain = \"static.example.test\"\n\
                 server_names = [\"static.example.test\", \"www.static.example.test\"]\n\
                 root = \"{}\"\n\
                 [handler]\n\
                 kind = \"static_spa\"\n\
                 index_file = \"index.html\"\n\
                 spa_fallback = true\n",
                static_root.display()
            ),
        )?;

        fs::write(
            sites_dir.join("proxy.example.test.toml"),
            format!(
                "domain = \"proxy.example.test\"\n\
                 root = \"{}\"\n\
                 [handler]\n\
                 kind = \"reverse_proxy\"\n\
                 upstream = \"http://{}\"\n\
                 pass_host_header = true\n\
                 forwarded_headers = true\n",
                static_root.display(),
                upstream_addr
            ),
        )?;

        fs::write(
            sites_dir.join("redir.example.test.toml"),
            format!(
                "domain = \"redir.example.test\"\n\
                 root = \"{}\"\n\
                 [acme]\n\
                 prefix = \"/.well-known/acme-challenge/\"\n\
                 webroot = \"{}\"\n\
                 serve_before_redirect = true\n\
                 [handler]\n\
                 kind = \"redirect_https\"\n\
                 status_code = 301\n",
                redirect_root.display(),
                acme_dir.display()
            ),
        )?;

        // Check --check-config library loader first
        let bundle_check = load_bundle(&master_path)?;
        ensure_eq(&bundle_check.sites.len(), &3usize, "initial loaded sites")?;

        let server = FerrousxServer::start(&master_path).await?;
        let http_addr = server.http_addr();

        // 1. Static / SPA assertions
        let (code, head, body) =
            send_http_request(http_addr, "static.example.test", "GET", "/", &[], b"").await?;
        ensure_eq(&code, &200u16, "GET / static status")?;
        ensure(head.to_ascii_lowercase().contains("text/html"), "GET / must return text/html")?;
        ensure_eq(&body, &"<h1>SPA v1</h1>".to_string(), "GET / body")?;

        let (code_alias, _, body_alias) =
            send_http_request(http_addr, "www.static.example.test", "GET", "/app.js", &[], b"")
                .await?;
        ensure_eq(&code_alias, &200u16, "alias /app.js status")?;
        ensure_eq(&body_alias, &"console.log('ok');".to_string(), "alias /app.js body")?;

        let (code_spa, _, body_spa) = send_http_request(
            http_addr,
            "static.example.test",
            "GET",
            "/dashboard/settings",
            &[],
            b"",
        )
        .await?;
        ensure_eq(&code_spa, &200u16, "SPA history fallback status")?;
        ensure_eq(&body_spa, &"<h1>SPA v1</h1>".to_string(), "SPA fallback body")?;

        // 2. Path traversal + symlink escape prevention
        let (code_trav1, _, _) = send_http_request(
            http_addr,
            "static.example.test",
            "GET",
            "/../../secret.txt",
            &[],
            b"",
        )
        .await?;
        ensure_eq(&code_trav1, &403u16, "dot-dot traversal blocked")?;

        let (code_trav2, _, _) = send_http_request(
            http_addr,
            "static.example.test",
            "GET",
            "/%2e%2e/secret.txt",
            &[],
            b"",
        )
        .await?;
        ensure_eq(&code_trav2, &403u16, "percent-encoded traversal blocked")?;

        let (code_sym, _, _) = send_http_request(
            http_addr,
            "static.example.test",
            "GET",
            "/escape.txt",
            &[],
            b"",
        )
        .await?;
        ensure_eq(&code_sym, &403u16, "symlink outside root blocked")?;

        // 3. Unknown Host -> 404 Not Found
        let (code_404, _, _) =
            send_http_request(http_addr, "unknown.example.test", "GET", "/", &[], b"").await?;
        ensure_eq(&code_404, &404u16, "unknown Host must return 404")?;

        // 4. Reverse proxy HTTP & body limit 413
        let (code_proxy, _, body_proxy) =
            send_http_request(http_addr, "proxy.example.test", "GET", "/hello?a=1", &[], b"")
                .await?;
        ensure_eq(&code_proxy, &200u16, "reverse proxy GET status")?;
        ensure_eq(
            &body_proxy,
            &"proxied:GET /hello?a=1 HTTP/1.1".to_string(),
            "reverse proxy GET body",
        )?;

        let oversized = vec![b'a'; 128];
        let (code_413, _, _) = send_http_request(
            http_addr,
            "proxy.example.test",
            "POST",
            "/upload",
            &[],
            &oversized,
        )
        .await?;
        ensure_eq(&code_413, &413u16, "body > max_body_bytes returns 413")?;

        // 5. Reverse proxy WebSocket upgrade bidirectional tunnel
        let mut ws_stream = AsyncTcpStream::connect(http_addr).await?;
        let ws_req = "GET /ws HTTP/1.1\r\n\
                      Host: proxy.example.test\r\n\
                      Connection: Upgrade\r\n\
                      Upgrade: websocket\r\n\
                      Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                      Sec-WebSocket-Version: 13\r\n\r\n";
        ws_stream.write_all(ws_req.as_bytes()).await?;
        ws_stream.flush().await?;

        let mut ws_buf = [0u8; 512];
        let n_ws = tokio::time::timeout(Duration::from_secs(3), ws_stream.read(&mut ws_buf)).await??;
        let ws_head = String::from_utf8_lossy(&ws_buf[..n_ws]);
        ensure(
            ws_head.starts_with("HTTP/1.1 101 Switching Protocols"),
            "WebSocket upgrade must return 101 Switching Protocols",
        )?;

        ws_stream.write_all(b"ping-ferrousx").await?;
        ws_stream.flush().await?;
        let n_echo =
            tokio::time::timeout(Duration::from_secs(3), ws_stream.read(&mut ws_buf)).await??;
        ensure_eq(
            &String::from_utf8_lossy(&ws_buf[..n_echo]).to_string(),
            &"echo:ping-ferrousx".to_string(),
            "WebSocket tunnel bidirectional echo",
        )?;
        drop(ws_stream);

        // 6. ACME challenge served BEFORE redirect_https, normal paths return 301
        let (code_acme, _, body_acme) = send_http_request(
            http_addr,
            "redir.example.test",
            "GET",
            "/.well-known/acme-challenge/token123",
            &[],
            b"",
        )
        .await?;
        ensure_eq(&code_acme, &200u16, "ACME challenge served before redirect")?;
        ensure_eq(&body_acme, &"acme-proof-token123".to_string(), "ACME challenge body")?;

        let (code_redir, head_redir, _) =
            send_http_request(http_addr, "redir.example.test", "GET", "/login?next=1", &[], b"")
                .await?;
        ensure_eq(&code_redir, &301u16, "redirect_https status")?;
        ensure(
            head_redir.contains("https://redir.example.test/login?next=1"),
            "redirect_https Location header",
        )?;

        // 7. Hot reload without dropping an open HTTP/1.1 keep-alive connection!
        let mut keepalive_conn = AsyncTcpStream::connect(http_addr).await?;
        keepalive_conn
            .write_all(
                b"GET / HTTP/1.1\r\nHost: static.example.test\r\nConnection: keep-alive\r\n\r\n",
            )
            .await?;
        keepalive_conn.flush().await?;
        let (ka_code1, _, ka_body1) = read_one_http_response(&mut keepalive_conn).await?;
        ensure_eq(&ka_code1, &200u16, "keepalive pre-reload request")?;
        ensure_eq(&ka_body1, &"<h1>SPA v1</h1>".to_string(), "keepalive pre-reload body")?;

        // Add a brand-new site & modify index.html while `keepalive_conn` stays open, then reload()
        let reloaded_root = ws.path().join("wwwroot/reloaded");
        fs::create_dir_all(&reloaded_root)?;
        fs::write(reloaded_root.join("index.html"), "reloaded-v2")?;
        fs::write(
            sites_dir.join("reloaded.example.test.toml"),
            format!(
                "domain = \"reloaded.example.test\"\n\
                 root = \"{}\"\n\
                 [handler]\n\
                 kind = \"static_spa\"\n\
                 index_file = \"index.html\"\n",
                reloaded_root.display()
            ),
        )?;

        let reloaded_count = server.reload()?;
        ensure_eq(&reloaded_count, &4usize, "reloaded sites count")?;

        // Send second request on the SAME TCP socket targeting newly loaded host!
        keepalive_conn
            .write_all(
                b"GET / HTTP/1.1\r\nHost: reloaded.example.test\r\nConnection: close\r\n\r\n",
            )
            .await?;
        keepalive_conn.flush().await?;
        let (ka_code2, _, ka_body2) = read_one_http_response(&mut keepalive_conn).await?;
        ensure_eq(&ka_code2, &200u16, "same keepalive socket post-reload status")?;
        ensure_eq(&ka_body2, &"reloaded-v2".to_string(), "same keepalive socket post-reload body")?;

        // Bad config during reload must fail validation AND keep active server working
        fs::write(sites_dir.join("broken.example.test.toml"), "not valid toml = [")?;
        ensure(server.reload().is_err(), "broken config reload must fail")?;
        let (code_after_bad, _, _) = send_http_request(
            http_addr,
            "reloaded.example.test",
            "GET",
            "/",
            &[],
            b"",
        )
        .await?;
        ensure_eq(
            &code_after_bad,
            &200u16,
            "existing valid routes remain active after failed reload",
        )?;

        upstream_task.abort();
        server.shutdown().await;
        Ok(())
    })
}

#[test]
fn ferrousx_tls_sni_and_alpn_selection() -> TestResult {
    // Only run live certificate generation when `/usr/bin/openssl` is present in host environment.
    if !Path::new("/usr/bin/openssl").is_file() {
        return Ok(());
    }

    run_async(async {
        let ws = TempDir::new("tls-sni")?;
        let sites_dir = ws.path().join("sites");
        let certs_dir = ws.path().join("certs");
        let root_a = ws.path().join("wwwroot/alpha");
        let root_b = ws.path().join("wwwroot/beta");

        fs::create_dir_all(&sites_dir)?;
        fs::create_dir_all(&certs_dir)?;
        fs::create_dir_all(&root_a)?;
        fs::create_dir_all(&root_b)?;
        fs::write(root_a.join("index.html"), "alpha-tls")?;
        fs::write(root_b.join("index.html"), "beta-tls")?;

        for (name, cn) in [("alpha", "alpha.example.test"), ("beta", "beta.example.test")] {
            let status = Command::new("/usr/bin/openssl")
                .args([
                    "req",
                    "-x509",
                    "-newkey",
                    "rsa:2048",
                    "-nodes",
                    "-keyout",
                    &certs_dir.join(format!("{name}.key")).display().to_string(),
                    "-out",
                    &certs_dir.join(format!("{name}.crt")).display().to_string(),
                    "-days",
                    "1",
                    "-subj",
                    &format!("/CN={cn}"),
                    "-addext",
                    &format!("subjectAltName=DNS:{cn}"),
                ])
                .status()?;
            ensure(status.success(), "openssl self-signed test cert generation failed")?;
        }

        let master_path = ws.path().join("ferrousx.toml");
        fs::write(
            &master_path,
            format!(
                "bind_http = \"127.0.0.1:0\"\n\
                 bind_https = \"127.0.0.1:0\"\n\
                 sites_dir = \"{}\"\n\
                 acme_webroot = \"{}\"\n",
                sites_dir.display(),
                ws.path().join("acme").display()
            ),
        )?;

        for (name, cn, root) in [
            ("alpha", "alpha.example.test", &root_a),
            ("beta", "beta.example.test", &root_b),
        ] {
            fs::write(
                sites_dir.join(format!("{cn}.toml")),
                format!(
                    "domain = \"{cn}\"\n\
                     root = \"{}\"\n\
                     [ssl]\n\
                     enabled = true\n\
                     redirect_https = true\n\
                     cert_path = \"{}\"\n\
                     key_path = \"{}\"\n\
                     [handler]\n\
                     kind = \"static_spa\"\n\
                     index_file = \"index.html\"\n",
                    root.display(),
                    certs_dir.join(format!("{name}.crt")).display(),
                    certs_dir.join(format!("{name}.key")).display(),
                ),
            )?;
        }

        let server = FerrousxServer::start(&master_path).await?;
        let https_addr = server.https_addr();

        let alpha_der = read_first_cert_der(&certs_dir.join("alpha.crt"))?;
        let beta_der = read_first_cert_der(&certs_dir.join("beta.crt"))?;

        let mut root_store = rustls::RootCertStore::empty();
        root_store.add(alpha_der.clone())?;
        root_store.add(beta_der.clone())?;

        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut client_cfg = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()?
            .with_root_certificates(root_store)
            .with_no_client_auth();
        client_cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
        let connector = TlsConnector::from(Arc::new(client_cfg));

        // Probe SNI alpha.example.test -> must negotiate alpha cert and serve alpha-tls
        let tcp_a = AsyncTcpStream::connect(https_addr).await?;
        let sni_a = ServerName::try_from("alpha.example.test".to_string())?;
        let mut tls_a = connector.connect(sni_a, tcp_a).await?;
        let peer_a = tls_a
            .get_ref()
            .1
            .peer_certificates()
            .and_then(|c| c.first())
            .ok_or("missing peer cert for alpha")?;
        ensure_eq(peer_a, &alpha_der, "SNI alpha certificate selection")?;
        ensure_eq(
            &tls_a.get_ref().1.alpn_protocol().map(|b| b.to_vec()),
            &Some(b"http/1.1".to_vec()),
            "ALPN http/1.1 negotiation",
        )?;

        tls_a
            .write_all(b"GET / HTTP/1.1\r\nHost: alpha.example.test\r\nConnection: close\r\n\r\n")
            .await?;
        tls_a.flush().await?;
        let mut raw_a = Vec::new();
        tls_a.read_to_end(&mut raw_a).await?;
        let (code_a, _, body_a) = parse_raw_http_response(&raw_a)?;
        ensure_eq(&code_a, &200u16, "TLS alpha status")?;
        ensure_eq(&body_a, &"alpha-tls".to_string(), "TLS alpha body")?;

        // Probe SNI beta.example.test -> must select beta cert (not alpha!)
        let tcp_b = AsyncTcpStream::connect(https_addr).await?;
        let sni_b = ServerName::try_from("beta.example.test".to_string())?;
        let mut tls_b = connector.connect(sni_b, tcp_b).await?;
        let peer_b = tls_b
            .get_ref()
            .1
            .peer_certificates()
            .and_then(|c| c.first())
            .ok_or("missing peer cert for beta")?;
        ensure_eq(peer_b, &beta_der, "SNI beta certificate selection")?;

        tls_b
            .write_all(b"GET / HTTP/1.1\r\nHost: beta.example.test\r\nConnection: close\r\n\r\n")
            .await?;
        tls_b.flush().await?;
        let mut raw_b = Vec::new();
        tls_b.read_to_end(&mut raw_b).await?;
        let (code_b, _, body_b) = parse_raw_http_response(&raw_b)?;
        ensure_eq(&code_b, &200u16, "TLS beta status")?;
        ensure_eq(&body_b, &"beta-tls".to_string(), "TLS beta body")?;

        server.shutdown().await;
        Ok(())
    })
}

#[test]
fn check_config_rejects_invalid_upstream_and_missing_tls_certs() -> TestResult {
    let ws = TempDir::new("check-cfg")?;
    let sites_dir = ws.path().join("sites");
    fs::create_dir_all(&sites_dir)?;

    let master_path = ws.path().join("ferrousx.toml");
    fs::write(
        &master_path,
        format!(
            "bind_http = \"127.0.0.1:0\"\n\
             bind_https = \"127.0.0.1:0\"\n\
             sites_dir = \"{}\"\n\
             acme_webroot = \"{}\"\n",
            sites_dir.display(),
            ws.path().join("acme").display()
        ),
    )?;

    fs::write(
        sites_dir.join("bad.example.test.toml"),
        "domain = \"bad.example.test\"\n\
         root = \"/www/wwwroot/bad\"\n\
         [ssl]\n\
         enabled = true\n\
         cert_path = \"/nonexistent/cert.pem\"\n\
         key_path = \"/nonexistent/key.pem\"\n\
         [handler]\n\
         kind = \"static_spa\"\n",
    )?;

    ensure(
        load_bundle(&master_path).is_err(),
        "load_bundle / --check-config must fail when ssl cert files are missing",
    )?;

    let _ = fs::remove_file(sites_dir.join("bad.example.test.toml"));
    fs::write(
        sites_dir.join("proxy.example.test.toml"),
        "domain = \"proxy.example.test\"\n\
         root = \"/www/wwwroot/proxy\"\n\
         [handler]\n\
         kind = \"reverse_proxy\"\n\
         upstream = \"ftp://not-http:8080\"\n",
    )?;
    ensure(
        load_bundle(&master_path).is_err(),
        "load_bundle must reject non-http upstream schemes",
    )?;

    Ok(())
}

fn read_first_cert_der(path: &Path) -> TestResult<CertificateDer<'static>> {
    let f = fs::File::open(path)?;
    let mut reader = BufReader::new(f);
    let cert = rustls_pemfile::certs(&mut reader)
        .next()
        .ok_or("empty certificate file")??;
    Ok(cert)
}
