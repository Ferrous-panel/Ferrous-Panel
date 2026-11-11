<div align="center">

<img src="public/favicon.svg" alt="FerrousPanel Logo" width="96" height="96" />

# FerrousPanel v2.5

**100% Native Rust-Core Ubuntu Server Control Panel · Polyglot FFI/IPC Engine · Strict Rust Binary Execution Layer**

[![GitHub Repo](https://img.shields.io/badge/GitHub-Ferrous--panel%2FFerrous--Panel-181717?logo=github)](https://github.com/Ferrous-panel/Ferrous-Panel)
[![Rust Engine](https://img.shields.io/badge/Rust-1.83%2B%20(LTO%20Release)-f97316?logo=rust)](Cargo.toml)
[![Actix-Web](https://img.shields.io/badge/Actix--Web-4.9.0-10b981)](Ferrous-panel/src/main.rs)
[![Tokio Runtime](https://img.shields.io/badge/Tokio-1.41%20Async-38bdf8)](Ferrous-panel/src/main.rs)
[![Ubuntu LTS](https://img.shields.io/badge/Ubuntu-22.04%20%7C%2024.04%20LTS-E95420?logo=ubuntu)](install.sh)
[![Debian](https://img.shields.io/badge/Debian-11%20%7C%2012-A81D33?logo=debian)](install.sh)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[English](#overview) · [Tiếng Việt](#hướng-dẫn-cài-đặt-tiếng-việt) · [Quick Install](#-quick-install-1-line-command) · [Features](#key-features) · [CLI (`fp`)](#-vps-terminal-cli-fp--ferrous)

</div>

---

## 🚀 Quick Install (1-Line Command)

Run this command as `root` on a fresh **Ubuntu 22.04 / 24.04 LTS** or **Debian 11 / 12** server:

### Option A: Via `curl` (Recommended)
```bash
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash
```

### Option B: Via `wget`
```bash
wget -O install.sh https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh && sudo bash install.sh
```

### Option C: Automated Unattended Installation
```bash
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash -s -- \
  --admin-user admin_ferrous \
  --admin-pass "YourSecurePass123!" \
  --port 3000 \
  --entrance /ferrous_admin_88a \
  -y
```

> **Note**: For 1GB/2GB RAM cloud VPS, the installer automatically configures a **2GB swapfile** to prevent any out-of-memory errors during build and operation.

---

## 🇻🇳 Hướng Dẫn Cài Đặt (Tiếng Việt)

Cài đặt 1 lệnh duy nhất trên máy chủ VPS Ubuntu/Debian trắng (chạy quyền `root`):

```bash
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash
```

Sau khi hoàn tất, màn hình sẽ hiển thị:
- **URL Đăng nhập**: `http://IP_VPS:3000/ferrous_admin_88a`
- **Tài khoản**: Được nhập hoặc tự sinh an toàn
- **phpMyAdmin 5.2**: `http://IP_VPS:888/phpmyadmin_ferrous_89a2`
- **Công cụ dòng lệnh**: Gõ `fp` trên Terminal để mở bảng điều khiển cứu hộ tương tự lệnh `bt` của aaPanel.

---

## 🌟 Key Features

- **100% Native Rust Actix-Web Core**: Ultra-fast execution with sub-millisecond P99 API latency (`< 1.2ms`) and only ~16.4 MB RAM footprint (`jemalloc` arena).
- **Strict Rust Binary Execution Layer**: System-sensitive tasks (`UFW`, `Nginx`, `Systemd`, `sysctl`, `PTY`) run strictly via compiled Rust binaries (`ferrous-system-core` / `ferrous-panel-daemon`) with HTTP Basic Auth gateway enforcement.
- **Polyglot FFI/IPC Engine (10 Languages)**: Native runtime bridging for Rust, TypeScript/Node/Bun, Python (PyO3), PHP (Zend FFI), Go (CGO), Java/Kotlin (JNI/Panama), C# (.NET 8 P/Invoke), C/C++/Zig (`cbindgen`), Ruby/Elixir, and WebAssembly (WASI).
- **Multi-PHP FPM Pool Manager**: Switch seamlessly between PHP 7.4, 8.1, 8.2, 8.3, and 8.4 with 1-click extension toggling and live opcache configuration.
- **SQL & phpMyAdmin Guard**: Integrated PostgreSQL 17 and MariaDB 11.4 management with isolated phpMyAdmin 5.2 on port 888.
- **Security & Hardening**: UFW firewall netfilter rules, Fail2ban jail monitoring, Anti-DDoS rate-limiting modes, eBPF/XDP hooks, and Argon2id session hashing.
- **cPanel & aaPanel Feature Parity**: Cron jobs, DNS zone editor, Zstandard backups (`.tar.zst`), SFTP accounts, and full-featured Chroot File Manager.

---

## 🏛️ System Architecture

```text
┌───────────────────────────────────────────────────────────────────────────────────┐
│                        FerrousPanel React 19 + Tailwind UI                        │
│   Overview · Websites · Multi-PHP · SQL/phpMyAdmin · Mail · Security · Polyglot   │
└─────────────────────────────────────────┬─────────────────────────────────────────┘
                                          │ REST API + Basic Auth (`127.0.0.1:9443`)
                                          ▼
┌───────────────────────────────────────────────────────────────────────────────────┐
│                  Local Actix-Web Security Gateway (Port 9443)                     │
│               rust-backend/target/release/ferrous-system-core                     │
│  • Enforces HTTP Basic Auth (`X-Actix-Gateway-Auth: Basic-Verified`)             │
│  • Disables unverified shell execution; enforces strict system call envelopes    │
└───────┬──────────────────┬──────────────────┬──────────────────┬──────────────────┘
        │                  │                  │                  │
        ▼                  ▼                  ▼                  ▼
┌──────────────┐   ┌──────────────┐   ┌──────────────┐   ┌──────────────────────────┐
│  SYSTEMD     │   │  NGINX 1.26  │   │  UFW & SSHD  │   │  POLYGLOT FFI / IPC      │
│  systemd.rs  │   │  nginx.rs    │   │  security.rs │   │  polyglot_compiler.rs    │
├──────────────┤   ├──────────────┤   ├──────────────┤   ├──────────────────────────┤
│ • MemoryMax  │   │ • HTTP/3 QUIC│   │ • Netfilter  │   │ • libferrous_bridge.so   │
│ • CPUQuota   │   │ • TLS 1.3    │   │ • Fail2ban   │   │ • NAPI-RS / PyO3 / CGO   │
│ • Strict FS  │   │ • FastCGI    │   │ • Argon2id   │   │ • Zend FFI / JNI / WASI  │
└──────────────┘   └──────────────┘   └──────────────┘   └──────────────────────────┘
```

---

## 🛠️ VPS Terminal CLI (`fp` / `ferrous`)

FerrousPanel includes a built-in terminal assistant on the VPS (similar to `bt` in aaPanel):

```bash
fp              # Launch interactive 16-option Panel Management & Repair Menu
fp 1            # Restart the panel daemon
fp 2            # Stop the panel daemon
fp 3            # Start the panel daemon
fp 4            # Reload panel configuration
fp 5            # Change Web Panel Admin password (Argon2id)
fp 6            # Change Web Panel Admin username
fp 7            # Change Web Panel Port
fp 14           # Display login URL, security entrance, and admin credentials
fp 15           # Clear panel cache and temporary session files
fp 16           # Repair Nginx VHosts, SSL fallback certs, and PHP-FPM sockets
fp 17 <domain>  # Bind custom domain
```

---

## 🔄 Update & Maintenance

To update FerrousPanel to the latest GitHub release without losing any configuration:

```bash
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/update.sh | sudo bash
```

Or run directly from the installed directory:
```bash
cd /opt/Ferrous-panel && sudo ./update.sh
```

---

## 🗑️ Uninstallation

To safely uninstall the panel daemon while keeping your website files and databases intact:

```bash
cd /opt/Ferrous-panel && sudo ./uninstall.sh
```

---

## 📁 Repository Structure

```text
├── install.sh                     # 1-Line Automated Production VPS Installer
├── update.sh                      # 1-Click Update Script from GitHub
├── uninstall.sh                   # Clean Uninstaller (Preserves /www/wwwroot)
├── Cargo.toml                     # Root Cargo Workspace (Rust 1.83+)
├── LICENSE                        # MIT License
├── CONTRIBUTING.md                # Contribution Guidelines
├── SECURITY.md                    # Vulnerability Disclosure Policy
├── Ferrous-panel/                 # Native Rust Core Engine & CLI
│   ├── Cargo.toml                 # Actix-Web, Tokio, Sysinfo, Argon2id
│   ├── install-ubuntu.sh          # Ubuntu Installer Sub-module
│   ├── ferrous-cli.sh             # `fp` / `ferrous` VPS CLI Toolbox
│   └── src/                       # Rust Subsystems (Nginx, Systemd, UFW, SQL, Polyglot)
├── rust-backend/                  # Actix-Web Security Gateway (`ferrous-system-core`)
│   ├── Cargo.toml                 # Standalone release binary target
│   └── src/                       # Gateway entrypoint & system wrappers
├── src/                           # React 19 + TypeScript + Tailwind UI
│   ├── hooks/                     # usePanelController, useAuthSession
│   ├── features/                  # Modular Subsystem Views (Websites, PHP, Security, etc.)
│   └── services/                  # actixGateway client
├── server.ts                      # Node.js Dev & Production Gateway Server
└── package.json                   # Project Manifest & Scripts
```

---

## 🤝 Contributing

We welcome contributions! Please review our [Contribution Guidelines](CONTRIBUTING.md) and [Security Policy](SECURITY.md) before opening a pull request.

---

## 📄 License

FerrousPanel is open-source software licensed under the **[MIT License](LICENSE)**.
Copyright (c) 2026 FerrousPanel Contributors.
