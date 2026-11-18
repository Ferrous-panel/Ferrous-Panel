#![forbid(unsafe_code)]

use clap::Parser;
use ferrousx::config::{load_bundle, CliArgs, MasterConfigToml};
use ferrousx::error::ServerError;
use ferrousx::server::FerrousxServer;
use std::process::ExitCode;
use tokio::signal::unix::{signal, SignalKind};
use tracing_subscriber::EnvFilter;

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .try_init();
}

fn read_worker_threads(config_path: &std::path::Path) -> usize {
    std::fs::read_to_string(config_path)
        .ok()
        .and_then(|raw| toml::from_str::<MasterConfigToml>(&raw).ok())
        .map(|cfg| cfg.worker_threads)
        .unwrap_or(0)
}

async fn run_daemon(args: CliArgs) -> Result<(), ServerError> {
    let server = FerrousxServer::start(&args.config).await?;
    let mut sighup = signal(SignalKind::hangup())?;
    let mut sigterm = signal(SignalKind::terminate())?;

    loop {
        tokio::select! {
            _ = sighup.recv() => {
                match server.reload() {
                    Ok(sites) => tracing::info!(sites, "SIGHUP reload succeeded"),
                    Err(err) => tracing::error!(error = %err, "SIGHUP reload rejected; keeping previous config"),
                }
            }
            _ = sigterm.recv() => {
                tracing::info!("received SIGTERM, shutting down ferrousx");
                break;
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("received SIGINT, shutting down ferrousx");
                break;
            }
        }
    }

    server.shutdown().await;
    Ok(())
}

fn main() -> ExitCode {
    init_tracing();
    let args = CliArgs::parse();

    if args.check_config {
        match load_bundle(&args.config) {
            Ok(bundle) => {
                println!(
                    "ferrousx: config syntax OK ({} sites, {} SNI certificates)",
                    bundle.sites.len(),
                    bundle.sni_certs.len()
                );
                return ExitCode::SUCCESS;
            }
            Err(err) => {
                eprintln!("ferrousx: config check failed: {err}");
                return ExitCode::FAILURE;
            }
        }
    }

    let workers = read_worker_threads(&args.config);
    let mut rt_builder = tokio::runtime::Builder::new_multi_thread();
    rt_builder.enable_all();
    if workers > 0 {
        rt_builder.worker_threads(workers);
    }

    let rt = match rt_builder.build() {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("ferrousx: failed to initialize Tokio runtime: {err}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(err) = rt.block_on(run_daemon(args)) {
        eprintln!("ferrousx: fatal error: {err}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
