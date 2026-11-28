#![forbid(unsafe_code)]

use actix_web::middleware::from_fn;
use actix_web::{web, App, HttpServer};
use anyhow::Context;
use clap::Parser;
use ferrous_panel::auth::middleware::security_headers_middleware;
use ferrous_panel::cli::{
    apply_env_file, run_fp_command, run_interactive_menu, FpCli, PanelCommand,
};
use ferrous_panel::config::Config;
use ferrous_panel::configure_app;
use ferrous_panel::state::AppState;
use ferrous_panel::system::validate::{is_reserved_website_port, DEFAULT_PANEL_PORT};
use ferrous_panel::webserver::WebServerManager;
use std::path::{Path, PathBuf};
use tracing_subscriber::EnvFilter;

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .try_init();
}

fn load_panel_rustls_config(
    cert_path: &Path,
    key_path: &Path,
) -> anyhow::Result<rustls::ServerConfig> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let cert_file = std::fs::File::open(cert_path)
        .with_context(|| format!("failed to open panel SSL certificate {}", cert_path.display()))?;
    let mut cert_reader = std::io::BufReader::new(cert_file);
    let certs: Vec<rustls::pki_types::CertificateDer<'static>> =
        rustls_pemfile::certs(&mut cert_reader)
            .collect::<Result<Vec<_>, _>>()
            .with_context(|| format!("failed to parse PEM certs from {}", cert_path.display()))?;
    if certs.is_empty() {
        anyhow::bail!("no X.509 certificates found in {}", cert_path.display());
    }

    let key_file = std::fs::File::open(key_path)
        .with_context(|| format!("failed to open panel SSL private key {}", key_path.display()))?;
    let mut key_reader = std::io::BufReader::new(key_file);
    let private_key = rustls_pemfile::private_key(&mut key_reader)
        .with_context(|| format!("failed to parse PEM private key from {}", key_path.display()))?
        .ok_or_else(|| anyhow::anyhow!("no private key found in {}", key_path.display()))?;

    let mut tls_cfg = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, private_key)
        .with_context(|| "invalid panel TLS certificate/private key pair")?;
    tls_cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(tls_cfg)
}

async fn run_server(mut config: Config) -> anyhow::Result<()> {
    if config.port == 0 || is_reserved_website_port(config.port) {
        tracing::warn!(
            configured_port = config.port,
            fallback_port = DEFAULT_PANEL_PORT,
            "ports 80 and 443 are reserved exclusively for websites (Nginx/FerrousX); falling back panel port to 8443"
        );
        config.port = DEFAULT_PANEL_PORT;
    }
    let panel_ssl_paths = config.resolved_panel_ssl_paths();
    let bind_addr = config.bind.clone();
    let bind_port = config.port;
    let pma_port = config.pma_port;
    let (state, initial_password) =
        AppState::init(config).context("failed to initialize ferrous-panel state")?;

    if let Some(secret) = initial_password {
        let cred_user = state
            .credentials
            .read()
            .map(|c| c.username.clone())
            .unwrap_or_else(|_| state.config.admin_username.clone());
        eprintln!("====================================================================");
        eprintln!("FerrousPanel FIRST-RUN INITIAL CREDENTIALS (shown ONCE ONLY):");
        eprintln!("  Username : {cred_user}");
        eprintln!("  Password : {}", secret.expose());
        eprintln!(
            "  Hash File: {} (mode 0600, Argon2id)",
            state.config.credentials_path().display()
        );
        eprintln!("====================================================================");
    }

    let shared_state = web::Data::new(state);
    tracing::info!(
        bind = %bind_addr,
        port = bind_port,
        pma_port = pma_port,
        "starting ferrous-panel control plane + built-in Pure-Rust FerrousAdmin (/phpadmin/)"
    );

    // Record initial real telemetry point and sample every 60s for real historical chart data.
    let _ = shared_state.record_telemetry_sample(true);
    let telemetry_state = shared_state.clone();
    actix_web::rt::spawn(async move {
        let mut ticker = actix_web::rt::time::interval(std::time::Duration::from_secs(60));
        // Skip first immediate tick since initial point was just recorded above.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            let sample_state = telemetry_state.clone();
            match web::block(move || sample_state.record_telemetry_sample(true)).await {
                Ok(Err(err)) => {
                    tracing::debug!(error = %err, "background telemetry sample skipped");
                }
                Err(err) => {
                    tracing::debug!(error = %err, "background telemetry worker skipped");
                }
                Ok(Ok(())) => {}
            }
        }
    });

    if shared_state.config.is_production()
        && Path::new("/var/lib/pgadmin/venv/bin/python3").is_file()
        && !ferrous_panel::pma_web::is_pgadmin_installation_healthy()
    {
        let pga_state = shared_state.clone();
        actix_web::rt::spawn(async move {
            tracing::info!("auto-repairing existing pgAdmin 4 installation in /var/lib/pgadmin");
            match ferrous_panel::pma_web::install_or_repair_official_pgadmin(
                pga_state.webserver.executor().as_ref(),
            )
            .await
            {
                Ok(st) => {
                    tracing::info!(
                        version = %st.version,
                        running = st.running,
                        "pgAdmin 4 installation auto-repaired and ready at /pgadmin/"
                    );
                }
                Err(err) => {
                    tracing::warn!(error = %err, "background pgAdmin 4 auto-repair did not complete");
                }
            }
        });
    }

    let pma_listener = if pma_port > 0 && pma_port != bind_port && !is_reserved_website_port(pma_port) {
        match std::net::TcpListener::bind(("127.0.0.1", pma_port)) {
            Ok(listener) => {
                tracing::info!(
                    pma_bind = "127.0.0.1",
                    pma_port = pma_port,
                    "Pure-Rust FerrousAdmin (/phpadmin/) loopback listener active (Zero PHP)"
                );
                Some(listener)
            }
            Err(err) => {
                tracing::warn!(
                    pma_port = pma_port,
                    error = %err,
                    "could not bind Pure-Rust FerrousAdmin loopback port 127.0.0.1:{pma_port}; /phpadmin/ remains available on main panel port {bind_port}"
                );
                None
            }
        }
    } else {
        None
    };

    let mut main_listener: Option<std::net::TcpListener> = None;
    let mut last_bind_err: Option<std::io::Error> = None;
    for attempt in 0..10 {
        match std::net::TcpListener::bind((bind_addr.as_str(), bind_port)) {
            Ok(listener) => {
                main_listener = Some(listener);
                break;
            }
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse && attempt < 9 => {
                tracing::warn!(
                    bind = %bind_addr,
                    port = bind_port,
                    attempt = attempt + 1,
                    "panel port briefly busy during restart; retrying bind"
                );
                last_bind_err = Some(err);
                actix_web::rt::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            Err(err) => {
                last_bind_err = Some(err);
                break;
            }
        }
    }
    let main_listener = match main_listener {
        Some(listener) => listener,
        None => {
            let err = last_bind_err.unwrap_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::AddrInUse, "port unavailable")
            });
            return Err(anyhow::Error::new(err))
                .with_context(|| format!("failed to bind {bind_addr}:{bind_port}"));
        }
    };

    let base_server = HttpServer::new(move || {
        let app_state = shared_state.clone();
        App::new()
            .wrap(actix_web::middleware::Compress::default())
            .wrap(from_fn(security_headers_middleware))
            .configure(move |cfg| configure_app(cfg, app_state))
    })
    .shutdown_timeout(5);

    let mut server = match panel_ssl_paths {
        Some((cert_path, key_path)) => match load_panel_rustls_config(&cert_path, &key_path) {
            Ok(tls_cfg) => {
                tracing::info!(
                    bind = %bind_addr,
                    port = bind_port,
                    cert = %cert_path.display(),
                    "native Rustls HTTPS enabled on panel port"
                );
                base_server
                    .listen_rustls_0_23(main_listener, tls_cfg)
                    .with_context(|| {
                        format!("failed to attach HTTPS listener on {bind_addr}:{bind_port}")
                    })?
            }
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    cert = %cert_path.display(),
                    "panel TLS certificate could not be loaded; falling back to HTTP on {bind_addr}:{bind_port}"
                );
                base_server
                    .listen(main_listener)
                    .with_context(|| format!("failed to attach listener on {bind_addr}:{bind_port}"))?
            }
        },
        None => base_server
            .listen(main_listener)
            .with_context(|| format!("failed to attach listener on {bind_addr}:{bind_port}"))?,
    };

    if let Some(listener) = pma_listener {
        server = server
            .listen(listener)
            .with_context(|| format!("failed to attach Pure-Rust FerrousAdmin listener on 127.0.0.1:{pma_port}"))?;
    }

    server
        .run()
        .await
        .context("http server terminated unexpectedly")?;

    Ok(())
}

fn invoked_as_fp_cli() -> bool {
    let Some(arg0) = std::env::args_os().next() else {
        return false;
    };
    let name = Path::new(&arg0)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    matches!(name, "fp" | "ferrous")
}

fn main() -> anyhow::Result<()> {
    let rt = actix_web::rt::Runtime::new().context("failed to start Actix runtime")?;

    if invoked_as_fp_cli() {
        let fp_cli = FpCli::parse();
        let mut cfg = Config::for_test(fp_cli.data_dir, fp_cli.wwwroot);
        cfg.admin_username = fp_cli.admin_username;
        apply_env_file(&mut cfg, &fp_cli.env_file);
        return match fp_cli.command {
            Some(cmd) => rt.block_on(run_fp_command(cmd, &cfg, &fp_cli.env_file)),
            None => rt.block_on(run_interactive_menu(&cfg, &fp_cli.env_file)),
        };
    }

    init_tracing();
    let mut config = Config::parse();
    ferrous_panel::cli::apply_default_env_files(&mut config);
    if config.reload_webserver {
        let hook_exe = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("/usr/local/bin/ferrous-panel"));
        let manager = WebServerManager::open_system(&config.db_path(), hook_exe)
            .context("failed to open webserver manager")?;
        return rt
            .block_on(manager.reload_configured_engine())
            .context("failed to reload configured frontend web server engine");
    }

    if let Some(cmd) = config.command.clone() {
        match cmd {
            PanelCommand::Serve => return rt.block_on(run_server(config)),
            PanelCommand::Fp(fp_args) => {
                apply_env_file(&mut config, &fp_args.env_file);
                return match fp_args.command {
                    Some(sub) => rt.block_on(run_fp_command(sub, &config, &fp_args.env_file)),
                    None => rt.block_on(run_interactive_menu(&config, &fp_args.env_file)),
                };
            }
        }
    }

    rt.block_on(run_server(config))
}
