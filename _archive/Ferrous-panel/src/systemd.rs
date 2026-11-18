use actix_web::error::ErrorInternalServerError;
use actix_web::Error;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppServiceSpec {
    pub id: String,
    pub name: String,
    pub framework: String,
    pub runtime_version: String,
    pub domain: String,
    pub php_version: Option<String>,
    pub internal_port: u16,
    pub working_dir: String,
    pub exec_command: String,
    pub workers: u16,
    pub memory_limit_mb: u32,
    pub cpu_quota_percent: u16,
    pub status: String,
    pub ssl_enabled: bool,
    pub websocket_enabled: bool,
    pub rate_limit_rps: u32,
    pub env_vars: Vec<(String, String)>,
}

pub struct SystemdManager;

impl SystemdManager {
    pub fn generate_unit_file(spec: &AppServiceSpec) -> String {
        let is_php = spec.framework.contains("PHP")
            || spec
                .php_version
                .as_deref()
                .map(|v| v != "None" && !v.is_empty())
                .unwrap_or(false);

        if is_php {
            let ver = spec.php_version.as_deref().unwrap_or("8.3");
            return format!(
                r#"; ============================================================================
; /etc/php/{ver}/fpm/pool.d/{name}.conf — Managed by Ferrous-panel
; Service Unit: php{ver}-fpm.service (Ubuntu Systemd FPM Pool Isolation)
; ============================================================================
[{name}]
user = www-data
group = www-data
listen = /run/php/php{ver}-fpm.sock
listen.owner = www-data
listen.group = www-data
listen.mode = 0660

pm = dynamic
pm.max_children = {max_children}
pm.start_servers = {start_servers}
pm.min_spare_servers = {min_spare}
pm.max_spare_servers = {max_spare}
pm.max_requests = 2000

php_admin_value[memory_limit] = {mem}M
php_admin_value[open_basedir] = {working_dir}:/tmp:/proc
php_admin_value[error_log] = /www/wwwlogs/{domain}.php-error.log
"#,
                ver = ver,
                name = spec.name,
                max_children = (spec.workers as u32).max(4) * 5,
                start_servers = (spec.workers as u32).max(2),
                min_spare = (spec.workers as u32).max(2),
                max_spare = (spec.workers as u32).max(4) * 2,
                mem = spec.memory_limit_mb,
                working_dir = spec.working_dir,
                domain = spec.domain
            );
        }

        let fw_lower = spec.framework.to_lowercase();
        let lang_runtime_envs = if fw_lower.contains("node")
            || fw_lower.contains("next")
            || fw_lower.contains("nuxt")
            || fw_lower.contains("svelte")
            || fw_lower.contains("astro")
            || fw_lower.contains("remix")
            || fw_lower.contains("nest")
            || fw_lower.contains("bun")
            || fw_lower.contains("typescript")
        {
            format!(
                "Environment=\"NODE_ENV=production\"\nEnvironment=\"UV_THREADPOOL_SIZE={}\"\nEnvironment=\"FERROUS_NAPI_BRIDGE={}/native/index.node\"",
                (spec.workers as usize).max(4),
                spec.working_dir
            )
        } else if fw_lower.contains("python")
            || fw_lower.contains("fastapi")
            || fw_lower.contains("django")
            || fw_lower.contains("flask")
        {
            format!(
                "Environment=\"PYTHONUNBUFFERED=1\"\nEnvironment=\"PYTHONDONTWRITEBYTECODE=1\"\nEnvironment=\"VIRTUAL_ENV={}/.venv\"\nEnvironment=\"PATH={}/.venv/bin:/usr/local/bin:/usr/bin:/bin\"\nEnvironment=\"WEB_CONCURRENCY={}\"",
                spec.working_dir,
                spec.working_dir,
                spec.workers
            )
        } else if fw_lower.contains("go") || fw_lower.contains("fiber") || fw_lower.contains("gin") {
            format!(
                "Environment=\"GOMAXPROCS={}\"\nEnvironment=\"GOGC=100\"\nEnvironment=\"CGO_ENABLED=1\"\nEnvironment=\"LD_LIBRARY_PATH={}/lib:/usr/local/lib\"",
                spec.workers,
                spec.working_dir
            )
        } else if fw_lower.contains("java")
            || fw_lower.contains("kotlin")
            || fw_lower.contains("spring")
            || fw_lower.contains("jvm")
        {
            format!(
                "Environment=\"JAVA_TOOL_OPTIONS=-XX:+UseZGC -Xmx{}m -Djava.library.path={}/lib\"",
                (spec.memory_limit_mb * 80) / 100,
                spec.working_dir
            )
        } else if fw_lower.contains(".net") || fw_lower.contains("c#") || fw_lower.contains("aspnet") {
            format!(
                "Environment=\"ASPNETCORE_ENVIRONMENT=Production\"\nEnvironment=\"ASPNETCORE_URLS=http://127.0.0.1:{}\"\nEnvironment=\"DOTNET_CLI_TELEMETRY_OPTOUT=1\"\nEnvironment=\"LD_LIBRARY_PATH={}/lib:/usr/local/lib\"",
                spec.internal_port,
                spec.working_dir
            )
        } else if fw_lower.contains("ruby") || fw_lower.contains("rails") || fw_lower.contains("elixir") || fw_lower.contains("phoenix") {
            format!(
                "Environment=\"RAILS_ENV=production\"\nEnvironment=\"MIX_ENV=prod\"\nEnvironment=\"RUBY_YJIT_ENABLE=1\"\nEnvironment=\"WEB_CONCURRENCY={}\"",
                spec.workers
            )
        } else if fw_lower.contains("wasm") || fw_lower.contains("wasi") {
            format!(
                "Environment=\"WASMTIME_BACKTRACE_DETAILS=1\"\nEnvironment=\"WASI_ROOT={}\"",
                spec.working_dir
            )
        } else {
            format!(
                "Environment=\"ACTIX_WORKERS={}\"\nEnvironment=\"RUST_BACKTRACE=1\"\nEnvironment=\"LD_LIBRARY_PATH={}/lib:/usr/local/lib\"",
                spec.workers,
                spec.working_dir
            )
        };

        let env_lines: String = spec
            .env_vars
            .iter()
            .map(|(k, v)| format!("Environment=\"{}={}\"", k, v))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"[Unit]
Description=Ferrous-panel Managed Polyglot Service: {name} ({framework})
After=network-online.target nginx.service
Wants=network-online.target

[Service]
Type=simple
User=www-data
Group=www-data
WorkingDirectory={working_dir}
ExecStart={exec_command}
Restart=always
RestartSec=3
LimitNOFILE=65535

# Resource Quota & Isolation
MemoryMax={memory_limit}M
CPUQuota={cpu_quota}%

# Systemd Security Sandboxing for Ubuntu
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths={working_dir} /www/wwwlogs /run/ferrous-polyglot.sock
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictSUIDSGID=true
LockPersonality=true

# Polyglot Runtime Environment & Rust FFI Bridge Socket
Environment="PORT={port}"
Environment="FERROUS_POLYGLOT_IPC=/run/ferrous-polyglot.sock"
{lang_runtime_envs}
{env_lines}

[Install]
WantedBy=multi-user.target
"#,
            name = spec.name,
            framework = spec.framework,
            working_dir = spec.working_dir,
            exec_command = spec.exec_command,
            memory_limit = spec.memory_limit_mb,
            cpu_quota = spec.cpu_quota_percent,
            port = spec.internal_port,
            lang_runtime_envs = lang_runtime_envs,
            env_lines = env_lines
        )
    }

    pub async fn write_and_reload(service_name: &str, unit_content: &str) -> Result<(), Error> {
        // Ensure webroot & log directories exist on Ubuntu VPS
        let _ = tokio::fs::create_dir_all("/www/wwwroot").await;
        let _ = tokio::fs::create_dir_all("/www/wwwlogs").await;

        if unit_content.starts_with("; ====") {
            // PHP-FPM pool config; reload active php-fpm service
            let _ = Command::new("systemctl")
                .args(["reload", "php8.3-fpm"])
                .output()
                .await;
            return Ok(());
        }

        let unit_path = format!("/etc/systemd/system/{}.service", service_name);
        if Path::new("/etc/systemd/system").exists() {
            tokio::fs::write(&unit_path, unit_content)
                .await
                .map_err(ErrorInternalServerError)?;
            let _ = Command::new("systemctl").arg("daemon-reload").output().await;
            let _ = Command::new("systemctl")
                .args(["enable", "--now", service_name])
                .output()
                .await;
        }
        Ok(())
    }
}

pub fn default_ubuntu_services() -> Vec<AppServiceSpec> {
    vec![
        AppServiceSpec {
            id: "svc-actix-core".into(),
            name: "commerce-api-actix".into(),
            framework: "Actix-Web (Rust)".into(),
            runtime_version: "rustc 1.83.0 (release LTO)".into(),
            domain: "api.ferroushub.vn".into(),
            php_version: None,
            internal_port: 8081,
            working_dir: "/www/wwwroot/api.ferroushub.vn".into(),
            exec_command: "/www/wwwroot/api.ferroushub.vn/target/release/commerce-api".into(),
            workers: 8,
            memory_limit_mb: 512,
            cpu_quota_percent: 400,
            status: "running".into(),
            ssl_enabled: true,
            websocket_enabled: true,
            rate_limit_rps: 120,
            env_vars: vec![
                ("RUST_LOG".into(), "info,actix_web=info".into()),
                ("DATABASE_POOL_MAX".into(), "32".into()),
            ],
        },
        AppServiceSpec {
            id: "svc-laravel-erp".into(),
            name: "erp-laravel-octane".into(),
            framework: "PHP 8.3-FPM".into(),
            runtime_version: "PHP 8.3.14-FPM (OPcache JIT)".into(),
            domain: "erp.ferroushub.vn".into(),
            php_version: Some("8.3".into()),
            internal_port: 9000,
            working_dir: "/www/wwwroot/erp.ferroushub.vn".into(),
            exec_command: "/usr/sbin/php-fpm8.3 --nodaemonize".into(),
            workers: 6,
            memory_limit_mb: 512,
            cpu_quota_percent: 200,
            status: "running".into(),
            ssl_enabled: true,
            websocket_enabled: false,
            rate_limit_rps: 150,
            env_vars: vec![("APP_ENV".into(), "production".into())],
        },
    ]
}
