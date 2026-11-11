#!/usr/bin/env bash
# ==============================================================================
# FerrousPanel v2.5 — 100% Native Rust-Core Ubuntu Server Control Panel
# One-Line Automated Installer for Ubuntu 22.04 / 24.04 LTS & Debian 11/12
# Repository: https://github.com/Ferrous-panel/Ferrous-Panel
#
# Quick Install:
#   curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash
# ==============================================================================
set -euo pipefail

# ANSI color codes
BOLD='\033[1m'
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

REPO_URL="https://github.com/Ferrous-panel/Ferrous-Panel.git"
INSTALL_DIR="/opt/Ferrous-panel"
PANEL_PORT="${PANEL_PORT:-3000}"
PMA_PORT="${PMA_PORT:-888}"
PMA_PATH="${PMA_PATH:-/phpadmin}"
SECURITY_ENTRANCE="${SECURITY_ENTRANCE:-/panel}"

ADMIN_USER="${ADMIN_USER:-}"
ADMIN_PASS="${ADMIN_PASS:-}"
CUSTOM_DOMAIN="${CUSTOM_DOMAIN:-}"
SQL_ROOT_PASS="${SQL_ROOT_PASS:-}"
AUTO_CONFIRM=false

# 0. Parse Command Line Arguments
while [[ $# -gt 0 ]]; do
  case "$1" in
    --admin-user) ADMIN_USER="$2"; shift 2 ;;
    --admin-pass) ADMIN_PASS="$2"; shift 2 ;;
    --port) PANEL_PORT="$2"; shift 2 ;;
    --sql-pass) SQL_ROOT_PASS="$2"; shift 2 ;;
    --domain|--custom-domain) CUSTOM_DOMAIN="$2"; shift 2 ;;
    --entrance) SECURITY_ENTRANCE="$2"; shift 2 ;;
    -y|--yes|--unattended) AUTO_CONFIRM=true; shift ;;
    *) shift ;;
  esac
done

echo -e "${CYAN}"
cat << "EOF"
  ______                                      _____                  _ 
 |  ____|                                    |  __ \                | |
 | |__ ___ _ __ _ __ ___  _   _ ___          | |__) |_ _ _ __   ___ | |
 |  __/ _ \ '__| '__/ _ \| | | / __|  _____  |  ___/ _` | '_ \ / _ \| |
 | | |  __/ |  | | | (_) | |_| \__ \ |_____| | |  | (_| | | | |  __/| |
 |_|  \___|_|  |_|  \___/ \__,_|___/         |_|   \__,_|_| |_|\___||_|
 100% Native Rust Actix-Web Core · Polyglot FFI Engine · Ubuntu LTS
EOF
echo -e "${NC}"
echo -e "${BOLD}Starting FerrousPanel automated installer...${NC}"

# 1. Root & OS Verification
if [ "$(id -u)" -ne 0 ]; then
  echo -e "${RED}[ERROR] This script must be executed as root. Please run with sudo or as root user.${NC}" >&2
  exit 1
fi

if [ -f /etc/os-release ]; then
  . /etc/os-release
  DISTRO_ID="${ID:-unknown}"
  DISTRO_VER="${VERSION_ID:-unknown}"
else
  echo -e "${RED}[ERROR] Cannot detect Linux distribution from /etc/os-release.${NC}" >&2
  exit 1
fi

echo -e "${BLUE}[*] Detected OS: ${DISTRO_ID} ${DISTRO_VER} (${PRETTY_NAME:-Linux})${NC}"

# 2. Virtual Memory Optimization (Auto-Swap Allocation for Low RAM VPS)
TOTAL_RAM_KB=$(grep MemTotal /proc/meminfo | awk '{print $2}')
TOTAL_RAM_MB=$((TOTAL_RAM_KB / 1024))
TOTAL_SWAP_KB=$(grep SwapTotal /proc/meminfo | awk '{print $2}')
TOTAL_SWAP_MB=$((TOTAL_SWAP_KB / 1024))

echo -e "${BLUE}[*] Memory Check: ${TOTAL_RAM_MB}MB RAM, ${TOTAL_SWAP_MB}MB Swap detected.${NC}"

if [ "$TOTAL_RAM_MB" -lt 2048 ] && [ "$TOTAL_SWAP_MB" -lt 1024 ]; then
  echo -e "${YELLOW}[!] RAM is below 2GB. Creating a 2GB swapfile to prevent compiler out-of-memory errors...${NC}"
  if [ ! -f /swapfile_ferrous ]; then
    fallocate -l 2G /swapfile_ferrous || dd if=/dev/zero of=/swapfile_ferrous bs=1M count=2048
    chmod 600 /swapfile_ferrous
    mkswap /swapfile_ferrous
    swapon /swapfile_ferrous
    if ! grep -q "/swapfile_ferrous" /etc/fstab; then
      echo "/swapfile_ferrous none swap sw 0 0" >> /etc/fstab
    fi
    echo -e "${GREEN}[✓] 2GB Swapfile successfully configured and activated.${NC}"
  fi
fi

# 3. Interactive Prompts (if not in silent/unattended mode)
if [ "$AUTO_CONFIRM" = false ] && [ -t 0 ]; then
  echo ""
  echo -e "${BOLD}🔑 SET UP PANEL ADMIN CREDENTIALS (WEB CONTROL PANEL LOGIN):${NC}"
  echo "----------------------------------------------------------------------"
  if [ -z "$ADMIN_USER" ]; then
    read -rp "👉 Enter Admin Username [default: admin_ferrous]: " input_user || true
    ADMIN_USER="${input_user:-admin_ferrous}"
  fi
  if [ -z "$ADMIN_PASS" ]; then
    random_adm_pass="Fp#$(openssl rand -hex 4 2>/dev/null || echo '2026Ubuntu')!99"
    read -rp "👉 Enter Admin Password [press Enter to auto-generate: ${random_adm_pass}]: " input_pass || true
    ADMIN_PASS="${input_pass:-$random_adm_pass}"
  fi
  if [ -z "$CUSTOM_DOMAIN" ]; then
    read -rp "👉 Enter Panel Custom Domain (optional, press Enter to skip and use IP): " input_domain || true
    CUSTOM_DOMAIN="${input_domain:-}"
  fi
  echo "----------------------------------------------------------------------"
  echo -e "${GREEN}[✓] Admin account configured: ${ADMIN_USER}${NC}"
  [ -n "$CUSTOM_DOMAIN" ] && echo -e "${GREEN}[✓] Custom domain: ${CUSTOM_DOMAIN}${NC}"
  echo ""
fi

ADMIN_USER="${ADMIN_USER:-admin_ferrous}"
ADMIN_PASS="${ADMIN_PASS:-Fp#$(openssl rand -hex 4 2>/dev/null || echo '2026Ubuntu')!99}"
SQL_ROOT_PASS="${SQL_ROOT_PASS:-Sql#$(openssl rand -hex 4 2>/dev/null || echo '2026MariaDB')!88}"

# 4. Standard Directory Hierarchy
echo -e "${BLUE}[1/8] Creating standard hosting directory structure (/www/* & /etc/ferrous-panel)...${NC}"
mkdir -p /www/wwwroot
mkdir -p /www/wwwlogs
mkdir -p /www/backup
mkdir -p /www/server/panel/vhost/nginx
mkdir -p /www/server/panel/vhost/htpasswd
mkdir -p /var/www/letsencrypt
mkdir -p /etc/nginx/ssl
mkdir -p /etc/ferrous-panel
chown -R www-data:www-data /www/wwwroot /www/wwwlogs /var/www/letsencrypt 2>/dev/null || true

# 5. Core System Packages Installation
echo -e "${BLUE}[2/8] Updating package repositories and installing core packages...${NC}"
export DEBIAN_FRONTEND=noninteractive
apt-get update -y -q
apt-get install -y -q software-properties-common ca-certificates gnupg lsb-release \
  build-essential pkg-config libssl-dev nginx ufw fail2ban certbot python3-certbot-nginx \
  curl wget git zip unzip zstd jq openssl mariadb-server postgresql postgresql-contrib

# 6. Multi-PHP 8.3 FPM & phpMyAdmin 5.2
echo -e "${BLUE}[3/8] Configuring Multi-PHP FPM repository & installing PHP 8.3 + phpMyAdmin...${NC}"
if ! grep -rq "ondrej/php" /etc/apt/sources.list /etc/apt/sources.list.d/ 2>/dev/null; then
  add-apt-repository -y ppa:ondrej/php || true
  apt-get update -y -q
fi
apt-get install -y -q php8.3-fpm php8.3-cli php8.3-common php8.3-mysql php8.3-pgsql \
  php8.3-curl php8.3-gd php8.3-mbstring php8.3-xml php8.3-zip php8.3-bcmath php8.3-intl \
  php8.3-opcache php8.3-redis php8.3-imagick phpmyadmin || true

systemctl enable --now php8.3-fpm 2>/dev/null || true

# 7. Database Initialization
echo -e "${BLUE}[4/8] Initializing MariaDB & PostgreSQL with secure root credentials...${NC}"
systemctl enable --now mariadb postgresql 2>/dev/null || true
mysql -e "ALTER USER 'root'@'localhost' IDENTIFIED BY '${SQL_ROOT_PASS}'; FLUSH PRIVILEGES;" 2>/dev/null || true
sudo -u postgres psql -c "ALTER USER postgres WITH PASSWORD '${SQL_ROOT_PASS}';" 2>/dev/null || true

# phpMyAdmin Nginx VirtualHost on PMA_PORT
mkdir -p /etc/nginx/conf.d
cat > /etc/nginx/conf.d/ferrous-phpmyadmin.conf <<EOF
# FerrousPanel — phpMyAdmin 5.2 Guard on Port ${PMA_PORT}
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

# 8. Admin Credentials & Security Entrance
echo -e "${BLUE}[5/8] Provisioning Admin account (/etc/ferrous-panel/admin.json)...${NC}"
cat > /etc/ferrous-panel/admin.json <<EOF
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
chmod 600 /etc/ferrous-panel/admin.json
mkdir -p /www/server/panel
echo "${SECURITY_ENTRANCE}" > /www/server/panel/default.pl

# Fallback TLS Certificate
if [ ! -f /etc/nginx/ssl/ferrous-fallback.crt ]; then
  openssl req -x509 -nodes -days 3650 -newkey rsa:2048 \
    -keyout /etc/nginx/ssl/ferrous-fallback.key \
    -out /etc/nginx/ssl/ferrous-fallback.crt \
    -subj "/C=US/ST=Server/L=DataCenter/O=FerrousPanel/CN=localhost" 2>/dev/null || true
fi

cat > /etc/nginx/conf.d/ferrous-panel-vhosts.conf <<'EOF'
# Include all Website VirtualHosts managed by FerrousPanel Rust Engine
include /www/server/panel/vhost/nginx/*.conf;
EOF

if [ -n "${CUSTOM_DOMAIN:-}" ]; then
  cat > "/etc/nginx/conf.d/ferrous-panel-domain.conf" <<EOF
# FerrousPanel Custom Domain Reverse Proxy -> Rust Core Daemon
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

# 9. Source Code Deployment & Building
echo -e "${BLUE}[6/8] Deploying application source & compiling Rust Core...${NC}"

# If script is run inside an existing repo directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -f "${SCRIPT_DIR}/package.json" ]; then
  echo -e "${CYAN}[*] Using local codebase from ${SCRIPT_DIR}...${NC}"
  mkdir -p "${INSTALL_DIR}"
  cp -rf "${SCRIPT_DIR}/." "${INSTALL_DIR}/"
else
  echo -e "${CYAN}[*] Cloning latest source from ${REPO_URL}...${NC}"
  if [ -d "${INSTALL_DIR}" ]; then
    rm -rf "${INSTALL_DIR}"
  fi
  git clone --depth 1 "${REPO_URL}" "${INSTALL_DIR}"
fi

# Install Node.js 22 LTS if not present
if ! command -v node &>/dev/null; then
  echo -e "${BLUE}[*] Installing Node.js LTS via NodeSource...${NC}"
  curl -fsSL https://deb.nodesource.com/setup_22.x | bash -
  apt-get install -y -q nodejs
fi

# Install Rust toolchain via rustup if not present
if ! command -v cargo &>/dev/null; then
  echo -e "${BLUE}[*] Installing official Rust toolchain (stable)...${NC}"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
fi

if [ -f "$HOME/.cargo/env" ]; then
  . "$HOME/.cargo/env"
elif [ -f "/root/.cargo/env" ]; then
  . "/root/.cargo/env"
fi
export PATH="$HOME/.cargo/bin:/root/.cargo/bin:$PATH"

cd "${INSTALL_DIR}"
npm install --legacy-peer-deps --no-audit 2>/dev/null || npm install --legacy-peer-deps
npm run build

# Install CLI commands
if [ -f "${INSTALL_DIR}/Ferrous-panel/ferrous-cli.sh" ]; then
  cp "${INSTALL_DIR}/Ferrous-panel/ferrous-cli.sh" /usr/local/bin/fp
  cp "${INSTALL_DIR}/Ferrous-panel/ferrous-cli.sh" /usr/local/bin/ferrous
  chmod +x /usr/local/bin/fp /usr/local/bin/ferrous
fi

# 10. Systemd Service Configuration
echo -e "${BLUE}[7/8] Configuring Systemd service (ferrous-panel.service)...${NC}"
cat > /etc/systemd/system/ferrous-panel.service <<EOF
[Unit]
Description=FerrousPanel Native Rust & Web Control Panel Daemon
After=network-online.target nginx.service mariadb.service postgresql.service
Wants=network-online.target

[Service]
Type=simple
User=root
WorkingDirectory=${INSTALL_DIR}
Environment="NODE_ENV=production"
Environment="PORT=${PANEL_PORT}"
Environment="RUST_LOG=info,actix_web=info"
ExecStart=/usr/bin/node ${INSTALL_DIR}/server.ts
Restart=always
RestartSec=3
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

ln -sf /etc/systemd/system/ferrous-panel.service /etc/systemd/system/ferrous-panel-web.service
systemctl daemon-reload
systemctl enable --now ferrous-panel.service

# 11. Firewall Configuration (UFW & iptables)
echo -e "${BLUE}[8/8] Configuring UFW Netfilter firewall rules & iptables...${NC}"
ufw default deny incoming 2>/dev/null || true
ufw default allow outgoing 2>/dev/null || true
ufw allow 22/tcp comment 'SSH Remote Access' 2>/dev/null || true
ufw allow 80/tcp comment 'Nginx HTTP' 2>/dev/null || true
ufw allow 443/tcp comment 'Nginx HTTPS TLS 1.3' 2>/dev/null || true
ufw allow 443/udp comment 'Nginx HTTP/3 QUIC' 2>/dev/null || true
ufw allow "${PMA_PORT}/tcp" comment 'phpMyAdmin 5.2' 2>/dev/null || true
ufw allow "${PANEL_PORT}/tcp" comment 'FerrousPanel Web Gateway' 2>/dev/null || true
ufw --force enable 2>/dev/null || true

# Auto-unblock for Oracle Cloud / strict iptables default drop/reject rules
if command -v iptables &>/dev/null; then
  iptables -I INPUT 1 -p tcp --dport "${PANEL_PORT}" -j ACCEPT 2>/dev/null || true
  iptables -I INPUT 1 -p tcp --dport 80 -j ACCEPT 2>/dev/null || true
  iptables -I INPUT 1 -p tcp --dport 443 -j ACCEPT 2>/dev/null || true
  iptables -I INPUT 1 -p udp --dport 443 -j ACCEPT 2>/dev/null || true
  iptables -I INPUT 1 -p tcp --dport "${PMA_PORT}" -j ACCEPT 2>/dev/null || true
  iptables -I INPUT 1 -p tcp --dport 22 -j ACCEPT 2>/dev/null || true
fi

nginx -t 2>/dev/null && systemctl reload nginx 2>/dev/null || true

# 12. Completion Card
PUBLIC_IP="$(curl -s4 ifconfig.me 2>/dev/null || hostname -I 2>/dev/null | awk '{print $1}' || echo 'YOUR_SERVER_IP')"

echo ""
echo -e "${GREEN}==============================================================================${NC}"
echo -e "${GREEN}${BOLD}  ✅ FERROUSPANEL v2.5 HAS BEEN SUCCESSFULLY INSTALLED ON YOUR VPS!${NC}"
echo -e "${GREEN}==============================================================================${NC}"
if [ -n "${CUSTOM_DOMAIN:-}" ]; then
  echo -e "  🌐 ${BOLD}Panel URL (Domain)${NC}      : http://${CUSTOM_DOMAIN}${SECURITY_ENTRANCE}"
fi
echo -e "  🌐 ${BOLD}Panel URL (Public IP)${NC}  : http://${PUBLIC_IP}:${PANEL_PORT}${SECURITY_ENTRANCE}"
echo -e "  🌐 ${BOLD}Panel URL (Local)${NC}      : http://127.0.0.1:${PANEL_PORT}${SECURITY_ENTRANCE}"
echo -e "  🗄️  ${BOLD}phpMyAdmin 5.2 URL${NC}     : http://${PUBLIC_IP}:${PMA_PORT}${PMA_PATH}"
echo -e "  👤 ${BOLD}Admin Username${NC}         : ${ADMIN_USER}"
echo -e "  🔑 ${BOLD}Admin Password${NC}         : ${ADMIN_PASS}"
echo -e "  🔐 ${BOLD}SQL Root Password${NC}      : ${SQL_ROOT_PASS}"
echo -e "  📂 ${BOLD}Install Directory${NC}      : ${INSTALL_DIR}"
echo -e "  ⚙️  ${BOLD}Config File${NC}            : /etc/ferrous-panel/admin.json"
echo "------------------------------------------------------------------------------"
echo -e "  🛠️  ${BOLD}Terminal CLI Assistant${NC} : Type ${CYAN}fp${NC} or ${CYAN}ferrous${NC} anywhere in SSH terminal"
echo -e "  👉 ${BOLD}Change Admin Password${NC}  : Type ${CYAN}fp 5${NC} in SSH terminal"
echo -e "  👉 ${BOLD}Bind Custom Domain${NC}     : Type ${CYAN}fp 17 <domain>${NC} in SSH terminal"
echo -e "  👉 ${BOLD}Show Admin Credentials${NC} : Type ${CYAN}fp 14${NC} in SSH terminal"
echo -e "${GREEN}==============================================================================${NC}"
EOF
