#![forbid(unsafe_code)]

use actix_web::{web, App, HttpServer};
use anyhow::Context;
use clap::Parser;
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

async fn run_server(mut config: Config) -> anyhow::Result<()> {
    if config.port == 0 || is_reserved_website_port(config.port) {
        tracing::warn!(
            configured_port = config.port,
            fallback_port = DEFAULT_PANEL_PORT,
            "ports 80 and 443 are reserved exclusively for websites (Nginx/FerrousX); falling back panel port to 8443"
        );
        config.port = DEFAULT_PANEL_PORT;
    }
    let bind_addr = config.bind.clone();
    let bind_port = config.port;
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
    tracing::info!(bind = %bind_addr, port = bind_port, "starting ferrous-panel control plane");

    // Record initial real telemetry point and sample every 60s for real historical chart data.
    let _ = shared_state.record_telemetry_sample(true);
    let telemetry_state = shared_state.clone();
    actix_web::rt::spawn(async move {
        let mut ticker = actix_web::rt::time::interval(std::time::Duration::from_secs(60));
        // Skip first immediate tick since initial point was just recorded above.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            if let Err(err) = telemetry_state.record_telemetry_sample(true) {
                tracing::debug!(error = %err, "background telemetry sample skipped");
            }
        }
    });

    HttpServer::new(move || {
        let app_state = shared_state.clone();
        App::new().configure(move |cfg| configure_app(cfg, app_state))
    })
    .bind((bind_addr.as_str(), bind_port))
    .with_context(|| format!("failed to bind {bind_addr}:{bind_port}"))?
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
