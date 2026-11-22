# FerrousPanel

[![License: MIT](https://img.shields.io/badge/License-MIT-orange.svg)](LICENSE)
[![Rust: 1.89+](https://img.shields.io/badge/Rust-1.89%2B-dea584.svg)](Cargo.toml)
[![OS: Ubuntu 22.04 / 24.04 LTS](https://img.shields.io/badge/Ubuntu-22.04%20%7C%2024.04%20LTS-E95420.svg)](install.sh)

Open-source, 100% pure-Rust Ubuntu 22.04 / 24.04 LTS VPS Control Panel (`ferrous-panel` + optional `ferrousx-bin`).

- **100% Pure-Rust Production Binary (Zero Node.js on VPS)**: The native web UI (`crates/ferrous-panel/src/ui/assets/`) is embedded directly into the `ferrous-panel` binary via `rust-embed` with zero Node.js runtime or build dependency on the server.
- **Secure-by-Default Installer (`install.sh`)**: Defaults to loopback bind (`FERROUS_BIND=127.0.0.1`), enforces SHA-256 checksum verification (`SHA256SUMS.txt`, failing closed unless `--insecure-skip-verify` is explicitly passed), warns loudly when binding publicly without HTTPS, auto-provisions 2GB swap on low-RAM VPS (`< 4GB`), and uses Rust `stable (>= 1.89)`.
- **Zero Fabricated Data & Live Capabilities (`GET /api/capabilities`)**: System metrics are read from `/proc` + `sysinfo`; all privileged host operations run via allowlisted argv execution (`execve`, no `sh -c`) with bounded 64 KiB stdout/stderr capture, verification readbacks, and automatic rollback on failure. Missing host packages report `unavailable` with install hints (and return `HTTP 409 Conflict` on execution) instead of simulating success.

---

## 1-Command Installation (Ubuntu 22.04 / 24.04 LTS)

### Option A — Default Secure Loopback Install (Nginx Engine, `127.0.0.1:8443`)
```bash
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash
```

Access securely over an SSH tunnel:
```bash
ssh -N -L 8443:127.0.0.1:8443 root@<VPS_IP>
```

### Option B — Pure-Rust Web Stack (FerrousX Engine)
```bash
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash -s -- --engine ferrousx
```

### Custom Options, Public Bind (with HTTPS Warning), Update & Uninstall
```bash
# Explicitly bind publicly (prints unencrypted HTTP & cookie warnings until HTTPS is configured):
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash -s -- --bind 0.0.0.0 --port 8443 --entrance /admin

# Fast prebuilt binary mode (requires verified SHA256SUMS.txt):
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash -s -- --prebuilt-binary

# 1-Command In-Place Update:
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/update.sh | sudo bash

# Sync / Update FerrousX (`ferrousx-bin` + `ferrousx-ctl` + `ferrousx.env`):
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/update.sh | sudo bash -s -- --ferrousx

# 1-Command Uninstall (preserves /www/wwwroot & databases):
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/uninstall.sh | sudo bash
```

---

## Core Features (Production Ready on Ubuntu 22.04 / 24.04)

1. **Authentication, Audit Log & Feature Registry (`GET /api/capabilities`)**
   - 32-byte `OsRng` session tokens hashed (SHA-256) in SQLite, verified in constant time (`subtle`), Argon2id admin password (`0600`), login rate-limiting (`5` failures / `15m` / IP -> `HTTP 429`), `HttpOnly; SameSite=Strict` cookies (`Secure` auto-enabled with domain + TLS).
   - `GET /api/capabilities` probes real host binaries, systemd units, and permissions, returning `ready | degraded | unavailable | experimental` with actionable `fixHint` commands.

2. **Websites & Dual Web Server Engine (`nginx` ⇄ `ferrousx`)**
   - Supports `static_spa`, `reverse_proxy` (including WebSockets), and `php_fpm`.
   - Per-site settings enforced on **both** engines: `rateLimitRps` (Nginx `limit_req_zone`/`limit_req`; FerrousX per-IP token bucket with real `429` & `rate_limited` metrics), `redirects`, `hotlinkProtection`, `directoryAuth` (shared Apache/OpenSSL `$apr1$` htpasswd files), and `websocketEnabled`.
   - Unsupported flags such as `http3Quic: true` on stock Ubuntu 24.04 Nginx 1.24 or FerrousX return `HTTP 409 Conflict` with an explicit explanation rather than silently ignoring the option.
   - Zero-port-overlap engine switching with HTTP probe verification and automatic rollback on failure.
   - Upstream-compatible FerrousX UDS Admin IPC (`/run/ferrousx/panel-admin.sock`) supporting `status`, `reload`, `panel_sync`, `vhost_add`, `vhost_del` with `SO_PEERCRED` + 256-bit token authentication and Zero-Downtime RCU reload.

3. **SSL Certificates (Let's Encrypt / Certbot)**
   - Pre-validates domain DNS resolution before invoking `certbot certonly --webroot -w /www/acme`, reads live X.509 expiry from `fullchain.pem`, manages systemd auto-renewal timers with `--deploy-hook`, and surfaces verbatim `certbot` errors.

4. **Applications & Systemd Services (`/api/apps*`)**
   - Git deployment with strict flag-injection validation (`--`), real `.service` units, background rebuild jobs, `systemctl is-active` readback verification, and `journalctl` failure diagnostics.

5. **SafePath File Manager (`/api/files*`)**
   - Strictly confined inside `/www/wwwroot` via `SafePath` canonicalization with symlink escape blocking.
   - Hardened `remote-download` SSRF defense: `--max-redirs 0`, `--proto-redir =http,https`, DNS pre-resolution + `--resolve` IP pinning, blocking RFC1918, loopback, CGNAT `100.64.0.0/10`, link-local/metadata `169.254.169.254`, IPv4-mapped IPv6 `::ffff:a.b.c.d`, and decimal/hex/octal obfuscated hosts.
   - Archive extraction validates both TAR (`tar -tf` + `tar -tvf`, space-safe filenames) and ZIP (`unzip -Z` + staging symlink check) against zip-slip and symlink attacks.

6. **Databases (`MariaDB`, `PostgreSQL`, embedded `SQLite 3 WAL`)**
   - Provisions databases/users, schema inspection, SQL execution, and dumps via `mysql`/`psql` with credentials piped over `stdin` (never exposed on argv or logs) and pure-Rust SQLite management.

7. **Firewall (`UFW`), Fail2ban & SSH Hardening (`/api/security*`, `/api/ssh*`)**
   - Real `ufw status numbered` readback and anti-lockout guard protecting active SSH and Panel ports unless confirmed (`?force=true`).
   - Fail2ban ban/unban via `fail2ban-client`.
   - OpenSSH configuration editing verified with `sshd -t` + automatic rollback, explicit `confirmPortChange` confirmation on port changes, real `ssh-keygen`, and strict read-only WebSSH allowlist.

8. **Multi-PHP FPM, Mail Server (Postfix/Dovecot), Optimizer, Cron & Zstd Backups**
   - **Multi-PHP (`8.4/8.3/8.2/8.1/7.4`)**: `php-fpm -t` syntax checks + rollback on `php.ini`/extension edits, per-site socket switching, `systemctl is-active` readback.
   - **Mail Server**: Real OpenSSL RSA-2048 DKIM key generation (`0600`), real `dig +short TXT` verification for SPF/DKIM/DMARC, atomic Postfix virtual maps + `postmap` + reload rollback.
   - **System Optimizer**: Allowlisted `sysctl` directives written to `/etc/sysctl.d/99-ferrous-optimizer.conf`, verified via `sysctl -p` + `/proc/sys` readback with rollback on error.
   - **Cron & Backups**: Real `crontab -u` management, `tar --zstd` archives with SHA-256 verification before restore.

---

## Experimental / Optional Subsystems

The following subsystems are explicitly marked **`experimental`** in `GET /api/capabilities` and the native UI. They require optional external runtimes/kernels and return `HTTP 409 Conflict` with install instructions when prerequisites are absent (never simulated):

- **Edge Cloud (`edge_cloud_wasi`, `edge_cloud_oci`, `edge_cloud_wireguard`)**: Requires `wasmtime` (WASI workers), `podman` or `docker` (OCI containers), and `wg` (`wireguard-tools` for WireGuard peers).
- **Polyglot FFI Bridge Compiler (`polyglot_ffi`)**: Requires `rustc` / `cargo` installed on the VPS to compile real cdylib/FFI crates.
- **eBPF / XDP Anti-DDoS (`ebpf_xdp`)**: Currently limited to `sysctl` SYN-flood/network hardening and Layer-7 configuration; native kernel XDP bytecode attachment is experimental and not enabled by default.

---

## Built-in Terminal CLI (`fp`)

`fp` and `ferrous` are symlinks to `/usr/local/bin/ferrous-panel`:

```bash
fp              # Interactive management menu
fp status       # Show bind addr, mode, web engine & credential file path
fp restart      # Restart ferrous-panel.service
fp stop / start # Stop or start ferrous-panel.service
fp passwd       # Update Admin password (Argon2id) or pass --generate
fp port <PORT>  # Update FERROUS_PORT in panel.env & open UFW port
fp repair       # Validate active web config & reload
fp check        # Check systemd units & validate engine config
fp logs         # Show journalctl logs for ferrous-panel.service
fp webserver    # View or switch active engine (`fp webserver nginx|ferrousx`)
```

---

## Verification & Smoke Testing (Ubuntu 24.04)

```bash
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --workspace

# Run full Ubuntu 24.04 end-to-end smoke & security verification:
sudo bash scripts/smoke-test.sh
```

---

## Repository Structure

```text
├── Cargo.toml                         # Rust 1.89+ Workspace (ferrous-panel + ferrousx)
├── PROGRESS.md                        # Remediation & verification status log
├── crates/
│   ├── ferrous-panel/                 # Control plane daemon, API, auth, modules, fp CLI & Native UI
│   │   ├── src/ui/assets/             # Pure-Rust embedded Native UI (HTML/CSS/ES6, zero Node.js)
│   │   └── tests/                     # Auth, contract, webserver, modules & security_and_real_ops tests
│   └── ferrousx/                      # Standalone HTTP/HTTPS/FastCGI/WS Rust web server + UDS IPC
│       └── tests/                     # Server & ipc_and_site_features E2E tests
├── scripts/
│   ├── panel.env.example              # Hardened default environment config (127.0.0.1, production mode)
│   └── smoke-test.sh                  # Ubuntu 24.04 14-point smoke & security verification suite
├── install.sh / update.sh / uninstall.sh
└── .github/workflows/ci-release.yml   # Workspace clippy/test, Ubuntu 24.04 smoke test & tag-only releases
```

## License

[MIT License](LICENSE)
