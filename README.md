# FerrousPanel

[![License: MIT](https://img.shields.io/badge/License-MIT-orange.svg)](LICENSE)
[![Rust: 1.85+](https://img.shields.io/badge/Rust-1.85%2B-dea584.svg)](Cargo.toml)
[![OS: Ubuntu 22.04 / 24.04 LTS](https://img.shields.io/badge/Ubuntu-22.04%20%7C%2024.04%20LTS-E95420.svg)](install.sh)

Open-source, 100% pure-Rust Ubuntu 22.04 / 24.04 LTS VPS Control Panel (`ferrous-panel` + optional `ferrousx-bin`).

- **Zero JS server runtime on VPS**: Frontend React assets (`dist/`) are compiled at CI/build time (`vite build`) and embedded directly inside the `ferrous-panel` binary via `rust-embed`.
- **Zero fabricated metrics & zero hidden shells**: System metrics come from `/proc` + `sysinfo`; all privileged host operations run via allowlisted argv execution (`execve`, no `sh -c`) and strict validated newtypes (`Domain`, `ServiceName`, `Port`, `SafePath`, `SqlIdentifier`, `PortOrRange`, `CronSchedule`, `SafeLinuxUser`, `OctalMode`).
- **Fail-closed HTTP 501**: Unimplemented subsystems return `HTTP 501 Not Implemented` and display `"Chưa hỗ trợ"` in the UI rather than returning mock data.

---

## 1-Command Installation (Ubuntu 22.04 / 24.04 LTS)

Install prebuilt release binaries (`x86_64` / `aarch64`, SHA-256 verified) with **zero Node.js, Bun, or Cargo installed on the VPS**:

### Option A — Default Install (with Nginx Engine)
```bash
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash
```

### Option B — Pure-Rust Stack (with FerrousX Engine)
```bash
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash -s -- --engine ferrousx
```

### Custom Port / Entrance / Update / Uninstall (1 Command)
```bash
# Custom port & entrance path:
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash -s -- --port 9443 --entrance /admin

# 1-Command In-Place Update (preserves /var/lib/ferrous-panel & sites):
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/update.sh | sudo bash

# 1-Command Uninstall (preserves /www/wwwroot & databases):
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/uninstall.sh | sudo bash
```

- **First-run credentials**: Generated once via kernel CSPRNG (`OsRng`), printed once to the terminal during install, and stored solely as an Argon2id hash (`0600`). Passwords are **never** accepted via command-line arguments.
- **Default loopback bind (`127.0.0.1:8443`)**: Access via SSH tunnel:
  ```bash
  ssh -N -L 8443:127.0.0.1:8443 root@<VPS_IP>
  ```

---

## Implemented Features (Real System Operations)

1. **Authentication & Security Middleware (`crates/ferrous-panel/src/auth/`)**
   - Token session 32-byte CSPRNG (`OsRng`) hashed and persisted in SQLite (`24h`, or `30d` with `rememberMe`), verified in constant time (`subtle`).
   - Argon2id password hashing stored in mode `0600` (`admin.credentials.json`); first run generates a random 24-character password once and prints it to stdout (no default password).
   - `HttpOnly; SameSite=Strict` cookies, login rate-limiting (`5` failures / `15` minutes / IP -> HTTP `429`), 1 MiB JSON cap, and SQLite audit log.

2. **Host Telemetry (`GET /api/telemetry`)**
   - Real CPU/per-core usage, memory, swap, disk, network throughput (`sysinfo`), `/proc/loadavg`, and real 60-second historical samples persisted in SQLite `telemetry_history`.

3. **Selectable Web Server Engine (`nginx` or `ferrousx`) + Certbot SSL**
   - Switchable global engine (`GET/PUT /api/webserver`, `POST /api/webserver/switch`) with zero port overlap: pre-validates all sites (`nginx -t` or `ferrousx-bin --check-config`), stops old engine, starts new engine on `:80`/`:443`, probes each site over TCP HTTP, and rolls back automatically on failure.
   - `crates/ferrousx` (`ferrousx-bin`): standalone pure-Rust HTTP/HTTPS server (`tokio`, `hyper 1`, `rustls` SNI + ALPN `h2`/`http1.1`) supporting `static_spa`, `reverse_proxy` (including WebSockets), `php_fpm` (Unix socket FastCGI), `redirect_https`, SIGHUP hot-reload, and `--check-config`.
   - SSL via `certbot certonly --webroot -w /www/acme` with `/.well-known/acme-challenge/` served by both engines, X.509 expiry parsed from `/etc/letsencrypt/live/<domain>/`, and systemd auto-renewal timer.

4. **Websites & Systemd Services (`/api/apps*`)**
   - Writes `.service` unit files, runs `systemctl daemon-reload / enable --now / start / stop / restart`, queries real `MainPID` & `MemoryCurrent` from `systemctl show`, measures real web-root disk usage, updates `MemoryMax` & `Environment=`, and performs real TCP HTTP `GET /` probes (`/test-http`).

5. **Firewall, Fail2ban & Kernel Hardening (`/api/security*`)**
   - Reads and manages `ufw status numbered` (`allow`/`deny`/`limit` by validated `PortOrRange` and source CIDR) and `ufw --force delete <id>`.
   - Queries `fail2ban-client status` and bans/unbans IPs per jail.
   - Reads and updates allowlisted `sysctl` kernel parameters persistently.

6. **Databases (`mysql` / `psql` CLI, `/api/databases*`)**
   - Provisions databases, roles/users, password updates, table schema inspection, and read/write SQL execution via `mysql` / `psql` CLI with strict `SqlIdentifier` & literal escaping.
   - Real sizes read from `information_schema.TABLES` / `pg_database_size`; returns `HTTP 409 Conflict` when engine CLI is absent. Plaintext database passwords are never stored or logged.

7. **SafePath File Manager (`/api/files*`)**
   - Lists, creates, edits, renames, chmods, compresses (`zip`), downloads (`curl` with SSRF private-IP blocking), and extracts (`unzip`/`tar`) files strictly confined inside `/www/wwwroot` with symlink escape prevention and pre-flight anti-zip-slip checks.

8. **Crontab & Zstandard Backups (`/api/cpanel*`)**
   - Reads/writes user crontabs (`crontab -u`), validates schedule/user/allowlisted binary, and executes jobs directly without a shell.
   - Creates `tar --zstd` archives (plus optional `mysqldump`/`pg_dump`), records real file byte size and streaming SHA-256 checksums, and verifies constant-time SHA-256 + listing safety before restore.

---

## Features Marked `HTTP 501` (`Chưa hỗ trợ`)

These UI sections remain visible for API contract compatibility and return `HTTP 501 Not Implemented`:
- Mail Server (`Postfix` / `Dovecot` / `Rspamd`)
- DNS Zone Editor (`Hickory DNS`) & SFTP Chroot (`russh`)
- Rust Edge Cloud (`Wasmtime`, `Youki OCI`, `WireGuard`, `BLAKE3` vaults)
- eBPF/XDP Anti-DDoS, Layer-7 WAF rules, SHA-256 FIM, per-rule UFW toggle
- Polyglot FFI/IPC bridge compiler
- Multi-PHP pool switching, Software Package installer, 1-Click templates, System Optimizer profiles, SSH Manager, Diagnostics, Git Auto-Deploy, Auto-Rebuild, and per-site Nginx/FerrousX `redirects` / `hotlinkProtection` / `directoryAuth` / `http3Quic` / `websocketEnabled` / `rateLimitRps`.

---

## Built-in Terminal CLI (`fp`)

`fp` and `ferrous` are symlinks to `/usr/local/bin/ferrous-panel` implemented directly with `clap`:

```bash
fp              # Interactive management menu
fp status       # (alias: 14, default, info) Show bind addr, web engine & credential file path
fp restart      # (alias: 1) Restart ferrous-panel.service
fp stop         # (alias: 2) Stop ferrous-panel.service
fp start        # (alias: 3) Start ferrous-panel.service
fp reload       # (alias: 4) Reload active web engine & panel service
fp passwd       # (alias: 5) Interactively update Admin password (Argon2id) or pass --generate
fp username     # (alias: 6) Change Admin username
fp port <PORT>  # (alias: 7) Update FERROUS_PORT in panel.env & open UFW port
fp entrance <P> # (alias: 8) Update FERROUS_SECURITY_ENTRANCE
fp repair       # (alias: 11) Validate active web config & reload
fp check        # (alias: 15) Check systemd units & validate engine config
fp logs         # (alias: 16) Show journalctl logs for ferrous-panel.service
fp webserver    # (alias: 19) View or switch active engine (`fp webserver nginx|ferrousx`)
```

---

## Building From Source & Smoke Testing

```bash
npm ci --legacy-peer-deps
npm run build
npx tsc --noEmit
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --workspace

# Run Ubuntu 24.04 end-to-end verification suite:
sudo bash scripts/smoke-test.sh
```

---

## Repository Structure

```text
├── Cargo.toml                         # Rust 1.85 Workspace (ferrous-panel + ferrousx; excludes _archive/)
├── PROGRESS.md                        # Phase tracker (P1–P7), assumptions & root-cause log
├── crates/
│   ├── ferrous-panel/                 # Control plane daemon, API, auth, modules, fp CLI & embedded dist/
│   └── ferrousx/                      # Standalone HTTP/HTTPS/FastCGI/WS Rust web server (ferrousx-bin)
├── scripts/
│   └── smoke-test.sh                  # Ubuntu 24.04 end-to-end verification script (P7)
├── src/                               # React 19 + TypeScript + Tailwind frontend UI
├── package.json                       # Frontend Vite scripts ("dev", "build")
├── install.sh / update.sh / uninstall.sh
├── .github/                           # CI/Release workflow & open-source Issue/PR templates
└── _archive/                          # Archived legacy prototypes (excluded from Cargo workspace)
```

## Open Source Governance & License

- **License**: [MIT License](LICENSE)
- **Contributing**: [CONTRIBUTING.md](CONTRIBUTING.md)
- **Code of Conduct**: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)
- **Security Policy**: [SECURITY.md](SECURITY.md)
