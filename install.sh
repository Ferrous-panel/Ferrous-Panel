#!/usr/bin/env bash
# ==============================================================================
# FerrousPanel — Pure-Rust VPS Control Panel Installer (Ubuntu 22.04 / 24.04 LTS)
# Downloads prebuilt release binaries only (zero Node.js, Bun, or Cargo on VPS).
#
# Quick Install:
#   curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash
# ==============================================================================
set -euo pipefail

BOLD='\033[1m'
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

REPO_SLUG="${FERROUS_REPO:-Ferrous-panel/Ferrous-Panel}"
RELEASE_TAG="${FERROUS_VERSION:-latest}"
PANEL_BIND="${FERROUS_BIND:-127.0.0.1}"
PANEL_PORT="${FERROUS_PORT:-8443}"
SECURITY_ENTRANCE="${FERROUS_SECURITY_ENTRANCE:-/panel}"
ADMIN_USER="${FERROUS_ADMIN_USER:-admin_ferrous}"
CUSTOM_DOMAIN="${FERROUS_CUSTOM_DOMAIN:-}"
WEB_ENGINE="${FERROUS_ENGINE:-}"
AUTO_CONFIRM=false

DATA_DIR="/var/lib/ferrous-panel"
ENV_FILE="/etc/ferrous-panel/panel.env"
WWWROOT="/www/wwwroot"
BACKUPS_DIR="/var/backups/ferrous-panel"
BIN_PANEL="/usr/local/bin/ferrous-panel"
BIN_FERROUSX="/usr/local/bin/ferrousx-bin"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --admin-pass|--password|--sql-pass)
      echo -e "${RED}[ERROR] Passing passwords via CLI flags is forbidden for security (leaks in process list).${NC}" >&2
      echo -e "${YELLOW}First-run admin password is generated from OsRng once and stored as an Argon2id hash (0600).${NC}" >&2
      echo -e "${YELLOW}Use 'sudo fp passwd' after installation to change it interactively.${NC}" >&2
      exit 1
      ;;
    --admin-user) ADMIN_USER="$2"; shift 2 ;;
    --bind) PANEL_BIND="$2"; shift 2 ;;
    --port) PANEL_PORT="$2"; shift 2 ;;
    --entrance) SECURITY_ENTRANCE="$2"; shift 2 ;;
    --domain|--custom-domain) CUSTOM_DOMAIN="$2"; shift 2 ;;
    --engine) WEB_ENGINE="$2"; shift 2 ;;
    --version) RELEASE_TAG="$2"; shift 2 ;;
    -y|--yes|--unattended) AUTO_CONFIRM=true; shift ;;
    *)
      echo -e "${RED}[ERROR] Unknown option: $1${NC}" >&2
      exit 1
      ;;
  esac
done

if [ "$(id -u)" -ne 0 ]; then
  echo -e "${RED}[ERROR] install.sh must be run as root (use sudo).${NC}" >&2
  exit 1
fi

if [ ! -f /etc/os-release ]; then
  echo -e "${RED}[ERROR] Missing /etc/os-release; Ubuntu 22.04/24.04 LTS required.${NC}" >&2
  exit 1
fi
. /etc/os-release
echo -e "${BLUE}[*] Detected OS: ${PRETTY_NAME:-${ID:-linux} ${VERSION_ID:-}}${NC}"

ARCH_RAW="$(uname -m)"
case "$ARCH_RAW" in
  x86_64|amd64) ARCH="x86_64" ;;
  aarch64|arm64) ARCH="aarch64" ;;
  *)
    echo -e "${RED}[ERROR] Unsupported CPU architecture: ${ARCH_RAW} (supported: x86_64, aarch64)${NC}" >&2
    exit 1
    ;;
esac

if [ -z "$WEB_ENGINE" ]; then
  if [ "$AUTO_CONFIRM" = false ] && [ -t 0 ]; then
    echo ""
    echo -e "${BOLD}Select Frontend Web Server Engine:${NC}"
    echo "  1) nginx    (apt install nginx)"
    echo "  2) ferrousx (Pure-Rust HTTP/HTTPS/FastCGI/WebSocket binary)"
    read -rp "Choose [1/2, default 1]: " engine_choice || true
    case "${engine_choice:-1}" in
      2|ferrousx) WEB_ENGINE="ferrousx" ;;
      *) WEB_ENGINE="nginx" ;;
    esac
  else
    WEB_ENGINE="nginx"
  fi
fi

WEB_ENGINE="$(echo "$WEB_ENGINE" | tr '[:upper:]' '[:lower:]')"
if [[ "$WEB_ENGINE" != "nginx" && "$WEB_ENGINE" != "ferrousx" ]]; then
  echo -e "${RED}[ERROR] --engine must be 'nginx' or 'ferrousx' (got '${WEB_ENGINE}').${NC}" >&2
  exit 1
fi

if ! [[ "$PANEL_PORT" =~ ^[0-9]+$ ]] || [ "$PANEL_PORT" -lt 1 ] || [ "$PANEL_PORT" -gt 65535 ]; then
  echo -e "${RED}[ERROR] Invalid --port '${PANEL_PORT}' (expected 1..=65535).${NC}" >&2
  exit 1
fi

if [ "$RELEASE_TAG" = "latest" ]; then
  RELEASE_BASE="https://github.com/${REPO_SLUG}/releases/latest/download"
else
  RELEASE_BASE="https://github.com/${REPO_SLUG}/releases/download/${RELEASE_TAG}"
fi

echo -e "${BLUE}[1/6] Installing runtime system utilities (no Node.js, Bun, or Cargo)...${NC}"
export DEBIAN_FRONTEND=noninteractive
apt-get update -y -q
CORE_PKGS=(ca-certificates curl tar zstd zip unzip openssl ufw fail2ban certbot)
if [ "$WEB_ENGINE" = "nginx" ]; then
  CORE_PKGS+=(nginx)
fi
apt-get install -y -q "${CORE_PKGS[@]}"

echo -e "${BLUE}[2/6] Preparing system directories (/var/lib/ferrous-panel, /www/wwwroot, /www/acme)...${NC}"
mkdir -p /etc/ferrous-panel "${DATA_DIR}" "${WWWROOT}" /www/acme "${BACKUPS_DIR}"
chmod 700 "${DATA_DIR}" /etc/ferrous-panel "${BACKUPS_DIR}"
chown -R www-data:www-data "${WWWROOT}" /www/acme 2>/dev/null || true

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

download_release_binary() {
  local asset_name="$1"
  local dest_path="$2"
  local url="${RELEASE_BASE}/${asset_name}"
  local tmp_file="${TMP_DIR}/${asset_name}"

  echo -e "${CYAN}[*] Downloading ${url}...${NC}"
  curl -fsSL --retry 3 --connect-timeout 10 -o "$tmp_file" "$url"

  if [ -f "${TMP_DIR}/SHA256SUMS.txt" ] && grep -q " ${asset_name}\$" "${TMP_DIR}/SHA256SUMS.txt"; then
    (cd "$TMP_DIR" && grep " ${asset_name}\$" SHA256SUMS.txt | sha256sum -c -)
  fi

  install -m 0755 "$tmp_file" "$dest_path"
}

echo -e "${BLUE}[3/6] Downloading verified FerrousPanel release binary (${ARCH})...${NC}"
curl -fsSL --connect-timeout 10 -o "${TMP_DIR}/SHA256SUMS.txt" "${RELEASE_BASE}/SHA256SUMS.txt" 2>/dev/null || rm -f "${TMP_DIR}/SHA256SUMS.txt"

download_release_binary "ferrous-panel-linux-${ARCH}" "${BIN_PANEL}"
ln -sf "${BIN_PANEL}" /usr/local/bin/fp
ln -sf "${BIN_PANEL}" /usr/local/bin/ferrous

if [ "$WEB_ENGINE" = "ferrousx" ]; then
  echo -e "${BLUE}[4/6] Downloading FerrousX web server binary & configuring ferrousx.service...${NC}"
  download_release_binary "ferrousx-bin-linux-${ARCH}" "${BIN_FERROUSX}"
  mkdir -p /etc/ferrousx/sites
  if [ ! -f /etc/ferrousx/ferrousx.toml ]; then
    cat > /etc/ferrousx/ferrousx.toml <<'EOF_FX'
# Managed by FerrousPanel — FerrousX master configuration
worker_threads = 4
bind_http = "0.0.0.0:80"
bind_https = "0.0.0.0:443"
sites_dir = "/etc/ferrousx/sites"
acme_webroot = "/www/acme"
acme_challenge_prefix = "/.well-known/acme-challenge/"
server_tokens = false
EOF_FX
  fi
  cat > /etc/systemd/system/ferrousx.service <<'EOF_FX_SVC'
[Unit]
Description=FerrousX Pure-Rust HTTP/HTTPS Reverse Proxy & FastCGI Web Server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=root
ExecStartPre=/usr/local/bin/ferrousx-bin --config /etc/ferrousx/ferrousx.toml --check-config
ExecStart=/usr/local/bin/ferrousx-bin --config /etc/ferrousx/ferrousx.toml
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=3
LimitNOFILE=65535
NoNewPrivileges=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF_FX_SVC
  systemctl daemon-reload
  systemctl enable --now ferrousx.service
else
  echo -e "${BLUE}[4/6] Configuring Nginx (nginx.service) with /www/acme ACME challenge root...${NC}"
  systemctl enable --now nginx.service
fi

echo -e "${BLUE}[5/6] Initializing Argon2id credentials, SQLite store, and systemd unit...${NC}"
cat > "${ENV_FILE}" <<EOF_ENV
FERROUS_BIND=${PANEL_BIND}
FERROUS_PORT=${PANEL_PORT}
FERROUS_DATA_DIR=${DATA_DIR}
FERROUS_WWWROOT=${WWWROOT}
FERROUS_BACKUPS_DIR=${BACKUPS_DIR}
FERROUS_ADMIN_USER=${ADMIN_USER}
FERROUS_SECURITY_ENTRANCE=${SECURITY_ENTRANCE}
FERROUS_CUSTOM_DOMAIN=${CUSTOM_DOMAIN}
RUST_LOG=info,actix_web=info
EOF_ENV
chmod 0600 "${ENV_FILE}"

"${BIN_PANEL}" \
  --data-dir "${DATA_DIR}" \
  --wwwroot "${WWWROOT}" \
  --admin-username "${ADMIN_USER}" \
  fp init --engine "${WEB_ENGINE}"

cat > /etc/systemd/system/ferrous-panel.service <<'EOF_SVC'
[Unit]
Description=FerrousPanel Pure-Rust VPS Control Plane
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=root
EnvironmentFile=-/etc/ferrous-panel/panel.env
ExecStart=/usr/local/bin/ferrous-panel serve
Restart=on-failure
RestartSec=3
LimitNOFILE=65535
NoNewPrivileges=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF_SVC

systemctl daemon-reload
systemctl enable --now ferrous-panel.service

echo -e "${BLUE}[6/6] Configuring UFW rules (SSH + 80 + 443)...${NC}"
ACTIVE_SSH_PORT="$(ss -tlnp 2>/dev/null | grep -E 'sshd|ssh' | awk '{print $4}' | awk -F':' '{print $NF}' | grep -E '^[0-9]+$' | head -n 1 || true)"
ACTIVE_SSH_PORT="${ACTIVE_SSH_PORT:-22}"
ufw allow "${ACTIVE_SSH_PORT}/tcp" comment 'SSH' >/dev/null 2>&1 || true
ufw allow 80/tcp comment 'HTTP' >/dev/null 2>&1 || true
ufw allow 443/tcp comment 'HTTPS' >/dev/null 2>&1 || true
ufw default deny incoming >/dev/null 2>&1 || true
ufw default allow outgoing >/dev/null 2>&1 || true
ufw --force enable >/dev/null 2>&1 || true

echo ""
echo -e "${GREEN}==============================================================================${NC}"
echo -e "${GREEN}${BOLD}  FerrousPanel installed cleanly (pure-Rust binaries only)${NC}"
echo -e "${GREEN}==============================================================================${NC}"
echo -e "  Binary            : ${BIN_PANEL} (embedded dist/ SPA)"
echo -e "  CLI Symlinks      : fp / ferrous -> ${BIN_PANEL}"
echo -e "  Web Engine        : ${WEB_ENGINE}"
echo -e "  Panel Listen Addr : http://${PANEL_BIND}:${PANEL_PORT}"
echo -e "  Credentials Hash  : ${DATA_DIR}/admin.credentials.json (mode 0600, Argon2id)"
echo -e "  Panel Env Config  : ${ENV_FILE}"
echo -e "  Reset Password    : sudo fp passwd"
echo -e "  SSH Tunnel Access : ssh -N -L ${PANEL_PORT}:${PANEL_BIND}:${PANEL_PORT} root@<VPS_IP>"
echo -e "${GREEN}==============================================================================${NC}"
