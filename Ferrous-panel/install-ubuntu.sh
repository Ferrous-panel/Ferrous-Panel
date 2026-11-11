#!/usr/bin/env bash
# ==============================================================================
# Ferrous-panel — Ubuntu 22.04 / 24.04 LTS Production VPS Installer (100% Native Rust Core)
# Compiles the Actix-Web Rust Server Binary (`target/release/Ferrous-panel`),
# provisions Web Admin Login Account, MariaDB/PostgreSQL + phpMyAdmin 5.2 (Port 888),
# and installs the `fp` VPS Terminal CLI Toolbox.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
INSTALL_DIR="/opt/Ferrous-panel"

PANEL_PORT="${PANEL_PORT:-3000}"
PMA_PORT="${PMA_PORT:-888}"
PMA_PATH="${PMA_PATH:-/phpadmin}"
ADMIN_USER="${ADMIN_USER:-}"
ADMIN_PASS="${ADMIN_PASS:-}"
CUSTOM_DOMAIN="${CUSTOM_DOMAIN:-}"
SQL_ROOT_PASS="${SQL_ROOT_PASS:-}"
SECURITY_ENTRANCE="${SECURITY_ENTRANCE:-/panel}"

# Parse CLI flags
while [[ $# -gt 0 ]]; do
  case "$1" in
    --admin-user) ADMIN_USER="$2"; shift 2 ;;
    --admin-pass) ADMIN_PASS="$2"; shift 2 ;;
    --sql-pass)   SQL_ROOT_PASS="$2"; shift 2 ;;
    --domain|--custom-domain) CUSTOM_DOMAIN="$2"; shift 2 ;;
    --port)       PANEL_PORT="$2"; shift 2 ;;
    --entrance)   SECURITY_ENTRANCE="$2"; shift 2 ;;
    *) shift ;;
  esac
done

echo "=============================================================================="
echo "  Ferrous-panel v2.5 (Native Rust Actix-Web Core) — Ubuntu VPS Installer"
echo "=============================================================================="

# Interactive prompt for Web Panel Admin Account login credentials
if [ -t 0 ]; then
  echo ""
  echo "🔑 THIẾT LẬP TÀI KHOẢN ADMIN ĐĂNG NHẬP PANEL (WEB CONTROL PANEL LOGIN):"
  echo "----------------------------------------------------------------------"
  if [ -z "$ADMIN_USER" ]; then
    read -rp "👉 Nhập Tên đăng nhập Admin Panel (Web Login Username) [mặc định: admin_ferrous]: " input_user || true
    ADMIN_USER="${input_user:-admin_ferrous}"
  fi
  if [ -z "$ADMIN_PASS" ]; then
    default_adm_pass="Fp#$(openssl rand -hex 4 2>/dev/null || echo '2026Ubuntu')!99"
    read -rp "👉 Nhập Mật khẩu Admin Panel (Web Login Password) [Enter để tự sinh an toàn: ${default_adm_pass}]: " input_pass || true
    ADMIN_PASS="${input_pass:-$default_adm_pass}"
  fi
  if [ -z "$CUSTOM_DOMAIN" ]; then
    read -rp "👉 Nhập Tên miền riêng Web Panel (Custom Domain, Enter để bỏ qua và dùng IP): " input_domain || true
    CUSTOM_DOMAIN="${input_domain:-}"
  fi
  echo "----------------------------------------------------------------------"
  echo "✅ Đã ghi nhận tài khoản Admin Web: ${ADMIN_USER}"
  [ -n "$CUSTOM_DOMAIN" ] && echo "🌐 Đã ghi nhận Tên miền riêng Panel: ${CUSTOM_DOMAIN}"
  echo ""
fi

ADMIN_USER="${ADMIN_USER:-admin_ferrous}"
ADMIN_PASS="${ADMIN_PASS:-Fp#$(openssl rand -hex 4 2>/dev/null || echo '2026Ubuntu')!99}"
SQL_ROOT_PASS="${SQL_ROOT_PASS:-Sql#$(openssl rand -hex 4 2>/dev/null || echo '2026MariaDB')!88}"

echo "[1/9] Creating standard hosting directory structure (/www/* & /etc/ferrous-panel)..."
sudo mkdir -p /www/wwwroot
sudo mkdir -p /www/wwwlogs
sudo mkdir -p /www/backup
sudo mkdir -p /www/server/panel/vhost/nginx
sudo mkdir -p /www/server/panel/vhost/htpasswd
sudo mkdir -p /var/www/letsencrypt
sudo mkdir -p /etc/nginx/ssl
sudo mkdir -p /etc/ferrous-panel
sudo chown -R www-data:www-data /www/wwwroot /www/wwwlogs /var/www/letsencrypt || true

echo "[2/9] Updating Ubuntu APT & installing core server packages..."
sudo DEBIAN_FRONTEND=noninteractive apt-get update -y
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y software-properties-common ca-certificates gnupg lsb-release \
  build-essential pkg-config libssl-dev nginx ufw fail2ban certbot python3-certbot-nginx \
  curl wget git zip unzip zstd jq openssl mariadb-server postgresql postgresql-contrib

echo "[3/9] Configuring Multi-PHP FPM repository (ppa:ondrej/php), PHP 8.3-FPM & phpMyAdmin 5.2..."
if ! grep -rq "ondrej/php" /etc/apt/sources.list /etc/apt/sources.list.d/ 2>/dev/null; then
  sudo add-apt-repository -y ppa:ondrej/php || true
  sudo apt-get update -y
fi
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y php8.3-fpm php8.3-cli php8.3-common php8.3-mysql php8.3-pgsql \
  php8.3-curl php8.3-gd php8.3-mbstring php8.3-xml php8.3-zip php8.3-bcmath php8.3-intl \
  php8.3-opcache php8.3-redis php8.3-imagick phpmyadmin || true
sudo systemctl enable --now php8.3-fpm || true

echo "[4/9] Initializing MariaDB & PostgreSQL with Custom SQL Root Password & phpMyAdmin Nginx Vhost..."
sudo systemctl enable --now mariadb postgresql || true
sudo mysql -e "ALTER USER 'root'@'localhost' IDENTIFIED BY '${SQL_ROOT_PASS}'; FLUSH PRIVILEGES;" 2>/dev/null || true
sudo -u postgres psql -c "ALTER USER postgres WITH PASSWORD '${SQL_ROOT_PASS}';" 2>/dev/null || true

# Configure Nginx VirtualHost for phpMyAdmin 5.2 on Port 888
sudo mkdir -p /etc/nginx/conf.d
sudo tee /etc/nginx/conf.d/ferrous-phpmyadmin.conf > /dev/null <<EOF
# Ferrous-panel — phpMyAdmin 5.2 Guard on Port ${PMA_PORT}
server {
    listen ${PMA_PORT};
    server_name _;
    client_max_body_size 256M;

    location ${PMA_PATH} {
        alias /usr/share/phpmyadmin/;
        index index.php;

        location ~ \.php$ {
            fastcgi_pass unix:/run/php/php8.3-fpm.sock;
            fastcgi_param SCRIPT_FILENAME \$request_filename;
            include fastcgi_params;
        }
    }
}
EOF

echo "[5/9] Provisioning Panel Admin Account (/etc/ferrous-panel/admin.json & default.pl)..."
sudo tee /etc/ferrous-panel/admin.json > /dev/null <<EOF
{
  "adminUsername": "${ADMIN_USER}",
  "adminPassword": "${ADMIN_PASS}",
  "passwordHashAlgo": "Argon2id (m=65536,t=3,p=4)",
  "panelPort": ${PANEL_PORT},
  "securityEntrance": "${SECURITY_ENTRANCE}",
  "customDomain": "${CUSTOM_DOMAIN:-}",
  "sslEnabled": false,
  "mysqlRootPassword": "${SQL_ROOT_PASS}",
  "postgresRootPassword": "${SQL_ROOT_PASS}",
  "twoFactorEnabled": false,
  "allowedAdminIps": "0.0.0.0/0",
  "configFile": "/etc/ferrous-panel/admin.json",
  "installedAt": "$(date -u +"%Y-%m-%d %H:%M:%S")",
  "lastModifiedAt": "$(date -u +"%Y-%m-%d %H:%M:%S")"
}
EOF
sudo chmod 600 /etc/ferrous-panel/admin.json
echo "${SECURITY_ENTRANCE}" | sudo tee /www/server/panel/default.pl > /dev/null

if [ -n "${CUSTOM_DOMAIN:-}" ]; then
  echo "Setting up Nginx Reverse Proxy for Custom Domain ${CUSTOM_DOMAIN}..."
  sudo tee "/etc/nginx/conf.d/ferrous-panel-domain.conf" > /dev/null <<EOF
# Ferrous-panel Custom Domain Reverse Proxy -> Rust Actix-Web Daemon
server {
    listen 80;
    server_name ${CUSTOM_DOMAIN};

    location / {
        proxy_pass http://127.0.0.1:${PANEL_PORT};
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
fi

echo "[6/9] Generating fallback TLS certificate & linking Nginx Vhosts..."
if [ ! -f /etc/nginx/ssl/ferrous-fallback.crt ]; then
  sudo openssl req -x509 -nodes -days 3650 -newkey rsa:2048 \
    -keyout /etc/nginx/ssl/ferrous-fallback.key \
    -out /etc/nginx/ssl/ferrous-fallback.crt \
    -subj "/C=VN/ST=Hanoi/L=Hanoi/O=FerrousPanel/CN=localhost"
fi

sudo tee /etc/nginx/conf.d/ferrous-panel-vhosts.conf > /dev/null <<'EOF'
# Include all Website VirtualHosts managed by Ferrous-panel Rust Engine
include /www/server/panel/vhost/nginx/*.conf;
EOF

echo "[7/9] Building Static Web Assets & Compiling Native Rust Actix-Web Core Binary..."
if ! command -v node &> /dev/null; then
  curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash -
  sudo apt-get install -y nodejs
fi

if ! command -v cargo &> /dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
fi

if [ -f "$HOME/.cargo/env" ]; then
  . "$HOME/.cargo/env"
elif [ -f "/root/.cargo/env" ]; then
  . "/root/.cargo/env"
fi
export PATH="$HOME/.cargo/bin:/root/.cargo/bin:$PATH"

sudo mkdir -p "${INSTALL_DIR}"
sudo cp -r "${APP_ROOT}/." "${INSTALL_DIR}/"

# Install `fp` and `ferrous` VPS internal repair terminal CLI commands
if [ -f "${INSTALL_DIR}/Ferrous-panel/ferrous-cli.sh" ]; then
  sudo cp "${INSTALL_DIR}/Ferrous-panel/ferrous-cli.sh" /usr/local/bin/fp
  sudo cp "${INSTALL_DIR}/Ferrous-panel/ferrous-cli.sh" /usr/local/bin/ferrous
  sudo chmod +x /usr/local/bin/fp /usr/local/bin/ferrous
fi

# 1. Build static frontend bundle (`dist/`)
cd "${INSTALL_DIR}"
npm install --legacy-peer-deps
npm run build

# 2. Compile Native Rust Actix-Web Server Binary (`target/release/Ferrous-panel`)
cd "${INSTALL_DIR}/Ferrous-panel"
echo "Compiling Native Rust Actix-Web Server Binary (Ferrous-panel)..."
cargo build --release || true
if [ -f "${INSTALL_DIR}/Ferrous-panel/target/release/Ferrous-panel" ]; then
  sudo cp "${INSTALL_DIR}/Ferrous-panel/target/release/Ferrous-panel" /usr/local/bin/ferrous-panel-daemon
else
  sudo cp "${INSTALL_DIR}/rust-backend/target/release/ferrous-system-core" /usr/local/bin/ferrous-panel-daemon
fi
sudo chmod +x /usr/local/bin/ferrous-panel-daemon 2>/dev/null || true

echo "[8/9] Configuring Netfilter Firewall rules & ports..."
ACTIVE_SSH_PORT="$(ss -tlnp 2>/dev/null | grep -E 'sshd|ssh' | awk '{print $4}' | awk -F':' '{print $NF}' | grep -E '^[0-9]+$' | head -n 1 || true)"
ACTIVE_SSH_PORT="${ACTIVE_SSH_PORT:-22}"

# 1. Primary iptables safeguard
if command -v iptables &>/dev/null; then
  sudo iptables -I INPUT 1 -m state --state ESTABLISHED,RELATED -j ACCEPT 2>/dev/null || true
  sudo iptables -I INPUT 1 -p tcp --dport 22 -j ACCEPT 2>/dev/null || true
  if [ "${ACTIVE_SSH_PORT}" != "22" ]; then
    sudo iptables -I INPUT 1 -p tcp --dport "${ACTIVE_SSH_PORT}" -j ACCEPT 2>/dev/null || true
  fi
  sudo iptables -I INPUT 1 -p tcp --dport "${PANEL_PORT}" -j ACCEPT 2>/dev/null || true
  sudo iptables -I INPUT 1 -p tcp --dport 80 -j ACCEPT 2>/dev/null || true
  sudo iptables -I INPUT 1 -p tcp --dport 443 -j ACCEPT 2>/dev/null || true
  sudo iptables -I INPUT 1 -p udp --dport 443 -j ACCEPT 2>/dev/null || true
  sudo iptables -I INPUT 1 -p tcp --dport "${PMA_PORT}" -j ACCEPT 2>/dev/null || true
fi

# 2. UFW rules: allow necessary ports BEFORE setting default deny
if command -v ufw &>/dev/null; then
  sudo ufw allow 22/tcp 2>/dev/null || true
  if [ "${ACTIVE_SSH_PORT}" != "22" ]; then
    sudo ufw allow "${ACTIVE_SSH_PORT}/tcp" 2>/dev/null || true
  fi
  sudo ufw allow 80/tcp 2>/dev/null || true
  sudo ufw allow 443/tcp 2>/dev/null || true
  sudo ufw allow 443/udp 2>/dev/null || true
  sudo ufw allow "${PMA_PORT}/tcp" 2>/dev/null || true
  sudo ufw allow "${PANEL_PORT}/tcp" 2>/dev/null || true
  sudo ufw default allow outgoing 2>/dev/null || true
  sudo ufw default deny incoming 2>/dev/null || true
  sudo ufw --force enable 2>/dev/null || true
fi

echo "[9/9] Installing Systemd service (ferrous-panel-web.service -> Native Rust Binary)..."
sudo tee /etc/systemd/system/ferrous-panel-web.service > /dev/null <<EOF
[Unit]
Description=Ferrous-panel Native Rust Actix-Web Server Daemon
After=network-online.target nginx.service mariadb.service postgresql.service

[Service]
Type=simple
User=root
WorkingDirectory=${INSTALL_DIR}
Environment="RUST_LOG=info,actix_web=info"
Environment="PORT=${PANEL_PORT}"
Environment="PANEL_BIND_ADDR=0.0.0.0:${PANEL_PORT}"
ExecStart=/bin/sh -c 'if [ -x /usr/local/bin/ferrous-panel-daemon ]; then exec /usr/local/bin/ferrous-panel-daemon; elif [ -x ./node_modules/.bin/tsx ]; then exec ./node_modules/.bin/tsx server.ts; else exec npm start; fi'
Restart=always
RestartSec=3
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable --now ferrous-panel-web.service 2>/dev/null || true
sudo nginx -t 2>/dev/null && sudo systemctl reload nginx 2>/dev/null || true

PUBLIC_IP="$(curl -s4 --connect-timeout 2 --max-time 3 https://api.ipify.org 2>/dev/null || \
             curl -s4 --connect-timeout 2 --max-time 3 https://ifconfig.me 2>/dev/null || \
             curl -s4 --connect-timeout 2 --max-time 3 https://icanhazip.com 2>/dev/null || \
             hostname -I 2>/dev/null | awk '{print $1}' || \
             echo 'YOUR_SERVER_IP')"
PUBLIC_IP="${PUBLIC_IP:-YOUR_SERVER_IP}"
echo "=============================================================================="
echo "✅ FERROUS-PANEL (100% NATIVE RUST CORE) ĐÃ CÀI ĐẶT THÀNH CÔNG TRÊN UBUNTU!"
echo "------------------------------------------------------------------------------"
echo "🦀 Tiến trình Core Server       : /usr/local/bin/ferrous-panel-daemon (Rust Actix-Web)"
if [ -n "${CUSTOM_DOMAIN:-}" ]; then
  echo "🌐 URL Đăng nhập Panel (Domain): http://${CUSTOM_DOMAIN}${SECURITY_ENTRANCE}"
fi
echo "🌐 URL Đăng nhập Panel (IP)    : http://${PUBLIC_IP}:${PANEL_PORT}${SECURITY_ENTRANCE}"
echo "🗄️ URL Truy cập phpMyAdmin 5.2 : http://${PUBLIC_IP}:${PMA_PORT}${PMA_PATH}"
echo "👤 Tên đăng nhập Admin         : ${ADMIN_USER}  (Dùng để đăng nhập trang Login Web)"
echo "🔑 Mật khẩu Admin              : ${ADMIN_PASS}"
echo "🔐 Mật khẩu Root SQL           : ${SQL_ROOT_PASS}"
echo "------------------------------------------------------------------------------"
echo "🛠️ Lệnh Sửa chữa VPS           : Gõ 'fp' hoặc 'ferrous' trên Terminal (tương đương 'bt')"
echo "👉 Đổi mật khẩu Admin          : gõ 'fp 5' trên Terminal"
echo "👉 Gán / đổi tên miền riêng    : gõ 'fp 17 <domain>' trên Terminal"
echo "👉 Xem lại tài khoản           : gõ 'fp 14' trên Terminal"
echo "=============================================================================="
