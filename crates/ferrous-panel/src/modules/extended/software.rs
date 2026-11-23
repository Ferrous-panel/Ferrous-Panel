use super::common::*;

// ============================================================================
// 4. Software Package Installer
// ============================================================================

struct PackageSystemMeta {
    binary: AllowedBinary,
    systemd_unit: &'static str,
    apt_package: Option<&'static str>,
    real_config_path: &'static str,
    is_cli_tool: bool,
}

fn package_system_meta(id: &str) -> Option<PackageSystemMeta> {
    match id {
        "pkg-ferrousx" => Some(PackageSystemMeta {
            binary: AllowedBinary::Ferrousx,
            systemd_unit: "ferrousx.service",
            apt_package: None,
            real_config_path: "/etc/ferrousx/ferrousx.toml",
            is_cli_tool: false,
        }),
        "pkg-bun" => Some(PackageSystemMeta {
            binary: AllowedBinary::Bun,
            systemd_unit: "",
            apt_package: None,
            real_config_path: "/etc/ferrous-panel/bunfig.toml",
            is_cli_tool: true,
        }),
        "pkg-tailwindcss" => Some(PackageSystemMeta {
            binary: AllowedBinary::TailwindCli,
            systemd_unit: "",
            apt_package: None,
            real_config_path: "/etc/ferrous-panel/tailwind.conf",
            is_cli_tool: true,
        }),
        "pkg-deno" => Some(PackageSystemMeta {
            binary: AllowedBinary::Deno,
            systemd_unit: "",
            apt_package: None,
            real_config_path: "/etc/ferrous-panel/deno.json",
            is_cli_tool: true,
        }),
        "pkg-nginx" => Some(PackageSystemMeta {
            binary: AllowedBinary::Nginx,
            systemd_unit: "nginx.service",
            apt_package: Some("nginx"),
            real_config_path: "/etc/nginx/nginx.conf",
            is_cli_tool: false,
        }),
        "pkg-postgresql" => Some(PackageSystemMeta {
            binary: AllowedBinary::Psql,
            systemd_unit: "postgresql.service",
            apt_package: Some("postgresql"),
            real_config_path: "/etc/postgresql/16/main/postgresql.conf",
            is_cli_tool: false,
        }),
        "pkg-mariadb" => Some(PackageSystemMeta {
            binary: AllowedBinary::Mysql,
            systemd_unit: "mariadb.service",
            apt_package: Some("mariadb-server"),
            real_config_path: "/etc/mysql/mariadb.conf.d/50-server.cnf",
            is_cli_tool: false,
        }),
        "pkg-certbot" => Some(PackageSystemMeta {
            binary: AllowedBinary::Certbot,
            systemd_unit: "certbot.timer",
            apt_package: Some("certbot"),
            real_config_path: "/etc/letsencrypt/cli.ini",
            is_cli_tool: false,
        }),
        "pkg-fail2ban" => Some(PackageSystemMeta {
            binary: AllowedBinary::Fail2banClient,
            systemd_unit: "fail2ban.service",
            apt_package: Some("fail2ban"),
            real_config_path: "/etc/fail2ban/jail.local",
            is_cli_tool: false,
        }),
        _ => None,
    }
}

async fn install_zero_node_cli_package(
    id: &str,
    executor: &dyn CommandExecutor,
) -> Result<String, PanelError> {
    if !executor.is_binary_available(AllowedBinary::Curl) {
        return Err(PanelError::BinaryUnavailable("curl"));
    }
    let arch = std::env::consts::ARCH;
    let tmp_dir = PathBuf::from(format!("/tmp/ferrous-pkg-{id}-{}", Utc::now().timestamp()));
    std::fs::create_dir_all(&tmp_dir)?;

    let res: Result<String, PanelError> = match id {
        "pkg-bun" => {
            if !executor.is_binary_available(AllowedBinary::Unzip) {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                return Err(PanelError::BinaryUnavailable("unzip"));
            }
            let bun_arch = if arch == "aarch64" { "aarch64" } else { "x64" };
            let url = format!(
                "https://github.com/oven-sh/bun/releases/latest/download/bun-linux-{bun_arch}.zip"
            );
            let zip_path = tmp_dir.join("bun.zip");
            let zip_str = zip_path.display().to_string();
            let dl = executor
                .run(
                    AllowedBinary::Curl,
                    &["-fsSL", "--retry", "3", "-o", &zip_str, &url],
                    APT_TIMEOUT,
                )
                .await?;
            if !dl.success {
                return Err(PanelError::CommandFailed {
                    binary: "curl",
                    exit_code: dl.exit_code,
                    stderr: dl.stderr,
                });
            }
            let tmp_str = tmp_dir.display().to_string();
            let uz = executor
                .run(AllowedBinary::Unzip, &["-qo", &zip_str, "-d", &tmp_str], APT_TIMEOUT)
                .await?;
            if !uz.success {
                return Err(PanelError::CommandFailed {
                    binary: "unzip",
                    exit_code: uz.exit_code,
                    stderr: uz.stderr,
                });
            }
            let src_bin = tmp_dir.join(format!("bun-linux-{bun_arch}")).join("bun");
            let src_str = src_bin.display().to_string();
            let inst = executor
                .run(
                    AllowedBinary::Install,
                    &["-m", "0755", &src_str, "/usr/local/bin/bun"],
                    CMD_TIMEOUT,
                )
                .await?;
            if !inst.success {
                return Err(PanelError::CommandFailed {
                    binary: "install",
                    exit_code: inst.exit_code,
                    stderr: inst.stderr,
                });
            }
            if !Path::new("/usr/bin/node").exists() && !Path::new("/usr/local/bin/node").exists() {
                let _ = std::os::unix::fs::symlink("/usr/local/bin/bun", "/usr/local/bin/node");
            }
            Ok("Installed standalone Bun native binary -> /usr/local/bin/bun (+ Zero-Node.js compat shim)".to_string())
        }
        "pkg-tailwindcss" => {
            let tw_arch = if arch == "aarch64" { "arm64" } else { "x64" };
            let url = format!(
                "https://github.com/tailwindlabs/tailwindcss/releases/latest/download/tailwindcss-linux-{tw_arch}"
            );
            let dl_path = tmp_dir.join("tailwindcss");
            let dl_str = dl_path.display().to_string();
            let dl = executor
                .run(
                    AllowedBinary::Curl,
                    &["-fsSL", "--retry", "3", "-o", &dl_str, &url],
                    APT_TIMEOUT,
                )
                .await?;
            if !dl.success {
                return Err(PanelError::CommandFailed {
                    binary: "curl",
                    exit_code: dl.exit_code,
                    stderr: dl.stderr,
                });
            }
            let inst = executor
                .run(
                    AllowedBinary::Install,
                    &["-m", "0755", &dl_str, "/usr/local/bin/tailwindcss"],
                    CMD_TIMEOUT,
                )
                .await?;
            if !inst.success {
                return Err(PanelError::CommandFailed {
                    binary: "install",
                    exit_code: inst.exit_code,
                    stderr: inst.stderr,
                });
            }
            Ok("Installed Tailwind CSS v4 standalone binary -> /usr/local/bin/tailwindcss (Rust Oxide, Zero-Node.js)".to_string())
        }
        "pkg-deno" => {
            if !executor.is_binary_available(AllowedBinary::Unzip) {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                return Err(PanelError::BinaryUnavailable("unzip"));
            }
            let deno_target = if arch == "aarch64" {
                "aarch64-unknown-linux-gnu"
            } else {
                "x86_64-unknown-linux-gnu"
            };
            let url = format!(
                "https://github.com/denoland/deno/releases/latest/download/deno-{deno_target}.zip"
            );
            let zip_path = tmp_dir.join("deno.zip");
            let zip_str = zip_path.display().to_string();
            let dl = executor
                .run(
                    AllowedBinary::Curl,
                    &["-fsSL", "--retry", "3", "-o", &zip_str, &url],
                    APT_TIMEOUT,
                )
                .await?;
            if !dl.success {
                return Err(PanelError::CommandFailed {
                    binary: "curl",
                    exit_code: dl.exit_code,
                    stderr: dl.stderr,
                });
            }
            let tmp_str = tmp_dir.display().to_string();
            let uz = executor
                .run(AllowedBinary::Unzip, &["-qo", &zip_str, "-d", &tmp_str], APT_TIMEOUT)
                .await?;
            if !uz.success {
                return Err(PanelError::CommandFailed {
                    binary: "unzip",
                    exit_code: uz.exit_code,
                    stderr: uz.stderr,
                });
            }
            let src_bin = tmp_dir.join("deno");
            let src_str = src_bin.display().to_string();
            let inst = executor
                .run(
                    AllowedBinary::Install,
                    &["-m", "0755", &src_str, "/usr/local/bin/deno"],
                    CMD_TIMEOUT,
                )
                .await?;
            if !inst.success {
                return Err(PanelError::CommandFailed {
                    binary: "install",
                    exit_code: inst.exit_code,
                    stderr: inst.stderr,
                });
            }
            Ok("Installed Deno 2 standalone Rust runtime -> /usr/local/bin/deno (Zero-Node.js)".to_string())
        }
        _ => Err(PanelError::Validation(format!(
            "unsupported CLI package '{id}'"
        ))),
    };

    let _ = std::fs::remove_dir_all(&tmp_dir);
    res
}

fn default_software_packages(executor: &dyn CommandExecutor) -> Vec<ServerPackageItem> {
    let ferrousx_avail = executor.is_binary_available(AllowedBinary::Ferrousx);
    let bun_avail = executor.is_binary_available(AllowedBinary::Bun);
    let tw_avail = executor.is_binary_available(AllowedBinary::TailwindCli);
    let deno_avail = executor.is_binary_available(AllowedBinary::Deno);
    let nginx_avail = executor.is_binary_available(AllowedBinary::Nginx);
    let psql_avail = executor.is_binary_available(AllowedBinary::Psql);
    let mysql_avail = executor.is_binary_available(AllowedBinary::Mysql);
    let certbot_avail = executor.is_binary_available(AllowedBinary::Certbot);
    let f2b_avail = executor.is_binary_available(AllowedBinary::Fail2banClient);

    vec![
        ServerPackageItem {
            id: "pkg-ferrousx".to_string(),
            name: "FerrousX HTTP/TLS & JS/CSS Asset Engine".to_string(),
            category: "Web Server".to_string(),
            version: "2.5.0 (Tokio + Hyper + Rustls)".to_string(),
            description: "Memory-safe HTTP/1.1, HTTP/2, TLS SNI, WebSocket, PHP FastCGI, and native Zero-Node.js Static/SPA server (.br/.gz precompressed + clean URLs + immutable JS/CSS cache).".to_string(),
            installed: ferrousx_avail,
            status: if ferrousx_avail { "stopped" } else { "not_installed" }.to_string(),
            memory_mb: 11.4,
            config_path: "/etc/ferrousx/ferrousx.toml".to_string(),
            is_rust_native: true,
            replaced_legacy_package: Some("Nginx / Apache httpd".to_string()),
            rust_crate: Some("crates/ferrousx (hyper 1 + rustls 0.23)".to_string()),
            port: Some(80),
            config_content: "worker_threads = 0\nbind_http = \"0.0.0.0:80\"\nbind_https = \"0.0.0.0:443\"\nsites_dir = \"/etc/ferrousx/sites\"\nacme_webroot = \"/www/acme\"\nserver_tokens = false\n".to_string(),
            logs: vec!["[ferrousx] listening on 0.0.0.0:80 and 0.0.0.0:443 (JS/CSS SPA + precompressed ready)".to_string()],
        },
        ServerPackageItem {
            id: "pkg-bun".to_string(),
            name: "Bun JS/TS All-in-One Runtime (Zero-Node.js)".to_string(),
            category: "JS/CSS Zero-Node".to_string(),
            version: "1.x Single Binary (No Node.js/npm)".to_string(),
            description: "Single native binary runtime, bundler, and package manager for React, Vue, Next.js, Nuxt, SvelteKit, Astro, Remix, and Vite — replaces Node.js & npm completely.".to_string(),
            installed: bun_avail,
            status: if bun_avail { "running" } else { "not_installed" }.to_string(),
            memory_mb: 0.0,
            config_path: "/etc/ferrous-panel/bunfig.toml".to_string(),
            is_rust_native: false,
            replaced_legacy_package: Some("Node.js + npm + npx + pm2".to_string()),
            rust_crate: Some("Zig + JavaScriptCore single native binary (/usr/local/bin/bun)".to_string()),
            port: None,
            config_content: "[install]\nauto = \"fallback\"\nexact = false\n\n[run]\nbun = true\n".to_string(),
            logs: vec![if bun_avail {
                "[bun] ready at /usr/local/bin/bun (Zero-Node.js JS/TS runtime active)".to_string()
            } else {
                "[bun] not installed yet — click Install or run update.sh to enable Zero-Node.js JS frameworks".to_string()
            }],
        },
        ServerPackageItem {
            id: "pkg-tailwindcss".to_string(),
            name: "Tailwind CSS v4 Standalone CLI (Rust Oxide)".to_string(),
            category: "JS/CSS Zero-Node".to_string(),
            version: "4.x Standalone Native Binary".to_string(),
            description: "Compiles Tailwind CSS v4 stylesheets directly into minified CSS via the Rust Oxide engine with zero Node.js and zero node_modules.".to_string(),
            installed: tw_avail,
            status: if tw_avail { "running" } else { "not_installed" }.to_string(),
            memory_mb: 0.0,
            config_path: "/etc/ferrous-panel/tailwind.conf".to_string(),
            is_rust_native: true,
            replaced_legacy_package: Some("Node.js PostCSS / Webpack CSS pipeline".to_string()),
            rust_crate: Some("tailwindcss-oxide + LightningCSS (/usr/local/bin/tailwindcss)".to_string()),
            port: None,
            config_content: "# FerrousPanel Zero-Node.js Tailwind CSS v4 CLI\n# Auto-compiles input.css -> output.css --minify on Deploy / Rebuild\n".to_string(),
            logs: vec![if tw_avail {
                "[tailwindcss] ready at /usr/local/bin/tailwindcss (Rust Oxide standalone)".to_string()
            } else {
                "[tailwindcss] not installed yet — click Install or run update.sh".to_string()
            }],
        },
        ServerPackageItem {
            id: "pkg-deno".to_string(),
            name: "Deno 2 TypeScript & Fresh Runtime (Rust)".to_string(),
            category: "JS/CSS Zero-Node".to_string(),
            version: "2.x Single Rust Binary".to_string(),
            description: "Secure Rust-powered TypeScript/JavaScript runtime with native Web APIs, Fresh, Hono, and zero Node.js dependency.".to_string(),
            installed: deno_avail,
            status: if deno_avail { "running" } else { "not_installed" }.to_string(),
            memory_mb: 0.0,
            config_path: "/etc/ferrous-panel/deno.json".to_string(),
            is_rust_native: true,
            replaced_legacy_package: Some("Node.js + ts-node".to_string()),
            rust_crate: Some("deno_core + rusty_v8 (/usr/local/bin/deno)".to_string()),
            port: None,
            config_content: "{\n  \"nodeModulesDir\": \"none\"\n}\n".to_string(),
            logs: vec![if deno_avail {
                "[deno] ready at /usr/local/bin/deno (Zero-Node.js Rust runtime)".to_string()
            } else {
                "[deno] optional — click Install to enable Deno 2 / Fresh apps".to_string()
            }],
        },
        ServerPackageItem {
            id: "pkg-nginx".to_string(),
            name: "Nginx 1.26 Web Server".to_string(),
            category: "Web Server".to_string(),
            version: "1.26.2".to_string(),
            description: "Classic event-driven HTTP and reverse proxy server.".to_string(),
            installed: nginx_avail,
            status: if nginx_avail { "running" } else { "stopped" }.to_string(),
            memory_mb: 24.8,
            config_path: "/etc/nginx/nginx.conf".to_string(),
            is_rust_native: false,
            replaced_legacy_package: None,
            rust_crate: None,
            port: Some(80),
            config_content: "user www-data;\nworker_processes auto;\nevents { worker_connections 4096; }\n".to_string(),
            logs: vec!["[nginx] master process /usr/sbin/nginx -g daemon on;".to_string()],
        },
        ServerPackageItem {
            id: "pkg-postgresql".to_string(),
            name: "PostgreSQL 16/17 Relational Engine".to_string(),
            category: "Database".to_string(),
            version: "16/17 LTS".to_string(),
            description: "ACID relational database server with JSONB and MVCC concurrency.".to_string(),
            installed: psql_avail,
            status: if psql_avail { "running" } else { "stopped" }.to_string(),
            memory_mb: 128.0,
            config_path: "/etc/postgresql/16/main/postgresql.conf".to_string(),
            is_rust_native: false,
            replaced_legacy_package: None,
            rust_crate: None,
            port: Some(5432),
            config_content: "listen_addresses = '127.0.0.1'\nport = 5432\nmax_connections = 200\nshared_buffers = 256MB\n".to_string(),
            logs: vec!["[postgresql] database system is ready to accept connections on 127.0.0.1:5432".to_string()],
        },
        ServerPackageItem {
            id: "pkg-mariadb".to_string(),
            name: "MariaDB 11.4 LTS".to_string(),
            category: "Database".to_string(),
            version: "11.4.4".to_string(),
            description: "MySQL-compatible relational database with InnoDB buffer pool tuning.".to_string(),
            installed: mysql_avail,
            status: if mysql_avail { "running" } else { "stopped" }.to_string(),
            memory_mb: 164.0,
            config_path: "/etc/mysql/mariadb.conf.d/50-server.cnf".to_string(),
            is_rust_native: false,
            replaced_legacy_package: None,
            rust_crate: None,
            port: Some(3306),
            config_content: "[mysqld]\nbind-address = 127.0.0.1\nport = 3306\ninnodb_buffer_pool_size = 512M\n".to_string(),
            logs: vec!["[mariadbd] ready for connections on /run/mysqld/mysqld.sock".to_string()],
        },
        ServerPackageItem {
            id: "pkg-certbot".to_string(),
            name: "Certbot ACME SSL Client".to_string(),
            category: "Security".to_string(),
            version: "2.11.0".to_string(),
            description: "Automated Let's Encrypt X.509 certificate provisioning via /www/acme.".to_string(),
            installed: certbot_avail,
            status: if certbot_avail { "running" } else { "stopped" }.to_string(),
            memory_mb: 4.2,
            config_path: "/etc/letsencrypt/cli.ini".to_string(),
            is_rust_native: false,
            replaced_legacy_package: None,
            rust_crate: Some("x509-parser 0.16 (Certificate Inspector)".to_string()),
            port: None,
            config_content: "rsa-key-size = 4096\nauthenticator = webroot\nwebroot-path = /www/acme\n".to_string(),
            logs: vec!["[certbot] systemd renewal timer active".to_string()],
        },
        ServerPackageItem {
            id: "pkg-fail2ban".to_string(),
            name: "Fail2ban & UFW Intrusion Shield".to_string(),
            category: "Security".to_string(),
            version: "1.1.0".to_string(),
            description: "Automated brute-force log scanner and netfilter IP ban manager.".to_string(),
            installed: f2b_avail,
            status: if f2b_avail { "running" } else { "stopped" }.to_string(),
            memory_mb: 18.5,
            config_path: "/etc/fail2ban/jail.local".to_string(),
            is_rust_native: false,
            replaced_legacy_package: None,
            rust_crate: None,
            port: None,
            config_content: "[DEFAULT]\nbantime = 3600\nfindtime = 600\nmaxretry = 5\n".to_string(),
            logs: vec!["[fail2ban] jail sshd and nginx-limit-req active".to_string()],
        },
    ]
}

pub async fn list_software_packages(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
) -> Result<Vec<ServerPackageItem>, PanelError> {
    let defaults = default_software_packages(executor);
    let mut pkgs = if let Some(mut existing) = store.get_json::<Vec<ServerPackageItem>>(KV_SOFTWARE_PKGS)? {
        let mut merged = false;
        for def in defaults {
            if !existing.iter().any(|p| p.id == def.id) {
                existing.push(def);
                merged = true;
            }
        }
        if merged {
            store.put_json(KV_SOFTWARE_PKGS, &existing)?;
        }
        existing
    } else {
        store.put_json(KV_SOFTWARE_PKGS, &defaults)?;
        defaults
    };

    if is_production_vps(config) {
        let has_systemctl = executor.is_binary_available(AllowedBinary::Systemctl);
        for pkg in &mut pkgs {
            if let Some(meta) = package_system_meta(&pkg.id) {
                pkg.installed = executor.is_binary_available(meta.binary);
                if meta.is_cli_tool {
                    pkg.status = if pkg.installed {
                        "running".to_string()
                    } else {
                        "not_installed".to_string()
                    };
                } else if pkg.installed && has_systemctl && !meta.systemd_unit.is_empty() {
                    let active = executor
                        .run(
                            AllowedBinary::Systemctl,
                            &["is-active", meta.systemd_unit],
                            CMD_TIMEOUT,
                        )
                        .await
                        .map(|o| o.success && o.stdout.trim() == "active")
                        .unwrap_or(false);
                    pkg.status = if active {
                        "running".to_string()
                    } else {
                        "stopped".to_string()
                    };
                } else if !pkg.installed {
                    pkg.status = "not_installed".to_string();
                }
                if let Ok(live_conf) = std::fs::read_to_string(meta.real_config_path) {
                    if !live_conf.trim().is_empty() && live_conf.len() <= 64 * 1024 {
                        pkg.config_content = live_conf;
                    }
                }
            }
        }
        let _ = store.put_json(KV_SOFTWARE_PKGS, &pkgs);
    }

    Ok(pkgs)
}

async fn validate_and_activate_service(
    id: &str,
    meta: &PackageSystemMeta,
    action: &str,
    executor: &dyn CommandExecutor,
) -> Result<(), PanelError> {
    if !executor.is_binary_available(AllowedBinary::Systemctl) {
        return Err(PanelError::Conflict(format!(
            "thiếu systemctl để thực hiện '{action}' trên '{}'",
            meta.systemd_unit
        )));
    }
    if !executor.is_binary_available(meta.binary) {
        let hint = if id == "pkg-ferrousx" {
            " (chạy `sudo bash update.sh --ferrousx` để cài FerrousX)"
        } else {
            ""
        };
        return Err(PanelError::Conflict(format!(
            "Chưa cài đặt binary '{}' cho dịch vụ '{}'{hint}. Không thay đổi trạng thái dịch vụ đang chạy.",
            meta.binary.label(),
            meta.systemd_unit
        )));
    }

    // 1. Validate web server configuration BEFORE stopping any running web server or service.
    if id == "pkg-ferrousx" {
        let chk = executor
            .run(
                AllowedBinary::Ferrousx,
                &["--check-config", "--config", meta.real_config_path],
                CMD_TIMEOUT,
            )
            .await?;
        if !chk.success {
            let detail = if chk.stderr.trim().is_empty() {
                chk.stdout.trim()
            } else {
                chk.stderr.trim()
            };
            return Err(PanelError::Conflict(format!(
                "Kiểm tra cấu hình FerrousX thất bại trước khi khởi động lại (đã hủy để tránh sập dịch vụ): {detail}"
            )));
        }
    } else if id == "pkg-nginx" {
        let chk = executor
            .run(AllowedBinary::Nginx, &["-t"], CMD_TIMEOUT)
            .await?;
        if !chk.success {
            let detail = if chk.stderr.trim().is_empty() {
                chk.stdout.trim()
            } else {
                chk.stderr.trim()
            };
            return Err(PanelError::Conflict(format!(
                "Kiểm tra cấu hình Nginx (-t) thất bại trước khi khởi động lại (đã hủy để tránh sập dịch vụ): {detail}"
            )));
        }
    }

    // 2. Verify systemd unit file exists BEFORE stopping any competing web server.
    let unit_cat = executor
        .run(AllowedBinary::Systemctl, &["cat", meta.systemd_unit], CMD_TIMEOUT)
        .await?;
    if !unit_cat.success {
        let hint = if id == "pkg-ferrousx" {
            " Hãy chạy `sudo bash update.sh --ferrousx` để tạo unit ferrousx.service."
        } else {
            ""
        };
        return Err(PanelError::Conflict(format!(
            "Không tìm thấy systemd unit '{}'.{hint} Đã hủy để giữ nguyên dịch vụ hiện tại.",
            meta.systemd_unit
        )));
    }

    // 3. Stop competing web server on ports 80/443 if active, remembering state for rollback.
    let competing = if id == "pkg-nginx" {
        Some("ferrousx.service")
    } else if id == "pkg-ferrousx" {
        Some("nginx.service")
    } else {
        None
    };

    let mut stopped_other: Option<&'static str> = None;
    if let Some(other_unit) = competing {
        let other_active = executor
            .run(AllowedBinary::Systemctl, &["is-active", other_unit], CMD_TIMEOUT)
            .await
            .map(|o| o.success && o.stdout.trim() == "active")
            .unwrap_or(false);
        if other_active {
            let stop_other = executor
                .run(AllowedBinary::Systemctl, &["stop", other_unit], CMD_TIMEOUT)
                .await?;
            if !stop_other.success {
                return Err(PanelError::CommandFailed {
                    binary: "systemctl",
                    exit_code: stop_other.exit_code,
                    stderr: if stop_other.stderr.is_empty() {
                        stop_other.stdout
                    } else {
                        stop_other.stderr
                    },
                });
            }
            stopped_other = Some(other_unit);
        }
    }

    // 4. Execute start / restart / enable --now on the target unit.
    let cmd_res = match action {
        "enable-now" => {
            executor
                .run(
                    AllowedBinary::Systemctl,
                    &["enable", "--now", meta.systemd_unit],
                    CMD_TIMEOUT,
                )
                .await
        }
        other => {
            executor
                .run(
                    AllowedBinary::Systemctl,
                    &[other, meta.systemd_unit],
                    CMD_TIMEOUT,
                )
                .await
        }
    };

    let mut failure_reason: Option<String> = match cmd_res {
        Ok(out) if out.success => None,
        Ok(out) => Some(if out.stderr.trim().is_empty() {
            out.stdout.trim().to_string()
        } else {
            out.stderr.trim().to_string()
        }),
        Err(err) => Some(err.to_string()),
    };

    // 5. Verify Type=simple daemon actually stays active (not immediately crashing after fork).
    if failure_reason.is_none() {
        let mut is_active = false;
        for attempt in 0..6 {
            let check = executor
                .run(
                    AllowedBinary::Systemctl,
                    &["is-active", meta.systemd_unit],
                    CMD_TIMEOUT,
                )
                .await;
            if let Ok(out) = check {
                if out.success && out.stdout.trim() == "active" {
                    is_active = true;
                    break;
                }
            }
            if attempt < 5 {
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        }
        if !is_active {
            failure_reason = Some(format!(
                "dịch vụ '{}' không đạt trạng thái active sau khi {action}",
                meta.systemd_unit
            ));
        }
    }

    if let Some(reason) = failure_reason {
        let journal_tail = if executor.is_binary_available(AllowedBinary::Journalctl) {
            executor
                .run(
                    AllowedBinary::Journalctl,
                    &["-u", meta.systemd_unit, "-n", "12", "--no-pager"],
                    CMD_TIMEOUT,
                )
                .await
                .ok()
                .map(|o| {
                    if o.stdout.trim().is_empty() {
                        o.stderr.trim().to_string()
                    } else {
                        o.stdout.trim().to_string()
                    }
                })
                .filter(|s| !s.is_empty())
        } else {
            None
        };

        // Rollback: ensure failed unit is stopped and restore previously active competing web server.
        let _ = executor
            .run(AllowedBinary::Systemctl, &["stop", meta.systemd_unit], CMD_TIMEOUT)
            .await;
        let rollback_note = if let Some(other_unit) = stopped_other {
            let _ = executor
                .run(AllowedBinary::Systemctl, &["start", other_unit], CMD_TIMEOUT)
                .await;
            format!(" Đã tự động khôi phục '{other_unit}' để không làm gián đoạn máy chủ.")
        } else {
            String::new()
        };

        let log_suffix = journal_tail
            .map(|j| format!("\nLog gần nhất:\n{j}"))
            .unwrap_or_default();
        return Err(PanelError::Conflict(format!(
            "Không thể {action} '{}': {reason}.{rollback_note}{log_suffix}",
            meta.systemd_unit
        )));
    }

    Ok(())
}

pub async fn software_package_action(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    id: &str,
    action: &str,
) -> Result<Vec<ServerPackageItem>, PanelError> {
    let mut pkgs = list_software_packages(store, config, executor).await?;
    let Some(pkg) = pkgs.iter_mut().find(|p| p.id == id) else {
        return Err(PanelError::NotFound(format!("package '{id}' not found")));
    };

    let prod = is_production_vps(config);
    let meta = package_system_meta(id);
    let now = Utc::now().to_rfc3339();

    match action {
        "install" => {
            if prod {
                if let Some(ref m) = meta {
                    if m.is_cli_tool {
                        let msg = install_zero_node_cli_package(id, executor).await?;
                        pkg.logs.push(format!("[{now}] {msg}"));
                    } else {
                        if let Some(apt_pkg) = m.apt_package {
                            if !executor.is_binary_available(AllowedBinary::AptGet) {
                                return Err(PanelError::Conflict(format!(
                                    "thiếu apt-get để cài '{apt_pkg}' (gợi ý: sudo apt install -y {apt_pkg})"
                                )));
                            }
                            let res = executor
                                .run_with_env(
                                    AllowedBinary::AptGet,
                                    &["install", "-y", "-qq", apt_pkg],
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
                        if !m.systemd_unit.is_empty() {
                            validate_and_activate_service(id, m, "enable-now", executor).await?;
                        }
                    }
                }
            }
            pkg.installed = true;
            pkg.status = "running".to_string();
            pkg.logs.push(format!("[{now}] Installed and enabled {}", pkg.name));
        }
        "uninstall" => {
            if prod {
                if let Some(ref m) = meta {
                    if m.is_cli_tool {
                        let bin_path = match id {
                            "pkg-bun" => Some("/usr/local/bin/bun"),
                            "pkg-tailwindcss" => Some("/usr/local/bin/tailwindcss"),
                            "pkg-deno" => Some("/usr/local/bin/deno"),
                            _ => None,
                        };
                        if let Some(p) = bin_path {
                            if Path::new(p).exists() {
                                std::fs::remove_file(p)?;
                            }
                        }
                    } else {
                        if !m.systemd_unit.is_empty()
                            && executor.is_binary_available(AllowedBinary::Systemctl)
                        {
                            let is_active = executor
                                .run(AllowedBinary::Systemctl, &["is-active", m.systemd_unit], CMD_TIMEOUT)
                                .await
                                .map(|o| o.success && o.stdout.trim() == "active")
                                .unwrap_or(false);
                            if is_active {
                                let dis = executor
                                    .run(AllowedBinary::Systemctl, &["disable", "--now", m.systemd_unit], CMD_TIMEOUT)
                                    .await?;
                                if !dis.success {
                                    return Err(PanelError::CommandFailed {
                                        binary: "systemctl",
                                        exit_code: dis.exit_code,
                                        stderr: if dis.stderr.is_empty() { dis.stdout } else { dis.stderr },
                                    });
                                }
                            }
                        }
                        if let Some(apt_pkg) = m.apt_package {
                            if !executor.is_binary_available(AllowedBinary::AptGet) {
                                return Err(PanelError::Conflict(format!(
                                    "thiếu apt-get để gỡ gói '{apt_pkg}'"
                                )));
                            }
                            let res = executor
                                .run_with_env(
                                    AllowedBinary::AptGet,
                                    &["remove", "-y", "-qq", apt_pkg],
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
                }
            }
            pkg.installed = false;
            pkg.status = "not_installed".to_string();
            pkg.logs.push(format!("[{now}] Uninstalled {}", pkg.name));
        }
        "start" | "restart" => {
            if prod {
                if let Some(ref m) = meta {
                    if !m.is_cli_tool && !m.systemd_unit.is_empty() {
                        validate_and_activate_service(id, m, action, executor).await?;
                    }
                }
            }
            pkg.installed = true;
            pkg.status = "running".to_string();
            pkg.logs.push(format!("[{now}] Action '{action}' completed on {}", pkg.name));
        }
        "stop" => {
            if prod {
                if let Some(ref m) = meta {
                    if !m.is_cli_tool && !m.systemd_unit.is_empty() {
                        if !executor.is_binary_available(AllowedBinary::Systemctl) {
                            return Err(PanelError::Conflict(format!(
                                "thiếu systemctl để dừng '{}'",
                                m.systemd_unit
                            )));
                        }
                        let res = executor
                            .run(AllowedBinary::Systemctl, &["stop", m.systemd_unit], CMD_TIMEOUT)
                            .await?;
                        if !res.success {
                            return Err(PanelError::CommandFailed {
                                binary: "systemctl",
                                exit_code: res.exit_code,
                                stderr: if res.stderr.is_empty() { res.stdout } else { res.stderr },
                            });
                        }
                    }
                }
            }
            pkg.status = "stopped".to_string();
            pkg.logs.push(format!("[{now}] Stopped service {}", pkg.name));
        }
        other => {
            return Err(PanelError::Validation(format!(
                "unsupported package action '{other}'"
            )));
        }
    }

    store.put_json(KV_SOFTWARE_PKGS, &pkgs)?;
    Ok(pkgs)
}

pub async fn save_software_package_config(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    id: &str,
    config_content: &str,
) -> Result<Vec<ServerPackageItem>, PanelError> {
    let mut pkgs = list_software_packages(store, config, executor).await?;
    let Some(pkg) = pkgs.iter_mut().find(|p| p.id == id) else {
        return Err(PanelError::NotFound(format!("package '{id}' not found")));
    };

    let pkg_dir = config.data_dir.join("packages");
    std::fs::create_dir_all(&pkg_dir)?;
    let conf_path = pkg_dir.join(format!("{id}.conf"));
    atomic_write_file(&conf_path, config_content.as_bytes(), 0o644)?;

    if is_production_vps(config) {
        if let Some(meta) = package_system_meta(id) {
            let real_path = Path::new(meta.real_config_path);
            let Some(parent) = real_path.parent() else {
                return Err(PanelError::Conflict(format!(
                    "đường dẫn cấu hình không hợp lệ: {}",
                    meta.real_config_path
                )));
            };
            if !parent.is_dir() {
                return Err(PanelError::Conflict(format!(
                    "thư mục cấu hình '{}' chưa tồn tại — hãy cài gói '{}' trước",
                    parent.display(),
                    pkg.name
                )));
            }
            let prev_backup = std::fs::read(real_path).ok();
            atomic_write_file(real_path, config_content.as_bytes(), 0o644)?;

            let rollback = || {
                if let Some(ref prev) = prev_backup {
                    let _ = atomic_write_file(real_path, prev, 0o644);
                } else {
                    let _ = std::fs::remove_file(real_path);
                }
            };

            // Validate web engine config before reload; rollback on syntax error
            let validation_err: Option<String> = if id == "pkg-nginx"
                && executor.is_binary_available(AllowedBinary::Nginx)
            {
                match executor.run(AllowedBinary::Nginx, &["-t"], CMD_TIMEOUT).await {
                    Ok(o) if o.success => None,
                    Ok(o) => Some(if o.stderr.is_empty() { o.stdout } else { o.stderr }),
                    Err(e) => Some(e.to_string()),
                }
            } else if id == "pkg-ferrousx"
                && executor.is_binary_available(AllowedBinary::Ferrousx)
            {
                match executor
                    .run(
                        AllowedBinary::Ferrousx,
                        &["--check-config", "--config", meta.real_config_path],
                        CMD_TIMEOUT,
                    )
                    .await
                {
                    Ok(o) if o.success => None,
                    Ok(o) => Some(if o.stderr.is_empty() { o.stdout } else { o.stderr }),
                    Err(e) => Some(e.to_string()),
                }
            } else {
                None
            };

            if let Some(err_detail) = validation_err {
                rollback();
                return Err(PanelError::Validation(format!(
                    "Configuration syntax check failed (rolled back): {err_detail}"
                )));
            }

            if !meta.is_cli_tool && !meta.systemd_unit.is_empty() {
                if !executor.is_binary_available(AllowedBinary::Systemctl) {
                    rollback();
                    return Err(PanelError::Conflict(format!(
                        "thiếu systemctl để reload '{}' (đã rollback)",
                        meta.systemd_unit
                    )));
                }
                let rel = executor
                    .run(
                        AllowedBinary::Systemctl,
                        &["reload-or-restart", meta.systemd_unit],
                        CMD_TIMEOUT,
                    )
                    .await;
                match rel {
                    Ok(o) if o.success => {}
                    Ok(o) => {
                        rollback();
                        return Err(PanelError::CommandFailed {
                            binary: "systemctl",
                            exit_code: o.exit_code,
                            stderr: if o.stderr.is_empty() { o.stdout } else { o.stderr },
                        });
                    }
                    Err(e) => {
                        rollback();
                        return Err(e);
                    }
                }
            }
        }
    }

    pkg.config_content = config_content.to_string();
    pkg.logs.push(format!(
        "[{}] Updated and applied configuration ({} bytes)",
        Utc::now().to_rfc3339(),
        config_content.len()
    ));

    store.put_json(KV_SOFTWARE_PKGS, &pkgs)?;
    Ok(pkgs)
}

