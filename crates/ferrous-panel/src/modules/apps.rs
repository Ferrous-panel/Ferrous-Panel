use crate::api::contract::{
    AppStatus, DeployWebsiteRequest, EnvVar, ManagedApp, PhpVersionTag, RewritePresetType,
    WebServerEngineType, WebsiteLiveTestResult, WebsiteRuntimeType,
};
use crate::config::Config;
use crate::error::PanelError;
use crate::modules::ModuleStore;
use crate::system::cmd::AllowedBinary;
use crate::system::paths::SafePath;
use crate::system::validate::{Domain, Port, ServiceName};
use crate::webserver::ssl::SslCertState;
use crate::webserver::{
    atomic_write_file, CommandExecutor, EngineKind, PhpFpmVersion, SafeFilename, SiteMode,
    SiteSpec, UpstreamHost, UpstreamTarget, WebServerEngine, WebServerManager, WwwRootPath,
};
use chrono::Utc;
use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const SYSTEMD_TIMEOUT: Duration = Duration::from_secs(20);
const BUILD_TIMEOUT: Duration = Duration::from_secs(180);

fn is_static_runtime(fw: WebsiteRuntimeType) -> bool {
    matches!(
        fw,
        WebsiteRuntimeType::StaticHtmlSpa | WebsiteRuntimeType::ViteSpa
    )
}

fn is_js_ssr_runtime(fw: WebsiteRuntimeType) -> bool {
    matches!(
        fw,
        WebsiteRuntimeType::NextJs15
            | WebsiteRuntimeType::Nuxt3
            | WebsiteRuntimeType::SvelteKit2
            | WebsiteRuntimeType::Astro5
            | WebsiteRuntimeType::RemixReactRouterV7
            | WebsiteRuntimeType::NestJs10
            | WebsiteRuntimeType::BunElysiaJs
            | WebsiteRuntimeType::Deno2
            | WebsiteRuntimeType::NodeJsNextJs
    )
}

fn is_js_or_css_runtime(fw: WebsiteRuntimeType) -> bool {
    is_static_runtime(fw) || is_js_ssr_runtime(fw)
}

fn is_php_runtime(fw: WebsiteRuntimeType) -> bool {
    matches!(
        fw,
        WebsiteRuntimeType::Php83Fpm | WebsiteRuntimeType::PhpSymfony7
    )
}

fn bun_binary_path() -> &'static str {
    AllowedBinary::Bun
        .resolved_path_str()
        .unwrap_or("/usr/local/bin/bun")
}

fn deno_binary_path() -> &'static str {
    AllowedBinary::Deno
        .resolved_path_str()
        .unwrap_or("/usr/local/bin/deno")
}

/// Automatically converts legacy `node`/`npm`/`npx` commands (or empty JS exec commands)
/// into Zero-Node.js native Bun/Deno commands so JS/TS frameworks run without installing Node.js.
fn normalize_zero_node_exec_command(
    framework: WebsiteRuntimeType,
    raw_exec: &str,
    working_dir: &Path,
) -> (String, Option<String>) {
    let trimmed = raw_exec.trim();
    if !is_js_ssr_runtime(framework) {
        return (trimmed.to_string(), None);
    }

    let bun = bun_binary_path();
    let deno = deno_binary_path();
    let dir_str = working_dir.display();

    if trimmed.is_empty() || trimmed == "/usr/bin/env" {
        let default_cmd = match framework {
            WebsiteRuntimeType::NextJs15 | WebsiteRuntimeType::NodeJsNextJs => {
                if working_dir.join(".next/standalone/server.js").is_file() {
                    format!("{bun} {dir_str}/.next/standalone/server.js")
                } else {
                    format!("{bun} --bun run start")
                }
            }
            WebsiteRuntimeType::Nuxt3 => format!("{bun} {dir_str}/.output/server/index.mjs"),
            WebsiteRuntimeType::SvelteKit2 => format!("{bun} {dir_str}/build/index.js"),
            WebsiteRuntimeType::Astro5 => format!("{bun} {dir_str}/dist/server/entry.mjs"),
            WebsiteRuntimeType::RemixReactRouterV7 => {
                format!("{bun} x react-router-serve {dir_str}/build/server/index.js")
            }
            WebsiteRuntimeType::NestJs10 => format!("{bun} {dir_str}/dist/main.js"),
            WebsiteRuntimeType::BunElysiaJs => format!("{bun} run {dir_str}/src/index.ts"),
            WebsiteRuntimeType::Deno2 => {
                format!("{deno} run --allow-net --allow-env --allow-read {dir_str}/main.ts")
            }
            _ => format!("{bun} run start"),
        };
        return (
            default_cmd.clone(),
            Some(format!(
                "Zero-Node.js runtime configured automatically: {default_cmd}"
            )),
        );
    }

    let mut parts = trimmed.split_whitespace();
    let first = parts.next().unwrap_or("");
    let rest: Vec<&str> = parts.collect();
    let first_name = Path::new(first)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(first);

    match first_name {
        "node" | "nodejs" => {
            let rewritten = if rest.is_empty() {
                format!("{bun} run start")
            } else {
                format!("{bun} {}", rest.join(" "))
            };
            (
                rewritten.clone(),
                Some(format!(
                    "Zero-Node.js mode: translated Node.js command -> {rewritten}"
                )),
            )
        }
        "npm" | "pnpm" | "yarn" => {
            let rewritten = if rest.is_empty() {
                format!("{bun} run start")
            } else if rest.first().copied() == Some("start") {
                format!("{bun} --bun run start")
            } else {
                format!("{bun} {}", rest.join(" "))
            };
            (
                rewritten.clone(),
                Some(format!(
                    "Zero-Node.js mode: translated {first_name} command -> {rewritten}"
                )),
            )
        }
        "npx" => {
            let rewritten = if rest.is_empty() {
                format!("{bun} run start")
            } else {
                format!("{bun} x {}", rest.join(" "))
            };
            (
                rewritten.clone(),
                Some(format!(
                    "Zero-Node.js mode: translated npx command -> {rewritten}"
                )),
            )
        }
        "bun" => (
            if rest.is_empty() {
                format!("{bun} run start")
            } else {
                format!("{bun} {}", rest.join(" "))
            },
            None,
        ),
        "deno" => (
            if rest.is_empty() {
                format!("{deno} run --allow-net --allow-env --allow-read main.ts")
            } else {
                format!("{deno} {}", rest.join(" "))
            },
            None,
        ),
        _ => (trimmed.to_string(), None),
    }
}

fn ensure_default_js_envs(
    framework: WebsiteRuntimeType,
    internal_port: u16,
    envs: &mut Vec<(String, String)>,
) {
    if !is_js_ssr_runtime(framework) {
        return;
    }
    let port_str = internal_port.to_string();
    let defaults = [
        ("PORT", port_str.as_str()),
        ("HOST", "127.0.0.1"),
        ("HOSTNAME", "127.0.0.1"),
        ("NITRO_PORT", port_str.as_str()),
        ("NITRO_HOST", "127.0.0.1"),
        ("NODE_ENV", "production"),
    ];
    for (k, v) in defaults {
        if !envs.iter().any(|(ek, _)| ek == k) {
            envs.push((k.to_string(), v.to_string()));
        }
    }
}

/// Resolves the effective static output directory (e.g. `dist/`, `build/`, `out/`, `.output/public`)
/// inside `working_dir` for zero-Node.js JS/CSS frameworks served directly by FerrousX/Nginx.
fn resolve_effective_doc_root(
    working_dir: &Path,
    run_sub_dir: &str,
    framework: WebsiteRuntimeType,
) -> PathBuf {
    let trimmed = run_sub_dir.trim().trim_start_matches('/').trim_end_matches('/');
    if !trimmed.is_empty()
        && !trimmed.contains("..")
        && !trimmed.contains('\0')
        && (is_static_runtime(framework) || is_php_runtime(framework))
    {
        let explicit = working_dir.join(trimmed);
        if explicit.is_dir() && explicit.starts_with(working_dir) {
            if is_php_runtime(framework) || explicit.join("index.html").is_file() {
                return explicit;
            }
        }
    }

    if is_static_runtime(framework) {
        for candidate in ["dist", "build", "out", ".output/public", "public"] {
            let sub = working_dir.join(candidate);
            if sub.is_dir() && sub.join("index.html").is_file() {
                return sub;
            }
        }
    }

    working_dir.to_path_buf()
}

fn package_json_has_script(working_dir: &Path, script_name: &str) -> bool {
    let pkg_path = working_dir.join("package.json");
    let Ok(raw) = std::fs::read_to_string(&pkg_path) else {
        return false;
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return false;
    };
    val.get("scripts")
        .and_then(|s| s.get(script_name))
        .and_then(|s| s.as_str())
        .is_some_and(|s| !s.trim().is_empty())
}

fn find_tailwind_input_output(working_dir: &Path) -> Option<(PathBuf, PathBuf)> {
    const INPUT_CANDIDATES: &[(&str, &str)] = &[
        ("src/input.css", "dist/output.css"),
        ("input.css", "output.css"),
        ("styles/input.css", "styles/output.css"),
        ("assets/input.css", "assets/output.css"),
        ("src/styles.css", "dist/styles.css"),
    ];
    for (inp, out) in INPUT_CANDIDATES {
        let inp_path = working_dir.join(inp);
        if inp_path.is_file() {
            let out_path = working_dir.join(out);
            return Some((inp_path, out_path));
        }
    }
    None
}

/// Executes a real Zero-Node.js JS/TS/CSS build directly via allowlisted native binaries
/// (`bun`, `tailwindcss` standalone CLI, or `deno`) without requiring Node.js/npm on the VPS.
async fn run_zero_node_js_css_build(
    working_dir: &Path,
    framework: WebsiteRuntimeType,
    executor: &dyn CommandExecutor,
) -> Vec<String> {
    let mut logs = Vec::new();
    if !is_js_or_css_runtime(framework) {
        return logs;
    }

    let now = Utc::now().to_rfc3339();
    let has_package_json = working_dir.join("package.json").is_file();
    let has_deno_json =
        working_dir.join("deno.json").is_file() || working_dir.join("deno.jsonc").is_file();
    let tailwind_pair = find_tailwind_input_output(working_dir);

    if has_package_json {
        if executor.is_binary_available(AllowedBinary::Bun) {
            logs.push(format!(
                "[{now}] Zero-Node.js build: installing dependencies via Bun (no Node.js required)..."
            ));
            match executor
                .run_with_env(
                    AllowedBinary::Bun,
                    &["install", "--no-progress"],
                    &[("NODE_ENV", "development")],
                    Some(working_dir),
                    BUILD_TIMEOUT,
                )
                .await
            {
                Ok(out) if out.success => {
                    logs.push(format!(
                        "[{}] Bun install succeeded.",
                        Utc::now().to_rfc3339()
                    ));
                    if package_json_has_script(working_dir, "build") {
                        match executor
                            .run_with_env(
                                AllowedBinary::Bun,
                                &["run", "build"],
                                &[("NODE_ENV", "production")],
                                Some(working_dir),
                                BUILD_TIMEOUT,
                            )
                            .await
                        {
                            Ok(build_out) if build_out.success => {
                                logs.push(format!(
                                    "[{}] Zero-Node.js framework build (`bun run build`) completed cleanly.",
                                    Utc::now().to_rfc3339()
                                ));
                            }
                            Ok(build_out) => {
                                let detail = if build_out.stderr.is_empty() {
                                    build_out.stdout
                                } else {
                                    build_out.stderr
                                };
                                let brief: String = detail.chars().take(400).collect();
                                logs.push(format!(
                                    "[{}] `bun run build` exited ({:?}): {brief}",
                                    Utc::now().to_rfc3339(),
                                    build_out.exit_code
                                ));
                            }
                            Err(err) => {
                                logs.push(format!(
                                    "[{}] `bun run build` failed: {err}",
                                    Utc::now().to_rfc3339()
                                ));
                            }
                        }
                    }
                }
                Ok(out) => {
                    let detail = if out.stderr.is_empty() {
                        out.stdout
                    } else {
                        out.stderr
                    };
                    let brief: String = detail.chars().take(400).collect();
                    logs.push(format!(
                        "[{}] `bun install` exited ({:?}): {brief}",
                        Utc::now().to_rfc3339(),
                        out.exit_code
                    ));
                }
                Err(err) => {
                    logs.push(format!(
                        "[{}] `bun install` error: {err}",
                        Utc::now().to_rfc3339()
                    ));
                }
            }
        } else {
            logs.push(format!(
                "[{now}] Detected package.json — install Bun (Zero-Node.js) in Software Store or run update.sh to enable automatic JS/TS framework builds without Node.js."
            ));
        }
    } else if has_deno_json && executor.is_binary_available(AllowedBinary::Deno) {
        let deno_raw = std::fs::read_to_string(working_dir.join("deno.json"))
            .or_else(|_| std::fs::read_to_string(working_dir.join("deno.jsonc")))
            .unwrap_or_default();
        if deno_raw.contains("\"build\"") {
            match executor
                .run_with_env(
                    AllowedBinary::Deno,
                    &["task", "build"],
                    &[],
                    Some(working_dir),
                    BUILD_TIMEOUT,
                )
                .await
            {
                Ok(out) if out.success => logs.push(format!(
                    "[{}] Zero-Node.js Deno build (`deno task build`) completed cleanly.",
                    Utc::now().to_rfc3339()
                )),
                Ok(out) => logs.push(format!(
                    "[{}] `deno task build` exited ({:?}): {}",
                    Utc::now().to_rfc3339(),
                    out.exit_code,
                    out.stderr.chars().take(300).collect::<String>()
                )),
                Err(err) => logs.push(format!(
                    "[{}] `deno task build` error: {err}",
                    Utc::now().to_rfc3339()
                )),
            }
        }
    }

    // Standalone Tailwind CSS v4 native binary (Rust Oxide engine — ZERO Node.js!)
    if let Some((inp_path, out_path)) = tailwind_pair {
        if !has_package_json && executor.is_binary_available(AllowedBinary::TailwindCli) {
            if let Some(parent) = out_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let inp_str = inp_path.display().to_string();
            let out_str = out_path.display().to_string();
            match executor
                .run_with_env(
                    AllowedBinary::TailwindCli,
                    &["-i", &inp_str, "-o", &out_str, "--minify"],
                    &[],
                    Some(working_dir),
                    BUILD_TIMEOUT,
                )
                .await
            {
                Ok(out) if out.success => {
                    logs.push(format!(
                        "[{}] Tailwind CSS Standalone CLI compiled {} -> {} (Zero-Node.js, minified).",
                        Utc::now().to_rfc3339(),
                        inp_path.display(),
                        out_path.display()
                    ));
                }
                Ok(out) => {
                    let detail = if out.stderr.is_empty() {
                        out.stdout
                    } else {
                        out.stderr
                    };
                    logs.push(format!(
                        "[{}] Tailwind CSS standalone build exited ({:?}): {}",
                        Utc::now().to_rfc3339(),
                        out.exit_code,
                        detail.chars().take(300).collect::<String>()
                    ));
                }
                Err(err) => {
                    logs.push(format!(
                        "[{}] Tailwind CSS standalone build error: {err}",
                        Utc::now().to_rfc3339()
                    ));
                }
            }
        }
    }

    logs
}

fn default_rewrite_preset(fw: WebsiteRuntimeType) -> RewritePresetType {
    match fw {
        WebsiteRuntimeType::StaticHtmlSpa | WebsiteRuntimeType::ViteSpa => {
            RewritePresetType::SpaHistory
        }
        WebsiteRuntimeType::Php83Fpm | WebsiteRuntimeType::PhpSymfony7 => {
            RewritePresetType::Laravel
        }
        WebsiteRuntimeType::NextJs15 => RewritePresetType::NextjsProxy,
        WebsiteRuntimeType::Nuxt3 => RewritePresetType::NuxtNitro,
        WebsiteRuntimeType::SvelteKit2 => RewritePresetType::SveltekitNode,
        WebsiteRuntimeType::Astro5 => RewritePresetType::AstroHybrid,
        WebsiteRuntimeType::ActixWebRust
        | WebsiteRuntimeType::AxumRust
        | WebsiteRuntimeType::RocketRust => RewritePresetType::RustProxy,
        _ => RewritePresetType::ApiProxy,
    }
}

fn unit_name_for_domain(domain: &Domain) -> Result<ServiceName, PanelError> {
    let sanitized: String = domain
        .as_str()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let candidate = format!("ferrous-app-{sanitized}");
    let truncated: String = candidate.chars().take(60).collect();
    ServiceName::parse(&truncated)
}

fn validate_exec_start(raw: &str) -> Result<String, PanelError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 512 {
        return Err(PanelError::Validation(
            "execCommand must be 1..=512 characters".to_string(),
        ));
    }
    if trimmed.contains('\0')
        || trimmed.contains('\n')
        || trimmed.contains('\r')
        || trimmed.contains(';')
        || trimmed.contains("&&")
        || trimmed.contains("||")
        || trimmed.contains('|')
        || trimmed.contains('`')
        || trimmed.contains("$(")
    {
        return Err(PanelError::Forbidden(
            "execCommand contains forbidden shell/control metacharacters".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_env_var(var: &EnvVar) -> Result<(String, String), PanelError> {
    let key = var.key.trim();
    if key.is_empty()
        || key.len() > 64
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        || key.bytes().next().is_some_and(|b| b.is_ascii_digit())
    {
        return Err(PanelError::Validation(format!(
            "invalid environment variable key '{key}'"
        )));
    }
    let val = var.value.trim();
    if val.contains('\0') || val.contains('\n') || val.contains('\r') {
        return Err(PanelError::Forbidden(
            "environment variable value contains forbidden control characters".to_string(),
        ));
    }
    Ok((key.to_string(), val.to_string()))
}

pub fn render_systemd_unit(
    app_name: &str,
    domain: &Domain,
    working_dir: &Path,
    exec_start: &str,
    memory_limit_mb: u64,
    env_vars: &[(String, String)],
) -> String {
    let mut out = String::new();
    out.push_str("[Unit]\n");
    out.push_str(&format!(
        "Description=FerrousPanel App {} ({})\n",
        app_name.replace(['\n', '\r'], " "),
        domain
    ));
    out.push_str("After=network.target\n\n");
    out.push_str("[Service]\n");
    out.push_str("Type=simple\n");
    out.push_str("User=www-data\n");
    out.push_str("Group=www-data\n");
    out.push_str(&format!("WorkingDirectory={}\n", working_dir.display()));
    out.push_str(&format!("ExecStart={exec_start}\n"));
    out.push_str("Restart=on-failure\n");
    out.push_str("RestartSec=3\n");
    if memory_limit_mb > 0 {
        out.push_str(&format!("MemoryMax={memory_limit_mb}M\n"));
    }
    out.push_str("NoNewPrivileges=true\n");
    out.push_str("PrivateTmp=true\n");
    for (k, v) in env_vars {
        let escaped = v.replace('\\', "\\\\").replace('"', "\\\"");
        out.push_str(&format!("Environment=\"{k}={escaped}\"\n"));
    }
    out.push_str("\n[Install]\n");
    out.push_str("WantedBy=multi-user.target\n");
    out
}

#[derive(Debug, Default)]
pub struct SystemdRuntimeMetrics {
    pub active: bool,
    pub sub_state: String,
    pub pid: Option<u32>,
    pub memory_used_mb: u64,
}

/// Parses `systemctl show <unit> --property=ActiveState,SubState,MainPID,MemoryCurrent`.
#[must_use]
pub fn parse_systemd_metrics(stdout: &str) -> SystemdRuntimeMetrics {
    let mut metrics = SystemdRuntimeMetrics::default();
    for line in stdout.lines() {
        if let Some((k, v)) = line.split_once('=') {
            let val = v.trim();
            match k.trim() {
                "ActiveState" => metrics.active = val == "active",
                "SubState" => metrics.sub_state = val.to_string(),
                "MainPID" => {
                    if let Ok(pid) = val.parse::<u32>() {
                        if pid > 0 {
                            metrics.pid = Some(pid);
                        }
                    }
                }
                "MemoryCurrent" => {
                    if let Ok(bytes) = val.parse::<u64>() {
                        // systemd prints 18446744073709551615 or `[not set]` when accounting is unavailable
                        if bytes < (1u64 << 60) {
                            metrics.memory_used_mb = bytes / (1024 * 1024);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    metrics
}

fn dir_size_mb(root: &Path) -> f64 {
    if !root.is_dir() {
        return 0.0;
    }
    let mut total_bytes: u64 = 0;
    let mut stack = vec![root.to_path_buf()];
    let mut visited = 0usize;
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            visited += 1;
            if visited > 10_000 {
                return ((total_bytes as f64 / (1024.0 * 1024.0)) * 100.0).round() / 100.0;
            }
            let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_file() {
                total_bytes = total_bytes.saturating_add(meta.len());
            } else if meta.is_dir() {
                stack.push(entry.path());
            }
        }
    }
    ((total_bytes as f64 / (1024.0 * 1024.0)) * 100.0).round() / 100.0
}

fn build_site_spec_for_app(
    domain: Domain,
    aliases: Vec<Domain>,
    working_path: &Path,
    wwwroot: &Path,
    framework: WebsiteRuntimeType,
    internal_port: u16,
    ssl_enabled: bool,
    force_https: bool,
) -> Result<SiteSpec, PanelError> {
    let raw_rel = working_path
        .strip_prefix(wwwroot)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| working_path.display().to_string());
    let root = WwwRootPath::under_root(wwwroot, &raw_rel)?;
    let mode = if is_static_runtime(framework) {
        SiteMode::StaticSpa {
            index_file: SafeFilename::parse("index.html")?,
            spa_fallback: true,
        }
    } else if is_php_runtime(framework) {
        SiteMode::PhpFpm {
            php_version: PhpFpmVersion::V83,
            index_file: SafeFilename::parse("index.php")?,
        }
    } else {
        let port = Port::new(internal_port)?;
        let upstream = UpstreamTarget::new(UpstreamHost::parse("127.0.0.1")?, port);
        SiteMode::ReverseProxy { upstream }
    };

    Ok(SiteSpec {
        domain,
        aliases,
        root,
        mode,
        ssl_enabled,
        redirect_https: ssl_enabled && force_https,
        http3_quic: false,
        websocket_enabled: false,
        rate_limit_rps: 0,
    })
}

async fn query_unit_metrics(
    executor: &dyn CommandExecutor,
    unit_file: &str,
) -> Result<SystemdRuntimeMetrics, PanelError> {
    if !executor.is_binary_available(AllowedBinary::Systemctl) {
        return Ok(SystemdRuntimeMetrics::default());
    }
    let out = executor
        .run(
            AllowedBinary::Systemctl,
            &[
                "show",
                unit_file,
                "--property=ActiveState,SubState,MainPID,MemoryCurrent",
            ],
            SYSTEMD_TIMEOUT,
        )
        .await?;
    Ok(parse_systemd_metrics(&out.stdout))
}

pub async fn enrich_app(
    mut app: ManagedApp,
    config: &Config,
    webserver: &WebServerManager,
) -> ManagedApp {
    let unit_to_query = if is_php_runtime(app.framework) {
        "php8.3-fpm.service".to_string()
    } else {
        app.systemd_unit.clone()
    };

    if !is_static_runtime(app.framework) && !unit_to_query.is_empty() {
        if let Ok(metrics) = query_unit_metrics(webserver.executor().as_ref(), &unit_to_query).await
        {
            app.pid = metrics.pid;
            app.memory_used_mb = metrics.memory_used_mb;
            app.status = if metrics.active {
                AppStatus::Running
            } else {
                AppStatus::Stopped
            };
        }
    } else if let Ok(cfg) = webserver.store.load_config() {
        let engine_active = match cfg.frontend_engine {
            EngineKind::Nginx => webserver.nginx.status().await.map(|s| s.active).unwrap_or(false),
            EngineKind::Ferrousx => webserver
                .ferrousx
                .status()
                .await
                .map(|s| s.active)
                .unwrap_or(false),
        };
        app.pid = None;
        app.memory_used_mb = 0;
        app.status = if engine_active {
            AppStatus::Running
        } else {
            AppStatus::Stopped
        };
    }

    if let Ok(safe_dir) = SafePath::under_root(&config.wwwroot, &app.working_dir) {
        app.disk_used_mb = dir_size_mb(safe_dir.as_path());
    }

    if let Ok(domain) = Domain::parse(&app.domain) {
        if let Ok(ssl) = webserver.ssl.read_cert_status(&domain).await {
            app.ssl_enabled = ssl.present;
            app.ssl_issuer = ssl
                .issuer
                .unwrap_or_else(|| if ssl.present { "Let's Encrypt".to_string() } else { "None".to_string() });
            app.ssl_expiry_days = match ssl.status {
                SslCertState::Missing => 0,
                _ => ssl.days_remaining.unwrap_or(0),
            };
        }
    }

    app
}

pub async fn list_apps(
    store: &ModuleStore,
    config: &Config,
    webserver: &WebServerManager,
) -> Result<Vec<ManagedApp>, PanelError> {
    let raw = store.list_apps()?;
    let mut enriched = Vec::with_capacity(raw.len());
    for app in raw {
        enriched.push(enrich_app(app, config, webserver).await);
    }
    Ok(enriched)
}

pub async fn deploy_app(
    req: DeployWebsiteRequest,
    store: &ModuleStore,
    config: &Config,
    webserver: &WebServerManager,
) -> Result<ManagedApp, PanelError> {
    let domain = Domain::parse(&req.domain)?;
    let mut parsed_aliases = Vec::with_capacity(req.aliases.len());
    for a in &req.aliases {
        let t = a.trim();
        if !t.is_empty() {
            parsed_aliases.push(Domain::parse(t)?);
        }
    }

    let raw_working = if req.working_dir.trim().is_empty() {
        config.wwwroot.join(domain.as_str()).display().to_string()
    } else {
        req.working_dir.clone()
    };
    let safe_working = SafePath::under_root(&config.wwwroot, &raw_working)?;
    std::fs::create_dir_all(safe_working.as_path())?;

    let mut clean_envs = Vec::new();
    let app_name = if req.name.trim().is_empty() {
        domain.as_str().to_string()
    } else {
        req.name.trim().to_string()
    };

    let preferred_engine = req.web_server_engine.map(|e| match e {
        WebServerEngineType::FerrousX => EngineKind::Ferrousx,
        WebServerEngineType::Nginx => EngineKind::Nginx,
    });

    let mut initial_logs = Vec::new();
    let trimmed_repo = req.git_repo_url.trim();
    if !trimmed_repo.is_empty()
        && (trimmed_repo.starts_with("https://") || trimmed_repo.starts_with("http://"))
        && !safe_working.as_path().join(".git").exists()
        && webserver
            .executor()
            .is_binary_available(AllowedBinary::Git)
    {
        let dir_empty = std::fs::read_dir(safe_working.as_path())
            .map(|mut rd| rd.next().is_none())
            .unwrap_or(false);
        if dir_empty {
            let branch = if req.git_branch.trim().is_empty() {
                "main"
            } else {
                req.git_branch.trim()
            };
            let dest_str = safe_working.as_path().display().to_string();
            match webserver
                .executor()
                .run(
                    AllowedBinary::Git,
                    &[
                        "clone",
                        "--depth",
                        "1",
                        "--branch",
                        branch,
                        trimmed_repo,
                        &dest_str,
                    ],
                    BUILD_TIMEOUT,
                )
                .await
            {
                Ok(out) if out.success => {
                    initial_logs.push(format!(
                        "[{}] Cloned repository {} (branch '{}')",
                        Utc::now().to_rfc3339(),
                        trimmed_repo,
                        branch
                    ));
                }
                Ok(out) => {
                    initial_logs.push(format!(
                        "[{}] Git clone skipped/failed ({:?}): {}",
                        Utc::now().to_rfc3339(),
                        out.exit_code,
                        out.stderr.chars().take(240).collect::<String>()
                    ));
                }
                Err(err) => {
                    initial_logs.push(format!(
                        "[{}] Git clone skipped: {err}",
                        Utc::now().to_rfc3339()
                    ));
                }
            }
        }
    }

    // Run Zero-Node.js JS/TS/CSS build (Bun / Deno / Standalone Tailwind CLI) if source exists.
    let build_logs = run_zero_node_js_css_build(
        safe_working.as_path(),
        req.framework,
        webserver.executor().as_ref(),
    )
    .await;
    initial_logs.extend(build_logs);

    let effective_doc_root =
        resolve_effective_doc_root(safe_working.as_path(), &req.run_sub_dir, req.framework);

    // Build and apply web server site configuration FIRST (validates nginx -t / ferrousx-bin --check-config).
    // Never write ssl_certificate directives before the real PEM files exist on disk (prevents `nginx -t` failure).
    let cert_ready = webserver
        .ssl
        .site_cert_paths(&domain)
        .map(|(cert, key)| cert.is_file() && key.is_file())
        .unwrap_or(false);
    let mut effective_ssl = req.ssl_enabled && cert_ready;
    let mut spec = build_site_spec_for_app(
        domain.clone(),
        parsed_aliases.clone(),
        &effective_doc_root,
        &config.wwwroot,
        req.framework,
        req.internal_port,
        effective_ssl,
        false,
    )?;
    spec.http3_quic = req.http3_quic;
    spec.websocket_enabled = req.websocket_enabled;
    spec.rate_limit_rps = req.rate_limit_rps;
    let mut rendered = webserver
        .upsert_and_apply_site(&spec, preferred_engine)
        .await?;
    if req.ssl_enabled && !effective_ssl {
        if webserver
            .executor()
            .is_binary_available(AllowedBinary::Certbot)
        {
            match webserver
                .ssl
                .issue_certificate(&domain, &parsed_aliases, None)
                .await
            {
                Ok(status) if status.present => {
                    spec.ssl_enabled = true;
                    if let Ok(ssl_rendered) = webserver
                        .upsert_and_apply_site(&spec, preferred_engine)
                        .await
                    {
                        rendered = ssl_rendered;
                        effective_ssl = true;
                        initial_logs.push(format!(
                            "[{}] Issued Let's Encrypt certificate and enabled HTTPS for {}",
                            Utc::now().to_rfc3339(),
                            domain.as_str()
                        ));
                    }
                }
                Ok(_) => {
                    initial_logs.push(format!(
                        "[{}] Deployed HTTP vhost cleanly (SSL pending: point DNS to this VPS then run SSL issue/renew)",
                        Utc::now().to_rfc3339()
                    ));
                }
                Err(err) => {
                    initial_logs.push(format!(
                        "[{}] Deployed HTTP vhost cleanly (initial SSL issuance skipped: {err}; point DNS to this VPS then retry SSL)",
                        Utc::now().to_rfc3339()
                    ));
                }
            }
        } else {
            initial_logs.push(format!(
                "[{}] Deployed HTTP vhost cleanly (certbot not installed on host)",
                Utc::now().to_rfc3339()
            ));
        }
    }

    let mut systemd_unit = String::new();
    let mut effective_exec_command = req.exec_command.clone();
    if !is_static_runtime(req.framework) && !is_php_runtime(req.framework) {
        let (normalized_exec, rewrite_note) =
            normalize_zero_node_exec_command(req.framework, &req.exec_command, safe_working.as_path());
        if let Some(note) = rewrite_note {
            initial_logs.push(format!("[{}] {note}", Utc::now().to_rfc3339()));
        }
        let exec_start = validate_exec_start(&normalized_exec)?;
        effective_exec_command = exec_start.clone();
        ensure_default_js_envs(req.framework, req.internal_port, &mut clean_envs);
        let unit = unit_name_for_domain(&domain)?;
        let unit_file = unit.unit_file_name();
        let unit_path = config.systemd_unit_dir.join(&unit_file);
        let unit_content = render_systemd_unit(
            &app_name,
            &domain,
            safe_working.as_path(),
            &exec_start,
            req.memory_limit_mb,
            &clean_envs,
        );
        atomic_write_file(&unit_path, unit_content.as_bytes(), 0o644)?;

        let executor = webserver.executor();
        let reload_res = executor
            .run(AllowedBinary::Systemctl, &["daemon-reload"], SYSTEMD_TIMEOUT)
            .await;
        if let Err(err) = reload_res {
            let _ = std::fs::remove_file(&unit_path);
            let _ = webserver.remove_and_apply_site(&domain).await;
            return Err(err);
        }
        let reload_out = reload_res?;
        if !reload_out.success {
            let _ = std::fs::remove_file(&unit_path);
            let _ = webserver.remove_and_apply_site(&domain).await;
            return Err(PanelError::CommandFailed {
                binary: "systemctl",
                exit_code: reload_out.exit_code,
                stderr: reload_out.stderr,
            });
        }

        let start_out = executor
            .run(
                AllowedBinary::Systemctl,
                &["enable", "--now", &unit_file],
                SYSTEMD_TIMEOUT,
            )
            .await;
        match start_out {
            Ok(out) if out.success => {}
            Ok(out) => {
                let _ = std::fs::remove_file(&unit_path);
                let _ = executor
                    .run(AllowedBinary::Systemctl, &["daemon-reload"], SYSTEMD_TIMEOUT)
                    .await;
                let _ = webserver.remove_and_apply_site(&domain).await;
                return Err(PanelError::CommandFailed {
                    binary: "systemctl",
                    exit_code: out.exit_code,
                    stderr: out.stderr,
                });
            }
            Err(err) => {
                let _ = std::fs::remove_file(&unit_path);
                let _ = webserver.remove_and_apply_site(&domain).await;
                return Err(err);
            }
        }
        systemd_unit = unit_file;
    }

    let id = format!("app-{}", domain.as_str().replace('.', "-"));
    let mut linked_db_name = None;
    if req.create_database && !req.db_name.trim().is_empty() {
        let db_user = if req.db_user.trim().is_empty() {
            req.db_name.trim()
        } else {
            req.db_user.trim()
        };
        if let Err(err) = crate::modules::databases::create_database(
            store,
            webserver.executor().as_ref(),
            &req.db_name,
            &req.db_engine,
            db_user,
            Some(&req.db_password),
            "utf8mb4_unicode_ci",
            &id,
        )
        .await
        {
            if !systemd_unit.is_empty() {
                let unit_path = config.systemd_unit_dir.join(&systemd_unit);
                let _ = webserver
                    .executor()
                    .run(
                        AllowedBinary::Systemctl,
                        &["disable", "--now", &systemd_unit],
                        SYSTEMD_TIMEOUT,
                    )
                    .await;
                let _ = std::fs::remove_file(&unit_path);
                let _ = webserver
                    .executor()
                    .run(AllowedBinary::Systemctl, &["daemon-reload"], SYSTEMD_TIMEOUT)
                    .await;
            }
            let _ = webserver.remove_and_apply_site(&domain).await;
            return Err(err);
        }
        linked_db_name = Some(req.db_name.trim().to_string());
    }

    clean_envs.clear();
    let app = ManagedApp {
        id: id.clone(),
        name: app_name,
        framework: req.framework,
        web_server_engine: Some(match rendered.engine {
            EngineKind::Ferrousx => WebServerEngineType::FerrousX,
            EngineKind::Nginx => WebServerEngineType::Nginx,
        }),
        runtime_version: String::new(),
        domain: domain.as_str().to_string(),
        aliases: parsed_aliases.iter().map(|d| d.as_str().to_string()).collect(),
        internal_port: req.internal_port,
        working_dir: safe_working.as_path().display().to_string(),
        run_sub_dir: req.run_sub_dir,
        exec_command: effective_exec_command,
        workers: req.workers,
        memory_limit_mb: req.memory_limit_mb,
        memory_used_mb: 0,
        cpu_percent: 0.0,
        pid: None,
        status: AppStatus::Stopped,
        ssl_enabled: effective_ssl,
        ssl_issuer: "None".to_string(),
        ssl_expiry_days: 0,
        force_https: false,
        http3_quic: req.http3_quic,
        websocket_enabled: req.websocket_enabled,
        rate_limit_rps: req.rate_limit_rps,
        requests_per_sec: 0,
        p99_latency_ms: 0.0,
        uptime_seconds: 0,
        disk_used_mb: 0.0,
        disk_quota_mb: 10240,
        traffic_month_gb: 0.0,
        linked_db_name,
        ftp_username: if req.create_ftp && !req.ftp_user.trim().is_empty() {
            Some(req.ftp_user.trim().to_string())
        } else {
            None
        },
        backup_count: 0,
        git_repo_url: req.git_repo_url,
        git_branch: req.git_branch,
        build_script: String::new(),
        last_deploy_commit: "HEAD".to_string(),
        last_deploy_time: Utc::now().to_rfc3339(),
        url_rewrite_preset: default_rewrite_preset(req.framework),
        url_rewrite_rules: rendered.content,
        php_version: if is_php_runtime(req.framework) {
            Some(PhpVersionTag::V83)
        } else {
            None
        },
        redirects: None,
        hotlink_protection: None,
        directory_auth: None,
        env_vars: Vec::new(),
        logs: initial_logs,
        systemd_unit,
        nginx_vhost: rendered.config_path.display().to_string(),
    };

    store.upsert_app(&app)?;
    Ok(enrich_app(app, config, webserver).await)
}

pub async fn app_action(
    id: &str,
    action: &str,
    store: &ModuleStore,
    config: &Config,
    webserver: &WebServerManager,
) -> Result<ManagedApp, PanelError> {
    let mut app = store
        .get_app(id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{id}' not found")))?;

    if !matches!(action, "start" | "stop" | "restart" | "rebuild") {
        return Err(PanelError::Validation(format!(
            "invalid app action '{action}'; must be start, stop, restart, or rebuild"
        )));
    }

    if action == "rebuild" {
        let safe_working = SafePath::under_root(&config.wwwroot, &app.working_dir)?;
        let build_logs = run_zero_node_js_css_build(
            safe_working.as_path(),
            app.framework,
            webserver.executor().as_ref(),
        )
        .await;
        app.logs.extend(build_logs);

        if is_static_runtime(app.framework) {
            if let Ok(domain) = Domain::parse(&app.domain) {
                let mut aliases = Vec::new();
                for a in &app.aliases {
                    if let Ok(d) = Domain::parse(a) {
                        aliases.push(d);
                    }
                }
                let effective_root = resolve_effective_doc_root(
                    safe_working.as_path(),
                    &app.run_sub_dir,
                    app.framework,
                );
                if let Ok(mut spec) = build_site_spec_for_app(
                    domain,
                    aliases,
                    &effective_root,
                    &config.wwwroot,
                    app.framework,
                    app.internal_port,
                    app.ssl_enabled,
                    app.force_https,
                ) {
                    spec.http3_quic = app.http3_quic;
                    spec.websocket_enabled = app.websocket_enabled;
                    spec.rate_limit_rps = app.rate_limit_rps;
                    if let Ok(rendered) = webserver.upsert_and_apply_site(&spec, None).await {
                        app.url_rewrite_rules = rendered.content;
                        app.nginx_vhost = rendered.config_path.display().to_string();
                    }
                }
            }
        }

        app.last_deploy_time = Utc::now().to_rfc3339();
        app.logs.push(format!(
            "[{}] Zero-Node.js rebuild pipeline completed for {} ({:?})",
            app.last_deploy_time, app.domain, app.framework
        ));
        if !app.systemd_unit.is_empty()
            && webserver
                .executor()
                .is_binary_available(AllowedBinary::Systemctl)
        {
            let service = ServiceName::parse(&app.systemd_unit)?;
            let unit_file = service.unit_file_name();
            let _ = webserver
                .executor()
                .run(AllowedBinary::Systemctl, &["restart", &unit_file], SYSTEMD_TIMEOUT)
                .await;
        }
        store.upsert_app(&app)?;
        return Ok(enrich_app(app, config, webserver).await);
    }

    if app.systemd_unit.is_empty() {
        return Err(PanelError::Conflict(format!(
            "application '{}' uses web server runtime directly and has no standalone systemd unit",
            app.domain
        )));
    }
    let service = ServiceName::parse(&app.systemd_unit)?;
    let unit_file = service.unit_file_name();
    let out = webserver
        .executor()
        .run(AllowedBinary::Systemctl, &[action, &unit_file], SYSTEMD_TIMEOUT)
        .await?;
    if !out.success {
        return Err(PanelError::CommandFailed {
            binary: "systemctl",
            exit_code: out.exit_code,
            stderr: out.stderr,
        });
    }

    Ok(enrich_app(app, config, webserver).await)
}

pub async fn git_deploy_app(
    id: &str,
    store: &ModuleStore,
    config: &Config,
    webserver: &WebServerManager,
) -> Result<ManagedApp, PanelError> {
    let mut app = store
        .get_app(id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{id}' not found")))?;

    let safe_working = SafePath::under_root(&config.wwwroot, &app.working_dir)?;
    std::fs::create_dir_all(safe_working.as_path())?;

    let branch = if app.git_branch.trim().is_empty() {
        "main"
    } else {
        app.git_branch.trim()
    };

    if webserver
        .executor()
        .is_binary_available(AllowedBinary::Git)
    {
        if safe_working.as_path().join(".git").exists() {
            match webserver
                .executor()
                .run_with_env(
                    AllowedBinary::Git,
                    &["pull", "--ff-only", "origin", branch],
                    &[],
                    Some(safe_working.as_path()),
                    BUILD_TIMEOUT,
                )
                .await
            {
                Ok(out) if out.success => {
                    app.logs.push(format!(
                        "[{}] git pull --ff-only origin {branch} succeeded.",
                        Utc::now().to_rfc3339()
                    ));
                }
                Ok(out) => {
                    app.logs.push(format!(
                        "[{}] git pull returned ({:?}): {}",
                        Utc::now().to_rfc3339(),
                        out.exit_code,
                        out.stderr.chars().take(240).collect::<String>()
                    ));
                }
                Err(err) => {
                    app.logs.push(format!(
                        "[{}] git pull skipped: {err}",
                        Utc::now().to_rfc3339()
                    ));
                }
            }
        } else if !app.git_repo_url.trim().is_empty()
            && (app.git_repo_url.trim().starts_with("https://")
                || app.git_repo_url.trim().starts_with("http://"))
        {
            let dir_empty = std::fs::read_dir(safe_working.as_path())
                .map(|mut rd| rd.next().is_none())
                .unwrap_or(false);
            if dir_empty {
                let dest_str = safe_working.as_path().display().to_string();
                let _ = webserver
                    .executor()
                    .run(
                        AllowedBinary::Git,
                        &[
                            "clone",
                            "--depth",
                            "1",
                            "--branch",
                            branch,
                            app.git_repo_url.trim(),
                            &dest_str,
                        ],
                        BUILD_TIMEOUT,
                    )
                    .await;
            }
        }
    }

    let build_logs = run_zero_node_js_css_build(
        safe_working.as_path(),
        app.framework,
        webserver.executor().as_ref(),
    )
    .await;
    app.logs.extend(build_logs);

    if is_static_runtime(app.framework) {
        if let Ok(domain) = Domain::parse(&app.domain) {
            let mut aliases = Vec::new();
            for a in &app.aliases {
                if let Ok(d) = Domain::parse(a) {
                    aliases.push(d);
                }
            }
            let effective_root = resolve_effective_doc_root(
                safe_working.as_path(),
                &app.run_sub_dir,
                app.framework,
            );
            if let Ok(mut spec) = build_site_spec_for_app(
                domain,
                aliases,
                &effective_root,
                &config.wwwroot,
                app.framework,
                app.internal_port,
                app.ssl_enabled,
                app.force_https,
            ) {
                spec.http3_quic = app.http3_quic;
                spec.websocket_enabled = app.websocket_enabled;
                spec.rate_limit_rps = app.rate_limit_rps;
                if let Ok(rendered) = webserver.upsert_and_apply_site(&spec, None).await {
                    app.url_rewrite_rules = rendered.content;
                    app.nginx_vhost = rendered.config_path.display().to_string();
                }
            }
        }
    }

    let git_head_path = safe_working.as_path().join(".git/HEAD");
    let commit_ref = if let Ok(raw_head) = std::fs::read_to_string(&git_head_path) {
        let trimmed = raw_head.trim();
        if let Some(rel_ref) = trimmed.strip_prefix("ref: ") {
            let ref_path = safe_working.as_path().join(".git").join(rel_ref.trim());
            std::fs::read_to_string(ref_path)
                .map(|s| s.trim().chars().take(7).collect::<String>())
                .unwrap_or_else(|_| "HEAD".to_string())
        } else {
            trimmed.chars().take(7).collect::<String>()
        }
    } else {
        format!("{:07x}", Utc::now().timestamp() & 0x0fff_ffff)
    };

    app.last_deploy_commit = commit_ref.clone();
    app.last_deploy_time = Utc::now().to_rfc3339();
    app.logs.push(format!(
        "[{}] Git deploy synced branch '{}' @ commit {}",
        app.last_deploy_time, app.git_branch, commit_ref
    ));

    if !app.systemd_unit.is_empty()
        && webserver
            .executor()
            .is_binary_available(AllowedBinary::Systemctl)
    {
        let service = ServiceName::parse(&app.systemd_unit)?;
        let unit_file = service.unit_file_name();
        let _ = webserver
            .executor()
            .run(AllowedBinary::Systemctl, &["restart", &unit_file], SYSTEMD_TIMEOUT)
            .await;
    }

    store.upsert_app(&app)?;
    Ok(enrich_app(app, config, webserver).await)
}

pub async fn update_app_config(
    id: &str,
    workers: u32,
    memory_limit_mb: u64,
    rate_limit_rps: u32,
    ssl_enabled: Option<bool>,
    http3_quic: Option<bool>,
    websocket_enabled: Option<bool>,
    env_vars: Vec<EnvVar>,
    store: &ModuleStore,
    config: &Config,
    webserver: &WebServerManager,
) -> Result<ManagedApp, PanelError> {
    let mut app = store
        .get_app(id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{id}' not found")))?;

    let mut validated_envs = Vec::with_capacity(env_vars.len());
    for v in &env_vars {
        validated_envs.push(validate_env_var(v)?);
    }

    let domain = Domain::parse(&app.domain)?;
    if !app.systemd_unit.is_empty() {
        let unit = ServiceName::parse(&app.systemd_unit)?;
        let unit_file = unit.unit_file_name();
        let unit_path = config.systemd_unit_dir.join(&unit_file);
        let safe_working = SafePath::under_root(&config.wwwroot, &app.working_dir)?;
        let (normalized_exec, _) =
            normalize_zero_node_exec_command(app.framework, &app.exec_command, safe_working.as_path());
        let exec_start = validate_exec_start(&normalized_exec)?;
        app.exec_command = exec_start.clone();
        ensure_default_js_envs(app.framework, app.internal_port, &mut validated_envs);
        let content = render_systemd_unit(
            &app.name,
            &domain,
            safe_working.as_path(),
            &exec_start,
            memory_limit_mb,
            &validated_envs,
        );
        atomic_write_file(&unit_path, content.as_bytes(), 0o644)?;
        let executor = webserver.executor();
        let reload = executor
            .run(AllowedBinary::Systemctl, &["daemon-reload"], SYSTEMD_TIMEOUT)
            .await?;
        if !reload.success {
            return Err(PanelError::CommandFailed {
                binary: "systemctl",
                exit_code: reload.exit_code,
                stderr: reload.stderr,
            });
        }
        let current = query_unit_metrics(executor.as_ref(), &unit_file).await?;
        if current.active {
            let restart = executor
                .run(AllowedBinary::Systemctl, &["restart", &unit_file], SYSTEMD_TIMEOUT)
                .await?;
            if !restart.success {
                return Err(PanelError::CommandFailed {
                    binary: "systemctl",
                    exit_code: restart.exit_code,
                    stderr: restart.stderr,
                });
            }
        }
    }

    app.workers = workers;
    app.memory_limit_mb = memory_limit_mb;
    app.rate_limit_rps = rate_limit_rps;
    if let Some(ssl) = ssl_enabled {
        app.ssl_enabled = ssl;
    }
    if let Some(h3) = http3_quic {
        app.http3_quic = h3;
    }
    if let Some(ws) = websocket_enabled {
        app.websocket_enabled = ws;
    }
    app.env_vars = env_vars;
    store.upsert_app(&app)?;

    Ok(enrich_app(app, config, webserver).await)
}

#[allow(clippy::too_many_arguments)]
pub async fn update_site_settings(
    id: &str,
    ssl_enabled: Option<bool>,
    force_https: Option<bool>,
    http3_quic: Option<bool>,
    websocket_enabled: Option<bool>,
    rate_limit_rps: Option<u32>,
    redirects: Option<Vec<crate::api::contract::RedirectRuleItem>>,
    hotlink_protection: Option<crate::api::contract::HotlinkProtectionConfig>,
    directory_auth: Option<Vec<crate::api::contract::DirectoryPrivacyItem>>,
    url_rewrite_preset: Option<RewritePresetType>,
    url_rewrite_rules: Option<String>,
    store: &ModuleStore,
    config: &Config,
    webserver: &WebServerManager,
) -> Result<ManagedApp, PanelError> {
    let mut app = store
        .get_app(id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{id}' not found")))?;
    let domain = Domain::parse(&app.domain)?;
    let mut aliases = Vec::new();
    for a in &app.aliases {
        aliases.push(Domain::parse(a)?);
    }
    let next_ssl = ssl_enabled.unwrap_or(app.ssl_enabled);
    let next_force = force_https.unwrap_or(app.force_https);
    let next_h3 = http3_quic.unwrap_or(app.http3_quic);
    let next_ws = websocket_enabled.unwrap_or(app.websocket_enabled);
    let next_rps = rate_limit_rps.unwrap_or(app.rate_limit_rps);

    if next_ssl {
        let mut cert_ready = webserver
            .ssl
            .site_cert_paths(&domain)
            .map(|(cert, key)| cert.is_file() && key.is_file())
            .unwrap_or(false);
        if !cert_ready
            && webserver
                .executor()
                .is_binary_available(AllowedBinary::Certbot)
        {
            if let Ok(issued) = webserver.ssl.issue_certificate(&domain, &aliases, None).await {
                cert_ready = issued.present;
            }
        }
        if !cert_ready {
            return Err(PanelError::Validation(format!(
                "SSL certificate not found for {} — ensure DNS A/AAAA points to this VPS and issue SSL first",
                domain.as_str()
            )));
        }
    }

    let working = SafePath::under_root(&config.wwwroot, &app.working_dir)?;
    let effective_doc_root =
        resolve_effective_doc_root(working.as_path(), &app.run_sub_dir, app.framework);
    let mut spec = build_site_spec_for_app(
        domain,
        aliases,
        &effective_doc_root,
        &config.wwwroot,
        app.framework,
        app.internal_port,
        next_ssl,
        next_force,
    )?;
    spec.http3_quic = next_h3;
    spec.websocket_enabled = next_ws;
    spec.rate_limit_rps = next_rps;

    let rendered = webserver.upsert_and_apply_site(&spec, None).await?;
    app.ssl_enabled = next_ssl;
    app.force_https = next_force;
    app.http3_quic = next_h3;
    app.websocket_enabled = next_ws;
    app.rate_limit_rps = next_rps;
    if let Some(r) = redirects {
        app.redirects = Some(r);
    }
    if let Some(h) = hotlink_protection {
        app.hotlink_protection = Some(h);
    }
    if let Some(d) = directory_auth {
        app.directory_auth = Some(d);
    }
    if let Some(preset) = url_rewrite_preset {
        app.url_rewrite_preset = preset;
    }
    app.url_rewrite_rules = url_rewrite_rules.unwrap_or(rendered.content);
    app.nginx_vhost = rendered.config_path.display().to_string();
    store.upsert_app(&app)?;
    Ok(enrich_app(app, config, webserver).await)
}

pub async fn renew_ssl(
    id: &str,
    store: &ModuleStore,
    config: &Config,
    webserver: &WebServerManager,
) -> Result<ManagedApp, PanelError> {
    let mut app = store
        .get_app(id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{id}' not found")))?;
    let domain = Domain::parse(&app.domain)?;
    let mut aliases = Vec::with_capacity(app.aliases.len());
    for a in &app.aliases {
        let trimmed = a.trim();
        if !trimmed.is_empty() {
            aliases.push(Domain::parse(trimmed)?);
        }
    }

    let current_cert = webserver.ssl.read_cert_status(&domain).await?;
    if !current_cert.present {
        webserver
            .ssl
            .issue_certificate(&domain, &aliases, None)
            .await?;
    } else {
        webserver.ssl.renew_all().await?;
    }

    let updated_cert = webserver.ssl.read_cert_status(&domain).await?;
    if updated_cert.present && !app.ssl_enabled {
        let working = SafePath::under_root(&config.wwwroot, &app.working_dir)?;
        let effective_doc_root =
            resolve_effective_doc_root(working.as_path(), &app.run_sub_dir, app.framework);
        let mut spec = build_site_spec_for_app(
            domain,
            aliases,
            &effective_doc_root,
            &config.wwwroot,
            app.framework,
            app.internal_port,
            true,
            app.force_https,
        )?;
        spec.http3_quic = app.http3_quic;
        spec.websocket_enabled = app.websocket_enabled;
        spec.rate_limit_rps = app.rate_limit_rps;
        let rendered = webserver.upsert_and_apply_site(&spec, None).await?;
        app.ssl_enabled = true;
        app.url_rewrite_rules = rendered.content;
        app.nginx_vhost = rendered.config_path.display().to_string();
        store.upsert_app(&app)?;
    } else {
        webserver.reload_configured_engine().await?;
    }

    Ok(enrich_app(app, config, webserver).await)
}

pub async fn test_http_live(
    id: &str,
    store: &ModuleStore,
    webserver: &WebServerManager,
) -> Result<WebsiteLiveTestResult, PanelError> {
    let app = store
        .get_app(id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{id}' not found")))?;
    let domain = Domain::parse(&app.domain)?;
    let addr = webserver.probe_addr();
    let host_hdr = domain.as_str().to_string();
    let app_id_owned = app.id.clone();

    tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5)).map_err(|e| {
            PanelError::BadGateway(format!("TCP connect to {addr} failed for {host_hdr}: {e}"))
        })?;
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
        let req = format!(
            "GET / HTTP/1.1\r\nHost: {host_hdr}\r\nUser-Agent: ferrous-panel/1.0\r\nConnection: close\r\n\r\n"
        );
        stream.write_all(req.as_bytes()).map_err(|e| {
            PanelError::BadGateway(format!("HTTP write failed for {host_hdr}: {e}"))
        })?;
        let mut buf = [0u8; 4096];
        let mut total = 0usize;
        loop {
            match stream.read(&mut buf[total..]) {
                Ok(0) => break,
                Ok(n) => {
                    total += n;
                    if total == buf.len() {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(e) => {
                    return Err(PanelError::BadGateway(format!(
                        "HTTP read failed for {host_hdr}: {e}"
                    )));
                }
            }
        }
        let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let raw = String::from_utf8_lossy(&buf[..total]);
        let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((raw.as_ref(), ""));
        let mut lines = head.lines();
        let status_line = lines.next().unwrap_or("");
        let mut parts = status_line.splitn(3, ' ');
        let proto = parts.next().unwrap_or("HTTP/1.1").to_string();
        let status_code = parts
            .next()
            .and_then(|s| s.parse::<u16>().ok())
            .ok_or_else(|| PanelError::BadGateway(format!("invalid HTTP status line: {status_line}")))?;
        let status_text = parts.next().unwrap_or("").trim().to_string();

        let mut headers = BTreeMap::new();
        for hline in lines {
            if let Some((k, v)) = hline.split_once(':') {
                headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
            }
        }

        let body_preview: String = body.chars().take(512).collect();
        Ok(WebsiteLiveTestResult {
            app_id: app_id_owned,
            domain: host_hdr.clone(),
            url: format!("http://{host_hdr}/"),
            status_code,
            status_text,
            latency_ms: elapsed_ms,
            protocol: proto,
            tls_cipher: "none (plain HTTP probe)".to_string(),
            headers,
            body_preview,
            verified_at: Utc::now().to_rfc3339(),
        })
    })
    .await
    .map_err(|_| PanelError::BadGateway("HTTP probe worker cancelled".to_string()))?
}

pub async fn delete_app(
    id: &str,
    store: &ModuleStore,
    config: &Config,
    webserver: &WebServerManager,
) -> Result<Vec<ManagedApp>, PanelError> {
    let app = store
        .get_app(id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{id}' not found")))?;
    let domain = Domain::parse(&app.domain)?;

    if !app.systemd_unit.is_empty() {
        let unit = ServiceName::parse(&app.systemd_unit)?;
        let unit_file = unit.unit_file_name();
        let executor = webserver.executor();
        if executor.is_binary_available(AllowedBinary::Systemctl) {
            let _ = executor
                .run(
                    AllowedBinary::Systemctl,
                    &["disable", "--now", &unit_file],
                    SYSTEMD_TIMEOUT,
                )
                .await;
        }
        let unit_path: PathBuf = config.systemd_unit_dir.join(&unit_file);
        if unit_path.exists() {
            let _ = std::fs::remove_file(&unit_path);
        }
        if executor.is_binary_available(AllowedBinary::Systemctl) {
            let _ = executor
                .run(AllowedBinary::Systemctl, &["daemon-reload"], SYSTEMD_TIMEOUT)
                .await;
        }
    }

    let _ = webserver.remove_and_apply_site(&domain).await;
    store.delete_app(id)?;
    list_apps(store, config, webserver).await
}
