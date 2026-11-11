//! Cargo Build Script (`rust-backend/build.rs`)
//! Enforces strict compile-time system binary paths and ABI invariants for
//! UFW Firewall, Nginx / FerrousX Reverse Proxy, and Systemd Service Manager.

use std::env;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/main.rs");
    println!("cargo:rerun-if-changed=src/ufw_engine.rs");
    println!("cargo:rerun-if-changed=src/nginx_engine.rs");
    println!("cargo:rerun-if-changed=src/systemd_engine.rs");
    println!("cargo:rerun-if-changed=src/wasm_exports.rs");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| "linux".to_string());
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "x86_64".to_string());

    println!("cargo:rustc-env=FERROUS_TARGET_OS={}", target_os);
    println!("cargo:rustc-env=FERROUS_TARGET_ARCH={}", target_arch);
    println!("cargo:rustc-env=FERROUS_ABI_SIGNATURE=FERROUS-CORE-ABI-2.5.0-STRICT");

    // Verify presence of system-sensitive binaries on Linux hosts at build time
    let system_binaries = [
        ("UFW_BIN", "/usr/sbin/ufw"),
        ("NGINX_BIN", "/usr/sbin/nginx"),
        ("FERROUSX_BIN", "/usr/local/bin/ferrousx"),
        ("SYSTEMCTL_BIN", "/bin/systemctl"),
        ("SYSCTL_BIN", "/sbin/sysctl"),
    ];

    for (env_key, bin_path) in system_binaries {
        let detected = if Path::new(bin_path).exists() {
            "DETECTED_ON_HOST"
        } else {
            "DEFERRED_RUNTIME_PATH"
        };
        println!("cargo:rustc-env=FERROUS_{}_PATH={}", env_key, bin_path);
        println!("cargo:rustc-env=FERROUS_{}_STATE={}", env_key, detected);
    }
}
