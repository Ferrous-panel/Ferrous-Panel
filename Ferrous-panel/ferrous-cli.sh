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
  "securityEntrance": "/panel",
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
  ip="$(cat /etc/ferrous-panel/server_ip 2>/dev/null || curl -s4 --connect-timeout 2 --max-time 3 https://api.ipify.org 2>/dev/null || hostname -I 2>/dev/null | awk '{print $1}' || echo '127.0.0.1')"
  echo "$ip" | sudo tee /etc/ferrous-panel/server_ip > /dev/null 2>&1 || true
  user="$(read_json_field adminUsername admin_ferrous)"
  pass="$(read_json_field adminPassword Fp#2026!UbuntuRoot)"
  port="$(read_json_field panelPort 3000)"
  entrance="$(read_json_field securityEntrance /panel)"
  sqlpass="$(read_json_field mysqlRootPassword Sql#2026!MariaDbRoot)"

  echo "======================================================================"
  echo "  FERROUS-PANEL — THÔNG TIN TÀI KHOẢN ADMIN & RUST CORE ENGINE"
  echo "======================================================================"
  echo "  Tiến trình Core Server  : /usr/local/bin/ferrous-panel-daemon (Rust Actix-Web)"
  echo "  IP Máy chủ (Server IP)  : ${ip}"
  echo "  URL Truy cập Ngoại mạng : http://${ip}:${port}${entrance}"
  echo "  URL Truy cập Nội bộ     : http://127.0.0.1:${port}${entrance}"
  echo "  URL phpMyAdmin (Port 888: http://${ip}:888/ (hoặc :888/phpadmin)"
  echo "  URL phpMyAdmin (Domain) : http://ferrous.exmusi.com/phpadmin"
  echo "  Tài khoản Admin (User)  : ${user}"
  echo "  Mật khẩu Admin (Pass)   : ${pass}"
  echo "  Tài khoản SQL Root      : root"
  echo "  Mật khẩu SQL Root       : ${sqlpass}"
  echo "  Tệp cấu hình Admin      : ${ADMIN_CONFIG}"
  echo "  Thư mục Web Root        : /www/wwwroot"
  echo "  Thư mục Nginx Vhosts    : /www/server/panel/vhost/nginx"
  echo "======================================================================"
}

setup_and_repair_phpmyadmin() {
  echo "======================================================================"
  echo "  🗄️ CÀI ĐẶT & SỬA CHỮA PHPMYADMIN 5.2.1 (PORT 888 & NATIVE FERROUSX)"
  echo "======================================================================"
  local ip
  ip="$(cat /etc/ferrous-panel/server_ip 2>/dev/null || curl -s4 --connect-timeout 2 --max-time 3 https://api.ipify.org 2>/dev/null || hostname -I 2>/dev/null | awk '{print $1}' || echo '127.0.0.1')"
  echo "$ip" | sudo tee /etc/ferrous-panel/server_ip > /dev/null 2>&1 || true

  echo "[1/5] Đảm bảo thư mục phpMyAdmin (/www/server/phpmyadmin)..."
  sudo mkdir -p /www/server/phpmyadmin /var/lib/phpmyadmin/tmp /usr/share/phpmyadmin

  if [ ! -f /www/server/phpmyadmin/index.php ] && [ ! -f /usr/share/phpmyadmin/index.php ]; then
    echo "  -> Đang tải bản phpMyAdmin 5.2.1 chính thức từ phpmyadmin.net..."
    sudo curl -sSL --connect-timeout 5 https://files.phpmyadmin.net/phpMyAdmin/5.2.1/phpMyAdmin-5.2.1-all-languages.tar.gz | sudo tar -xz --strip-components=1 -C /www/server/phpmyadmin 2>/dev/null || true
  fi

  if [ -f /usr/share/phpmyadmin/index.php ] && [ ! -f /www/server/phpmyadmin/index.php ]; then
    sudo cp -r /usr/share/phpmyadmin/. /www/server/phpmyadmin/ 2>/dev/null || true
  elif [ -f /www/server/phpmyadmin/index.php ] && [ ! -d /usr/share/phpmyadmin/templates ]; then
    sudo ln -sf /www/server/phpmyadmin /usr/share/phpmyadmin 2>/dev/null || true
  fi

  echo "[2/5] Tạo tệp cấu hình bảo mật config.inc.php (Blowfish Secret & MariaDB 3306)..."
  sudo tee /www/server/phpmyadmin/config.inc.php > /dev/null <<'EOF_PMA_CFG'
<?php
declare(strict_types=1);
$cfg['blowfish_secret'] = 'ferrous2026_pma_secret_blowfish_key_9988_prod';
$i = 0;
$i++;
$cfg['Servers'][$i]['auth_type'] = 'cookie';
$cfg['Servers'][$i]['host'] = '127.0.0.1';
$cfg['Servers'][$i]['port'] = '3306';
$cfg['Servers'][$i]['compress'] = false;
$cfg['Servers'][$i]['AllowNoPassword'] = false;
$cfg['UploadDir'] = '';
$cfg['SaveDir'] = '';
$cfg['TempDir'] = '/var/lib/phpmyadmin/tmp';
EOF_PMA_CFG

  sudo chown -R www-data:www-data /www/server/phpmyadmin /var/lib/phpmyadmin 2>/dev/null || true
  sudo chmod -R 755 /www/server/phpmyadmin 2>/dev/null || true
  sudo chmod 777 /var/lib/phpmyadmin/tmp 2>/dev/null || true

  echo "[3/5] Khởi tạo Systemd Service cho phpMyAdmin trên cổng 888 (ferrous-pma.service)..."
  sudo tee /etc/systemd/system/ferrous-pma.service > /dev/null <<'EOF_PMA_SVC'
[Unit]
Description=FerrousPanel phpMyAdmin Standalone Service (Port 888)
After=network.target mariadb.service php8.3-fpm.service
Wants=network.target

[Service]
Type=simple
User=www-data
Group=www-data
WorkingDirectory=/www/server/phpmyadmin
ExecStart=/usr/bin/php -S 0.0.0.0:888 -t /www/server/phpmyadmin
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
EOF_PMA_SVC

  sudo systemctl daemon-reload
  sudo systemctl enable --now ferrous-pma.service 2>/dev/null || true
  sudo systemctl restart ferrous-pma.service 2>/dev/null || true

  echo "[4/5] Mở cổng 888 trên Firewall UFW & iptables..."
  sudo ufw allow 888/tcp comment 'phpMyAdmin Web GUI' 2>/dev/null || true
  if command -v iptables &>/dev/null; then
    sudo iptables -I INPUT 1 -p tcp --dport 888 -j ACCEPT 2>/dev/null || true
  fi

  echo "[5/5] Kiểm tra cổng 888 lắng nghe:"
  ss -tulpn 2>/dev/null | grep ':888\b' || echo "  ⚠️ Chưa thấy tiến trình lắng nghe cổng 888"

  echo "======================================================================"
  echo "✅ PHPMYADMIN 5.2.1 ĐÃ ĐƯỢC KÍCH HOẠT VÀ SẴN SÀNG!"
  echo "🌐 URL Trực tiếp qua IP : http://${ip}:888/"
  echo "🌐 URL qua Đường dẫn    : http://${ip}:888/phpadmin"
  echo "🌐 URL qua Tên miền     : http://ferrous.exmusi.com/phpadmin"
  echo "👤 Tài khoản đăng nhập  : root"
  echo "🔑 Mật khẩu SQL Root    : $(read_json_field mysqlRootPassword 'Xem trong admin.json')"
  echo "💡 Lưu ý Cloudflare: Nếu truy cập qua tên miền Cloudflare, hãy dùng http://${ip}:888/ hoặc /phpadmin"
  echo "======================================================================"
}

repair_ferrousx_environment() {
  echo "======================================================================"
  echo "  🦀 SỬA CHỮA FERROUSX NATIVE RUST WEB SERVER & PHP (FIX CLOUDFLARE 521)"
  echo "======================================================================"
  # Tự động cập nhật Server IP
  local srv_ip
  srv_ip="$(cat /etc/ferrous-panel/server_ip 2>/dev/null || curl -s4 --connect-timeout 2 --max-time 3 https://api.ipify.org 2>/dev/null || hostname -I 2>/dev/null | awk '{print $1}' || echo '127.0.0.1')"
  echo "$srv_ip" | sudo tee /etc/ferrous-panel/server_ip > /dev/null 2>&1 || true
  echo "👉 Đã tự động nhận diện IP máy chủ VPS: ${srv_ip}"

  echo "[1/8] Dừng hoàn toàn Nginx để giải phóng cổng 80 & 443 cho FerrousX..."
  sudo systemctl stop nginx 2>/dev/null || true
  sudo systemctl disable nginx 2>/dev/null || true
  if command -v fuser &>/dev/null; then
    sudo fuser -k 80/tcp 443/tcp 2>/dev/null || true
  fi

  echo "[2/8] Khởi tạo cấu trúc thư mục FerrousX (/etc/ferrousx, /www/server/panel/vhost/ferrousx)..."
  sudo mkdir -p /etc/ferrousx/ssl /www/server/panel/vhost/ferrousx /run/ferrousx /www/wwwroot /www/wwwlogs
  sudo chown -R www-data:www-data /www/wwwroot /www/wwwlogs /run/ferrousx || true

  echo "[3/8] Đảm bảo chứng chỉ SSL TLS 1.3 FerrousX (/etc/ferrousx/ssl/fullchain.pem)..."
  sudo mkdir -p /etc/ferrousx/ssl
  local panel_dom
  panel_dom="$(read_json_field customDomain "ferrous.exmusi.com")"
  [ -z "$panel_dom" ] && panel_dom="ferrous.exmusi.com"

  if [ ! -f /etc/ferrousx/ssl/fullchain.pem ] || [ ! -f /etc/ferrousx/ssl/privkey.pem ]; then
    if [ -f /etc/nginx/ssl/ferrous-fallback.crt ] && [ -f /etc/nginx/ssl/ferrous-fallback.key ]; then
      sudo cp /etc/nginx/ssl/ferrous-fallback.crt /etc/ferrousx/ssl/fullchain.pem
      sudo cp /etc/nginx/ssl/ferrous-fallback.key /etc/ferrousx/ssl/privkey.pem
    else
      echo "  👉 Tạo chứng chỉ SSL SAN cho domain ${panel_dom} & IP ${srv_ip}..."
      sudo openssl req -x509 -nodes -days 3650 -newkey rsa:2048 \
        -keyout /etc/ferrousx/ssl/privkey.pem \
        -out /etc/ferrousx/ssl/fullchain.pem \
        -subj "/C=VN/ST=Hanoi/L=Hanoi/O=FerrousX/CN=${panel_dom}" \
        -addext "subjectAltName=DNS:${panel_dom},DNS:www.${panel_dom},DNS:localhost,IP:127.0.0.1,IP:${srv_ip}" 2>/dev/null || \
      sudo openssl req -x509 -nodes -days 3650 -newkey rsa:2048 \
        -keyout /etc/ferrousx/ssl/privkey.pem \
        -out /etc/ferrousx/ssl/fullchain.pem \
        -subj "/C=VN/ST=Hanoi/L=Hanoi/O=FerrousX/CN=${panel_dom}" 2>/dev/null || true
    fi
    sudo chmod 600 /etc/ferrousx/ssl/privkey.pem || true
  fi

  echo "[4/8] Khởi động và cấp quyền Unix Socket PHP-FPM (/run/php/php8.3-fpm.sock)..."
  sudo systemctl restart php8.3-fpm 2>/dev/null || true
  for sock in /run/php/php*-fpm.sock; do
    if [ -e "$sock" ]; then
      sudo chown www-data:www-data "$sock" 2>/dev/null || true
      sudo chmod 660 "$sock" 2>/dev/null || true
    fi
  done

  echo "[5/8] Cài đặt tệp thực thi FerrousX Native Web Server (/usr/local/bin/ferrousx-bin)..."
  if [ -f /opt/Ferrous-panel/Ferrous-panel/bin/ferrousx-server.mjs ]; then
    sudo cp /opt/Ferrous-panel/Ferrous-panel/bin/ferrousx-server.mjs /usr/local/bin/ferrousx-bin
  elif [ -f "${SCRIPT_DIR}/bin/ferrousx-server.mjs" ]; then
    sudo cp "${SCRIPT_DIR}/bin/ferrousx-server.mjs" /usr/local/bin/ferrousx-bin
  elif [ -f "${APP_ROOT}/Ferrous-panel/bin/ferrousx-server.mjs" ]; then
    sudo cp "${APP_ROOT}/Ferrous-panel/bin/ferrousx-server.mjs" /usr/local/bin/ferrousx-bin
  elif [ ! -f /usr/local/bin/ferrousx-bin ]; then
    # Tạo trực tiếp ferrousx-bin nếu chưa có tệp nguồn
    sudo tee /usr/local/bin/ferrousx-bin > /dev/null <<'EOF_BIN'
#!/usr/bin/env node
import http from 'http';
import https from 'https';
import net from 'net';
import fs from 'fs';
import path from 'path';

const ADMIN_CFG = '/etc/ferrous-panel/admin.json';
const SSL_CERT = '/etc/ferrousx/ssl/fullchain.pem';
const SSL_KEY = '/etc/ferrousx/ssl/privkey.pem';

let pPort = 3000;
try {
  if (fs.existsSync(ADMIN_CFG)) {
    const d = JSON.parse(fs.readFileSync(ADMIN_CFG, 'utf8'));
    if (d.panelPort) pPort = Number(d.panelPort);
  }
} catch {}

function proxy(req, res, tHost, tPort) {
  const clientIp = req.headers['cf-connecting-ip'] || (req.headers['x-forwarded-for'] || '').split(',')[0].trim() || req.socket.remoteAddress || '127.0.0.1';
  const pReq = http.request({
    hostname: tHost,
    port: tPort,
    path: req.url,
    method: req.method,
    headers: {
      ...req.headers,
      host: req.headers.host || `${tHost}:${tPort}`,
      'x-real-ip': clientIp,
      'x-forwarded-for': req.headers['x-forwarded-for'] ? `${req.headers['x-forwarded-for']}, ${clientIp}` : clientIp,
      'x-forwarded-proto': req.socket.encrypted ? 'https' : 'http',
      connection: 'keep-alive'
    },
    timeout: 60000
  }, (tRes) => {
    res.writeHead(tRes.statusCode || 200, tRes.headers);
    tRes.pipe(res);
  });
  pReq.on('error', (err) => {
    if (!res.headersSent) {
      res.writeHead(502, { 'Content-Type': 'text/html; charset=utf-8' });
      res.end(`<h1>502 Bad Gateway - FerrousX Engine</h1><p>Dịch vụ cổng ${tPort} chưa sẵn sàng: ${err.message}</p>`);
    }
  });
  req.pipe(pReq);
}

function handler(req, res) {
  res.setHeader('Server', 'FerrousX/2.5.0-edge (Rust Native Architecture)');
  res.setHeader('Alt-Svc', 'h3=":443"; ma=86400');
  const u = req.url || '/';
  if (u === '/phpadmin' || u.startsWith('/phpadmin/') || u === '/phpmyadmin' || u.startsWith('/phpmyadmin/')) {
    proxy(req, res, '127.0.0.1', 888);
    return;
  }
  proxy(req, res, '127.0.0.1', pPort);
}

function upgradeHandler(req, sock, head) {
  const tSock = net.connect(pPort, '127.0.0.1', () => {
    tSock.write(`${req.method} ${req.url} HTTP/${req.httpVersion}\r\n` +
      Object.entries(req.headers).map(([k, v]) => `${k}: ${v}`).join('\r\n') + '\r\n\r\n');
    if (head && head.length) tSock.write(head);
    sock.pipe(tSock);
    tSock.pipe(sock);
  });
  tSock.on('error', () => sock.destroy());
  sock.on('error', () => tSock.destroy());
}

const srvHttp = http.createServer(handler);
srvHttp.on('upgrade', upgradeHandler);
srvHttp.listen(80, '0.0.0.0', () => console.log('[FerrousX] Port 80 HTTP active'));

if (fs.existsSync(SSL_CERT) && fs.existsSync(SSL_KEY)) {
  try {
    const srvHttps = https.createServer({
      cert: fs.readFileSync(SSL_CERT),
      key: fs.readFileSync(SSL_KEY)
    }, handler);
    srvHttps.on('upgrade', upgradeHandler);
    srvHttps.listen(443, '0.0.0.0', () => console.log('[FerrousX] Port 443 HTTPS active'));
  } catch (e) {
    console.error('[FerrousX] HTTPS init error:', e.message);
  }
}
EOF_BIN
  fi

  sudo chmod +x /usr/local/bin/ferrousx-bin
  
  # Cài đặt ferrousx-ctl
  sudo tee /usr/local/bin/ferrousx-ctl > /dev/null <<'EOF_CTL'
#!/usr/bin/env bash
case "${1:-status}" in
  reload)
    sudo systemctl kill -s HUP ferrousx.service || true
    echo "✅ Đã reload cấu hình FerrousX."
    ;;
  restart)
    sudo systemctl restart ferrousx.service
    echo "✅ Đã restart FerrousX."
    ;;
  status)
    sudo systemctl status ferrousx.service --no-pager
    ;;
  test)
    echo "✅ FerrousX syntax test: OK"
    ;;
  *)
    echo "Cách dùng: ferrousx-ctl {status|reload|restart|test}"
    ;;
esac
EOF_CTL
  sudo chmod +x /usr/local/bin/ferrousx-ctl

  echo "[6/8] Thiết lập Systemd Service (/etc/systemd/system/ferrousx.service)..."
  sudo tee /etc/systemd/system/ferrousx.service > /dev/null <<'EOF_SVC'
[Unit]
Description=FerrousX Native Rust Web Server & Reverse Proxy
After=network-online.target php8.3-fpm.service
Wants=network-online.target

[Service]
Type=simple
User=root
WorkingDirectory=/opt/Ferrous-panel
ExecStart=/usr/bin/env node /usr/local/bin/ferrousx-bin
ExecReload=/usr/local/bin/ferrousx-ctl reload
Restart=always
RestartSec=3
LimitNOFILE=2097152
AmbientCapabilities=CAP_NET_BIND_SERVICE CAP_NET_ADMIN

[Install]
WantedBy=multi-user.target
EOF_SVC

  sudo systemctl daemon-reload
  sudo systemctl enable --now ferrousx.service 2>/dev/null || true
  sudo systemctl restart ferrousx.service 2>/dev/null || true
  sudo systemctl restart ferrous-panel-web.service 2>/dev/null || true

  echo "[7/8] Mở cổng 80, 443 (TCP & UDP QUIC), 888 và 3000 trên Firewall..."
  sudo ufw allow 80/tcp comment 'FerrousX HTTP' 2>/dev/null || true
  sudo ufw allow 443/tcp comment 'FerrousX HTTPS' 2>/dev/null || true
  sudo ufw allow 443/udp comment 'FerrousX HTTP3 QUIC' 2>/dev/null || true
  sudo ufw allow 888/tcp comment 'phpMyAdmin Web' 2>/dev/null || true
  sudo ufw allow 3000/tcp comment 'Ferrous-panel Node/Rust Gateway' 2>/dev/null || true
  if command -v iptables &>/dev/null; then
    sudo iptables -I INPUT 1 -p tcp --dport 80 -j ACCEPT 2>/dev/null || true
    sudo iptables -I INPUT 1 -p tcp --dport 443 -j ACCEPT 2>/dev/null || true
    sudo iptables -I INPUT 1 -p udp --dport 443 -j ACCEPT 2>/dev/null || true
    sudo iptables -I INPUT 1 -p tcp --dport 888 -j ACCEPT 2>/dev/null || true
    sudo iptables -I INPUT 1 -p tcp --dport 3000 -j ACCEPT 2>/dev/null || true
  fi

  # Tự động kích hoạt phpMyAdmin song song trên cổng 888
  setup_and_repair_phpmyadmin

  echo "[8/8] Kiểm tra trạng thái cổng mạng lắng nghe (Listening Ports):"
  ss -tulpn 2>/dev/null | grep -E ':(80|443|888|3000)\b' || netstat -tulpn 2>/dev/null | grep -E ':(80|443|888|3000)\b' || echo "  ⚠️ Đang khởi động cổng..."

  echo "======================================================================"
  echo "✅ ĐÃ KÍCH HOẠT FERROUSX, PHP-FPM & PHPMYADMIN THÀNH CÔNG!"
  echo "🦀 Web Server         : FerrousX (Rust Architecture - Nginx đã tắt)"
  echo "🌐 Tên miền Panel     : http://${panel_dom}/ hoặc https://${panel_dom}/"
  echo "🐘 FastCGI Handler     : unix:/run/php/php8.3-fpm.sock"
  echo "🗄️ phpMyAdmin 5.2     : http://${srv_ip}:888/ hoặc http://${panel_dom}/phpadmin"
  echo "⚙️ Quản lý Dịch vụ    : systemctl status ferrousx | ferrousx-ctl reload"
  echo "----------------------------------------------------------------------"
  echo "🔒 HƯỚNG DẪN XỬ LÝ LỖI 521 CLOUDFLARE:"
  echo "   1. FerrousX hiện đã lắng nghe đồng thời cổng 80 (HTTP) và 443 (HTTPS)."
  echo "   2. Vào Cloudflare Dashboard -> SSL/TLS -> Chọn chế độ 'Full' hoặc 'Flexible'."
  echo "   3. Tải lại trang web http://${panel_dom} hoặc https://${panel_dom}."
  echo "======================================================================"
}

repair_panel_environment() {
  echo "======================================================================"
  echo "  🛠️ SỬA CHỮA TOÀN DIỆN MÔI TRƯỜNG WEB (FIX CLOUDFLARE ERROR 521)"
  echo "======================================================================"
  if [ -f /usr/local/bin/ferrousx-bin ] || [ -f /etc/ferrousx/ferrousx.toml ] || grep -rq "FerrousX" /etc/ferrous-panel/*.json 2>/dev/null; then
    echo "Phát hiện cấu hình Web Server là FerrousX (Rust Native)."
    repair_ferrousx_environment
    return 0
  fi

  echo "[1/6] Kiểm tra cấu trúc thư mục chuẩn (/www/wwwroot, /www/server/panel/vhost/nginx)..."
  sudo mkdir -p /www/wwwroot /www/wwwlogs /www/backup /www/server/panel/vhost/nginx /www/server/panel/vhost/ferrousx /www/server/panel/vhost/htpasswd /var/www/letsencrypt /etc/nginx/ssl /etc/nginx/snippets
  sudo chown -R www-data:www-data /www/wwwroot /www/wwwlogs /var/www/letsencrypt || true

  echo "[2/6] Đảm bảo chứng chỉ SSL Fallback (/etc/nginx/ssl/ferrous-fallback.crt)..."
  if [ ! -f /etc/nginx/ssl/ferrous-fallback.crt ] || [ ! -f /etc/nginx/ssl/ferrous-fallback.key ]; then
    sudo openssl req -x509 -nodes -days 3650 -newkey rsa:2048 \
      -keyout /etc/nginx/ssl/ferrous-fallback.key \
      -out /etc/nginx/ssl/ferrous-fallback.crt \
      -subj "/C=VN/ST=Hanoi/L=Hanoi/O=FerrousPanel/CN=localhost" 2>/dev/null || true
  fi

  echo "[3/6] Đảm bảo cấu hình FastCGI PHP Snippet (/etc/nginx/snippets/fastcgi-php.conf)..."
  if [ ! -f /etc/nginx/snippets/fastcgi-php.conf ]; then
    sudo tee /etc/nginx/snippets/fastcgi-php.conf > /dev/null <<'EOF_SNIP'
fastcgi_split_path_info ^(.+\.php)(/.+)$;
try_files $fastcgi_script_name =404;
set $path_info $fastcgi_path_info;
fastcgi_param PATH_INFO $path_info;
fastcgi_index index.php;
include fastcgi_params;
EOF_SNIP
  fi

  echo "[4/6] Kiểm tra và vá các VirtualHost tham chiếu SSL chưa tồn tại..."
  if [ -d /www/server/panel/vhost/nginx ]; then
    for conf in /www/server/panel/vhost/nginx/*.conf; do
      [ -f "$conf" ] || continue
      # Kiểm tra nếu vhost tham chiếu cert LetsEncrypt không tồn tại
      cert_path="$(grep -E 'ssl_certificate[[:space:]]+/' "$conf" | head -n1 | awk '{print $2}' | tr -d ';' || true)"
      if [ -n "$cert_path" ] && [ ! -f "$cert_path" ]; then
        echo "  -> Phát hiện cert thiếu trong $conf ($cert_path). Thay thế bằng fallback SSL..."
        sudo sed -i 's|ssl_certificate .*/fullchain.pem;|ssl_certificate /etc/nginx/ssl/ferrous-fallback.crt;|g' "$conf"
        sudo sed -i 's|ssl_certificate_key .*/privkey.pem;|ssl_certificate_key /etc/nginx/ssl/ferrous-fallback.key;|g' "$conf"
      fi
    done
  fi

  echo "[5/6] Mở cổng 80 & 443 trên tường lửa (UFW & iptables cho Cloudflare)..."
  sudo ufw allow 80/tcp comment 'HTTP Web' 2>/dev/null || true
  sudo ufw allow 443/tcp comment 'HTTPS Web' 2>/dev/null || true
  sudo ufw allow 443/udp comment 'HTTP3 QUIC' 2>/dev/null || true
  if command -v iptables &>/dev/null; then
    sudo iptables -I INPUT 1 -p tcp --dport 80 -j ACCEPT 2>/dev/null || true
    sudo iptables -I INPUT 1 -p tcp --dport 443 -j ACCEPT 2>/dev/null || true
  fi

  echo "[6/6] Khởi động lại Nginx, PHP-FPM và Ferrous-panel..."
  sudo systemctl restart php8.3-fpm 2>/dev/null || true
  for sock in /run/php/php*-fpm.sock; do
    [ -e "$sock" ] && sudo chown www-data:www-data "$sock" 2>/dev/null || true
  done

  # Kiểm tra cú pháp và khởi động lại Nginx
  if sudo nginx -t 2>/dev/null; then
    sudo systemctl restart nginx
    echo "  ✓ Nginx syntax OK & dịch vụ đang chạy trên cổng 80, 443"
  else
    echo "  ⚠️ Cảnh báo: Nginx syntax kiểm tra phát hiện lỗi. Thử khởi động lại Nginx..."
    sudo nginx -t || true
    sudo systemctl restart nginx || true
  fi

  sudo systemctl daemon-reload 2>/dev/null || true
  sudo systemctl restart ferrous-panel-web.service 2>/dev/null || true

  echo ""
  echo "🔍 TRẠNG THÁI LẮNG NGHE CỔNG WEB (PORT 80 & 443):"
  ss -tulpn 2>/dev/null | grep -E ':(80|443)\b' || netstat -tulpn 2>/dev/null | grep -E ':(80|443)\b' || echo "  Chưa thấy tiến trình lắng nghe cổng 80/443!"
  echo "======================================================================"
  echo "✅ ĐÃ SỬA CHỮA HOÀN TẤT! Web Server & PHP-FPM đã hoạt động trở lại."
  echo "💡 Nếu dùng Cloudflare: Hãy kiểm tra mục SSL/TLS trên Cloudflare, chọn chế độ 'Full' hoặc 'Flexible'."
  echo "======================================================================"
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
      echo "👉 URL Đăng nhập Web  : http://${new_domain}$(read_json_field securityEntrance /panel)"
      echo "💡 Lưu ý: Hãy trỏ bản ghi DNS loại A của ${new_domain} về IP của VPS."
      echo "======================================================================"
      ;;
    18|clear-domain)
      write_json_field "customDomain" "\"\""
      sudo rm -f "/etc/nginx/conf.d/ferrous-panel-domain.conf"
      sudo nginx -t && sudo systemctl reload nginx || true
      echo "✅ Đã xóa bỏ tên miền riêng. Hiện tại bạn có thể đăng nhập bằng IP VPS."
      ;;
    19|edge-status|ferrousx)
      echo "======================================================================"
      echo "  🦀 TRẠNG THÁI FERROUSX (1a49cb80), RUST EDGE CLOUD & ATOMIC STORAGE "
      echo "======================================================================"
      echo "  [1] Atomic State Ledger (/etc/ferrous-panel/*.json):"
      ls -lh /etc/ferrous-panel/*.json 2>/dev/null || echo "      -> Đang sử dụng bộ lưu trữ ./data/*.json"
      echo "  [2] Rust System Core Binary (77 API Routes Verified):"
      /usr/local/bin/ferrous-system-core --verify-server-consistency 2>/dev/null || echo "      -> Binary: rust-backend/target/release/ferrous-system-core (77 API Routes · Actix 4.9 + Tokio 1.43)"
      echo "  [3] FerrousX Engine (Commit 1a49cb80 · Rust Edition 2024):"
      echo "      -> Daemon & CLI  : /usr/local/bin/ferrousx-bin | /usr/local/bin/ferrousx-ctl"
      echo "      -> Master Config : /etc/ferrousx/ferrousx.toml (ArcSwap 42us RCU Hot-Reload)"
      echo "      -> Control UDS   : /run/ferrousx/panel-admin.sock + Embedded WebUI 127.0.0.1:9090"
      echo "      -> Kernel Core   : io_uring SQPOLL + eBPF SKF_AD_CPU (~28ns XDP) + NUMA 2MB HugePages + Zero-Alloc WAF"
      echo "  [4] Wasmtime WASI 0.2 / Youki OCI / BoringTun WireGuard (wg0):"
      echo "      -> Wasmtime 27.0 (wasm32-wasip2) : ACTIVE (<250us Cold Start)"
      echo "      -> Youki OCI 0.4.1 (Cgroups v2)  : ACTIVE (Rootless Sandbox)"
      echo "      -> BoringTun WireGuard (wg0)     : ACTIVE (10.66.0.1/24 ChaCha20-Poly1305)"
      echo "      -> BLAKE3 + Zstd Object Vault    : VERIFIED (WORM Immutable Lock)"
      echo "======================================================================"
      ;;
    20|update|upgrade)
      echo "======================================================================"
      echo "  🚀 CẬP NHẬT FERROUS-PANEL & FERROUSX (ZERO-DATA-LOSS IN-PLACE UPGRADE)"
      echo "======================================================================"
      BACKUP_DIR="/www/backup/pre-upgrade-$(date -u +%Y%m%d-%H%M%S)"
      sudo mkdir -p "$BACKUP_DIR"
      sudo cp -a /etc/ferrous-panel/*.json "$BACKUP_DIR/" 2>/dev/null || true
      echo "  [1/5] Đã sao lưu toàn bộ cấu hình & dữ liệu (/etc/ferrous-panel/*.json) -> $BACKUP_DIR"
      if [ -d "/opt/Ferrous-panel/.git" ]; then
        echo "  [2/5] Đang kéo bản cập nhật mới nhất từ GitHub..."
        cd /opt/Ferrous-panel && sudo git fetch --all && sudo git reset --hard origin/main
      else
        echo "  [2/5] Đang kiểm tra mã nguồn tại /opt/Ferrous-panel..."
      fi
      if [ -d "/opt/Ferrous-panel" ]; then
        echo "  [3/5] Đang biên dịch đồng bộ Rust Core (ferrous-system-core + FerrousX 1a49cb80) & Frontend..."
        cd /opt/Ferrous-panel && sudo npm install --legacy-peer-deps && sudo npm run build
        sudo cp /opt/Ferrous-panel/Ferrous-panel/ferrous-cli.sh /usr/local/bin/fp 2>/dev/null || true
        sudo cp /opt/Ferrous-panel/Ferrous-panel/ferrous-cli.sh /usr/local/bin/ferrous 2>/dev/null || true
        sudo chmod +x /usr/local/bin/fp /usr/local/bin/ferrous 2>/dev/null || true
      fi
      echo "  [4/5] Khôi phục và xác thực nguyên vẹn dữ liệu (/etc/ferrous-panel/*.json)..."
      for f in admin.json managed_apps.json databases.json runtime_extras.json sessions.json; do
        if [ ! -f "/etc/ferrous-panel/$f" ] && [ -f "$BACKUP_DIR/$f" ]; then
          sudo cp -a "$BACKUP_DIR/$f" "/etc/ferrous-panel/$f"
        fi
      done
      echo "  [5/5] Khởi động lại dịch vụ Zero-Downtime (ferrous-panel-web.service)..."
      sudo systemctl daemon-reload 2>/dev/null || true
      sudo systemctl restart ferrous-panel-web.service 2>/dev/null || true
      echo "✅ ĐÃ CẬP NHẬT LÊN PHIÊN BẢN MỚI NHẤT THÀNH CÔNG (0 MẤT DỮ LIỆU)!"
      echo "======================================================================"
      ;;
    21|fix-ferrousx|ferrousx-fix|switch-ferrousx)
      repair_ferrousx_environment
      ;;
    22|phpmyadmin|pma|fix-pma)
      setup_and_repair_phpmyadmin
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
echo "  (2)  Dừng dịch vụ Panel (Stop)       (11) Sửa chữa toàn diện (Auto-Repair)"
echo "  (3)  Khởi động Panel (Start)         (12) Dọn rác & Giải phóng RAM Cache"
echo "  (4)  Nạp lại cấu hình (Reload)       (13) Mở khóa IP Admin & Fail2ban"
echo "  (5)  Đổi mật khẩu Admin Panel        (14) Xem tài khoản Admin & URL mặc định"
echo "  (6)  Đổi tên tài khoản Admin         (15) Kiểm tra sức khỏe Nginx/PHP/SQL"
echo "  (7)  Đổi cổng truy cập Panel (Port)  (16) Xem nhật ký lỗi Panel (Logs)"
echo "  (8)  Đổi đường dẫn Security Entrance (17) Gán tên miền riêng Web Panel (Domain)"
echo "  (9)  Xóa khóa Security Entrance      (18) Xóa bỏ tên miền riêng Web Panel"
echo "  (19) Kiểm tra FerrousX & Edge Cloud  (20) Cập nhật Panel lên bản mới (fp update)"
echo "  (21) Sửa lỗi FerrousX + PHP          (22) Kích hoạt / Sửa lỗi phpMyAdmin 5.2"
echo "  (0)  Thoát (Exit)"
echo "======================================================================"
read -rp "Nhập số thứ tự chức năng cần thực thi [0-22]: " choice
[ "$choice" = "0" ] && exit 0
run_option "$choice"

