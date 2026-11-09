use actix_cors::Cors;
use actix_web::{
    dev::ServiceRequest, error::ErrorUnauthorized, get, middleware, post, web, App, Error,
    HttpMessage, HttpRequest, HttpResponse, HttpServer, Responder,
};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, Networks, RefreshKind, System};
use tracing::{info, warn};

mod cpanel_tools;
mod database;
mod file_manager;
mod git_deploy;
mod mail_server;
mod nginx;
mod oneclick_store;
mod optimizer;
mod php_manager;
mod rust_packages;
mod security;
mod ssh_terminal;
mod systemd;

use git_deploy::{GitDeployEngine, GitDeploySpec};
use nginx::{NginxConfigRequest, NginxManager};
use oneclick_store::{OneClickStoreEngine, OneClickTemplateSpec};
use optimizer::{KernelSysctlModel, SystemOptimizer};
use php_manager::{control_php_fpm, MultiPhpState};
use rust_packages::{control_package, update_package_config};
use security::{FirewallRuleRequest, SecurityAuditReport, SecurityManager};
use ssh_terminal::{SshDaemonConfig, SshTerminalManager, WebSshExecRequest};
use systemd::{AppServiceSpec, SystemdManager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostTelemetry {
    pub hostname: String,
    pub os_version: String,
    pub kernel_version: String,
    pub uptime_seconds: u64,
    pub cpu_brand: String,
    pub cpu_cores: usize,
    pub cpu_usage_percent: f32,
    pub load_average: [f64; 3],
    pub memory_total_mb: u64,
    pub memory_used_mb: u64,
    pub swap_total_mb: u64,
    pub swap_used_mb: u64,
    pub timestamp: DateTime<Utc>,
}

pub struct AppState {
    pub sys: RwLock<System>,
    pub apps: RwLock<Vec<AppServiceSpec>>,
    pub jwt_secret: String,
}

#[get("/api/v1/telemetry")]
async fn get_telemetry(data: web::Data<Arc<AppState>>) -> Result<HttpResponse, Error> {
    let mut sys = data.sys.write().unwrap();
    sys.refresh_cpu_all();
    sys.refresh_memory();

    let load = System::load_average();
    let cpu_usage = sys.global_cpu_usage();
    let cpu_brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "AMD EPYC / Ubuntu x86_64".to_string());

    let telemetry = HostTelemetry {
        hostname: System::host_name().unwrap_or_else(|| "ubuntu-prod-node01".to_string()),
        os_version: System::long_os_version().unwrap_or_else(|| "Ubuntu 24.04.1 LTS".to_string()),
        kernel_version: System::kernel_version().unwrap_or_else(|| "6.8.0-48-generic".to_string()),
        uptime_seconds: System::uptime(),
        cpu_brand,
        cpu_cores: sys.cpus().len(),
        cpu_usage_percent: cpu_usage,
        load_average: [load.one, load.five, load.fifteen],
        memory_total_mb: sys.total_memory() / (1024 * 1024),
        memory_used_mb: sys.used_memory() / (1024 * 1024),
        swap_total_mb: sys.total_swap() / (1024 * 1024),
        swap_used_mb: sys.used_swap() / (1024 * 1024),
        timestamp: Utc::now(),
    };

    Ok(HttpResponse::Ok().json(telemetry))
}

#[get("/api/v1/apps")]
async fn list_apps(data: web::Data<Arc<AppState>>) -> impl Responder {
    let apps = data.apps.read().unwrap();
    HttpResponse::Ok().json(&*apps)
}

#[post("/api/v1/apps/deploy")]
async fn deploy_app(
    data: web::Data<Arc<AppState>>,
    payload: web::Json<AppServiceSpec>,
) -> Result<HttpResponse, Error> {
    let spec = payload.into_inner();
    info!(
        service = %spec.name,
        framework = %spec.framework,
        port = spec.internal_port,
        "Deploying application unit & Nginx reverse proxy"
    );

    // 1. Generate hardened Systemd unit with sandboxing directives
    let unit_file = SystemdManager::generate_unit_file(&spec);
    SystemdManager::write_and_reload(&spec.name, &unit_file).await?;

    // 2. Generate Nginx upstream & reverse proxy configuration
    let nginx_conf = NginxManager::generate_vhost_from_app(&spec);
    NginxManager::test_and_reload(&spec.domain, &nginx_conf).await?;

    let mut apps = data.apps.write().unwrap();
    apps.push(spec.clone());

    Ok(HttpResponse::Created().json(serde_json::json!({
        "status": "deployed",
        "systemd_unit": unit_file,
        "nginx_vhost": nginx_conf
    })))
}

#[post("/api/v1/nginx/validate")]
async fn validate_nginx(payload: web::Json<NginxConfigRequest>) -> Result<HttpResponse, Error> {
    let config_str = NginxManager::build_config(&payload);
    let validation = NginxManager::validate_syntax(&config_str).await;
    Ok(HttpResponse::Ok().json(validation))
}

#[get("/api/v1/security/audit")]
async fn security_audit() -> Result<HttpResponse, Error> {
    let report: SecurityAuditReport = SecurityManager::run_ubuntu_audit().await;
    Ok(HttpResponse::Ok().json(report))
}

#[post("/api/v1/security/ufw")]
async fn update_ufw_rule(payload: web::Json<FirewallRuleRequest>) -> Result<HttpResponse, Error> {
    let res = SecurityManager::apply_ufw_rule(&payload).await?;
    Ok(HttpResponse::Ok().json(res))
}

#[post("/api/v1/ssh/config")]
async fn configure_sshd(payload: web::Json<SshDaemonConfig>) -> Result<HttpResponse, Error> {
    let rendered = SshTerminalManager::apply_sshd_config(&payload).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "reloaded",
        "sshd_config": rendered
    })))
}

#[post("/api/v1/ssh/exec")]
async fn execute_web_ssh(payload: web::Json<WebSshExecRequest>) -> Result<HttpResponse, Error> {
    let result = SshTerminalManager::execute_web_pty_command(&payload).await;
    Ok(HttpResponse::Ok().json(result))
}

#[post("/api/v1/git/deploy")]
async fn git_atomic_deploy(payload: web::Json<GitDeploySpec>) -> Result<HttpResponse, Error> {
    let res = GitDeployEngine::trigger_atomic_deploy(&payload).await?;
    Ok(HttpResponse::Ok().json(res))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,actix_web=info,ferrous_panel_daemon=debug")
        .init();

    let bind_addr = std::env::var("PANEL_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:9443".to_string());
    let workers: usize = std::env::var("ACTIX_WORKERS")
        .ok()
        .and_then(|w| w.parse().ok())
        .unwrap_or_else(num_cpus_default);

    info!("Starting FerrousPanel Actix-Web Daemon v2.0.0 on {} with {} workers", bind_addr, workers);

    let sys = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );

    let shared_state = Arc::new(AppState {
        sys: RwLock::new(sys),
        apps: RwLock::new(systemd::default_ubuntu_services()),
        jwt_secret: std::env::var("PANEL_JWT_SECRET").unwrap_or_else(|_| "ed25519-hmac-secret-key".into()),
    });

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("https://panel.internal.lan")
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE"])
            .allowed_headers(vec![
                actix_web::http::header::AUTHORIZATION,
                actix_web::http::header::CONTENT_TYPE,
            ])
            .max_age(3600);

        App::new()
            .app_data(web::Data::new(shared_state.clone()))
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::default())
            .wrap(
                middleware::DefaultHeaders::new()
                    .add(("X-Frame-Options", "DENY"))
                    .add(("X-Content-Type-Options", "nosniff"))
                    .add(("Referrer-Policy", "strict-origin-when-cross-origin"))
                    .add((
                        "Strict-Transport-Security",
                        "max-age=63072000; includeSubDomains; preload",
                    )),
            )
            .wrap(cors)
            .service(get_telemetry)
            .service(list_apps)
            .service(deploy_app)
            .service(validate_nginx)
            .service(security_audit)
            .service(update_ufw_rule)
            .service(configure_sshd)
            .service(execute_web_ssh)
            .service(git_atomic_deploy)
    })
    .workers(workers)
    .keep_alive(std::time::Duration::from_secs(75))
    .bind(bind_addr)?
    .run()
    .await
}

fn num_cpus_default() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
