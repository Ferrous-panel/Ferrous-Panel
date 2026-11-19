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
SRC_DIR="/usr/local/src/ferrous-panel"
FERROUSX_SRC_DIR="/usr/local/src/ferrousx"
BIN_PANEL="/usr/local/bin/ferrous-panel"
BIN_FERROUSX="/usr/local/bin/ferrousx-bin"
BIN_FERROUSX_CTL="/usr/local/bin/ferrousx-ctl"
FERROUSX_ENV_FILE="/etc/ferrousx/ferrousx.env"
FERROUSX_ADMIN_TOKEN_FILE="/etc/ferrousx/admin.token"

UPDATE_PANEL=true
FORCE_FERROUSX=false

usage() {
  cat <<'EOF_HELP'
FerrousPanel & FerrousX 1-Command Updater

Usage:
  sudo bash update.sh [OPTIONS] [RELEASE_TAG]

Options:
  --ferrousx                   Sync & rebuild latest FerrousX from github.com/Ferrous-panel/FerrousX
  --ferrousx-only              Update ONLY FerrousX from GitHub (do not rebuild ferrous-panel)
  --ferrousx-branch <BRANCH>   Upstream FerrousX Git branch (default: main)
  --branch <BRANCH>            FerrousPanel Git branch (default: main)
  --version <TAG|latest>       GitHub Release tag for prebuilt fallback (default: latest)
  -h, --help                   Show this help message
EOF_HELP
}

while [[ $# -gt 0 ]]; do
  case "$1" in
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
      if [ "${2:-}" = "ferrousx" ]; then
        FORCE_FERROUSX=true
      fi
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

ensure_ferrousx_runtime_files() {
  mkdir -p /etc/ferrousx/sites /etc/ferrousx/ssl /run/ferrousx /var/log/ferrousx /var/cache/ferrousx
  chmod 0750 /etc/ferrousx /etc/ferrousx/sites /etc/ferrousx/ssl /run/ferrousx

  if [ ! -f "${FERROUSX_ADMIN_TOKEN_FILE}" ]; then
    (umask 077 && openssl rand -hex 32 > "${FERROUSX_ADMIN_TOKEN_FILE}")
    chmod 0600 "${FERROUSX_ADMIN_TOKEN_FILE}"
  fi

  if [ ! -f "${FERROUSX_ENV_FILE}" ]; then
    cat > "${FERROUSX_ENV_FILE}" <<'EOF_FX_ENV'
# Managed by FerrousPanel — FerrousX dynamic runtime environment
FERROUSX_CONFIG=/etc/ferrousx/ferrousx.toml
FERROUSX_SITES_DIR=/etc/ferrousx/sites
FERROUSX_ENV_FILE=/etc/ferrousx/ferrousx.env
FERROUSX_PANEL_UDS=/run/ferrousx/panel-admin.sock
FERROUSX_ADMIN_TOKEN_FILE=/etc/ferrousx/admin.token
FERROUSX_WORKERS=4
FERROUSX_LOG_LEVEL=info
FERROUSX_ACME_WEBROOT=/www/acme
FERROUSX_TLS_DIR=/etc/ferrousx/ssl
FERROUSX_CACHE_DIR=/var/cache/ferrousx
FERROUSX_SERVER_TOKENS=false
EOF_FX_ENV
    chmod 0640 "${FERROUSX_ENV_FILE}"
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
server_tokens = false
EOF_FX
  fi

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
    systemctl restart ferrousx.service
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

if [ "$FORCE_FERROUSX" = true ] || [ -x "${BIN_FERROUSX}" ]; then
  update_ferrousx_from_github
fi

echo -e "${GREEN}${BOLD}Update completed cleanly. State in /var/lib/ferrous-panel and /etc/ferrousx preserved.${NC}"
