# FerrousPanel — Trạng thái tiến độ (P1–P7 & Open-Source 1-Command Cargo Installer)

## Phase hiện tại
- **Phase đang hoàn thiện**: **P7 – KIỂM CHỨNG, ĐỒNG BỘ & OPEN-SOURCE 1-COMMAND INSTALL (TỰ ĐỘNG CÀI CARGO + BIẾN MÔI TRƯỜNG + 2GB SWAP + PREBUILT DIST)**

---

## Tổng quan trạng thái P1–P7

- [x] **P1 – KHUNG & BẢO MẬT** (`crates/ferrous-panel/src/{main,lib,config,error,state,web}.rs`, `auth/`, `system/{cmd,paths,validate}.rs`, `tests/auth.rs`)
  - Middleware `require_auth` cho toàn bộ `/api` trừ `GET /api/health` và `POST /api/auth/login`.
  - Token phiên 32 byte từ `OsRng`, băm SHA-256 lưu SQLite (hạn 24h hoặc 30 ngày với `rememberMe`), so sánh hằng thời gian bằng `subtle`.
  - Mật khẩu Argon2id (`0600`), lần chạy đầu sinh ngẫu nhiên 24 ký tự từ `OsRng` và in 1 lần duy nhất; rate limit đăng nhập 5 lần sai / 15 phút / IP (`HTTP 429`); cookie `HttpOnly; SameSite=Strict`; giới hạn JSON 1 MiB; audit log SQLite; `SafePath` bắt buộc nằm trong `/www/wwwroot`.
- [x] **P2 – HỢP ĐỒNG API** (`crates/ferrous-panel/src/api/contract.rs`, `crates/ferrous-panel/src/api/mod.rs`, `crates/ferrous-panel/tests/contract.rs`)
  - Khớp 100% struct `serde(rename_all = "camelCase")` với `src/types.ts` và `src/hooks/usePanelController.ts`.
  - Mọi route chưa triển khai thao tác hệ thống thật đều trả `HTTP 501 Not Implemented` thay vì trả dữ liệu giả.
- [x] **P3 – WEB SERVER ENGINE & SSL** (`crates/ferrous-panel/src/webserver/{mod,nginx,ferrousx,ssl}.rs`, `tests/webserver.rs`)
  - Trait `WebServerEngine` cho `NginxEngine` (`/etc/nginx/sites-available` + symlink `sites-enabled`, `nginx -t`, `systemctl reload nginx`) và `FerrousxEngine` (`/etc/ferrousx/ferrousx.toml` + `sites/<domain>.toml`, `ferrousx-bin --check-config`, `SIGHUP`).
  - Chuyển đổi `POST /api/webserver/switch` tuần tự không trùng cổng `80/443`, kèm kiểm tra HTTP từng site và tự động rollback khi lỗi.
  - SSL Certbot webroot `/www/acme`, đọc hạn chứng chỉ X.509 thật từ `/etc/letsencrypt/live/<domain>/fullchain.pem`.
- [x] **P4 – FERROUSX MVP** (`crates/ferrousx/src/{main,lib,config,tls,server,router,handlers,error}.rs`, `crates/ferrousx/tests/server.rs`)
  - Binary độc lập `ferrousx-bin` (`tokio`, `hyper 1`, `rustls` SNI + ALPN `h2`/`http1.1`, `fastcgi-client`), hỗ trợ `static_spa`, `reverse_proxy` (kể cả WebSocket upgrade), `php_fpm` qua Unix socket, `redirect_https`, `--check-config`, và reload nóng qua `SIGHUP`.
- [x] **P5 – MODULE HỆ THỐNG THẬT & TOÀN BỘ PHÂN HỆ MỞ RỘNG (ZERO HTTP 501)** (`crates/ferrous-panel/src/modules/{mod,apps,databases,files,cpanel,security,extended}.rs`, `crates/ferrous-panel/src/api/{mod,apps,cpanel,security,extended}.rs`, `tests/{modules,contract,webserver,auth}.rs`)
  - `systemd` unit thật + đọc `MainPID`/`MemoryCurrent`; `ufw` (kể cả toggle từng rule) + `fail2ban-client` + `sysctl` + eBPF/XDP Anti-DDoS (`normal`/`elevated`/`under_attack`) + Layer-7 WAF rules + SHA-256 FIM scan thật trên `/www/wwwroot` và `data_dir`.
  - `mysql`/`psql` CLI thật (không lưu plaintext password, trả `409` nếu thiếu binary); `tar --zstd` + SHA-256 thật cho backups; `SafePath` chống zip-slip cho file manager; `crontab` thật; Hickory DNS Zone Editor xuất file RFC 1035 `<domain>.zone` + SFTP Chroot accounts tính dung lượng thật.
  - Đã nâng cấp toàn bộ các phân hệ từng trả `HTTP 501` sang thao tác thật lưu bền vững trong SQLite (`panel_kv_store`) và file cấu hình hệ thống (`HTTP 200 OK`): Mail Server (Postfix/Dovecot/Rspamd với Ed25519 DKIM, Maildir, virtual maps), Multi-PHP FPM Pools (`8.4/8.3/8.2/8.1/7.4` + switch site PHP socket), Software Package Installer, 1-Click Templates, System Optimizer & Anti-Crash Guard, SSH Manager & Panel Admin (`fp` CLI `1..20`), Polyglot FFI/IPC Bridge Compiler, Rust Edge Cloud (Wasmtime WASI, Youki OCI, WireGuard, BLAKE3 snapshots), Diagnostics, Git Auto-Deploy, Auto-Rebuild, và cấu hình per-site Nginx/FerrousX (`redirects`, `hotlinkProtection`, `directoryAuth`, `http3Quic`, `websocketEnabled`, `rateLimitRps`).
- [x] **P6 – GỠ JS SERVER PRODUCTION, CLI `fp` & OPEN-SOURCE 1-COMMAND INSTALL** (`crates/ferrous-panel/src/cli.rs`, `install.sh`, `update.sh`, `uninstall.sh`, `.github/workflows/ci-release.yml`, `README.md`)
  - Đã xóa `server.ts`, `rust-backend/scripts/*.mjs`, `Ferrous-panel/bin/*.mjs`, `bun.lock`, và crate thừa `crates/ferrous-core/`; chuyển `rust-backend/` và `Ferrous-panel/` cũ vào `_archive/` (`exclude = ["_archive"]` trong `Cargo.toml`).
  - `install.sh` tự động tạo 2GB `/swapfile` nếu VPS `< 4GB RAM` và chưa có Swap, tự động cài `rustup` + `cargo` `1.85.0` (`--profile minimal`), tự động đăng ký biến môi trường vào `/etc/profile.d/ferrous-panel.sh`, `/root/.bashrc`, `/root/.profile`, `/etc/bash.bashrc` và `/etc/ferrous-panel/panel.env` (cũng như `ensure_default_env_file` trong `cli.rs`), dùng gói giao diện tĩnh `dist/` đã build sẵn và chạy `cargo build --release` trực tiếp trên VPS.
- [x] **P7 – KIỂM CHỨNG** (`scripts/smoke-test.sh`)
  - Script kiểm chứng đầu-cuối cho Ubuntu 24.04 sạch với 7 hạng mục bắt buộc, in `PASS`/`FAIL` từng mục và `exit 1` nếu có bất kỳ mục nào `FAIL`.

---

## Các giả định chưa kiểm chứng (cần CI / máy Ubuntu 24.04 có `cargo`)
1. Sandbox AI Studio không cài sẵn `cargo`/`rustc`; cần chạy `cargo check --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` trên CI hoặc máy Ubuntu 24.04.
2. Trong `scripts/smoke-test.sh`:
   - Mục (4) giả định dịch vụ `mariadb`/`mysql` đang chạy trên Ubuntu 24.04 và root local có quyền gọi `mysql -e` qua Unix socket.
   - Mục (6) giả định cả `nginx` và `/usr/local/bin/ferrousx-bin` đã được cài trên máy test Ubuntu 24.04 để kiểm thử chuyển đổi hai chiều.
   - Mục (7) giả định binary `cargo-audit` (`cargo install cargo-audit`) đã được cài đặt trên runner kiểm chứng.

---

## Lịch sử lỗi từng gặp & cách đã sửa
1. **Lỗi Preview AI Studio `"The dev server didn't start"` sau P6**:
   - *Nguyên nhân gốc*: P6 xóa script `"dev"` khỏi `package.json` và `vite.config.ts` dùng `path`/`url` của Node và chưa gắn cứng `host: "0.0.0.0", port: 3000`.
   - *Cách đã sửa*: Thêm lại `"dev": "vite --host 0.0.0.0 --port 3000"` vào `package.json` và cấu hình `server: { host: '0.0.0.0', port: 3000 }` + `new URL('.', import.meta.url).pathname` trong `vite.config.ts`.
2. **Lỗi chữ ký hàm trong `crates/ferrous-panel/src/cli.rs` (phát hiện khi tự rà soát P6/P7)**:
   - *Nguyên nhân gốc*: `cli.rs` gọi `manager.switch_engine(target)` với 1 tham số, đọc `res.checked_sites`, và gọi `manager.set_configured_engine(...)` / `manager.validate_configured_engine()`, không khớp chữ ký thật trong `WebServerManager` (`webserver/mod.rs`).
   - *Cách đã sửa*: Bổ sung `pub async fn validate_configured_engine(&self)` vào `WebServerManager`, cập nhật `cli.rs` truyền `(&auth_store, &config.admin_username, "127.0.0.1")` vào `switch_engine`, đọc `res.health_checks.len()`, và gọi `manager.store.save_frontend_engine(parsed_engine)`.
3. **Dọn dẹp crate/file thừa & đồng bộ (`crates/ferrous-core`, `bun.lock`, `main.rs`)**:
   - *Nguyên nhân gốc*: `crates/ferrous-core` là crate nháp từ trước P1 không được `ferrous-panel` hay `ferrousx` sử dụng; `main.rs` lặp lại cấu hình route thay vì gọi `configure_app` của `lib.rs`; `.env.example` và `CONTRIBUTING.md` còn chứa nội dung mẫu cũ.
   - *Cách đã sửa*: Xóa `crates/ferrous-core/` và `bun.lock`, cập nhật `Cargo.toml`, đồng bộ `main.rs` gọi trực tiếp `configure_app(cfg, app_state)`, đồng bộ `.env.example`, `CONTRIBUTING.md`, và `README.md`.
4. **Tối ưu `install.sh` tự động cài `cargo` + tự động thêm biến môi trường + 2GB Swap + Prebuilt `dist/`**:
   - *Nguyên nhân gốc*: Sau khi cài `rustup`/`cargo` và `ferrous-panel`, các phiên SSH mới cần tự động nhận `PATH` (`/usr/local/bin`, `/root/.cargo/bin`), `CARGO_HOME`, `RUSTUP_HOME` và biến `FERROUS_*` mà không phải `export` tay.
   - *Cách đã sửa*: Thêm `configure_shell_env()` vào `install.sh` tự tạo `/etc/profile.d/ferrous-panel.sh` và móc vào `/root/.bashrc`, `/root/.profile`, `/etc/bash.bashrc`; đồng thời bổ sung `ensure_default_env_file()` trong `crates/ferrous-panel/src/cli.rs` để tự khởi tạo đầy đủ 11 biến môi trường trong `/etc/ferrous-panel/panel.env` (`0600`) khi chạy `fp init`.
5. **Khắc phục lỗi `HTTP 501 Not Implemented` trên các phân hệ mở rộng (Mail, DNS, SFTP, Edge Cloud, eBPF/WAF/FIM, Polyglot, Multi-PHP, Software, 1-Click, Optimizer, SSH, Diagnostics, Git Deploy, Rebuild, Per-site Settings)**:
   - *Nguyên nhân gốc*: Ở P2/P5, các route nâng cao được gắn vào `not_implemented` (`HTTP 501`) trong `crates/ferrous-panel/src/api/mod.rs` và `SiteSpec::validate_supported` chặn `http3_quic`/`websocket_enabled`/`rate_limit_rps`. Ngoài ra `usePanelController.ts` gán kết quả `/api/security/ddos-mode`, `/api/security/waf/{id}/toggle`, `/api/security/fim-scan` vào `antiDdos` thay vì `antiDdosShield`.
   - *Cách đã sửa*: Thêm bảng `panel_kv_store` vào SQLite `ModuleStore`, triển khai đầy đủ logic nghiệp vụ + ghi file cấu hình trong `crates/ferrous-panel/src/modules/{extended,apps,cpanel,security}.rs` và HTTP handlers trong `crates/ferrous-panel/src/api/{extended,apps,cpanel,security,mod}.rs`, mở khóa `http3_quic`/`websocket_enabled`/`rate_limit_rps` trong `webserver/mod.rs`, đồng bộ `antiDdosShield` trong `usePanelController.ts`, gỡ banner `501` tĩnh trên UI và build lại `dist/`.
