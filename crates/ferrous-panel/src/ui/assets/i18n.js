// FerrousPanel Pure-Rust UI — Bilingual Localization Engine (Tiếng Việt / English)
const LANG_KEY = 'fp_lang';

const EXACT_VI_TO_EN = new Map([
  // Shell, Auth & Navigation
  ['Điều hành cốt lõi', 'Core Operations'],
  ['Hạ tầng & Dịch vụ', 'Infrastructure & Services'],
  ['Bảo mật & Hệ thống', 'Security & System'],
  ['Tổng quan hệ thống', 'System Overview'],
  ['Website & Ứng dụng', 'Websites & Apps'],
  ['Quản lý Tệp', 'File Manager'],
  ['Cơ sở dữ liệu SQL', 'FPSql & Database'],
  ['FPSql & Database', 'FPSql & Database'],
  ['Web Engine (Nginx/FX)', 'Web Engine (Nginx/FX)'],
  ['PHP-FPM & Phần mềm', 'PHP-FPM & Software'],
  ['Cron, Backup & DNS', 'Cron, Backup & DNS'],
  ['Máy chủ Mail', 'Mail Server'],
  ['Edge Cloud & Polyglot', 'Edge Cloud & Polyglot'],
  ['Tiến trình & Hệ thống', 'Processes & SysOps'],
  ['Bảo mật & Audit Log', 'Security & Audit Log'],
  ['Tối ưu & Chẩn đoán', 'Optimizer & Diagnostics'],
  ['SSH & Terminal', 'SSH & Terminal'],
  ['Tác vụ nền & Cài đặt', 'Jobs & Settings'],
  ['Đang khởi tạo FerrousPanel Pure-Rust UI…', 'Initializing FerrousPanel Pure-Rust UI…'],
  ['Bảng điều khiển VPS thuần Rust (Zero-Node.js)', 'Pure-Rust VPS Control Plane (Zero-Node.js)'],
  ['Tài khoản quản trị', 'Admin Username'],
  ['Tài khoản', 'Username'],
  ['Mật khẩu', 'Password'],
  ['Ghi nhớ phiên đăng nhập (30 ngày)', 'Remember session (30 days)'],
  ['Đăng nhập', 'Sign In'],
  ['Đăng nhập thành công.', 'Signed in successfully.'],
  ['Sai tài khoản hoặc mật khẩu.', 'Invalid username or password.'],
  ['Đăng xuất', 'Sign Out'],
  ['Đã đăng xuất.', 'Signed out.'],
  ['Làm mới', 'Refresh'],
  ['Giao diện sáng', 'Light Theme'],
  ['Giao diện tối', 'Dark Theme'],
  ['Tìm nhanh', 'Quick Jump'],
  ['Chuyển nhanh phân hệ (Ctrl+K)', 'Quick Jump Module (Ctrl+K)'],
  ['Không tìm thấy phân hệ phù hợp.', 'No matching module found.'],
  ['Đóng', 'Close'],
  ['Hủy', 'Cancel'],
  ['Xác nhận', 'Confirm'],
  ['Lưu', 'Save'],
  ['Xóa', 'Delete'],
  ['Sửa', 'Edit'],
  ['Đổi tên', 'Rename'],
  ['Tải lên', 'Upload'],
  ['Tải xuống', 'Download'],
  ['Giải nén', 'Extract'],
  ['Nén .zip', 'Compress .zip'],
  ['Đang xử lý…', 'Working…'],
  ['Thao tác', 'Actions'],
  ['Trạng thái', 'Status'],
  ['Tên miền', 'Domain'],
  ['Thời gian', 'Timestamp'],
  ['Chi tiết', 'Details'],
  ['Cấu hình', 'Configuration'],
  ['Nhật ký (Logs)', 'Logs'],

  // Overview
  ['Giám sát thời gian thực từ /proc, sysinfo, SQLite WAL và systemd thật trên máy chủ.', 'Real-time host telemetry from Linux /proc, sysinfo, SQLite WAL, and systemd.'],
  ['Cập nhật Panel', 'Update Panel'],
  ['Thêm Website', 'Add Website'],
  ['1-Click Deploy', '1-Click Deploy'],
  ['Tải CPU', 'CPU Load'],
  ['Bộ nhớ RAM', 'Memory (RAM)'],
  ['Lưu trữ đĩa', 'Disk Storage'],
  ['Mạng & Kết nối', 'Network & Connections'],
  ['Biểu đồ tải hệ thống (Lịch sử SQLite)', 'System Load History (SQLite Telemetry)'],
  ['Thông số máy chủ Linux', 'Linux Host Specifications'],
  ['Hệ điều hành', 'Operating System'],
  ['Nhân Linux (Kernel)', 'Linux Kernel'],
  ['Thời gian hoạt động (Uptime)', 'System Uptime'],
  ['Địa chỉ IP Công khai', 'Public IP Address'],
  ['Tải trung bình (1m / 5m / 15m)', 'Load Average (1m / 5m / 15m)'],
  ['Phiên bản Daemon', 'Daemon Version'],
  ['Danh sách Website đang chạy', 'Managed Websites'],
  ['Chưa có website nào được triển khai.', 'No websites deployed yet.'],

  // Websites & Apps
  ['Quản lý VirtualHost (FerrousX / Nginx), chứng chỉ SSL Let\'s Encrypt, PHP-FPM, Zero-Node.js Bun/Deno và tiến trình systemd.', 'Manage VirtualHosts (FerrousX / Nginx), Let\'s Encrypt SSL, PHP-FPM, Zero-Node.js Bun/Deno, and systemd services.'],
  ['Triển khai 1-Click (WordPress / App)', '1-Click Deploy (WordPress / App)'],
  ['Thêm Website mới', 'Add New Website'],
  ['Tên miền & Đường dẫn', 'Domain & Root Path'],
  ['Runtime / Engine', 'Runtime / Engine'],
  ['Tài nguyên & PID', 'Resources & PID'],
  ['SSL / HTTPS', 'SSL / HTTPS'],
  ['Kiểm tra HTTP', 'Test HTTP'],
  ['Cài SSL', 'Setup SSL'],
  ['Quản lý SSL', 'Manage SSL'],
  ['Tệp', 'Files'],
  ['Cấu hình Site', 'Site Settings'],
  ['Sửa Vhost', 'Edit Vhost'],
  ['Git Deploy', 'Git Deploy'],
  ['Rebuild', 'Rebuild'],
  ['Khởi động lại', 'Restart'],
  ['Dừng', 'Stop'],
  ['Chạy', 'Start'],
  ['Xóa Website', 'Delete Website'],
  ['Chưa có website nào. Bấm "Thêm Website mới" hoặc "1-Click Deploy" để bắt đầu.', 'No websites yet. Click "Add New Website" or "1-Click Deploy" to get started.'],

  // File Manager
  ['Duyệt, chỉnh sửa, tải lên nhị phân, nén/giải nén an toàn trong phạm vi SafePath (/www/wwwroot).', 'Browse, edit, binary upload, and archive safely inside SafePath (/www/wwwroot).'],
  ['Thư mục mới', 'New Folder'],
  ['Tệp mới', 'New File'],
  ['Tải tệp lên', 'Upload File'],
  ['Tải từ URL', 'Remote Download'],
  ['Phân quyền hàng loạt', 'Batch Chmod'],
  ['Tên tệp / thư mục', 'Name'],
  ['Kích thước', 'Size'],
  ['Quyền (Octal)', 'Permissions'],
  ['Cập nhật lúc', 'Modified'],
  ['Thư mục trống.', 'Directory is empty.'],
  ['Cần tạo ít nhất 1 website trước khi duyệt tệp trong /www/wwwroot.', 'Create at least one website first before browsing files in /www/wwwroot.'],

  // FPSql & Database + Myphpadmin + pgAdmin
  ['Đồng bộ SQL', 'Sync SQL'],
  ['Tạo Database mới', 'Create Database'],
  ['Tên Database', 'Database Name'],
  ['Engine & Trạng thái', 'Engine & Status'],
  ['Kết nối / Bảng / Dung lượng', 'Conns / Tables / Size'],
  ['Chạy SQL', 'Run SQL'],
  ['Đổi MK', 'Password'],
  ['Xuất .sql', 'Export .sql'],
  ['Nhập .sql', 'Import .sql'],
  ['Mở toàn màn hình', 'Open Fullscreen'],
  ['Làm mới dịch vụ', 'Refresh Service'],
  ['Cấu hình SQL', 'SQL Config'],
  ['Cấu hình MySQL', 'MySQL Config'],
  ['Lưu cấu hình & Reload MariaDB', 'Save Config & Reload MariaDB'],
  ['Chưa có cơ sở dữ liệu nào. Bạn có thể tạo ngay MariaDB 11.4, PostgreSQL 17 hoặc SQLite 3 (WAL).', 'No databases yet. Create a MariaDB 11.4, PostgreSQL 17, or SQLite 3 (WAL) database right away.'],

  // Infrastructure: Web Engine, Software, Cron/Backup/DNS, Mail, Edge Cloud
  ['Web Server Engine (Nginx & FerrousX)', 'Web Server Engine (Nginx & FerrousX)'],
  ['Kho Phần mềm & Quản lý PHP-FPM', 'Software Store & PHP-FPM Manager'],
  ['Cron Jobs, Sao lưu Zstd, DNS Zone & SFTP', 'Cron Jobs, Zstd Backups, DNS Zone & SFTP'],
  ['Máy chủ Mail (Postfix / Dovecot / Rspamd)', 'Mail Server (Postfix / Dovecot / Rspamd)'],
  ['Thêm tên miền', 'Add Domain'],
  ['Thêm Forwarder', 'Add Forwarder'],
  ['Gửi Mail thử', 'Send Test Email'],
  ['Tạo hộp thư', 'Create Mailbox'],
  ['Edge Cloud & Polyglot FFI', 'Edge Cloud & Polyglot FFI'],
  ['Vận hành thật với Wasmtime, Podman/Docker, WireGuard VPN, BLAKE3 Merkle Snapshots và Rust C-ABI (.so). Có thể cài 1-click ngay bên dưới hoặc trong Kho Phần mềm.', 'Real Linux execution with Wasmtime, Podman/Docker, WireGuard VPN, BLAKE3 Merkle Snapshots, and Rust C-ABI (.so). One-click install supported below.'],
  ['Cài toàn bộ Edge Runtimes (1-Click)', 'Install All Edge Runtimes (1-Click)'],
  ['Cài Wasmtime (1-Click)', 'Install Wasmtime (1-Click)'],
  ['Cài Podman (1-Click)', 'Install Podman (1-Click)'],
  ['Cài WireGuard (1-Click)', 'Install WireGuard (1-Click)'],
  ['Cài Mail Stack (1-Click)', 'Install Mail Stack (1-Click)'],
  ['Cài Fail2ban (1-Click)', 'Install Fail2ban (1-Click)'],
  ['Cài MariaDB (1-Click)', 'Install MariaDB (1-Click)'],
  ['Cài PostgreSQL (1-Click)', 'Install PostgreSQL (1-Click)'],
  ['Cài Certbot (1-Click)', 'Install Certbot (1-Click)'],
  ['Cài Nginx (1-Click)', 'Install Nginx (1-Click)'],
  ['Thêm WASI Worker', 'Add WASI Worker'],
  ['Chạy OCI Container', 'Run OCI Container'],
  ['Thêm WireGuard Peer', 'Add WireGuard Peer'],
  ['Tạo BLAKE3 Snapshot', 'Create BLAKE3 Snapshot'],
  ['Biên dịch FFI (.so)', 'Compile FFI (.so)'],
  ['Tên & Image', 'Name & Image'],
  ['Trạng thái & Tài nguyên', 'Status & Resources'],
  ['Chưa có container OCI nào.', 'No OCI containers yet.'],
  ['Chưa có WireGuard peer nào.', 'No WireGuard peers yet.'],
  ['Chưa có WASI worker nào.', 'No WASI workers yet.'],
  ['Chưa có BLAKE3 snapshot nào.', 'No BLAKE3 snapshots yet.'],
  ['Ngôn ngữ đích', 'Target Language'],
  ['Giao thức Bridge', 'Bridge Protocol'],
  ['Trạng thái trên VPS', 'Host Status'],

  // Security, SysOps, Optimizer, SSH, Settings
  ['Tiến trình Linux, Cổng mạng, Systemd & Phân vùng đĩa', 'Linux Processes, Open Ports, Systemd & Disks'],
  ['Tiến trình, Dịch vụ & Hệ thống Linux', 'Linux Processes, Services & Host System'],
  ['Trung tâm Bảo mật (UFW, Fail2ban, WAF, FIM & Audit)', 'Security Center (UFW, Fail2ban, WAF, FIM & Audit)'],
  ['Tối ưu Kernel Sysctl, ZRAM & Tự chẩn đoán Hệ thống', 'Kernel Sysctl Optimizer, ZRAM & Self-Diagnostics'],
  ['Tối ưu hệ thống, Kernel Sysctl & Chẩn đoán VPS', 'System Optimizer, Kernel Sysctl & VPS Diagnostics'],
  ['Quản trị SSH, Khóa Ed25519 & Web Terminal Allowlist', 'SSH Hardening, Ed25519 Keys & Web Terminal'],
  ['Quản trị SSH & Web Terminal', 'SSH Hardening & Web Terminal'],
  ['Tác vụ nền (Async Jobs), Cập nhật & Cài đặt Panel', 'Background Jobs, Panel Updater & Settings'],
  ['Tác vụ nền & Cài đặt FerrousPanel', 'Background Jobs & FerrousPanel Settings'],
  ['Tính năng chưa sẵn sàng trên máy chủ này.', 'This capability is not yet ready on this host.'],

  // Additional Modal, Table & Form Labels across all tabs
  ['Kiểm tra cú pháp & Áp dụng (Reload)', 'Test Syntax & Reload'],
  ['Kiểm tra DNS Preflight', 'Run DNS Preflight'],
  ['Kiểm tra khóa & Cài chứng chỉ', 'Verify Key & Install Certificate'],
  ['Làm trống log', 'Truncate Log'],
  ['Lên 1 cấp', 'Up One Level'],
  ['Lệnh gợi ý', 'Suggested Command'],
  ['Lệnh khởi chạy (tùy chọn)', 'Start Command (optional)'],
  ['Let\'s Encrypt (Miễn phí)', 'Let\'s Encrypt (Free)'],
  ['Loại', 'Type'],
  ['Loại FS', 'FS Type'],
  ['Log dùng chung của hệ thống', 'System Shared Log'],
  ['Lưu & Áp dụng cấu hình', 'Save & Apply Config'],
  ['Lưu & Áp dụng', 'Save & Apply'],
  ['Lưu thay đổi', 'Save Changes'],
  ['Lý do kiểm chứng', 'Verification Detail'],
  ['Mật khẩu Admin (hiển thị 1 lần)', 'Admin Password (shown once)'],
  ['Mật khẩu Admin WordPress (để trống = tự sinh bảo mật)', 'WP Admin Password (blank = auto-generate)'],
  ['Mật khẩu DB (để trống = tự sinh bảo mật)', 'DB Password (blank = auto-generate)'],
  ['Mật khẩu (để trống = tự sinh)', 'Password (blank = auto-generate)'],
  ['Mật khẩu mới (để trống để giữ hash cũ)', 'New Password (blank = keep current)'],
  ['Mật khẩu mới (tối thiểu 8 ký tự)', 'New Password (min 8 chars)'],
  ['Nén ngay', 'Compress Now'],
  ['Nén thư mục', 'Archive Folder'],
  ['Nhánh Git (Branch — để trống = tự nhận main/master)', 'Git Branch (blank = auto-detect main/master)'],
  ['Nhãn / Realm', 'Label / Realm'],
  ['Nhật ký Website (Error / Access / Journal)', 'Website Logs (Error / Access / Journal)'],
  ['Nội dung ban đầu', 'Initial Content'],
  ['Nội dung tệp', 'File Content'],
  ['Phân hệ', 'Module'],
  ['Phần mở rộng bảo vệ Hotlink', 'Hotlink Protected Extensions'],
  ['Quản lý SQL', 'Manage SQL'],
  ['Quản lý Tệp (SafePath)', 'File Manager (SafePath)'],
  ['Quyền', 'Permissions'],
  ['Rate Limit theo IP (req/giây, 0 = tắt)', 'Per-IP Rate Limit (req/s, 0 = off)'],
  ['Sẵn sàng cấp Let\'s Encrypt', 'Ready for Let\'s Encrypt'],
  ['Tác vụ', 'Task'],
  ['Tài khoản Admin', 'Admin Username'],
  ['Tài khoản Admin WordPress', 'WordPress Admin User'],
  ['Tài khoản DB', 'DB Username'],
  ['Tài khoản DB (để trống = tự sinh)', 'DB Username (blank = auto-generate)'],
  ['Tải về máy chủ', 'Fetch to Server'],
  ['Tạo & Áp dụng Self-Signed SSL', 'Generate & Apply Self-Signed SSL'],
  ['Tạo & Triển khai', 'Create & Deploy'],
  ['Tạo Database', 'Create Database'],
  ['Tạo mới', 'Create New'],
  ['Tạo tệp', 'Create File'],
  ['Tạo thư mục', 'Create Folder'],
  ['Tắt & Xóa chứng chỉ', 'Disable & Remove Cert'],
  ['Tắt HTTPS (giữ tệp)', 'Disable HTTPS (keep cert)'],
  ['Tên', 'Name'],
  ['Tên Database (để trống = tự đặt theo tên miền)', 'Database Name (blank = derive from domain)'],
  ['Tên file nén', 'Archive Filename'],
  ['Tên hiển thị', 'Display Name'],
  ['Tên miền chính', 'Primary Domain'],
  ['Tên miền phụ / SAN bổ sung (phân cách dấu phẩy)', 'Aliases / Extra SANs (comma-separated)'],
  ['Tên miền triển khai', 'Target Domain'],
  ['Tên mới', 'New Name'],
  ['Tên tệp lưu lại', 'Save As Filename'],
  ['Thời hạn hiệu lực', 'Validity Period'],
  ['Thực thi SQL', 'Execute SQL'],
  ['Thư mục chạy con (Subdirectory — tùy chọn, ví dụ: public / dist)', 'App Subdirectory (optional, e.g. public / dist)'],
  ['Thư mục nguồn', 'Source Directory'],
  ['Tiêu đề trang', 'Site Title'],
  ['Trang Quản trị (WP-Admin)', 'Admin URL (WP-Admin)'],
  ['Trạng thái ACME HTTP-01', 'ACME HTTP-01 Status'],
  ['Triển khai ngay', 'Deploy Now'],
  ['Triển khai Website', 'Deploy Website'],
  ['Tự động cấp SSL / HTTPS', 'Auto-Issue SSL / HTTPS'],
  ['Tự động gia hạn (systemd)', 'Auto-Renewal (systemd)'],
  ['URL (HTTP/HTTPS công khai)', 'Public URL (HTTP/HTTPS)'],
  ['URL nguồn tài nguyên bên ngoài (tùy chọn: .git / .tar.gz / .zip)', 'External Source URL (optional: .git / .tar.gz / .zip)'],
  ['Vhost & Log', 'Vhost & Logs'],
  ['Web Server & Tác vụ nền gần nhất', 'Web Server & Recent Background Jobs'],
  ['Xem log', 'View Logs'],
  ['Áp dụng & Reload Vhost', 'Apply & Reload Vhost'],
  ['Bật/Tắt', 'Toggle'],
  ['Bật Zend OPcache', 'Enable Zend OPcache'],
  ['Biên dịch (.so)', 'Compile (.so)'],
  ['Cài đặt', 'Install'],
  ['Cập nhật', 'Update'],
  ['Chạy sao lưu nền', 'Run Background Backup'],
  ['Chế độ định tuyến', 'Routing Mode'],
  ['Chỉnh php.ini', 'Edit php.ini'],
  ['Chứng chỉ hết hạn', 'Expired Certs'],
  ['Chuyển sang FerrousX', 'Switch to FerrousX'],
  ['Chuyển sang Nginx', 'Switch to Nginx'],
  ['Đường dẫn bảo mật (Security Entrance)', 'Security Entrance Path'],
  ['Fail2ban chưa kích hoạt jail nào', 'No active Fail2ban jails'],
  ['Fail2ban Tổng chặn', 'Fail2ban Total Bans'],
  ['Ghi chú', 'Notes'],
  ['Giải phóng RAM', 'Drop PageCache'],
  ['Giải phóng & Reload PHP', 'Flush & Reload PHP'],
  ['Giao thức', 'Protocol'],
  ['Giá trị thật', 'Live Value'],
  ['Hành động', 'Action'],
  ['Hiện tại / Chuẩn', 'Current / Baseline'],
  ['ID & Tác vụ', 'ID & Task'],
  ['IP nguồn / Thời gian', 'Source IP / Time'],
  ['Kernel & CPU / Múi giờ', 'Kernel & CPU / Timezone'],
  ['Khởi động lại FerrousPanel', 'Restart FerrousPanel'],
  ['Khuyến nghị', 'Recommended'],
  ['Kiểm tra bản mới', 'Check Updates'],
  ['Lưu cấu hình SSH', 'Save SSH Config'],
  ['Lưu Public Key', 'Save Public Key'],
  ['Máy chủ & Hệ điều hành', 'Host & Operating System'],
  ['Mô tả', 'Description'],
  ['Ngắt', 'Terminate'],
  ['Ngắt phiên', 'Kill Session'],
  ['Ngưỡng xả tải CPU (%)', 'CPU Load-Shedding (%)'],
  ['Ngưỡng xả tải RAM (%)', 'RAM Load-Shedding (%)'],
  ['Nhãn', 'Label'],
  ['Nhãn thiết bị', 'Device Label'],
  ['Nhật ký', 'Logs'],
  ['Phân vùng & Ổ đĩa thực tế (df -PT)', 'Mounted Filesystems & Disks (df -PT)'],
  ['Quét BLAKE3 + SHA-256 FIM', 'Run BLAKE3 + SHA-256 FIM Scan'],
  ['RAM & Swap thực tế', 'Live RAM & Swap'],
  ['Rollback bản trước', 'Rollback Previous'],
  ['Sinh cặp khóa Ed25519', 'Generate Ed25519 Keypair'],
  ['Sử dụng', 'Usage'],
  ['Swap, Ổ đĩa & Kernel', 'Swap, Disks & Kernel'],
  ['Tạo / Cập nhật /swapfile', 'Create / Resize /swapfile'],
  ['Tắt & Xóa /swapfile', 'Disable & Delete /swapfile'],
  ['Tham số Kernel Sysctl (đọc thật từ /proc/sys)', 'Live Kernel Sysctl Parameters (/proc/sys)'],
  ['Thêm luật UFW', 'Add UFW Rule'],
  ['Thêm Public Key', 'Add Public Key'],
  ['Thiết bị', 'Device'],
  ['Thực thi', 'Execute'],
  ['Thu hồi', 'Revoke'],
  ['Tiến trình', 'Processes'],
  ['Tiến trình & Lệnh', 'Process & Command'],
  ['Tinh chỉnh nhân Linux (Sysctl Allowlist)', 'Linux Kernel Tuning (Sysctl Allowlist)'],
  ['Toàn vẹn tệp (BLAKE3 FIM)', 'File Integrity (BLAKE3 FIM)'],
  ['Top tiến trình theo RAM (đọc trực tiếp từ /proc)', 'Top Processes by RAM (live /proc)'],
  ['Xác thực Public Key', 'Public Key Auth'],
]);

const PATTERN_VI_TO_EN = [
  [/^Danh sách DB \((\d+)\)$/, 'Databases ($1)'],
  [/^Gợi ý khắc phục:\s*(.+)$/, 'Suggested fix: $1'],
  [/^(\d+\/\d+) conn · (\d+) bảng · ([\d.]+) MB$/, '$1 conn · $2 tables · $3 MB'],
  [/^(\d+) lần · (\d+) µs$/, '$1 runs · $2 µs'],
  [/^Khởi động (.+)$/, 'Start $1'],
  [/^Cài (.+)$/, 'Install $1'],
  [/^Tải & Kích hoạt (.+)$/, 'Download & Enable $1'],
  [/^Clone & Kích hoạt (.+)$/, 'Clone & Enable $1'],
  [/^Đang chạy \((.+)\)$/, 'Running ($1)'],
  [/^Đã cài · Chưa chạy$/, 'Installed · Stopped'],
  [/^Chưa tải$/, 'Not Installed'],
  [/^Chưa cài$/, 'Not Installed'],
  [/^Gõ tên phân hệ.*$/, 'Type module name (Websites, FPSql, PHP, Mail, Security, Edge…)'],
  [/^Đã đồng bộ 2 chiều (\d+) cơ sở dữ liệu với (.+)\.$/, 'Synchronized $1 databases two-way with $2.'],
  [/^Đã cài đặt & khởi chạy (.+) tại (.+)!$/, 'Installed & started $1 at $2!'],
  [/^Đang tự động cài đặt gói dịch vụ hệ thống \(1-Click\)…$/, 'Installing system runtime package (1-Click)…'],
  [/^Đã cài đặt và kích hoạt tính năng thành công\.$/, 'Package installed and activated successfully.'],
];

export function getLang() {
  try {
    const saved = localStorage.getItem(LANG_KEY);
    if (saved === 'en' || saved === 'vi') return saved;
  } catch (_) {}
  return 'vi';
}

export function isEn() {
  return getLang() === 'en';
}

export function t(vi, en) {
  return isEn() ? en : vi;
}

export function translateString(raw) {
  if (!isEn() || typeof raw !== 'string') return raw;
  const trimmed = raw.trim();
  if (!trimmed) return raw;
  if (EXACT_VI_TO_EN.has(trimmed)) {
    const mapped = EXACT_VI_TO_EN.get(trimmed);
    const lead = raw.match(/^\s*/)?.[0] || '';
    const trail = raw.match(/\s*$/)?.[0] || '';
    return `${lead}${mapped}${trail}`;
  }
  for (const [re, repl] of PATTERN_VI_TO_EN) {
    if (re.test(trimmed)) {
      const converted = trimmed.replace(re, repl);
      const lead = raw.match(/^\s*/)?.[0] || '';
      const trail = raw.match(/\s*$/)?.[0] || '';
      return `${lead}${converted}${trail}`;
    }
  }
  return raw;
}

const SKIP_TAGS = new Set(['SCRIPT', 'STYLE', 'CODE', 'PRE', 'TEXTAREA']);

export function translateDomSubtree(root) {
  if (!isEn() || !root) return;
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT);
  let node = walker.currentNode;
  while (node) {
    if (node.nodeType === Node.ELEMENT_NODE) {
      const el = node;
      for (const attr of ['placeholder', 'title', 'aria-label']) {
        const val = el.getAttribute?.(attr);
        if (val) {
          const next = translateString(val);
          if (next !== val) el.setAttribute(attr, next);
        }
      }
    } else if (node.nodeType === Node.TEXT_NODE) {
      const parent = node.parentElement;
      if (parent && !SKIP_TAGS.has(parent.tagName) && !parent.closest('.terminal-box')) {
        const cur = node.nodeValue;
        if (cur && cur.trim()) {
          const next = translateString(cur);
          if (next !== cur) node.nodeValue = next;
        }
      }
    }
    node = walker.nextNode();
  }
}

let observerInstalled = false;
let isMutating = false;

export function installDomTranslator() {
  if (observerInstalled || typeof MutationObserver === 'undefined') return;
  observerInstalled = true;
  document.documentElement.lang = getLang();
  const obs = new MutationObserver((mutations) => {
    if (!isEn() || isMutating) return;
    isMutating = true;
    try {
      for (const m of mutations) {
        if (m.type === 'childList') {
          m.addedNodes.forEach((n) => {
            if (n.nodeType === Node.ELEMENT_NODE || n.nodeType === Node.TEXT_NODE) {
              translateDomSubtree(n);
            }
          });
        }
      }
    } finally {
      isMutating = false;
    }
  });
  if (document.body) {
    obs.observe(document.body, { childList: true, subtree: true });
    if (isEn()) translateDomSubtree(document.body);
  }
}

export function setLang(nextLang) {
  const clean = nextLang === 'en' ? 'en' : 'vi';
  try {
    localStorage.setItem(LANG_KEY, clean);
  } catch (_) {}
  document.documentElement.lang = clean;
  return clean;
}
