#!/usr/bin/env bash
# ==============================================================================
# FerrousPanel — 1-Command Open-Source Installer (Ubuntu 22.04 / 24.04 LTS)
#
# Automatically:
#   1. Provisions a 2GB /swapfile if RAM < 4GB and no swap is active (OOM guard)
#   2. Installs system packages + Rust stable (1.89+) toolchain (rustup + cargo)
#   3. Fetches prebuilt static UI (dist/) & compiles ferrous-panel via Cargo
#      (zero Node.js or npm needed on the VPS; embeds dist/ via rust-embed)
#   4. Configures Nginx or FerrousX, Argon2id 0600 credentials, systemd & UFW
#
# 1-Command Install (default Nginx engine):
#   curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash
#
# 1-Command Install with FerrousX (Pure-Rust HTTP/HTTPS/FastCGI/WS engine):
#   curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash -s -- --engine ferrousx
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
GIT_BRANCH="${FERROUS_BRANCH:-main}"
FERROUSX_REPO="${FERROUSX_REPO:-Ferrous-panel/FerrousX}"
FERROUSX_BRANCH="${FERROUSX_BRANCH:-main}"
RELEASE_TAG="${FERROUS_VERSION:-latest}"
RUST_TOOLCHAIN="${FERROUS_RUST_VERSION:-stable}"
PANEL_BIND="${FERROUS_BIND:-0.0.0.0}"
PANEL_PORT="${FERROUS_PORT:-8443}"
SECURITY_ENTRANCE="${FERROUS_SECURITY_ENTRANCE:-/panel}"
ADMIN_USER="${FERROUS_ADMIN_USER:-admin_ferrous}"
ADMIN_USER_EXPLICIT=false
CUSTOM_ADMIN_PASS=""
CUSTOM_DOMAIN="${FERROUS_CUSTOM_DOMAIN:-}"
WEB_ENGINE="${FERROUS_ENGINE:-nginx}"
INTERACTIVE_MODE=false
UNATTENDED_MODE=false
AUTO_SWAP=true
PREBUILT_BINARY_ONLY=false

DATA_DIR="/var/lib/ferrous-panel"
ENV_FILE="/etc/ferrous-panel/panel.env"
FERROUSX_ENV_FILE="/etc/ferrousx/ferrousx.env"
FERROUSX_ADMIN_TOKEN_FILE="/etc/ferrousx/admin.token"
WWWROOT="/www/wwwroot"
BACKUPS_DIR="/var/backups/ferrous-panel"
SRC_DIR="/usr/local/src/ferrous-panel"
FERROUSX_SRC_DIR="/usr/local/src/ferrousx"
BIN_PANEL="/usr/local/bin/ferrous-panel"
BIN_FERROUSX="/usr/local/bin/ferrousx-bin"
BIN_FERROUSX_CTL="/usr/local/bin/ferrousx-ctl"

usage() {
  cat <<'EOF_HELP'
FerrousPanel 1-Command Installer (Ubuntu 22.04 / 24.04 LTS)

Usage:
  curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash
  curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/install.sh | sudo bash -s -- [OPTIONS]

Options:
  --engine <nginx|ferrousx>   Frontend web server engine (default: nginx)
  --bind <IP>                 Listen address (default: 0.0.0.0 for direct VPS IP access)
  --local-only                Bind strictly to 127.0.0.1 (access via SSH tunnel only)
  --port <PORT>               Listen TCP port 1..65535 (default: 8443)
  --entrance <PATH>           Security entrance path (default: /panel)
  --admin-user <USER>         Initial admin username (default: admin_ferrous)
  --domain <DOMAIN>           Optional custom panel domain
  --branch <BRANCH>           Git branch to build from (default: main)
  --version <TAG|latest>      GitHub Release version tag (default: latest)
  --prebuilt-binary           Download prebuilt binary instead of compiling with Cargo
  --no-swap                   Do not auto-create 2GB /swapfile on low-RAM VPS
  --interactive               Prompt interactively for engine and admin account setup
  -y, --yes, --unattended     Skip interactive prompts (auto-generate credentials)
  -h, --help                  Show this help message
EOF_HELP
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --admin-pass|--password|--sql-pass)
      echo -e "${RED}[ERROR] Passing passwords via CLI flags is forbidden for security (leaks in process list).${NC}" >&2
      echo -e "${YELLOW}First-run admin password is generated from OsRng once and stored as an Argon2id hash (0600).${NC}" >&2
      echo -e "${YELLOW}Use 'sudo fp passwd' after installation to change it interactively.${NC}" >&2
      exit 1
      ;;
    --admin-user) ADMIN_USER="$2"; ADMIN_USER_EXPLICIT=true; shift 2 ;;
    --bind) PANEL_BIND="$2"; shift 2 ;;
    --local-only) PANEL_BIND="127.0.0.1"; shift ;;
    --port) PANEL_PORT="$2"; shift 2 ;;
    --entrance) SECURITY_ENTRANCE="$2"; shift 2 ;;
    --domain|--custom-domain) CUSTOM_DOMAIN="$2"; shift 2 ;;
    --engine) WEB_ENGINE="$2"; shift 2 ;;
    --branch) GIT_BRANCH="$2"; shift 2 ;;
    --version) RELEASE_TAG="$2"; shift 2 ;;
    --prebuilt-binary) PREBUILT_BINARY_ONLY=true; shift ;;
    --no-swap) AUTO_SWAP=false; shift ;;
    --interactive) INTERACTIVE_MODE=true; UNATTENDED_MODE=false; shift ;;
    -y|--yes|--unattended) INTERACTIVE_MODE=false; UNATTENDED_MODE=true; shift ;;
    -h|--help) usage; exit 0 ;;
    *)
      echo -e "${RED}[ERROR] Unknown option: $1${NC}" >&2
      usage >&2
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

VPS_LOCAL_IP="$(detect_local_ip)"
VPS_PUBLIC_IP="$(detect_public_ip)"
echo -e "${BLUE}[*] Detected VPS IP: ${BOLD}${VPS_PUBLIC_IP}${NC}${BLUE} (Local: ${VPS_LOCAL_IP})${NC}"

HAS_TTY=false
if [ -r /dev/tty ] && [ -w /dev/tty ]; then
  HAS_TTY=true
fi

if [ "$INTERACTIVE_MODE" = true ] && [ "$HAS_TTY" = true ]; then
  echo ""
  echo -e "${BOLD}Select Frontend Web Server Engine:${NC}"
  echo "  1) nginx    (apt install nginx)"
  echo "  2) ferrousx (Pure-Rust HTTP/HTTPS/FastCGI/WebSocket binary)"
  read -t 15 -rp "Choose [1/2, default ${WEB_ENGINE}]: " engine_choice </dev/tty || true
  case "${engine_choice:-}" in
    1|nginx) WEB_ENGINE="nginx" ;;
    2|ferrousx) WEB_ENGINE="ferrousx" ;;
  esac
fi

if [ "$UNATTENDED_MODE" = false ] && [ "$HAS_TTY" = true ]; then
  if [ "$ADMIN_USER_EXPLICIT" = false ] && [ -z "${FERROUS_ADMIN_USER:-}" ]; then
    echo ""
    echo -e "${CYAN}${BOLD}[*] Admin Account Setup (press Enter to use defaults / auto-generate):${NC}"
    read -t 15 -rp "  Admin username [${ADMIN_USER}] (15s timeout): " input_user </dev/tty || true
    input_user="$(echo "${input_user:-}" | tr -d '[:space:]')"
    if [ -n "$input_user" ]; then
      if [[ "$input_user" =~ ^[A-Za-z0-9_-]{3,64}$ ]]; then
        ADMIN_USER="$input_user"
      else
        echo -e "${YELLOW}[!] Invalid username '${input_user}' (must be 3..=64 chars [A-Za-z0-9_-]); keeping '${ADMIN_USER}'.${NC}"
      fi
    fi

    if [ ! -f "${DATA_DIR}/admin.credentials.json" ]; then
      read -t 15 -rsp "  Admin password (8+ chars, Enter = auto-generate 24-char OsRng password): " input_pass </dev/tty || true
      echo "" >/dev/tty 2>/dev/null || true
      if [ -n "${input_pass:-}" ]; then
        if [ "${#input_pass}" -ge 8 ] && [ "${#input_pass}" -le 256 ]; then
          read -t 15 -rsp "  Confirm Admin password: " confirm_pass </dev/tty || true
          echo "" >/dev/tty 2>/dev/null || true
          if [ "$input_pass" = "${confirm_pass:-}" ]; then
            CUSTOM_ADMIN_PASS="$input_pass"
            echo -e "${GREEN}[+] Custom Admin password accepted (will be hashed with Argon2id 0600).${NC}"
          else
            echo -e "${YELLOW}[!] Passwords did not match; falling back to auto-generated OsRng password.${NC}"
          fi
        else
          echo -e "${YELLOW}[!] Password must be 8..=256 chars; falling back to auto-generated OsRng password.${NC}"
        fi
      fi
    fi
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

# ==============================================================================
# [1/7] Memory & 2GB Swap Protection (prevents Cargo OOM on 1GB-2GB RAM VPS)
# ==============================================================================
echo -e "${BLUE}[1/7] Checking system memory & swap protection...${NC}"
MEM_TOTAL_KB="$(awk '/^MemTotal:/ {print $2}' /proc/meminfo 2>/dev/null || echo 4194304)"
SWAP_TOTAL_KB="$(awk '/^SwapTotal:/ {print $2}' /proc/meminfo 2>/dev/null || echo 0)"
MEM_TOTAL_MB=$((MEM_TOTAL_KB / 1024))
SWAP_TOTAL_MB=$((SWAP_TOTAL_KB / 1024))

if [ "$AUTO_SWAP" = true ] && [ "$MEM_TOTAL_MB" -lt 3800 ] && [ "$SWAP_TOTAL_MB" -lt 512 ]; then
  if [ ! -f /swapfile ]; then
    echo -e "${CYAN}[*] Low RAM detected (${MEM_TOTAL_MB} MB) with no active swap. Creating 2GB /swapfile to prevent Cargo OOM...${NC}"
    if fallocate -l 2G /swapfile 2>/dev/null || dd if=/dev/zero of=/swapfile bs=1M count=2048 status=none 2>/dev/null; then
      chmod 0600 /swapfile
      mkswap /swapfile >/dev/null 2>&1 || true
      swapon /swapfile >/dev/null 2>&1 || true
      if ! grep -q '^/swapfile ' /etc/fstab 2>/dev/null; then
        echo '/swapfile none swap sw 0 0' >> /etc/fstab
      fi
      echo -e "${GREEN}[+] Activated 2GB /swapfile.${NC}"
    else
      echo -e "${YELLOW}[!] Could not allocate /swapfile (container/filesystem limit); continuing...${NC}"
    fi
  else
    swapon /swapfile >/dev/null 2>&1 || true
  fi
else
  echo -e "${CYAN}[*] Memory OK (RAM=${MEM_TOTAL_MB} MB, Swap=${SWAP_TOTAL_MB} MB).${NC}"
fi

# ==============================================================================
# [2/7] System Dependencies & Automated Rust / Cargo (1.89+ / stable) Installation
# ==============================================================================
echo -e "${BLUE}[2/7] Installing system packages & build utilities (zero Node.js on VPS)...${NC}"
export DEBIAN_FRONTEND=noninteractive
apt-get update -y -q
CORE_PKGS=(
  ca-certificates curl git build-essential pkg-config libssl-dev
  tar zstd zip unzip openssl ufw fail2ban certbot
)
if [ "$WEB_ENGINE" = "nginx" ]; then
  CORE_PKGS+=(nginx)
fi
apt-get install -y -q "${CORE_PKGS[@]}"

export RUSTUP_HOME="${RUSTUP_HOME:-/root/.rustup}"
export CARGO_HOME="${CARGO_HOME:-/root/.cargo}"
export PATH="${CARGO_HOME}/bin:${PATH}"

ensure_cargo_installed() {
  local need_install=false
  local min_ver_code=189
  if ! command -v cargo >/dev/null 2>&1 || ! command -v rustc >/dev/null 2>&1; then
    need_install=true
  else
    local ver_code
    ver_code="$(rustc --version | awk '{print $2}' | awk -F'.' '{print ($1 * 100) + $2}' 2>/dev/null || echo 0)"
    if [ "${ver_code:-0}" -lt "${min_ver_code}" ]; then
      echo -e "${YELLOW}[!] Existing $(rustc --version) is older than required Rust 1.89+; upgrading to ${RUST_TOOLCHAIN}...${NC}"
      need_install=true
    fi
  fi

  if [ "$need_install" = true ]; then
    if command -v rustup >/dev/null 2>&1; then
      echo -e "${BLUE}[*] Upgrading Rust toolchain to ${RUST_TOOLCHAIN} via rustup...${NC}"
      rustup toolchain install "${RUST_TOOLCHAIN}" --profile minimal
      rustup default "${RUST_TOOLCHAIN}"
    else
      echo -e "${BLUE}[*] Installing Rust toolchain (${RUST_TOOLCHAIN}) & Cargo automatically via rustup...${NC}"
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "${RUST_TOOLCHAIN}"
    fi
  else
    echo -e "${CYAN}[*] Found compatible Rust toolchain: $(rustc --version) / $(cargo --version)${NC}"
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

configure_shell_env() {
  echo -e "${CYAN}[*] Registering environment variables in /etc/profile.d/ferrous-panel.sh and shell profiles...${NC}"
  cat > /etc/profile.d/ferrous-panel.sh <<'EOF_PROFILE'
# Managed by FerrousPanel — System-wide Rust/Cargo & FerrousPanel environment variables
export RUSTUP_HOME="/root/.rustup"
export CARGO_HOME="/root/.cargo"
export FERROUS_ENV_FILE="/etc/ferrous-panel/panel.env"
export FERROUSX_ENV_FILE="/etc/ferrousx/ferrousx.env"

case ":${PATH}:" in
  *":/usr/local/bin:"*) ;;
  *) export PATH="/usr/local/bin:${PATH}" ;;
esac

if [ -d "/root/.cargo/bin" ]; then
  case ":${PATH}:" in
    *":/root/.cargo/bin:"*) ;;
    *) export PATH="/root/.cargo/bin:${PATH}" ;;
  esac
fi

if [ -r "/etc/ferrous-panel/panel.env" ]; then
  set -a
  # shellcheck disable=SC1091
  . /etc/ferrous-panel/panel.env
  set +a
fi

if [ -r "/etc/ferrousx/ferrousx.env" ]; then
  set -a
  # shellcheck disable=SC1091
  . /etc/ferrousx/ferrousx.env
  set +a
fi
EOF_PROFILE
  chmod 0644 /etc/profile.d/ferrous-panel.sh

  for rc_file in /root/.bashrc /root/.profile /etc/bash.bashrc; do
    if [ -f "$rc_file" ] && ! grep -q '/etc/profile.d/ferrous-panel.sh' "$rc_file" 2>/dev/null; then
      printf '\n# FerrousPanel & Cargo environment\n[ -f /etc/profile.d/ferrous-panel.sh ] && . /etc/profile.d/ferrous-panel.sh\n' >> "$rc_file"
    fi
  done
}

ensure_cargo_installed
configure_shell_env

# ==============================================================================
# [3/7] Prepare System Directories
# ==============================================================================
echo -e "${BLUE}[3/7] Preparing system directories (/var/lib/ferrous-panel, /www/wwwroot, /www/acme)...${NC}"
mkdir -p /etc/ferrous-panel "${DATA_DIR}" "${WWWROOT}" /www/acme "${BACKUPS_DIR}"
chmod 700 "${DATA_DIR}" /etc/ferrous-panel "${BACKUPS_DIR}"
chown -R www-data:www-data "${WWWROOT}" /www/acme 2>/dev/null || true

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

curl -fsSL --connect-timeout 10 -o "${TMP_DIR}/SHA256SUMS.txt" "${RELEASE_BASE}/SHA256SUMS.txt" 2>/dev/null || rm -f "${TMP_DIR}/SHA256SUMS.txt"

download_release_binary() {
  local asset_name="$1"
  local dest_path="$2"
  local url="${RELEASE_BASE}/${asset_name}"
  local tmp_file="${TMP_DIR}/${asset_name}"

  echo -e "${CYAN}[*] Downloading prebuilt binary ${url}...${NC}"
  curl -fsSL --retry 3 --connect-timeout 10 -o "$tmp_file" "$url"

  if [ -f "${TMP_DIR}/SHA256SUMS.txt" ] && grep -q " ${asset_name}\$" "${TMP_DIR}/SHA256SUMS.txt"; then
    (cd "$TMP_DIR" && grep " ${asset_name}\$" SHA256SUMS.txt | sha256sum -c -)
  fi

  install -m 0755 "$tmp_file" "$dest_path"
}

# ==============================================================================
# [4/7] Fetch Source + Prebuilt Static dist/ & Build with Cargo
# ==============================================================================
echo -e "${BLUE}[4/7] Building FerrousPanel with Cargo (embedding prebuilt static dist/ UI)...${NC}"

if [ "$PREBUILT_BINARY_ONLY" = true ]; then
  download_release_binary "ferrous-panel-linux-${ARCH}" "${BIN_PANEL}"
  if [ "$WEB_ENGINE" = "ferrousx" ]; then
    download_release_binary "ferrousx-bin-linux-${ARCH}" "${BIN_FERROUSX}"
    if download_release_binary "ferrousx-ctl-linux-${ARCH}" "${BIN_FERROUSX_CTL}" 2>/dev/null; then
      ln -sf "${BIN_FERROUSX_CTL}" /usr/local/bin/fx
    fi
  fi
else
  # Determine source workspace directory (current directory if inside repo, otherwise clone to /usr/local/src/ferrous-panel)
  BUILD_WORKSPACE=""
  if [ -f "./Cargo.toml" ] && [ -d "./crates/ferrous-panel" ]; then
    BUILD_WORKSPACE="$(pwd)"
    echo -e "${CYAN}[*] Using current repository workspace at ${BUILD_WORKSPACE}${NC}"
  else
    echo -e "${CYAN}[*] Syncing source repository (${REPO_SLUG}@${GIT_BRANCH}) to ${SRC_DIR}...${NC}"
    mkdir -p "$(dirname "${SRC_DIR}")"
    if [ -d "${SRC_DIR}/.git" ]; then
      git -C "${SRC_DIR}" fetch --depth 1 origin "${GIT_BRANCH}"
      git -C "${SRC_DIR}" reset --hard "origin/${GIT_BRANCH}"
    else
      rm -rf "${SRC_DIR}"
      git clone --depth 1 --branch "${GIT_BRANCH}" "https://github.com/${REPO_SLUG}.git" "${SRC_DIR}"
    fi
    BUILD_WORKSPACE="${SRC_DIR}"
  fi

  # Ensure prebuilt dist/ static UI exists (download ferrous-dist.tar.gz if dist/index.html is missing,
  # or build locally if npm is available on a self-hosted open-source fork)
  if [ ! -f "${BUILD_WORKSPACE}/dist/index.html" ]; then
    echo -e "${CYAN}[*] Downloading prebuilt static frontend bundle (ferrous-dist.tar.gz)...${NC}"
    mkdir -p "${BUILD_WORKSPACE}/dist"
    if curl -fsSL --retry 3 --connect-timeout 10 -o "${TMP_DIR}/ferrous-dist.tar.gz" "${RELEASE_BASE}/ferrous-dist.tar.gz" 2>/dev/null; then
      tar -xzf "${TMP_DIR}/ferrous-dist.tar.gz" -C "${BUILD_WORKSPACE}/dist"
    elif command -v npm >/dev/null 2>&1 && [ -f "${BUILD_WORKSPACE}/package.json" ]; then
      echo -e "${CYAN}[*] Prebuilt bundle unavailable; building frontend locally via npm...${NC}"
      (cd "${BUILD_WORKSPACE}" && npm install --no-audit --no-fund && npm run build) || true
    fi
  fi

  echo -e "${CYAN}[*] Running: cargo build --release -p ferrous-panel ...${NC}"
  (
    cd "${BUILD_WORKSPACE}"
    cargo build --release -p ferrous-panel
  )

  install -m 0755 "${BUILD_WORKSPACE}/target/release/ferrous-panel" "${BIN_PANEL}"

  if [ "$WEB_ENGINE" = "ferrousx" ]; then
    echo -e "${CYAN}[*] Syncing latest FerrousX from GitHub (${FERROUSX_REPO}@${FERROUSX_BRANCH}) into ${FERROUSX_SRC_DIR}...${NC}"
    FX_BUILT=false
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
      FX_COMMIT="$(git -C "${FERROUSX_SRC_DIR}" rev-parse --short HEAD 2>/dev/null || echo 'main')"
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
      echo -e "${CYAN}[*] Building upstream FerrousX (${FX_COMMIT}) binaries (ferrousx-bin + ferrousx-ctl)...${NC}"
      if (cd "${FERROUSX_SRC_DIR}" && cargo build --release --bin ferrousx-bin --bin ferrousx-ctl); then
        if [ -x "${FERROUSX_SRC_DIR}/target/release/ferrousx-bin" ]; then
          install -m 0755 "${FERROUSX_SRC_DIR}/target/release/ferrousx-bin" "${BIN_FERROUSX}"
          FX_BUILT=true
        fi
        if [ -x "${FERROUSX_SRC_DIR}/target/release/ferrousx-ctl" ]; then
          install -m 0755 "${FERROUSX_SRC_DIR}/target/release/ferrousx-ctl" "${BIN_FERROUSX_CTL}"
          ln -sf "${BIN_FERROUSX_CTL}" /usr/local/bin/fx
        fi
      fi
    fi

    if [ "$FX_BUILT" = false ]; then
      echo -e "${YELLOW}[!] Falling back to embedded workspace FerrousX crate (-p ferrousx)...${NC}"
      (
        cd "${BUILD_WORKSPACE}"
        cargo build --release -p ferrousx
      )
      if [ -x "${BUILD_WORKSPACE}/target/release/ferrousx-bin" ]; then
        install -m 0755 "${BUILD_WORKSPACE}/target/release/ferrousx-bin" "${BIN_FERROUSX}"
      fi
      if [ -x "${BUILD_WORKSPACE}/target/release/ferrousx-ctl" ]; then
        install -m 0755 "${BUILD_WORKSPACE}/target/release/ferrousx-ctl" "${BIN_FERROUSX_CTL}"
        ln -sf "${BIN_FERROUSX_CTL}" /usr/local/bin/fx
      fi
    fi
  fi
fi

ln -sf "${BIN_PANEL}" /usr/local/bin/fp
ln -sf "${BIN_PANEL}" /usr/local/bin/ferrous

# ==============================================================================
# [5/7] Configure Active Web Server Engine (Nginx or FerrousX)
# ==============================================================================
if [ "$WEB_ENGINE" = "ferrousx" ]; then
  echo -e "${BLUE}[5/7] Configuring FerrousX web server (ferrousx.env + admin.token + ferrousx.service)...${NC}"
  # Prevent port 80/443 collision if Ubuntu/VPS image had nginx running
  if systemctl is-active --quiet nginx.service 2>/dev/null || systemctl is-enabled --quiet nginx.service 2>/dev/null; then
    echo -e "${CYAN}[*] Stopping & disabling nginx.service so FerrousX exclusively binds ports 80/443...${NC}"
    systemctl disable --now nginx.service 2>/dev/null || true
  fi
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
FERROUSX_CONTROL_SOCKET=/run/ferrousx/panel-admin.sock
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
  systemctl daemon-reload
  systemctl enable --now ferrousx.service
else
  echo -e "${BLUE}[5/7] Configuring Nginx (nginx.service) with /www/acme ACME challenge root & snippets...${NC}"
  # Prevent port 80/443 collision if ferrousx.service was previously running
  if systemctl is-active --quiet ferrousx.service 2>/dev/null || systemctl is-enabled --quiet ferrousx.service 2>/dev/null; then
    echo -e "${CYAN}[*] Stopping & disabling ferrousx.service so Nginx exclusively binds ports 80/443...${NC}"
    systemctl disable --now ferrousx.service 2>/dev/null || true
  fi

  mkdir -p /etc/nginx/sites-available /etc/nginx/sites-enabled /etc/nginx/conf.d /etc/nginx/snippets /www/acme/.well-known/acme-challenge
  chown -R www-data:www-data /www/acme 2>/dev/null || true

  # Ensure standard fastcgi-php.conf snippet exists for PHP-FPM sites on all Debian/Ubuntu variants
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

  # Safe global upload/domain bucket defaults (only directives not duplicated in stock nginx.conf)
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

  # Upgrade stock untouched Ubuntu default vhost so port 80 serves /www/acme/.well-known/acme-challenge/
  if [ -L /etc/nginx/sites-enabled/default ] && [ "$(readlink -f /etc/nginx/sites-enabled/default 2>/dev/null || true)" = "/etc/nginx/sites-available/default" ]; then
    cat > /etc/nginx/sites-available/ferrous-default.conf <<'EOF_NGX_DEF'
# Managed by FerrousPanel — Default catch-all vhost with ACME HTTP-01 webroot
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

    location / {
        try_files $uri $uri/ =404;
    }
}
EOF_NGX_DEF
    ln -sf /etc/nginx/sites-available/ferrous-default.conf /etc/nginx/sites-enabled/default
  fi

  nginx -t
  systemctl enable nginx.service
  systemctl restart nginx.service
fi

# ==============================================================================
# [6/7] Initialize Argon2id 0600 Credentials, SQLite & ferrous-panel.service
# ==============================================================================
echo -e "${BLUE}[6/7] Initializing Argon2id credentials, SQLite store, and systemd unit...${NC}"
cat > "${ENV_FILE}" <<EOF_ENV
FERROUS_BIND=${PANEL_BIND}
FERROUS_PORT=${PANEL_PORT}
FERROUS_DATA_DIR=${DATA_DIR}
FERROUS_WWWROOT=${WWWROOT}
FERROUS_BACKUPS_DIR=${BACKUPS_DIR}
FERROUS_SYSTEMD_DIR=/etc/systemd/system
FERROUS_ADMIN_USER=${ADMIN_USER}
FERROUS_SECURITY_ENTRANCE=${SECURITY_ENTRANCE}
FERROUS_CUSTOM_DOMAIN=${CUSTOM_DOMAIN}
FERROUS_SECURE_COOKIE=false
RUSTUP_HOME=${RUSTUP_HOME}
CARGO_HOME=${CARGO_HOME}
RUST_LOG=info,actix_web=info
EOF_ENV
chmod 0600 "${ENV_FILE}"

INIT_OUTPUT="$("${BIN_PANEL}" \
  --data-dir "${DATA_DIR}" \
  --wwwroot "${WWWROOT}" \
  --admin-username "${ADMIN_USER}" \
  fp init --engine "${WEB_ENGINE}")"
echo "${INIT_OUTPUT}"

GENERATED_PASS="$(printf '%s\n' "${INIT_OUTPUT}" | awk '/Password[[:space:]]*:/ {print $3; exit}' || true)"

# If user explicitly chose a different admin username on an already-initialized VPS, apply it cleanly
CURRENT_STORED_USER="$(printf '%s\n' "${INIT_OUTPUT}" | awk '/Username[[:space:]]*:/ {print $3; exit}' || true)"
if [ -n "${CURRENT_STORED_USER}" ] && [ "${CURRENT_STORED_USER}" != "${ADMIN_USER}" ]; then
  "${BIN_PANEL}" \
    --data-dir "${DATA_DIR}" \
    --wwwroot "${WWWROOT}" \
    fp username "${ADMIN_USER}"
fi

# If user entered a custom password interactively, pass it via stdin (never via CLI args)
if [ -n "${CUSTOM_ADMIN_PASS}" ]; then
  printf '%s\n%s\n' "${CUSTOM_ADMIN_PASS}" "${CUSTOM_ADMIN_PASS}" | "${BIN_PANEL}" \
    --data-dir "${DATA_DIR}" \
    --wwwroot "${WWWROOT}" \
    --admin-username "${ADMIN_USER}" \
    fp passwd >/dev/null
  GENERATED_PASS="(custom password set during install)"
  unset CUSTOM_ADMIN_PASS
fi

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
systemctl restart ferrous-panel.service

# Wait up to 5s for /api/health to respond
HEALTH_PROBE_HOST="${PANEL_BIND}"
if [ "${HEALTH_PROBE_HOST}" = "0.0.0.0" ]; then
  HEALTH_PROBE_HOST="127.0.0.1"
fi
for _ in $(seq 1 25); do
  if curl -fsS "http://${HEALTH_PROBE_HOST}:${PANEL_PORT}/api/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.2
done

# ==============================================================================
# [7/7] Configure UFW Firewall
# ==============================================================================
echo -e "${BLUE}[7/7] Configuring UFW rules (SSH + 80 + 443 + Panel Port ${PANEL_PORT})...${NC}"
ACTIVE_SSH_PORT="$(ss -tlnp 2>/dev/null | grep -E 'sshd|ssh' | awk '{print $4}' | awk -F':' '{print $NF}' | grep -E '^[0-9]+$' | head -n 1 || true)"
ACTIVE_SSH_PORT="${ACTIVE_SSH_PORT:-22}"
ufw allow "${ACTIVE_SSH_PORT}/tcp" comment 'SSH' >/dev/null 2>&1 || true
ufw allow 80/tcp comment 'HTTP' >/dev/null 2>&1 || true
ufw allow 443/tcp comment 'HTTPS' >/dev/null 2>&1 || true
if [ "$WEB_ENGINE" = "ferrousx" ]; then
  ufw allow 443/udp comment 'HTTP3-QUIC' >/dev/null 2>&1 || true
fi
if [ "${PANEL_BIND}" != "127.0.0.1" ] && [ "${PANEL_BIND}" != "::1" ]; then
  ufw allow "${PANEL_PORT}/tcp" comment 'FerrousPanel' >/dev/null 2>&1 || true
fi
ufw default deny incoming >/dev/null 2>&1 || true
ufw default allow outgoing >/dev/null 2>&1 || true
ufw --force enable >/dev/null 2>&1 || true

echo ""
echo -e "${GREEN}==============================================================================${NC}"
echo -e "${GREEN}${BOLD}  FerrousPanel Installed Cleanly (100% Pure-Rust + Cargo stable)${NC}"
echo -e "${GREEN}==============================================================================${NC}"
echo -e "  Binary            : ${BIN_PANEL} (embedded dist/ SPA)"
echo -e "  CLI Symlinks      : fp / ferrous -> ${BIN_PANEL}"
echo -e "  Rust Toolchain    : $(cargo --version 2>/dev/null || echo 'prebuilt')"
echo -e "  Web Engine        : ${WEB_ENGINE}"
if [ "$WEB_ENGINE" = "ferrousx" ]; then
  echo -e "  FerrousX Binary   : ${BIN_FERROUSX} (CLI: ${BIN_FERROUSX_CTL} / fx)"
  echo -e "  FerrousX Env/Tok  : ${FERROUSX_ENV_FILE} (0640) | ${FERROUSX_ADMIN_TOKEN_FILE} (0600)"
else
  NGINX_VER="$(nginx -v 2>&1 | sed 's|.*nginx/||' || echo 'installed')"
  echo -e "  Nginx Version     : ${NGINX_VER} (/etc/nginx/sites-enabled)"
fi
if [ "${PANEL_BIND}" = "127.0.0.1" ] || [ "${PANEL_BIND}" = "::1" ]; then
  echo -e "  Panel Listen Addr : http://${PANEL_BIND}:${PANEL_PORT}${SECURITY_ENTRANCE} (local-only)"
  echo -e "  SSH Tunnel Access : ssh -N -L ${PANEL_PORT}:${HEALTH_PROBE_HOST}:${PANEL_PORT} root@${VPS_PUBLIC_IP}"
else
  echo -e "  Internet Panel URL: ${BOLD}http://${VPS_PUBLIC_IP}:${PANEL_PORT}${SECURITY_ENTRANCE}${NC}"
  echo -e "  Internal Panel URL: http://${VPS_LOCAL_IP}:${PANEL_PORT}${SECURITY_ENTRANCE}"
fi
echo -e "  Admin Username    : ${BOLD}${ADMIN_USER}${NC}"
if [ -n "${GENERATED_PASS}" ] && [[ "${GENERATED_PASS}" != "(already"* ]]; then
  echo -e "  Admin Password    : ${BOLD}${GENERATED_PASS}${NC} (SHOWN ONCE ONLY)"
else
  echo -e "  Admin Password    : (already set; run 'sudo fp passwd' to change)"
fi
echo -e "  Credentials Hash  : ${DATA_DIR}/admin.credentials.json (mode 0600, Argon2id)"
echo -e "  Panel Env Config  : ${ENV_FILE}"
echo -e "  CLI Toolbox       : sudo fp        (or: sudo fp passwd / sudo fp username / sudo fp status)"
echo -e "${GREEN}==============================================================================${NC}"
