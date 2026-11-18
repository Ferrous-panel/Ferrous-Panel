# FerrousPanel

[![License: MIT](https://img.shields.io/badge/License-MIT-orange.svg)](LICENSE)
[![Rust: 1.85+](https://img.shields.io/badge/Rust-1.85%2B-dea584.svg)](Cargo.toml)
[![OS: Ubuntu 22.04 / 24.04 LTS](https://img.shields.io/badge/Ubuntu-22.04%20%7C%2024.04%20LTS-E95420.svg)](install.sh)

Open-source, 100% pure-Rust Ubuntu 22.04 / 24.04 LTS VPS Control Panel (`ferrous-panel` + optional `ferrousx-bin`).

- **100% Pure-Rust Backend on VPS**: Frontend React assets (`dist/`) are prebuilt (`vite build`) and embedded directly inside the `ferrous-panel` binary via `rust-embed` (zero Node.js runtime on the VPS).
- **Automated 1-Command Installer (`install.sh`)**: Automatically creates a 2GB `/swapfile` on low-RAM VPS (`< 4GB RAM`) to prevent OOM, installs `rustup` / `cargo` `1.85.0` (`--profile minimal`), embeds the prebuilt `dist/` UI, compiles `ferrous-panel` with `cargo build --release`, and configures `systemd` + `UFW`.
- **Zero fabricated metrics & zero hidden shells**: System metrics come from `/proc` + `sysinfo`; all privileged host operations run via allowlisted argv execution (`execve`, no `sh -c`) and strict validated newtypes (`Domain`, `ServiceName`, `Port`, `SafePath`, `SqlIdentifier`, `PortOrRange`, `CronSchedule`, `SafeLinuxUser`, `OctalMode`).
- **Fail-closed HTTP 501**: Unimplemented subsystems return `HTTP 501 Not Implemented` and display `"Chưa hỗ trợ"` in the UI rather than returning mock data.

---

## 1-Command Installation (Ubuntu 22.04 / 24.04 LTS)

Run **one single command** on a fresh Ubuntu 22.04 / 24.04 VPS. The installer automatically provisions a 2GB swap file (if RAM < 4GB), installs `cargo` 1.85.0 via `rustup`, embeds the prebuilt `dist/` frontend without needing Node.js on the VPS, compiles the release binary, and starts `ferrous-panel.service`:

### Option A — Default 1-Command Install (with Nginx Engine)
```bash
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash
```

### Option B — Pure-Rust Stack (with FerrousX Engine)
```bash
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash -s -- --engine ferrousx
```

### Custom Options / Fast Prebuilt Mode / Update / Uninstall
```bash
# Custom port & security entrance path:
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash -s -- --port 9443 --entrance /admin

# Optional: Skip Cargo compilation and download prebuilt binary directly:
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash -s -- --prebuilt-binary

# 1-Command In-Place Update (pulls latest code & rebuilds via Cargo, preserving /var/lib/ferrous-panel):
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/update.sh | sudo bash

# 1-Command Uninstall (preserves /www/wwwroot & databases):
curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/uninstall.sh | sudo bash
```

### What `install.sh` Does Automatically
1. **OOM Memory Guard**: Checks `/proc/meminfo`; if RAM `< 4GB` and no swap is active, creates and enables a `2GB /swapfile` (`chmod 0600`) so `cargo build --release` (with LTO) never runs out of memory on 1GB–2GB VPS instances.
2. **Auto-Install Rust & Cargo 1.85.0 + Environment Variables**: Installs `build-essential`, `pkg-config`, `libssl-dev`, runs `rustup-init -y --profile minimal --default-toolchain 1.85.0`, links `cargo` and `rustc` into `/usr/local/bin`, and automatically registers `/etc/profile.d/ferrous-panel.sh` + `/etc/ferrous-panel/panel.env` (`FERROUS_*`, `CARGO_HOME`, `RUSTUP_HOME`, `PATH`) across login and interactive shells (`/root/.bashrc`, `/root/.profile`, `/etc/bash.bashrc`).
3. **Prebuilt Static UI (`dist/`) + Cargo Compilation**: Uses the prebuilt `dist/` directory (or downloads `ferrous-dist.tar.gz` from GitHub Releases) so **Node.js is never needed on the VPS**, then compiles `ferrous-panel` (and `ferrousx-bin` when `--engine ferrousx` is selected) with `cargo build --release`.
4. **First-Run Argon2id Credentials & Systemd Hardening**: Generates a 24-character one-time admin password from kernel `OsRng`, stores only its Argon2id hash (`0600`), configures `ferrous-panel.service` (`NoNewPrivileges=true`, `PrivateTmp=true`), verifies `GET /api/health == 200`, and enables `UFW`.

- **Default loopback bind (`127.0.0.1:8443`)**: Access securely via SSH port forwarding:
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

8. **cPanel Suite: Crontab, Zstandard Backups, Hickory DNS Zone Editor & SFTP Chroot (`/api/cpanel*`)**
   - Reads/writes user crontabs (`crontab -u`), validates schedule/user/allowlisted binary, and executes jobs directly without a shell.
   - Creates `tar --zstd` archives (plus optional `mysqldump`/`pg_dump`), records real file byte size and streaming SHA-256 checksums, and verifies constant-time SHA-256 + listing safety before restore.
   - Manages DNS records (`A`, `AAAA`, `CNAME`, `MX`, `TXT`, `CAA`) persisted in SQLite and exported to RFC 1035 Hickory DNS zone files (`<data_dir>/dns/zones/<domain>.zone`), plus chrooted SFTP accounts under `/www/wwwroot` with real disk quota usage calculation.

9. **Security Center: UFW, Fail2ban, Kernel Sysctl, eBPF/XDP Anti-DDoS, Layer-7 WAF & SHA-256 FIM (`/api/security*`)**
   - Manages UFW firewall rules (including per-rule enable/disable toggle), Fail2ban jail bans/unbans, and kernel sysctl hardening directives.
   - Controls eBPF/XDP protection modes (`normal`, `elevated`, `under_attack`), Layer-7 WAF rules (`FX-RCE-01`, `FX-SQLI-02`, `FX-TRAV-03`, `FX-SSRF-04`, `FX-XSS-05`, `FX-BOT-06`), and real SHA-256 File Integrity Monitoring scans across `/www/wwwroot` and `/var/lib/ferrous-panel`.

10. **Multi-PHP FPM Pools, 1-Click Templates & Software Package Installer (`/api/php*`, `/api/oneclick*`, `/api/software*`)**
    - Manages PHP 8.4 / 8.3 / 8.2 / 8.1 / 7.4 FPM pools (`php.ini`, `pool.d/www.conf`, OPcache JIT, extension toggles, and per-site PHP socket switching with automatic Nginx/FerrousX vhost regeneration).
    - Deploys 1-Click application templates (Actix-Web, Axum, WordPress, Laravel, Next.js, FastAPI) and manages server packages (`FerrousX`, `Nginx`, `PostgreSQL 17`, `MariaDB 11.4`, `Redis 7.4`, `Postfix/Dovecot`, `Certbot`, `Fail2ban`).

11. **Mail Server: Postfix / Dovecot / Rspamd (`/api/mail*`)**
    - Manages mail domains with Ed25519 DKIM selectors, SPF/DMARC records, virtual Maildir accounts (`<data_dir>/mail/vhosts/<domain>/<user>/Maildir/{cur,new,tmp}`), virtual forwarder maps, and outbound message spooling.

12. **System Optimizer, Anti-Crash Guard, SSH Manager & Panel Admin (`/api/optimizer*`, `/api/ssh*`, `/api/panel-admin*`)**
    - Applies kernel sysctl tuning profiles (`99-ferrous-optimizer.conf`), memory reclamation, watchdog self-healing, and cgroups v2 / ZRAM Anti-Crash Guard policies.
    - Hardens OpenSSH/russh daemon settings, manages Ed25519 authorized keys & keypair generation, executes safe allowlisted Web SSH commands, and exposes `fp` CLI options (`1..20`) via the Web UI.

13. **Rust Edge Cloud, Polyglot FFI/IPC Bridge Compiler & Diagnostics (`/api/edge-cloud*`, `/api/polyglot*`, `/api/rust-source`, `/api/system/diagnostics`)**
    - Manages Wasmtime WASI Preview 2 edge workers (with sub-millisecond fuel invocation telemetry), Youki/Firecracker OCI sandboxes, BoringTun WireGuard mesh peers, and BLAKE3 Merkle-verified Zstd snapshots.
    - Compiles Polyglot FFI/IPC bridge artifacts (`PyO3`, `N-API`, `C-ABI`, `WASI`, `JNI`) with generated C headers and systemd env bindings.

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

## Local Verification & Smoke Testing

```bash
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --workspace

# Run Ubuntu 24.04 end-to-end verification suite:
sudo bash scripts/smoke-test.sh
```

---

## Required Libraries & Dependencies

### 1. Rust Workspace Crates (`Cargo.toml` — `[workspace.dependencies]`)
All Rust dependencies are centrally version-pinned in the root `Cargo.toml` and inherited via `.workspace = true` by `crates/ferrous-panel` and `crates/ferrousx`:

| Library (`Crate`) | Version | Target Crate | Purpose |
| :--- | :--- | :--- | :--- |
| `actix-web` | `4.9` | `ferrous-panel` | Control-plane HTTP/REST server, middleware & routing |
| `tokio` | `1.43` | `ferrous-panel`, `ferrousx` | Multi-threaded async I/O, process, signal & timer runtime |
| `rusqlite` (`bundled`) | `0.32` | `ferrous-panel` | Embedded SQLite 3 with WAL mode & parameterized `params![]` queries |
| `argon2` / `sha2` / `subtle` / `rand` | `0.5` / `0.10` / `2.6` / `0.8` | `ferrous-panel` | Argon2id password hashing, SHA-256 tokens/FIM & constant-time checks |
| `sysinfo` | `0.33` | `ferrous-panel` | Real Linux `/proc` CPU, RAM, swap, disk & network telemetry |
| `rust-embed` / `mime_guess` | `8.5` / `2.0` | `ferrous-panel`, `ferrousx` | Embeds prebuilt `dist/` SPA into the Rust binary |
| `hyper` / `hyper-util` / `http-body-util` | `1.6` / `0.1` / `0.1` | `ferrousx` | Standalone HTTP/1.1 + HTTP/2 server & reverse proxy (`ferrousx-bin`) |
| `rustls` / `tokio-rustls` / `rustls-pemfile` | `0.23` / `0.26` / `2.2` | `ferrousx` | Memory-safe TLS 1.2/1.3 (`ring`) with SNI & ALPN `h2`/`http1.1` |
| `fastcgi-client` | `0.9` | `ferrousx` | Native PHP-FPM FastCGI gateway over Unix domain sockets |
| `serde` / `serde_json` / `toml` | `1.0` / `1.0` / `0.8` | `ferrous-panel`, `ferrousx` | Strict `camelCase` JSON API contract & TOML vhost parser |
| `clap` / `chrono` | `4.5` / `0.4` | `ferrous-panel`, `ferrousx` | Built-in `fp` & `ferrousx-bin` CLI parser + UTC timestamps |
| `thiserror` / `anyhow` / `tracing` / `tracing-subscriber` | `2.0` / `1.0` / `0.1` / `0.3` | `ferrous-panel`, `ferrousx` | Typed error mapping (`PanelError`) & structured logging |

### 2. Frontend UI Libraries (`package.json` — Build-Time Only, Zero Node.js on VPS)
- **Runtime UI (`dependencies`)**: `react` (`^19.0.1`), `react-dom` (`^19.0.1`), `react-is` (`^19.3.1`), `lucide-react` (`^0.546.0`), `recharts` (`^3.10.1`), `motion` (`^12.23.24`).
- **Build & Type-Checking (`devDependencies`)**: `vite` (`^8.3.0`), `@vitejs/plugin-react` (`^6.1.1`), `tailwindcss` (`^4.3.3`), `@tailwindcss/vite` (`^4.3.3`), `autoprefixer` (`^10.4.21`), `typescript` (`^7.0.2`), `@types/react` (`^19.3.0`), `@types/react-dom` (`^19.3.0`).

### 3. Ubuntu 22.04 / 24.04 LTS System Packages (`install.sh`)
Automatically installed via `apt-get` during 1-command installation:
- **Compiler & Build**: `build-essential`, `pkg-config`, `libssl-dev`, `ca-certificates`, `curl`, `git` + `rustup` (`1.85.0` minimal profile).
- **Archive & Crypto**: `tar`, `zstd`, `zip`, `unzip`, `openssl`.
- **Network & Security**: `ufw`, `fail2ban`, `certbot`, `nginx` (when `--engine nginx`).

---

## Repository Structure

```text
├── Cargo.toml                         # Rust 1.85 Workspace (ferrous-panel + ferrousx; excludes _archive/)
├── PROGRESS.md                        # Phase tracker (P1–P7), assumptions & root-cause log
├── crates/
│   ├── ferrous-panel/                 # Control plane daemon, API, auth, modules, fp CLI & embedded dist/
│   └── ferrousx/                      # Standalone HTTP/HTTPS/FastCGI/WS Rust web server (ferrousx-bin)
├── dist/                              # Prebuilt static React UI embedded into ferrous-panel via rust-embed
├── scripts/
│   └── smoke-test.sh                  # Ubuntu 24.04 end-to-end verification script (P7)
├── src/                               # React 19 + TypeScript + Tailwind frontend UI source
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
