#!/usr/bin/env bash
# ==============================================================================
# Ferrous-panel Internal VPS Terminal CLI Toolbox (`fp` / `ferrous`)
# Equivalent to `bt` in aaPanel — Repairs, configures, and manages Ferrous-panel
# directly from the Ubuntu VPS SSH Terminal.
# Installed at: /usr/local/bin/fp and /usr/local/bin/ferrous
# ==============================================================================
set -euo pipefail

ADMIN_CONFIG="/etc/ferrous-panel/admin.json"
DEFAULT_PL="/www/server/panel/default.pl"
INSTALL_DIR="/opt/Ferrous-panel"

ensure_admin_config() {
  sudo mkdir -p /etc/ferrous-panel /www/server/panel/vhost/nginx /etc/nginx/ssl
  if [ ! -f "$ADMIN_CONFIG" ]; then
    sudo tee "$ADMIN_CONFIG" > /dev/null <<EOF
{
  "adminUsername": "admin_ferrous",
  "adminPassword": "Fp#2026!UbuntuRoot",
  "panelPort": 3000,
  "securityEntrance": "/ferrous_admin_88a",
  "mysqlRootPassword": "Sql#2026!MariaDbRoot"
}
EOF
    sudo chmod 600 "$ADMIN_CONFIG"
  fi
}

read_json_field() {
  local field="$1"
  local fallback="$2"
  if command -v jq &>/dev/null && [ -f "$ADMIN_CONFIG" ]; then
    jq -r ".${field} // \"${fallback}\"" "$ADMIN_CONFIG"
  else
    echo "$fallback"
  fi
}

write_json_field() {
  local field="$1"
  local value="$2"
  ensure_admin_config
  if command -v jq &>/dev/null; then
    local tmp
    tmp="$(mktemp)"
    sudo jq ".${field} = ${value}" "$ADMIN_CONFIG" > "$tmp"
    sudo mv "$tmp" "$ADMIN_CONFIG"
    sudo chmod 600 "$ADMIN_CONFIG"
  fi
}

show_default_info() {
  ensure_admin_config
  local ip user pass port entrance sqlpass
  ip="$(hostname -I 2>/dev/null | awk '{print $1}' || echo '127.0.0.1')"
  user="$(read_json_field adminUsername admin_ferrous)"
  pass="$(read_json_field adminPassword Fp#2026!UbuntuRoot)"
  port="$(read_json_field panelPort 3000)"
  entrance="$(read_json_field securityEntrance /ferrous_admin_88a)"
  sqlpass="$(read_json_field mysqlRootPassword Sql#2026!MariaDbRoot)"

  echo "======================================================================"
  echo "  FERROUS-PANEL — THÔNG TIN TÀI KHOẢN ADMIN & RUST CORE ENGINE"
  echo "======================================================================"
  echo "  Tiến trình Core Server  : /usr/local/bin/ferrous-panel-daemon (Rust Actix-Web)"
  echo "  URL Truy cập Ngoại mạng : http://${ip}:${port}${entrance}"
  echo "  URL Truy cập Nội bộ     : http://127.0.0.1:${port}${entrance}"
  echo "  URL phpMyAdmin 5.2      : http://${ip}:888/phpmyadmin_ferrous_89a2"
  echo "  Tài khoản Admin (User)  : ${user}"
  echo "  Mật khẩu Admin (Pass)   : ${pass}"
  echo "  Mật khẩu SQL Root       : ${sqlpass}"
  echo "  Tệp cấu hình Admin      : ${ADMIN_CONFIG}"
  echo "  Thư mục Web Root        : /www/wwwroot"
  echo "  Thư mục Nginx Vhosts    : /www/server/panel/vhost/nginx"
  echo "======================================================================"
}

repair_panel_environment() {
  echo "[1/5] Kiểm tra cấu trúc thư mục chuẩn (/www/wwwroot, /www/server/panel/vhost/nginx)..."
  sudo mkdir -p /www/wwwroot /www/wwwlogs /www/backup /www/server/panel/vhost/nginx /www/server/panel/vhost/htpasswd /var/www/letsencrypt /etc/nginx/ssl
  sudo chown -R www-data:www-data /www/wwwroot /www/wwwlogs /var/www/letsencrypt || true

  echo "[2/5] Kiểm tra chứng chỉ SSL Fallback (/etc/nginx/ssl/ferrous-fallback.crt)..."
  if [ ! -f /etc/nginx/ssl/ferrous-fallback.crt ]; then
    sudo openssl req -x509 -nodes -days 3650 -newkey rsa:2048 \
      -keyout /etc/nginx/ssl/ferrous-fallback.key \
      -out /etc/nginx/ssl/ferrous-fallback.crt \
      -subj "/C=VN/ST=Hanoi/L=Hanoi/O=FerrousPanel/CN=localhost"
  fi

  echo "[3/5] Đồng bộ chỉ thị nạp Vhost vào /etc/nginx/conf.d/ferrous-panel-vhosts.conf..."
  sudo tee /etc/nginx/conf.d/ferrous-panel-vhosts.conf > /dev/null <<'EOF'
include /www/server/panel/vhost/nginx/*.conf;
EOF

  echo "[4/5] Kiểm tra cú pháp Nginx (nginx -t) & khởi động lại PHP-FPM / MariaDB..."
  sudo nginx -t && sudo systemctl reload nginx || true
  sudo systemctl restart php8.3-fpm mariadb postgresql 2>/dev/null || true

  echo "[5/5] Khởi động lại dịch vụ Ferrous-panel..."
  sudo systemctl daemon-reload
  sudo systemctl restart ferrous-panel-web.service 2>/dev/null || true
  echo "✅ Đã sửa chữa và đồng bộ hoàn tất môi trường chạy Web trên VPS!"
}

run_option() {
  local opt="$1"
  case "$opt" in
    1|restart)
      echo "🔄 Đang khởi động lại dịch vụ Ferrous-panel..."
      sudo systemctl restart ferrous-panel-web.service || true
      echo "✅ Đã khởi động lại Ferrous-panel thành công."
      ;;
    2|stop)
      echo "⏹️ Đang dừng dịch vụ Ferrous-panel..."
      sudo systemctl stop ferrous-panel-web.service || true
      echo "✅ Đã dừng dịch vụ Ferrous-panel."
      ;;
    3|start)
      echo "▶️ Đang khởi động dịch vụ Ferrous-panel..."
      sudo systemctl start ferrous-panel-web.service || true
      echo "✅ Đã khởi động Ferrous-panel."
      ;;
    4|reload)
      echo "♻️ Đang nạp lại cấu hình Panel & Nginx..."
      sudo nginx -t && sudo systemctl reload nginx || true
      sudo systemctl reload-or-restart ferrous-panel-web.service || true
      echo "✅ Đã reload Nginx & Ferrous-panel không gián đoạn."
      ;;
    5|passwd)
      local new_pass="${2:-}"
      if [ -z "$new_pass" ]; then
        read -rp "Nhập mật khẩu Admin mới cho Panel: " new_pass
      fi
      write_json_field "adminPassword" "\"${new_pass}\""
      echo "✅ Đã cập nhật mật khẩu Admin Panel thành: ${new_pass}"
      ;;
    6|username)
      local new_user="${2:-}"
      if [ -z "$new_user" ]; then
        read -rp "Nhập tên tài khoản Admin mới: " new_user
      fi
      write_json_field "adminUsername" "\"${new_user}\""
      echo "✅ Đã đổi tên đăng nhập Admin Panel thành: ${new_user}"
      ;;
    7|port)
      local new_port="${2:-}"
      if [ -z "$new_port" ]; then
        read -rp "Nhập cổng (Port) mới cho Ferrous-panel (1024-65535): " new_port
      fi
      write_json_field "panelPort" "${new_port}"
      sudo ufw allow "${new_port}/tcp" comment 'Ferrous-panel Custom Port' || true
      echo "✅ Đã đổi cổng Panel sang ${new_port} và mở tường lửa UFW."
      ;;
    8|entrance)
      local new_ent="${2:-}"
      if [ -z "$new_ent" ]; then
        read -rp "Nhập đường dẫn Security Entrance mới (VD: /admin_88a): " new_ent
      fi
      [[ "$new_ent" != /* ]] && new_ent="/${new_ent}"
      write_json_field "securityEntrance" "\"${new_ent}\""
      echo "✅ Đã cập nhật đường dẫn bảo mật thành: ${new_ent}"
      ;;
    9|clear-entrance)
      write_json_field "securityEntrance" "\"/\""
      echo "✅ Đã tắt khóa đường dẫn Security Entrance."
      ;;
    10|sql-passwd)
      local new_sql="${2:-}"
      if [ -z "$new_sql" ]; then
        read -rp "Nhập mật khẩu Root mới cho MySQL/MariaDB & PostgreSQL: " new_sql
      fi
      sudo mysql -e "ALTER USER 'root'@'localhost' IDENTIFIED BY '${new_sql}'; FLUSH PRIVILEGES;" 2>/dev/null || true
      sudo -u postgres psql -c "ALTER USER postgres WITH PASSWORD '${new_sql}';" 2>/dev/null || true
      write_json_field "mysqlRootPassword" "\"${new_sql}\""
      echo "✅ Đã đặt lại mật khẩu Root SQL (MariaDB & PostgreSQL) thành: ${new_sql}"
      ;;
    11|repair)
      repair_panel_environment
      ;;
    12|clean)
      echo "🧹 Đang dọn dẹp PageCache, nhật ký tạm và giải phóng RAM..."
      sync && echo 1 | sudo tee /proc/sys/vm/drop_caches > /dev/null || true
      sudo journalctl --vacuum-time=3d 2>/dev/null || true
      echo "✅ Đã dọn dẹp bộ nhớ đệm và nhật ký hệ thống."
      ;;
    13|unban-admin)
      echo "🔓 Đang gỡ bỏ giới hạn IP Admin và đặt lại trạng thái khóa đăng nhập..."
      sudo fail2ban-client unban --all 2>/dev/null || true
      echo "✅ Đã mở khóa truy cập Admin thành công."
      ;;
    14|default)
      show_default_info
      ;;
    15|check)
      echo "🔍 Kiểm tra trạng thái hệ thống VPS:"
      sudo nginx -t || true
      systemctl is-active nginx mariadb postgresql php8.3-fpm ferrous-panel-web.service || true
      ss -tulpn | grep -E ':(80|443|3000|3306|5432|9443)\b' || true
      ;;
    16|logs)
      sudo journalctl -u ferrous-panel-web.service -n 30 --no-pager || true
      ;;
    17|domain)
      local new_domain="${2:-}"
      if [ -z "$new_domain" ]; then
        read -rp "Nhập tên miền muốn gán cho Web Panel (VD: panel.domain.com): " new_domain
      fi
      new_domain="$(echo "${new_domain}" | tr '[:upper:]' '[:lower:]' | tr -d ' ')"
      if [ -z "$new_domain" ]; then
        echo "❌ Tên miền không hợp lệ."
        return 1
      fi
      local port
      port="$(read_json_field panelPort 3000)"
      write_json_field "customDomain" "\"${new_domain}\""
      
      # Configure Nginx Reverse Proxy for Custom Domain
      sudo mkdir -p /etc/nginx/conf.d
      sudo tee "/etc/nginx/conf.d/ferrous-panel-domain.conf" > /dev/null <<EOF
# Ferrous-panel Custom Domain Reverse Proxy
server {
    listen 80;
    server_name ${new_domain};

    location / {
        proxy_pass http://127.0.0.1:${port};
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }
}
EOF
      sudo nginx -t && sudo systemctl reload nginx || true
      echo "======================================================================"
      echo "✅ ĐÃ GÁN TÊN MIỀN CHO WEB PANEL THÀNH CÔNG: ${new_domain}"
      echo "----------------------------------------------------------------------"
      echo "🌐 Tên miền Panel     : ${new_domain}"
      echo "⚙️ Cấu hình Nginx     : /etc/nginx/conf.d/ferrous-panel-domain.conf"
      echo "👉 URL Đăng nhập Web  : http://${new_domain}$(read_json_field securityEntrance /ferrous_admin_88a)"
      echo "💡 Lưu ý: Hãy trỏ bản ghi DNS loại A của ${new_domain} về IP của VPS."
      echo "======================================================================"
      ;;
    18|clear-domain)
      write_json_field "customDomain" "\"\""
      sudo rm -f "/etc/nginx/conf.d/ferrous-panel-domain.conf"
      sudo nginx -t && sudo systemctl reload nginx || true
      echo "✅ Đã xóa bỏ tên miền riêng. Hiện tại bạn có thể đăng nhập bằng IP VPS."
      ;;
    *)
      echo "Lựa chọn không hợp lệ: $opt"
      ;;
  esac
}

if [ $# -gt 0 ]; then
  run_option "$@"
  exit 0
fi

echo "======================================================================"
echo "   FERROUS-PANEL CLI TOOLBOX (Gõ 'fp' hoặc 'ferrous' trên VPS)        "
echo "======================================================================"
echo "  (1)  Khởi động lại Panel (Restart)   (10) Đổi mật khẩu Root SQL"
echo "  (2)  Dừng dịch vụ Panel (Stop)       (11) Sửa chữa toàn diện Panel & Nginx"
echo "  (3)  Khởi động Panel (Start)         (12) Dọn rác & Giải phóng RAM Cache"
echo "  (4)  Nạp lại cấu hình (Reload)       (13) Mở khóa IP Admin & Fail2ban"
echo "  (5)  Đổi mật khẩu Admin Panel        (14) Xem tài khoản Admin & URL mặc định"
echo "  (6)  Đổi tên tài khoản Admin         (15) Kiểm tra sức khỏe Nginx/PHP/SQL"
echo "  (7)  Đổi cổng truy cập Panel (Port)  (16) Xem nhật ký lỗi Panel (Logs)"
echo "  (8)  Đổi đường dẫn Security Entrance (17) Gán tên miền riêng Web Panel (Domain)"
echo "  (9)  Xóa khóa Security Entrance      (18) Xóa bỏ tên miền riêng Web Panel"
echo "  (0)  Thoát (Exit)"
echo "======================================================================"
read -rp "Nhập số thứ tự chức năng cần thực thi [0-18]: " choice
[ "$choice" = "0" ] && exit 0
run_option "$choice"
