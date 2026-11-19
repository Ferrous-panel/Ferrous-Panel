use crate::config::{LoadedBundle, SiteHandler, UpstreamOrigin, ValidatedSite};
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
use std::io::Cursor;
use std::net::SocketAddr;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, ReadBuf};
use tokio::net::{TcpStream, UnixStream};

pub type RespBody = BoxBody<Bytes, hyper::Error>;
pub type HttpLegacyClient = Client<HttpConnector, Limited<Incoming>>;

const FILE_CHUNK_SIZE: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct AppRuntime {
    pub bundle: Arc<RwLock<Arc<LoadedBundle>>>,
    pub http_client: HttpLegacyClient,
    pub metrics: Arc<EngineRuntimeMetrics>,
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
            false,
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
                serve_static_file_from_root(
                    &site.root,
                    &req_path,
                    Some(index_file.as_str()),
                    *spa_fallback,
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

async fn serve_static_file_from_root(
    root: &Path,
    req_path: &str,
    default_index: Option<&str>,
    spa_fallback: bool,
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

    match open_regular_file(&target_file, root).await {
        Ok(Some((file, len, mime))) => build_file_response(file, len, &mime, is_head, server_tokens),
        Ok(None) => {
            if spa_fallback {
                if let Some(idx) = default_index {
                    let has_extension = Path::new(req_path).extension().is_some();
                    if !has_extension {
                        let fallback_path = root.join(idx);
                        if let Ok(Some((file, len, mime))) =
                            open_regular_file(&fallback_path, root).await
                        {
                            return build_file_response(file, len, &mime, is_head, server_tokens);
                        }
                    }
                }
            }
            text_response(StatusCode::NOT_FOUND, "Not Found", server_tokens)
        }
        Err(status) => text_response(status, "Forbidden", server_tokens),
    }
}

async fn open_regular_file(
    target_path: &Path,
    root: &Path,
) -> Result<Option<(tokio::fs::File, u64, String)>, StatusCode> {
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
    let mime = mime_guess::from_path(&canon_file)
        .first_or_octet_stream()
        .to_string();
    Ok(Some((file, meta.len(), mime)))
}

fn build_file_response(
    file: tokio::fs::File,
    len: u64,
    mime: &str,
    is_head: bool,
    server_tokens: bool,
) -> Response<RespBody> {
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CONTENT_LENGTH, len.to_string());
    if server_tokens {
        builder = builder.header(header::SERVER, "FerrousX");
    }

    let body = if is_head {
        empty_body()
    } else {
        FileStreamBody::new(file).boxed()
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
        apply_forwarded_headers(&mut parts.headers, peer_addr, is_tls);
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
        apply_forwarded_headers(up_req.headers_mut(), peer_addr, is_tls);
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
        if let (Ok(client_upgraded), Ok(upstream_upgraded)) =
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
        return serve_static_file_from_root(
            &site.root,
            &req_path,
            None,
            false,
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

fn apply_forwarded_headers(headers: &mut HeaderMap, peer_addr: SocketAddr, is_tls: bool) {
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
