use crate::config::{
    LoadedBundle, SiteHandler, UpstreamOrigin, ValidatedDirectoryAuth, ValidatedHotlinkConfig,
    ValidatedRedirectRule, ValidatedSite,
};
use crate::ipc::EngineRuntimeMetrics;
use crate::router::{match_site_for_request, resolve_safe_path, PathError};
use fastcgi_client::{Client as FastcgiClient, Params as FastcgiParams, Request as FastcgiRequest};
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty, Full, Limited};
use hyper::body::{Body, Bytes, Frame, Incoming};
use hyper::header::{self, HeaderMap, HeaderName, HeaderValue};
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Cursor;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};
use std::task::{Context, Poll};
use std::time::Instant;
use subtle::ConstantTimeEq;
use tokio::io::{AsyncRead, ReadBuf};
use tokio::net::{TcpStream, UnixStream};

pub type RespBody = BoxBody<Bytes, hyper::Error>;
pub type HttpLegacyClient = Client<HttpConnector, Limited<Incoming>>;

const FILE_CHUNK_SIZE: usize = 64 * 1024;

/// Per-site, per-IP token-bucket rate limiter enforced on every HTTP/HTTPS request.
#[derive(Debug, Default)]
pub struct IpRateLimiter {
    buckets: Mutex<HashMap<(String, IpAddr), (f64, Instant)>>,
}

impl IpRateLimiter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
        }
    }

    #[must_use]
    pub fn allow(&self, domain: &str, ip: IpAddr, rps: u32) -> bool {
        if rps == 0 {
            return true;
        }
        let Ok(mut map) = self.buckets.lock() else {
            return true;
        };
        let now = Instant::now();
        if map.len() > 4096 {
            map.retain(|_, (_, last)| now.duration_since(*last).as_secs() < 60);
        }
        let capacity = f64::from(rps).max(1.0);
        let rate = f64::from(rps);
        let key = (domain.to_string(), ip);
        let entry = map.entry(key).or_insert((capacity, now));
        let elapsed = now.duration_since(entry.1).as_secs_f64();
        entry.0 = (entry.0 + elapsed * rate).min(capacity);
        entry.1 = now;
        if entry.0 >= 1.0 {
            entry.0 -= 1.0;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppRuntime {
    pub bundle: Arc<RwLock<Arc<LoadedBundle>>>,
    pub http_client: HttpLegacyClient,
    pub metrics: Arc<EngineRuntimeMetrics>,
    pub rate_limiter: Arc<IpRateLimiter>,
}

impl AppRuntime {
    #[must_use]
    pub fn new(initial: LoadedBundle) -> Self {
        let mut connector = HttpConnector::new();
        connector.set_connect_timeout(Some(initial.master.upstream_connect_timeout));
        let http_client = Client::builder(TokioExecutor::new()).build(connector);
        Self {
            bundle: Arc::new(RwLock::new(Arc::new(initial))),
            http_client,
            metrics: Arc::new(EngineRuntimeMetrics::new()),
            rate_limiter: Arc::new(IpRateLimiter::new()),
        }
    }

    #[must_use]
    pub fn current_bundle(&self) -> Option<Arc<LoadedBundle>> {
        self.bundle.read().ok().map(|g| Arc::clone(&g))
    }

    pub fn swap_bundle(&self, next: LoadedBundle) -> Result<(), &'static str> {
        let mut guard = self
            .bundle
            .write()
            .map_err(|_| "runtime bundle write lock poisoned")?;
        *guard = Arc::new(next);
        Ok(())
    }
}

pub async fn dispatch_request(
    req: Request<Incoming>,
    peer_addr: SocketAddr,
    is_tls: bool,
    runtime: Arc<AppRuntime>,
) -> Result<Response<RespBody>, std::convert::Infallible> {
    runtime.metrics.record_request();
    let Some(bundle) = runtime.current_bundle() else {
        return Ok(text_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "server state unavailable",
            false,
        ));
    };

    let server_tokens = bundle.master.server_tokens;
    let (site, host) = match match_site_for_request(&req, &bundle) {
        Ok(pair) => pair,
        Err(err) => {
            return Ok(text_response(err.status_code(), err.message(), server_tokens));
        }
    };

    let req_path = req.uri().path().to_string();

    // 1. Serve ACME HTTP-01 challenge cleanly before HTTPS redirects when enabled.
    if !is_tls && site.acme_serve_before_redirect && req_path.starts_with(&site.acme_prefix) {
        if !matches!(*req.method(), Method::GET | Method::HEAD) {
            return Ok(text_response(
                StatusCode::METHOD_NOT_ALLOWED,
                "Method Not Allowed",
                server_tokens,
            ));
        }
        let is_head = *req.method() == Method::HEAD;
        return Ok(serve_static_file_from_root(
            &site.acme_webroot,
            &req_path,
            None,
            StaticServeOptions::acme(),
            None,
            is_head,
            server_tokens,
        )
        .await);
    }

    // 2. HTTPS redirection (explicit handler OR [ssl] redirect_https = true).
    if !is_tls {
        let redirect_code = match &site.handler {
            SiteHandler::RedirectHttps { status_code } => Some(*status_code),
            _ if site.ssl_enabled && site.redirect_https => Some(301),
            _ => None,
        };
        if let Some(code) = redirect_code {
            return Ok(build_https_redirect(
                code,
                &host,
                req.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or("/"),
                server_tokens,
            ));
        }
    }

    // 2.5 Per-site IP token-bucket rate limiting.
    if site.rate_limit_rps > 0
        && !runtime
            .rate_limiter
            .allow(site.domain.as_str(), peer_addr.ip(), site.rate_limit_rps)
    {
        runtime.metrics.record_rate_limited();
        return Ok(build_rate_limited_response(server_tokens));
    }

    // 2.6 Per-site directory HTTP Basic Authentication (htpasswd).
    if let Some(auth_rule) = match_directory_auth(&site.directory_auth, &req_path) {
        if !verify_request_basic_auth(req.headers(), auth_rule) {
            return Ok(build_basic_auth_challenge(&auth_rule.label, server_tokens));
        }
    }

    // 2.7 Per-site URL redirect rules.
    if let Some(redir) = match_redirect_rule(&site.redirects, &req_path) {
        return Ok(build_custom_redirect(
            redir,
            req.uri().query(),
            server_tokens,
        ));
    }

    // 2.8 Per-site hotlink protection.
    if let Some(ref hl) = site.hotlink {
        if is_hotlink_blocked(hl, &req_path, req.headers(), site) {
            runtime.metrics.record_waf_blocked();
            return Ok(text_response(
                StatusCode::FORBIDDEN,
                "Hotlink Forbidden",
                server_tokens,
            ));
        }
    }

    // 3. Enforce Content-Length against max_body_bytes before dispatching upstream/FastCGI.
    if let Some(cl_hdr) = req.headers().get(header::CONTENT_LENGTH) {
        let Ok(cl_str) = cl_hdr.to_str() else {
            return Ok(text_response(
                StatusCode::BAD_REQUEST,
                "Invalid Content-Length header",
                server_tokens,
            ));
        };
        let Ok(cl_val) = cl_str.trim().parse::<usize>() else {
            return Ok(text_response(
                StatusCode::BAD_REQUEST,
                "Invalid Content-Length header",
                server_tokens,
            ));
        };
        if cl_val > bundle.master.max_body_bytes {
            return Ok(text_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "Payload Too Large",
                server_tokens,
            ));
        }
    }

    let response = match &site.handler {
        SiteHandler::StaticSpa {
            index_file,
            spa_fallback,
            clean_urls,
            precompressed,
            immutable_assets,
        } => {
            runtime.metrics.record_static();
            if !matches!(*req.method(), Method::GET | Method::HEAD) {
                text_response(
                    StatusCode::METHOD_NOT_ALLOWED,
                    "Method Not Allowed",
                    server_tokens,
                )
            } else {
                let is_head = *req.method() == Method::HEAD;
                let accept_encoding = req
                    .headers()
                    .get(header::ACCEPT_ENCODING)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string);
                let opts = StaticServeOptions {
                    spa_fallback: *spa_fallback,
                    clean_urls: *clean_urls,
                    precompressed: *precompressed,
                    immutable_assets: *immutable_assets,
                };
                serve_static_file_from_root(
                    &site.root,
                    &req_path,
                    Some(index_file.as_str()),
                    opts,
                    accept_encoding.as_deref(),
                    is_head,
                    server_tokens,
                )
                .await
            }
        }
        SiteHandler::ReverseProxy {
            upstream,
            pass_host_header,
            forwarded_headers,
        } => {
            let res = handle_reverse_proxy(
                req,
                peer_addr,
                is_tls,
                &host,
                upstream,
                *pass_host_header,
                *forwarded_headers,
                &bundle,
                &runtime.http_client,
            )
            .await;
            if res.status().is_server_error() {
                runtime.metrics.record_upstream_error();
            }
            res
        }
        SiteHandler::PhpFpm {
            fastcgi_socket,
            index_file,
        } => {
            runtime.metrics.record_php_fpm();
            let res = handle_php_fpm(
                req,
                peer_addr,
                is_tls,
                &host,
                site,
                fastcgi_socket,
                index_file.as_str(),
                &bundle,
            )
            .await;
            if res.status().is_server_error() {
                runtime.metrics.record_upstream_error();
            }
            res
        }
        SiteHandler::RedirectHttps { .. } => text_response(
            StatusCode::NOT_FOUND,
            "Not Found",
            server_tokens,
        ),
    };

    if let Some(len_bytes) = response
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
    {
        runtime.metrics.record_bytes_sent(len_bytes);
    }

    Ok(response)
}

fn build_https_redirect(
    code: u16,
    host: &str,
    path_and_query: &str,
    server_tokens: bool,
) -> Response<RespBody> {
    let status = StatusCode::from_u16(code).unwrap_or(StatusCode::MOVED_PERMANENTLY);
    let location = format!("https://{host}{path_and_query}");
    let mut builder = Response::builder().status(status);
    if let Ok(val) = HeaderValue::from_str(&location) {
        builder = builder.header(header::LOCATION, val);
    }
    if server_tokens {
        builder = builder.header(header::SERVER, "FerrousX");
    }
    builder
        .body(empty_body())
        .unwrap_or_else(|_| text_response(StatusCode::INTERNAL_SERVER_ERROR, "redirect error", false))
}

fn build_rate_limited_response(server_tokens: bool) -> Response<RespBody> {
    let body = Full::new(Bytes::from_static(b"Too Many Requests"))
        .map_err(|never| match never {})
        .boxed();
    let mut builder = Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::RETRY_AFTER, "1");
    if server_tokens {
        builder = builder.header(header::SERVER, "FerrousX");
    }
    builder
        .body(body)
        .unwrap_or_else(|_| text_response(StatusCode::TOO_MANY_REQUESTS, "Too Many Requests", false))
}

fn match_redirect_rule<'a>(
    rules: &'a [ValidatedRedirectRule],
    req_path: &str,
) -> Option<&'a ValidatedRedirectRule> {
    rules.iter().find(|r| {
        if r.source_path == req_path {
            true
        } else if r.source_path.ends_with('/') {
            req_path.starts_with(&r.source_path)
        } else {
            false
        }
    })
}

fn build_custom_redirect(
    rule: &ValidatedRedirectRule,
    query: Option<&str>,
    server_tokens: bool,
) -> Response<RespBody> {
    let status = StatusCode::from_u16(rule.status_code).unwrap_or(StatusCode::MOVED_PERMANENTLY);
    let mut location = rule.target_url.clone();
    if rule.keep_query {
        if let Some(q) = query {
            if !q.is_empty() {
                if location.contains('?') {
                    location.push('&');
                } else {
                    location.push('?');
                }
                location.push_str(q);
            }
        }
    }
    let mut builder = Response::builder().status(status);
    if let Ok(val) = HeaderValue::from_str(&location) {
        builder = builder.header(header::LOCATION, val);
    }
    if server_tokens {
        builder = builder.header(header::SERVER, "FerrousX");
    }
    builder
        .body(empty_body())
        .unwrap_or_else(|_| text_response(StatusCode::INTERNAL_SERVER_ERROR, "redirect error", false))
}

fn is_hotlink_blocked(
    hl: &ValidatedHotlinkConfig,
    req_path: &str,
    headers: &HeaderMap,
    site: &ValidatedSite,
) -> bool {
    let Some(ext) = Path::new(req_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
    else {
        return false;
    };
    if !hl.protected_extensions.iter().any(|e| e == &ext) {
        return false;
    }

    let referer = headers
        .get(header::REFERER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .unwrap_or("");
    if referer.is_empty() {
        return hl.block_empty_referer;
    }

    let Some(ref_host) = extract_url_host(referer) else {
        return true;
    };
    let lower_host = ref_host.to_ascii_lowercase();
    if site
        .server_names
        .iter()
        .any(|d| lower_host == d.as_str() || lower_host.ends_with(&format!(".{}", d.as_str())))
    {
        return false;
    }
    for allowed in &hl.allowed_domains {
        let clean = allowed.trim_start_matches("*.").trim_start_matches('.');
        if !clean.is_empty()
            && (lower_host == clean || lower_host.ends_with(&format!(".{clean}")))
        {
            return false;
        }
    }
    true
}

fn extract_url_host(raw: &str) -> Option<&str> {
    let rest = raw
        .strip_prefix("https://")
        .or_else(|| raw.strip_prefix("http://"))?;
    let authority = rest.split('/').next()?;
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    if host_port.starts_with('[') {
        host_port.split(']').next().map(|s| s.trim_start_matches('['))
    } else {
        Some(host_port.split(':').next().unwrap_or(host_port))
    }
}

fn match_directory_auth<'a>(
    rules: &'a [ValidatedDirectoryAuth],
    req_path: &str,
) -> Option<&'a ValidatedDirectoryAuth> {
    rules.iter().find(|r| {
        if r.path == "/" {
            true
        } else {
            let trimmed = r.path.trim_end_matches('/');
            req_path == trimmed || req_path.starts_with(&format!("{trimmed}/"))
        }
    })
}

fn build_basic_auth_challenge(realm: &str, server_tokens: bool) -> Response<RespBody> {
    let body = Full::new(Bytes::from_static(b"Unauthorized"))
        .map_err(|never| match never {})
        .boxed();
    let header_val = format!("Basic realm=\"{}\"", realm.replace('"', "'"));
    let mut builder = Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::WWW_AUTHENTICATE, header_val);
    if server_tokens {
        builder = builder.header(header::SERVER, "FerrousX");
    }
    builder
        .body(body)
        .unwrap_or_else(|_| text_response(StatusCode::UNAUTHORIZED, "Unauthorized", false))
}

fn verify_request_basic_auth(headers: &HeaderMap, rule: &ValidatedDirectoryAuth) -> bool {
    let Some(auth_hdr) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    let Some(b64_part) = auth_hdr
        .trim()
        .strip_prefix("Basic ")
        .or_else(|| auth_hdr.trim().strip_prefix("basic "))
    else {
        return false;
    };
    let Some(decoded) = decode_base64(b64_part.trim()) else {
        return false;
    };
    let Ok(creds) = String::from_utf8(decoded) else {
        return false;
    };
    let Some((req_user, req_pass)) = creds.split_once(':') else {
        return false;
    };
    let Ok(htpasswd_content) = std::fs::read_to_string(&rule.htpasswd_file) else {
        return false;
    };
    for line in htpasswd_content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((file_user, stored_hash)) = trimmed.split_once(':') else {
            continue;
        };
        if file_user == req_user && verify_htpasswd_hash(req_pass, stored_hash.trim()) {
            return true;
        }
    }
    false
}

/// Verifies Apache/Nginx `$apr1$...`, `{SSHA}...`, and `{SHA}...` htpasswd hashes in constant time.
#[must_use]
pub fn verify_htpasswd_hash(password: &str, stored_hash: &str) -> bool {
    if let Some(rest) = stored_hash.strip_prefix("$apr1$") {
        let Some((salt, _)) = rest.split_once('$') else {
            return false;
        };
        if salt.is_empty() || salt.len() > 8 {
            return false;
        }
        let computed = compute_apr1_hash(password, salt);
        return bool::from(computed.as_bytes().ct_eq(stored_hash.as_bytes()));
    }
    if let Some(b64) = stored_hash.strip_prefix("{SSHA}") {
        let Some(raw) = decode_base64(b64) else {
            return false;
        };
        if raw.len() <= 20 {
            return false;
        }
        let (expected_digest, salt) = raw.split_at(20);
        let mut msg = Vec::with_capacity(password.len() + salt.len());
        msg.extend_from_slice(password.as_bytes());
        msg.extend_from_slice(salt);
        let actual = sha1_digest(&msg);
        return bool::from(actual.ct_eq(expected_digest));
    }
    if let Some(b64) = stored_hash.strip_prefix("{SHA}") {
        let Some(raw) = decode_base64(b64) else {
            return false;
        };
        if raw.len() != 20 {
            return false;
        }
        let actual = sha1_digest(password.as_bytes());
        return bool::from(actual.ct_eq(raw.as_slice()));
    }
    false
}

/// Computes Apache `$apr1$<salt>$<22-char-hash>` compatible with `openssl passwd -apr1` and Nginx.
#[must_use]
pub fn compute_apr1_hash(password: &str, salt_raw: &str) -> String {
    let salt = &salt_raw[..salt_raw.len().min(8)];
    let pw = password.as_bytes();
    let salt_bytes = salt.as_bytes();

    // Alternate digest: MD5(pw + salt + pw)
    let mut alt_input = Vec::with_capacity(pw.len() * 2 + salt_bytes.len());
    alt_input.extend_from_slice(pw);
    alt_input.extend_from_slice(salt_bytes);
    alt_input.extend_from_slice(pw);
    let alt = md5_digest(&alt_input);

    // Initial digest: pw + "$apr1$" + salt + alt chunks + length bit encoding
    let mut init_input = Vec::with_capacity(pw.len() * 2 + salt_bytes.len() + 32);
    init_input.extend_from_slice(pw);
    init_input.extend_from_slice(b"$apr1$");
    init_input.extend_from_slice(salt_bytes);
    let mut rem = pw.len();
    while rem > 0 {
        let take = rem.min(16);
        init_input.extend_from_slice(&alt[..take]);
        rem -= take;
    }
    let mut bit_len = pw.len();
    while bit_len > 0 {
        if (bit_len & 1) != 0 {
            init_input.push(0u8);
        } else if let Some(&first) = pw.first() {
            init_input.push(first);
        } else {
            init_input.push(0u8);
        }
        bit_len >>= 1;
    }
    let mut digest = md5_digest(&init_input);

    let mut round_buf = Vec::with_capacity(pw.len() * 2 + salt_bytes.len() + 32);
    for i in 0..1000usize {
        round_buf.clear();
        if (i & 1) != 0 {
            round_buf.extend_from_slice(pw);
        } else {
            round_buf.extend_from_slice(&digest);
        }
        if i % 3 != 0 {
            round_buf.extend_from_slice(salt_bytes);
        }
        if i % 7 != 0 {
            round_buf.extend_from_slice(pw);
        }
        if (i & 1) != 0 {
            round_buf.extend_from_slice(&digest);
        } else {
            round_buf.extend_from_slice(pw);
        }
        digest = md5_digest(&round_buf);
    }

    const ITOA64: &[u8; 64] = b"./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let mut encoded = String::with_capacity(22);
    let mut push_64 = |mut v: u32, count: usize| {
        for _ in 0..count {
            encoded.push(char::from(ITOA64[(v & 0x3f) as usize]));
            v >>= 6;
        }
    };
    let groups = [
        (0usize, 6usize, 12usize),
        (1, 7, 13),
        (2, 8, 14),
        (3, 9, 15),
        (4, 10, 5),
    ];
    for (a, b, c) in groups {
        let v = (u32::from(digest[a]) << 16)
            | (u32::from(digest[b]) << 8)
            | u32::from(digest[c]);
        push_64(v, 4);
    }
    push_64(u32::from(digest[11]), 2);

    format!("$apr1${salt}${encoded}")
}

fn md5_digest(input: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
        5,  9, 14, 20, 5,  9, 14, 20, 5,  9, 14, 20, 5,  9, 14, 20,
        4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
        6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
        0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
        0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
        0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
        0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
        0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
        0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
    ];

    let mut msg = input.to_vec();
    let bit_len = (input.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());

    let mut a0: u32 = 0x6745_2301;
    let mut b0: u32 = 0xefcd_ab89;
    let mut c0: u32 = 0x98ba_dcfe;
    let mut d0: u32 = 0x1032_5476;

    for chunk in msg.chunks_exact(64) {
        let mut m = [0u32; 16];
        for (idx, word) in m.iter_mut().enumerate() {
            let off = idx * 4;
            *word = u32::from_le_bytes([
                chunk[off],
                chunk[off + 1],
                chunk[off + 2],
                chunk[off + 3],
            ]);
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64usize {
            let (f, g) = match i {
                0..=15 => ((b & c) | ((!b) & d), i),
                16..=31 => ((d & b) | ((!d) & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | (!d)), (7 * i) % 16),
            };
            let tmp = d;
            d = c;
            c = b;
            let sum = a.wrapping_add(f).wrapping_add(K[i]).wrapping_add(m[g]);
            b = b.wrapping_add(sum.rotate_left(S[i]));
            a = tmp;
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..16].copy_from_slice(&d0.to_le_bytes());
    out
}

fn sha1_digest(input: &[u8]) -> [u8; 20] {
    let mut msg = input.to_vec();
    let bit_len = (input.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    let mut h0: u32 = 0x6745_2301;
    let mut h1: u32 = 0xefcd_ab89;
    let mut h2: u32 = 0x98ba_dcfe;
    let mut h3: u32 = 0x1032_5476;
    let mut h4: u32 = 0xc3d2_e1f0;

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            let off = i * 4;
            w[i] = u32::from_be_bytes([
                chunk[off],
                chunk[off + 1],
                chunk[off + 2],
                chunk[off + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h0, h1, h2, h3, h4);
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5a82_7999u32),
                20..=39 => (b ^ c ^ d, 0x6ed9_eba1u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc_u32),
                _ => (b ^ c ^ d, 0xca62_c1d6u32),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let bytes: Vec<u8> = input
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    if bytes.is_empty() || bytes.len() % 4 != 0 {
        return None;
    }
    let val = |c: u8| -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let mut out = Vec::with_capacity((bytes.len() / 4) * 3);
    for chunk in bytes.chunks_exact(4) {
        let a = val(chunk[0])?;
        let b = val(chunk[1])?;
        out.push((a << 2) | (b >> 4));
        if chunk[2] != b'=' {
            let c = val(chunk[2])?;
            out.push((b << 4) | (c >> 2));
            if chunk[3] != b'=' {
                let d = val(chunk[3])?;
                out.push((c << 6) | d);
            }
        }
    }
    Some(out)
}

#[derive(Debug, Clone, Copy)]
struct StaticServeOptions {
    spa_fallback: bool,
    clean_urls: bool,
    precompressed: bool,
    immutable_assets: bool,
}

impl StaticServeOptions {
    const fn acme() -> Self {
        Self {
            spa_fallback: false,
            clean_urls: false,
            precompressed: false,
            immutable_assets: false,
        }
    }

    const fn static_assets_only() -> Self {
        Self {
            spa_fallback: false,
            clean_urls: false,
            precompressed: true,
            immutable_assets: true,
        }
    }
}

fn resolve_web_mime(path: &Path) -> String {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("js" | "mjs" | "cjs") => "application/javascript; charset=utf-8".to_string(),
        Some("css") => "text/css; charset=utf-8".to_string(),
        Some("html" | "htm") => "text/html; charset=utf-8".to_string(),
        Some("json" | "map" | "webmanifest") => "application/json; charset=utf-8".to_string(),
        Some("wasm") => "application/wasm".to_string(),
        Some("svg") => "image/svg+xml".to_string(),
        Some("woff2") => "font/woff2".to_string(),
        Some("woff") => "font/woff".to_string(),
        Some("ttf") => "font/ttf".to_string(),
        Some("otf") => "font/otf".to_string(),
        Some("webp") => "image/webp".to_string(),
        Some("avif") => "image/avif".to_string(),
        Some("ico") => "image/x-icon".to_string(),
        Some("xml") => "application/xml; charset=utf-8".to_string(),
        Some("txt") => "text/plain; charset=utf-8".to_string(),
        _ => mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string(),
    }
}

fn is_compressible_asset(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .as_deref(),
        Some("html" | "htm" | "css" | "js" | "mjs" | "cjs" | "json" | "svg" | "xml" | "txt" | "wasm" | "map")
    )
}

fn accept_encoding_supports(accept_encoding: Option<&str>, encoding: &str) -> bool {
    let Some(header_val) = accept_encoding else {
        return false;
    };
    header_val.split(',').any(|token| {
        let mut parts = token.trim().split(';');
        let Some(name) = parts.next() else {
            return false;
        };
        if !name.trim().eq_ignore_ascii_case(encoding) {
            return false;
        }
        for param in parts {
            let trimmed = param.trim();
            if let Some(q_val) = trimmed.strip_prefix("q=") {
                if q_val.trim().parse::<f32>().ok().is_some_and(|q| q <= 0.0) {
                    return false;
                }
            }
        }
        true
    })
}

fn cache_control_for_asset(req_path: &str, file_path: &Path, immutable_assets: bool) -> Option<&'static str> {
    if !immutable_assets {
        return None;
    }
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());
    if matches!(ext.as_deref(), Some("html" | "htm")) {
        return Some("no-cache, must-revalidate");
    }
    let is_framework_immutable_dir = req_path.starts_with("/assets/")
        || req_path.starts_with("/_next/static/")
        || req_path.starts_with("/_astro/")
        || req_path.starts_with("/_nuxt/")
        || req_path.starts_with("/_app/immutable/");
    if is_framework_immutable_dir {
        return Some("public, max-age=31536000, immutable");
    }
    if matches!(
        ext.as_deref(),
        Some("js" | "mjs" | "css" | "wasm" | "woff2" | "woff" | "svg" | "webp" | "avif" | "png" | "jpg" | "jpeg" | "ico")
    ) {
        return Some("public, max-age=86400");
    }
    None
}

async fn serve_static_file_from_root(
    root: &Path,
    req_path: &str,
    default_index: Option<&str>,
    opts: StaticServeOptions,
    accept_encoding: Option<&str>,
    is_head: bool,
    server_tokens: bool,
) -> Response<RespBody> {
    let resolved = match resolve_safe_path(root, req_path) {
        Ok(p) => p,
        Err(PathError::BadEncoding) => {
            return text_response(StatusCode::BAD_REQUEST, "Bad Request", server_tokens);
        }
        Err(PathError::Forbidden) => {
            return text_response(StatusCode::FORBIDDEN, "Forbidden", server_tokens);
        }
    };

    let mut target_file = resolved.clone();
    let ends_with_slash = req_path.ends_with('/');
    let is_dir = tokio::fs::metadata(&resolved)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false);

    if is_dir || ends_with_slash {
        if let Some(idx) = default_index {
            target_file = resolved.join(idx);
        } else {
            return text_response(StatusCode::NOT_FOUND, "Not Found", server_tokens);
        }
    }

    match open_regular_file_negotiated(&target_file, root, opts.precompressed, accept_encoding).await {
        Ok(Some(opened)) => {
            let cc = cache_control_for_asset(req_path, &target_file, opts.immutable_assets);
            build_file_response(opened, cc, is_head, server_tokens)
        }
        Ok(None) => {
            let has_extension = Path::new(req_path).extension().is_some();

            // Clean URL resolution for Astro / Next.js static export / Nuxt generate / Tailwind multi-page sites
            if opts.clean_urls && !ends_with_slash && !has_extension {
                let mut html_candidate = resolved.as_os_str().to_os_string();
                html_candidate.push(".html");
                let html_path = std::path::PathBuf::from(html_candidate);
                if let Ok(Some(opened)) =
                    open_regular_file_negotiated(&html_path, root, opts.precompressed, accept_encoding).await
                {
                    let cc = cache_control_for_asset(req_path, &html_path, opts.immutable_assets);
                    return build_file_response(opened, cc, is_head, server_tokens);
                }
                if let Some(idx) = default_index {
                    let dir_idx = resolved.join(idx);
                    if let Ok(Some(opened)) =
                        open_regular_file_negotiated(&dir_idx, root, opts.precompressed, accept_encoding).await
                    {
                        let cc = cache_control_for_asset(req_path, &dir_idx, opts.immutable_assets);
                        return build_file_response(opened, cc, is_head, server_tokens);
                    }
                }
            }

            if opts.spa_fallback {
                if let Some(idx) = default_index {
                    if !has_extension {
                        let fallback_path = root.join(idx);
                        if let Ok(Some(opened)) = open_regular_file_negotiated(
                            &fallback_path,
                            root,
                            opts.precompressed,
                            accept_encoding,
                        )
                        .await
                        {
                            let cc = cache_control_for_asset(req_path, &fallback_path, opts.immutable_assets);
                            return build_file_response(opened, cc, is_head, server_tokens);
                        }
                    }
                }
            }
            text_response(StatusCode::NOT_FOUND, "Not Found", server_tokens)
        }
        Err(status) => text_response(status, "Forbidden", server_tokens),
    }
}

struct OpenedStaticFile {
    file: tokio::fs::File,
    len: u64,
    mime: String,
    content_encoding: Option<&'static str>,
    vary_accept_encoding: bool,
}

async fn open_regular_file_negotiated(
    target_path: &Path,
    root: &Path,
    precompressed: bool,
    accept_encoding: Option<&str>,
) -> Result<Option<OpenedStaticFile>, StatusCode> {
    let compressible = is_compressible_asset(target_path);
    let mime = resolve_web_mime(target_path);

    if precompressed && compressible {
        if accept_encoding_supports(accept_encoding, "br") {
            let mut br_os = target_path.as_os_str().to_os_string();
            br_os.push(".br");
            let br_path = std::path::PathBuf::from(br_os);
            if let Ok(Some((file, len))) = open_contained_file(&br_path, root).await {
                return Ok(Some(OpenedStaticFile {
                    file,
                    len,
                    mime,
                    content_encoding: Some("br"),
                    vary_accept_encoding: true,
                }));
            }
        }
        if accept_encoding_supports(accept_encoding, "gzip") {
            let mut gz_os = target_path.as_os_str().to_os_string();
            gz_os.push(".gz");
            let gz_path = std::path::PathBuf::from(gz_os);
            if let Ok(Some((file, len))) = open_contained_file(&gz_path, root).await {
                return Ok(Some(OpenedStaticFile {
                    file,
                    len,
                    mime,
                    content_encoding: Some("gzip"),
                    vary_accept_encoding: true,
                }));
            }
        }
    }

    match open_contained_file(target_path, root).await? {
        Some((file, len)) => Ok(Some(OpenedStaticFile {
            file,
            len,
            mime,
            content_encoding: None,
            vary_accept_encoding: precompressed && compressible,
        })),
        None => Ok(None),
    }
}

async fn open_contained_file(
    target_path: &Path,
    root: &Path,
) -> Result<Option<(tokio::fs::File, u64)>, StatusCode> {
    if !target_path.is_file() {
        return Ok(None);
    }
    // Re-check canonical containment right before opening file descriptor.
    let canon_root = std::fs::canonicalize(root).map_err(|_| StatusCode::FORBIDDEN)?;
    let canon_file = std::fs::canonicalize(target_path).map_err(|_| StatusCode::FORBIDDEN)?;
    if !canon_file.starts_with(&canon_root) {
        return Err(StatusCode::FORBIDDEN);
    }

    let file = tokio::fs::File::open(&canon_file)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let meta = file.metadata().await.map_err(|_| StatusCode::NOT_FOUND)?;
    if !meta.is_file() {
        return Ok(None);
    }
    Ok(Some((file, meta.len())))
}

fn build_file_response(
    opened: OpenedStaticFile,
    cache_control: Option<&'static str>,
    is_head: bool,
    server_tokens: bool,
) -> Response<RespBody> {
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, &opened.mime)
        .header(header::CONTENT_LENGTH, opened.len.to_string())
        .header(HeaderName::from_static("x-content-type-options"), "nosniff");
    if let Some(enc) = opened.content_encoding {
        builder = builder.header(header::CONTENT_ENCODING, enc);
    }
    if opened.vary_accept_encoding {
        builder = builder.header(header::VARY, "Accept-Encoding");
    }
    if let Some(cc) = cache_control {
        builder = builder.header(header::CACHE_CONTROL, cc);
    }
    if server_tokens {
        builder = builder.header(header::SERVER, "FerrousX");
    }

    let body = if is_head {
        empty_body()
    } else {
        FileStreamBody::new(opened.file).boxed()
    };

    builder
        .body(body)
        .unwrap_or_else(|_| text_response(StatusCode::INTERNAL_SERVER_ERROR, "response build error", false))
}

#[allow(clippy::too_many_arguments)]
async fn handle_reverse_proxy(
    req: Request<Incoming>,
    peer_addr: SocketAddr,
    is_tls: bool,
    host: &str,
    upstream: &UpstreamOrigin,
    pass_host_header: bool,
    forwarded_headers: bool,
    bundle: &LoadedBundle,
    http_client: &HttpLegacyClient,
) -> Response<RespBody> {
    let server_tokens = bundle.master.server_tokens;
    let is_upgrade = is_websocket_upgrade(req.headers());
    if is_upgrade {
        return handle_websocket_upgrade(
            req,
            peer_addr,
            is_tls,
            host,
            upstream,
            pass_host_header,
            forwarded_headers,
            bundle,
        )
        .await;
    }

    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    let upstream_uri_str = format!("{}://{}{path_and_query}", upstream.scheme, upstream.authority);
    let Ok(upstream_uri) = upstream_uri_str.parse::<hyper::Uri>() else {
        return text_response(StatusCode::BAD_REQUEST, "Invalid target URI", server_tokens);
    };

    let (mut parts, incoming) = req.into_parts();
    parts.uri = upstream_uri;
    strip_hop_by_hop_headers(&mut parts.headers);
    parts.headers.remove(header::HOST);

    let host_hdr = if pass_host_header {
        host.to_string()
    } else {
        upstream.authority.clone()
    };
    if let Ok(hv) = HeaderValue::from_str(&host_hdr) {
        parts.headers.insert(header::HOST, hv);
    }

    if forwarded_headers {
        apply_forwarded_headers(&mut parts.headers, peer_addr, is_tls, host);
    }

    let limited_body = Limited::new(incoming, bundle.master.max_body_bytes);
    let out_req = Request::from_parts(parts, limited_body);

    let fut = http_client.request(out_req);
    let upstream_res = match tokio::time::timeout(bundle.master.request_timeout, fut).await {
        Ok(Ok(res)) => res,
        Ok(Err(_)) => {
            return text_response(StatusCode::BAD_GATEWAY, "Bad Gateway", server_tokens);
        }
        Err(_) => {
            return text_response(StatusCode::GATEWAY_TIMEOUT, "Gateway Timeout", server_tokens);
        }
    };

    let (mut res_parts, res_body) = upstream_res.into_parts();
    strip_hop_by_hop_headers(&mut res_parts.headers);
    if server_tokens {
        res_parts
            .headers
            .insert(header::SERVER, HeaderValue::from_static("FerrousX"));
    } else {
        res_parts.headers.remove(header::SERVER);
    }
    Response::from_parts(res_parts, res_body.boxed())
}

#[allow(clippy::too_many_arguments)]
async fn handle_websocket_upgrade(
    client_req: Request<Incoming>,
    peer_addr: SocketAddr,
    is_tls: bool,
    host: &str,
    upstream: &UpstreamOrigin,
    pass_host_header: bool,
    forwarded_headers: bool,
    bundle: &LoadedBundle,
) -> Response<RespBody> {
    let server_tokens = bundle.master.server_tokens;
    let stream = match tokio::time::timeout(
        bundle.master.upstream_connect_timeout,
        TcpStream::connect(&upstream.connect_addr),
    )
    .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(_)) => {
            return text_response(StatusCode::BAD_GATEWAY, "Bad Gateway", server_tokens);
        }
        Err(_) => {
            return text_response(StatusCode::GATEWAY_TIMEOUT, "Gateway Timeout", server_tokens);
        }
    };

    let io = hyper_util::rt::TokioIo::new(stream);
    let Ok((mut sender, conn)) = hyper::client::conn::http1::handshake(io).await else {
        return text_response(StatusCode::BAD_GATEWAY, "Bad Gateway", server_tokens);
    };

    tokio::spawn(async move {
        let _ = conn.with_upgrades().await;
    });

    let path_and_query = client_req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    let Ok(up_uri) = path_and_query.parse::<hyper::Uri>() else {
        return text_response(StatusCode::BAD_REQUEST, "Bad Request", server_tokens);
    };

    let mut up_builder = Request::builder().method(client_req.method().clone()).uri(up_uri);
    for (name, value) in client_req.headers() {
        if name != header::HOST {
            up_builder = up_builder.header(name, value);
        }
    }

    let host_hdr = if pass_host_header {
        host.to_string()
    } else {
        upstream.authority.clone()
    };
    if let Ok(hv) = HeaderValue::from_str(&host_hdr) {
        up_builder = up_builder.header(header::HOST, hv);
    }

    let Ok(mut up_req) = up_builder.body(Empty::<Bytes>::new()) else {
        return text_response(StatusCode::INTERNAL_SERVER_ERROR, "Internal Error", server_tokens);
    };
    if forwarded_headers {
        apply_forwarded_headers(up_req.headers_mut(), peer_addr, is_tls, host);
    }

    let up_res = match tokio::time::timeout(
        bundle.master.request_timeout,
        sender.send_request(up_req),
    )
    .await
    {
        Ok(Ok(r)) => r,
        Ok(Err(_)) => {
            return text_response(StatusCode::BAD_GATEWAY, "Bad Gateway", server_tokens);
        }
        Err(_) => {
            return text_response(StatusCode::GATEWAY_TIMEOUT, "Gateway Timeout", server_tokens);
        }
    };

    if up_res.status() != StatusCode::SWITCHING_PROTOCOLS {
        let (parts, body) = up_res.into_parts();
        return Response::from_parts(parts, body.boxed());
    }

    let mut client_res_builder = Response::builder().status(StatusCode::SWITCHING_PROTOCOLS);
    for (name, value) in up_res.headers() {
        client_res_builder = client_res_builder.header(name, value);
    }
    let Ok(client_res) = client_res_builder.body(empty_body()) else {
        return text_response(StatusCode::INTERNAL_SERVER_ERROR, "Internal Error", server_tokens);
    };

    let client_up = hyper::upgrade::on(client_req);
    let upstream_up = hyper::upgrade::on(up_res);
    tokio::spawn(async move {
        if let Ok((client_upgraded, upstream_upgraded)) =
            tokio::try_join!(client_up, upstream_up)
        {
            let mut c_io = hyper_util::rt::TokioIo::new(client_upgraded);
            let mut u_io = hyper_util::rt::TokioIo::new(upstream_upgraded);
            let _ = tokio::io::copy_bidirectional(&mut c_io, &mut u_io).await;
        }
    });

    client_res
}

#[allow(clippy::too_many_arguments)]
async fn handle_php_fpm(
    req: Request<Incoming>,
    peer_addr: SocketAddr,
    is_tls: bool,
    host: &str,
    site: &ValidatedSite,
    fastcgi_socket: &Path,
    default_index: &str,
    bundle: &LoadedBundle,
) -> Response<RespBody> {
    let server_tokens = bundle.master.server_tokens;
    let req_path = req.uri().path().to_string();

    // Block dotfiles like .htaccess / .env
    if req_path.split('/').any(|seg| seg.starts_with(".ht") || seg == ".env") {
        return text_response(StatusCode::FORBIDDEN, "Forbidden", server_tokens);
    }

    let resolved = match resolve_safe_path(&site.root, &req_path) {
        Ok(p) => p,
        Err(PathError::BadEncoding) => {
            return text_response(StatusCode::BAD_REQUEST, "Bad Request", server_tokens);
        }
        Err(PathError::Forbidden) => {
            return text_response(StatusCode::FORBIDDEN, "Forbidden", server_tokens);
        }
    };

    // Serve existing non-PHP static assets directly without invoking PHP-FPM.
    let is_php_ext = resolved
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("php"));
    if resolved.is_file() && !is_php_ext {
        let is_head = *req.method() == Method::HEAD;
        let accept_encoding = req
            .headers()
            .get(header::ACCEPT_ENCODING)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        return serve_static_file_from_root(
            &site.root,
            &req_path,
            None,
            StaticServeOptions::static_assets_only(),
            accept_encoding.as_deref(),
            is_head,
            server_tokens,
        )
        .await;
    }

    // Choose script filename: direct .php file if present, else fallback front-controller index.php.
    let (script_path, script_name) = if is_php_ext && resolved.is_file() {
        (resolved, req_path.clone())
    } else {
        let idx_path = site.root.join(default_index);
        if !idx_path.is_file() {
            return text_response(StatusCode::NOT_FOUND, "Not Found", server_tokens);
        }
        (idx_path, format!("/{default_index}"))
    };

    let (parts, incoming) = req.into_parts();
    let collected = match Limited::new(incoming, bundle.master.max_body_bytes)
        .collect()
        .await
    {
        Ok(c) => c.to_bytes(),
        Err(_) => {
            return text_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "Payload Too Large",
                server_tokens,
            );
        }
    };

    let unix_stream = match tokio::time::timeout(
        bundle.master.upstream_connect_timeout,
        UnixStream::connect(fastcgi_socket),
    )
    .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(_)) => {
            return text_response(StatusCode::BAD_GATEWAY, "PHP-FPM Unavailable", server_tokens);
        }
        Err(_) => {
            return text_response(
                StatusCode::GATEWAY_TIMEOUT,
                "PHP-FPM Connect Timeout",
                server_tokens,
            );
        }
    };

    let method_str = parts.method.as_str().to_string();
    let request_uri = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/")
        .to_string();
    let query_string = parts.uri.query().unwrap_or("").to_string();
    let script_filename = script_path.to_string_lossy().to_string();
    let doc_root = site.root.to_string_lossy().to_string();
    let content_type = parts
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let remote_ip = peer_addr.ip().to_string();
    let remote_port = peer_addr.port();
    let server_port = if is_tls {
        bundle.master.bind_https.port()
    } else {
        bundle.master.bind_http.port()
    };

    let mut params = FastcgiParams::default()
        .request_method(&method_str)
        .script_name(&script_name)
        .script_filename(&script_filename)
        .request_uri(&request_uri)
        .document_uri(&req_path)
        .remote_addr(&remote_ip)
        .remote_port(remote_port)
        .server_addr("127.0.0.1")
        .server_port(server_port)
        .server_name(host)
        .content_type(&content_type)
        .content_length(collected.len());

    params.insert(Cow::Borrowed("QUERY_STRING"), Cow::Owned(query_string));
    params.insert(Cow::Borrowed("DOCUMENT_ROOT"), Cow::Owned(doc_root));
    if is_tls {
        params.insert(Cow::Borrowed("HTTPS"), Cow::Borrowed("on"));
    }

    for (name, value) in &parts.headers {
        if let Ok(val_str) = value.to_str() {
            let fcgi_key = format!(
                "HTTP_{}",
                name.as_str().to_ascii_uppercase().replace('-', "_")
            );
            params.insert(Cow::Owned(fcgi_key), Cow::Owned(val_str.to_string()));
        }
    }

    let mut body_cursor = Cursor::new(collected.as_ref());
    let fcgi_client = FastcgiClient::new(unix_stream);
    let fcgi_res = match tokio::time::timeout(
        bundle.master.request_timeout,
        fcgi_client.execute_once(FastcgiRequest::new(params, &mut body_cursor)),
    )
    .await
    {
        Ok(Ok(out)) => out,
        Ok(Err(_)) => {
            return text_response(StatusCode::BAD_GATEWAY, "FastCGI Error", server_tokens);
        }
        Err(_) => {
            return text_response(StatusCode::GATEWAY_TIMEOUT, "FastCGI Timeout", server_tokens);
        }
    };

    let raw_stdout = fcgi_res.stdout.unwrap_or_default();
    parse_cgi_output(&raw_stdout, server_tokens)
}

fn parse_cgi_output(raw: &[u8], server_tokens: bool) -> Response<RespBody> {
    let (hdr_bytes, body_bytes) = if let Some(pos) = find_subsequence(raw, b"\r\n\r\n") {
        (&raw[..pos], &raw[pos + 4..])
    } else if let Some(pos) = find_subsequence(raw, b"\n\n") {
        (&raw[..pos], &raw[pos + 2..])
    } else {
        (&[][..], raw)
    };

    let mut status = StatusCode::OK;
    let mut builder = Response::builder();
    let hdr_text = String::from_utf8_lossy(hdr_bytes);
    let mut has_content_type = false;

    for line in hdr_text.lines() {
        let trimmed = line.trim_end_matches('\r');
        let Some((name, value)) = trimmed.split_once(':') else {
            continue;
        };
        let clean_name = name.trim();
        let clean_val = value.trim();
        if clean_name.eq_ignore_ascii_case("status") {
            if let Some(code_str) = clean_val.split_whitespace().next() {
                if let Ok(code) = code_str.parse::<u16>() {
                    if let Ok(parsed) = StatusCode::from_u16(code) {
                        status = parsed;
                    }
                }
            }
            continue;
        }
        if clean_name.eq_ignore_ascii_case("content-type") {
            has_content_type = true;
        }
        if let (Ok(hn), Ok(hv)) = (
            HeaderName::from_bytes(clean_name.as_bytes()),
            HeaderValue::from_str(clean_val),
        ) {
            builder = builder.header(hn, hv);
        }
    }

    builder = builder.status(status);
    if !has_content_type {
        builder = builder.header(header::CONTENT_TYPE, "text/html; charset=utf-8");
    }
    if server_tokens {
        builder = builder.header(header::SERVER, "FerrousX");
    }

    let body = Full::new(Bytes::copy_from_slice(body_bytes))
        .map_err(|never| match never {})
        .boxed();
    builder
        .body(body)
        .unwrap_or_else(|_| text_response(StatusCode::INTERNAL_SERVER_ERROR, "CGI parse error", false))
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn is_websocket_upgrade(headers: &HeaderMap) -> bool {
    let has_upgrade_conn = headers
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .any(|s| s.split(',').any(|tok| tok.trim().eq_ignore_ascii_case("upgrade")));
    let has_ws_upgrade = headers
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("websocket"));
    has_upgrade_conn && has_ws_upgrade
}

fn strip_hop_by_hop_headers(headers: &mut HeaderMap) {
    const HOP_HEADERS: &[&str] = &[
        "connection",
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "te",
        "trailers",
        "transfer-encoding",
        "upgrade",
    ];
    let connection_extras: Vec<String> = headers
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|s| s.split(','))
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    for h in HOP_HEADERS {
        headers.remove(*h);
    }
    for extra in connection_extras {
        if let Ok(name) = HeaderName::from_bytes(extra.as_bytes()) {
            headers.remove(name);
        }
    }
}

fn apply_forwarded_headers(
    headers: &mut HeaderMap,
    peer_addr: SocketAddr,
    is_tls: bool,
    host: &str,
) {
    let ip_str = peer_addr.ip().to_string();
    if let Ok(val) = HeaderValue::from_str(&ip_str) {
        headers.insert(HeaderName::from_static("x-real-ip"), val.clone());
        let existing = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let combined = match existing {
            Some(prev) => format!("{prev}, {ip_str}"),
            None => ip_str,
        };
        if let Ok(xff) = HeaderValue::from_str(&combined) {
            headers.insert(HeaderName::from_static("x-forwarded-for"), xff);
        }
    }
    let proto = if is_tls { "https" } else { "http" };
    headers.insert(
        HeaderName::from_static("x-forwarded-proto"),
        HeaderValue::from_static(proto),
    );
    if !host.is_empty() {
        if let Ok(xfh) = HeaderValue::from_str(host) {
            headers.insert(HeaderName::from_static("x-forwarded-host"), xfh);
        }
    }
    let port = if is_tls { "443" } else { "80" };
    headers.insert(
        HeaderName::from_static("x-forwarded-port"),
        HeaderValue::from_static(port),
    );
}

#[must_use]
pub fn empty_body() -> RespBody {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}

#[must_use]
pub fn text_response(status: StatusCode, msg: &str, server_tokens: bool) -> Response<RespBody> {
    let body = Full::new(Bytes::from(msg.to_string()))
        .map_err(|never| match never {})
        .boxed();
    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
    if server_tokens {
        builder = builder.header(header::SERVER, "FerrousX");
    }
    builder
        .body(body)
        .unwrap_or_else(|_| Response::new(empty_body()))
}

struct FileStreamBody {
    file: tokio::fs::File,
    buf: Vec<u8>,
}

impl FileStreamBody {
    fn new(file: tokio::fs::File) -> Self {
        Self {
            file,
            buf: vec![0u8; FILE_CHUNK_SIZE],
        }
    }
}

impl Body for FileStreamBody {
    type Data = Bytes;
    type Error = hyper::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = &mut *self;
        let mut read_buf = ReadBuf::new(&mut this.buf);
        match Pin::new(&mut this.file).poll_read(cx, &mut read_buf) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(())) => {
                let filled = read_buf.filled();
                if filled.is_empty() {
                    Poll::Ready(None)
                } else {
                    Poll::Ready(Some(Ok(Frame::data(Bytes::copy_from_slice(filled)))))
                }
            }
            Poll::Ready(Err(_)) => Poll::Ready(None),
        }
    }
}
