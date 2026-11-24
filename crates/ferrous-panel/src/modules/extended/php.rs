use super::common::*;
use std::collections::HashSet;

// ============================================================================
// 3. Multi-PHP FPM Pools (Live php-cli/php-fpm probing + real config sync)
// ============================================================================

fn default_php_extensions(prod: bool) -> Vec<PhpExtensionItem> {
    let default_on = !prod;
    vec![
        PhpExtensionItem {
            name: "opcache".to_string(),
            category: "Performance".to_string(),
            enabled: default_on,
            description: "Zend OPcache bytecode accelerator + Tracing JIT compiler.".to_string(),
        },
        PhpExtensionItem {
            name: "pdo_mysql".to_string(),
            category: "Database".to_string(),
            enabled: default_on,
            description: "MariaDB / MySQL native PDO driver (mysqlnd).".to_string(),
        },
        PhpExtensionItem {
            name: "pdo_pgsql".to_string(),
            category: "Database".to_string(),
            enabled: default_on,
            description: "PostgreSQL native libpq PDO driver.".to_string(),
        },
        PhpExtensionItem {
            name: "redis".to_string(),
            category: "Cache".to_string(),
            enabled: default_on,
            description: "PhpRedis C extension for high-speed session & object caching.".to_string(),
        },
        PhpExtensionItem {
            name: "curl".to_string(),
            category: "Network".to_string(),
            enabled: default_on,
            description: "libcurl HTTP/TLS client bindings.".to_string(),
        },
        PhpExtensionItem {
            name: "mbstring".to_string(),
            category: "Core".to_string(),
            enabled: default_on,
            description: "Multibyte UTF-8 string manipulation functions.".to_string(),
        },
        PhpExtensionItem {
            name: "imagick".to_string(),
            category: "Media".to_string(),
            enabled: false,
            description: "ImageMagick raster/vector image processing extension.".to_string(),
        },
        PhpExtensionItem {
            name: "ffi".to_string(),
            category: "Rust Bridge".to_string(),
            enabled: default_on,
            description: "Zend Foreign Function Interface for loading Rust C-ABI shared libraries (.so).".to_string(),
        },
    ]
}

fn apt_package_for_php_ext(version: &str, ext_name: &str) -> Option<String> {
    let pkg_suffix = match ext_name {
        "pdo_mysql" | "mysqli" | "mysqlnd" => "mysql",
        "pdo_pgsql" | "pgsql" => "pgsql",
        "redis" => "redis",
        "imagick" => "imagick",
        "mbstring" => "mbstring",
        "curl" => "curl",
        "xml" | "dom" | "simplexml" => "xml",
        "zip" => "zip",
        "gd" => "gd",
        "intl" => "intl",
        "bcmath" => "bcmath",
        "sqlite3" | "pdo_sqlite" => "sqlite3",
        "opcache" | "ffi" => "common",
        _ => return None,
    };
    Some(format!("php{version}-{pkg_suffix}"))
}

fn validate_php_ini_value(val: &str, field: &str) -> Result<String, PanelError> {
    let clean = val.trim();
    if clean.is_empty()
        || clean.len() > 64
        || clean
            .chars()
            .any(|c| matches!(c, '\r' | '\n' | '\0' | ';' | '#'))
    {
        return Err(PanelError::Validation(format!(
            "PHP setting '{field}' contains invalid characters"
        )));
    }
    Ok(clean.to_string())
}

fn render_php_ini(inst: &PhpVersionInstance) -> String {
    format!(
        "; Managed by FerrousPanel — PHP {} FPM php.ini\n\
         [PHP]\n\
         engine = On\n\
         memory_limit = {}\n\
         upload_max_filesize = {}\n\
         post_max_size = {}\n\
         max_execution_time = {}\n\
         max_input_vars = {}\n\
         display_errors = {}\n\
         disable_functions = {}\n\
         \n\
         [opcache]\n\
         opcache.enable = {}\n\
         opcache.jit = tracing\n\
         opcache.jit_buffer_size = {}\n",
        inst.version,
        inst.memory_limit,
        inst.upload_max_filesize,
        inst.post_max_size,
        inst.max_execution_time,
        inst.max_input_vars,
        if inst.display_errors { "On" } else { "Off" },
        inst.disable_functions.join(","),
        if inst.opcache_enabled { 1 } else { 0 },
        inst.opcache_jit_buffer
    )
}

fn render_fpm_pool(inst: &PhpVersionInstance) -> String {
    format!(
        "; Managed by FerrousPanel — PHP {} FPM www.conf\n\
         [www]\n\
         user = www-data\n\
         group = www-data\n\
         listen = {}\n\
         listen.owner = www-data\n\
         listen.group = www-data\n\
         pm = dynamic\n\
         pm.max_children = {}\n\
         pm.start_servers = 4\n\
         pm.min_spare_servers = 2\n\
         pm.max_spare_servers = 8\n",
        inst.version, inst.fpm_socket, inst.max_children
    )
}

fn parse_ini_kv<'a>(content: &'a str, target_key: &str) -> Option<&'a str> {
    let mut result = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') || trimmed.starts_with('[') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            if k.trim().eq_ignore_ascii_case(target_key) {
                let clean = v.split(';').next().unwrap_or(v).trim().trim_matches('"');
                if !clean.is_empty() {
                    result = Some(clean);
                }
            }
        }
    }
    result
}

fn sync_php_ini_from_disk(inst: &mut PhpVersionInstance) {
    let override_ini = PathBuf::from(format!("/etc/php/{}/fpm/conf.d/99-ferrous.ini", inst.version));
    let main_ini = PathBuf::from(format!("/etc/php/{}/fpm/php.ini", inst.version));
    let pool_conf = PathBuf::from(format!("/etc/php/{}/fpm/pool.d/www.conf", inst.version));

    for path in [&main_ini, &override_ini] {
        if let Ok(raw) = std::fs::read_to_string(path) {
            if let Some(v) = parse_ini_kv(&raw, "memory_limit") {
                inst.memory_limit = v.to_string();
            }
            if let Some(v) = parse_ini_kv(&raw, "upload_max_filesize") {
                inst.upload_max_filesize = v.to_string();
            }
            if let Some(v) = parse_ini_kv(&raw, "post_max_size") {
                inst.post_max_size = v.to_string();
            }
            if let Some(v) = parse_ini_kv(&raw, "max_execution_time").and_then(|s| s.parse::<u32>().ok()) {
                inst.max_execution_time = v;
            }
            if let Some(v) = parse_ini_kv(&raw, "max_input_vars").and_then(|s| s.parse::<u32>().ok()) {
                inst.max_input_vars = v;
            }
            if let Some(v) = parse_ini_kv(&raw, "display_errors") {
                inst.display_errors = matches!(v.to_ascii_lowercase().as_str(), "on" | "1" | "true");
            }
            if let Some(v) = parse_ini_kv(&raw, "opcache.enable") {
                inst.opcache_enabled = matches!(v.to_ascii_lowercase().as_str(), "on" | "1" | "true");
            }
            if let Some(v) = parse_ini_kv(&raw, "opcache.jit_buffer_size") {
                inst.opcache_jit_buffer = v.to_string();
            }
            if let Some(v) = parse_ini_kv(&raw, "disable_functions") {
                inst.disable_functions = v
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
            }
        }
    }

    if let Ok(raw_override) = std::fs::read_to_string(&override_ini) {
        if !raw_override.trim().is_empty() && raw_override.len() <= 64 * 1024 {
            inst.raw_php_ini = raw_override;
        }
    }

    if let Ok(raw_pool) = std::fs::read_to_string(&pool_conf) {
        if let Some(v) = parse_ini_kv(&raw_pool, "pm.max_children").and_then(|s| s.parse::<u32>().ok()) {
            inst.max_children = v;
        }
        if !raw_pool.trim().is_empty() && raw_pool.len() <= 64 * 1024 {
            inst.raw_fpm_pool_conf = raw_pool;
        }
    }
}

async fn probe_php_loaded_modules(
    executor: &dyn CommandExecutor,
    version: &str,
) -> Option<HashSet<String>> {
    let candidates = [
        AllowedBinary::php_fpm_for_version(version),
        AllowedBinary::php_cli_for_version(version),
    ];
    for maybe_bin in candidates {
        let Some(bin) = maybe_bin else {
            continue;
        };
        if !executor.is_binary_available(bin) {
            continue;
        }
        if let Ok(out) = executor.run(bin, &["-m"], Duration::from_secs(4)).await {
            if out.success && !out.stdout.trim().is_empty() {
                let mut set = HashSet::new();
                for line in out.stdout.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.starts_with('[') {
                        continue;
                    }
                    let lower = trimmed.to_ascii_lowercase();
                    if lower == "zend opcache" {
                        set.insert("opcache".to_string());
                    }
                    set.insert(lower);
                }
                return Some(set);
            }
        }
    }

    // Fallback: inspect /etc/php/{version}/fpm/conf.d/*.ini symlinks
    let conf_d = PathBuf::from(format!("/etc/php/{version}/fpm/conf.d"));
    let entries = std::fs::read_dir(&conf_d).ok()?;
    let mut set = HashSet::new();
    for entry in entries.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            if let Some(stem) = name.strip_suffix(".ini") {
                let mod_part = stem
                    .split_once('-')
                    .map(|(_, rest)| rest)
                    .unwrap_or(stem)
                    .to_ascii_lowercase();
                set.insert(mod_part);
            }
        }
    }
    Some(set)
}

fn count_live_php_fpm_workers(version: &str) -> u32 {
    let Ok(proc_dir) = std::fs::read_dir("/proc") else {
        return 0;
    };
    let mut count = 0u32;
    let needle_fpm = format!("php-fpm{version}");
    for entry in proc_dir.flatten() {
        let Some(pid_str) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !pid_str.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let cmdline_path = entry.path().join("cmdline");
        if let Ok(raw) = std::fs::read(&cmdline_path) {
            let text = String::from_utf8_lossy(&raw);
            if text.contains("php-fpm: pool") {
                // Verify parent/binary belongs to this PHP version via /proc/<pid>/exe
                if let Ok(exe) = std::fs::read_link(entry.path().join("exe")) {
                    if exe.to_string_lossy().contains(&needle_fpm) {
                        count = count.saturating_add(1);
                    }
                }
            }
        }
    }
    count
}

fn default_php_versions(prod: bool) -> Vec<PhpVersionInstance> {
    let specs = [
        ("8.4", "8.4.x", !prod),
        ("8.3", "8.3.x", !prod),
        ("8.2", "8.2.x", false),
        ("8.1", "8.1.x", false),
        ("7.4", "7.4.x", false),
    ];
    let mut list = Vec::with_capacity(specs.len());
    for (ver, full, test_installed) in specs {
        let fpm_bin = format!("/usr/sbin/php-fpm{ver}");
        let sock_exists = Path::new(&format!("/run/php/php{ver}-fpm.sock")).exists();
        let installed = if prod {
            Path::new(&fpm_bin).is_file() || sock_exists
        } else {
            test_installed || sock_exists
        };
        let status = if sock_exists || (!prod && test_installed) {
            "running"
        } else {
            "stopped"
        };
        let mut inst = PhpVersionInstance {
            version: ver.to_string(),
            full_version: if installed {
                format!("PHP {full} (cli/fpm-fcgi)")
            } else {
                format!("PHP {ver} (not installed)")
            },
            status: status.to_string(),
            installed,
            fpm_socket: format!("/run/php/php{ver}-fpm.sock"),
            ini_path: format!("/etc/php/{ver}/fpm/php.ini"),
            pool_path: format!("/etc/php/{ver}/fpm/pool.d/www.conf"),
            memory_usage_mb: 0.0,
            active_workers: 0,
            max_children: 32,
            memory_limit: "256M".to_string(),
            upload_max_filesize: "64M".to_string(),
            post_max_size: "64M".to_string(),
            max_execution_time: 60,
            max_input_vars: 3000,
            display_errors: false,
            opcache_enabled: true,
            opcache_jit_buffer: "128M".to_string(),
            disable_functions: vec![
                "exec".to_string(),
                "passthru".to_string(),
                "shell_exec".to_string(),
                "system".to_string(),
                "proc_open".to_string(),
                "popen".to_string(),
            ],
            extensions: default_php_extensions(prod),
            raw_php_ini: String::new(),
            raw_fpm_pool_conf: String::new(),
        };
        inst.raw_php_ini = render_php_ini(&inst);
        inst.raw_fpm_pool_conf = render_fpm_pool(&inst);
        list.push(inst);
    }
    list
}

fn require_command_ok(binary: &'static str, out: crate::system::cmd::CmdOutput) -> Result<(), PanelError> {
    if out.success {
        return Ok(());
    }
    let stderr = if out.stderr.trim().is_empty() {
        out.stdout
    } else {
        out.stderr
    };
    Err(PanelError::CommandFailed {
        binary,
        exit_code: out.exit_code,
        stderr,
    })
}

pub async fn list_php_versions(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
) -> Result<Vec<PhpVersionInstance>, PanelError> {
    let prod = is_production_vps(config);
    let mut list = if let Some(existing) = store.get_json::<Vec<PhpVersionInstance>>(KV_PHP_VERSIONS)? {
        existing
    } else {
        let initial = default_php_versions(prod);
        store.put_json(KV_PHP_VERSIONS, &initial)?;
        initial
    };

    if prod {
        let has_systemctl = executor.is_binary_available(AllowedBinary::Systemctl);
        for inst in &mut list {
            let fpm_bin_path = format!("/usr/sbin/php-fpm{}", inst.version);
            let sock_exists = Path::new(&inst.fpm_socket).exists();
            let fpm_bin_enum = AllowedBinary::php_fpm_for_version(&inst.version);
            let cli_bin_enum = AllowedBinary::php_cli_for_version(&inst.version);
            let installed = Path::new(&fpm_bin_path).is_file()
                || sock_exists
                || fpm_bin_enum.map(|b| executor.is_binary_available(b)).unwrap_or(false);
            inst.installed = installed;

            if installed {
                // 1. Live full PHP version from php-fpm{ver} -v or php{ver} -v
                let live_ver = if let Some(bin) = fpm_bin_enum {
                    probe_binary_version_line(executor, bin, &["-v"]).await
                } else {
                    None
                }
                .or(if let Some(bin) = cli_bin_enum {
                    probe_binary_version_line(executor, bin, &["-v"]).await
                } else {
                    None
                });
                if let Some(ver_line) = live_ver {
                    inst.full_version = ver_line;
                }

                // 2. Live loaded extensions from php-fpm{ver} -m / php{ver} -m
                if let Some(loaded) = probe_php_loaded_modules(executor, &inst.version).await {
                    for ext in &mut inst.extensions {
                        let key = ext.name.to_ascii_lowercase();
                        ext.enabled = loaded.contains(&key);
                    }
                }

                // 3. Live php.ini & www.conf values from disk
                sync_php_ini_from_disk(inst);

                // 4. Live systemd active status, cgroup RAM MB, and worker process count
                if has_systemctl {
                    let unit = format!("php{}-fpm.service", inst.version);
                    let active = executor
                        .run(AllowedBinary::Systemctl, &["is-active", &unit], CMD_TIMEOUT)
                        .await
                        .map(|o| o.success && o.stdout.trim() == "active")
                        .unwrap_or(false);
                    inst.status = if active {
                        "running".to_string()
                    } else {
                        "stopped".to_string()
                    };
                    if active {
                        inst.memory_usage_mb = query_systemd_memory_mb(executor, &unit).await;
                        let proc_workers = count_live_php_fpm_workers(&inst.version);
                        inst.active_workers = if proc_workers > 0 {
                            proc_workers
                        } else {
                            query_systemd_property(executor, &unit, "TasksCurrent")
                                .await
                                .and_then(|s| s.parse::<u32>().ok())
                                .map(|t| t.saturating_sub(1))
                                .unwrap_or(0)
                        };
                    } else {
                        inst.active_workers = 0;
                        inst.memory_usage_mb = 0.0;
                    }
                } else {
                    inst.status = if sock_exists { "running" } else { "stopped" }.to_string();
                    inst.active_workers = if sock_exists {
                        count_live_php_fpm_workers(&inst.version)
                    } else {
                        0
                    };
                    inst.memory_usage_mb = 0.0;
                }
            } else {
                inst.full_version = format!("PHP {} (not installed)", inst.version);
                inst.status = "stopped".to_string();
                inst.active_workers = 0;
                inst.memory_usage_mb = 0.0;
                for ext in &mut inst.extensions {
                    ext.enabled = false;
                }
            }
        }
        store.put_json(KV_PHP_VERSIONS, &list)?;
    }

    Ok(list)
}

pub async fn php_action(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    version: &str,
    action: &str,
) -> Result<Vec<PhpVersionInstance>, PanelError> {
    let mut list = list_php_versions(store, config, executor).await?;
    let Some(inst) = list.iter_mut().find(|p| p.version == version) else {
        return Err(PanelError::NotFound(format!(
            "PHP version '{version}' not found"
        )));
    };

    let prod = is_production_vps(config);
    let unit = format!("php{version}-fpm.service");

    match action {
        "install" => {
            if prod {
                if !executor.is_binary_available(AllowedBinary::AptGet) {
                    return Err(PanelError::Conflict(format!(
                        "thiếu binary apt-get để cài PHP {version}; hãy cài thủ công: sudo apt install -y php{version}-fpm php{version}-cli"
                    )));
                }
                apt_update_quiet(executor).await;
                let pkg_fpm = format!("php{version}-fpm");
                let pkg_cli = format!("php{version}-cli");
                let pkg_common = format!("php{version}-common");
                let pkg_mysql = format!("php{version}-mysql");
                let pkg_mb = format!("php{version}-mbstring");
                let pkg_xml = format!("php{version}-xml");
                let pkg_curl = format!("php{version}-curl");
                let pkg_zip = format!("php{version}-zip");
                let res = apt_install_quiet(
                    executor,
                    &[
                        &pkg_fpm,
                        &pkg_cli,
                        &pkg_common,
                        &pkg_mysql,
                        &pkg_mb,
                        &pkg_xml,
                        &pkg_curl,
                        &pkg_zip,
                    ],
                )
                .await?;
                require_command_ok("apt-get", res)?;

                if !executor.is_binary_available(AllowedBinary::Systemctl) {
                    return Err(PanelError::Conflict(format!(
                        "đã cài gói php{version}-fpm nhưng thiếu systemctl để bật dịch vụ"
                    )));
                }
                let _ = std::fs::create_dir_all("/run/php");
                let en = executor
                    .run(AllowedBinary::Systemctl, &["enable", "--now", &unit], CMD_TIMEOUT)
                    .await?;
                require_command_ok("systemctl", en)?;

                let active = executor
                    .run(AllowedBinary::Systemctl, &["is-active", &unit], CMD_TIMEOUT)
                    .await?;
                if !active.success || active.stdout.trim() != "active" {
                    return Err(PanelError::Conflict(format!(
                        "dịch vụ {unit} chưa ở trạng thái active sau khi cài đặt: {}",
                        active.stdout.trim()
                    )));
                }
                let sock_path = Path::new(&inst.fpm_socket);
                let mut sock_ready = sock_path.exists();
                for _ in 0..10 {
                    if sock_ready {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    sock_ready = sock_path.exists();
                }
                if !sock_ready {
                    return Err(PanelError::Conflict(format!(
                        "dịch vụ {unit} đã active nhưng vẫn thiếu socket '{}'. Kiểm tra /etc/php/{version}/fpm/pool.d/www.conf.",
                        inst.fpm_socket
                    )));
                }
            }
            inst.installed = true;
            inst.status = "running".to_string();
        }
        "uninstall" => {
            if prod {
                if !executor.is_binary_available(AllowedBinary::AptGet) {
                    return Err(PanelError::Conflict(
                        "thiếu binary apt-get để gỡ cài đặt PHP-FPM".to_string(),
                    ));
                }
                if executor.is_binary_available(AllowedBinary::Systemctl) {
                    let is_active = executor
                        .run(AllowedBinary::Systemctl, &["is-active", &unit], CMD_TIMEOUT)
                        .await
                        .map(|o| o.success && o.stdout.trim() == "active")
                        .unwrap_or(false);
                    if is_active {
                        let dis = executor
                            .run(AllowedBinary::Systemctl, &["disable", "--now", &unit], CMD_TIMEOUT)
                            .await?;
                        require_command_ok("systemctl", dis)?;
                    }
                }
                let pkg_fpm = format!("php{version}-fpm");
                let res = apt_remove_quiet(executor, &[&pkg_fpm]).await?;
                require_command_ok("apt-get", res)?;
            }
            inst.installed = false;
            inst.status = "stopped".to_string();
            inst.active_workers = 0;
            inst.memory_usage_mb = 0.0;
        }
        "start" | "restart" | "reload" => {
            if prod {
                if !executor.is_binary_available(AllowedBinary::Systemctl) {
                    return Err(PanelError::Conflict(format!(
                        "thiếu systemctl để thực hiện '{action}' trên {unit}"
                    )));
                }
                if !inst.installed {
                    return Err(PanelError::Conflict(format!(
                        "PHP {version}-FPM chưa được cài đặt trên máy chủ (gợi ý: sudo apt install -y php{version}-fpm)"
                    )));
                }
                if let Some(fpm_bin) = AllowedBinary::php_fpm_for_version(version) {
                    if executor.is_binary_available(fpm_bin) {
                        let check = executor.run(fpm_bin, &["-t"], CMD_TIMEOUT).await?;
                        require_command_ok(fpm_bin.label(), check)?;
                    }
                }
                let _ = std::fs::create_dir_all("/run/php");
                let res = executor
                    .run(AllowedBinary::Systemctl, &[action, &unit], CMD_TIMEOUT)
                    .await?;
                require_command_ok("systemctl", res)?;

                let verify = executor
                    .run(AllowedBinary::Systemctl, &["is-active", &unit], CMD_TIMEOUT)
                    .await?;
                if !verify.success || verify.stdout.trim() != "active" {
                    return Err(PanelError::Conflict(format!(
                        "kiểm chứng sau '{action}' thất bại: {unit} đang ở trạng thái '{}'",
                        verify.stdout.trim()
                    )));
                }
                let sock_path = Path::new(&inst.fpm_socket);
                let mut sock_ready = sock_path.exists();
                for _ in 0..10 {
                    if sock_ready {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    sock_ready = sock_path.exists();
                }
                if !sock_ready {
                    return Err(PanelError::Conflict(format!(
                        "{unit} đã active sau '{action}' nhưng thiếu socket '{}'",
                        inst.fpm_socket
                    )));
                }
            }
            inst.installed = true;
            inst.status = "running".to_string();
        }
        "stop" => {
            if prod {
                if !executor.is_binary_available(AllowedBinary::Systemctl) {
                    return Err(PanelError::Conflict(format!(
                        "thiếu systemctl để dừng {unit}"
                    )));
                }
                let res = executor
                    .run(AllowedBinary::Systemctl, &["stop", &unit], CMD_TIMEOUT)
                    .await?;
                require_command_ok("systemctl", res)?;
            }
            inst.status = "stopped".to_string();
            inst.active_workers = 0;
            inst.memory_usage_mb = 0.0;
        }
        other => {
            return Err(PanelError::Validation(format!(
                "unsupported PHP action '{other}'"
            )));
        }
    }

    store.put_json(KV_PHP_VERSIONS, &list)?;
    if prod {
        return list_php_versions(store, config, executor).await;
    }
    Ok(list)
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhpSettingsPatch {
    pub max_children: Option<u32>,
    pub memory_limit: Option<String>,
    pub upload_max_filesize: Option<String>,
    pub post_max_size: Option<String>,
    pub max_execution_time: Option<u32>,
    pub max_input_vars: Option<u32>,
    pub display_errors: Option<bool>,
    pub opcache_enabled: Option<bool>,
    pub opcache_jit_buffer: Option<String>,
    pub disable_functions: Option<Vec<String>>,
    pub raw_php_ini: Option<String>,
    pub raw_fpm_pool_conf: Option<String>,
}

pub async fn update_php_settings(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    version: &str,
    patch: PhpSettingsPatch,
) -> Result<Vec<PhpVersionInstance>, PanelError> {
    let mut list = list_php_versions(store, config, executor).await?;
    let Some(inst) = list.iter_mut().find(|p| p.version == version) else {
        return Err(PanelError::NotFound(format!(
            "PHP version '{version}' not found"
        )));
    };

    let pool_changed = patch.max_children.is_some() || patch.raw_fpm_pool_conf.is_some();

    if let Some(v) = patch.max_children {
        inst.max_children = v.clamp(2, 512);
    }
    if let Some(v) = patch.memory_limit {
        inst.memory_limit = validate_php_ini_value(&v, "memory_limit")?;
    }
    if let Some(v) = patch.upload_max_filesize {
        inst.upload_max_filesize = validate_php_ini_value(&v, "upload_max_filesize")?;
    }
    if let Some(v) = patch.post_max_size {
        inst.post_max_size = validate_php_ini_value(&v, "post_max_size")?;
    }
    if let Some(v) = patch.max_execution_time {
        inst.max_execution_time = v;
    }
    if let Some(v) = patch.max_input_vars {
        inst.max_input_vars = v;
    }
    if let Some(v) = patch.display_errors {
        inst.display_errors = v;
    }
    if let Some(v) = patch.opcache_enabled {
        inst.opcache_enabled = v;
    }
    if let Some(v) = patch.opcache_jit_buffer {
        inst.opcache_jit_buffer = validate_php_ini_value(&v, "opcache_jit_buffer")?;
    }
    if let Some(v) = patch.disable_functions {
        for fn_name in &v {
            if fn_name.is_empty()
                || !fn_name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            {
                return Err(PanelError::Validation(format!(
                    "invalid PHP disable_functions entry '{fn_name}'"
                )));
            }
        }
        inst.disable_functions = v;
    }

    inst.raw_php_ini = patch.raw_php_ini.unwrap_or_else(|| render_php_ini(inst));
    inst.raw_fpm_pool_conf = patch
        .raw_fpm_pool_conf
        .unwrap_or_else(|| render_fpm_pool(inst));

    let php_dir = config.data_dir.join("php").join(version);
    std::fs::create_dir_all(&php_dir)?;
    atomic_write_file(&php_dir.join("php.ini"), inst.raw_php_ini.as_bytes(), 0o644)?;
    atomic_write_file(
        &php_dir.join("www.conf"),
        inst.raw_fpm_pool_conf.as_bytes(),
        0o644,
    )?;

    if is_production_vps(config) {
        let sys_conf_dir = PathBuf::from(format!("/etc/php/{version}/fpm/conf.d"));
        if !sys_conf_dir.is_dir() {
            return Err(PanelError::Conflict(format!(
                "không tìm thấy thư mục cấu hình {} (gợi ý: sudo apt install -y php{version}-fpm)",
                sys_conf_dir.display()
            )));
        }
        let ini_target = sys_conf_dir.join("99-ferrous.ini");
        let previous_ini = std::fs::read(&ini_target).ok();
        atomic_write_file(&ini_target, inst.raw_php_ini.as_bytes(), 0o644)?;

        let pool_target = PathBuf::from(format!("/etc/php/{version}/fpm/pool.d/www.conf"));
        let previous_pool = if pool_changed && pool_target.parent().map(Path::is_dir).unwrap_or(false) {
            let prev = std::fs::read(&pool_target).ok();
            atomic_write_file(&pool_target, inst.raw_fpm_pool_conf.as_bytes(), 0o644)?;
            Some(prev)
        } else {
            None
        };

        let rollback_files = |prev_i: &Option<Vec<u8>>, prev_p: &Option<Option<Vec<u8>>>| {
            if let Some(bytes) = prev_i {
                let _ = atomic_write_file(&ini_target, bytes, 0o644);
            } else {
                let _ = std::fs::remove_file(&ini_target);
            }
            if let Some(Some(bytes)) = prev_p {
                let _ = atomic_write_file(&pool_target, bytes, 0o644);
            }
        };

        if let Some(fpm_bin) = AllowedBinary::php_fpm_for_version(version) {
            if executor.is_binary_available(fpm_bin) {
                let check = executor.run(fpm_bin, &["-t"], CMD_TIMEOUT).await;
                match check {
                    Ok(out) if out.success => {}
                    Ok(out) => {
                        rollback_files(&previous_ini, &previous_pool);
                        let detail = if out.stderr.trim().is_empty() {
                            out.stdout
                        } else {
                            out.stderr
                        };
                        return Err(PanelError::Conflict(format!(
                            "php-fpm{version} -t kiểm tra cấu hình thất bại (đã rollback): {detail}"
                        )));
                    }
                    Err(err) => {
                        rollback_files(&previous_ini, &previous_pool);
                        return Err(err);
                    }
                }
            }
        }

        if !executor.is_binary_available(AllowedBinary::Systemctl) {
            rollback_files(&previous_ini, &previous_pool);
            return Err(PanelError::Conflict(format!(
                "thiếu systemctl để reload php{version}-fpm.service (đã rollback cấu hình)"
            )));
        }
        let unit = format!("php{version}-fpm.service");
        let reload = executor
            .run(AllowedBinary::Systemctl, &["reload-or-restart", &unit], CMD_TIMEOUT)
            .await;
        match reload {
            Ok(out) if out.success => {}
            Ok(out) => {
                rollback_files(&previous_ini, &previous_pool);
                let _ = executor
                    .run(AllowedBinary::Systemctl, &["reload-or-restart", &unit], CMD_TIMEOUT)
                    .await;
                return Err(PanelError::CommandFailed {
                    binary: "systemctl",
                    exit_code: out.exit_code,
                    stderr: if out.stderr.trim().is_empty() {
                        out.stdout
                    } else {
                        out.stderr
                    },
                });
            }
            Err(err) => {
                rollback_files(&previous_ini, &previous_pool);
                return Err(err);
            }
        }
    }

    store.put_json(KV_PHP_VERSIONS, &list)?;
    Ok(list)
}

pub async fn toggle_php_extension(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    version: &str,
    ext_name: &str,
) -> Result<Vec<PhpVersionInstance>, PanelError> {
    let mut list = list_php_versions(store, config, executor).await?;
    let Some(inst) = list.iter_mut().find(|p| p.version == version) else {
        return Err(PanelError::NotFound(format!(
            "PHP version '{version}' not found"
        )));
    };
    let Some(ext) = inst
        .extensions
        .iter_mut()
        .find(|e| e.name.eq_ignore_ascii_case(ext_name.trim()))
    else {
        return Err(PanelError::NotFound(format!(
            "PHP extension '{ext_name}' not found"
        )));
    };
    let target_enabled = !ext.enabled;
    let mod_name = ext.name.to_ascii_lowercase();
    if mod_name.is_empty()
        || mod_name.starts_with('-')
        || !mod_name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(PanelError::Validation(format!(
            "invalid PHP extension name '{mod_name}'"
        )));
    }

    if is_production_vps(config) {
        let bin = if target_enabled {
            AllowedBinary::Phpenmod
        } else {
            AllowedBinary::Phpdismod
        };
        let rollback_bin = if target_enabled {
            AllowedBinary::Phpdismod
        } else {
            AllowedBinary::Phpenmod
        };
        if !executor.is_binary_available(bin) {
            return Err(PanelError::Conflict(format!(
                "thiếu công cụ {} (gợi ý: sudo apt install -y php-common php{version}-fpm)",
                bin.label()
            )));
        }
        let mut mod_out = executor
            .run(bin, &["-v", version, "-s", "fpm", &mod_name], CMD_TIMEOUT)
            .await?;

        // Auto-install missing apt extension package when enabling (e.g. php8.3-redis, php8.3-imagick, php8.3-pgsql)
        if !mod_out.success && target_enabled && executor.is_binary_available(AllowedBinary::AptGet) {
            if let Some(apt_pkg) = apt_package_for_php_ext(version, &mod_name) {
                apt_update_quiet(executor).await;
                let inst_res = apt_install_quiet(executor, &[&apt_pkg]).await?;
                if inst_res.success {
                    mod_out = executor
                        .run(bin, &["-v", version, "-s", "fpm", &mod_name], CMD_TIMEOUT)
                        .await?;
                }
            }
        }
        require_command_ok(bin.label(), mod_out)?;

        if let Some(fpm_bin) = AllowedBinary::php_fpm_for_version(version) {
            if executor.is_binary_available(fpm_bin) {
                let check = executor.run(fpm_bin, &["-t"], CMD_TIMEOUT).await?;
                if !check.success {
                    let _ = executor
                        .run(rollback_bin, &["-v", version, "-s", "fpm", &mod_name], CMD_TIMEOUT)
                        .await;
                    return Err(PanelError::Conflict(format!(
                        "php-fpm{version} -t thất bại sau khi đổi extension '{mod_name}' (đã rollback): {}",
                        if check.stderr.trim().is_empty() {
                            check.stdout
                        } else {
                            check.stderr
                        }
                    )));
                }
            }
        }

        if !executor.is_binary_available(AllowedBinary::Systemctl) {
            let _ = executor
                .run(rollback_bin, &["-v", version, "-s", "fpm", &mod_name], CMD_TIMEOUT)
                .await;
            return Err(PanelError::Conflict(format!(
                "thiếu systemctl để reload php{version}-fpm.service (đã rollback)"
            )));
        }
        let unit = format!("php{version}-fpm.service");
        let rel = executor
            .run(AllowedBinary::Systemctl, &["reload", &unit], CMD_TIMEOUT)
            .await?;
        if !rel.success {
            let _ = executor
                .run(rollback_bin, &["-v", version, "-s", "fpm", &mod_name], CMD_TIMEOUT)
                .await;
            return Err(PanelError::CommandFailed {
                binary: "systemctl",
                exit_code: rel.exit_code,
                stderr: if rel.stderr.trim().is_empty() {
                    rel.stdout
                } else {
                    rel.stderr
                },
            });
        }
    }

    ext.enabled = target_enabled;
    store.put_json(KV_PHP_VERSIONS, &list)?;
    Ok(list)
}

pub async fn switch_site_php_version(
    store: &ModuleStore,
    config: &Config,
    webserver: &WebServerManager,
    app_id: &str,
    php_version: &str,
) -> Result<(ManagedApp, Vec<PhpVersionInstance>), PanelError> {
    let mut app = store
        .get_app(app_id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{app_id}' not found")))?;

    let tag = match php_version.trim() {
        "8.4" => PhpVersionTag::V84,
        "8.3" => PhpVersionTag::V83,
        "8.2" => PhpVersionTag::V82,
        "8.1" => PhpVersionTag::V81,
        "7.4" => PhpVersionTag::V74,
        "None" | "none" | "" => PhpVersionTag::None,
        other => {
            return Err(PanelError::Validation(format!(
                "unsupported PHP version tag '{other}'"
            )));
        }
    };
    app.php_version = Some(tag);

    if matches!(
        app.framework,
        WebsiteRuntimeType::Php83Fpm | WebsiteRuntimeType::PhpSymfony7
    ) && !matches!(tag, PhpVersionTag::None)
    {
        let fpm_ver = match tag {
            PhpVersionTag::V84 => PhpFpmVersion::V84,
            PhpVersionTag::V83 | PhpVersionTag::None => PhpFpmVersion::V83,
            PhpVersionTag::V82 => PhpFpmVersion::V82,
            PhpVersionTag::V81 => PhpFpmVersion::V81,
            PhpVersionTag::V74 => PhpFpmVersion::V74,
        };
        if is_production_vps(config) {
            let sock = fpm_ver.socket_path();
            if !fpm_ver.socket_exists() {
                if webserver.executor().is_binary_available(fpm_ver.binary())
                    && webserver
                        .executor()
                        .is_binary_available(AllowedBinary::Systemctl)
                {
                    let _ = std::fs::create_dir_all("/run/php");
                    let _ = webserver
                        .executor()
                        .run(
                            AllowedBinary::Systemctl,
                            &["enable", "--now", fpm_ver.systemd_unit()],
                            CMD_TIMEOUT,
                        )
                        .await;
                    let _ = fpm_ver.wait_for_socket().await;
                }
                if !fpm_ver.socket_exists() {
                    return Err(PanelError::Conflict(format!(
                        "socket PHP-FPM '{sock}' chưa tồn tại — hãy cài và khởi chạy php{}-fpm trong mục 'PHP-FPM & Phần mềm' trước khi chuyển site",
                        fpm_ver.version_str()
                    )));
                }
            }
        }
        let domain = Domain::parse(&app.domain)?;
        let mut aliases = Vec::new();
        for a in &app.aliases {
            aliases.push(Domain::parse(a)?);
        }
        let sub = app.run_sub_dir.trim().trim_start_matches('/').trim_end_matches('/');
        let effective_rel = if !sub.is_empty() && !sub.contains("..") && !sub.contains('\0') {
            let base = SafePath::under_root(&config.wwwroot, &app.working_dir)?;
            let candidate = base.as_path().join(sub);
            if candidate.is_dir() && candidate.starts_with(base.as_path()) {
                format!("{}/{sub}", app.working_dir.trim_end_matches('/'))
            } else {
                app.working_dir.clone()
            }
        } else {
            app.working_dir.clone()
        };
        let root = crate::webserver::WwwRootPath::under_root(&config.wwwroot, &effective_rel)?;
        let redirects = app
            .redirects
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|r| crate::webserver::SiteRedirectRule {
                source_path: r.source_path.clone(),
                target_url: r.target_url.clone(),
                status_code: r.redirect_type.trim().parse::<u16>().unwrap_or(301),
                keep_query: r.keep_query,
                enabled: r.enabled,
            })
            .collect();
        let hotlink_protection = app.hotlink_protection.as_ref().map(|h| {
            crate::webserver::SiteHotlinkProtection {
                enabled: h.enabled,
                protected_extensions: h.protected_extensions.clone(),
                allowed_domains: h.allowed_domains.clone(),
                block_empty_referer: h.block_empty_referer,
            }
        });
        let directory_auth = app
            .directory_auth
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|d| crate::webserver::SiteDirectoryAuth {
                path: d.path.clone(),
                label: d.label.clone(),
                username: d.username.clone(),
                password: d.password.clone(),
                enabled: d.enabled,
            })
            .collect();

        let spec = crate::webserver::SiteSpec {
            domain,
            aliases,
            root,
            mode: crate::webserver::SiteMode::PhpFpm {
                php_version: fpm_ver,
                index_file: SafeFilename::parse("index.php")?,
            },
            ssl_enabled: app.ssl_enabled,
            redirect_https: app.ssl_enabled && app.force_https,
            http3_quic: app.http3_quic,
            websocket_enabled: app.websocket_enabled,
            rate_limit_rps: app.rate_limit_rps,
            redirects,
            hotlink_protection,
            directory_auth,
        };
        let rendered = webserver.upsert_and_apply_site(&spec, None).await?;
        app.url_rewrite_rules = rendered.content;
        app.nginx_vhost = rendered.config_path.display().to_string();
    }

    store.upsert_app(&app)?;
    let enriched = crate::modules::apps::enrich_app(app, config, webserver).await;
    let php_versions = list_php_versions(store, config, webserver.executor().as_ref()).await?;
    Ok((enriched, php_versions))
}
