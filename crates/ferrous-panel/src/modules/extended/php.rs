use super::common::*;

// ============================================================================
// 3. Multi-PHP FPM Pools
// ============================================================================

fn default_php_extensions() -> Vec<PhpExtensionItem> {
    vec![
        PhpExtensionItem {
            name: "opcache".to_string(),
            category: "Performance".to_string(),
            enabled: true,
            description: "Zend OPcache bytecode accelerator + Tracing JIT compiler.".to_string(),
        },
        PhpExtensionItem {
            name: "pdo_mysql".to_string(),
            category: "Database".to_string(),
            enabled: true,
            description: "MariaDB / MySQL native PDO driver (mysqlnd).".to_string(),
        },
        PhpExtensionItem {
            name: "pdo_pgsql".to_string(),
            category: "Database".to_string(),
            enabled: true,
            description: "PostgreSQL native libpq PDO driver.".to_string(),
        },
        PhpExtensionItem {
            name: "redis".to_string(),
            category: "Cache".to_string(),
            enabled: true,
            description: "PhpRedis C extension for high-speed session & object caching.".to_string(),
        },
        PhpExtensionItem {
            name: "curl".to_string(),
            category: "Network".to_string(),
            enabled: true,
            description: "libcurl HTTP/TLS client bindings.".to_string(),
        },
        PhpExtensionItem {
            name: "mbstring".to_string(),
            category: "Core".to_string(),
            enabled: true,
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
            enabled: true,
            description: "Zend Foreign Function Interface for loading Rust C-ABI shared libraries (.so).".to_string(),
        },
    ]
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

fn default_php_versions() -> Vec<PhpVersionInstance> {
    let specs = [
        ("8.4", "8.4.4", true, "running", 42.5, 4),
        ("8.3", "8.3.17", true, "running", 38.2, 6),
        ("8.2", "8.2.27", true, "stopped", 0.0, 0),
        ("8.1", "8.1.31", false, "stopped", 0.0, 0),
        ("7.4", "7.4.33", false, "stopped", 0.0, 0),
    ];
    let mut list = Vec::with_capacity(specs.len());
    for (ver, full, installed, status, mem, workers) in specs {
        let sock_exists = Path::new(&format!("/run/php/php{ver}-fpm.sock")).exists();
        let mut inst = PhpVersionInstance {
            version: ver.to_string(),
            full_version: format!("PHP {full} (cli/fpm-fcgi) + Zend OPcache JIT"),
            status: if sock_exists { "running".to_string() } else { status.to_string() },
            installed: installed || sock_exists,
            fpm_socket: format!("/run/php/php{ver}-fpm.sock"),
            ini_path: format!("/etc/php/{ver}/fpm/php.ini"),
            pool_path: format!("/etc/php/{ver}/fpm/pool.d/www.conf"),
            memory_usage_mb: mem,
            active_workers: workers,
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
            extensions: default_php_extensions(),
            raw_php_ini: String::new(),
            raw_fpm_pool_conf: String::new(),
        };
        inst.raw_php_ini = render_php_ini(&inst);
        inst.raw_fpm_pool_conf = render_fpm_pool(&inst);
        list.push(inst);
    }
    list
}

pub async fn list_php_versions(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
) -> Result<Vec<PhpVersionInstance>, PanelError> {
    let mut list = if let Some(existing) = store.get_json::<Vec<PhpVersionInstance>>(KV_PHP_VERSIONS)? {
        existing
    } else {
        let initial = default_php_versions();
        store.put_json(KV_PHP_VERSIONS, &initial)?;
        initial
    };

    if is_production_vps(config) {
        let has_systemctl = executor.is_binary_available(AllowedBinary::Systemctl);
        for inst in &mut list {
            let fpm_bin = format!("/usr/sbin/php-fpm{}", inst.version);
            let cli_bin = format!("/usr/bin/php{}", inst.version);
            let installed = Path::new(&fpm_bin).is_file() || Path::new(&cli_bin).is_file();
            inst.installed = installed;
            if installed && has_systemctl {
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
                if !active {
                    inst.active_workers = 0;
                    inst.memory_usage_mb = 0.0;
                } else if inst.active_workers == 0 {
                    inst.active_workers = 2;
                }
            } else if !installed {
                inst.status = "stopped".to_string();
                inst.active_workers = 0;
                inst.memory_usage_mb = 0.0;
            }
        }
        let _ = store.put_json(KV_PHP_VERSIONS, &list);
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
            if prod && executor.is_binary_available(AllowedBinary::AptGet) {
                let pkg_fpm = format!("php{version}-fpm");
                let pkg_cli = format!("php{version}-cli");
                let pkg_common = format!("php{version}-common");
                let pkg_mysql = format!("php{version}-mysql");
                let pkg_mb = format!("php{version}-mbstring");
                let pkg_xml = format!("php{version}-xml");
                let pkg_curl = format!("php{version}-curl");
                let pkg_zip = format!("php{version}-zip");
                let res = executor
                    .run_with_env(
                        AllowedBinary::AptGet,
                        &[
                            "install",
                            "-y",
                            "-qq",
                            &pkg_fpm,
                            &pkg_cli,
                            &pkg_common,
                            &pkg_mysql,
                            &pkg_mb,
                            &pkg_xml,
                            &pkg_curl,
                            &pkg_zip,
                        ],
                        &[("DEBIAN_FRONTEND", "noninteractive")],
                        None,
                        APT_TIMEOUT,
                    )
                    .await?;
                if !res.success {
                    return Err(PanelError::CommandFailed {
                        binary: "apt-get",
                        exit_code: res.exit_code,
                        stderr: if res.stderr.is_empty() { res.stdout } else { res.stderr },
                    });
                }
                if executor.is_binary_available(AllowedBinary::Systemctl) {
                    let _ = executor
                        .run(AllowedBinary::Systemctl, &["enable", "--now", &unit], CMD_TIMEOUT)
                        .await;
                }
            }
            inst.installed = true;
            inst.status = "running".to_string();
            inst.active_workers = 4;
            inst.memory_usage_mb = 36.4;
        }
        "uninstall" => {
            if prod {
                if executor.is_binary_available(AllowedBinary::Systemctl) {
                    let _ = executor
                        .run(AllowedBinary::Systemctl, &["disable", "--now", &unit], CMD_TIMEOUT)
                        .await;
                }
                if executor.is_binary_available(AllowedBinary::AptGet) {
                    let pkg_fpm = format!("php{version}-fpm");
                    let res = executor
                        .run_with_env(
                            AllowedBinary::AptGet,
                            &["remove", "-y", "-qq", &pkg_fpm],
                            &[("DEBIAN_FRONTEND", "noninteractive")],
                            None,
                            APT_TIMEOUT,
                        )
                        .await?;
                    if !res.success {
                        return Err(PanelError::CommandFailed {
                            binary: "apt-get",
                            exit_code: res.exit_code,
                            stderr: if res.stderr.is_empty() { res.stdout } else { res.stderr },
                        });
                    }
                }
            }
            inst.installed = false;
            inst.status = "stopped".to_string();
            inst.active_workers = 0;
            inst.memory_usage_mb = 0.0;
        }
        "start" | "restart" | "reload" => {
            if prod && executor.is_binary_available(AllowedBinary::Systemctl) {
                let res = executor
                    .run(AllowedBinary::Systemctl, &[action, &unit], CMD_TIMEOUT)
                    .await?;
                if !res.success {
                    return Err(PanelError::CommandFailed {
                        binary: "systemctl",
                        exit_code: res.exit_code,
                        stderr: if res.stderr.is_empty() { res.stdout } else { res.stderr },
                    });
                }
            }
            inst.installed = true;
            inst.status = "running".to_string();
            inst.active_workers = 4;
            if inst.memory_usage_mb == 0.0 {
                inst.memory_usage_mb = 38.0;
            }
        }
        "stop" => {
            if prod && executor.is_binary_available(AllowedBinary::Systemctl) {
                let _ = executor
                    .run(AllowedBinary::Systemctl, &["stop", &unit], CMD_TIMEOUT)
                    .await;
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

    if let Some(v) = patch.max_children {
        inst.max_children = v.clamp(2, 512);
    }
    if let Some(v) = patch.memory_limit {
        inst.memory_limit = v;
    }
    if let Some(v) = patch.upload_max_filesize {
        inst.upload_max_filesize = v;
    }
    if let Some(v) = patch.post_max_size {
        inst.post_max_size = v;
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
        inst.opcache_jit_buffer = v;
    }
    if let Some(v) = patch.disable_functions {
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
        if sys_conf_dir.is_dir() {
            let _ = atomic_write_file(
                &sys_conf_dir.join("99-ferrous.ini"),
                inst.raw_php_ini.as_bytes(),
                0o644,
            );
        }
        if executor.is_binary_available(AllowedBinary::Systemctl) {
            let unit = format!("php{version}-fpm.service");
            let _ = executor
                .run(AllowedBinary::Systemctl, &["reload-or-restart", &unit], CMD_TIMEOUT)
                .await;
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

    if is_production_vps(config)
        && mod_name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        let bin = if target_enabled {
            AllowedBinary::Phpenmod
        } else {
            AllowedBinary::Phpdismod
        };
        if executor.is_binary_available(bin) {
            let _ = executor
                .run(bin, &["-v", version, "-s", "fpm", &mod_name], CMD_TIMEOUT)
                .await;
        }
        if executor.is_binary_available(AllowedBinary::Systemctl) {
            let unit = format!("php{version}-fpm.service");
            let _ = executor
                .run(AllowedBinary::Systemctl, &["reload", &unit], CMD_TIMEOUT)
                .await;
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
        let domain = Domain::parse(&app.domain)?;
        let mut aliases = Vec::new();
        for a in &app.aliases {
            aliases.push(Domain::parse(a)?);
        }
        let root = crate::webserver::WwwRootPath::under_root(&config.wwwroot, &app.working_dir)?;
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
        };
        if let Ok(rendered) = webserver.upsert_and_apply_site(&spec, None).await {
            app.url_rewrite_rules = rendered.content;
            app.nginx_vhost = rendered.config_path.display().to_string();
        }
    }

    store.upsert_app(&app)?;
    let enriched = crate::modules::apps::enrich_app(app, config, webserver).await;
    let php_versions = list_php_versions(store, config, webserver.executor().as_ref()).await?;
    Ok((enriched, php_versions))
}

