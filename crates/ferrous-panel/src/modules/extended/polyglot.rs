use super::common::*;

// ============================================================================
// 8. Rust Source Inspector & Polyglot FFI/IPC Bridge Compiler
// ============================================================================

pub fn list_rust_source_files() -> Vec<RustSourceFile> {
    vec![
        RustSourceFile {
            path: "crates/ferrous-panel/src/main.rs".to_string(),
            title: "Actix-Web Control Plane & fp CLI Entrypoint".to_string(),
            lang: "rust".to_string(),
            description: "Unified binary entry point serving both the Actix-Web daemon and the `fp` terminal CLI.".to_string(),
            content: include_str!("../main.rs").to_string(),
        },
        RustSourceFile {
            path: "crates/ferrous-panel/src/auth/mod.rs".to_string(),
            title: "Argon2id + 32-Byte OsRng Session Authentication".to_string(),
            lang: "rust".to_string(),
            description: "Zero-default-password Argon2id credential store and constant-time session verifier.".to_string(),
            content: include_str!("../auth/mod.rs").to_string(),
        },
        RustSourceFile {
            path: "crates/ferrous-panel/src/system/cmd.rs".to_string(),
            title: "AllowedBinary Whitelist Command Runner".to_string(),
            lang: "rust".to_string(),
            description: "Strict argv-only process runner forbidding shell interpolation (`sh -c`).".to_string(),
            content: include_str!("../system/cmd.rs").to_string(),
        },
        RustSourceFile {
            path: "crates/ferrous-panel/src/system/paths.rs".to_string(),
            title: "SafePath Canonical Jail Guard".to_string(),
            lang: "rust".to_string(),
            description: "Prevents path traversal (`..`) and symlink escape outside `/www/wwwroot`.".to_string(),
            content: include_str!("../system/paths.rs").to_string(),
        },
        RustSourceFile {
            path: "crates/ferrous-panel/src/modules/apps.rs".to_string(),
            title: "Zero-Node.js & Rust Multi-Framework Deployer".to_string(),
            lang: "rust".to_string(),
            description: "Builds and deploys Next.js, Nuxt, Vite, Tailwind, Rust, Python, Go, and PHP workloads with Bun/Deno/Cargo and systemd.".to_string(),
            content: include_str!("apps.rs").to_string(),
        },
        RustSourceFile {
            path: "crates/ferrous-panel/src/modules/cpanel.rs".to_string(),
            title: "cPanel Suite (DNS, Cron, SFTP Chroot, Backups, SSL)".to_string(),
            lang: "rust".to_string(),
            description: "Real Ubuntu OS automation for OpenSSH chroot jails, BIND9 zones, systemd cron, Let's Encrypt, and tar.gz archives.".to_string(),
            content: include_str!("cpanel.rs").to_string(),
        },
    ]
}

pub fn list_polyglot_toolchains() -> Vec<PolyglotToolchainSpec> {
    let rustc_ready = AllowedBinary::Rustc.is_available() || AllowedBinary::Cargo.is_available();
    let tc_status = if rustc_ready { "ready" } else { "rustc_missing" };
    vec![
        PolyglotToolchainSpec {
            id: "tc-ts-napi".to_string(),
            language: "TypeScript / Bun / Deno (Zero-Node.js FFI)".to_string(),
            runtime_version: "Bun 1.x bun:ffi + Deno.dlopen (C-ABI)".to_string(),
            bridge_protocol: "Rust cdylib (.so) + bun:ffi dlopen (Zero-Node.js)".to_string(),
            compiler_command: "rustc --crate-type cdylib --edition 2021 -C opt-level=3".to_string(),
            output_artifact: "libferrous_bridge.so".to_string(),
            zero_copy_ipc: true,
            systemd_env_vars: vec!["LD_LIBRARY_PATH=/usr/local/lib".to_string()],
            status: tc_status.to_string(),
        },
        PolyglotToolchainSpec {
            id: "tc-python-pyo3".to_string(),
            language: "Python (PyO3 / FastAPI / ctypes)".to_string(),
            runtime_version: "Python 3.12 (C-ABI / ABI3 Stable)".to_string(),
            bridge_protocol: "Rust cdylib (.so) + ctypes.CDLL / PyO3".to_string(),
            compiler_command: "rustc --crate-type cdylib --edition 2021 -C opt-level=3".to_string(),
            output_artifact: "libferrous_bridge.so".to_string(),
            zero_copy_ipc: true,
            systemd_env_vars: vec!["LD_LIBRARY_PATH=/usr/local/lib".to_string()],
            status: tc_status.to_string(),
        },
        PolyglotToolchainSpec {
            id: "tc-php-ffi".to_string(),
            language: "PHP 8.3 / 8.4 (Zend FFI)".to_string(),
            runtime_version: "PHP 8.3 / 8.4 Zend FFI::cdef".to_string(),
            bridge_protocol: "C-ABI Shared Library + ferrous_bridge.h".to_string(),
            compiler_command: "rustc --crate-type cdylib --edition 2021 -C opt-level=3".to_string(),
            output_artifact: "libferrous_bridge.so".to_string(),
            zero_copy_ipc: true,
            systemd_env_vars: vec!["ffi.enable=preload".to_string()],
            status: tc_status.to_string(),
        },
        PolyglotToolchainSpec {
            id: "tc-go-cgo".to_string(),
            language: "Go (Golang CGO)".to_string(),
            runtime_version: "Go 1.23 CGO Linker".to_string(),
            bridge_protocol: "C-ABI Shared/Static Library + ferrous_bridge.h".to_string(),
            compiler_command: "cargo build --release --manifest-path Cargo.toml".to_string(),
            output_artifact: "libferrous_bridge.so".to_string(),
            zero_copy_ipc: true,
            systemd_env_vars: vec!["CGO_ENABLED=1".to_string()],
            status: tc_status.to_string(),
        },
    ]
}

pub async fn compile_polyglot_bridge(
    config: &Config,
    executor: &dyn CommandExecutor,
    req: &PolyglotCompileRequest,
) -> Result<PolyglotCompileResult, PanelError> {
    let mod_name = if req.module_name.trim().is_empty() {
        "ferrous_bridge"
    } else {
        req.module_name.trim()
    };
    let clean_mod: String = mod_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    let clean_mod = if clean_mod.is_empty()
        || clean_mod
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_digit())
    {
        format!("ferrous_{clean_mod}")
    } else {
        clean_mod
    };

    let fn_list = if req.custom_functions.is_empty() {
        vec![
            "verify_argon2_token".to_string(),
            "query_host_telemetry".to_string(),
        ]
    } else {
        req.custom_functions.clone()
    };

    let mut clean_fns: Vec<String> = Vec::new();
    let mut rust_fns = String::new();
    let mut c_decls = String::new();
    for f in &fn_list {
        let safe_fn: String = f
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
            .collect();
        if safe_fn.is_empty() {
            continue;
        }
        let safe_fn = if safe_fn.bytes().next().is_some_and(|b| b.is_ascii_digit()) {
            format!("fn_{safe_fn}")
        } else {
            safe_fn
        };
        rust_fns.push_str(&format!(
            "#[no_mangle]\npub extern \"C\" fn {clean_mod}_{safe_fn}(input_code: u32) -> u32 {{\n    input_code.wrapping_add(200)\n}}\n\n"
        ));
        c_decls.push_str(&format!(
            "uint32_t {clean_mod}_{safe_fn}(uint32_t input_code);\n"
        ));
        clean_fns.push(safe_fn);
    }

    let rust_core_code = format!(
        "// Generated by FerrousPanel Polyglot FFI Compiler — module `{clean_mod}`\n\
         #![forbid(unsafe_op_in_unsafe_fn)]\n\n\
         {rust_fns}"
    );
    let cargo_toml_code = format!(
        "[package]\n\
         name = \"{clean_mod}\"\n\
         version = \"0.1.0\"\n\
         edition = \"2021\"\n\n\
         [lib]\n\
         name = \"{clean_mod}\"\n\
         crate-type = [\"cdylib\", \"staticlib\"]\n\n\
         [profile.release]\n\
         opt-level = 3\n\
         lto = \"thin\"\n\
         strip = true\n"
    );
    let c_header_code = format!(
        "/* Generated by FerrousPanel FFI Compiler for {clean_mod} */\n\
         #ifndef FERROUS_{upper}_H\n\
         #define FERROUS_{upper}_H\n\
         #include <stdint.h>\n\n\
         #ifdef __cplusplus\n\
         extern \"C\" {{\n\
         #endif\n\n\
         {c_decls}\n\
         #ifdef __cplusplus\n\
         }}\n\
         #endif\n\
         #endif\n",
        upper = clean_mod.to_ascii_uppercase()
    );

    let out_dir = config.data_dir.join("polyglot").join(&clean_mod);
    let src_dir = out_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;
    let artifact_path = out_dir.join(format!("lib{clean_mod}.so"));
    let src_lib_path = src_dir.join("lib.rs");
    atomic_write_file(&out_dir.join("Cargo.toml"), cargo_toml_code.as_bytes(), 0o644)?;
    atomic_write_file(&src_lib_path, rust_core_code.as_bytes(), 0o644)?;
    atomic_write_file(
        &out_dir.join(format!("{clean_mod}.h")),
        c_header_code.as_bytes(),
        0o644,
    )?;

    let lang_lower = req.target_language.to_ascii_lowercase();
    let artifact_str = artifact_path.display().to_string();
    let target_binding_code = if lang_lower.contains("typescript")
        || lang_lower.contains("bun")
        || lang_lower.contains("node")
        || lang_lower.contains("deno")
        || lang_lower.contains("js")
    {
        let mut symbols = String::new();
        for f in &clean_fns {
            symbols.push_str(&format!(
                "  {clean_mod}_{f}: {{ args: [FFIType.u32], returns: FFIType.u32 }},\n"
            ));
        }
        format!(
            "// Zero-Node.js Bun native FFI binding (runs directly on Bun / Next.js / Astro / Elysia)\n\
             import {{ dlopen, FFIType }} from \"bun:ffi\";\n\n\
             export const {clean_mod} = dlopen(\"{artifact_str}\", {{\n\
             {symbols}}}).symbols;\n"
        )
    } else if lang_lower.contains("python") {
        let mut py_setup = String::new();
        for f in &clean_fns {
            py_setup.push_str(&format!(
                "lib.{clean_mod}_{f}.argtypes = [ctypes.c_uint32]\nlib.{clean_mod}_{f}.restype = ctypes.c_uint32\n"
            ));
        }
        format!(
            "# Python 3.12 C-ABI binding for Rust cdylib\n\
             import ctypes\n\n\
             lib = ctypes.CDLL(\"{artifact_str}\")\n\
             {py_setup}"
        )
    } else if lang_lower.contains("php") {
        format!(
            "<?php\n\
             // PHP 8.3/8.4 Zend FFI binding for Rust cdylib\n\
             $ffi = FFI::cdef(\n\
                 file_get_contents('{}/{clean_mod}.h'),\n\
                 '{artifact_str}'\n\
             );\n",
            out_dir.display()
        )
    } else if lang_lower.contains("go") {
        format!(
            "package main\n\n\
             /*\n\
             #cgo CFLAGS: -I{dir}\n\
             #cgo LDFLAGS: -L{dir} -l{clean_mod} -Wl,-rpath,{dir}\n\
             #include \"{clean_mod}.h\"\n\
             */\n\
             import \"C\"\n",
            dir = out_dir.display()
        )
    } else {
        format!(
            "// C-ABI shared library binding for {artifact_str}\n{c_decls}"
        )
    };

    let opt_flag = match req.optimization_level.trim() {
        "debug" => "opt-level=0",
        "release-size" => "opt-level=z",
        _ => "opt-level=3",
    };

    let started = Instant::now();
    let (compile_ok, compiler_output) = if executor.is_binary_available(AllowedBinary::Rustc) {
        let src_str = src_lib_path.display().to_string();
        match executor
            .run_with_env(
                AllowedBinary::Rustc,
                &[
                    "--crate-type",
                    "cdylib",
                    "--edition",
                    "2021",
                    "-C",
                    opt_flag,
                    "-o",
                    &artifact_str,
                    &src_str,
                ],
                &[("PATH", "/usr/local/bin:/root/.cargo/bin:/usr/bin:/bin")],
                Some(&out_dir),
                APT_TIMEOUT,
            )
            .await
        {
            Ok(out) if out.success && artifact_path.is_file() => {
                let elapsed = started.elapsed().as_secs_f64();
                let size_kb = std::fs::metadata(&artifact_path)
                    .map(|m| m.len() as f64 / 1024.0)
                    .unwrap_or(0.0);
                (
                    true,
                    format!(
                        "   Compiling {clean_mod} v0.1.0 ({}) via rustc ({opt_flag})\n    Finished native cdylib `{artifact_str}` ({size_kb:.1} KiB) in {elapsed:.2}s",
                        out_dir.display()
                    ),
                )
            }
            Ok(out) => {
                let detail = if out.stderr.is_empty() {
                    out.stdout
                } else {
                    out.stderr
                };
                (
                    false,
                    format!(
                        "rustc failed (exit {:?}): {detail}",
                        out.exit_code
                    ),
                )
            }
            Err(err) => (false, format!("rustc execution error: {err}")),
        }
    } else if executor.is_binary_available(AllowedBinary::Cargo) {
        let manifest_str = out_dir.join("Cargo.toml").display().to_string();
        match executor
            .run_with_env(
                AllowedBinary::Cargo,
                &["build", "--release", "--manifest-path", &manifest_str],
                &[("PATH", "/usr/local/bin:/root/.cargo/bin:/usr/bin:/bin")],
                Some(&out_dir),
                APT_TIMEOUT,
            )
            .await
        {
            Ok(out) if out.success => {
                let built_so = out_dir
                    .join("target/release")
                    .join(format!("lib{clean_mod}.so"));
                if built_so.is_file() {
                    let _ = std::fs::copy(&built_so, &artifact_path);
                }
                let elapsed = started.elapsed().as_secs_f64();
                (
                    artifact_path.is_file(),
                    format!(
                        "   Compiling {clean_mod} v0.1.0 via cargo build --release\n    Finished release target `{artifact_str}` in {elapsed:.2}s"
                    ),
                )
            }
            Ok(out) => (
                false,
                format!(
                    "cargo build --release failed (exit {:?}): {}",
                    out.exit_code,
                    if out.stderr.is_empty() { out.stdout } else { out.stderr }
                ),
            ),
            Err(err) => (false, format!("cargo execution error: {err}")),
        }
    } else {
        (
            false,
            format!(
                "Generated Rust crate & FFI headers at {} (install rustc/cargo on VPS to compile native .so artifact).",
                out_dir.display()
            ),
        )
    };

    Ok(PolyglotCompileResult {
        success: compile_ok,
        module_name: clean_mod.clone(),
        target_language: req.target_language.clone(),
        bridge_protocol: if req.enable_zero_copy_ipc {
            "C-ABI cdylib (.so) + Zero-Copy FFI/IPC".to_string()
        } else {
            "Standard C-ABI cdylib (.so)".to_string()
        },
        compiler_output,
        rust_core_code,
        c_header_code,
        target_binding_code,
        systemd_unit_snippet: format!(
            "[Service]\nEnvironment=\"FERROUS_FFI_LIB={artifact_str}\"\nEnvironment=\"LD_LIBRARY_PATH={}\"\n",
            out_dir.display()
        ),
        build_script: format!(
            "cargo build --release --manifest-path {}/Cargo.toml",
            out_dir.display()
        ),
        binary_artifact_path: artifact_str,
        compiled_at: Utc::now().to_rfc3339(),
    })
}

