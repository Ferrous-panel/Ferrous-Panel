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
            description: "React 19 Server Components built and executed directly with single-binary Bun + FerrousX reverse proxy (zero Node.js/npm required on VPS).".to_string(),
            default_port: 3001,
            requires_db: "PostgreSQL 17".to_string(),
            build_command: "bun install && bun run build".to_string(),
            exec_command_template: "/usr/local/bin/bun --bun run start".to_string(),
            popular_score: "Zero-Node.js SSR".to_string(),
            git_repo_url: Some("https://github.com/vercel/next.js.git".to_string()),
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
            version: "4.9.0 (Rust 1.85)".to_string(),
            description: "High-throughput native Rust REST microservice with systemd cgroups v2 supervision.".to_string(),
            default_port: 8081,
            requires_db: "PostgreSQL 17".to_string(),
            build_command: "cargo build --release".to_string(),
            exec_command_template: "/usr/bin/env".to_string(),
            popular_score: "99.8% Uptime · 320k req/s".to_string(),
            git_repo_url: Some("https://github.com/actix/examples.git".to_string()),
        },
        OneClickTemplate {
            id: "tpl-axum-realtime".to_string(),
            name: "Axum 0.8 + Tokio WebSocket".to_string(),
            category: "Rust Backend".to_string(),
            framework: WebsiteRuntimeType::AxumRust,
            version: "0.8.1 (Tokio 1.43)".to_string(),
            description: "Ergonomic modular Tokio/Tower service with native WebSocket and HTTP/2 multiplexing.".to_string(),
            default_port: 8082,
            requires_db: "PostgreSQL 17".to_string(),
            build_command: "cargo build --release".to_string(),
            exec_command_template: "/usr/bin/env".to_string(),
            popular_score: "Ultra Low p99 (0.28ms)".to_string(),
            git_repo_url: Some("https://github.com/tokio-rs/axum.git".to_string()),
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

    // Use StaticHtmlSpa only when running in non-production test/sandbox without systemctl
    let effective_framework = if prod || executor.is_binary_available(AllowedBinary::Systemctl) {
        tpl.framework
    } else {
        WebsiteRuntimeType::StaticHtmlSpa
    };

    let is_wordpress = tpl.id == "tpl-wordpress-fpm";
    let is_php = matches!(
        effective_framework,
        WebsiteRuntimeType::Php83Fpm | WebsiteRuntimeType::PhpSymfony7
    );

    // Pre-flight checks in production BEFORE downloading files so failures are instant & clear (HTTP 409)
    if prod && is_php {
        let sock = PhpFpmVersion::V83.socket_path();
        if !Path::new(sock).exists() {
            if executor.is_binary_available(AllowedBinary::PhpFpm83)
                && executor.is_binary_available(AllowedBinary::Systemctl)
            {
                let _ = executor
                    .run(
                        AllowedBinary::Systemctl,
                        &["enable", "--now", "php8.3-fpm.service"],
                        CMD_TIMEOUT,
                    )
                    .await;
            }
            if !Path::new(sock).exists() {
                return Err(PanelError::Conflict(format!(
                    "PHP 8.3-FPM chưa được cài đặt hoặc chưa chạy (thiếu socket '{sock}'). Hãy vào mục 'PHP-FPM & Phần mềm' để cài đặt và bật PHP 8.3 trước khi triển khai {}.",
                    tpl.name
                )));
            }
        }
    }

    let wants_db = req.create_db && tpl.requires_db != "None";
    if prod && wants_db {
        let lower_db = tpl.requires_db.to_ascii_lowercase();
        if (lower_db.contains("mariadb") || lower_db.contains("mysql"))
            && !executor.is_binary_available(AllowedBinary::Mysql)
        {
            return Err(PanelError::Conflict(
                "MariaDB/MySQL chưa được cài đặt trên máy chủ. Hãy cài MariaDB trong mục 'PHP-FPM & Phần mềm' hoặc bỏ chọn 'Tự động tạo cơ sở dữ liệu'.".to_string(),
            ));
        }
        if lower_db.contains("postgres") && !executor.is_binary_available(AllowedBinary::Psql) {
            return Err(PanelError::Conflict(
                "PostgreSQL chưa được cài đặt trên máy chủ. Hãy cài PostgreSQL trong mục 'PHP-FPM & Phần mềm' hoặc bỏ chọn 'Tự động tạo cơ sở dữ liệu'.".to_string(),
            ));
        }
    }

    let site_dir = SafePath::under_root(&config.wwwroot, domain.as_str())?;
    std::fs::create_dir_all(site_dir.as_path())?;

    if is_wordpress {
        if prod {
            download_and_extract_wordpress(site_dir.as_path(), &config.data_dir, executor).await?;
        } else {
            let index_php = site_dir.as_path().join("index.php");
            if !index_php.exists() {
                atomic_write_file(&index_php, b"<?php echo 'WordPress 1-Click Ready';\n", 0o644)?;
            }
        }
    } else if tpl.id == "tpl-tailwind-static" {
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
    } else if tpl.git_repo_url.is_none() {
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
        run_sub_dir: "/".to_string(),
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
        git_repo_url: tpl.git_repo_url.unwrap_or_default(),
        git_branch: String::new(),
        web_server_engine: req.web_server_engine,
    };

    let mut deployed = crate::modules::apps::deploy_app(deploy_req, store, config, webserver).await?;

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
