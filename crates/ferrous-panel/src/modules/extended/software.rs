use super::common::*;

// ============================================================================
// 4. Software Package Installer (Real systemd / apt / standalone binaries)
// ============================================================================

struct PackageSystemMeta {
    binary: AllowedBinary,
    systemd_unit: &'static str,
    apt_packages: &'static [&'static str],
    real_config_path: &'static str,
    is_cli_tool: bool,
    version_args: &'static [&'static str],
}

fn detect_postgresql_config_path() -> &'static str {
    for candidate in [
        "/etc/postgresql/17/main/postgresql.conf",
        "/etc/postgresql/16/main/postgresql.conf",
        "/etc/postgresql/15/main/postgresql.conf",
        "/etc/postgresql/14/main/postgresql.conf",
        "/etc/postgresql/13/main/postgresql.conf",
    ] {
        if Path::new(candidate).is_file() {
            return candidate;
        }
    }
    "/etc/postgresql/16/main/postgresql.conf"
}

fn detect_mariadb_unit() -> &'static str {
    if Path::new("/lib/systemd/system/mariadb.service").is_file()
        || Path::new("/usr/lib/systemd/system/mariadb.service").is_file()
        || Path::new("/etc/systemd/system/mariadb.service").is_file()
    {
        "mariadb.service"
    } else if Path::new("/lib/systemd/system/mysql.service").is_file()
        || Path::new("/usr/lib/systemd/system/mysql.service").is_file()
        || Path::new("/etc/systemd/system/mysql.service").is_file()
    {
        "mysql.service"
    } else {
        "mariadb.service"
    }
}

fn package_system_meta(id: &str) -> Option<PackageSystemMeta> {
    match id {
        "pkg-ferrousx" => Some(PackageSystemMeta {
            binary: AllowedBinary::Ferrousx,
            systemd_unit: "ferrousx.service",
            apt_packages: &[],
            real_config_path: "/etc/ferrousx/ferrousx.toml",
            is_cli_tool: false,
            version_args: &["--version"],
        }),
        "pkg-bun" => Some(PackageSystemMeta {
            binary: AllowedBinary::Bun,
            systemd_unit: "",
            apt_packages: &[],
            real_config_path: "/etc/ferrous-panel/bunfig.toml",
            is_cli_tool: true,
            version_args: &["--version"],
        }),
        "pkg-tailwindcss" => Some(PackageSystemMeta {
            binary: AllowedBinary::TailwindCli,
            systemd_unit: "",
            apt_packages: &[],
            real_config_path: "/etc/ferrous-panel/tailwind.conf",
            is_cli_tool: true,
            version_args: &["--help"],
        }),
        "pkg-deno" => Some(PackageSystemMeta {
            binary: AllowedBinary::Deno,
            systemd_unit: "",
            apt_packages: &[],
            real_config_path: "/etc/ferrous-panel/deno.json",
            is_cli_tool: true,
            version_args: &["--version"],
        }),
        "pkg-wasmtime" => Some(PackageSystemMeta {
            binary: AllowedBinary::Wasmtime,
            systemd_unit: "",
            apt_packages: &[],
            real_config_path: "/etc/ferrous-panel/wasmtime.toml",
            is_cli_tool: true,
            version_args: &["--version"],
        }),
        "pkg-nginx" => Some(PackageSystemMeta {
            binary: AllowedBinary::Nginx,
            systemd_unit: "nginx.service",
            apt_packages: &["nginx"],
            real_config_path: "/etc/nginx/nginx.conf",
            is_cli_tool: false,
            version_args: &["-v"],
        }),
        "pkg-postgresql" => Some(PackageSystemMeta {
            binary: AllowedBinary::Psql,
            systemd_unit: "postgresql.service",
            apt_packages: &["postgresql", "postgresql-client"],
            real_config_path: detect_postgresql_config_path(),
            is_cli_tool: false,
            version_args: &["--version"],
        }),
        "pkg-mariadb" => Some(PackageSystemMeta {
            binary: AllowedBinary::Mysql,
            systemd_unit: detect_mariadb_unit(),
            apt_packages: &["mariadb-server", "mariadb-client"],
            real_config_path: "/etc/mysql/mariadb.conf.d/50-server.cnf",
            is_cli_tool: false,
            version_args: &["--version"],
        }),
        "pkg-certbot" => Some(PackageSystemMeta {
            binary: AllowedBinary::Certbot,
            systemd_unit: "certbot.timer",
            apt_packages: &["certbot"],
            real_config_path: "/etc/letsencrypt/cli.ini",
            is_cli_tool: false,
            version_args: &["--version"],
        }),
        "pkg-fail2ban" => Some(PackageSystemMeta {
            binary: AllowedBinary::Fail2banClient,
            systemd_unit: "fail2ban.service",
            apt_packages: &["fail2ban"],
            real_config_path: "/etc/fail2ban/jail.local",
            is_cli_tool: false,
            version_args: &["--version"],
        }),
        "pkg-postfix" => Some(PackageSystemMeta {
            binary: AllowedBinary::Postconf,
            systemd_unit: "postfix.service",
            apt_packages: &["postfix"],
            real_config_path: "/etc/postfix/main.cf",
            is_cli_tool: false,
            version_args: &["-h", "mail_version"],
        }),
        "pkg-dovecot" => Some(PackageSystemMeta {
            binary: AllowedBinary::Dovecot,
            systemd_unit: "dovecot.service",
            apt_packages: &["dovecot-imapd", "dovecot-pop3d", "dovecot-lmtpd"],
            real_config_path: "/etc/dovecot/dovecot.conf",
            is_cli_tool: false,
            version_args: &["--version"],
        }),
        "pkg-rspamd" => Some(PackageSystemMeta {
            binary: AllowedBinary::Rspamd,
            systemd_unit: "rspamd.service",
            apt_packages: &["rspamd"],
            real_config_path: "/etc/rspamd/rspamd.conf",
            is_cli_tool: false,
            version_args: &["--version"],
        }),
        "pkg-podman" => Some(PackageSystemMeta {
            binary: AllowedBinary::Podman,
            systemd_unit: "",
            apt_packages: &["podman"],
            real_config_path: "/etc/containers/containers.conf",
            is_cli_tool: true,
            version_args: &["--version"],
        }),
        "pkg-wireguard" => Some(PackageSystemMeta {
            binary: AllowedBinary::Wg,
            systemd_unit: "wg-quick@wg0.service",
            apt_packages: &["wireguard-tools"],
            real_config_path: "/etc/wireguard/wg0.conf",
            is_cli_tool: false,
            version_args: &["--version"],
        }),
        _ => None,
    }
}

fn find_file_recursive(dir: &Path, filename: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() && p.file_name().and_then(|n| n.to_str()) == Some(filename) {
            return Some(p);
        }
        if p.is_dir() {
            if let Some(found) = find_file_recursive(&p, filename) {
                return Some(found);
            }
        }
    }
    None
}

async fn download_https_verified(
    executor: &dyn CommandExecutor,
    url: &str,
    dest_path: &Path,
    label: &str,
) -> Result<String, PanelError> {
    let dest_str = dest_path.display().to_string();
    let dl = executor
        .run(
            AllowedBinary::Curl,
            &[
                "-fsSL",
                "--proto",
                "=https",
                "--tlsv1.2",
                "--connect-timeout",
                "15",
                "--max-time",
                "180",
                "--retry",
                "3",
                "--retry-delay",
                "2",
                "-o",
                &dest_str,
                url,
            ],
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
    if std::fs::metadata(dest_path).map(|m| m.len()).unwrap_or(0) == 0 {
        return Err(PanelError::BadGateway(format!(
            "Tệp tải về {label} rỗng (0 byte) từ GitHub Releases."
        )));
    }
    Ok(dest_str)
}

pub(super) async fn install_zero_node_cli_package(
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
            let zip_str = download_https_verified(executor, &url, &zip_path, "Bun").await?;
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
            let dl_str =
                download_https_verified(executor, &url, &dl_path, "Tailwind CSS").await?;
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
            let zip_str = download_https_verified(executor, &url, &zip_path, "Deno").await?;
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
        "pkg-wasmtime" => {
            if !executor.is_binary_available(AllowedBinary::Tar) {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                return Err(PanelError::BinaryUnavailable("tar"));
            }
            let wt_arch = if arch == "aarch64" { "aarch64" } else { "x86_64" };
            let meta_out = executor
                .run(
                    AllowedBinary::Curl,
                    &[
                        "-fsSL",
                        "--proto",
                        "=https",
                        "--tlsv1.2",
                        "--connect-timeout",
                        "12",
                        "--max-time",
                        "25",
                        "--retry",
                        "2",
                        "--retry-delay",
                        "1",
                        "-H",
                        "Accept: application/vnd.github+json",
                        "-H",
                        "User-Agent: FerrousPanel/2.5",
                        "https://api.github.com/repos/bytecodealliance/wasmtime/releases/latest",
                    ],
                    CMD_TIMEOUT,
                )
                .await?;
            if !meta_out.success {
                return Err(PanelError::CommandFailed {
                    binary: "curl",
                    exit_code: meta_out.exit_code,
                    stderr: meta_out.stderr,
                });
            }
            let release: serde_json::Value = serde_json::from_str(&meta_out.stdout)
                .map_err(|e| PanelError::Conflict(format!("không đọc được metadata Wasmtime release: {e}")))?;
            let suffix = format!("-{wt_arch}-linux.tar.xz");
            let download_url = release
                .get("assets")
                .and_then(|a| a.as_array())
                .and_then(|arr| {
                    arr.iter().find_map(|item| {
                        let name = item.get("name")?.as_str()?;
                        if name.starts_with("wasmtime-") && name.ends_with(&suffix) {
                            item.get("browser_download_url")?.as_str().map(str::to_string)
                        } else {
                            None
                        }
                    })
                })
                .ok_or_else(|| {
                    PanelError::Conflict(format!(
                        "không tìm thấy asset Wasmtime phù hợp cho kiến trúc {wt_arch}-linux"
                    ))
                })?;

            let tar_path = tmp_dir.join("wasmtime.tar.xz");
            let tar_str =
                download_https_verified(executor, &download_url, &tar_path, "Wasmtime").await?;
            let tmp_str = tmp_dir.display().to_string();
            let ex = executor
                .run(
                    AllowedBinary::Tar,
                    &["-xJf", &tar_str, "-C", &tmp_str],
                    APT_TIMEOUT,
                )
                .await?;
            if !ex.success {
                return Err(PanelError::CommandFailed {
                    binary: "tar",
                    exit_code: ex.exit_code,
                    stderr: ex.stderr,
                });
            }
            let src_bin = find_file_recursive(&tmp_dir, "wasmtime").ok_or_else(|| {
                PanelError::Conflict(
                    "đã giải nén Wasmtime nhưng không tìm thấy file binary 'wasmtime'".to_string(),
                )
            })?;
            let src_str = src_bin.display().to_string();
            let inst = executor
                .run(
                    AllowedBinary::Install,
                    &["-m", "0755", &src_str, "/usr/local/bin/wasmtime"],
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
            Ok("Installed Bytecode Alliance Wasmtime runtime -> /usr/local/bin/wasmtime".to_string())
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
    let wasmtime_avail = executor.is_binary_available(AllowedBinary::Wasmtime);
    let nginx_avail = executor.is_binary_available(AllowedBinary::Nginx);
    let psql_avail = executor.is_binary_available(AllowedBinary::Psql);
    let mysql_avail = executor.is_binary_available(AllowedBinary::Mysql);
    let certbot_avail = executor.is_binary_available(AllowedBinary::Certbot);
    let f2b_avail = executor.is_binary_available(AllowedBinary::Fail2banClient);
    let postfix_avail = executor.is_binary_available(AllowedBinary::Postconf);
    let dovecot_avail = executor.is_binary_available(AllowedBinary::Dovecot);
    let rspamd_avail = executor.is_binary_available(AllowedBinary::Rspamd);
    let podman_avail = executor.is_binary_available(AllowedBinary::Podman);
    let wg_avail = executor.is_binary_available(AllowedBinary::Wg);

    vec![
        ServerPackageItem {
            id: "pkg-ferrousx".to_string(),
            name: "FerrousX HTTP/TLS & JS/CSS Asset Engine".to_string(),
            category: "Web Server".to_string(),
            version: if ferrousx_avail { "installed" } else { "not installed" }.to_string(),
            description: "Memory-safe HTTP/1.1, HTTP/2, TLS SNI, WebSocket, PHP FastCGI, and native Zero-Node.js Static/SPA server (.br/.gz precompressed + clean URLs + immutable JS/CSS cache).".to_string(),
            installed: ferrousx_avail,
            status: if ferrousx_avail { "stopped" } else { "not_installed" }.to_string(),
            memory_mb: 0.0,
            config_path: "/etc/ferrousx/ferrousx.toml".to_string(),
            is_rust_native: true,
            replaced_legacy_package: Some("Nginx / Apache httpd".to_string()),
            rust_crate: Some("crates/ferrousx (hyper 1 + rustls 0.23)".to_string()),
            port: Some(80),
            config_content: "worker_threads = 0\nbind_http = \"0.0.0.0:80\"\nbind_https = \"0.0.0.0:443\"\nsites_dir = \"/etc/ferrousx/sites\"\nacme_webroot = \"/www/acme\"\nserver_tokens = false\n".to_string(),
            logs: Vec::new(),
        },
        ServerPackageItem {
            id: "pkg-bun".to_string(),
            name: "Bun JS/TS All-in-One Runtime (Zero-Node.js)".to_string(),
            category: "JS/CSS Zero-Node".to_string(),
            version: if bun_avail { "installed" } else { "not installed" }.to_string(),
            description: "Single native binary runtime, bundler, and package manager for React, Vue, Next.js, Nuxt, SvelteKit, Astro, Remix, and Vite — replaces Node.js & npm completely.".to_string(),
            installed: bun_avail,
            status: if bun_avail { "running" } else { "not_installed" }.to_string(),
            memory_mb: 0.0,
            config_path: "/etc/ferrous-panel/bunfig.toml".to_string(),
            is_rust_native: false,
            replaced_legacy_package: Some("Node.js + npm + npx + pm2".to_string()),
            rust_crate: Some("oven-sh/bun native binary (/usr/local/bin/bun)".to_string()),
            port: None,
            config_content: "[install]\nauto = \"fallback\"\nexact = false\n\n[run]\nbun = true\n".to_string(),
            logs: Vec::new(),
        },
        ServerPackageItem {
            id: "pkg-tailwindcss".to_string(),
            name: "Tailwind CSS v4 Standalone CLI (Rust Oxide)".to_string(),
            category: "JS/CSS Zero-Node".to_string(),
            version: if tw_avail { "installed" } else { "not installed" }.to_string(),
            description: "Compiles Tailwind CSS v4 stylesheets directly into minified CSS via the Rust Oxide engine with zero Node.js and zero node_modules.".to_string(),
            installed: tw_avail,
            status: if tw_avail { "running" } else { "not_installed" }.to_string(),
            memory_mb: 0.0,
            config_path: "/etc/ferrous-panel/tailwind.conf".to_string(),
            is_rust_native: true,
            replaced_legacy_package: Some("Node.js PostCSS / Webpack CSS pipeline".to_string()),
            rust_crate: Some("tailwindcss-oxide + LightningCSS (/usr/local/bin/tailwindcss)".to_string()),
            port: None,
            config_content: "# FerrousPanel Zero-Node.js Tailwind CSS v4 CLI\n".to_string(),
            logs: Vec::new(),
        },
        ServerPackageItem {
            id: "pkg-deno".to_string(),
            name: "Deno 2 TypeScript & Fresh Runtime (Rust)".to_string(),
            category: "JS/CSS Zero-Node".to_string(),
            version: if deno_avail { "installed" } else { "not installed" }.to_string(),
            description: "Secure Rust-powered TypeScript/JavaScript runtime with native Web APIs, Fresh, Hono, and zero Node.js dependency.".to_string(),
            installed: deno_avail,
            status: if deno_avail { "running" } else { "not_installed" }.to_string(),
            memory_mb: 0.0,
            config_path: "/etc/ferrous-panel/deno.json".to_string(),
            is_rust_native: true,
            replaced_legacy_package: Some("Node.js + ts-node".to_string()),
            rust_crate: Some("denoland/deno (/usr/local/bin/deno)".to_string()),
            port: None,
            config_content: "{\n  \"nodeModulesDir\": \"none\"\n}\n".to_string(),
            logs: Vec::new(),
        },
        ServerPackageItem {
            id: "pkg-wasmtime".to_string(),
            name: "Wasmtime WASI Edge Runtime (Bytecode Alliance)".to_string(),
            category: "Edge & Cloud".to_string(),
            version: if wasmtime_avail { "installed" } else { "not installed" }.to_string(),
            description: "Real Rust WebAssembly / WASI sandbox runtime executing .wasm Edge Workers directly on Linux.".to_string(),
            installed: wasmtime_avail,
            status: if wasmtime_avail { "running" } else { "not_installed" }.to_string(),
            memory_mb: 0.0,
            config_path: "/etc/ferrous-panel/wasmtime.toml".to_string(),
            is_rust_native: true,
            replaced_legacy_package: None,
            rust_crate: Some("bytecodealliance/wasmtime (/usr/local/bin/wasmtime)".to_string()),
            port: None,
            config_content: "# Wasmtime CLI runtime used by FerrousPanel WASI Edge Workers\n".to_string(),
            logs: Vec::new(),
        },
        ServerPackageItem {
            id: "pkg-nginx".to_string(),
            name: "Nginx Web Server".to_string(),
            category: "Web Server".to_string(),
            version: if nginx_avail { "installed" } else { "not installed" }.to_string(),
            description: "Classic event-driven HTTP and reverse proxy server.".to_string(),
            installed: nginx_avail,
            status: if nginx_avail { "stopped" } else { "not_installed" }.to_string(),
            memory_mb: 0.0,
            config_path: "/etc/nginx/nginx.conf".to_string(),
            is_rust_native: false,
            replaced_legacy_package: None,
            rust_crate: None,
            port: Some(80),
            config_content: "user www-data;\nworker_processes auto;\nevents { worker_connections 4096; }\n".to_string(),
            logs: Vec::new(),
        },
        ServerPackageItem {
            id: "pkg-postgresql".to_string(),
            name: "PostgreSQL Relational Engine".to_string(),
            category: "Database".to_string(),
            version: if psql_avail { "installed" } else { "not installed" }.to_string(),
            description: "ACID relational database server with JSONB and MVCC concurrency.".to_string(),
            installed: psql_avail,
            status: if psql_avail { "stopped" } else { "not_installed" }.to_string(),
            memory_mb: 0.0,
            config_path: detect_postgresql_config_path().to_string(),
            is_rust_native: false,
            replaced_legacy_package: None,
            rust_crate: None,
            port: Some(5432),
            config_content: "listen_addresses = '127.0.0.1'\nport = 5432\nmax_connections = 200\nshared_buffers = 256MB\n".to_string(),
            logs: Vec::new(),
        },
        ServerPackageItem {
            id: "pkg-mariadb".to_string(),
            name: "MariaDB Server".to_string(),
            category: "Database".to_string(),
            version: if mysql_avail { "installed" } else { "not installed" }.to_string(),
            description: "MySQL-compatible relational database with InnoDB buffer pool tuning.".to_string(),
            installed: mysql_avail,
            status: if mysql_avail { "stopped" } else { "not_installed" }.to_string(),
            memory_mb: 0.0,
            config_path: "/etc/mysql/mariadb.conf.d/50-server.cnf".to_string(),
            is_rust_native: false,
            replaced_legacy_package: None,
            rust_crate: None,
            port: Some(3306),
            config_content: "[mysqld]\nbind-address = 127.0.0.1\nport = 3306\ninnodb_buffer_pool_size = 512M\n".to_string(),
            logs: Vec::new(),
        },
        ServerPackageItem {
            id: "pkg-certbot".to_string(),
            name: "Certbot ACME SSL Client".to_string(),
            category: "Security".to_string(),
            version: if certbot_avail { "installed" } else { "not installed" }.to_string(),
            description: "Automated Let's Encrypt X.509 certificate provisioning via /www/acme.".to_string(),
            installed: certbot_avail,
            status: if certbot_avail { "stopped" } else { "not_installed" }.to_string(),
            memory_mb: 0.0,
            config_path: "/etc/letsencrypt/cli.ini".to_string(),
            is_rust_native: false,
            replaced_legacy_package: None,
            rust_crate: Some("x509-parser 0.16 (Certificate Inspector)".to_string()),
            port: None,
            config_content: "rsa-key-size = 4096\nauthenticator = webroot\nwebroot-path = /www/acme\n".to_string(),
            logs: Vec::new(),
        },
        ServerPackageItem {
            id: "pkg-fail2ban".to_string(),
            name: "Fail2ban Intrusion Shield".to_string(),
            category: "Security".to_string(),
            version: if f2b_avail { "installed" } else { "not installed" }.to_string(),
            description: "Automated brute-force log scanner and netfilter IP ban manager.".to_string(),
            installed: f2b_avail,
            status: if f2b_avail { "stopped" } else { "not_installed" }.to_string(),
            memory_mb: 0.0,
            config_path: "/etc/fail2ban/jail.local".to_string(),
            is_rust_native: false,
            replaced_legacy_package: None,
            rust_crate: None,
            port: None,
            config_content: "[DEFAULT]\nbantime = 3600\nfindtime = 600\nmaxretry = 5\n".to_string(),
            logs: Vec::new(),
        },
        ServerPackageItem {
            id: "pkg-postfix".to_string(),
            name: "Postfix SMTP Mail Server".to_string(),
            category: "Mail Server".to_string(),
            version: if postfix_avail { "installed" } else { "not installed" }.to_string(),
            description: "Production SMTP MTA powering virtual mail domains, aliases, and outbound relay.".to_string(),
            installed: postfix_avail,
            status: if postfix_avail { "stopped" } else { "not_installed" }.to_string(),
            memory_mb: 0.0,
            config_path: "/etc/postfix/main.cf".to_string(),
            is_rust_native: false,
            replaced_legacy_package: None,
            rust_crate: None,
            port: Some(25),
            config_content: "# Managed via /etc/postfix/main.cf\n".to_string(),
            logs: Vec::new(),
        },
        ServerPackageItem {
            id: "pkg-dovecot".to_string(),
            name: "Dovecot IMAP/POP3 Server".to_string(),
            category: "Mail Server".to_string(),
            version: if dovecot_avail { "installed" } else { "not installed" }.to_string(),
            description: "Production Maildir IMAP/POP3 server for FerrousPanel mailboxes and Webmail.".to_string(),
            installed: dovecot_avail,
            status: if dovecot_avail { "stopped" } else { "not_installed" }.to_string(),
            memory_mb: 0.0,
            config_path: "/etc/dovecot/dovecot.conf".to_string(),
            is_rust_native: false,
            replaced_legacy_package: None,
            rust_crate: None,
            port: Some(993),
            config_content: "# Managed via /etc/dovecot/dovecot.conf\n".to_string(),
            logs: Vec::new(),
        },
        ServerPackageItem {
            id: "pkg-rspamd".to_string(),
            name: "Rspamd Anti-Spam & DKIM Filter".to_string(),
            category: "Mail Server".to_string(),
            version: if rspamd_avail { "installed" } else { "not installed" }.to_string(),
            description: "High-speed Bayesian anti-spam, SPF/DKIM/DMARC verifier, and milter filter.".to_string(),
            installed: rspamd_avail,
            status: if rspamd_avail { "stopped" } else { "not_installed" }.to_string(),
            memory_mb: 0.0,
            config_path: "/etc/rspamd/rspamd.conf".to_string(),
            is_rust_native: false,
            replaced_legacy_package: None,
            rust_crate: None,
            port: Some(11334),
            config_content: "# Managed via /etc/rspamd/rspamd.conf\n".to_string(),
            logs: Vec::new(),
        },
        ServerPackageItem {
            id: "pkg-podman".to_string(),
            name: "Podman Rootless OCI Container Engine".to_string(),
            category: "Edge & Cloud".to_string(),
            version: if podman_avail { "installed" } else { "not installed" }.to_string(),
            description: "Daemonless OCI container runtime backing FerrousPanel container sandboxes.".to_string(),
            installed: podman_avail,
            status: if podman_avail { "running" } else { "not_installed" }.to_string(),
            memory_mb: 0.0,
            config_path: "/etc/containers/containers.conf".to_string(),
            is_rust_native: false,
            replaced_legacy_package: Some("Docker Daemon".to_string()),
            rust_crate: None,
            port: None,
            config_content: "# Podman OCI container configuration\n".to_string(),
            logs: Vec::new(),
        },
        ServerPackageItem {
            id: "pkg-wireguard".to_string(),
            name: "WireGuard Kernel VPN Mesh (wireguard-tools)".to_string(),
            category: "Edge & Cloud".to_string(),
            version: if wg_avail { "installed" } else { "not installed" }.to_string(),
            description: "In-kernel Noise-protocol L3 VPN mesh (wg / wg-quick) for multi-VPS clustering.".to_string(),
            installed: wg_avail,
            status: if wg_avail { "stopped" } else { "not_installed" }.to_string(),
            memory_mb: 0.0,
            config_path: "/etc/wireguard/wg0.conf".to_string(),
            is_rust_native: false,
            replaced_legacy_package: Some("OpenVPN / IPsec".to_string()),
            rust_crate: None,
            port: Some(51820),
            config_content: "# WireGuard interface configuration (/etc/wireguard/wg0.conf)\n".to_string(),
            logs: Vec::new(),
        },
    ]
}

fn clean_version_output(id: &str, raw: &str) -> String {
    let trimmed = raw.trim();
    if id == "pkg-tailwindcss" {
        if let Some(first_word) = trimmed.lines().next() {
            return first_word.trim().chars().take(64).collect();
        }
    }
    trimmed.chars().take(64).collect()
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
                if pkg.installed {
                    if let Some(ver) =
                        probe_binary_version_line(executor, meta.binary, meta.version_args).await
                    {
                        pkg.version = clean_version_output(&pkg.id, &ver);
                    }
                } else {
                    pkg.version = "not installed".to_string();
                }

                if meta.is_cli_tool {
                    pkg.status = if pkg.installed {
                        "running".to_string()
                    } else {
                        "not_installed".to_string()
                    };
                    pkg.memory_mb = 0.0;
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
                    pkg.memory_mb = if active {
                        query_systemd_memory_mb(executor, meta.systemd_unit).await
                    } else {
                        0.0
                    };
                    let live_logs = query_systemd_logs(executor, meta.systemd_unit, "15").await;
                    if !live_logs.is_empty() {
                        pkg.logs = live_logs;
                    } else if !active {
                        pkg.logs.clear();
                    }
                } else if !pkg.installed {
                    pkg.status = "not_installed".to_string();
                    pkg.memory_mb = 0.0;
                    pkg.logs.clear();
                } else {
                    pkg.memory_mb = 0.0;
                }

                pkg.config_path = meta.real_config_path.to_string();
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

    // 5. Verify Type=simple daemon actually stays active.
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
                    if m.is_cli_tool && m.apt_packages.is_empty() {
                        let msg = install_zero_node_cli_package(id, executor).await?;
                        pkg.logs.push(format!("[{now}] {msg}"));
                    } else {
                        if !m.apt_packages.is_empty() {
                            if !executor.is_binary_available(AllowedBinary::AptGet) {
                                return Err(PanelError::Conflict(format!(
                                    "thiếu apt-get để cài '{}' (gợi ý: sudo apt install -y {})",
                                    m.apt_packages.join(" "),
                                    m.apt_packages.join(" ")
                                )));
                            }
                            apt_update_quiet(executor).await;
                            let res = apt_install_quiet(executor, m.apt_packages).await?;
                            if !res.success {
                                return Err(PanelError::CommandFailed {
                                    binary: "apt-get",
                                    exit_code: res.exit_code,
                                    stderr: if res.stderr.is_empty() { res.stdout } else { res.stderr },
                                });
                            }
                        }
                        if !m.systemd_unit.is_empty() && id != "pkg-wireguard" {
                            let refreshed = package_system_meta(id);
                            let active_meta = refreshed.as_ref().unwrap_or(m);
                            validate_and_activate_service(id, active_meta, "enable-now", executor).await?;
                        }
                    }
                }
            }
            pkg.installed = true;
            pkg.status = if id == "pkg-wireguard" && prod && !Path::new("/etc/wireguard/wg0.conf").is_file() {
                "stopped".to_string()
            } else {
                "running".to_string()
            };
            pkg.logs.push(format!("[{now}] Installed {}", pkg.name));
        }
        "uninstall" => {
            if prod {
                if let Some(ref m) = meta {
                    if m.is_cli_tool && m.apt_packages.is_empty() {
                        let bin_path = match id {
                            "pkg-bun" => Some("/usr/local/bin/bun"),
                            "pkg-tailwindcss" => Some("/usr/local/bin/tailwindcss"),
                            "pkg-deno" => Some("/usr/local/bin/deno"),
                            "pkg-wasmtime" => Some("/usr/local/bin/wasmtime"),
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
                        if !m.apt_packages.is_empty() {
                            if !executor.is_binary_available(AllowedBinary::AptGet) {
                                return Err(PanelError::Conflict(format!(
                                    "thiếu apt-get để gỡ gói '{}'",
                                    m.apt_packages.join(" ")
                                )));
                            }
                            let res = apt_remove_quiet(executor, m.apt_packages).await?;
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
            pkg.memory_mb = 0.0;
            pkg.version = "not installed".to_string();
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
            pkg.memory_mb = 0.0;
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

            // Validate engine config before reload; rollback on syntax error
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
