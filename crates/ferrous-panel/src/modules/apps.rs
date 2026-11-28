use crate::api::contract::{
    AppStatus, DeployWebsiteRequest, EnvVar, ManagedApp, PhpVersionTag, RewritePresetType,
    WebServerEngineType, WebsiteLiveTestResult, WebsiteRuntimeType,
};
use crate::config::Config;
use crate::error::PanelError;
use crate::modules::ModuleStore;
use crate::system::cmd::{validate_no_flag_injection, AllowedBinary};
use crate::system::paths::SafePath;
use crate::system::validate::{Domain, Port, ServiceName};
use crate::webserver::ssl::{
    SiteSslStatus, SslCertState, SslDnsPreflight, SslRenewalTimerStatus,
};
use crate::webserver::{
    atomic_write_file, AcmeEmail, CommandExecutor, EngineKind, FileSnapshot, PhpFpmVersion,
    SafeFilename, SiteDirectoryAuth, SiteHotlinkProtection, SiteMode, SiteRedirectRule, SiteSpec,
    UpstreamHost, UpstreamTarget, WebServerEngine, WebServerManager, WwwRootPath,
};
use chrono::Utc;
use std::collections::BTreeMap;
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
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

fn is_rust_runtime(fw: WebsiteRuntimeType) -> bool {
    matches!(
        fw,
        WebsiteRuntimeType::ActixWebRust
            | WebsiteRuntimeType::AxumRust
            | WebsiteRuntimeType::RocketRust
    )
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

/// Detects the compiled release binary inside `<working_dir>/target/release/` from `Cargo.toml`.
fn detect_rust_release_binary(working_dir: &Path) -> Option<PathBuf> {
    let cargo_toml = std::fs::read_to_string(working_dir.join("Cargo.toml")).ok()?;
    let mut in_pkg = false;
    let mut in_bin = false;
    let mut bin_names: Vec<String> = Vec::new();
    let mut pkg_name: Option<String> = None;

    for raw_line in cargo_toml.lines() {
        let line = raw_line.trim();
        if line.starts_with("[[bin]]") {
            in_bin = true;
            in_pkg = false;
            continue;
        }
        if line.starts_with("[package]") {
            in_pkg = true;
            in_bin = false;
            continue;
        }
        if line.starts_with('[') {
            in_pkg = false;
            in_bin = false;
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == "name" {
                let cleaned = v
                    .split('#')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .trim();
                if !cleaned.is_empty() {
                    if in_bin {
                        bin_names.push(cleaned.to_string());
                    } else if in_pkg && pkg_name.is_none() {
                        pkg_name = Some(cleaned.to_string());
                    }
                }
            }
        }
    }

    if let Some(pkg) = pkg_name {
        bin_names.push(pkg);
    }

    for name in bin_names {
        let candidate = working_dir.join("target/release").join(&name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Automatically normalizes framework execution commands:
/// - Converts legacy `node`/`npm`/`pnpm`/`yarn`/`npx` commands (or empty JS exec commands)
///   into Zero-Node.js native `bun --bun` / `deno` commands.
/// - Auto-resolves compiled `target/release/<bin>` binaries for Rust frameworks.
fn normalize_zero_node_exec_command(
    framework: WebsiteRuntimeType,
    raw_exec: &str,
    working_dir: &Path,
) -> (String, Option<String>) {
    let trimmed = raw_exec.trim();

    if is_rust_runtime(framework) {
        if let Some(detected_bin) = detect_rust_release_binary(working_dir) {
            let bin_str = detected_bin.display().to_string();
            let first_token = trimmed.split_whitespace().next().unwrap_or("");
            let needs_override = trimmed.is_empty()
                || first_token == "/usr/bin/env"
                || (!first_token.is_empty() && !Path::new(first_token).is_file());
            if needs_override {
                return (
                    bin_str.clone(),
                    Some(format!(
                        "Rust release binary auto-detected after Cargo build -> {bin_str}"
                    )),
                );
            }
        }
        return (trimmed.to_string(), None);
    }

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
            WebsiteRuntimeType::Nuxt3 => {
                if working_dir.join(".output/server/index.mjs").is_file() {
                    format!("{bun} {dir_str}/.output/server/index.mjs")
                } else if package_json_has_script(working_dir, "start") {
                    format!("{bun} --bun run start")
                } else {
                    format!("{bun} {dir_str}/.output/server/index.mjs")
                }
            }
            WebsiteRuntimeType::SvelteKit2 => {
                if working_dir.join("build/index.js").is_file() {
                    format!("{bun} {dir_str}/build/index.js")
                } else if package_json_has_script(working_dir, "start") {
                    format!("{bun} --bun run start")
                } else {
                    format!("{bun} {dir_str}/build/index.js")
                }
            }
            WebsiteRuntimeType::Astro5 => {
                if working_dir.join("dist/server/entry.mjs").is_file() {
                    format!("{bun} {dir_str}/dist/server/entry.mjs")
                } else if package_json_has_script(working_dir, "start") {
                    format!("{bun} --bun run start")
                } else if package_json_has_script(working_dir, "preview") {
                    format!("{bun} --bun run preview -- --host 127.0.0.1")
                } else {
                    format!("{bun} {dir_str}/dist/server/entry.mjs")
                }
            }
            WebsiteRuntimeType::RemixReactRouterV7 => {
                if working_dir.join("build/server/index.js").is_file() {
                    format!("{bun} --bun x react-router-serve {dir_str}/build/server/index.js")
                } else {
                    format!("{bun} --bun run start")
                }
            }
            WebsiteRuntimeType::NestJs10 => {
                if working_dir.join("dist/main.js").is_file() {
                    format!("{bun} {dir_str}/dist/main.js")
                } else if working_dir.join("dist/src/main.js").is_file() {
                    format!("{bun} {dir_str}/dist/src/main.js")
                } else {
                    format!("{bun} --bun run start")
                }
            }
            WebsiteRuntimeType::BunElysiaJs => {
                if working_dir.join("src/index.ts").is_file() {
                    format!("{bun} run {dir_str}/src/index.ts")
                } else if package_json_has_script(working_dir, "start") {
                    format!("{bun} --bun run start")
                } else {
                    format!("{bun} run {dir_str}/src/index.ts")
                }
            }
            WebsiteRuntimeType::Deno2 => {
                format!("{deno} run --allow-net --allow-env --allow-read {dir_str}/main.ts")
            }
            _ => format!("{bun} --bun run start"),
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
                format!("{bun} --bun run start")
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
            let rewritten = if rest.is_empty() || rest.first().copied() == Some("start") {
                format!("{bun} --bun run start")
            } else if rest.first().copied() == Some("run") {
                format!("{bun} --bun {}", rest.join(" "))
            } else {
                format!("{bun} --bun run {}", rest.join(" "))
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
                format!("{bun} --bun run start")
            } else {
                format!("{bun} --bun x {}", rest.join(" "))
            };
            (
                rewritten.clone(),
                Some(format!(
                    "Zero-Node.js mode: translated npx command -> {rewritten}"
                )),
            )
        }
        "bun" => {
            let rewritten = if rest.is_empty() {
                format!("{bun} --bun run start")
            } else if matches!(rest.first().copied(), Some("run" | "x")) {
                format!("{bun} --bun {}", rest.join(" "))
            } else {
                format!("{bun} {}", rest.join(" "))
            };
            (rewritten, None)
        }
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
    let port_str = internal_port.to_string();
    if is_js_ssr_runtime(framework) {
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
    } else if is_rust_runtime(framework) {
        let defaults = [
            ("PORT", port_str.as_str()),
            ("HOST", "127.0.0.1"),
            ("ROCKET_PORT", port_str.as_str()),
            ("ROCKET_ADDRESS", "127.0.0.1"),
            ("RUST_LOG", "info"),
        ];
        for (k, v) in defaults {
            if !envs.iter().any(|(ek, _)| ek == k) {
                envs.push((k.to_string(), v.to_string()));
            }
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
        if explicit.is_dir()
            && explicit.starts_with(working_dir)
            && (is_php_runtime(framework) || explicit.join("index.html").is_file())
        {
            return explicit;
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

/// Executes a real Zero-Node.js JS/TS/CSS or native Rust Cargo build directly via allowlisted
/// native binaries (`bun --bun`, `tailwindcss` standalone CLI, `deno`, or `cargo build --release`).
async fn run_zero_node_js_css_build(
    working_dir: &Path,
    framework: WebsiteRuntimeType,
    executor: &dyn CommandExecutor,
) -> Vec<String> {
    let mut logs = Vec::new();
    let now = Utc::now().to_rfc3339();

    if is_rust_runtime(framework) {
        if working_dir.join("Cargo.toml").is_file() {
            if executor.is_binary_available(AllowedBinary::Cargo) {
                logs.push(format!(
                    "[{now}] Rust framework build: running `cargo build --release`..."
                ));
                match executor
                    .run_with_env(
                        AllowedBinary::Cargo,
                        &["build", "--release"],
                        &[("PATH", "/usr/local/bin:/root/.cargo/bin:/usr/bin:/bin")],
                        Some(working_dir),
                        BUILD_TIMEOUT,
                    )
                    .await
                {
                    Ok(out) if out.success => {
                        logs.push(format!(
                            "[{}] Native Rust build (`cargo build --release`) succeeded.",
                            Utc::now().to_rfc3339()
                        ));
                    }
                    Ok(out) => {
                        let detail = if out.stderr.is_empty() {
                            out.stdout
                        } else {
                            out.stderr
                        };
                        logs.push(format!(
                            "[{}] `cargo build --release` exited ({:?}): {}",
                            Utc::now().to_rfc3339(),
                            out.exit_code,
                            detail.chars().take(400).collect::<String>()
                        ));
                    }
                    Err(err) => {
                        logs.push(format!(
                            "[{}] `cargo build --release` failed: {err}",
                            Utc::now().to_rfc3339()
                        ));
                    }
                }
            } else {
                logs.push(format!(
                    "[{now}] Detected Cargo.toml — Rust cargo toolchain not found on host."
                ));
            }
        }
        return logs;
    }

    if is_php_runtime(framework)
        && working_dir.join("composer.json").is_file()
        && !working_dir.join("vendor/autoload.php").is_file()
        && executor.is_binary_available(AllowedBinary::Composer)
    {
        logs.push(format!(
            "[{now}] PHP Composer project detected: running `composer install --no-dev --optimize-autoloader`..."
        ));
        match executor
            .run_with_env(
                AllowedBinary::Composer,
                &[
                    "install",
                    "--no-interaction",
                    "--no-progress",
                    "--prefer-dist",
                    "--no-dev",
                    "--optimize-autoloader",
                ],
                &[
                    ("COMPOSER_ALLOW_SUPERUSER", "1"),
                    ("PATH", "/usr/local/bin:/usr/bin:/bin"),
                ],
                Some(working_dir),
                BUILD_TIMEOUT,
            )
            .await
        {
            Ok(out) if out.success => {
                logs.push(format!(
                    "[{}] `composer install` completed cleanly.",
                    Utc::now().to_rfc3339()
                ));
            }
            Ok(out) => {
                let detail = if out.stderr.is_empty() {
                    out.stdout
                } else {
                    out.stderr
                };
                logs.push(format!(
                    "[{}] `composer install` exited ({:?}): {}",
                    Utc::now().to_rfc3339(),
                    out.exit_code,
                    detail.chars().take(300).collect::<String>()
                ));
            }
            Err(err) => {
                logs.push(format!(
                    "[{}] `composer install` error: {err}",
                    Utc::now().to_rfc3339()
                ));
            }
        }
    }

    if !is_js_or_css_runtime(framework) {
        return logs;
    }

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
                    &[
                        ("NODE_ENV", "development"),
                        ("PATH", "/usr/local/bin:/usr/bin:/bin"),
                    ],
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
                                &["--bun", "run", "build"],
                                &[
                                    ("NODE_ENV", "production"),
                                    ("PATH", "/usr/local/bin:/usr/bin:/bin"),
                                ],
                                Some(working_dir),
                                BUILD_TIMEOUT,
                            )
                            .await
                        {
                            Ok(build_out) if build_out.success => {
                                logs.push(format!(
                                    "[{}] Zero-Node.js framework build (`bun --bun run build`) completed cleanly.",
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
    if !env_vars.iter().any(|(k, _)| k == "PATH") {
        out.push_str("Environment=\"PATH=/usr/local/bin:/usr/bin:/bin\"\n");
    }
    if !env_vars.iter().any(|(k, _)| k == "HOME") {
        out.push_str("Environment=\"HOME=/tmp\"\n");
    }
    for (k, v) in env_vars {
        let escaped = v.replace('\\', "\\\\").replace('"', "\\\"");
        out.push_str(&format!("Environment=\"{k}={escaped}\"\n"));
    }
    out.push_str("\n[Install]\n");
    out.push_str("WantedBy=multi-user.target\n");
    out
}

async fn ensure_wwwroot_ownership(
    working_dir: &Path,
    config: &Config,
    executor: &dyn CommandExecutor,
) {
    if config.is_production() && executor.is_binary_available(AllowedBinary::Chown) {
        let dir_str = working_dir.display().to_string();
        if let Ok(out) = executor
            .run(
                AllowedBinary::Chown,
                &["-R", "www-data:www-data", "--", &dir_str],
                SYSTEMD_TIMEOUT,
            )
            .await
        {
            if !out.success {
                tracing::warn!(
                    path = %dir_str,
                    stderr = %out.stderr,
                    "chown www-data on wwwroot returned non-zero"
                );
            }
        }
    }
}

async fn verify_unit_active_or_journal(
    executor: &dyn CommandExecutor,
    unit_file: &str,
) -> Result<(), PanelError> {
    let active_out = executor
        .run(
            AllowedBinary::Systemctl,
            &["is-active", unit_file],
            SYSTEMD_TIMEOUT,
        )
        .await?;
    if active_out.success && active_out.stdout.trim() == "active" {
        return Ok(());
    }
    let journal_tail = if executor.is_binary_available(AllowedBinary::Journalctl) {
        executor
            .run(
                AllowedBinary::Journalctl,
                &["-u", unit_file, "-n", "30", "--no-pager"],
                SYSTEMD_TIMEOUT,
            )
            .await
            .map(|o| {
                if !o.stdout.trim().is_empty() {
                    o.stdout.trim().to_string()
                } else {
                    o.stderr.trim().to_string()
                }
            })
            .unwrap_or_default()
    } else {
        String::new()
    };
    let state_txt = if active_out.stdout.trim().is_empty() {
        active_out.stderr.trim().to_string()
    } else {
        active_out.stdout.trim().to_string()
    };
    let detail = if journal_tail.is_empty() {
        format!("service {unit_file} is not active (state={state_txt})")
    } else {
        format!("service {unit_file} failed to stay active (state={state_txt}):\n{journal_tail}")
    };
    Err(PanelError::CommandFailed {
        binary: "systemctl",
        exit_code: active_out.exit_code,
        stderr: detail,
    })
}

fn normalize_git_repo_and_branch(
    repo_raw: &str,
    branch_raw: &str,
) -> Result<(String, Option<String>), PanelError> {
    let trimmed = validate_no_flag_injection(repo_raw, "gitRepoUrl")?;
    let mut url = trimmed.trim_end_matches('/').to_string();
    let mut extracted_branch: Option<String> = None;

    // Convert SSH `git@github.com:owner/repo.git` to `https://github.com/owner/repo.git`
    if let Some(rest) = url.strip_prefix("git@") {
        if let Some((host, path)) = rest.split_once(':') {
            if !host.is_empty() && !path.is_empty() {
                url = format!("https://{host}/{}", path.trim_start_matches('/'));
            }
        }
    } else if let Some(rest) = url.strip_prefix("ssh://git@") {
        if let Some((host, path)) = rest.split_once('/') {
            if !host.is_empty() && !path.is_empty() {
                let clean_host = host.split(':').next().unwrap_or(host);
                url = format!("https://{clean_host}/{path}");
            }
        }
    }

    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(PanelError::Validation(
            "gitRepoUrl must start with https://, http://, or git@".to_string(),
        ));
    }

    // Handle browser URLs such as `https://github.com/owner/repo/tree/main` or `/blob/dev/...`
    for marker in ["/-/tree/", "/tree/", "/-/blob/", "/blob/"] {
        if let Some((base, tail)) = url.split_once(marker) {
            let seg = tail.split('/').next().unwrap_or("").trim();
            if !seg.is_empty() {
                extracted_branch = Some(seg.to_string());
            }
            url = base.trim_end_matches('/').to_string();
            break;
        }
    }

    let explicit_branch = branch_raw.trim();
    let branch_opt = if !explicit_branch.is_empty() {
        let b = validate_no_flag_injection(explicit_branch, "gitBranch")?;
        Some(b.to_string())
    } else {
        extracted_branch
    };

    if let Some(ref b) = branch_opt {
        if !b
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-' | b'/'))
            || b.contains("..")
        {
            return Err(PanelError::Validation(
                "gitBranch contains invalid characters".to_string(),
            ));
        }
    }

    Ok((url, branch_opt))
}

async fn clone_or_sync_git_repository(
    executor: &dyn CommandExecutor,
    working_dir: &Path,
    repo_raw: &str,
    branch_raw: &str,
    auto_install_git: bool,
) -> Result<(String, String, String), PanelError> {
    let (safe_repo, branch_opt) = normalize_git_repo_and_branch(repo_raw, branch_raw)?;
    if !executor.is_binary_available(AllowedBinary::Git)
        && auto_install_git
        && executor.is_binary_available(AllowedBinary::AptGet)
    {
        let noninteractive = [("DEBIAN_FRONTEND", "noninteractive")];
        let _ = executor
            .run_with_env(
                AllowedBinary::AptGet,
                &["update", "-qq"],
                &noninteractive,
                None,
                BUILD_TIMEOUT,
            )
            .await;
        let _ = executor
            .run_with_env(
                AllowedBinary::AptGet,
                &["install", "-y", "-qq", "git", "ca-certificates"],
                &noninteractive,
                None,
                BUILD_TIMEOUT,
            )
            .await;
    }

    if !executor.is_binary_available(AllowedBinary::Git) {
        return Err(PanelError::Conflict(
            "Chưa cài đặt `git` trên máy chủ (chạy: sudo apt-get install -y git).".to_string(),
        ));
    }

    std::fs::create_dir_all(working_dir)?;
    let git_env = [("GIT_TERMINAL_PROMPT", "0")];

    if working_dir.join(".git").exists() {
        let _ = executor
            .run_with_env(
                AllowedBinary::Git,
                &["remote", "set-url", "origin", "--", &safe_repo],
                &git_env,
                Some(working_dir),
                SYSTEMD_TIMEOUT,
            )
            .await;

        let target_branch = branch_opt.as_deref().unwrap_or("HEAD");
        let pull_out = if let Some(ref b) = branch_opt {
            let fetch = executor
                .run_with_env(
                    AllowedBinary::Git,
                    &["fetch", "--depth", "1", "origin", b],
                    &git_env,
                    Some(working_dir),
                    BUILD_TIMEOUT,
                )
                .await?;
            if fetch.success {
                executor
                    .run_with_env(
                        AllowedBinary::Git,
                        &["reset", "--hard", "FETCH_HEAD"],
                        &git_env,
                        Some(working_dir),
                        SYSTEMD_TIMEOUT,
                    )
                    .await?
            } else {
                executor
                    .run_with_env(
                        AllowedBinary::Git,
                        &["pull", "--ff-only", "--", "origin", b],
                        &git_env,
                        Some(working_dir),
                        BUILD_TIMEOUT,
                    )
                    .await?
            }
        } else {
            executor
                .run_with_env(
                    AllowedBinary::Git,
                    &["pull", "--ff-only"],
                    &git_env,
                    Some(working_dir),
                    BUILD_TIMEOUT,
                )
                .await?
        };
        if !pull_out.success {
            return Err(PanelError::CommandFailed {
                binary: "git",
                exit_code: pull_out.exit_code,
                stderr: format!(
                    "git pull failed for '{safe_repo}' (branch '{target_branch}'): {}",
                    pull_out.stderr.trim()
                ),
            });
        }
        return Ok((
            safe_repo.clone(),
            target_branch.to_string(),
            format!(
                "[{}] Synced repository {safe_repo} (branch '{target_branch}')",
                Utc::now().to_rfc3339()
            ),
        ));
    }

    let dir_empty = std::fs::read_dir(working_dir)
        .map(|mut rd| rd.next().is_none())
        .unwrap_or(true);

    if dir_empty {
        let dest_str = working_dir.display().to_string();
        let mut clone_out = if let Some(ref b) = branch_opt {
            executor
                .run_with_env(
                    AllowedBinary::Git,
                    &[
                        "clone",
                        "--depth",
                        "1",
                        "--branch",
                        b,
                        "--",
                        &safe_repo,
                        &dest_str,
                    ],
                    &git_env,
                    None,
                    BUILD_TIMEOUT,
                )
                .await?
        } else {
            executor
                .run_with_env(
                    AllowedBinary::Git,
                    &["clone", "--depth", "1", "--", &safe_repo, &dest_str],
                    &git_env,
                    None,
                    BUILD_TIMEOUT,
                )
                .await?
        };

        // If user left default branch 'main' on a repo whose default branch is 'master', retry without --branch
        if !clone_out.success && branch_opt.is_some() {
            let empty_after_fail = std::fs::read_dir(working_dir)
                .map(|mut rd| rd.next().is_none())
                .unwrap_or(true);
            if empty_after_fail {
                if let Ok(fallback_out) = executor
                    .run_with_env(
                        AllowedBinary::Git,
                        &["clone", "--depth", "1", "--", &safe_repo, &dest_str],
                        &git_env,
                        None,
                        BUILD_TIMEOUT,
                    )
                    .await
                {
                    if fallback_out.success {
                        clone_out = fallback_out;
                    }
                }
            }
        }

        if !clone_out.success {
            return Err(PanelError::CommandFailed {
                binary: "git",
                exit_code: clone_out.exit_code,
                stderr: format!(
                    "git clone failed for '{safe_repo}': {}",
                    clone_out.stderr.trim()
                ),
            });
        }
    } else {
        // Directory already has placeholder files (e.g. index.html): initialize git in-place and reset to remote HEAD
        let _ = executor
            .run_with_env(AllowedBinary::Git, &["init"], &git_env, Some(working_dir), SYSTEMD_TIMEOUT)
            .await;
        let _ = executor
            .run_with_env(
                AllowedBinary::Git,
                &["remote", "remove", "origin"],
                &git_env,
                Some(working_dir),
                SYSTEMD_TIMEOUT,
            )
            .await;
        let _ = executor
            .run_with_env(
                AllowedBinary::Git,
                &["remote", "add", "origin", "--", &safe_repo],
                &git_env,
                Some(working_dir),
                SYSTEMD_TIMEOUT,
            )
            .await;
        let mut fetch_out = if let Some(ref b) = branch_opt {
            executor
                .run_with_env(
                    AllowedBinary::Git,
                    &["fetch", "--depth", "1", "origin", b],
                    &git_env,
                    Some(working_dir),
                    BUILD_TIMEOUT,
                )
                .await?
        } else {
            executor
                .run_with_env(
                    AllowedBinary::Git,
                    &["fetch", "--depth", "1", "origin", "HEAD"],
                    &git_env,
                    Some(working_dir),
                    BUILD_TIMEOUT,
                )
                .await?
        };
        if !fetch_out.success && branch_opt.is_some() {
            if let Ok(fb) = executor
                .run_with_env(
                    AllowedBinary::Git,
                    &["fetch", "--depth", "1", "origin", "HEAD"],
                    &git_env,
                    Some(working_dir),
                    BUILD_TIMEOUT,
                )
                .await
            {
                if fb.success {
                    fetch_out = fb;
                }
            }
        }
        if !fetch_out.success {
            return Err(PanelError::CommandFailed {
                binary: "git",
                exit_code: fetch_out.exit_code,
                stderr: format!(
                    "git fetch failed for '{safe_repo}': {}",
                    fetch_out.stderr.trim()
                ),
            });
        }
        let reset_out = executor
            .run_with_env(
                AllowedBinary::Git,
                &["reset", "--hard", "FETCH_HEAD"],
                &git_env,
                Some(working_dir),
                SYSTEMD_TIMEOUT,
            )
            .await?;
        if !reset_out.success {
            return Err(PanelError::CommandFailed {
                binary: "git",
                exit_code: reset_out.exit_code,
                stderr: reset_out.stderr,
            });
        }
    }

    let effective_branch = branch_opt.unwrap_or_else(|| "default".to_string());
    Ok((
        safe_repo.clone(),
        effective_branch.clone(),
        format!(
            "[{}] Cloned repository {safe_repo} (branch '{effective_branch}')",
            Utc::now().to_rfc3339()
        ),
    ))
}

fn map_site_rules_to_spec(
    spec: &mut SiteSpec,
    redirects: Option<&[crate::api::contract::RedirectRuleItem]>,
    hotlink: Option<&crate::api::contract::HotlinkProtectionConfig>,
    directory_auth: Option<&[crate::api::contract::DirectoryPrivacyItem]>,
) {
    if let Some(rules) = redirects {
        spec.redirects = rules
            .iter()
            .map(|r| SiteRedirectRule {
                source_path: r.source_path.clone(),
                target_url: r.target_url.clone(),
                status_code: r.redirect_type.trim().parse::<u16>().unwrap_or(301),
                keep_query: r.keep_query,
                enabled: r.enabled,
            })
            .collect();
    }
    if let Some(hl) = hotlink {
        spec.hotlink_protection = Some(SiteHotlinkProtection {
            enabled: hl.enabled,
            protected_extensions: hl.protected_extensions.clone(),
            allowed_domains: hl.allowed_domains.clone(),
            block_empty_referer: hl.block_empty_referer,
        });
    }
    if let Some(auths) = directory_auth {
        spec.directory_auth = auths
            .iter()
            .map(|a| SiteDirectoryAuth {
                path: a.path.clone(),
                label: a.label.clone(),
                username: a.username.clone(),
                password: a.password.clone(),
                enabled: a.enabled,
            })
            .collect();
    }
}

#[derive(Debug, Clone, Default)]
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

fn php_fpm_from_tag(tag: Option<PhpVersionTag>) -> PhpFpmVersion {
    match tag {
        Some(PhpVersionTag::V84) => PhpFpmVersion::V84,
        Some(PhpVersionTag::V82) => PhpFpmVersion::V82,
        Some(PhpVersionTag::V81) => PhpFpmVersion::V81,
        Some(PhpVersionTag::V74) => PhpFpmVersion::V74,
        Some(PhpVersionTag::V83) | Some(PhpVersionTag::None) | None => PhpFpmVersion::V83,
    }
}

fn php_tag_from_fpm(ver: PhpFpmVersion) -> PhpVersionTag {
    match ver {
        PhpFpmVersion::V84 => PhpVersionTag::V84,
        PhpFpmVersion::V83 => PhpVersionTag::V83,
        PhpFpmVersion::V82 => PhpVersionTag::V82,
        PhpFpmVersion::V81 => PhpVersionTag::V81,
        PhpFpmVersion::V74 => PhpVersionTag::V74,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_site_spec_for_app(
    domain: Domain,
    aliases: Vec<Domain>,
    working_path: &Path,
    wwwroot: &Path,
    framework: WebsiteRuntimeType,
    php_version: PhpFpmVersion,
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
            php_version,
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
        redirects: Vec::new(),
        hotlink_protection: None,
        directory_auth: Vec::new(),
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

fn dir_size_cache() -> &'static std::sync::Mutex<std::collections::HashMap<PathBuf, (std::time::Instant, f64)>> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<PathBuf, (std::time::Instant, f64)>>,
    > = std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

async fn cached_dir_size_mb(path: &Path) -> f64 {
    if let Ok(guard) = dir_size_cache().lock() {
        if let Some(&(ts, size)) = guard.get(path) {
            if ts.elapsed() < Duration::from_secs(60) {
                return size;
            }
        }
    }
    let owned = path.to_path_buf();
    let computed = tokio::task::spawn_blocking(move || dir_size_mb(&owned))
        .await
        .unwrap_or(0.0);
    if let Ok(mut guard) = dir_size_cache().lock() {
        if guard.len() > 256 {
            guard.clear();
        }
        guard.insert(path.to_path_buf(), (std::time::Instant::now(), computed));
    }
    computed
}

pub async fn enrich_app(
    app: ManagedApp,
    config: &Config,
    webserver: &WebServerManager,
) -> ManagedApp {
    enrich_app_with_engine_hint(app, config, webserver, None).await
}

async fn enrich_app_with_engine_hint(
    mut app: ManagedApp,
    config: &Config,
    webserver: &WebServerManager,
    precomputed_engine_active: Option<bool>,
) -> ManagedApp {
    let unit_to_query = if is_php_runtime(app.framework) {
        php_fpm_from_tag(app.php_version).systemd_unit().to_string()
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
    } else if let Some(engine_active) = precomputed_engine_active {
        app.pid = None;
        app.memory_used_mb = 0;
        app.status = if engine_active {
            AppStatus::Running
        } else {
            AppStatus::Stopped
        };
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
        app.disk_used_mb = cached_dir_size_mb(safe_dir.as_path()).await;
    }

    if let Ok(domain) = Domain::parse(&app.domain) {
        if let Ok(ssl) = webserver.ssl.read_cert_status(&domain).await {
            app.ssl_enabled = app.ssl_enabled && ssl.present;
            let provider_tag = ssl.provider.as_deref().unwrap_or("");
            app.ssl_issuer = if provider_tag == "Self-Signed" {
                "Self-Signed (OpenSSL)".to_string()
            } else {
                ssl.issuer.unwrap_or_else(|| {
                    if ssl.present {
                        "Let's Encrypt".to_string()
                    } else {
                        "None".to_string()
                    }
                })
            };
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
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let engine_active = match webserver.store.load_config() {
        Ok(cfg) => Some(match cfg.frontend_engine {
            EngineKind::Nginx => webserver.nginx.status().await.map(|s| s.active).unwrap_or(false),
            EngineKind::Ferrousx => webserver
                .ferrousx
                .status()
                .await
                .map(|s| s.active)
                .unwrap_or(false),
        }),
        Err(_) => None,
    };

    let mut unit_cache: std::collections::HashMap<String, SystemdRuntimeMetrics> =
        std::collections::HashMap::new();
    let mut enriched = Vec::with_capacity(raw.len());
    for mut app in raw {
        let unit_to_query = if is_php_runtime(app.framework) {
            php_fpm_from_tag(app.php_version).systemd_unit().to_string()
        } else {
            app.systemd_unit.clone()
        };
        if !is_static_runtime(app.framework) && !unit_to_query.is_empty() {
            let metrics = if let Some(m) = unit_cache.get(&unit_to_query) {
                Some(m.clone())
            } else if let Ok(m) =
                query_unit_metrics(webserver.executor().as_ref(), &unit_to_query).await
            {
                unit_cache.insert(unit_to_query.clone(), m.clone());
                Some(m)
            } else {
                None
            };
            if let Some(m) = metrics {
                app.pid = m.pid;
                app.memory_used_mb = m.memory_used_mb;
                app.status = if m.active {
                    AppStatus::Running
                } else {
                    AppStatus::Stopped
                };
            }
        } else if let Some(active) = engine_active {
            app.pid = None;
            app.memory_used_mb = 0;
            app.status = if active {
                AppStatus::Running
            } else {
                AppStatus::Stopped
            };
        }

        if let Ok(safe_dir) = SafePath::under_root(&config.wwwroot, &app.working_dir) {
            app.disk_used_mb = cached_dir_size_mb(safe_dir.as_path()).await;
        }

        if let Ok(domain) = Domain::parse(&app.domain) {
            if let Ok(ssl) = webserver.ssl.read_cert_status(&domain).await {
                app.ssl_enabled = app.ssl_enabled && ssl.present;
                let provider_tag = ssl.provider.as_deref().unwrap_or("");
                app.ssl_issuer = if provider_tag == "Self-Signed" {
                    "Self-Signed (OpenSSL)".to_string()
                } else {
                    ssl.issuer.unwrap_or_else(|| {
                        if ssl.present {
                            "Let's Encrypt".to_string()
                        } else {
                            "None".to_string()
                        }
                    })
                };
                app.ssl_expiry_days = match ssl.status {
                    SslCertState::Missing => 0,
                    _ => ssl.days_remaining.unwrap_or(0),
                };
            }
        }
        enriched.push(app);
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
    let mut normalized_repo_url = trimmed_repo.to_string();
    let mut normalized_branch_name = req.git_branch.trim().to_string();
    if !trimmed_repo.is_empty()
        && (config.is_production()
            || webserver
                .executor()
                .is_binary_available(AllowedBinary::Git))
    {
        let (clean_url, clean_branch, git_log) = clone_or_sync_git_repository(
            webserver.executor().as_ref(),
            safe_working.as_path(),
            trimmed_repo,
            &req.git_branch,
            config.is_production(),
        )
        .await?;
        normalized_repo_url = clean_url;
        if clean_branch != "default" && clean_branch != "HEAD" {
            normalized_branch_name = clean_branch;
        }
        initial_logs.push(git_log);
    }

    // Run Zero-Node.js JS/TS/CSS or native Rust Cargo build if source exists.
    let build_logs = run_zero_node_js_css_build(
        safe_working.as_path(),
        req.framework,
        webserver.executor().as_ref(),
    )
    .await;
    initial_logs.extend(build_logs);
    ensure_wwwroot_ownership(
        safe_working.as_path(),
        config,
        webserver.executor().as_ref(),
    )
    .await;

    let effective_doc_root =
        resolve_effective_doc_root(safe_working.as_path(), &req.run_sub_dir, req.framework);
    if trimmed_repo.is_empty() && (is_static_runtime(req.framework) || is_php_runtime(req.framework)) {
        let runtime_label = serde_json::to_value(req.framework)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "Pure-Rust Edge".to_string());
        crate::modules::default_pages::ensure_new_site_welcome_index(
            &effective_doc_root,
            domain.as_str(),
            &runtime_label,
            is_php_runtime(req.framework),
        );
        ensure_wwwroot_ownership(
            safe_working.as_path(),
            config,
            webserver.executor().as_ref(),
        )
        .await;
    }

    let resolved_php_fpm = if is_php_runtime(req.framework) && config.is_production() {
        PhpFpmVersion::resolve_active_or_start(webserver.executor().as_ref()).await?
    } else {
        PhpFpmVersion::V83
    };

    // For reverse-proxy runtimes, start systemd unit BEFORE applying the web server vhost
    // so post-reload HTTP health verification hits a live upstream instead of returning 502.
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
        let reload_out = match reload_res {
            Ok(out) => out,
            Err(err) => {
                let _ = std::fs::remove_file(&unit_path);
                return Err(err);
            }
        };
        if !reload_out.success {
            let _ = std::fs::remove_file(&unit_path);
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
            Ok(out) if out.success => {
                if config.is_production() {
                    if let Err(err) = verify_unit_active_or_journal(executor.as_ref(), &unit_file).await {
                        let _ = executor
                            .run(AllowedBinary::Systemctl, &["disable", "--now", &unit_file], SYSTEMD_TIMEOUT)
                            .await;
                        let _ = std::fs::remove_file(&unit_path);
                        let _ = executor
                            .run(AllowedBinary::Systemctl, &["daemon-reload"], SYSTEMD_TIMEOUT)
                            .await;
                        return Err(err);
                    }
                }
            }
            Ok(out) => {
                let _ = std::fs::remove_file(&unit_path);
                let _ = executor
                    .run(AllowedBinary::Systemctl, &["daemon-reload"], SYSTEMD_TIMEOUT)
                    .await;
                return Err(PanelError::CommandFailed {
                    binary: "systemctl",
                    exit_code: out.exit_code,
                    stderr: out.stderr,
                });
            }
            Err(err) => {
                let _ = std::fs::remove_file(&unit_path);
                return Err(err);
            }
        }
        systemd_unit = unit_file;
    }

    // Build and apply web server site configuration (validates nginx -t / ferrousx-bin --check-config).
    // Never write ssl_certificate directives before the real PEM files exist on disk (prevents `nginx -t` failure).
    let existing_cert_status = webserver.ssl.read_cert_status(&domain).await.ok();
    let cert_ready = existing_cert_status
        .as_ref()
        .map(|s| s.present)
        .unwrap_or_else(|| {
            webserver
                .ssl
                .site_cert_paths(&domain)
                .map(|(cert, key)| cert.is_file() && key.is_file())
                .unwrap_or(false)
        });
    let cert_needs_trusted_upgrade = !cert_ready
        || existing_cert_status
            .as_ref()
            .map(|s| {
                s.provider.as_deref() == Some("Self-Signed")
                    || matches!(s.status, SslCertState::Missing | SslCertState::Expired)
            })
            .unwrap_or(false);
    let mut effective_ssl = req.ssl_enabled && cert_ready;
    let mut spec = build_site_spec_for_app(
        domain.clone(),
        parsed_aliases.clone(),
        &effective_doc_root,
        &config.wwwroot,
        req.framework,
        resolved_php_fpm,
        req.internal_port,
        effective_ssl,
        false,
    )?;
    spec.http3_quic = req.http3_quic;
    spec.websocket_enabled = req.websocket_enabled;
    spec.rate_limit_rps = req.rate_limit_rps;
    let mut rendered = match webserver
        .upsert_and_apply_site(&spec, preferred_engine)
        .await
    {
        Ok(r) => r,
        Err(err) => {
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
            return Err(err);
        }
    };
    if req.ssl_enabled && cert_needs_trusted_upgrade {
        let mut issued_le = false;
        if config.is_production() {
            let _ = webserver.ssl.ensure_acme_webroot_permissions();
            let _ = webserver.reload_or_start_configured_engine().await;
            ensure_ssl_firewall_ports(config, webserver).await;
        }
        if config.is_production()
            || webserver
                .executor()
                .is_binary_available(AllowedBinary::Certbot)
        {
            match webserver
                .ssl
                .issue_or_renew_letsencrypt_with_options(
                    &domain,
                    &parsed_aliases,
                    None,
                    config.is_production(),
                    false,
                    false,
                )
                .await
            {
                Ok(status) if status.present => {
                    spec.ssl_enabled = true;
                    spec.redirect_https = true;
                    if let Ok(ssl_rendered) = webserver
                        .upsert_and_apply_site(&spec, preferred_engine)
                        .await
                    {
                        rendered = ssl_rendered;
                        effective_ssl = true;
                        issued_le = true;
                        initial_logs.push(format!(
                            "[{}] Issued Let's Encrypt certificate and enabled HTTPS for {}",
                            Utc::now().to_rfc3339(),
                            domain.as_str()
                        ));
                    }
                }
                Ok(_) => {
                    initial_logs.push(format!(
                        "[{}] Let's Encrypt pending for {} (falling back to Self-Signed HTTPS so port 443 works immediately)",
                        Utc::now().to_rfc3339(),
                        domain.as_str()
                    ));
                }
                Err(err) => {
                    initial_logs.push(format!(
                        "[{}] Let's Encrypt issuance skipped ({err}); falling back to Self-Signed HTTPS so port 443 works immediately",
                        Utc::now().to_rfc3339()
                    ));
                }
            }
        }
        if !issued_le
            && !effective_ssl
            && config.is_production()
            && webserver
                .executor()
                .is_binary_available(AllowedBinary::OpenSsl)
        {
            if let Ok(self_status) = webserver
                .ssl
                .issue_self_signed(&domain, &parsed_aliases, 3650)
                .await
            {
                if self_status.present {
                    spec.ssl_enabled = true;
                    spec.redirect_https = false;
                    if let Ok(ssl_rendered) = webserver
                        .upsert_and_apply_site(&spec, preferred_engine)
                        .await
                    {
                        rendered = ssl_rendered;
                        effective_ssl = true;
                        initial_logs.push(format!(
                            "[{}] Enabled HTTPS on port 443 with OpenSSL X.509 certificate for {} (re-run Let's Encrypt once DNS points directly to VPS)",
                            Utc::now().to_rfc3339(),
                            domain.as_str()
                        ));
                    }
                }
            }
        }
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
            "",
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
        if config.is_production() && req.db_engine.to_ascii_lowercase().contains("postgre") {
            let _ = crate::modules::databases::ensure_pgadmin_sso_postgres_account(
                webserver.executor().as_ref(),
            )
            .await;
            if !Path::new("/var/lib/pgadmin/venv/bin/gunicorn").is_file() {
                let bg_exec = std::sync::Arc::clone(webserver.executor());
                tokio::spawn(async move {
                    let _ = crate::pma_web::install_or_repair_official_pgadmin(bg_exec.as_ref()).await;
                });
            }
        }
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
        git_repo_url: normalized_repo_url,
        git_branch: normalized_branch_name,
        build_script: String::new(),
        last_deploy_commit: "HEAD".to_string(),
        last_deploy_time: Utc::now().to_rfc3339(),
        url_rewrite_preset: default_rewrite_preset(req.framework),
        url_rewrite_rules: rendered.content,
        php_version: if is_php_runtime(req.framework) {
            Some(php_tag_from_fpm(resolved_php_fpm))
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
        setup_summary: None,
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
        ensure_wwwroot_ownership(
            safe_working.as_path(),
            config,
            webserver.executor().as_ref(),
        )
        .await;

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
                    php_fpm_from_tag(app.php_version),
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
            let restart_out = webserver
                .executor()
                .run(AllowedBinary::Systemctl, &["restart", &unit_file], SYSTEMD_TIMEOUT)
                .await?;
            if !restart_out.success {
                return Err(PanelError::CommandFailed {
                    binary: "systemctl",
                    exit_code: restart_out.exit_code,
                    stderr: restart_out.stderr,
                });
            }
            if config.is_production() {
                verify_unit_active_or_journal(webserver.executor().as_ref(), &unit_file).await?;
            }
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
    if config.is_production() && matches!(action, "start" | "restart") {
        verify_unit_active_or_journal(webserver.executor().as_ref(), &unit_file).await?;
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

    let branch = app.git_branch.trim().to_string();

    if !app.git_repo_url.trim().is_empty() || safe_working.as_path().join(".git").exists() {
        if safe_working.as_path().join(".git").exists() && app.git_repo_url.trim().is_empty() {
            let safe_branch = if branch.is_empty() {
                "main"
            } else {
                validate_no_flag_injection(&branch, "gitBranch")?
            };
            if webserver
                .executor()
                .is_binary_available(AllowedBinary::Git)
            {
                let pull_out = webserver
                    .executor()
                    .run_with_env(
                        AllowedBinary::Git,
                        &["pull", "--ff-only", "--", "origin", safe_branch],
                        &[("GIT_TERMINAL_PROMPT", "0")],
                        Some(safe_working.as_path()),
                        BUILD_TIMEOUT,
                    )
                    .await?;
                if pull_out.success {
                    app.logs.push(format!(
                        "[{}] git pull --ff-only origin {safe_branch} succeeded.",
                        Utc::now().to_rfc3339()
                    ));
                } else {
                    return Err(PanelError::CommandFailed {
                        binary: "git",
                        exit_code: pull_out.exit_code,
                        stderr: pull_out.stderr,
                    });
                }
            }
        } else if config.is_production()
            || webserver
                .executor()
                .is_binary_available(AllowedBinary::Git)
        {
            let (clean_url, clean_branch, git_log) = clone_or_sync_git_repository(
                webserver.executor().as_ref(),
                safe_working.as_path(),
                &app.git_repo_url,
                &branch,
                config.is_production(),
            )
            .await?;
            app.git_repo_url = clean_url;
            if clean_branch != "default" && clean_branch != "HEAD" {
                app.git_branch = clean_branch;
            }
            app.logs.push(git_log);
        }
    }

    let build_logs = run_zero_node_js_css_build(
        safe_working.as_path(),
        app.framework,
        webserver.executor().as_ref(),
    )
    .await;
    app.logs.extend(build_logs);
    ensure_wwwroot_ownership(
        safe_working.as_path(),
        config,
        webserver.executor().as_ref(),
    )
    .await;

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
                php_fpm_from_tag(app.php_version),
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
        let restart_out = webserver
            .executor()
            .run(AllowedBinary::Systemctl, &["restart", &unit_file], SYSTEMD_TIMEOUT)
            .await?;
        if !restart_out.success {
            return Err(PanelError::CommandFailed {
                binary: "systemctl",
                exit_code: restart_out.exit_code,
                stderr: restart_out.stderr,
            });
        }
        if config.is_production() {
            verify_unit_active_or_journal(webserver.executor().as_ref(), &unit_file).await?;
        }
    }

    store.upsert_app(&app)?;
    Ok(enrich_app(app, config, webserver).await)
}

#[allow(clippy::too_many_arguments)]
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
        let cert_ready = webserver
            .ssl
            .site_cert_paths(&domain)
            .map(|(cert, key)| cert.is_file() && key.is_file())
            .unwrap_or(false);
        app.ssl_enabled = ssl && cert_ready;
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
    let next_force = if next_ssl {
        force_https.unwrap_or(app.force_https)
    } else {
        false
    };
    let next_h3 = if next_ssl {
        http3_quic.unwrap_or(false)
    } else {
        false
    };
    let next_ws = websocket_enabled.unwrap_or(app.websocket_enabled);
    let next_rps = rate_limit_rps.unwrap_or(app.rate_limit_rps);

    if next_ssl {
        let existing_status = webserver.ssl.read_cert_status(&domain).await.ok();
        let has_trusted_ca_cert = existing_status.as_ref().is_some_and(|st| {
            st.present
                && st.status != SslCertState::Expired
                && st.provider.as_deref() != Some("Self-Signed")
        });
        let mut cert_ready = existing_status.as_ref().is_some_and(|st| st.present);

        if !has_trusted_ca_cert
            && (config.is_production()
                || webserver
                    .executor()
                    .is_binary_available(AllowedBinary::Certbot))
        {
            apply_app_ssl_vhost(&mut app, cert_ready, false, store, config, webserver).await?;
            if config.is_production() {
                let _ = webserver.ssl.ensure_acme_webroot_permissions();
                let _ = webserver.reload_or_start_configured_engine().await;
                ensure_ssl_firewall_ports(config, webserver).await;
                if let Err(err) = webserver
                    .ssl
                    .verify_acme_http01_reachable(&domain, webserver.probe_addr())
                    .await
                {
                    tracing::warn!(
                        domain = %domain.as_str(),
                        error = %err,
                        "ACME HTTP-01 local loopback preflight warning before Certbot"
                    );
                }
            }
            if let Ok(issued) = webserver
                .ssl
                .issue_or_renew_letsencrypt_with_options(
                    &domain,
                    &aliases,
                    None,
                    config.is_production(),
                    false,
                    false,
                )
                .await
            {
                if issued.present {
                    cert_ready = true;
                }
            }
        }
        if !cert_ready
            && config.is_production()
            && webserver
                .executor()
                .is_binary_available(AllowedBinary::OpenSsl)
        {
            if let Ok(self_issued) = webserver
                .ssl
                .issue_self_signed(&domain, &aliases, 3650)
                .await
            {
                cert_ready = self_issued.present;
            }
        }
        if !cert_ready {
            return Err(PanelError::Validation(format!(
                "Chưa có chứng chỉ SSL cho {} — hãy mở trình quản lý SSL để cấp Let's Encrypt, dán chứng chỉ PEM, hoặc tạo chứng chỉ Self-Signed.",
                domain.as_str()
            )));
        }
    }

    let next_redirects = redirects.or_else(|| app.redirects.clone());
    let next_hotlink = hotlink_protection.or_else(|| app.hotlink_protection.clone());
    let next_dir_auth = directory_auth.or_else(|| app.directory_auth.clone());

    let working = SafePath::under_root(&config.wwwroot, &app.working_dir)?;
    let effective_doc_root =
        resolve_effective_doc_root(working.as_path(), &app.run_sub_dir, app.framework);
    let mut spec = build_site_spec_for_app(
        domain,
        aliases,
        &effective_doc_root,
        &config.wwwroot,
        app.framework,
        php_fpm_from_tag(app.php_version),
        app.internal_port,
        next_ssl,
        next_force,
    )?;
    spec.http3_quic = next_h3;
    spec.websocket_enabled = next_ws;
    spec.rate_limit_rps = next_rps;
    map_site_rules_to_spec(
        &mut spec,
        next_redirects.as_deref(),
        next_hotlink.as_ref(),
        next_dir_auth.as_deref(),
    );

    let rendered = webserver.upsert_and_apply_site(&spec, None).await?;
    if next_ssl && config.is_production() {
        let _ = webserver.reload_or_start_configured_engine().await;
        ensure_ssl_firewall_ports(config, webserver).await;
    }
    app.ssl_enabled = next_ssl;
    app.force_https = next_force;
    app.http3_quic = next_h3;
    app.websocket_enabled = next_ws;
    app.rate_limit_rps = next_rps;
    app.redirects = next_redirects;
    app.hotlink_protection = next_hotlink;
    // Strip plaintext passwords before persisting ManagedApp metadata to SQLite.
    app.directory_auth = next_dir_auth.map(|list| {
        list.into_iter()
            .map(|mut item| {
                item.password = None;
                item
            })
            .collect()
    });
    if let Some(preset) = url_rewrite_preset {
        app.url_rewrite_preset = preset;
    }
    app.url_rewrite_rules = url_rewrite_rules.unwrap_or(rendered.content);
    app.nginx_vhost = rendered.config_path.display().to_string();
    store.upsert_app(&app)?;
    sync_wordpress_siteurl_for_app(&app, &effective_doc_root, next_ssl, store, webserver).await;
    Ok(enrich_app(app, config, webserver).await)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSslOverview {
    pub app_id: String,
    pub domain: String,
    pub aliases: Vec<String>,
    pub ssl_enabled: bool,
    pub force_https: bool,
    pub http3_quic: bool,
    pub web_server_engine: WebServerEngineType,
    pub certbot_installed: bool,
    pub openssl_installed: bool,
    pub acme_webroot: String,
    pub cert: SiteSslStatus,
    pub dns_preflight: Vec<SslDnsPreflight>,
    pub renewal_timer: SslRenewalTimerStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

fn parse_app_aliases(app: &ManagedApp) -> Result<Vec<Domain>, PanelError> {
    let mut out = Vec::with_capacity(app.aliases.len());
    for raw in &app.aliases {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            let parsed = Domain::parse(trimmed)?;
            if parsed.as_str() != app.domain.as_str()
                && !out.iter().any(|d: &Domain| d.as_str() == parsed.as_str())
            {
                out.push(parsed);
            }
        }
    }
    Ok(out)
}

/// If `app` is a WordPress site with a linked MariaDB database, automatically updates `siteurl` and `home`
/// in `wp_options` to match `https://` or `http://` so WordPress does not suffer mixed-content or redirect loops.
async fn sync_wordpress_siteurl_for_app(
    app: &ManagedApp,
    effective_doc_root: &Path,
    ssl_enabled: bool,
    store: &ModuleStore,
    webserver: &WebServerManager,
) {
    if is_php_runtime(app.framework) && effective_doc_root.join("wp-config.php").is_file() {
        if let Some(ref db_name) = app.linked_db_name {
            let db_id = format!("db-mysql-{}", db_name.trim());
            let scheme = if ssl_enabled { "https" } else { "http" };
            let target_url = format!("{scheme}://{}", app.domain);
            let escaped_url = target_url.replace('\\', "\\\\").replace('\'', "\\'");
            let sql = format!(
                "UPDATE `wp_options` SET `option_value` = '{escaped_url}' WHERE `option_name` IN ('siteurl', 'home');"
            );
            let _ = crate::modules::databases::execute_sql_query(
                store,
                webserver.executor().as_ref(),
                &db_id,
                &sql,
            )
            .await;
        }
    }
}

async fn apply_app_ssl_vhost(
    app: &mut ManagedApp,
    ssl_enabled: bool,
    force_https: bool,
    store: &ModuleStore,
    config: &Config,
    webserver: &WebServerManager,
) -> Result<(), PanelError> {
    let domain = Domain::parse(&app.domain)?;
    let aliases = parse_app_aliases(app)?;
    let working = SafePath::under_root(&config.wwwroot, &app.working_dir)?;
    let effective_doc_root =
        resolve_effective_doc_root(working.as_path(), &app.run_sub_dir, app.framework);
    let effective_force = if ssl_enabled { force_https } else { false };
    let effective_h3 = false;

    let mut spec = build_site_spec_for_app(
        domain,
        aliases,
        &effective_doc_root,
        &config.wwwroot,
        app.framework,
        php_fpm_from_tag(app.php_version),
        app.internal_port,
        ssl_enabled,
        effective_force,
    )?;
    spec.http3_quic = effective_h3;
    spec.websocket_enabled = app.websocket_enabled;
    spec.rate_limit_rps = app.rate_limit_rps;
    map_site_rules_to_spec(
        &mut spec,
        app.redirects.as_deref(),
        app.hotlink_protection.as_ref(),
        app.directory_auth.as_deref(),
    );

    let rendered = webserver.upsert_and_apply_site(&spec, None).await?;
    app.ssl_enabled = ssl_enabled;
    app.force_https = effective_force;
    app.http3_quic = effective_h3;
    app.url_rewrite_rules = rendered.content;
    app.nginx_vhost = rendered.config_path.display().to_string();
    store.upsert_app(app)?;
    sync_wordpress_siteurl_for_app(app, &effective_doc_root, ssl_enabled, store, webserver).await;
    Ok(())
}

pub async fn get_app_ssl_overview(
    id: &str,
    store: &ModuleStore,
    webserver: &WebServerManager,
) -> Result<AppSslOverview, PanelError> {
    let app = store
        .get_app(id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{id}' not found")))?;
    let domain = Domain::parse(&app.domain)?;
    let aliases = parse_app_aliases(&app)?;
    let (cert_res, dns_preflight, renewal_timer) = tokio::join!(
        webserver.ssl.read_cert_status(&domain),
        webserver.ssl.check_dns_preflight(&domain, &aliases),
        webserver.ssl.renewal_timer_status(),
    );
    let cert = cert_res?;
    let exec = webserver.executor();
    let engine = match webserver.store.load_config().map(|c| c.frontend_engine) {
        Ok(EngineKind::Ferrousx) => WebServerEngineType::FerrousX,
        _ => WebServerEngineType::Nginx,
    };
    let notice = if cert.present && cert.provider.as_deref() == Some("Self-Signed") {
        Some(
            "Website đang dùng chứng chỉ tự ký (Self-Signed) — Safari/Chrome sẽ cảnh báo nếu truy cập trực tiếp không qua Cloudflare Proxy. Hãy nhấn 'Cấp chứng chỉ Let's Encrypt' bên dưới để đổi sang chứng chỉ CA hợp lệ miễn phí."
                .to_string(),
        )
    } else {
        None
    };

    Ok(AppSslOverview {
        app_id: app.id,
        domain: app.domain,
        aliases: aliases.into_iter().map(|d| d.as_str().to_string()).collect(),
        ssl_enabled: app.ssl_enabled,
        force_https: app.force_https,
        http3_quic: app.http3_quic,
        web_server_engine: engine,
        certbot_installed: exec.is_binary_available(AllowedBinary::Certbot),
        openssl_installed: exec.is_binary_available(AllowedBinary::OpenSsl),
        acme_webroot: webserver
            .ssl
            .paths()
            .acme_webroot
            .as_path()
            .display()
            .to_string(),
        cert,
        dns_preflight,
        renewal_timer,
        notice,
    })
}

fn site_vhost_exists(domain: &Domain, webserver: &WebServerManager) -> bool {
    match webserver.store.load_config().map(|c| c.frontend_engine) {
        Ok(EngineKind::Ferrousx) => webserver
            .ferrousx
            .paths()
            .sites_dir
            .join(format!("{}.toml", domain.as_str()))
            .is_file(),
        _ => webserver
            .nginx
            .paths()
            .sites_enabled
            .join(format!("{}.conf", domain.as_str()))
            .exists(),
    }
}

/// Best-effort opens TCP 80 and 443 in UFW on production VPS hosts if UFW is installed and active.
async fn ensure_ssl_firewall_ports(config: &Config, webserver: &WebServerManager) {
    if !config.is_production() {
        return;
    }
    let exec = webserver.executor();
    if !exec.is_binary_available(AllowedBinary::Ufw) {
        return;
    }
    let active = exec
        .run(AllowedBinary::Ufw, &["status"], Duration::from_secs(5))
        .await
        .map(|o| o.success && o.stdout.contains("Status: active"))
        .unwrap_or(false);
    if !active {
        return;
    }
    let _ = exec
        .run(AllowedBinary::Ufw, &["allow", "80/tcp"], Duration::from_secs(5))
        .await;
    let _ = exec
        .run(AllowedBinary::Ufw, &["allow", "443/tcp"], Duration::from_secs(5))
        .await;
}

#[allow(clippy::too_many_arguments)]
pub async fn install_app_ssl_letsencrypt(
    id: &str,
    email_raw: Option<&str>,
    extra_aliases: &[String],
    force_https: bool,
    auto_install_certbot: bool,
    force_renewal: bool,
    skip_dns_check: bool,
    store: &ModuleStore,
    config: &Config,
    webserver: &WebServerManager,
) -> Result<AppSslOverview, PanelError> {
    let mut app = store
        .get_app(id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{id}' not found")))?;
    let domain = Domain::parse(&app.domain)?;
    let email = match email_raw.map(str::trim).filter(|s| !s.is_empty()) {
        Some(raw) => Some(AcmeEmail::parse(raw)?),
        None => None,
    };

    let mut aliases = parse_app_aliases(&app)?;
    let mut aliases_changed = false;
    for raw in extra_aliases {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed = Domain::parse(trimmed)?;
        if parsed.as_str() != domain.as_str()
            && !aliases.iter().any(|d| d.as_str() == parsed.as_str())
        {
            aliases.push(parsed);
            aliases_changed = true;
        }
    }

    let cert_files_present = webserver.ssl.site_cert_ready(&domain);

    // Ensure the site vhost exists and serves `/.well-known/acme-challenge/` on port 80 (and 443 if a cert already exists) before running Certbot.
    if aliases_changed {
        app.aliases = aliases.iter().map(|d| d.as_str().to_string()).collect();
    }
    let safe_ssl = cert_files_present;
    let safe_force = safe_ssl && app.force_https;
    if aliases_changed
        || !site_vhost_exists(&domain, webserver)
        || (app.ssl_enabled && !cert_files_present)
        || config.is_production()
    {
        apply_app_ssl_vhost(&mut app, safe_ssl, safe_force, store, config, webserver).await?;
    }

    let _ = webserver.ssl.ensure_acme_webroot_permissions();
    if config.is_production() {
        let _ = webserver.reload_or_start_configured_engine().await;
        ensure_ssl_firewall_ports(config, webserver).await;
        if let Err(err) = webserver
            .ssl
            .verify_acme_http01_reachable(&domain, webserver.probe_addr())
            .await
        {
            tracing::warn!(
                domain = %domain.as_str(),
                error = %err,
                "ACME HTTP-01 local loopback preflight warning; proceeding with Certbot webroot challenge"
            );
        }
    }

    let issue_err = match webserver
        .ssl
        .issue_or_renew_letsencrypt_with_options(
            &domain,
            &aliases,
            email.as_ref(),
            auto_install_certbot,
            force_renewal,
            skip_dns_check,
        )
        .await
    {
        Ok(issued) if issued.present => {
            apply_app_ssl_vhost(&mut app, true, force_https, store, config, webserver).await?;
            return finalize_app_ssl_install(id, store, config, webserver).await;
        }
        Ok(issued) => PanelError::BadGateway(format!(
            "Certbot kết thúc nhưng không tìm thấy tệp chứng chỉ tại {}",
            issued.cert_path
        )),
        Err(err) => err,
    };

    if config.is_production()
        && webserver
            .executor()
            .is_binary_available(AllowedBinary::OpenSsl)
    {
        let fallback_ready = if webserver.ssl.site_cert_ready(&domain) {
            true
        } else {
            webserver
                .ssl
                .issue_self_signed(&domain, &aliases, 3650)
                .await
                .map(|s| s.present)
                .unwrap_or(false)
        };
        if fallback_ready {
            apply_app_ssl_vhost(&mut app, true, false, store, config, webserver).await?;
            let mut overview = finalize_app_ssl_install(id, store, config, webserver).await?;
            overview.notice = Some(format!(
                "Đã kích hoạt ngay HTTPS (cổng 443) với chứng chỉ OpenSSL X.509 (tương thích Cloudflare Full SSL & trình duyệt). Lưu ý Let's Encrypt ACME HTTP-01 chưa cấp được do: {issue_err}"
            ));
            return Ok(overview);
        }
    }

    Err(issue_err)
}

async fn finalize_app_ssl_install(
    id: &str,
    store: &ModuleStore,
    config: &Config,
    webserver: &WebServerManager,
) -> Result<AppSslOverview, PanelError> {
    if config.is_production() {
        let _ = webserver.reload_or_start_configured_engine().await;
        ensure_ssl_firewall_ports(config, webserver).await;
    }
    get_app_ssl_overview(id, store, webserver).await
}

pub async fn install_app_ssl_custom(
    id: &str,
    fullchain_pem: &str,
    private_key_pem: &str,
    force_https: bool,
    store: &ModuleStore,
    config: &Config,
    webserver: &WebServerManager,
) -> Result<AppSslOverview, PanelError> {
    let mut app = store
        .get_app(id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{id}' not found")))?;
    let domain = Domain::parse(&app.domain)?;

    let installed = webserver
        .ssl
        .install_custom_pem(&domain, fullchain_pem, private_key_pem)
        .await?;
    if !installed.present {
        return Err(PanelError::Validation(
            "Không thể xác minh chứng chỉ vừa cài đặt trên đĩa.".to_string(),
        ));
    }

    apply_app_ssl_vhost(&mut app, true, force_https, store, config, webserver).await?;
    finalize_app_ssl_install(id, store, config, webserver).await
}

pub async fn install_app_ssl_self_signed(
    id: &str,
    validity_days: u32,
    force_https: bool,
    store: &ModuleStore,
    config: &Config,
    webserver: &WebServerManager,
) -> Result<AppSslOverview, PanelError> {
    let mut app = store
        .get_app(id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{id}' not found")))?;
    let domain = Domain::parse(&app.domain)?;
    let aliases = parse_app_aliases(&app)?;

    let installed = webserver
        .ssl
        .issue_self_signed(&domain, &aliases, validity_days)
        .await?;
    if !installed.present {
        return Err(PanelError::BadGateway(
            "OpenSSL tạo chứng chỉ Self-Signed thất bại.".to_string(),
        ));
    }

    apply_app_ssl_vhost(&mut app, true, force_https, store, config, webserver).await?;
    finalize_app_ssl_install(id, store, config, webserver).await
}

pub async fn update_app_ssl_settings(
    id: &str,
    force_https: Option<bool>,
    enable_auto_renew: Option<bool>,
    store: &ModuleStore,
    config: &Config,
    webserver: &WebServerManager,
) -> Result<AppSslOverview, PanelError> {
    let mut app = store
        .get_app(id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{id}' not found")))?;

    if let Some(force) = force_https {
        if force && !app.ssl_enabled {
            return Err(PanelError::Conflict(
                "Cần bật HTTPS / cài chứng chỉ SSL trước khi bật ép chuyển hướng HTTPS (301)."
                    .to_string(),
            ));
        }
        let cur_ssl = app.ssl_enabled;
        apply_app_ssl_vhost(&mut app, cur_ssl, force, store, config, webserver).await?;
    }

    if enable_auto_renew == Some(true) {
        webserver.ssl.install_renewal_timer().await?;
    }

    get_app_ssl_overview(id, store, webserver).await
}

pub async fn disable_app_ssl(
    id: &str,
    delete_cert_files: bool,
    store: &ModuleStore,
    config: &Config,
    webserver: &WebServerManager,
) -> Result<AppSslOverview, PanelError> {
    let mut app = store
        .get_app(id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{id}' not found")))?;
    let domain = Domain::parse(&app.domain)?;

    apply_app_ssl_vhost(&mut app, false, false, store, config, webserver).await?;
    if delete_cert_files {
        let _ = webserver.ssl.remove_site_cert_files(&domain)?;
    }

    get_app_ssl_overview(id, store, webserver).await
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
    let aliases = parse_app_aliases(&app)?;

    let current_cert = webserver.ssl.read_cert_status(&domain).await?;
    let is_self_signed = current_cert
        .issuer
        .as_deref()
        .is_some_and(|i| i.to_ascii_lowercase().contains("self-signed"))
        || current_cert.provider.as_deref() == Some("Self-Signed");
    let le_res = if !current_cert.present || is_self_signed {
        let cur_ssl = app.ssl_enabled && current_cert.present;
        let cur_force = app.force_https && cur_ssl;
        apply_app_ssl_vhost(&mut app, cur_ssl, cur_force, store, config, webserver).await?;
        webserver
            .ssl
            .issue_or_renew_letsencrypt(&domain, &aliases, None, true, false)
            .await
            .map(|_| ())
    } else {
        webserver.ssl.renew_all().await
    };

    if let Err(err) = le_res {
        if config.is_production()
            && webserver
                .executor()
                .is_binary_available(AllowedBinary::OpenSsl)
        {
            let has_cert = webserver
                .ssl
                .site_cert_paths(&domain)
                .map(|(c, k)| c.is_file() && k.is_file())
                .unwrap_or(false);
            if !has_cert {
                let _ = webserver.ssl.issue_self_signed(&domain, &aliases, 3650).await;
            }
        } else {
            return Err(err);
        }
    }

    let updated_cert = webserver.ssl.read_cert_status(&domain).await?;
    if updated_cert.present && !app.ssl_enabled {
        let cur_force = app.force_https;
        apply_app_ssl_vhost(&mut app, true, cur_force, store, config, webserver).await?;
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

// ============================================================================
// Live Vhost Config Editor (with Atomic Snapshot + Rollback) & Website Log Center
// ============================================================================

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppVhostInfo {
    pub app_id: String,
    pub domain: String,
    pub engine: String,
    pub config_path: String,
    pub exists: bool,
    pub content: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppVhostSaveResult {
    pub app_id: String,
    pub domain: String,
    pub engine: String,
    pub config_path: String,
    pub validation_output: String,
    pub reloaded: bool,
    pub saved_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppLogsResponse {
    pub app_id: String,
    pub domain: String,
    pub kind: String,
    pub source_path: String,
    pub is_shared_log: bool,
    pub size_bytes: u64,
    pub lines: usize,
    pub content: String,
    pub fetched_at: String,
}

fn resolve_app_vhost_path(
    domain: &Domain,
    webserver: &WebServerManager,
) -> Result<(EngineKind, PathBuf), PanelError> {
    let engine = webserver.store.load_config()?.frontend_engine;
    let path = match engine {
        EngineKind::Nginx => webserver
            .nginx
            .paths()
            .sites_available
            .join(format!("{}.conf", domain.as_str())),
        EngineKind::Ferrousx => webserver
            .ferrousx
            .paths()
            .sites_dir
            .join(format!("{}.toml", domain.as_str())),
    };
    Ok((engine, path))
}

pub async fn get_app_vhost(
    id: &str,
    store: &ModuleStore,
    webserver: &WebServerManager,
) -> Result<AppVhostInfo, PanelError> {
    let app = store
        .get_app(id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{id}' not found")))?;
    let domain = Domain::parse(&app.domain)?;
    let (engine, path) = resolve_app_vhost_path(&domain, webserver)?;

    let (exists, content) = if path.is_file() {
        (true, std::fs::read_to_string(&path)?)
    } else {
        (false, app.url_rewrite_rules.clone())
    };

    Ok(AppVhostInfo {
        app_id: app.id,
        domain: app.domain,
        engine: engine.as_str().to_string(),
        config_path: path.display().to_string(),
        exists,
        content,
    })
}

pub async fn save_app_vhost(
    id: &str,
    content: &str,
    store: &ModuleStore,
    webserver: &WebServerManager,
) -> Result<AppVhostSaveResult, PanelError> {
    if content.trim().is_empty() {
        return Err(PanelError::Validation(
            "Nội dung cấu hình Vhost không được để trống.".to_string(),
        ));
    }
    if content.len() > 256 * 1024 {
        return Err(PanelError::Validation(
            "Cấu hình Vhost vượt quá giới hạn tối đa 256 KiB.".to_string(),
        ));
    }
    if content.contains('\0') {
        return Err(PanelError::Forbidden(
            "Cấu hình Vhost chứa ký tự NUL không hợp lệ.".to_string(),
        ));
    }

    let mut app = store
        .get_app(id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{id}' not found")))?;
    let domain = Domain::parse(&app.domain)?;
    let (engine, path) = resolve_app_vhost_path(&domain, webserver)?;

    let snap = FileSnapshot::capture(&path)?;
    atomic_write_file(&path, content.as_bytes(), 0o644)?;

    let validation = match engine {
        EngineKind::Nginx => webserver.nginx.validate().await,
        EngineKind::Ferrousx => webserver.ferrousx.validate().await,
    };

    let report = match validation {
        Ok(rep) if rep.valid => rep,
        Ok(rep) => {
            let _ = snap.restore();
            return Err(PanelError::Validation(format!(
                "Kiểm tra cú pháp {} thất bại — đã tự động khôi phục cấu hình cũ:\n{}",
                engine.as_str(),
                rep.output
            )));
        }
        Err(err) => {
            let _ = snap.restore();
            return Err(err);
        }
    };

    let reload_res = match engine {
        EngineKind::Nginx => webserver.nginx.reload().await,
        EngineKind::Ferrousx => webserver.ferrousx.reload().await,
    };
    if let Err(err) = reload_res {
        let _ = snap.restore();
        return Err(err);
    }

    app.url_rewrite_rules = content.to_string();
    app.nginx_vhost = path.display().to_string();
    store.upsert_app(&app)?;

    Ok(AppVhostSaveResult {
        app_id: app.id,
        domain: app.domain,
        engine: engine.as_str().to_string(),
        config_path: path.display().to_string(),
        validation_output: report.output,
        reloaded: true,
        saved_at: Utc::now().to_rfc3339(),
    })
}

fn parse_nginx_log_directive(vhost: &str, directive: &str) -> Option<PathBuf> {
    for raw_line in vhost.lines() {
        let line = raw_line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix(directive) {
            let token = rest
                .trim()
                .split(|c: char| c.is_ascii_whitespace() || c == ';')
                .next()
                .unwrap_or("");
            if token.starts_with("/var/log/nginx/")
                && !token.contains("..")
                && !token.contains('\0')
                && token != "/var/log/nginx/"
            {
                return Some(PathBuf::from(token));
            }
        }
    }
    None
}

fn resolve_app_nginx_log_path(
    domain: &Domain,
    kind: &str,
    vhost_path: &Path,
) -> (PathBuf, bool) {
    let directive = if kind == "access" {
        "access_log"
    } else {
        "error_log"
    };
    if let Ok(vhost_raw) = std::fs::read_to_string(vhost_path) {
        if let Some(custom_path) = parse_nginx_log_directive(&vhost_raw, directive) {
            let is_shared = custom_path == Path::new("/var/log/nginx/access.log")
                || custom_path == Path::new("/var/log/nginx/error.log");
            return (custom_path, is_shared);
        }
    }
    let per_site = PathBuf::from(format!("/var/log/nginx/{}.{kind}.log", domain.as_str()));
    if per_site.is_file() {
        return (per_site, false);
    }
    (PathBuf::from(format!("/var/log/nginx/{kind}.log")), true)
}

fn read_log_tail_sync(path: &Path, max_lines: usize) -> Result<(u64, String), PanelError> {
    if !path.is_file() {
        return Ok((
            0,
            format!("(Chưa có dữ liệu nhật ký tại {})", path.display()),
        ));
    }
    let mut f = std::fs::File::open(path)?;
    let size_bytes = f.metadata()?.len();
    const MAX_TAIL_BYTES: u64 = 128 * 1024;
    if size_bytes > MAX_TAIL_BYTES {
        f.seek(SeekFrom::Start(size_bytes - MAX_TAIL_BYTES))?;
    }
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    let text = String::from_utf8_lossy(&buf);
    let lines_vec: Vec<&str> = text.lines().collect();
    let start = lines_vec.len().saturating_sub(max_lines);
    let tail = lines_vec[start..].join("\n");
    let display = if tail.trim().is_empty() {
        "(Tệp nhật ký hiện đang trống)".to_string()
    } else {
        tail
    };
    Ok((size_bytes, display))
}

pub async fn get_app_logs(
    id: &str,
    raw_kind: &str,
    lines: usize,
    store: &ModuleStore,
    webserver: &WebServerManager,
) -> Result<AppLogsResponse, PanelError> {
    let app = store
        .get_app(id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{id}' not found")))?;
    let domain = Domain::parse(&app.domain)?;
    let kind = match raw_kind.trim().to_ascii_lowercase().as_str() {
        "access" => "access",
        "journal" | "systemd" => "journal",
        _ => "error",
    };
    let clamped = lines.clamp(10, 500);

    if kind == "journal" {
        let engine = webserver.store.load_config()?.frontend_engine;
        let unit = if is_php_runtime(app.framework) {
            php_fpm_from_tag(app.php_version).systemd_unit().to_string()
        } else if !app.systemd_unit.is_empty() {
            let parsed = ServiceName::parse(&app.systemd_unit)?;
            parsed.unit_file_name()
        } else {
            match engine {
                EngineKind::Nginx => "nginx.service".to_string(),
                EngineKind::Ferrousx => "ferrousx.service".to_string(),
            }
        };
        let executor = webserver.executor();
        if !executor.is_binary_available(AllowedBinary::Journalctl) {
            return Err(PanelError::BinaryUnavailable("journalctl"));
        }
        let lines_str = clamped.to_string();
        let out = executor
            .run(
                AllowedBinary::Journalctl,
                &["-u", &unit, "-n", &lines_str, "--no-pager", "-o", "short-iso"],
                SYSTEMD_TIMEOUT,
            )
            .await?;
        let content = if !out.stdout.is_empty() {
            out.stdout
        } else if !out.stderr.is_empty() {
            out.stderr
        } else {
            format!("(Chưa có nhật ký systemd journald cho {unit})")
        };
        return Ok(AppLogsResponse {
            app_id: app.id,
            domain: app.domain,
            kind: "journal".to_string(),
            source_path: format!("journalctl -u {unit}"),
            is_shared_log: is_php_runtime(app.framework) || app.systemd_unit.is_empty(),
            size_bytes: u64::try_from(content.len()).unwrap_or(0),
            lines: clamped,
            content,
            fetched_at: Utc::now().to_rfc3339(),
        });
    }

    let vhost_path = webserver
        .nginx
        .paths()
        .sites_available
        .join(format!("{}.conf", domain.as_str()));
    let (log_path, is_shared_log) = resolve_app_nginx_log_path(&domain, kind, &vhost_path);
    let log_path_owned = log_path.clone();
    let (size_bytes, content) =
        tokio::task::spawn_blocking(move || read_log_tail_sync(&log_path_owned, clamped))
            .await
            .map_err(|_| PanelError::Internal("log reader worker cancelled".to_string()))??;

    Ok(AppLogsResponse {
        app_id: app.id,
        domain: app.domain,
        kind: kind.to_string(),
        source_path: log_path.display().to_string(),
        is_shared_log,
        size_bytes,
        lines: clamped,
        content,
        fetched_at: Utc::now().to_rfc3339(),
    })
}

pub async fn clear_app_logs(
    id: &str,
    raw_kind: &str,
    store: &ModuleStore,
    webserver: &WebServerManager,
) -> Result<AppLogsResponse, PanelError> {
    let app = store
        .get_app(id)?
        .ok_or_else(|| PanelError::NotFound(format!("application '{id}' not found")))?;
    let domain = Domain::parse(&app.domain)?;
    let kind = match raw_kind.trim().to_ascii_lowercase().as_str() {
        "access" => "access",
        "error" => "error",
        _ => {
            return Err(PanelError::Validation(
                "Chỉ hỗ trợ làm trống nhật ký 'access' hoặc 'error' (không xóa journald).".to_string(),
            ))
        }
    };

    let vhost_path = webserver
        .nginx
        .paths()
        .sites_available
        .join(format!("{}.conf", domain.as_str()));
    let (log_path, is_shared_log) = resolve_app_nginx_log_path(&domain, kind, &vhost_path);

    if !log_path.starts_with("/var/log/nginx") {
        return Err(PanelError::Forbidden(
            "Chỉ cho phép làm trống tệp nhật ký nằm bên trong /var/log/nginx/.".to_string(),
        ));
    }
    if let Ok(meta) = std::fs::symlink_metadata(&log_path) {
        if meta.file_type().is_symlink() || !meta.is_file() {
            return Err(PanelError::Forbidden(
                "Từ chối thao tác trên symlink hoặc thư mục không hợp lệ.".to_string(),
            ));
        }
        std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&log_path)?;
    }

    Ok(AppLogsResponse {
        app_id: app.id,
        domain: app.domain,
        kind: kind.to_string(),
        source_path: log_path.display().to_string(),
        is_shared_log,
        size_bytes: 0,
        lines: 0,
        content: "(Đã làm trống tệp nhật ký)".to_string(),
        fetched_at: Utc::now().to_rfc3339(),
    })
}

fn has_any_runnable_source(working_dir: &Path, framework: WebsiteRuntimeType) -> bool {
    if is_rust_runtime(framework) {
        return working_dir.join("Cargo.toml").is_file()
            || detect_rust_release_binary(working_dir).is_some();
    }
    for marker in [
        "package.json",
        "deno.json",
        "deno.jsonc",
        "pyproject.toml",
        "requirements.txt",
        "go.mod",
        ".next/standalone/server.js",
        ".output/server/index.mjs",
        "dist/server/entry.mjs",
        "dist/main.js",
        "build/index.js",
        "build/server/index.js",
    ] {
        if working_dir.join(marker).is_file() {
            return true;
        }
    }
    let mut stack = vec![(working_dir.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > 3 {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if matches!(name.as_str(), ".git" | "node_modules" | "vendor" | "target") {
                continue;
            }
            let Ok(meta) = std::fs::symlink_metadata(&p) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                stack.push((p, depth + 1));
            } else if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                if matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "js" | "mjs" | "cjs" | "ts" | "tsx" | "py" | "go" | "rs" | "html" | "php"
                ) {
                    return true;
                }
            }
        }
    }
    false
}

/// Synchronizes live web runtime state immediately after File Manager mutates or deletes files:
/// 1. Removes stale auto-generated build directories (`dist/`, `build/`, `out/`, `.next/`, `.output/`, `target/`)
///    when the user deletes core project source files so old compiled bundles stop serving.
/// 2. Re-evaluates `effective_doc_root` and updates + reloads the FerrousX / Nginx VirtualHost if the served root changed.
/// 3. Stops `ferrous-app-<domain>.service` immediately when all runnable source/binaries are deleted from RAM-backed apps
///    (or restarts the unit when source files are updated/partially modified).
/// 4. Reloads active `php*-fpm.service` on PHP websites so Zend OPcache drops deleted/modified `.php` scripts immediately.
pub async fn sync_app_runtime_after_file_change(
    app_id: &str,
    affected_rels: &[String],
    is_destructive: bool,
    store: &ModuleStore,
    config: &Config,
    webserver: &WebServerManager,
) {
    let Ok(Some(mut app)) = store.get_app(app_id) else {
        return;
    };
    let Ok(working) = SafePath::under_root(&config.wwwroot, &app.working_dir) else {
        return;
    };
    let _ = std::fs::create_dir_all(working.as_path());
    if let Ok(mut guard) = dir_size_cache().lock() {
        guard.remove(working.as_path());
    }

    if is_destructive && app.run_sub_dir.trim().is_empty() {
        let touched_core_source = affected_rels.iter().any(|r| {
            let clean = r.trim_matches('/');
            matches!(
                clean,
                "*"
                    | "src"
                    | "app"
                    | "pages"
                    | "index.html"
                    | "index.htm"
                    | "index.php"
                    | "package.json"
                    | "Cargo.toml"
                    | "deno.json"
                    | "deno.jsonc"
                    | "main.py"
                    | "app.py"
                    | "go.mod"
            )
        });
        let no_root_source_left = !working.as_path().join("src").exists()
            && !working.as_path().join("app").exists()
            && !working.as_path().join("pages").exists()
            && !working.as_path().join("package.json").exists()
            && !working.as_path().join("index.html").exists()
            && !working.as_path().join("index.php").exists()
            && !working.as_path().join("Cargo.toml").exists();

        if touched_core_source && no_root_source_left {
            for build_dir in ["dist", "build", "out", ".next", ".output", "target"] {
                let candidate = working.as_path().join(build_dir);
                if std::fs::symlink_metadata(&candidate).is_ok() {
                    let _ = crate::modules::files::force_remove_entry(&candidate);
                }
            }
        }
    }

    let effective_doc_root =
        resolve_effective_doc_root(working.as_path(), &app.run_sub_dir, app.framework);
    let _ = std::fs::create_dir_all(&effective_doc_root);

    if let Ok(domain) = Domain::parse(&app.domain) {
        let aliases: Vec<Domain> = app
            .aliases
            .iter()
            .filter_map(|a| Domain::parse(a).ok())
            .collect();
        if let Ok(mut spec) = build_site_spec_for_app(
            domain,
            aliases,
            &effective_doc_root,
            &config.wwwroot,
            app.framework,
            php_fpm_from_tag(app.php_version),
            app.internal_port,
            app.ssl_enabled,
            app.force_https,
        ) {
            spec.http3_quic = app.http3_quic;
            spec.websocket_enabled = app.websocket_enabled;
            spec.rate_limit_rps = app.rate_limit_rps;
            map_site_rules_to_spec(
                &mut spec,
                app.redirects.as_deref(),
                app.hotlink_protection.as_ref(),
                app.directory_auth.as_deref(),
            );
            let preferred_engine = app.web_server_engine.map(|e| match e {
                WebServerEngineType::FerrousX => EngineKind::Ferrousx,
                WebServerEngineType::Nginx => EngineKind::Nginx,
            });
            let preview_content = match preferred_engine.unwrap_or(EngineKind::Nginx) {
                EngineKind::Ferrousx => webserver.ferrousx.render_site(&spec).ok().map(|r| r.content),
                EngineKind::Nginx => webserver.nginx.render_site(&spec).ok().map(|r| r.content),
            };
            let vhost_changed = preview_content
                .as_deref()
                .is_some_and(|c| c != app.url_rewrite_rules);
            if vhost_changed {
                if let Ok(rendered) = webserver
                    .upsert_and_apply_site(&spec, preferred_engine)
                    .await
                {
                    app.url_rewrite_rules = rendered.content;
                    app.nginx_vhost = rendered.config_path.display().to_string();
                    let _ = store.upsert_app(&app);
                }
            }
        }
    }

    let executor = webserver.executor();
    if !is_static_runtime(app.framework) && !is_php_runtime(app.framework) && !app.systemd_unit.is_empty()
    {
        if !has_any_runnable_source(working.as_path(), app.framework) {
            if executor.is_binary_available(AllowedBinary::Systemctl) {
                let _ = executor
                    .run(
                        AllowedBinary::Systemctl,
                        &["stop", &app.systemd_unit],
                        SYSTEMD_TIMEOUT,
                    )
                    .await;
            }
            app.status = AppStatus::Stopped;
            app.pid = None;
            let _ = store.upsert_app(&app);
        } else if config.is_production() && executor.is_binary_available(AllowedBinary::Systemctl) {
            let _ = executor
                .run(
                    AllowedBinary::Systemctl,
                    &["restart", &app.systemd_unit],
                    SYSTEMD_TIMEOUT,
                )
                .await;
        }
    } else if is_php_runtime(app.framework)
        && config.is_production()
        && executor.is_binary_available(AllowedBinary::Systemctl)
    {
        let fpm_unit = php_fpm_from_tag(app.php_version).systemd_unit();
        let _ = executor
            .run(AllowedBinary::Systemctl, &["reload", fpm_unit], SYSTEMD_TIMEOUT)
            .await;
    }
}

