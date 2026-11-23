use super::common::*;
use rand::rngs::OsRng;
use rand::RngCore;

// ============================================================================
// 2. 1-Click Templates & Deployment
// ============================================================================

const WP_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);

fn generate_secure_token(len: usize, alphabet: &[u8]) -> Result<String, PanelError> {
    let mut raw = vec![0u8; len];
    OsRng
        .try_fill_bytes(&mut raw)
        .map_err(|_| PanelError::Internal("OS entropy source unavailable".to_string()))?;
    let mut out = String::with_capacity(len);
    for b in raw {
        let idx = (b as usize) % alphabet.len();
        if let Some(&ch) = alphabet.get(idx) {
            out.push(char::from(ch));
        }
    }
    Ok(out)
}

fn derive_db_ident(prefix: &str, domain: &str) -> String {
    let mut s = String::from(prefix);
    for ch in domain.chars() {
        if ch.is_ascii_alphanumeric() {
            s.push(ch.to_ascii_lowercase());
        } else {
            s.push('_');
        }
        if s.len() >= 28 {
            break;
        }
    }
    while s.ends_with('_') {
        s.pop();
    }
    if s.len() < 4 {
        s.push_str("site");
    }
    s
}

fn escape_php_single_quoted(val: &str) -> String {
    val.replace('\\', "\\\\").replace('\'', "\\'")
}

fn render_wp_config_from_sample(
    sample: &str,
    db_name: &str,
    db_user: &str,
    db_password: &str,
) -> Result<String, PanelError> {
    const SALT_CHARS: &[u8] =
        b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#%^&*()-_=+[]{}|;:,.<>?";
    let mut cfg = sample
        .replace(
            "database_name_here",
            &escape_php_single_quoted(db_name),
        )
        .replace("username_here", &escape_php_single_quoted(db_user))
        .replace(
            "password_here",
            &escape_php_single_quoted(db_password),
        )
        .replace("'localhost'", "'127.0.0.1'");

    for key in [
        "AUTH_KEY",
        "SECURE_AUTH_KEY",
        "LOGGED_IN_KEY",
        "NONCE_KEY",
        "AUTH_SALT",
        "SECURE_AUTH_SALT",
        "LOGGED_IN_SALT",
        "NONCE_SALT",
    ] {
        let salt = generate_secure_token(64, SALT_CHARS)?;
        let target = format!("define( '{key}',         'put your unique phrase here' );");
        let replacement = format!(
            "define( '{key}', '{}' );",
            escape_php_single_quoted(&salt)
        );
        if cfg.contains(&target) {
            cfg = cfg.replacen(&target, &replacement, 1);
        } else if cfg.contains("put your unique phrase here") {
            cfg = cfg.replacen(
                "put your unique phrase here",
                &escape_php_single_quoted(&salt),
                1,
            );
        }
    }
    Ok(cfg)
}

pub fn list_oneclick_templates() -> Vec<OneClickTemplate> {
    vec![
        OneClickTemplate {
            id: "tpl-tailwind-static".to_string(),
            name: "Tailwind CSS v4 + HTML/JS (Zero-Node.js)".to_string(),
            category: "Modern JS Frameworks".to_string(),
            framework: WebsiteRuntimeType::StaticHtmlSpa,
            version: "Tailwind v4 Oxide + FerrousX".to_string(),
            description: "Pure CSS/HTML/JS site compiled via standalone Tailwind CSS v4 native binary (Rust Oxide) and served directly by FerrousX with zero Node.js and 0 MB daemon RAM.".to_string(),
            default_port: 80,
            requires_db: "None".to_string(),
            build_command: "tailwindcss -i input.css -o output.css --minify".to_string(),
            exec_command_template: "".to_string(),
            popular_score: "Zero-Node.js · 0 MB RAM".to_string(),
            git_repo_url: None,
        },
        OneClickTemplate {
            id: "tpl-static-spa".to_string(),
            name: "Vite + React 19 / Vue 3 SPA (Bun Zero-Node.js)".to_string(),
            category: "Modern JS Frameworks".to_string(),
            framework: WebsiteRuntimeType::ViteSpa,
            version: "Vite 6 + React 19 / Vue 3".to_string(),
            description: "Builds React/Vue/Svelte/Tailwind SPA via native Bun binary (no Node.js) and serves dist/ directly via FerrousX with Brotli/Gzip precompressed delivery & SPA fallback.".to_string(),
            default_port: 80,
            requires_db: "None".to_string(),
            build_command: "bun install && bun run build".to_string(),
            exec_command_template: "".to_string(),
            popular_score: "Zero-Node.js · FerrousX Static".to_string(),
            git_repo_url: None,
        },
        OneClickTemplate {
            id: "tpl-nextjs-15".to_string(),
            name: "Next.js 15 App Router SSR (Bun Zero-Node.js)".to_string(),
            category: "Modern JS Frameworks".to_string(),
            framework: WebsiteRuntimeType::NextJs15,
            version: "15.2 (Bun Native · No Node.js)".to_string(),
            description: "React 19 / Next.js SSR starter executed directly with single-binary Bun + FerrousX reverse proxy (zero Node.js/npm required on VPS).".to_string(),
            default_port: 3001,
            requires_db: "PostgreSQL 17".to_string(),
            build_command: "bun install && bun run build".to_string(),
            exec_command_template: "/usr/local/bin/bun --bun run start".to_string(),
            popular_score: "Zero-Node.js SSR".to_string(),
            git_repo_url: None,
        },
        OneClickTemplate {
            id: "tpl-astro-5".to_string(),
            name: "Astro 5 + Tailwind CSS (FerrousX / Bun Zero-Node.js)".to_string(),
            category: "Modern JS Frameworks".to_string(),
            framework: WebsiteRuntimeType::Astro5,
            version: "Astro 5 (Zero-Node.js)".to_string(),
            description: "Content-focused islands architecture built via Bun and served via FerrousX clean URLs or Bun SSR without installing Node.js.".to_string(),
            default_port: 4321,
            requires_db: "None".to_string(),
            build_command: "bun install && bun run build".to_string(),
            exec_command_template: "/usr/local/bin/bun dist/server/entry.mjs".to_string(),
            popular_score: "Islands · Zero-Node.js".to_string(),
            git_repo_url: None,
        },
        OneClickTemplate {
            id: "tpl-actix-microservice".to_string(),
            name: "Actix-Web 4 Rust API".to_string(),
            category: "Rust Backend".to_string(),
            framework: WebsiteRuntimeType::ActixWebRust,
            version: "4.9.0 (Rust Native)".to_string(),
            description: "High-throughput native Rust REST microservice compiled locally with Cargo and supervised by systemd cgroups v2.".to_string(),
            default_port: 8081,
            requires_db: "PostgreSQL 17".to_string(),
            build_command: "cargo build --release".to_string(),
            exec_command_template: "/usr/bin/env".to_string(),
            popular_score: "99.8% Uptime · Native Rust".to_string(),
            git_repo_url: None,
        },
        OneClickTemplate {
            id: "tpl-axum-realtime".to_string(),
            name: "Axum 0.8 + Tokio WebSocket".to_string(),
            category: "Rust Backend".to_string(),
            framework: WebsiteRuntimeType::AxumRust,
            version: "0.8.1 (Rust Native)".to_string(),
            description: "Ergonomic modular Rust HTTP service compiled locally with Cargo and proxied via FerrousX / Nginx.".to_string(),
            default_port: 8082,
            requires_db: "PostgreSQL 17".to_string(),
            build_command: "cargo build --release".to_string(),
            exec_command_template: "/usr/bin/env".to_string(),
            popular_score: "Ultra Low p99 (0.28ms)".to_string(),
            git_repo_url: None,
        },
        OneClickTemplate {
            id: "tpl-wordpress-fpm".to_string(),
            name: "WordPress 6.7 (PHP 8.3-FPM + OPcache JIT)".to_string(),
            category: "PHP CMS / Framework".to_string(),
            framework: WebsiteRuntimeType::Php83Fpm,
            version: "6.7.2 (PHP 8.3)".to_string(),
            description: "Tải trực tiếp gói WordPress chính thức (wordpress.org/latest.tar.gz), tự cấu hình MariaDB + wp-config.php và chạy qua PHP 8.3-FPM Unix socket.".to_string(),
            default_port: 9000,
            requires_db: "MariaDB 11.4".to_string(),
            build_command: "php -v".to_string(),
            exec_command_template: "/usr/sbin/php-fpm8.3".to_string(),
            popular_score: "1-Click CMS Ready".to_string(),
            git_repo_url: None,
        },
        OneClickTemplate {
            id: "tpl-laravel-11".to_string(),
            name: "Laravel 11 Enterprise Suite".to_string(),
            category: "PHP CMS / Framework".to_string(),
            framework: WebsiteRuntimeType::PhpSymfony7,
            version: "11.31 (PHP 8.3)".to_string(),
            description: "Full-featured PHP 8.3 application preconfigured with try_files front-controller routing.".to_string(),
            default_port: 9000,
            requires_db: "MariaDB 11.4".to_string(),
            build_command: "composer install --no-dev -o".to_string(),
            exec_command_template: "/usr/sbin/php-fpm8.3".to_string(),
            popular_score: "Enterprise PHP".to_string(),
            git_repo_url: Some("https://github.com/laravel/laravel.git".to_string()),
        },
    ]
}

async fn download_and_extract_wordpress(
    site_dir: &Path,
    data_dir: &Path,
    executor: &dyn CommandExecutor,
) -> Result<(), PanelError> {
    if site_dir.join("wp-login.php").is_file() && site_dir.join("wp-includes").is_dir() {
        return Ok(());
    }
    if !executor.is_binary_available(AllowedBinary::Curl) {
        return Err(PanelError::Conflict(
            "Thiếu công cụ 'curl' trên máy chủ để tải WordPress (apt-get install -y curl).".to_string(),
        ));
    }
    if !executor.is_binary_available(AllowedBinary::Tar) {
        return Err(PanelError::Conflict(
            "Thiếu công cụ 'tar' trên máy chủ để giải nén WordPress.".to_string(),
        ));
    }

    let tmp_dir = data_dir.join("tmp");
    std::fs::create_dir_all(&tmp_dir)?;
    let suffix = generate_secure_token(12, b"abcdefghijklmnopqrstuvwxyz0123456789")?;
    let archive_path = tmp_dir.join(format!("wordpress-{suffix}.tar.gz"));
    let archive_str = archive_path.display().to_string();

    let dl = executor
        .run(
            AllowedBinary::Curl,
            &[
                "-fsSL",
                "--connect-timeout",
                "12",
                "--max-time",
                "55",
                "--proto",
                "=https",
                "-o",
                &archive_str,
                "https://wordpress.org/latest.tar.gz",
            ],
            WP_DOWNLOAD_TIMEOUT,
        )
        .await?;
    if !dl.success {
        let _ = std::fs::remove_file(&archive_path);
        return Err(PanelError::CommandFailed {
            binary: "curl",
            exit_code: dl.exit_code,
            stderr: format!(
                "Không thể tải WordPress từ wordpress.org/latest.tar.gz: {}",
                dl.stderr.trim()
            ),
        });
    }

    let dest_str = site_dir.display().to_string();
    let extract = executor
        .run(
            AllowedBinary::Tar,
            &["-xzf", &archive_str, "--strip-components=1", "-C", &dest_str],
            WP_DOWNLOAD_TIMEOUT,
        )
        .await;
    let _ = std::fs::remove_file(&archive_path);
    let extract_out = extract?;
    if !extract_out.success {
        return Err(PanelError::CommandFailed {
            binary: "tar",
            exit_code: extract_out.exit_code,
            stderr: format!(
                "Giải nén gói WordPress thất bại: {}",
                extract_out.stderr.trim()
            ),
        });
    }

    // Remove any static placeholder index.html so index.php serves WordPress.
    let stale_html = site_dir.join("index.html");
    if stale_html.is_file() {
        let _ = std::fs::remove_file(&stale_html);
    }

    if executor.is_binary_available(AllowedBinary::Chown) {
        let _ = executor
            .run(
                AllowedBinary::Chown,
                &["-R", "www-data:www-data", "--", &dest_str],
                WP_DOWNLOAD_TIMEOUT,
            )
            .await;
    }

    Ok(())
}

fn any_path_exists(paths: &[&str]) -> bool {
    paths.iter().any(|p| Path::new(p).exists())
}

struct AptSession {
    updated: bool,
}

impl AptSession {
    fn new() -> Self {
        Self { updated: false }
    }

    async fn install_packages(
        &mut self,
        executor: &dyn CommandExecutor,
        packages: &[&str],
    ) -> Result<bool, PanelError> {
        if !executor.is_binary_available(AllowedBinary::AptGet) {
            return Ok(false);
        }
        if !self.updated {
            let _ = executor
                .run_with_env(
                    AllowedBinary::AptGet,
                    &["update", "-qq"],
                    &[("DEBIAN_FRONTEND", "noninteractive")],
                    None,
                    APT_TIMEOUT,
                )
                .await;
            self.updated = true;
        }
        let mut args: Vec<&str> = vec!["install", "-y", "-qq"];
        args.extend_from_slice(packages);
        let res = executor
            .run_with_env(
                AllowedBinary::AptGet,
                &args,
                &[("DEBIAN_FRONTEND", "noninteractive")],
                None,
                APT_TIMEOUT,
            )
            .await?;
        Ok(res.success)
    }
}

async fn ensure_apt_tool(
    executor: &dyn CommandExecutor,
    apt: &mut AptSession,
    bin: AllowedBinary,
    apt_pkg: &str,
    auto_install: bool,
    logs: &mut Vec<String>,
) -> Result<(), PanelError> {
    if executor.is_binary_available(bin) {
        return Ok(());
    }
    if auto_install && apt.install_packages(executor, &[apt_pkg]).await? && executor.is_binary_available(bin) {
        logs.push(format!(
            "[{}] [auto-provision] Đã tự động cài gói '{}' từ kho APT bên ngoài.",
            Utc::now().to_rfc3339(),
            apt_pkg
        ));
        return Ok(());
    }
    Err(PanelError::Conflict(format!(
        "Thiếu công cụ '{}' trên máy chủ (gợi ý: sudo apt-get update && sudo apt-get install -y {apt_pkg}).",
        bin.label()
    )))
}

async fn ensure_php_fpm_ready(
    executor: &dyn CommandExecutor,
    apt: &mut AptSession,
    auto_install: bool,
    logs: &mut Vec<String>,
) -> Result<PhpFpmVersion, PanelError> {
    if let Ok(ver) = PhpFpmVersion::resolve_active_or_start(executor).await {
        return Ok(ver);
    }
    if !auto_install {
        return PhpFpmVersion::resolve_active_or_start(executor).await;
    }

    // Try PHP 8.3 first (Ubuntu 24.04 / Ondrej PPA), then fall back to distro default meta-packages (Ubuntu 22.04 PHP 8.1 / Debian 12 PHP 8.2).
    let installed_83 = apt
        .install_packages(
            executor,
            &[
                "php8.3-fpm",
                "php8.3-cli",
                "php8.3-common",
                "php8.3-mysql",
                "php8.3-mbstring",
                "php8.3-xml",
                "php8.3-curl",
                "php8.3-zip",
            ],
        )
        .await?;

    if installed_83 {
        logs.push(format!(
            "[{}] [auto-provision] Đã tự động tải và cài đặt PHP 8.3-FPM + extensions từ kho APT bên ngoài.",
            Utc::now().to_rfc3339()
        ));
    } else {
        let installed_default = apt
            .install_packages(
                executor,
                &[
                    "php-fpm",
                    "php-cli",
                    "php-common",
                    "php-mysql",
                    "php-mbstring",
                    "php-xml",
                    "php-curl",
                    "php-zip",
                ],
            )
            .await?;
        if installed_default {
            logs.push(format!(
                "[{}] [auto-provision] Đã tự động tải và cài đặt PHP-FPM mặc định của hệ điều hành + extensions từ kho APT.",
                Utc::now().to_rfc3339()
            ));
        }
    }

    PhpFpmVersion::resolve_active_or_start(executor).await
}

async fn ensure_db_service_ready(
    executor: &dyn CommandExecutor,
    apt: &mut AptSession,
    requires_db: &str,
    auto_install: bool,
    logs: &mut Vec<String>,
) -> Result<(), PanelError> {
    let lower_db = requires_db.to_ascii_lowercase();
    let has_systemctl = executor.is_binary_available(AllowedBinary::Systemctl);

    if lower_db.contains("mariadb") || lower_db.contains("mysql") {
        if !executor.is_binary_available(AllowedBinary::Mysql) {
            if auto_install && apt.install_packages(executor, &["mariadb-server"]).await? {
                logs.push(format!(
                    "[{}] [auto-provision] Đã tự động tải và cài đặt MariaDB Server từ kho APT bên ngoài.",
                    Utc::now().to_rfc3339()
                ));
            }
            if !executor.is_binary_available(AllowedBinary::Mysql) {
                return Err(PanelError::Conflict(
                    "MariaDB/MySQL chưa được cài đặt trên máy chủ và không thể tự động nạp qua APT. Hãy cài MariaDB trong mục 'PHP-FPM & Phần mềm' hoặc bỏ chọn 'Tự động tạo cơ sở dữ liệu'."
                        .to_string(),
                ));
            }
        }
        const MYSQL_SOCKETS: &[&str] = &[
            "/run/mysqld/mysqld.sock",
            "/var/run/mysqld/mysqld.sock",
            "/tmp/mysql.sock",
        ];
        if !any_path_exists(MYSQL_SOCKETS) && has_systemctl {
            for unit in ["mariadb.service", "mysql.service"] {
                let _ = executor
                    .run(AllowedBinary::Systemctl, &["enable", "--now", unit], CMD_TIMEOUT)
                    .await;
                for _ in 0..10 {
                    if any_path_exists(MYSQL_SOCKETS) {
                        return Ok(());
                    }
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
            }
        }
        if !any_path_exists(MYSQL_SOCKETS) {
            return Err(PanelError::Conflict(
                "Dịch vụ MariaDB/MySQL chưa chạy (thiếu socket '/run/mysqld/mysqld.sock'). Hãy bật MariaDB trong mục 'PHP-FPM & Phần mềm' (hoặc bỏ chọn 'Tự động tạo cơ sở dữ liệu')."
                    .to_string(),
            ));
        }
    } else if lower_db.contains("postgres") {
        if !executor.is_binary_available(AllowedBinary::Psql) {
            if auto_install && apt.install_packages(executor, &["postgresql"]).await? {
                logs.push(format!(
                    "[{}] [auto-provision] Đã tự động tải và cài đặt PostgreSQL Server từ kho APT bên ngoài.",
                    Utc::now().to_rfc3339()
                ));
            }
            if !executor.is_binary_available(AllowedBinary::Psql) {
                return Err(PanelError::Conflict(
                    "PostgreSQL chưa được cài đặt trên máy chủ và không thể tự động nạp qua APT. Hãy cài PostgreSQL trong mục 'PHP-FPM & Phần mềm' hoặc bỏ chọn 'Tự động tạo cơ sở dữ liệu'."
                        .to_string(),
                ));
            }
        }
        const PG_SOCKETS: &[&str] = &[
            "/var/run/postgresql/.s.PGSQL.5432",
            "/run/postgresql/.s.PGSQL.5432",
            "/tmp/.s.PGSQL.5432",
        ];
        if !any_path_exists(PG_SOCKETS) && has_systemctl {
            let _ = executor
                .run(
                    AllowedBinary::Systemctl,
                    &["enable", "--now", "postgresql.service"],
                    CMD_TIMEOUT,
                )
                .await;
            for _ in 0..10 {
                if any_path_exists(PG_SOCKETS) {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        }
        if !any_path_exists(PG_SOCKETS) {
            return Err(PanelError::Conflict(
                "Dịch vụ PostgreSQL chưa chạy (thiếu socket '/var/run/postgresql/.s.PGSQL.5432'). Hãy bật PostgreSQL trong mục 'PHP-FPM & Phần mềm' (hoặc bỏ chọn 'Tự động tạo cơ sở dữ liệu')."
                    .to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_external_source_url(raw: &str) -> Result<String, PanelError> {
    let trimmed = raw.trim();
    if !trimmed.starts_with("https://")
        || trimmed.len() > 512
        || trimmed
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '\'' | '"' | ';' | '`'))
    {
        return Err(PanelError::Validation(
            "URL nguồn bên ngoài phải bắt đầu bằng https:// và không chứa ký tự đặc biệt."
                .to_string(),
        ));
    }
    let after_scheme = &trimmed["https://".len()..];
    let host_port = after_scheme.split('/').next().unwrap_or("");
    let host = host_port
        .split('@')
        .next_back()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if host.is_empty()
        || host == "localhost"
        || host.starts_with("127.")
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || host.starts_with("169.254.")
        || host == "[::1]"
        || host == "::1"
    {
        return Err(PanelError::Forbidden(
            "Không được phép tải tài nguyên từ địa chỉ nội bộ (SSRF protection).".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

async fn download_and_extract_external_archive(
    url: &str,
    site_dir: &Path,
    data_dir: &Path,
    executor: &dyn CommandExecutor,
    apt: &mut AptSession,
    auto_install: bool,
    logs: &mut Vec<String>,
) -> Result<(), PanelError> {
    ensure_apt_tool(executor, apt, AllowedBinary::Curl, "curl", auto_install, logs).await?;
    let lower = url.to_ascii_lowercase();
    let is_zip = lower.ends_with(".zip");
    if is_zip {
        ensure_apt_tool(executor, apt, AllowedBinary::Unzip, "unzip", auto_install, logs).await?;
    } else {
        ensure_apt_tool(executor, apt, AllowedBinary::Tar, "tar", auto_install, logs).await?;
    }

    let tmp_dir = data_dir.join("tmp");
    std::fs::create_dir_all(&tmp_dir)?;
    let suffix = generate_secure_token(12, b"abcdefghijklmnopqrstuvwxyz0123456789")?;
    let ext = if is_zip { "zip" } else { "tar.gz" };
    let archive_path = tmp_dir.join(format!("ext-src-{suffix}.{ext}"));
    let archive_str = archive_path.display().to_string();

    let dl = executor
        .run(
            AllowedBinary::Curl,
            &[
                "-fsSL",
                "--connect-timeout",
                "12",
                "--max-time",
                "60",
                "--proto",
                "=https",
                "--max-filesize",
                "209715200",
                "-o",
                &archive_str,
                url,
            ],
            WP_DOWNLOAD_TIMEOUT,
        )
        .await?;
    if !dl.success {
        let _ = std::fs::remove_file(&archive_path);
        return Err(PanelError::CommandFailed {
            binary: "curl",
            exit_code: dl.exit_code,
            stderr: format!("Không thể tải gói tài nguyên từ {url}: {}", dl.stderr.trim()),
        });
    }

    let dest_str = site_dir.display().to_string();
    let extract_res = if is_zip {
        executor
            .run(
                AllowedBinary::Unzip,
                &["-qo", &archive_str, "-d", &dest_str],
                WP_DOWNLOAD_TIMEOUT,
            )
            .await
    } else {
        executor
            .run(
                AllowedBinary::Tar,
                &["-xzf", &archive_str, "-C", &dest_str],
                WP_DOWNLOAD_TIMEOUT,
            )
            .await
    };
    let _ = std::fs::remove_file(&archive_path);
    let out = extract_res?;
    if !out.success {
        return Err(PanelError::CommandFailed {
            binary: if is_zip { "unzip" } else { "tar" },
            exit_code: out.exit_code,
            stderr: format!("Giải nén gói tài nguyên thất bại: {}", out.stderr.trim()),
        });
    }
    logs.push(format!(
        "[{}] [external-source] Đã tải và giải nén tài nguyên từ {url}.",
        Utc::now().to_rfc3339()
    ));
    Ok(())
}

fn scaffold_bun_ssr_starter(
    site_dir: &Path,
    domain: &str,
    tpl_name: &str,
    entry_rel: &str,
    default_port: u16,
) -> Result<(), PanelError> {
    let entry_path = site_dir.join(entry_rel);
    if let Some(parent) = entry_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if !entry_path.exists() {
        let server_js = format!(
            "const port = Number(process.env.PORT || {default_port});\n\
             const host = process.env.HOST || '127.0.0.1';\n\
             Bun.serve({{\n\
               hostname: host,\n\
               port,\n\
               fetch(req) {{\n\
                 const url = new URL(req.url);\n\
                 if (url.pathname === '/health') {{\n\
                   return Response.json({{ ok: true, domain: {dom_json}, runtime: 'bun-zero-node' }});\n\
                 }}\n\
                 return new Response(\n\
                   `<!doctype html><html lang=\"vi\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{dom} — {tpl}</title></head><body style=\"font-family:system-ui;background:#0b0f17;color:#f1f5f9;display:grid;place-items:center;min-height:100vh;margin:0;padding:24px\"><main style=\"max-width:560px;background:#111827;border:1px solid #1e293b;border-radius:14px;padding:28px\"><h1 style=\"margin-top:0\">{dom}</h1><p>{tpl} đang chạy trực tiếp bằng Bun (Zero-Node.js) trên cổng ${{port}}.</p></main></body></html>`,\n\
                   {{ headers: {{ 'content-type': 'text/html; charset=utf-8' }} }}\n\
                 );\n\
               }},\n\
             }});\n\
             console.log('FerrousPanel Bun SSR listening on http://' + host + ':' + port);\n",
            dom = domain,
            dom_json = serde_json::to_string(domain).unwrap_or_else(|_| "\"localhost\"".to_string()),
            tpl = tpl_name,
        );
        atomic_write_file(&entry_path, server_js.as_bytes(), 0o644)?;
    }
    let pkg_json_path = site_dir.join("package.json");
    if !pkg_json_path.exists() {
        let pkg_json = format!(
            "{{\n  \"name\": \"ferrous-ssr-app\",\n  \"private\": true,\n  \"scripts\": {{\n    \"start\": \"bun {entry_rel}\"\n  }}\n}}\n"
        );
        atomic_write_file(&pkg_json_path, pkg_json.as_bytes(), 0o644)?;
    }
    Ok(())
}

fn scaffold_rust_http_starter(
    site_dir: &Path,
    domain: &str,
    tpl_name: &str,
    default_port: u16,
) -> Result<(), PanelError> {
    let cargo_toml_path = site_dir.join("Cargo.toml");
    let main_rs_path = site_dir.join("src").join("main.rs");
    if !cargo_toml_path.exists() {
        let bin_name = "ferrous_service";
        let cargo_toml = format!(
            "[package]\nname = \"{bin_name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[profile.release]\nopt-level = 2\n"
        );
        atomic_write_file(&cargo_toml_path, cargo_toml.as_bytes(), 0o644)?;
    }
    if !main_rs_path.exists() {
        std::fs::create_dir_all(site_dir.join("src"))?;
        let main_rs = format!(
            "use std::io::{{Read, Write}};\n\
             use std::net::TcpListener;\n\
             \n\
             fn main() {{\n\
                 let port = std::env::var(\"PORT\").unwrap_or_else(|_| \"{default_port}\".to_string());\n\
                 let host = std::env::var(\"HOST\").unwrap_or_else(|_| \"127.0.0.1\".to_string());\n\
                 let addr = format!(\"{{host}}:{{port}}\");\n\
                 let listener = match TcpListener::bind(&addr) {{\n\
                     Ok(l) => l,\n\
                     Err(err) => {{\n\
                         eprintln!(\"bind {{addr}} failed: {{err}}\");\n\
                         std::process::exit(1);\n\
                     }}\n\
                 }};\n\
                 println!(\"Rust service listening on http://{{addr}}\");\n\
                 for stream in listener.incoming().flatten() {{\n\
                     let mut s = stream;\n\
                     let mut buf = [0u8; 1024];\n\
                     let _ = s.read(&mut buf);\n\
                     let body = \"{{\\\"ok\\\":true,\\\"domain\\\":\\\"{dom}\\\",\\\"template\\\":\\\"{tpl}\\\"}}\";\n\
                     let resp = format!(\n\
                         \"HTTP/1.1 200 OK\\r\\nContent-Type: application/json\\r\\nContent-Length: {{}}\\r\\nConnection: close\\r\\n\\r\\n{{}}\",\n\
                         body.len(),\n\
                         body\n\
                     );\n\
                     let _ = s.write_all(resp.as_bytes());\n\
                 }}\n\
             }}\n",
            dom = domain,
            tpl = tpl_name.replace('"', ""),
        );
        atomic_write_file(&main_rs_path, main_rs.as_bytes(), 0o644)?;
    }
    Ok(())
}

pub async fn deploy_oneclick(
    req: OneClickDeployRequest,
    store: &ModuleStore,
    config: &Config,
    webserver: &WebServerManager,
) -> Result<ManagedApp, PanelError> {
    let domain = Domain::parse(&req.domain)?;
    let templates = list_oneclick_templates();
    let tpl = templates
        .into_iter()
        .find(|t| t.id == req.template_id)
        .ok_or_else(|| {
            PanelError::NotFound(format!(
                "Không tìm thấy mẫu triển khai 1-Click '{}'",
                req.template_id
            ))
        })?;

    let prod = is_production_vps(config);
    let executor = webserver.executor().as_ref();
    let auto_install = req.auto_install_deps;
    let mut apt = AptSession::new();
    let mut provision_logs: Vec<String> = Vec::new();

    // Optional external source URL (.git repo or .tar.gz / .zip archive)
    let custom_source = match req
        .source_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(raw) => Some(validate_external_source_url(raw)?),
        None => None,
    };
    let custom_is_archive = custom_source.as_deref().map(|u| {
        let l = u.to_ascii_lowercase();
        l.ends_with(".tar.gz") || l.ends_with(".tgz") || l.ends_with(".zip")
    }).unwrap_or(false);
    let effective_git_url = if custom_is_archive {
        String::new()
    } else if let Some(ref url) = custom_source {
        url.clone()
    } else {
        tpl.git_repo_url.clone().unwrap_or_default()
    };

    // Use StaticHtmlSpa only when running in non-production test/sandbox without systemctl
    let effective_framework = if prod || executor.is_binary_available(AllowedBinary::Systemctl) {
        tpl.framework
    } else {
        WebsiteRuntimeType::StaticHtmlSpa
    };

    let is_wordpress = tpl.id == "tpl-wordpress-fpm";
    let is_laravel = tpl.id == "tpl-laravel-11";
    let is_tailwind = tpl.id == "tpl-tailwind-static";
    let is_php = matches!(
        effective_framework,
        WebsiteRuntimeType::Php83Fpm | WebsiteRuntimeType::PhpSymfony7
    );
    let is_bun_ssr = matches!(
        effective_framework,
        WebsiteRuntimeType::NextJs15 | WebsiteRuntimeType::Astro5
    );
    let is_rust = matches!(
        effective_framework,
        WebsiteRuntimeType::ActixWebRust | WebsiteRuntimeType::AxumRust
    );

    // Auto-provision missing external runtimes & services in production before deploying
    if prod && is_php {
        let ver = ensure_php_fpm_ready(executor, &mut apt, auto_install, &mut provision_logs).await?;
        provision_logs.push(format!(
            "[{}] [runtime] PHP {}-FPM socket '{}' đã sẵn sàng.",
            Utc::now().to_rfc3339(),
            ver.version_str(),
            ver.socket_path()
        ));
    }
    if prod && is_bun_ssr && !executor.is_binary_available(AllowedBinary::Bun) {
        if auto_install {
            ensure_apt_tool(executor, &mut apt, AllowedBinary::Curl, "curl", true, &mut provision_logs).await?;
            ensure_apt_tool(executor, &mut apt, AllowedBinary::Unzip, "unzip", true, &mut provision_logs).await?;
            let msg = super::software::install_zero_node_cli_package("pkg-bun", executor).await?;
            provision_logs.push(format!(
                "[{}] [auto-provision] {msg}",
                Utc::now().to_rfc3339()
            ));
        }
        if !executor.is_binary_available(AllowedBinary::Bun) {
            return Err(PanelError::Conflict(format!(
                "Thiếu runtime Bun (/usr/local/bin/bun) để triển khai {}. Hãy bật tự động nạp tài nguyên hoặc cài Bun trong mục 'PHP-FPM & Phần mềm'.",
                tpl.name
            )));
        }
    }
    if prod && is_tailwind && !executor.is_binary_available(AllowedBinary::TailwindCli) && auto_install {
        if ensure_apt_tool(executor, &mut apt, AllowedBinary::Curl, "curl", true, &mut provision_logs).await.is_ok() {
            if let Ok(msg) = super::software::install_zero_node_cli_package("pkg-tailwindcss", executor).await {
                provision_logs.push(format!(
                    "[{}] [auto-provision] {msg}",
                    Utc::now().to_rfc3339()
                ));
            }
        }
    }
    if prod && is_rust && !executor.is_binary_available(AllowedBinary::Cargo) {
        if auto_install && apt.install_packages(executor, &["cargo", "rustc"]).await? {
            provision_logs.push(format!(
                "[{}] [auto-provision] Đã tự động cài đặt Cargo + Rustc từ kho APT bên ngoài.",
                Utc::now().to_rfc3339()
            ));
        }
        if !executor.is_binary_available(AllowedBinary::Cargo) {
            return Err(PanelError::Conflict(format!(
                "Thiếu công cụ Cargo/Rust trên máy chủ để biên dịch {}. Hãy cài Rust toolchain (rustup hoặc apt install cargo rustc) trước khi triển khai mẫu Rust.",
                tpl.name
            )));
        }
    }
    if prod && !effective_git_url.is_empty() {
        ensure_apt_tool(executor, &mut apt, AllowedBinary::Git, "git", auto_install, &mut provision_logs).await?;
    }

    let wants_db = req.create_db && tpl.requires_db != "None";
    if prod && wants_db {
        ensure_db_service_ready(executor, &mut apt, &tpl.requires_db, auto_install, &mut provision_logs).await?;
    }

    let site_dir = SafePath::under_root(&config.wwwroot, domain.as_str())?;
    std::fs::create_dir_all(site_dir.as_path())?;

    if custom_is_archive {
        if prod {
            if let Some(ref url) = custom_source {
                download_and_extract_external_archive(
                    url,
                    site_dir.as_path(),
                    &config.data_dir,
                    executor,
                    &mut apt,
                    auto_install,
                    &mut provision_logs,
                )
                .await?;
            }
        }
    } else if is_wordpress {
        if prod {
            ensure_apt_tool(executor, &mut apt, AllowedBinary::Curl, "curl", auto_install, &mut provision_logs).await?;
            ensure_apt_tool(executor, &mut apt, AllowedBinary::Tar, "tar", auto_install, &mut provision_logs).await?;
            download_and_extract_wordpress(site_dir.as_path(), &config.data_dir, executor).await?;
        } else {
            let index_php = site_dir.as_path().join("index.php");
            if !index_php.exists() {
                atomic_write_file(&index_php, b"<?php echo 'WordPress 1-Click Ready';\n", 0o644)?;
            }
        }
    } else if is_tailwind && effective_git_url.is_empty() {
        let input_css = site_dir.as_path().join("input.css");
        if !input_css.exists() {
            atomic_write_file(&input_css, b"@import \"tailwindcss\";\n", 0o644)?;
        }
        let index_html = site_dir.as_path().join("index.html");
        if !index_html.exists() {
            let html = format!(
                "<!doctype html>\n<html lang=\"vi\">\n<head>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <title>{dom} — Tailwind CSS v4 (Zero-Node.js)</title>\n  <link rel=\"stylesheet\" href=\"/output.css\">\n</head>\n<body class=\"min-h-screen bg-slate-950 text-slate-100 flex items-center justify-center p-6\">\n  <main class=\"max-w-xl w-full rounded-2xl border border-slate-800 bg-slate-900/90 p-8 space-y-4\">\n    <p class=\"text-xs font-mono uppercase tracking-wider text-orange-400\">FerrousX + Tailwind CSS v4 · Zero-Node.js</p>\n    <h1 class=\"text-2xl font-bold\">{dom}</h1>\n    <p class=\"text-sm text-slate-400\">Trang web chạy trực tiếp bằng FerrousX và Tailwind CSS Standalone CLI (Rust Oxide) — không cần cài Node.js.</p>\n  </main>\n</body>\n</html>\n",
                dom = domain.as_str()
            );
            atomic_write_file(&index_html, html.as_bytes(), 0o644)?;
        }
    } else if tpl.id == "tpl-nextjs-15" && effective_git_url.is_empty() {
        scaffold_bun_ssr_starter(
            site_dir.as_path(),
            domain.as_str(),
            &tpl.name,
            "server.mjs",
            tpl.default_port,
        )?;
    } else if tpl.id == "tpl-astro-5" && effective_git_url.is_empty() {
        scaffold_bun_ssr_starter(
            site_dir.as_path(),
            domain.as_str(),
            &tpl.name,
            "dist/server/entry.mjs",
            tpl.default_port,
        )?;
    } else if is_rust && effective_git_url.is_empty() {
        scaffold_rust_http_starter(
            site_dir.as_path(),
            domain.as_str(),
            &tpl.name,
            tpl.default_port,
        )?;
    } else if effective_git_url.is_empty() {
        let index_html = site_dir.as_path().join("index.html");
        if !index_html.exists() {
            let html = format!(
                "<!doctype html><html><head><meta charset=\"utf-8\"><title>{} — {}</title></head><body><h1>{} deployed via FerrousPanel 1-Click ({})</h1></body></html>\n",
                domain.as_str(),
                tpl.name,
                domain.as_str(),
                tpl.version
            );
            atomic_write_file(&index_html, html.as_bytes(), 0o644)?;
        }
    }

    let (create_database, db_name, db_user, db_password) = if wants_db {
        const PASS_CHARS: &[u8] =
            b"abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789!@#%^&*";
        let prefix = if is_wordpress { "wp_" } else { "app_" };
        let ident = derive_db_ident(prefix, domain.as_str());
        let pass = generate_secure_token(20, PASS_CHARS)?;
        (true, ident.clone(), ident, pass)
    } else {
        (false, String::new(), String::new(), String::new())
    };

    let display_title = req
        .site_title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&tpl.name);

    let deploy_req = DeployWebsiteRequest {
        name: format!("{} ({})", domain.as_str(), display_title),
        framework: effective_framework,
        domain: domain.as_str().to_string(),
        aliases: Vec::new(),
        internal_port: tpl.default_port.max(1024),
        working_dir: site_dir.as_path().display().to_string(),
        run_sub_dir: if is_laravel {
            "/public".to_string()
        } else {
            "/".to_string()
        },
        exec_command: if tpl.exec_command_template.is_empty() {
            "/usr/bin/env".to_string()
        } else {
            tpl.exec_command_template
        },
        workers: 2,
        memory_limit_mb: 512,
        ssl_enabled: req.enable_ssl,
        http3_quic: false,
        websocket_enabled: true,
        rate_limit_rps: 0,
        create_database,
        db_engine: tpl.requires_db,
        db_name: db_name.clone(),
        db_user: db_user.clone(),
        db_password: db_password.clone(),
        create_ftp: false,
        ftp_user: String::new(),
        git_repo_url: effective_git_url,
        git_branch: String::new(),
        web_server_engine: req.web_server_engine,
    };

    let mut deployed = crate::modules::apps::deploy_app(deploy_req, store, config, webserver).await?;
    if !provision_logs.is_empty() {
        provision_logs.append(&mut deployed.logs);
        deployed.logs = provision_logs;
        let _ = store.upsert_app(&deployed);
    }

    // Auto-generate wp-config.php when WordPress was downloaded and database was created
    if is_wordpress && create_database {
        let sample_path = site_dir.as_path().join("wp-config-sample.php");
        let target_path = site_dir.as_path().join("wp-config.php");
        if sample_path.is_file() && !target_path.exists() {
            if let Ok(sample_raw) = std::fs::read_to_string(&sample_path) {
                let wp_cfg = render_wp_config_from_sample(
                    &sample_raw,
                    &db_name,
                    &db_user,
                    &db_password,
                )?;
                atomic_write_file(&target_path, wp_cfg.as_bytes(), 0o640)?;
                if prod && executor.is_binary_available(AllowedBinary::Chown) {
                    let target_str = target_path.display().to_string();
                    let _ = executor
                        .run(
                            AllowedBinary::Chown,
                            &["www-data:www-data", "--", &target_str],
                            CMD_TIMEOUT,
                        )
                        .await;
                }
                deployed.logs.push(format!(
                    "[{}] Generated wp-config.php linked to database '{}' (user '{}'). Complete setup at http://{}/wp-admin/install.php",
                    Utc::now().to_rfc3339(),
                    db_name,
                    db_user,
                    domain.as_str()
                ));
                let _ = store.upsert_app(&deployed);
            }
        }
    }

    Ok(deployed)
}
