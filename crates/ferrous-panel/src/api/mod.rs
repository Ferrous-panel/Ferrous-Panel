pub mod apps;
pub mod auth;
pub mod capabilities;
pub mod contract;
pub mod cpanel;
pub mod databases;
pub mod extended;
pub mod files;
pub mod health;
pub mod jobs;
pub mod security;
pub mod telemetry;
pub mod webserver;

use crate::auth::middleware::require_auth;
use crate::error::PanelError;
use actix_web::middleware::from_fn;
use actix_web::{web, HttpResponse};

const LARGE_SQL_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;
const LARGE_ZIP_JSON_BYTES: usize = 32 * 1024 * 1024;
const LARGE_RAW_UPLOAD_BYTES: usize = 64 * 1024 * 1024;

async fn not_implemented() -> Result<HttpResponse, PanelError> {
    Err(PanelError::NotImplemented(
        "endpoint not yet implemented in ferrous-panel",
    ))
}

fn register_p5_real_routes(cfg: &mut web::ServiceConfig) {
    cfg
        // Websites & Systemd Services
        .route("/apps", web::get().to(apps::list_apps))
        .route("/apps", web::post().to(apps::deploy_app))
        .route("/apps/{id}/action", web::post().to(apps::app_action))
        .route("/apps/{id}/rebuild", web::post().to(apps::rebuild_app))
        .route("/apps/{id}/git-deploy", web::post().to(apps::git_deploy_app))
        .route("/apps/{id}/config", web::put().to(apps::update_app_config))
        .route(
            "/apps/{id}/site-settings",
            web::put().to(apps::update_site_settings),
        )
        .route("/apps/{id}/test-http", web::post().to(apps::test_http_live))
        .route("/apps/{id}/renew-ssl", web::post().to(apps::renew_ssl))
        .route("/apps/{id}", web::delete().to(apps::delete_app))
        .route("/nginx/validate", web::post().to(apps::validate_nginx))
        // SQL Databases & Pure-Rust FerrousAdmin (SQLite 3 WAL, MariaDB, PostgreSQL)
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
        .route(
            "/databases/{id}/export",
            web::get().to(databases::export_database),
        )
        .service(
            web::resource("/databases/{id}/import")
                .app_data(web::PayloadConfig::default().limit(LARGE_SQL_PAYLOAD_BYTES))
                .route(web::post().to(databases::import_database)),
        )
        .route(
            "/databases/{id}/tables/{table}/rows",
            web::get().to(databases::browse_table_rows),
        )
        .route(
            "/databases/{id}/tables/{table}/truncate",
            web::post().to(databases::truncate_table),
        )
        .route(
            "/databases/{id}/tables/{table}",
            web::delete().to(databases::drop_table),
        )
        .route("/databases/{id}", web::delete().to(databases::delete_database))
        // SafePath File Manager (static ops before dynamic {file_id})
        .route("/files/{app_id}", web::get().to(files::list_files))
        .route("/files/{app_id}", web::post().to(files::create_node))
        .service(
            web::resource("/files/{app_id}/upload-zip")
                .app_data(web::JsonConfig::default().limit(LARGE_ZIP_JSON_BYTES))
                .route(web::post().to(files::upload_zip)),
        )
        .service(
            web::resource("/files/{app_id}/upload-raw")
                .app_data(web::PayloadConfig::default().limit(LARGE_RAW_UPLOAD_BYTES))
                .route(web::post().to(files::upload_raw)),
        )
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
        // cPanel Suite: Real Crontab, Zstd Backups, Hickory DNS Zone Editor & SFTP Chroot
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
        .route("/cpanel/dns", web::post().to(cpanel::add_dns))
        .route("/cpanel/dns/{id}", web::delete().to(cpanel::delete_dns))
        .route("/cpanel/sftp", web::post().to(cpanel::add_sftp))
        .route("/cpanel/sftp/{id}", web::delete().to(cpanel::delete_sftp))
        // Security: Real UFW, Fail2ban, sysctl Hardening, eBPF/XDP Anti-DDoS, L7 WAF & SHA-256 FIM
        .route("/security", web::get().to(security::get_security))
        .route("/security/ufw", web::post().to(security::add_ufw_rule))
        .route(
            "/security/ufw/{id}/toggle",
            web::put().to(security::toggle_ufw_rule),
        )
        .route("/security/ufw/{id}", web::delete().to(security::delete_ufw_rule))
        .route("/security/fail2ban/ban", web::post().to(security::ban_ip))
        .route("/security/fail2ban/unban", web::post().to(security::unban_ip))
        .route(
            "/security/hardening/{id}",
            web::put().to(security::toggle_hardening),
        )
        .route("/security/ddos-mode", web::post().to(security::set_ddos_mode))
        .route(
            "/security/waf/{id}/toggle",
            web::put().to(security::toggle_waf_rule),
        )
        .route("/security/fim-scan", web::post().to(security::run_fim_scan))
        .route(
            "/security/audit-logs",
            web::get().to(security::get_audit_logs),
        )
        // Asynchronous Background Job Manager (prevents HTTP / Cloudflare 522/524 timeouts)
        .route("/jobs", web::get().to(jobs::list_jobs))
        .route(
            "/jobs/apps/{id}/rebuild",
            web::post().to(jobs::start_rebuild_app_job),
        )
        .route(
            "/jobs/apps/{id}/git-deploy",
            web::post().to(jobs::start_git_deploy_job),
        )
        .route(
            "/jobs/apps/{id}/renew-ssl",
            web::post().to(jobs::start_renew_ssl_job),
        )
        .route(
            "/jobs/cpanel/backups",
            web::post().to(jobs::start_create_backup_job),
        )
        .route("/jobs/{id}/poll", web::get().to(jobs::poll_job))
        .route("/jobs/{id}", web::get().to(jobs::get_job));
}

fn register_extended_routes(cfg: &mut web::ServiceConfig) {
    cfg
        // Diagnostics, Self-Update & FerrousX Sync
        .route(
            "/system/diagnostics",
            web::get().to(extended::get_system_diagnostics),
        )
        .route(
            "/system/update-status",
            web::get().to(extended::get_system_update_status),
        )
        .route(
            "/system/update",
            web::post().to(extended::post_system_update),
        )
        .route(
            "/ferrousx/sync-status",
            web::get().to(extended::get_ferrousx_sync),
        )
        .route(
            "/ferrousx/sync-status",
            web::post().to(extended::post_ferrousx_sync),
        )
        .route(
            "/ferrousx/ipc/status",
            web::get().to(extended::get_ferrousx_ipc_status),
        )
        .route(
            "/ferrousx/ipc/command",
            web::post().to(extended::post_ferrousx_ipc_command),
        )
        // 1-Click Templates
        .route(
            "/oneclick/templates",
            web::get().to(extended::get_oneclick_templates),
        )
        .route(
            "/oneclick/deploy",
            web::post().to(extended::post_oneclick_deploy),
        )
        // Multi-PHP FPM Pools
        .route("/php", web::get().to(extended::get_php_versions))
        .route(
            "/php/switch-site",
            web::post().to(extended::post_switch_site_php),
        )
        .route(
            "/php/{version}/action",
            web::post().to(extended::post_php_action),
        )
        .route(
            "/php/{version}/settings",
            web::put().to(extended::put_php_settings),
        )
        .route(
            "/php/{version}/extensions/toggle",
            web::post().to(extended::post_php_extension_toggle),
        )
        // Software Packages
        .route("/software", web::get().to(extended::get_software_packages))
        .route(
            "/software/{id}/action",
            web::post().to(extended::post_software_action),
        )
        .route(
            "/software/{id}/config",
            web::put().to(extended::put_software_config),
        )
        // Mail Server (Postfix / Dovecot / Rspamd)
        .route("/mail", web::get().to(extended::get_mail))
        .route("/mail/domains", web::post().to(extended::post_mail_domain))
        .route(
            "/mail/mailboxes",
            web::post().to(extended::post_mail_mailbox),
        )
        .route(
            "/mail/mailboxes/{id}",
            web::delete().to(extended::delete_mail_mailbox),
        )
        .route(
            "/mail/forwarders",
            web::post().to(extended::post_mail_forwarder),
        )
        .route(
            "/mail/forwarders/{id}",
            web::delete().to(extended::delete_mail_forwarder),
        )
        .route("/mail/send", web::post().to(extended::post_mail_send))
        // System Optimizer & Anti-Crash Guard
        .route("/optimizer", web::get().to(extended::get_optimizer))
        .route(
            "/optimizer/apply-profile",
            web::post().to(extended::post_optimizer_apply_profile),
        )
        .route(
            "/optimizer/sysctl/{id}",
            web::put().to(extended::put_optimizer_sysctl),
        )
        .route(
            "/optimizer/reclaim-memory",
            web::post().to(extended::post_optimizer_reclaim_memory),
        )
        .route(
            "/optimizer/watchdog-heal",
            web::post().to(extended::post_optimizer_watchdog_heal),
        )
        .route(
            "/optimizer/anti-crash",
            web::put().to(extended::put_optimizer_anti_crash),
        )
        .route(
            "/optimizer/emergency-mitigate",
            web::post().to(extended::post_optimizer_emergency_mitigate),
        )
        // SSH Manager & Panel Admin
        .route("/ssh", web::get().to(extended::get_ssh_panel))
        .route("/ssh/config", web::put().to(extended::put_ssh_config))
        .route(
            "/ssh/keys/generate",
            web::post().to(extended::post_ssh_key_generate),
        )
        .route("/ssh/keys", web::post().to(extended::post_ssh_key))
        .route(
            "/ssh/keys/{id}",
            web::delete().to(extended::delete_ssh_key),
        )
        .route(
            "/ssh/sessions/{id}",
            web::delete().to(extended::delete_ssh_session),
        )
        .route("/ssh/exec", web::post().to(extended::post_ssh_exec))
        .route("/panel-admin", web::get().to(extended::get_panel_admin))
        .route("/panel-admin", web::put().to(extended::put_panel_admin))
        .route(
            "/panel-admin/cli-action",
            web::post().to(extended::post_panel_cli_action),
        )
        // Rust Source Inspector & Polyglot FFI/IPC Bridge Compiler
        .route("/rust-source", web::get().to(extended::get_rust_source))
        .route(
            "/polyglot/toolchains",
            web::get().to(extended::get_polyglot_toolchains),
        )
        .route(
            "/polyglot/compile",
            web::post().to(extended::post_polyglot_compile),
        )
        // Rust Edge Cloud (WASI Workers, Youki/Firecracker OCI, WireGuard Mesh, BLAKE3 Snapshots)
        .route("/edge-cloud", web::get().to(extended::get_edge_cloud))
        .route(
            "/edge-cloud/workers",
            web::post().to(extended::post_edge_worker),
        )
        .route(
            "/edge-cloud/workers/{id}/invoke",
            web::post().to(extended::post_edge_worker_invoke),
        )
        .route(
            "/edge-cloud/workers/{id}/toggle",
            web::post().to(extended::post_edge_worker_toggle),
        )
        .route(
            "/edge-cloud/workers/{id}",
            web::delete().to(extended::delete_edge_worker),
        )
        .route(
            "/edge-cloud/containers",
            web::post().to(extended::post_edge_container),
        )
        .route(
            "/edge-cloud/containers/{id}/action",
            web::post().to(extended::post_edge_container_action),
        )
        .route(
            "/edge-cloud/wireguard",
            web::post().to(extended::post_edge_wireguard),
        )
        .route(
            "/edge-cloud/wireguard/{id}/toggle",
            web::post().to(extended::post_edge_wireguard_toggle),
        )
        .route(
            "/edge-cloud/wireguard/{id}",
            web::delete().to(extended::delete_edge_wireguard),
        )
        .route(
            "/edge-cloud/snapshots",
            web::post().to(extended::post_edge_snapshot),
        )
        .route(
            "/edge-cloud/snapshots/{id}",
            web::delete().to(extended::delete_edge_snapshot),
        );
}

fn register_pma_api_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(health::health))
        .route("/auth/login", web::post().to(auth::login))
        .route("/auth/session", web::get().to(auth::session))
        .route("/auth/logout", web::post().to(auth::logout))
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
        .route(
            "/databases/{id}/export",
            web::get().to(databases::export_database),
        )
        .service(
            web::resource("/databases/{id}/import")
                .app_data(web::PayloadConfig::default().limit(LARGE_SQL_PAYLOAD_BYTES))
                .route(web::post().to(databases::import_database)),
        )
        .route(
            "/databases/{id}/tables/{table}/rows",
            web::get().to(databases::browse_table_rows),
        )
        .route(
            "/databases/{id}/tables/{table}/truncate",
            web::post().to(databases::truncate_table),
        )
        .route(
            "/databases/{id}/tables/{table}",
            web::delete().to(databases::drop_table),
        )
        .route("/databases/{id}", web::delete().to(databases::delete_database))
        .default_service(web::to(not_implemented));
}

/// Mounts `/api`, `/phpadmin/api`, and `/phpmyadmin/api` scopes protected by
/// authentication middleware on all routes except public health/login endpoints.
pub fn configure_api(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .wrap(from_fn(require_auth))
            .route("/health", web::get().to(health::health))
            .route("/auth/login", web::post().to(auth::login))
            .route("/auth/session", web::get().to(auth::session))
            .route("/auth/logout", web::post().to(auth::logout))
            .route("/telemetry", web::get().to(telemetry::get_telemetry))
            .route("/capabilities", web::get().to(capabilities::get_capabilities))
            .route("/webserver", web::get().to(webserver::get_webserver))
            .route("/webserver", web::put().to(webserver::put_webserver))
            .route("/webserver/switch", web::post().to(webserver::switch_webserver))
            .configure(register_p5_real_routes)
            .configure(register_extended_routes)
            .service(
                web::scope("/actix-gateway/{subsystem}")
                    .configure(register_p5_real_routes)
                    .configure(register_extended_routes)
                    .default_service(web::to(not_implemented)),
            )
            .default_service(web::to(not_implemented)),
    )
    .service(
        web::scope("/phpadmin/api")
            .wrap(from_fn(require_auth))
            .configure(register_pma_api_routes),
    )
    .service(
        web::scope("/phpmyadmin/api")
            .wrap(from_fn(require_auth))
            .configure(register_pma_api_routes),
    );
}
