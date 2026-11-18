pub mod apps;
pub mod auth;
pub mod contract;
pub mod cpanel;
pub mod databases;
pub mod files;
pub mod health;
pub mod security;
pub mod telemetry;
pub mod webserver;

use crate::auth::middleware::require_auth;
use crate::error::PanelError;
use actix_web::middleware::from_fn;
use actix_web::{web, HttpResponse};

async fn not_implemented() -> Result<HttpResponse, PanelError> {
    Err(PanelError::NotImplemented(
        "endpoint not yet implemented in ferrous-panel",
    ))
}

fn register_p5_real_routes(cfg: &mut web::ServiceConfig) {
    cfg
        // Websites & Systemd Services (P5 Real)
        .route("/apps", web::get().to(apps::list_apps))
        .route("/apps", web::post().to(apps::deploy_app))
        .route("/apps/{id}/action", web::post().to(apps::app_action))
        .route("/apps/{id}/config", web::put().to(apps::update_app_config))
        .route(
            "/apps/{id}/site-settings",
            web::put().to(apps::update_site_settings),
        )
        .route("/apps/{id}/test-http", web::post().to(apps::test_http_live))
        .route("/apps/{id}/renew-ssl", web::post().to(apps::renew_ssl))
        .route("/apps/{id}", web::delete().to(apps::delete_app))
        // SQL Databases & MariaDB/PostgreSQL CLI (P5 Real; static mysql-config before dynamic {id})
        .route("/databases", web::get().to(databases::list_databases))
        .route("/databases", web::post().to(databases::create_database))
        .route(
            "/databases/mysql-config",
            web::get().to(databases::get_mysql_config),
        )
        .route(
            "/databases/mysql-config",
            web::put().to(databases::update_mysql_config),
        )
        .route(
            "/databases/{id}/password",
            web::put().to(databases::change_password),
        )
        .route("/databases/{id}/query", web::post().to(databases::execute_query))
        .route("/databases/{id}", web::delete().to(databases::delete_database))
        // SafePath File Manager (P5 Real; static ops before dynamic {file_id})
        .route("/files/{app_id}", web::get().to(files::list_files))
        .route("/files/{app_id}", web::post().to(files::create_node))
        .route("/files/{app_id}/upload-zip", web::post().to(files::upload_zip))
        .route("/files/{app_id}/compress", web::post().to(files::compress_dir))
        .route(
            "/files/{app_id}/remote-download",
            web::post().to(files::remote_download),
        )
        .route("/files/{app_id}/batch-chmod", web::post().to(files::batch_chmod))
        .route(
            "/files/{app_id}/{file_id}/extract",
            web::post().to(files::extract_archive),
        )
        .route(
            "/files/{app_id}/{file_id}/rename",
            web::post().to(files::rename_node),
        )
        .route("/files/{app_id}/{file_id}", web::put().to(files::update_file))
        .route("/files/{app_id}/{file_id}", web::delete().to(files::delete_node))
        // cPanel Suite: Real Crontab + Real Zstd Backups (DNS/SFTP remain 501)
        .route("/cpanel", web::get().to(cpanel::get_cpanel))
        .route("/cpanel/cron", web::post().to(cpanel::add_cron))
        .route("/cpanel/cron/{id}/run", web::post().to(cpanel::run_cron))
        .route("/cpanel/cron/{id}/toggle", web::put().to(cpanel::toggle_cron))
        .route("/cpanel/cron/{id}", web::delete().to(cpanel::delete_cron))
        .route("/cpanel/backups", web::post().to(cpanel::create_backup))
        .route(
            "/cpanel/backups/{id}/restore",
            web::post().to(cpanel::restore_backup),
        )
        .route("/cpanel/backups/{id}", web::delete().to(cpanel::delete_backup))
        // Security: Real UFW, Fail2ban, sysctl Hardening (eBPF/WAF/FIM remain 501)
        .route("/security", web::get().to(security::get_security))
        .route("/security/ufw", web::post().to(security::add_ufw_rule))
        .route("/security/ufw/{id}", web::delete().to(security::delete_ufw_rule))
        .route("/security/fail2ban/ban", web::post().to(security::ban_ip))
        .route("/security/fail2ban/unban", web::post().to(security::unban_ip))
        .route(
            "/security/hardening/{id}",
            web::put().to(security::toggle_hardening),
        );
}

fn register_contract_501_routes(cfg: &mut web::ServiceConfig) {
    cfg
        // Diagnostics & FerrousX Sync
        .route("/system/diagnostics", web::get().to(not_implemented))
        .route("/ferrousx/sync-status", web::get().to(not_implemented))
        .route("/ferrousx/sync-status", web::post().to(not_implemented))
        // Remaining Websites stubs
        .route("/nginx/validate", web::post().to(not_implemented))
        .route("/apps/{id}/rebuild", web::post().to(not_implemented))
        .route("/apps/{id}/git-deploy", web::post().to(not_implemented))
        // 1-Click Templates
        .route("/oneclick/templates", web::get().to(not_implemented))
        .route("/oneclick/deploy", web::post().to(not_implemented))
        // Multi-PHP FPM Pools
        .route("/php", web::get().to(not_implemented))
        .route("/php/switch-site", web::post().to(not_implemented))
        .route("/php/{version}/action", web::post().to(not_implemented))
        .route("/php/{version}/settings", web::put().to(not_implemented))
        .route("/php/{version}/extensions/toggle", web::post().to(not_implemented))
        // Software Packages
        .route("/software", web::get().to(not_implemented))
        .route("/software/{id}/action", web::post().to(not_implemented))
        .route("/software/{id}/config", web::put().to(not_implemented))
        // Mail Server (explicitly kept 501)
        .route("/mail", web::get().to(not_implemented))
        .route("/mail/domains", web::post().to(not_implemented))
        .route("/mail/mailboxes", web::post().to(not_implemented))
        .route("/mail/mailboxes/{id}", web::delete().to(not_implemented))
        .route("/mail/forwarders", web::post().to(not_implemented))
        .route("/mail/forwarders/{id}", web::delete().to(not_implemented))
        .route("/mail/send", web::post().to(not_implemented))
        // System Optimizer & Anti-Crash
        .route("/optimizer", web::get().to(not_implemented))
        .route("/optimizer/apply-profile", web::post().to(not_implemented))
        .route("/optimizer/sysctl/{id}", web::put().to(not_implemented))
        .route("/optimizer/reclaim-memory", web::post().to(not_implemented))
        .route("/optimizer/watchdog-heal", web::post().to(not_implemented))
        .route("/optimizer/anti-crash", web::put().to(not_implemented))
        .route("/optimizer/emergency-mitigate", web::post().to(not_implemented))
        // cPanel DNS & SFTP (explicitly kept 501)
        .route("/cpanel/dns", web::post().to(not_implemented))
        .route("/cpanel/dns/{id}", web::delete().to(not_implemented))
        .route("/cpanel/sftp", web::post().to(not_implemented))
        .route("/cpanel/sftp/{id}", web::delete().to(not_implemented))
        // Security eBPF/WAF/FIM & UFW toggle (explicitly kept 501)
        .route("/security/ufw/{id}/toggle", web::put().to(not_implemented))
        .route("/security/ddos-mode", web::post().to(not_implemented))
        .route("/security/waf/{id}/toggle", web::put().to(not_implemented))
        .route("/security/fim-scan", web::post().to(not_implemented))
        // SSH & Panel Admin
        .route("/ssh", web::get().to(not_implemented))
        .route("/ssh/config", web::put().to(not_implemented))
        .route("/ssh/keys/generate", web::post().to(not_implemented))
        .route("/ssh/keys", web::post().to(not_implemented))
        .route("/ssh/keys/{id}", web::delete().to(not_implemented))
        .route("/ssh/sessions/{id}", web::delete().to(not_implemented))
        .route("/ssh/exec", web::post().to(not_implemented))
        .route("/panel-admin", web::get().to(not_implemented))
        .route("/panel-admin", web::put().to(not_implemented))
        .route("/panel-admin/cli-action", web::post().to(not_implemented))
        // Rust Source & Polyglot Bridge (explicitly kept 501)
        .route("/rust-source", web::get().to(not_implemented))
        .route("/polyglot/toolchains", web::get().to(not_implemented))
        .route("/polyglot/compile", web::post().to(not_implemented))
        // Edge Cloud (WASI, OCI, WireGuard, BLAKE3 Snapshots — explicitly kept 501)
        .route("/edge-cloud", web::get().to(not_implemented))
        .route("/edge-cloud/workers", web::post().to(not_implemented))
        .route("/edge-cloud/workers/{id}/invoke", web::post().to(not_implemented))
        .route("/edge-cloud/workers/{id}/toggle", web::post().to(not_implemented))
        .route("/edge-cloud/workers/{id}", web::delete().to(not_implemented))
        .route("/edge-cloud/containers", web::post().to(not_implemented))
        .route("/edge-cloud/containers/{id}/action", web::post().to(not_implemented))
        .route("/edge-cloud/wireguard", web::post().to(not_implemented))
        .route("/edge-cloud/wireguard/{id}/toggle", web::post().to(not_implemented))
        .route("/edge-cloud/wireguard/{id}", web::delete().to(not_implemented))
        .route("/edge-cloud/snapshots", web::post().to(not_implemented))
        .route("/edge-cloud/snapshots/{id}", web::delete().to(not_implemented));
}

/// Mounts `/api` scope protected by authentication middleware on all routes
/// except `GET /api/health` and `POST /api/auth/login`.
pub fn configure_api(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .wrap(from_fn(require_auth))
            .route("/health", web::get().to(health::health))
            .route("/auth/login", web::post().to(auth::login))
            .route("/auth/session", web::get().to(auth::session))
            .route("/auth/logout", web::post().to(auth::logout))
            .route("/telemetry", web::get().to(telemetry::get_telemetry))
            .route("/webserver", web::get().to(webserver::get_webserver))
            .route("/webserver", web::put().to(webserver::put_webserver))
            .route("/webserver/switch", web::post().to(webserver::switch_webserver))
            .configure(register_p5_real_routes)
            .configure(register_contract_501_routes)
            .service(
                web::scope("/actix-gateway/{subsystem}")
                    .configure(register_p5_real_routes)
                    .configure(register_contract_501_routes)
                    .default_service(web::to(not_implemented)),
            )
            .default_service(web::to(not_implemented)),
    );
}
