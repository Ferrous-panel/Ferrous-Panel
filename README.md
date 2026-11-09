# FerrousPanel

FerrousPanel là một bảng điều khiển quản trị máy chủ Ubuntu tập trung vào việc quản lý website, PHP, SSH, database, mail server, bảo mật, và tối ưu hóa hệ thống theo hướng web-based. Dự án tích hợp giao diện React + Vite phía frontend và Node/Express + TypeScript để cung cấp trải nghiệm quản trị trực quan cho server Linux.

## Tổng quan

FerrousPanel được thiết kế để hỗ trợ các thao tác quản trị máy chủ nhanh và an toàn, bao gồm:

- Quản lý nhiều website và ứng dụng
- Hỗ trợ Multi-PHP (7.4–8.4)
- Quản lý file, ZIP, và cấu hình ứng dụng
- Web SSH, terminal và CLI quản trị
- Quản lý cơ sở dữ liệu MySQL / MariaDB
- Mail server và mailbox
- Tường lửa, bảo mật và fail2ban
- Cronjob, DNS, backup, FTP
- Tối ưu hóa hệ thống và watchdog
- Quản lý module Rust trong môi trường server

## Tính năng chính

### 1. Quản lý website
- Deploy website mới
- Hỗ trợ nhiều framework frontend/backend
- Cấu hình SSL, HTTP/3, reverse proxy
- Khởi động, dừng, khởi động lại, rebuild ứng dụng
- Kiểm tra trạng thái website trực tiếp

### 2. Multi-PHP
- Quản lý nhiều phiên bản PHP cùng lúc
- Thay đổi cấu hình php.ini
- Bật/tắt extension
- Chuyển PHP version cho từng website

### 3. File Manager
- Quản lý tập tin trên server
- Nén / giải nén ZIP
- Quản lý cấu trúc thư mục ứng dụng

### 4. SSH & Panel CLI
- Web SSH
- Quản lý public key SSH
- Xử lý session SSH
- CLI quản trị panel

### 5. Database
- Tạo, xóa database
- Quản lý MySQL / MariaDB
- Truy vấn SQL trực tiếp
- Tích hợp phpMyAdmin-like management

### 6. Mail Server
- Tạo domain mail
- Tạo mailbox
- Forwarder email
- Gửi test email

### 7. Security & Optimization
- Quản lý UFW firewall
- Khóa IP qua fail2ban
- Các thiết lập hardening
- Tối ưu RAM và tài nguyên
- Watchdog tự phục hồi

### 8. cPanel-like tools
- Cronjobs
- DNS records
- Backup / restore
- SFTP accounts

## Công nghệ sử dụng

- React 19
- Vite
- TypeScript
- Express
- Tailwind CSS
- Lucide React
- Google GenAI
- Node.js + TSX
- Rust modules (cho phần source management)

## Cấu trúc thư mục

```bash
.
├── .aistudio/
├── .env.example
├── .gitignore
├── Ferrous-panel/
├── index.html
├── metadata.json
├── package.json
├── server.ts
├── src/
│   ├── App.tsx
│   ├── components/
│   ├── index.css
│   ├── main.tsx
│   └── types.ts
├── tsconfig.json
├── vite.config.ts
└── ...
