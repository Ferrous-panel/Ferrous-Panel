use crate::config::load_bundle;
use crate::error::ServerError;
use crate::handlers::{dispatch_request, AppRuntime};
use crate::ipc::{
    bind_secure_unix_listener, load_or_init_admin_token, run_admin_ipc_loop, BruteForceGuard,
    ReloadFn,
};
use crate::tls::{build_tls_server_config, DynamicSniResolver};
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo, TokioTimer};
use hyper_util::server::conn::auto;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio_rustls::TlsAcceptor;

/// Running FerrousX HTTP/1.1 + HTTP/2 + TLS (SNI/ALPN) + Admin UDS IPC server instance.
#[derive(Debug)]
pub struct FerrousxServer {
    master_config_path: PathBuf,
    http_addr: SocketAddr,
    https_addr: SocketAddr,
    ipc_socket_path: Option<PathBuf>,
    runtime: Arc<AppRuntime>,
    sni_resolver: Arc<DynamicSniResolver>,
    shutdown_tx: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}

fn perform_hot_reload(
    master_config_path: &Path,
    runtime: &AppRuntime,
    sni_resolver: &DynamicSniResolver,
) -> Result<(u64, usize), ServerError> {
    let next = load_bundle(master_config_path)?;
    let count = next.sites.len();
    sni_resolver.swap(next.sni_certs.clone())?;
    runtime
        .swap_bundle(next)
        .map_err(|msg| ServerError::Config(msg.to_string()))?;
    let epoch = runtime.metrics.bump_epoch();
    tracing::info!(
        epoch,
        sites = count,
        "ferrousx configuration hot-reloaded (Zero-Downtime RCU)"
    );
    Ok((epoch, count))
}

impl FerrousxServer {
    /// Loads and validates config, builds TLS SNI/ALPN acceptor, binds HTTP+HTTPS sockets,
    /// and starts the Admin UDS IPC listener when `control_socket` is configured.
    pub async fn start(master_config_path: &Path) -> Result<Self, ServerError> {
        Self::start_with_ipc(master_config_path, None).await
    }

    /// Starts FerrousX with an optional explicit UDS Admin IPC socket path override
    /// (for example `/run/ferrousx/panel-admin.sock` in production daemon mode or a temp socket in tests).
    pub async fn start_with_ipc(
        master_config_path: &Path,
        ipc_socket_override: Option<PathBuf>,
    ) -> Result<Self, ServerError> {
        let initial = load_bundle(master_config_path)?;
        let bind_http = initial.master.bind_http;
        let bind_https = initial.master.bind_https;
        let effective_ipc_socket = ipc_socket_override.or_else(|| initial.master.control_socket.clone());
        let token_file = initial.master.admin_token_file.clone();
        let allow_read_gid = initial.master.allow_read_group_gid;
        let max_failed_attempts = initial.master.max_failed_attempts;
        let brute_force_lockout_secs = initial.master.brute_force_lockout_secs;

        let sni_resolver = Arc::new(DynamicSniResolver::new(initial.sni_certs.clone()));
        let tls_cfg = build_tls_server_config(Arc::clone(&sni_resolver))?;
        let tls_acceptor = TlsAcceptor::from(tls_cfg);

        let runtime = Arc::new(AppRuntime::new(initial));

        let http_listener = TcpListener::bind(bind_http).await?;
        let https_listener = TcpListener::bind(bind_https).await?;
        let http_addr = http_listener.local_addr()?;
        let https_addr = https_listener.local_addr()?;

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let mut tasks = Vec::with_capacity(3);
        tasks.push(tokio::spawn(run_http_accept_loop(
            http_listener,
            Arc::clone(&runtime),
            shutdown_rx.clone(),
        )));
        tasks.push(tokio::spawn(run_https_accept_loop(
            https_listener,
            tls_acceptor,
            Arc::clone(&runtime),
            shutdown_rx.clone(),
        )));

        let mut active_ipc_socket = None;
        if let Some(socket_path) = effective_ipc_socket {
            let sock_mode = if allow_read_gid.is_some() {
                0o660
            } else {
                0o600
            };
            let listener = bind_secure_unix_listener(&socket_path, sock_mode)?;
            let fallback_token = Arc::new(
                load_or_init_admin_token(&token_file).unwrap_or_default(),
            );
            let brute_guard = Arc::new(BruteForceGuard::new(
                max_failed_attempts,
                brute_force_lockout_secs,
            ));

            let cfg_path_owned = master_config_path.to_path_buf();
            let rt_reload = Arc::clone(&runtime);
            let sni_reload = Arc::clone(&sni_resolver);
            let reload_fn: ReloadFn = Arc::new(move || {
                perform_hot_reload(&cfg_path_owned, &rt_reload, &sni_reload)
            });

            tracing::info!(
                socket = %socket_path.display(),
                mode = format!("{sock_mode:#o}"),
                "ferrousx admin IPC control socket ready (SO_PEERCRED + 256-bit token)"
            );
            tasks.push(tokio::spawn(run_admin_ipc_loop(
                listener,
                socket_path.clone(),
                Arc::clone(&runtime),
                fallback_token,
                brute_guard,
                reload_fn,
                shutdown_rx,
            )));
            active_ipc_socket = Some(socket_path);
        }

        tracing::info!(
            http = %http_addr,
            https = %https_addr,
            "ferrousx listeners ready"
        );

        Ok(Self {
            master_config_path: master_config_path.to_path_buf(),
            http_addr,
            https_addr,
            ipc_socket_path: active_ipc_socket,
            runtime,
            sni_resolver,
            shutdown_tx,
            tasks,
        })
    }

    #[must_use]
    pub const fn http_addr(&self) -> SocketAddr {
        self.http_addr
    }

    #[must_use]
    pub const fn https_addr(&self) -> SocketAddr {
        self.https_addr
    }

    #[must_use]
    pub fn ipc_socket_path(&self) -> Option<&Path> {
        self.ipc_socket_path.as_deref()
    }

    #[must_use]
    pub fn config_epoch(&self) -> u64 {
        self.runtime.metrics.epoch()
    }

    #[must_use]
    pub fn site_count(&self) -> usize {
        self.runtime
            .current_bundle()
            .map(|b| b.sites.len())
            .unwrap_or(0)
    }

    /// Atomically reloads `ferrousx.toml`, `sites/*.toml`, and SNI TLS certs without closing active sockets.
    ///
    /// If validation of the new config fails, keeps the previous valid config live.
    pub fn reload(&self) -> Result<usize, ServerError> {
        let (_epoch, count) =
            perform_hot_reload(&self.master_config_path, &self.runtime, &self.sni_resolver)?;
        Ok(count)
    }

    /// Signals listener accept loops (HTTP, HTTPS, and Admin IPC UDS) to stop cleanly.
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(true);
        for task in self.tasks {
            let _ = task.await;
        }
        if let Some(sock) = self.ipc_socket_path {
            let _ = std::fs::remove_file(sock);
        }
    }
}

struct ActiveConnGuard {
    runtime: Arc<AppRuntime>,
}

impl ActiveConnGuard {
    fn new(runtime: Arc<AppRuntime>) -> Self {
        runtime.metrics.conn_open();
        Self { runtime }
    }
}

impl Drop for ActiveConnGuard {
    fn drop(&mut self) {
        self.runtime.metrics.conn_close();
    }
}

async fn run_http_accept_loop(
    listener: TcpListener,
    runtime: Arc<AppRuntime>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    break;
                }
            }
            accept_res = listener.accept() => {
                let Ok((stream, peer_addr)) = accept_res else {
                    continue;
                };
                let _ = stream.set_nodelay(true);
                let rt = Arc::clone(&runtime);
                tokio::spawn(async move {
                    let _conn = ActiveConnGuard::new(Arc::clone(&rt));
                    serve_plain_connection(stream, peer_addr, rt).await;
                });
            }
        }
    }
}

async fn run_https_accept_loop(
    listener: TcpListener,
    tls_acceptor: TlsAcceptor,
    runtime: Arc<AppRuntime>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    break;
                }
            }
            accept_res = listener.accept() => {
                let Ok((stream, peer_addr)) = accept_res else {
                    continue;
                };
                let _ = stream.set_nodelay(true);
                let acceptor = tls_acceptor.clone();
                let rt = Arc::clone(&runtime);
                tokio::spawn(async move {
                    let _conn = ActiveConnGuard::new(Arc::clone(&rt));
                    serve_tls_connection(stream, peer_addr, acceptor, rt).await;
                });
            }
        }
    }
}

fn build_conn_builder(runtime: &AppRuntime) -> auto::Builder<TokioExecutor> {
    let (header_timeout, max_buf) = runtime
        .current_bundle()
        .map(|b| {
            (
                b.master.header_read_timeout,
                b.master.max_header_bytes.max(8192),
            )
        })
        .unwrap_or((Duration::from_secs(10), 16 * 1024));

    let mut builder = auto::Builder::new(TokioExecutor::new());
    builder
        .http1()
        .timer(TokioTimer::new())
        .header_read_timeout(header_timeout)
        .max_buf_size(max_buf);
    builder.http2().timer(TokioTimer::new());
    builder
}

async fn serve_plain_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    runtime: Arc<AppRuntime>,
) {
    let builder = build_conn_builder(&runtime);
    let io = TokioIo::new(stream);
    let svc = service_fn(move |req| {
        let rt = Arc::clone(&runtime);
        dispatch_request(req, peer_addr, false, rt)
    });
    if let Err(err) = builder.serve_connection_with_upgrades(io, svc).await {
        tracing::debug!(peer = %peer_addr, error = %err, "plain HTTP connection ended");
    }
}

async fn serve_tls_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    acceptor: TlsAcceptor,
    runtime: Arc<AppRuntime>,
) {
    let handshake_timeout = runtime
        .current_bundle()
        .map(|b| b.master.header_read_timeout)
        .unwrap_or(Duration::from_secs(10));

    let tls_stream = match tokio::time::timeout(handshake_timeout, acceptor.accept(stream)).await {
        Ok(Ok(s)) => s,
        Ok(Err(err)) => {
            tracing::debug!(peer = %peer_addr, error = %err, "TLS handshake failed");
            return;
        }
        Err(_) => {
            tracing::debug!(peer = %peer_addr, "TLS handshake timed out");
            return;
        }
    };

    let builder = build_conn_builder(&runtime);
    let io = TokioIo::new(tls_stream);
    let svc = service_fn(move |req| {
        let rt = Arc::clone(&runtime);
        dispatch_request(req, peer_addr, true, rt)
    });
    if let Err(err) = builder.serve_connection_with_upgrades(io, svc).await {
        tracing::debug!(peer = %peer_addr, error = %err, "TLS HTTP connection ended");
    }
}
