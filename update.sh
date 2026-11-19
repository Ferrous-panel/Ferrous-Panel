#!/usr/bin/env bash
# ==============================================================================
# FerrousPanel & FerrousX — 1-Command In-Place Updater
# Preserves /var/lib/ferrous-panel, /etc/ferrous-panel, and /etc/ferrousx data.
#
# 1-Command Update (FerrousPanel + FerrousX if installed):
#   curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/update.sh | sudo bash
#
# Force sync/install latest FerrousX from GitHub (Ferrous-panel/FerrousX):
#   curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/update.sh | sudo bash -s -- --ferrousx
#
# Update FerrousPanel & validate/sync Nginx engine:
#   curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/update.sh | sudo bash -s -- --engine nginx
#
# Update ONLY FerrousX from GitHub (skip rebuilding ferrous-panel):
#   curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/update.sh | sudo bash -s -- --ferrousx-only
# ==============================================================================
set -euo pipefail

BOLD='\033[1m'
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

if [ "$(id -u)" -ne 0 ]; then
  echo -e "${RED}[ERROR] update.sh must be run as root (use sudo).${NC}" >&2
  exit 1
fi

REPO_SLUG="${FERROUS_REPO:-Ferrous-panel/Ferrous-Panel}"
GIT_BRANCH="${FERROUS_BRANCH:-main}"
FERROUSX_REPO="${FERROUSX_REPO:-Ferrous-panel/FerrousX}"
FERROUSX_BRANCH="${FERROUSX_BRANCH:-main}"
RELEASE_TAG="${FERROUS_VERSION:-latest}"
RUST_TOOLCHAIN="${FERROUS_RUST_VERSION:-stable}"
SRC_DIR="/usr/local/src/ferrous-panel"
FERROUSX_SRC_DIR="/usr/local/src/ferrousx"
BIN_PANEL="/usr/local/bin/ferrous-panel"
BIN_FERROUSX="/usr/local/bin/ferrousx-bin"
BIN_FERROUSX_CTL="/usr/local/bin/ferrousx-ctl"
FERROUSX_ENV_FILE="/etc/ferrousx/ferrousx.env"
FERROUSX_ADMIN_TOKEN_FILE="/etc/ferrousx/admin.token"

UPDATE_PANEL=true
FORCE_FERROUSX=false
FORCE_NGINX=false

usage() {
  cat <<'EOF_HELP'
FerrousPanel, Nginx & FerrousX 1-Command Updater

Usage:
  sudo bash update.sh [OPTIONS] [RELEASE_TAG]

Options:
  --nginx                      Ensure Nginx is installed, synced, validated (nginx -t) & reloaded
  --nginx-only                 Validate & sync ONLY Nginx (do not rebuild ferrous-panel)
  --ferrousx                   Sync & rebuild latest FerrousX from github.com/Ferrous-panel/FerrousX
  --ferrousx-only              Update ONLY FerrousX from GitHub (do not rebuild ferrous-panel)
  --engine <nginx|ferrousx>    Target web engine to update & validate
  --ferrousx-branch <BRANCH>   Upstream FerrousX Git branch (default: main)
  --branch <BRANCH>            FerrousPanel Git branch (default: main)
  --version <TAG|latest>       GitHub Release tag for prebuilt fallback (default: latest)
  -h, --help                   Show this help message
EOF_HELP
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --nginx|--with-nginx)
      FORCE_NGINX=true
      shift
      ;;
    --nginx-only)
      FORCE_NGINX=true
      UPDATE_PANEL=false
      shift
      ;;
    --ferrousx|--with-ferrousx)
      FORCE_FERROUSX=true
      shift
      ;;
    --ferrousx-only)
      FORCE_FERROUSX=true
      UPDATE_PANEL=false
      shift
      ;;
    --engine)
      case "${2:-}" in
        ferrousx) FORCE_FERROUSX=true ;;
        nginx) FORCE_NGINX=true ;;
        *)
          echo -e "${RED}[ERROR] --engine must be 'nginx' or 'ferrousx' (got '${2:-}').${NC}" >&2
          exit 1
          ;;
      esac
      shift 2
      ;;
    --ferrousx-branch)
      FERROUSX_BRANCH="$2"
      shift 2
      ;;
    --branch)
      GIT_BRANCH="$2"
      shift 2
      ;;
    --version)
      RELEASE_TAG="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    -*)
      echo -e "${RED}[ERROR] Unknown option: $1${NC}" >&2
      usage >&2
      exit 1
      ;;
    *)
      RELEASE_TAG="$1"
      shift
      ;;
  esac
done

export RUSTUP_HOME="${RUSTUP_HOME:-/root/.rustup}"
export CARGO_HOME="${CARGO_HOME:-/root/.cargo}"
export PATH="${CARGO_HOME}/bin:/usr/local/bin:${PATH}"

ARCH_RAW="$(uname -m)"
case "$ARCH_RAW" in
  x86_64|amd64) ARCH="x86_64" ;;
  aarch64|arm64) ARCH="aarch64" ;;
  *)
    echo -e "${RED}[ERROR] Unsupported CPU architecture: ${ARCH_RAW}${NC}" >&2
    exit 1
    ;;
esac

if [ "$RELEASE_TAG" = "latest" ]; then
  RELEASE_BASE="https://github.com/${REPO_SLUG}/releases/latest/download"
else
  RELEASE_BASE="https://github.com/${REPO_SLUG}/releases/download/${RELEASE_TAG}"
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

ensure_rust_toolchain() {
  local min_ver_code=189
  if command -v rustc >/dev/null 2>&1; then
    local ver_code
    ver_code="$(rustc --version | awk '{print $2}' | awk -F'.' '{print ($1 * 100) + $2}' 2>/dev/null || echo 0)"
    if [ "${ver_code:-0}" -ge "${min_ver_code}" ]; then
      return 0
    fi
    echo -e "${YELLOW}[!] Existing $(rustc --version) is older than Rust 1.89+ required by crates; upgrading to ${RUST_TOOLCHAIN}...${NC}"
  else
    return 0
  fi

  if command -v rustup >/dev/null 2>&1; then
    rustup toolchain install "${RUST_TOOLCHAIN}" --profile minimal
    rustup default "${RUST_TOOLCHAIN}"
  else
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "${RUST_TOOLCHAIN}"
  fi

  if [ -x "${CARGO_HOME}/bin/cargo" ]; then
    ln -sf "${CARGO_HOME}/bin/cargo" /usr/local/bin/cargo
  fi
  if [ -x "${CARGO_HOME}/bin/rustc" ]; then
    ln -sf "${CARGO_HOME}/bin/rustc" /usr/local/bin/rustc
  fi
  if [ -x "${CARGO_HOME}/bin/rustup" ]; then
    ln -sf "${CARGO_HOME}/bin/rustup" /usr/local/bin/rustup
  fi
}

ensure_rust_toolchain

detect_local_ip() {
  local ip=""
  ip="$(ip -4 route get 1.1.1.1 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i=="src") {print $(i+1); exit}}' || true)"
  if [ -z "$ip" ]; then
    ip="$(hostname -I 2>/dev/null | awk '{print $1}' || true)"
  fi
  echo "${ip:-127.0.0.1}"
}

detect_public_ip() {
  local ip=""
  for url in "https://api.ipify.org" "https://ifconfig.me/ip" "https://ipv4.icanhazip.com"; do
    ip="$(curl -4 -fsS --connect-timeout 2 --max-time 3 "$url" 2>/dev/null | tr -d '[:space:]' || true)"
    if [[ "$ip" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}$ ]]; then
      echo "$ip"
      return 0
    fi
  done
  detect_local_ip
}

VPS_PUBLIC_IP="$(detect_public_ip)"

setup_phpmyadmin_proxy() {
  mkdir -p /etc/ferrousx /var/lib/phpmyadmin
  chown www-data:www-data /var/lib/phpmyadmin 2>/dev/null || true

  if [ -f "${FERROUSX_SRC_DIR}/scripts/pma-router.php" ]; then
    install -m 0644 "${FERROUSX_SRC_DIR}/scripts/pma-router.php" /etc/ferrousx/pma-router.php
  else
    cat > /etc/ferrousx/pma-router.php <<'EOF_PMA_PHP'
<?php
// Managed by FerrousPanel — phpMyAdmin Internal Loopback Router (127.0.0.1:888)
$remoteAddr = $_SERVER['REMOTE_ADDR'] ?? '';
if ($remoteAddr !== '127.0.0.1' && $remoteAddr !== '::1') {
    http_response_code(403);
    header('Content-Type: text/plain; charset=utf-8');
    echo '403 Forbidden: Direct external access to phpMyAdmin internal port is disabled.';
    exit;
}

$docRoot = '/usr/share/phpmyadmin';
$realDocRoot = realpath($docRoot);
if ($realDocRoot === false) {
    http_response_code(500);
    header('Content-Type: text/plain; charset=utf-8');
    echo '500 Internal Server Error: phpMyAdmin directory not found.';
    exit;
}

$uri = parse_url($_SERVER['REQUEST_URI'] ?? '/', PHP_URL_PATH);
if (!is_string($uri) || $uri === '' || strpos($uri, "\0") !== false) {
    http_response_code(400);
    exit;
}

if (preg_match('#^/(phpadmin|phpmyadmin)(/.*)?$#i', $uri, $m)) {
    $uri = isset($m[2]) && $m[2] !== '' ? $m[2] : '/';
}

if (
    strpos($uri, '..') !== false ||
    preg_match('#^/setup(/|$)#i', $uri) ||
    preg_match('#/\.(ht|env|git)#i', $uri)
) {
    http_response_code(403);
    header('Content-Type: text/plain; charset=utf-8');
    echo '403 Forbidden';
    exit;
}

$target = $realDocRoot . $uri;
if (is_dir($target)) {
    $target = rtrim($target, '/') . '/index.php';
}

$realTarget = realpath($target);
if (
    $realTarget === false ||
    ($realTarget !== $realDocRoot && strpos($realTarget, $realDocRoot . DIRECTORY_SEPARATOR) !== 0) ||
    !is_file($realTarget)
) {
    http_response_code(404);
    header('Content-Type: text/plain; charset=utf-8');
    echo '404 Not Found';
    exit;
}

$ext = strtolower(pathinfo($realTarget, PATHINFO_EXTENSION));
$staticMimes = [
    'css'   => 'text/css; charset=utf-8',
    'js'    => 'application/javascript; charset=utf-8',
    'png'   => 'image/png',
    'gif'   => 'image/gif',
    'jpg'   => 'image/jpeg',
    'jpeg'  => 'image/jpeg',
    'svg'   => 'image/svg+xml',
    'ico'   => 'image/x-icon',
    'woff'  => 'font/woff',
    'woff2' => 'font/woff2',
    'ttf'   => 'font/ttf',
    'json'  => 'application/json; charset=utf-8',
];
if (isset($staticMimes[$ext])) {
    header('Content-Type: ' . $staticMimes[$ext]);
    header('X-Content-Type-Options: nosniff');
    readfile($realTarget);
    return true;
}

if ($ext === 'php') {
    chdir(dirname($realTarget));
    $_SERVER['SCRIPT_FILENAME'] = $realTarget;
    $_SERVER['SCRIPT_NAME'] = $uri;
    $_SERVER['DOCUMENT_ROOT'] = $realDocRoot;
    require $realTarget;
    return true;
}

http_response_code(403);
echo '403 Forbidden';
return true;
EOF_PMA_PHP
    chmod 0644 /etc/ferrousx/pma-router.php
  fi

  cat > /etc/systemd/system/ferrous-pma.service <<'EOF_PMA_SVC'
[Unit]
Description=FerrousPanel phpMyAdmin Internal Loopback Service (127.0.0.1:888 -> /phpadmin/)
After=network.target mariadb.service mysql.service
StartLimitIntervalSec=0

[Service]
Type=simple
User=www-data
Group=www-data
WorkingDirectory=/usr/share/phpmyadmin
Environment="PHP_CLI_SERVER_WORKERS=4"
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
ExecStart=/usr/bin/php -S 127.0.0.1:888 -t /usr/share/phpmyadmin /etc/ferrousx/pma-router.php
Restart=always
RestartSec=2
NoNewPrivileges=true
PrivateTmp=true
PrivateDevices=true
ProtectHome=true
ProtectSystem=strict
ReadWritePaths=-/var/lib/phpmyadmin /tmp /var/tmp
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectKernelLogs=true
ProtectControlGroups=true
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
RestrictNamespaces=true
RestrictRealtime=true
RestrictSUIDSGID=true
LockPersonality=true
SystemCallArchitectures=native
StandardOutput=journal
StandardError=journal
SyslogIdentifier=ferrous-pma

[Install]
WantedBy=multi-user.target
EOF_PMA_SVC
  systemctl daemon-reload 2>/dev/null || true

  if [ -d /usr/share/phpmyadmin ] && [ -x /usr/bin/php ]; then
    systemctl enable --now ferrous-pma.service 2>/dev/null || true
    systemctl restart ferrous-pma.service 2>/dev/null || true
  fi
}

ensure_ferrousx_runtime_files() {
  mkdir -p /etc/ferrousx/sites /etc/ferrousx/ssl /run/ferrousx /var/log/ferrousx /var/cache/ferrousx
  chmod 0750 /etc/ferrousx /etc/ferrousx/sites /etc/ferrousx/ssl /run/ferrousx

  if [ ! -f "${FERROUSX_ADMIN_TOKEN_FILE}" ]; then
    (umask 077 && openssl rand -hex 32 > "${FERROUSX_ADMIN_TOKEN_FILE}")
    chmod 0600 "${FERROUSX_ADMIN_TOKEN_FILE}"
  fi

  if [ ! -f "${FERROUSX_ENV_FILE}" ]; then
    cat > "${FERROUSX_ENV_FILE}" <<EOF_FX_ENV
# Managed by FerrousPanel — FerrousX dynamic runtime environment
FERROUSX_CONFIG=/etc/ferrousx/ferrousx.toml
FERROUSX_SITES_DIR=/etc/ferrousx/sites
FERROUSX_ENV_FILE=/etc/ferrousx/ferrousx.env
FERROUSX_CONTROL_SOCKET=/run/ferrousx/panel-admin.sock
FERROUSX_PANEL_UDS=/run/ferrousx/panel-admin.sock
FERROUSX_ADMIN_TOKEN_FILE=/etc/ferrousx/admin.token
FERROUSX_SERVER_IP=${VPS_PUBLIC_IP}
FERROUSX_ENABLE_PHPMYADMIN=true
FERROUSX_PHPMYADMIN_UPSTREAM=127.0.0.1:888
FERROUSX_WORKERS=4
FERROUSX_LOG_LEVEL=info
FERROUSX_ACME_WEBROOT=/www/acme
FERROUSX_TLS_DIR=/etc/ferrousx/ssl
FERROUSX_CACHE_DIR=/var/cache/ferrousx
FERROUSX_SERVER_TOKENS=false
EOF_FX_ENV
    chmod 0640 "${FERROUSX_ENV_FILE}"
  else
    if ! grep -q '^FERROUSX_SERVER_IP=' "${FERROUSX_ENV_FILE}" 2>/dev/null; then
      echo "FERROUSX_SERVER_IP=${VPS_PUBLIC_IP}" >> "${FERROUSX_ENV_FILE}"
    elif [ "${VPS_PUBLIC_IP}" != "127.0.0.1" ]; then
      sed -i "s|^FERROUSX_SERVER_IP=127\.0\.0\.1$|FERROUSX_SERVER_IP=${VPS_PUBLIC_IP}|" "${FERROUSX_ENV_FILE}"
    fi
    if ! grep -q '^FERROUSX_ENABLE_PHPMYADMIN=' "${FERROUSX_ENV_FILE}" 2>/dev/null; then
      echo "FERROUSX_ENABLE_PHPMYADMIN=true" >> "${FERROUSX_ENV_FILE}"
    fi
    if ! grep -q '^FERROUSX_PHPMYADMIN_UPSTREAM=' "${FERROUSX_ENV_FILE}" 2>/dev/null; then
      echo "FERROUSX_PHPMYADMIN_UPSTREAM=127.0.0.1:888" >> "${FERROUSX_ENV_FILE}"
    fi
  fi

  if [ ! -f /etc/ferrousx/ferrousx.toml ]; then
    cat > /etc/ferrousx/ferrousx.toml <<'EOF_FX'
# Managed by FerrousPanel — FerrousX master configuration
worker_threads = 4
bind_http = "0.0.0.0:80"
bind_https = "0.0.0.0:443"
sites_dir = "/etc/ferrousx/sites"
acme_webroot = "/www/acme"
acme_challenge_prefix = "/.well-known/acme-challenge/"
enable_phpmyadmin_route = true
phpmyadmin_upstream = "127.0.0.1:888"
server_tokens = false
EOF_FX
  else
    if ! grep -Eq '^[[:space:]]*enable_phpmyadmin_route[[:space:]]*=' /etc/ferrousx/ferrousx.toml 2>/dev/null; then
      echo 'enable_phpmyadmin_route = true' >> /etc/ferrousx/ferrousx.toml
    fi
    if ! grep -Eq '^[[:space:]]*phpmyadmin_upstream[[:space:]]*=' /etc/ferrousx/ferrousx.toml 2>/dev/null; then
      echo 'phpmyadmin_upstream = "127.0.0.1:888"' >> /etc/ferrousx/ferrousx.toml
    fi
  fi

  setup_phpmyadmin_proxy

  if [ -f /etc/profile.d/ferrous-panel.sh ] && ! grep -q '/etc/ferrousx/ferrousx.env' /etc/profile.d/ferrous-panel.sh 2>/dev/null; then
    cat >> /etc/profile.d/ferrous-panel.sh <<'EOF_FX_PROF'

export FERROUSX_ENV_FILE="/etc/ferrousx/ferrousx.env"
if [ -r "/etc/ferrousx/ferrousx.env" ]; then
  set -a
  # shellcheck disable=SC1091
  . /etc/ferrousx/ferrousx.env
  set +a
fi
EOF_FX_PROF
  fi

  cat > /etc/systemd/system/ferrousx.service <<'EOF_FX_SVC'
[Unit]
Description=FerrousX Pure-Rust HTTP/HTTPS Reverse Proxy & FastCGI Web Server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=root
Environment=FERROUSX_CONFIG=/etc/ferrousx/ferrousx.toml
EnvironmentFile=-/etc/ferrousx/ferrousx.env
EnvironmentFile=-/etc/ferrous-panel/panel.env
RuntimeDirectory=ferrousx
RuntimeDirectoryMode=0750
ExecStartPre=/usr/local/bin/ferrousx-bin --check-config --config ${FERROUSX_CONFIG}
ExecStart=/usr/local/bin/ferrousx-bin --config ${FERROUSX_CONFIG}
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=3
LimitNOFILE=65535
NoNewPrivileges=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF_FX_SVC
  systemctl daemon-reload 2>/dev/null || true
}

update_ferrousx_from_github() {
  echo -e "${BLUE}${BOLD}[*] Updating FerrousX from GitHub (${FERROUSX_REPO}@${FERROUSX_BRANCH})...${NC}"
  local fx_installed=false

  if command -v cargo >/dev/null 2>&1 && command -v git >/dev/null 2>&1; then
    mkdir -p "$(dirname "${FERROUSX_SRC_DIR}")"
    if (
      if [ -d "${FERROUSX_SRC_DIR}/.git" ]; then
        git -C "${FERROUSX_SRC_DIR}" fetch --depth 1 origin "${FERROUSX_BRANCH}" &&
        git -C "${FERROUSX_SRC_DIR}" reset --hard "origin/${FERROUSX_BRANCH}"
      else
        rm -rf "${FERROUSX_SRC_DIR}" &&
        git clone --depth 1 --branch "${FERROUSX_BRANCH}" "https://github.com/${FERROUSX_REPO}.git" "${FERROUSX_SRC_DIR}"
      fi
    ); then
      local fx_commit
      fx_commit="$(git -C "${FERROUSX_SRC_DIR}" rev-parse --short HEAD 2>/dev/null || echo 'main')"
      if [ -f "${FERROUSX_SRC_DIR}/Cargo.toml" ] && grep -q '^rust-version = ' "${FERROUSX_SRC_DIR}/Cargo.toml"; then
        sed -i '/^rust-version = /d' "${FERROUSX_SRC_DIR}/Cargo.toml"
      fi
      if [ -f "${FERROUSX_SRC_DIR}/crates/ferrousx-core/Cargo.toml" ] && ! grep -Eq '^[[:space:]]*anyhow[[:space:]]*[.=]' "${FERROUSX_SRC_DIR}/crates/ferrousx-core/Cargo.toml"; then
        echo -e "${CYAN}[*] Applying compatibility patch: adding anyhow.workspace = true to upstream ferrousx-core/Cargo.toml...${NC}"
        awk '/^\[dependencies\]$/ { print; print "anyhow.workspace = true"; next } { print }' \
          "${FERROUSX_SRC_DIR}/crates/ferrousx-core/Cargo.toml" > "${FERROUSX_SRC_DIR}/crates/ferrousx-core/Cargo.toml.tmp" && \
          mv "${FERROUSX_SRC_DIR}/crates/ferrousx-core/Cargo.toml.tmp" "${FERROUSX_SRC_DIR}/crates/ferrousx-core/Cargo.toml"
      fi
      if [ -f "${FERROUSX_SRC_DIR}/crates/ferrousx-bin/Cargo.toml" ]; then
        for _dep in tokio-rustls toml; do
          if ! grep -Eq "^[[:space:]]*${_dep}[[:space:]]*[.=]" "${FERROUSX_SRC_DIR}/crates/ferrousx-bin/Cargo.toml"; then
            echo -e "${CYAN}[*] Applying compatibility patch: adding ${_dep}.workspace = true to upstream ferrousx-bin/Cargo.toml...${NC}"
            awk -v dep="${_dep}.workspace = true" '/^\[dependencies\]$/ { print; print dep; next } { print }' \
              "${FERROUSX_SRC_DIR}/crates/ferrousx-bin/Cargo.toml" > "${FERROUSX_SRC_DIR}/crates/ferrousx-bin/Cargo.toml.tmp" && \
              mv "${FERROUSX_SRC_DIR}/crates/ferrousx-bin/Cargo.toml.tmp" "${FERROUSX_SRC_DIR}/crates/ferrousx-bin/Cargo.toml"
          fi
        done
      fi
      if [ -f "${FERROUSX_SRC_DIR}/Cargo.toml" ] && grep -q 'h3-quinn = "0\.0\.7"' "${FERROUSX_SRC_DIR}/Cargo.toml" && grep -q 'quinn = { version = "0\.11\.6"' "${FERROUSX_SRC_DIR}/Cargo.toml"; then
        echo -e "${CYAN}[*] Applying compatibility patch: pinning quinn = 0.11.6 & quinn-proto = 0.11.9 for h3-quinn 0.0.7...${NC}"
        sed -i 's/quinn = { version = "0\.11\.6"/quinn = { version = "=0.11.6"/' "${FERROUSX_SRC_DIR}/Cargo.toml"
        git -C "${FERROUSX_SRC_DIR}" ls-files --error-unmatch Cargo.lock >/dev/null 2>&1 || rm -f "${FERROUSX_SRC_DIR}/Cargo.lock"
        (cd "${FERROUSX_SRC_DIR}" && cargo generate-lockfile && cargo update -p quinn-proto --precise 0.11.9) || true
      fi
      if [ -f "${FERROUSX_SRC_DIR}/crates/ferrousx-http/src/server.rs" ]; then
        if grep -q 'mut stream: up_stream,' "${FERROUSX_SRC_DIR}/crates/ferrousx-http/src/server.rs"; then
          echo -e "${CYAN}[*] Applying compatibility patch: fixing struct pattern mutability in upstream ferrousx-http/src/server.rs...${NC}"
          sed -i 's/mut stream: up_stream,/stream: mut up_stream,/g' "${FERROUSX_SRC_DIR}/crates/ferrousx-http/src/server.rs"
        fi
        if grep -q 'while let Some(chunk) = match decoder\.next_decoded()' "${FERROUSX_SRC_DIR}/crates/ferrousx-http/src/server.rs"; then
          echo -e "${CYAN}[*] Applying compatibility patch: fixing chunked decoder loop in upstream ferrousx-http/src/server.rs...${NC}"
          awk '
            /while let Some\(chunk\) = match decoder\.next_decoded\(\) \{/ {
              print "                    let mut stream_err = false;"
              print "                    loop {"
              print "                        match decoder.next_decoded() {"
              print "                            Ok(Some(chunk)) => {"
              print "                                write_http1_stream_chunk(stream, &chunk, use_chunked, stats).await?;"
              print "                            }"
              print "                            Ok(None) => break,"
              print "                            Err(_) => {"
              print "                                stream_err = true;"
              print "                                break;"
              print "                            }"
              print "                        }"
              print "                    }"
              print "                    if stream_err || decoder.is_done() {"
              print "                        break;"
              print "                    }"
              skip = 8
              next
            }
            skip > 0 { skip--; next }
            { print }
          ' "${FERROUSX_SRC_DIR}/crates/ferrousx-http/src/server.rs" > "${FERROUSX_SRC_DIR}/crates/ferrousx-http/src/server.rs.tmp" && \
            mv "${FERROUSX_SRC_DIR}/crates/ferrousx-http/src/server.rs.tmp" "${FERROUSX_SRC_DIR}/crates/ferrousx-http/src/server.rs"
        fi
      fi
      if [ -f "${FERROUSX_SRC_DIR}/crates/ferrousx-proxy/src/fastcgi.rs" ] && grep -q 'let mut saw_end_request = false;' "${FERROUSX_SRC_DIR}/crates/ferrousx-proxy/src/fastcgi.rs"; then
        sed -i 's/let mut saw_end_request = false;/let saw_end_request: bool;/' "${FERROUSX_SRC_DIR}/crates/ferrousx-proxy/src/fastcgi.rs"
      fi
      if [ -f "${FERROUSX_SRC_DIR}/crates/ferrousx-bin/src/cli.rs" ] && grep -q 'http://<Server-IP>/phpadmin/' "${FERROUSX_SRC_DIR}/crates/ferrousx-bin/src/cli.rs"; then
        echo -e "${CYAN}[*] Applying compatibility patch: auto-detecting VPS IP/domain for phpMyAdmin in upstream ferrousx-bin/src/cli.rs...${NC}"
        awk '
          /^fn print_admin_overview\(socket: &Path\) \{/ {
            print "fn resolve_vps_public_host(file_env: &std::collections::HashMap<String, String>) -> String {"
            print "    if let Some(domain) = resolve_env_var(\"FERROUS_CUSTOM_DOMAIN\", file_env) {"
            print "        let d = domain.trim();"
            print "        if !d.is_empty() && d != \"127.0.0.1\" && d != \"localhost\" {"
            print "            return d.to_string();"
            print "        }"
            print "    }"
            print "    if let Ok(panel_env) = std::fs::read_to_string(\"/etc/ferrous-panel/panel.env\") {"
            print "        for line in panel_env.lines() {"
            print "            if let Some(v) = line.trim().strip_prefix(\"FERROUS_CUSTOM_DOMAIN=\") {"
            print "                let d = v.trim().trim_matches(|c: char| c as u32 == 34 || c as u32 == 39);"
            print "                if !d.is_empty() && d != \"127.0.0.1\" && d != \"localhost\" {"
            print "                    return d.to_string();"
            print "                }"
            print "            }"
            print "        }"
            print "    }"
            print "    if let Some(ip) = resolve_env_var(\"FERROUSX_SERVER_IP\", file_env) {"
            print "        let s = ip.trim();"
            print "        if !s.is_empty() && s != \"127.0.0.1\" && s != \"localhost\" {"
            print "            return s.to_string();"
            print "        }"
            print "    }"
            print "    if let Ok(out) = Command::new(\"hostname\").arg(\"-I\").output() {"
            print "        if out.status.success() {"
            print "            let stdout = String::from_utf8_lossy(&out.stdout);"
            print "            for tok in stdout.split_whitespace() {"
            print "                if tok.contains(\".\") && !tok.starts_with(\"127.\") {"
            print "                    return tok.to_string();"
            print "                }"
            print "            }"
            print "        }"
            print "    }"
            print "    \"127.0.0.1\".to_string()"
            print "}"
            print ""
            print
            next
          }
          /phpMyAdmin Proxy  : http:\/\/<Server-IP>\/phpadmin\// {
            print "    let vps_host = resolve_vps_public_host(&file_env);"
            print "    println!(\" • phpMyAdmin Proxy  : http://{vps_host}/phpadmin/ (backend 127.0.0.1:888)\");"
            next
          }
          /Khởi động lại phpMyAdmin nội bộ \(127\.0\.0\.1:888\)/ {
            print "        let pma_host = resolve_vps_public_host(&load_default_env_map());"
            print "        let pma_label = format!(\"Khởi động lại phpMyAdmin ({pma_host}/phpadmin/)\");"
            print "        println!(\"\\x1b[1;36m║\\x1b[0m  \\x1b[1;33m[5]\\x1b[0m {:<61} \\x1b[1;36m║\\x1b[0m\", pma_label);"
            next
          }
          /Đã khởi động lại ferrous-pma\.service \(127\.0\.0\.1:888\)!/ {
            print "                        \"\\x1b[1;32m✓ Đã khởi động lại ferrous-pma.service -> http://{}/phpadmin/\\x1b[0m\","
            print "                        resolve_vps_public_host(&load_default_env_map())"
            next
          }
          { print }
        ' "${FERROUSX_SRC_DIR}/crates/ferrousx-bin/src/cli.rs" > "${FERROUSX_SRC_DIR}/crates/ferrousx-bin/src/cli.rs.tmp" && \
          mv "${FERROUSX_SRC_DIR}/crates/ferrousx-bin/src/cli.rs.tmp" "${FERROUSX_SRC_DIR}/crates/ferrousx-bin/src/cli.rs"
      fi
      echo -e "${CYAN}[*] Compiling upstream FerrousX (${fx_commit}) via Cargo (ferrousx-bin + ferrousx-ctl)...${NC}"
      if (cd "${FERROUSX_SRC_DIR}" && cargo build --release --bin ferrousx-bin --bin ferrousx-ctl); then
        if [ -x "${FERROUSX_SRC_DIR}/target/release/ferrousx-bin" ]; then
          install -m 0755 "${FERROUSX_SRC_DIR}/target/release/ferrousx-bin" "${BIN_FERROUSX}"
          fx_installed=true
        fi
        if [ -x "${FERROUSX_SRC_DIR}/target/release/ferrousx-ctl" ]; then
          install -m 0755 "${FERROUSX_SRC_DIR}/target/release/ferrousx-ctl" "${BIN_FERROUSX_CTL}"
          ln -sf "${BIN_FERROUSX_CTL}" /usr/local/bin/fx
        fi
        echo -e "${GREEN}[+] Installed upstream FerrousX (${fx_commit}) -> ${BIN_FERROUSX}${NC}"
      fi
    fi
  fi

  if [ "$fx_installed" = false ] && [ -d "${SRC_DIR}/crates/ferrousx" ] && command -v cargo >/dev/null 2>&1; then
    echo -e "${YELLOW}[!] Upstream FerrousX build unavailable; building embedded workspace crate (-p ferrousx)...${NC}"
    (
      cd "${SRC_DIR}"
      cargo build --release -p ferrousx
    )
    if [ -x "${SRC_DIR}/target/release/ferrousx-bin" ]; then
      install -m 0755 "${SRC_DIR}/target/release/ferrousx-bin" "${BIN_FERROUSX}"
      fx_installed=true
    fi
    if [ -x "${SRC_DIR}/target/release/ferrousx-ctl" ]; then
      install -m 0755 "${SRC_DIR}/target/release/ferrousx-ctl" "${BIN_FERROUSX_CTL}"
      ln -sf "${BIN_FERROUSX_CTL}" /usr/local/bin/fx
    fi
  fi

  if [ "$fx_installed" = false ]; then
    echo -e "${CYAN}[*] Downloading prebuilt ferrousx-bin release (${RELEASE_TAG}, ${ARCH})...${NC}"
    curl -fsSL --connect-timeout 10 -o "${TMP_DIR}/SHA256SUMS.txt" "${RELEASE_BASE}/SHA256SUMS.txt" 2>/dev/null || rm -f "${TMP_DIR}/SHA256SUMS.txt"
    curl -fsSL --retry 3 --connect-timeout 10 -o "${TMP_DIR}/ferrousx-bin" "${RELEASE_BASE}/ferrousx-bin-linux-${ARCH}"
    if [ -f "${TMP_DIR}/SHA256SUMS.txt" ] && grep -q " ferrousx-bin-linux-${ARCH}\$" "${TMP_DIR}/SHA256SUMS.txt"; then
      (cd "$TMP_DIR" && mv ferrousx-bin "ferrousx-bin-linux-${ARCH}" && grep " ferrousx-bin-linux-${ARCH}\$" SHA256SUMS.txt | sha256sum -c - && mv "ferrousx-bin-linux-${ARCH}" ferrousx-bin)
    fi
    install -m 0755 "${TMP_DIR}/ferrousx-bin" "${BIN_FERROUSX}"
  fi

  ensure_ferrousx_runtime_files

  if [ -x "${BIN_FERROUSX}" ]; then
    "${BIN_FERROUSX}" --check-config --config /etc/ferrousx/ferrousx.toml || true
  fi

  if systemctl is-active --quiet ferrousx.service 2>/dev/null; then
    if command -v ufw >/dev/null 2>&1 && ufw status 2>/dev/null | grep -qi "Status: active"; then
      ufw allow 443/udp comment 'HTTP3-QUIC' >/dev/null 2>&1 || true
    fi
    systemctl restart ferrousx.service
    if [ -x "${BIN_FERROUSX_CTL}" ]; then
      "${BIN_FERROUSX_CTL}" sync >/dev/null 2>&1 || true
    fi
    echo -e "${GREEN}[+] Restarted ferrousx.service with updated binary & ferrousx.env.${NC}"
  else
    echo -e "${CYAN}[*] ferrousx.service is installed (inactive — switch active engine via 'sudo fp webserver ferrousx').${NC}"
  fi
}

if [ "$UPDATE_PANEL" = true ]; then
  if [ -d "${SRC_DIR}/.git" ] && command -v cargo >/dev/null 2>&1; then
    echo -e "${BLUE}${BOLD}[*] Pulling latest FerrousPanel (${REPO_SLUG}@${GIT_BRANCH}) & rebuilding with Cargo...${NC}"
    git -C "${SRC_DIR}" fetch --depth 1 origin "${GIT_BRANCH}"
    git -C "${SRC_DIR}" reset --hard "origin/${GIT_BRANCH}"

    # Refresh embedded static frontend bundle (dist/) directly on the VPS before compiling
    mkdir -p "${TMP_DIR}/dist_new"
    if curl -fsSL --retry 2 --connect-timeout 10 -o "${TMP_DIR}/ferrous-dist.tar.gz" "${RELEASE_BASE}/ferrous-dist.tar.gz" 2>/dev/null \
      && tar -xzf "${TMP_DIR}/ferrous-dist.tar.gz" -C "${TMP_DIR}/dist_new" 2>/dev/null \
      && [ -f "${TMP_DIR}/dist_new/index.html" ]; then
      rm -rf "${SRC_DIR}/dist"
      mv "${TMP_DIR}/dist_new" "${SRC_DIR}/dist"
      echo -e "${GREEN}[+] Synced latest static UI bundle into ${SRC_DIR}/dist.${NC}"
    elif command -v npm >/dev/null 2>&1 && [ -f "${SRC_DIR}/package.json" ]; then
      echo -e "${CYAN}[*] Building updated frontend UI locally via npm...${NC}"
      (cd "${SRC_DIR}" && npm install --no-audit --no-fund && npm run build) || true
    fi

    (
      cd "${SRC_DIR}"
      cargo build --release -p ferrous-panel
    )

    install -m 0755 "${SRC_DIR}/target/release/ferrous-panel" "${BIN_PANEL}"
  else
    echo -e "${CYAN}[*] Downloading updated FerrousPanel release (${RELEASE_TAG}, ${ARCH})...${NC}"
    curl -fsSL --connect-timeout 10 -o "${TMP_DIR}/SHA256SUMS.txt" "${RELEASE_BASE}/SHA256SUMS.txt" 2>/dev/null || rm -f "${TMP_DIR}/SHA256SUMS.txt"

    curl -fsSL --retry 3 --connect-timeout 10 -o "${TMP_DIR}/ferrous-panel" "${RELEASE_BASE}/ferrous-panel-linux-${ARCH}"
    if [ -f "${TMP_DIR}/SHA256SUMS.txt" ] && grep -q " ferrous-panel-linux-${ARCH}\$" "${TMP_DIR}/SHA256SUMS.txt"; then
      (cd "$TMP_DIR" && mv ferrous-panel "ferrous-panel-linux-${ARCH}" && grep " ferrous-panel-linux-${ARCH}\$" SHA256SUMS.txt | sha256sum -c - && mv "ferrous-panel-linux-${ARCH}" ferrous-panel)
    fi
    install -m 0755 "${TMP_DIR}/ferrous-panel" "${BIN_PANEL}"
  fi

  ln -sf "${BIN_PANEL}" /usr/local/bin/fp
  ln -sf "${BIN_PANEL}" /usr/local/bin/ferrous
  systemctl restart ferrous-panel.service 2>/dev/null || true
fi

update_nginx_engine() {
  echo -e "${BLUE}${BOLD}[*] Syncing & verifying Nginx web server on VPS...${NC}"
  if ! command -v nginx >/dev/null 2>&1; then
    if [ "$FORCE_NGINX" = true ] && command -v apt-get >/dev/null 2>&1; then
      echo -e "${CYAN}[*] Installing nginx package via apt-get...${NC}"
      export DEBIAN_FRONTEND=noninteractive
      apt-get update -y -q
      apt-get install -y -q nginx
    else
      return 0
    fi
  fi

  mkdir -p /etc/nginx/sites-available /etc/nginx/sites-enabled /etc/nginx/conf.d /etc/nginx/snippets /www/acme/.well-known/acme-challenge
  chown -R www-data:www-data /www/acme 2>/dev/null || true

  if [ ! -f /etc/nginx/snippets/fastcgi-php.conf ]; then
    cat > /etc/nginx/snippets/fastcgi-php.conf <<'EOF_NGX_PHP'
# Managed by FerrousPanel — fallback FastCGI PHP snippet
fastcgi_split_path_info ^(.+?\.php)(/.*)$;
try_files $fastcgi_script_name =404;
set $path_info $fastcgi_path_info;
fastcgi_param PATH_INFO $path_info;
fastcgi_index index.php;
include fastcgi.conf;
EOF_NGX_PHP
  fi

  if [ ! -f /etc/nginx/conf.d/ferrous-defaults.conf ]; then
    {
      echo "# Managed by FerrousPanel — safe global Nginx defaults"
      if ! grep -Eq '^[[:space:]]*client_max_body_size[[:space:]]' /etc/nginx/nginx.conf 2>/dev/null; then
        echo "client_max_body_size 128m;"
      fi
      if ! grep -Eq '^[[:space:]]*server_names_hash_bucket_size[[:space:]]' /etc/nginx/nginx.conf 2>/dev/null; then
        echo "server_names_hash_bucket_size 128;"
      fi
    } > /etc/nginx/conf.d/ferrous-defaults.conf
    if ! nginx -t >/dev/null 2>&1; then
      rm -f /etc/nginx/conf.d/ferrous-defaults.conf
    fi
  fi

  local ngx_default_target
  ngx_default_target="$(readlink -f /etc/nginx/sites-enabled/default 2>/dev/null || true)"
  if [ "$ngx_default_target" = "/etc/nginx/sites-available/default" ] || \
     { [ "$ngx_default_target" = "/etc/nginx/sites-available/ferrous-default.conf" ] && \
       head -n 1 /etc/nginx/sites-available/ferrous-default.conf 2>/dev/null | grep -q 'Managed by FerrousPanel' && \
       ! grep -q '/phpadmin/' /etc/nginx/sites-available/ferrous-default.conf 2>/dev/null; }; then
    cat > /etc/nginx/sites-available/ferrous-default.conf <<'EOF_NGX_DEF'
# Managed by FerrousPanel — Default catch-all vhost with ACME HTTP-01 webroot & phpMyAdmin reverse proxy
server {
    listen 80 default_server;
    listen [::]:80 default_server;
    server_name _;

    root /www/wwwroot;
    server_tokens off;

    location ^~ /.well-known/acme-challenge/ {
        root /www/acme;
        default_type "text/plain";
        try_files $uri =404;
    }

    location = /phpadmin {
        return 301 $scheme://$http_host/phpadmin/;
    }

    location ^~ /phpadmin/ {
        proxy_pass http://127.0.0.1:888;
        proxy_set_header Host $http_host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    location = /phpmyadmin {
        return 301 $scheme://$http_host/phpmyadmin/;
    }

    location ^~ /phpmyadmin/ {
        proxy_pass http://127.0.0.1:888;
        proxy_set_header Host $http_host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    location / {
        try_files $uri $uri/ =404;
    }
}
EOF_NGX_DEF
    ln -sf /etc/nginx/sites-available/ferrous-default.conf /etc/nginx/sites-enabled/default
  fi

  setup_phpmyadmin_proxy

  if nginx -t; then
    if systemctl is-active --quiet nginx.service 2>/dev/null; then
      systemctl reload nginx.service
      echo -e "${GREEN}[+] Validated & gracefully reloaded nginx.service.${NC}"
    elif [ "$FORCE_NGINX" = true ] && ! systemctl is-active --quiet ferrousx.service 2>/dev/null; then
      systemctl enable --now nginx.service
      echo -e "${GREEN}[+] Validated, enabled & started nginx.service.${NC}"
    else
      echo -e "${CYAN}[*] nginx config syntax OK (service inactive — switch active engine via 'sudo fp webserver nginx').${NC}"
    fi
  else
    echo -e "${YELLOW}[!] nginx -t reported an issue; leaving existing running process untouched.${NC}"
  fi
}

if [ "$FORCE_FERROUSX" = true ] || [ -x "${BIN_FERROUSX}" ]; then
  update_ferrousx_from_github
fi

if [ "$FORCE_NGINX" = true ] || systemctl is-active --quiet nginx.service 2>/dev/null; then
  update_nginx_engine
fi

echo -e "${GREEN}${BOLD}Update completed cleanly. State in /var/lib/ferrous-panel, /etc/nginx, and /etc/ferrousx preserved.${NC}"
