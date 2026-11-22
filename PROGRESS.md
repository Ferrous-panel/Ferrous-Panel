# FerrousPanel — Trạng thái tiến độ & Nhật ký khắc phục (Phần 1–6)

## Tổng quan trạng thái hiện tại
- **Phiên bản Rust yêu cầu**: `stable (>= 1.89)` (đồng bộ với `rust-version = "1.89"` trong `Cargo.toml`).
- **Trạng thái biên dịch trong sandbox**: **chưa biên dịch** (môi trường sandbox AI Studio không cài sẵn `cargo`/`rustc`; cần chạy `cargo check --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` trên Ubuntu 24.04 hoặc GitHub Actions).
- **Trạng thái giao diện**: Production chạy 100% Pure-Rust Native UI (`crates/ferrous-panel/src/ui/assets/`, nhúng trực tiếp qua `rust-embed`, không cần Node.js). Preview AI Studio vẫn hoạt động bình thường trên cổng `3000` (`vite.config.ts` / `package.json`).

---

## Trạng thái khắc phục 6 phần (Bảo mật · Chống giả lập · Chạy thật · UI · CI)

### Phần 1 — Bảo mật
- [x] **1.1** `install.sh`, `update.sh`, `scripts/panel.env.example`: Mặc định `FERROUS_BIND=127.0.0.1`, chỉ bind công khai khi truyền `--bind` tường minh kèm cảnh báo HTTP không mã hóa và cảnh báo cookie; tự bật `FERROUS_SECURE_COOKIE=true` khi có domain + chứng chỉ Let's Encrypt.
- [x] **1.2** `install.sh`, `update.sh`: Xác thực SHA-256 bắt buộc (`exit 1` khi thiếu `SHA256SUMS.txt` hoặc không khớp asset trừ khi truyền `--insecure-skip-verify`), áp dụng cho cả binary chính và binary phụ (`bun`, `tailwindcss`).
- [x] **1.3** `modules/extended/ssh_admin.rs`: Thu hẹp allowlist WebSSH (`sysctl` chỉ đọc, `ip` chỉ đọc, chặn `ss -K`, chặn `journalctl --vacuum*/--rotate/--flush`, `ls` chỉ trong `wwwroot`/`data_dir` sau `canonicalize`), bỏ output giả (`whoami` gọi `id -un`, thiếu binary trả `409 Conflict`).
- [x] **1.4** `modules/files.rs`: Chống SSRF cho `remote_download` (`--max-redirs 0`, `--proto-redir =http,https`, phân giải DNS trước + ghim IP qua `--resolve`, chặn IP nội bộ/CGNAT `100.64.0.0/10`/`0.0.0.0/8`/`192.0.0.0/24`/`198.18.0.0/15`/`169.254.169.254`/`::ffff:a.b.c.d` và host số decimal/hex/octal).
- [x] **1.5** `modules/files.rs`: Kiểm tra symlink trong ZIP (`unzip -Z` + staging dir check) và tách `tar -tf` + `tar -tvf` xử lý đúng tên file có khoảng trắng.
- [x] **1.6** `system/cmd.rs`: Giới hạn đọc stdout/stderr ngay khi đọc (`AsyncReadExt::take(64 KiB)`), thêm `validate_no_flag_injection` + `--` trước tham số vị trí.
- [x] **1.7** `state.rs` & `api/telemetry.rs`: Thay `std::process::Command` blocking ngoài allowlist bằng `executor.run(AllowedBinary::Nginx, &["-v"], ...)`.
- [x] **1.8** `api/mod.rs`: Rà soát `/api/actix-gateway/{subsystem}` — giữ lại vì được sử dụng bởi AI Studio Preview (`src/services/actixGateway.ts`) và bộ test hợp đồng hiện có (`tests/modules.rs`, `tests/contract.rs`).

### Phần 2 — Nền tảng chống giả lập
- [x] **2.1** `config.rs` & `modules/extended/common.rs`: Bỏ kiểm tra đường dẫn `/var/lib/ferrous-panel`; thay bằng `FERROUS_MODE=production|test` (`RuntimeMode`, mặc định `Production`).
- [x] **2.2** Rà soát và xử lý lỗi thực tế toàn bộ `let _ = executor.run(...)` / `let _ = atomic_write_file(...)` trên nhánh thực thi chính trong `apps.rs`, `php.rs`, `mail.rs`, `optimizer.rs`, `software.rs`, `edge_cloud.rs`, `polyglot.rs`, `ssh_admin.rs`, `security.rs`, `cpanel.rs`.
- [x] **2.3** `modules/capabilities.rs` + `GET /api/capabilities`: Trả danh sách khả năng thực tế của máy chủ (`ready | degraded | unavailable | experimental`, kèm `reason` và `fixHint`), kết nối trực tiếp vào UI.

### Phần 3 — Tính năng hạt nhân chạy thật
- [x] **3.1** Nginx & FerrousX: Render đầy đủ `rate_limit_rps` (Nginx `limit_req_zone`/`limit_req`; FerrousX token-bucket theo IP + bộ đếm `rate_limited`), `redirects`, `hotlinkProtection`, `directoryAuth` (file `$apr1$` htpasswd dùng chung hai engine), `websocketEnabled`; từ chối `http3Quic: true` bằng `HTTP 409` khi engine không hỗ trợ; rollback tự động khi lỗi.
- [x] **3.2** SSL Certbot: Kiểm tra phân giải DNS trước khi xin chứng chỉ, đọc hạn chứng chỉ X.509 thật, trả lỗi nguyên văn từ `certbot`.
- [x] **3.3** Ứng dụng: `git` qua validator + `--`, tạo/điều khiển `systemd` unit thật, xác nhận `systemctl is-active` và trả log `journalctl` khi khởi động thất bại.
- [x] **3.4** File manager: Mọi thao tác qua `SafePath`, chặn traversal & symlink.
- [x] **3.5** Database: Truyền mật khẩu MariaDB/PostgreSQL qua `stdin` (`run_with_stdin`), tuyệt đối không lộ trên command line hay log.
- [x] **3.6** Firewall + Fail2ban: Chặn tự khóa cổng SSH & cổng Panel trừ khi có xác nhận tường minh (`?force=true`), xóa số liệu giả DDoS/WAF, gắn nhãn `eBPF/XDP` là `experimental`.
- [x] **3.7** Cron + Backup: Crontab thật, `tar --zstd` + kiểm tra SHA-256 trước khi restore.
- [x] **3.8** Telemetry + Audit log: Số liệu thật từ `/proc` + `sysinfo`, ghi audit log cho mọi thay đổi.

### Phần 4 — Tính năng mở rộng chạy thật
- [x] **4.1** Multi-PHP: Kiểm tra cú pháp `php-fpm -t` + rollback khi sửa `php.ini`/extension, xác nhận `systemctl is-active`, trả `409` khi thiếu gói.
- [x] **4.2** Mail Server: Sinh khóa RSA-2048 thật qua `openssl` (`0600`), kiểm tra DNS thật qua `dig +short TXT` (SPF/DKIM/DMARC), snapshot + `postmap` + reload Postfix có rollback, bỏ hoàn toàn dữ liệu giả.
- [x] **4.3** Optimizer: Whitelist `sysctl` an toàn, ghi `/etc/sysctl.d/99-ferrous-optimizer.conf` có rollback khi `sysctl -p` lỗi và đọc lại `/proc/sys`.
- [x] **4.4** SSH Manager: Yêu cầu `confirmPortChange: true` khi đổi cổng SSH, `sshd -t` + rollback, sinh khóa thật bằng `ssh-keygen`.
- [x] **4.5** FerrousX IPC: Kiểm thử đầu-cuối thật qua UDS (`status`, `vhost_add`, `reload`, `vhost_del`, bảo toàn cấu hình cũ khi reload lỗi).
- [x] **4.6** Edge Cloud & Polyglot: Bỏ toàn bộ mock seed và giả lập chữ ký/khóa; gắn nhãn `experimental`, chỉ chạy thật khi có `wasmtime`/`podman`/`docker`/`wg`/`rustc`/`cargo`, thiếu trả `409`.

### Phần 5 — Giao diện Pure-Rust Native UI
- [x] **5.1–5.4** Hoàn thiện `crates/ferrous-panel/src/ui/assets/{app.js,views-core.js,views-infra.js,views-security.js,panel.css}`: đồng bộ `GET /api/capabilities` (làm mờ nút + hiển thị lý do & lệnh cài), thêm đầy đủ modal Site Settings, PHP ini/switch-site, Mail DNS/DKIM/forwarder/test, Software config editor, Edge Cloud & Polyglot, File rename/compress/remote-download/chmod, hộp thoại xác nhận hai bước cho thao tác nguy hiểm (UFW anti-lockout, đổi cổng SSH/Panel, xóa), sửa mọi payload lệch giữa UI và API, không dùng `window.alert`/`window.confirm`/`window.open`.

### Phần 6 — Kiểm chứng, CI & Tài liệu
- [x] **6.1** Mở rộng `scripts/smoke-test.sh` (14 hạng mục với `PASS`/`FAIL`/`SKIP (lý do)` và `exit 1` khi `FAIL > 0`).
- [x] **6.2** Thêm `crates/ferrous-panel/tests/security_and_real_ops.rs` và `crates/ferrousx/tests/ipc_and_site_features.rs` (kể cả `MockExecutor` kiểm chứng chuỗi lệnh & rollback).
- [x] **6.3** Cập nhật `.github/workflows/ci-release.yml`: chỉ publish Release khi push tag `v*` và thêm job `integration-smoke-test` trên `ubuntu-24.04` cài `nginx`, `php-fpm`, `mariadb-server`, `ufw`.
- [x] **6.4** Đồng bộ `PROGRESS.md` và `README.md`.

---

## Lệnh kiểm chứng trên Ubuntu 24.04
```bash
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
sudo bash scripts/smoke-test.sh
```
