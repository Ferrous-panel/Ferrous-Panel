# Kế Hoạch Xây Dựng Trang Đăng Nhập Bắt Buộc & Quản Lý Phiên Admin Cho FerrousPanel

Xây dựng hệ thống bảo vệ toàn diện cho FerrousPanel với màn hình Đăng nhập (Login) bắt buộc mang phong cách quản trị viên hiện đại (chuẩn aaPanel / cPanel), xác thực tài khoản Admin VPS, quản lý phiên làm việc (Session Token) an toàn và tích hợp nút Đăng xuất (Logout) trên giao diện.

---

## Quyết Định Cốt Lõi Từ Người Dùng (Confirmed Decisions)

- **Phong cách giao diện**: Form đăng nhập hiện đại kiểu aaPanel/cPanel với biểu tượng thương hiệu, ô tài khoản, mật khẩu (kèm nút ẩn/hiện), checkbox ghi nhớ đăng nhập và thẻ tra cứu/sao chép thông tin tài khoản mặc định nhanh cho quản trị viên.
- **Cơ chế phiên làm việc**: Quản lý phiên bằng Session Token được cấp phát bởi backend (`/api/auth/login`), lưu trữ an toàn trong `localStorage` / cookie, tự động kiểm tra tính hợp lệ khi tải lại trang (`/api/auth/session`), hỗ trợ nút Đăng xuất (Logout) trên thanh điều hướng.
- **Tài khoản quản trị viên**: Đồng bộ trực tiếp với tài khoản Admin được tạo khi cài đặt VPS (`/etc/ferrous-panel/admin.json` và `/www/server/panel/default.pl`, mặc định `admin_ferrous` / `Fp#2026!UbuntuRoot`), có thể đổi mật khẩu qua cả Web UI lẫn lệnh Terminal VPS (`fp 5`).

---

## 1. Kiến Trúc Hệ Thống & Luồng Xác Thực

```
┌────────────────────────────────────────────────────────────────────────┐
│                        TRÌNH DUYỆT (BROWSER)                           │
│                                                                        │
│   Chưa đăng nhập ───► [ Màn hình Login Bắt buộc (aaPanel style) ]      │
│                              │                                         │
│                              │ POST /api/auth/login (user, pass)       │
│                              ▼                                         │
│   Đã đăng nhập  ───► [ Toàn bộ Workspace Quản trị FerrousPanel ]       │
│                      ├── Topbar: Admin Avatar + Username + [Đăng xuất] │
│                      └── 12 Phân hệ Quản trị (Websites, PHP, SSH, ...) │
└────────────────────────────────────────────────┬───────────────────────┘
                                                 │
                                                 ▼
┌────────────────────────────────────────────────────────────────────────┐
│                      BACKEND SERVER (server.ts)                        │
│                                                                        │
│   ├── POST /api/auth/login   ──► So khớp với admin.json / Argon2id     │
│   ├── GET  /api/auth/session ──► Xác thực Session Token hiện tại       │
│   ├── POST /api/auth/logout  ──► Hủy Token & Xóa phiên đăng nhập       │
│   └── PUT  /api/panel-admin  ──► Đồng bộ tài khoản Admin VPS           │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Giao Diện & Trải Nghiệm Người Dùng (Login View)

### A. Màn hình Login Bắt buộc (`src/components/LoginView.tsx`)
- **Khung chứa trung tâm**: Thiết kế thanh lịch, gọn gàng trên nền tối/sáng hài hòa chuẩn công nghiệp hosting control panel (tông Slate & Orange Accent của FerrousPanel).
- **Thương hiệu & Nhận diện**:
  - Logo FerrousPanel kèm nhãn `Ubuntu Web Hosting Control Panel`.
  - Huy hiệu thông báo trạng thái an toàn: `Bảo vệ bởi Rust Native Engine & Argon2id`.
- **Form đăng nhập**:
  - Ô nhập **Tên tài khoản** (`adminUsername`).
  - Ô nhập **Mật khẩu** kèm nút con mắt **Hiện / Ẩn mật khẩu**.
  - Checkbox **Ghi nhớ đăng nhập trên thiết bị này**.
  - Nút bấm **Đăng nhập vào Panel** với hiệu ứng loading và phản hồi lỗi rõ ràng (sai tài khoản hoặc mật khẩu).
- **Thẻ thông tin mặc định (aaPanel Default Credentials Card)**:
  - Khung tra cứu nhanh tài khoản mặc định được sinh khi cài đặt (`admin_ferrous` / `Fp#2026!UbuntuRoot`), có nút **1-Click Tự Điền & Đăng Nhập** giúp người dùng test nhanh hoặc dễ dàng đăng nhập lần đầu.
  - Hướng dẫn lệnh khôi phục mật khẩu trên VPS: `fp 5` hoặc `fp default`.

### B. Thanh Header & Menu Sau Khi Đăng Nhập (`src/App.tsx`)
- Thêm cụm người dùng ở góc trên bên phải thanh Header:
  - Huy hiệu trạng thái **Admin** kèm tên tài khoản (`admin_ferrous`).
  - Nút **Đăng xuất (Logout)** với icon thoát, khi bấm sẽ hủy token và chuyển ngay về màn hình Login bắt buộc.

---

## 3. Backend & API Xác Thực (`server.ts`)

- Bổ sung các endpoint xác thực:
  - `POST /api/auth/login`: Nhận `{ username, password }`, kiểm tra khớp với `panelAdminState.adminUsername` và `panelAdminState.adminPassword`. Khi đúng, sinh một token phiên làm việc `fp_session_<timestamp>_<random>` và trả về cho client.
  - `GET /api/auth/session`: Xác thực header `Authorization: Bearer <token>` để kiểm tra phiên còn hiệu lực hay không.
  - `POST /api/auth/logout`: Vô hiệu hóa token phiên đăng nhập.
- Bảo vệ các thay đổi mật khẩu từ web và đồng bộ nhất quán với tệp `/etc/ferrous-panel/admin.json` và `/www/server/panel/default.pl`.

---

## 4. Các Bước Triển Khai Chi Tiết

1. **Tạo Component `src/components/LoginView.tsx`**:
   - Viết toàn diện giao diện trang Login chuẩn aaPanel/cPanel, form xác thực, thẻ tự điền tài khoản mặc định, thông báo lỗi đẹp mắt.
2. **Cập nhật `server.ts`**:
   - Thêm bộ lưu trữ phiên (session store in-memory).
   - Thêm các route `/api/auth/login`, `/api/auth/session`, `/api/auth/logout`.
3. **Cập nhật `src/App.tsx`**:
   - Quản lý trạng thái `isAuthenticated`, `currentUser`, `sessionToken`.
   - Nếu chưa đăng nhập, bắt buộc render `LoginView` (chặn toàn bộ quyền truy cập vào các tab điều khiển).
   - Khi đã đăng nhập, hiển thị thông tin tài khoản và nút Đăng xuất trên thanh Header.
4. **Kiểm tra & Biên dịch**:
   - Chạy đồng thời `compile_applet` và `lint_applet` để đảm bảo 0 lỗi TypeScript và cú pháp.
   - Kiểm tra đăng nhập thành công, đăng nhập sai, tự điền tài khoản, và chức năng đăng xuất.
