# FerrousPanel — Trạng thái tiến độ (P1–P7 & Open-Source 1-Command Cargo Installer)

## Phase hiện tại
- **Phase đang hoàn thiện**: **P7 – KIỂM CHỨNG, ĐỒNG BỘ & OPEN-SOURCE 1-COMMAND INSTALL (TỰ ĐỘNG CÀI CARGO + 2GB SWAP + PREBUILT DIST)**

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
- [x] **P5 – MODULE HỆ THỐNG THẬT** (`crates/ferrous-panel/src/modules/{apps,databases,files,cpanel,security}.rs`, `tests/modules.rs`)
  - `systemd` unit thật + đọc `MainPID`/`MemoryCurrent`; `ufw` + `fail2ban-client` + `sysctl`; `mysql`/`psql` CLI thật (không lưu plaintext password, trả `409` nếu thiếu binary); `tar --zstd` + SHA-256 thật cho backups; `SafePath` chống zip-slip cho file manager; `crontab` thật.
  - Các mục Mail, DNS, SFTP, Edge Cloud, eBPF/WAF/FIM, Polyglot giữ nguyên `501` và hiển thị nhãn `"Chưa hỗ trợ"` trên UI.
- [x] **P6 – GỠ JS SERVER PRODUCTION, CLI `fp` & OPEN-SOURCE 1-COMMAND INSTALL** (`crates/ferrous-panel/src/cli.rs`, `install.sh`, `update.sh`, `uninstall.sh`, `.github/workflows/ci-release.yml`, `README.md`)
  - Đã xóa `server.ts`, `rust-backend/scripts/*.mjs`, `Ferrous-panel/bin/*.mjs`, `bun.lock`, và crate thừa `crates/ferrous-core/`; chuyển `rust-backend/` và `Ferrous-panel/` cũ vào `_archive/` (`exclude = ["_archive"]` trong `Cargo.toml`).
  - Theo lựa chọn của người dùng (Lựa chọn 2): `install.sh` tự động tạo 2GB `/swapfile` nếu VPS `< 4GB RAM` và chưa có Swap, tự động cài `rustup` + `cargo` `1.85.0` (`--profile minimal`), dùng gói giao diện tĩnh `dist/` đã build sẵn (không cần cài Node.js trên VPS) và chạy `cargo build --release` trực tiếp trên VPS (kèm tuỳ chọn `--prebuilt-binary`).
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
4. **Tối ưu `install.sh` tự động cài `cargo` + 2GB Swap + Prebuilt `dist/`**:
   - *Nguyên nhân gốc*: Người dùng chọn Lựa chọn 2 (tự động cài `cargo` khi chạy `install.sh`), yêu cầu tải sẵn gói giao diện tĩnh `dist/` (để không cần cài Node.js trên VPS) và tự động tạo 2GB Swap nếu VPS `< 4GB RAM` và chưa có Swap.
   - *Cách đã sửa*: Bỏ `dist/` khỏi `.gitignore` và đóng gói thêm `ferrous-dist.tar.gz` trong `.github/workflows/ci-release.yml`; cập nhật `install.sh` tự động tạo `/swapfile` 2GB khi RAM `< 4GB`, tự động cài `rustup` + `cargo` 1.85.0 (`--profile minimal`), nạp `dist/` và chạy `cargo build --release` trực tiếp trên VPS.
