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
DATA_DIR="/var/lib/ferrous-panel"
ROLLBACK_DIR="${DATA_DIR}/rollback"
PANEL_COMMIT_FILE="${DATA_DIR}/.installed-commit"
FERROUSX_COMMIT_FILE="${DATA_DIR}/.installed-ferrousx-commit"
FERROUSX_ENV_FILE="/etc/ferrousx/ferrousx.env"
FERROUSX_ADMIN_TOKEN_FILE="/etc/ferrousx/admin.token"

UPDATE_PANEL=true
FORCE_FERROUSX=false
FORCE_NGINX=false
SYNC_JS_TOOLCHAIN=true
INSECURE_SKIP_VERIFY=false
CHECK_ONLY=false
PREBUILT_ONLY=false
SOURCE_ONLY=false
FULL_LTO=false
FORCE_REBUILD=false
DO_ROLLBACK=false
EXPLICIT_BIND=""
PANEL_CONFIG_CHANGED=false

usage() {
  cat <<'EOF_HELP'
FerrousPanel, Nginx & FerrousX 1-Command Updater (Fast Incremental + Atomic Rollback)

Usage:
  sudo fp update [OPTIONS]
  sudo bash update.sh [OPTIONS] [RELEASE_TAG]

Options:
  --check                      Check local vs upstream commit/version without modifying anything (~1s)
  --prebuilt                   Fast update (~3s): download SHA-256 verified GitHub Release binary
  --source                     Compile from source using fast incremental [profile.vps] (thin-LTO)
  --full-lto                   Use full fat-LTO [profile.release] instead of fast [profile.vps]
  --force                      Force rebuild/reinstall even if already on the latest commit
  --rollback                   Instantly restore previous working binary & verify /api/health
  --nginx                      Ensure Nginx is installed, synced, validated (nginx -t) & reloaded
  --nginx-only                 Validate & sync ONLY Nginx (do not rebuild ferrous-panel)
  --ferrousx                   Sync & rebuild latest FerrousX from github.com/Ferrous-panel/FerrousX
  --ferrousx-only              Update ONLY FerrousX from GitHub (do not rebuild ferrous-panel)
  --js-only                    Install/update ONLY Zero-Node.js JS/CSS binaries (bun + tailwindcss)
  --no-js-toolchain            Skip syncing Zero-Node.js JS/CSS binaries (bun + tailwindcss)
  --insecure-skip-verify       Bypass SHA-256 verification on downloaded release/tool binaries (NOT recommended)
  --engine <nginx|ferrousx>    Target web engine to update & validate
  --ferrousx-branch <BRANCH>   Upstream FerrousX Git branch (default: main)
  --branch <BRANCH>            FerrousPanel Git branch (default: main)
  --version <TAG|latest>       GitHub Release tag for prebuilt binary (default: latest)
  --bind <IP>                  Set panel listen IP in panel.env (e.g. 0.0.0.0 for public IP, 127.0.0.1 for local-only)
  -h, --help                   Show this help message
EOF_HELP
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --check)
      CHECK_ONLY=true
      SYNC_JS_TOOLCHAIN=false
      shift
      ;;
    --prebuilt|--prebuilt-binary)
      PREBUILT_ONLY=true
      shift
      ;;
    --source)
      SOURCE_ONLY=true
      shift
      ;;
    --full-lto)
      FULL_LTO=true
      shift
      ;;
    --force)
      FORCE_REBUILD=true
      shift
      ;;
    --rollback)
      DO_ROLLBACK=true
      SYNC_JS_TOOLCHAIN=false
      shift
      ;;
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
    --js-only)
      SYNC_JS_TOOLCHAIN=true
      UPDATE_PANEL=false
      shift
      ;;
    --no-js-toolchain)
      SYNC_JS_TOOLCHAIN=false
      shift
      ;;
    --insecure-skip-verify)
      INSECURE_SKIP_VERIFY=true
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
    --bind)
      EXPLICIT_BIND="${2:-}"
      if [ -z "${EXPLICIT_BIND}" ] || ! [[ "${EXPLICIT_BIND}" =~ ^[0-9a-fA-F.:]+$ ]]; then
        echo -e "${RED}[ERROR] --bind requires a valid IP address (e.g. 0.0.0.0 or 127.0.0.1).${NC}" >&2
        exit 1
      fi
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

# Prevent overlapping concurrent updates from corrupting git/cargo/binaries
if [ "$CHECK_ONLY" = false ] && command -v flock >/dev/null 2>&1; then
  mkdir -p /run
  exec 9>/run/ferrous-update.lock
  if ! flock -n 9; then
    echo -e "${RED}[ERROR] Another FerrousPanel update is already running (/run/ferrous-update.lock).${NC}" >&2
    exit 1
  fi
fi

mkdir -p "${DATA_DIR}" "${ROLLBACK_DIR}"
UPDATE_START_TS="$(date +%s)"

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

verify_sha256_entry() {
  local sums_file="$1"
  local asset_name="$2"
  local work_dir="$3"
  local label="$4"

  if [ ! -f "${sums_file}" ]; then
    if [ "${INSECURE_SKIP_VERIFY}" = true ]; then
      echo -e "${YELLOW}[WARNING] --insecure-skip-verify active: skipping missing SHA-256 manifest for ${label} (${asset_name}).${NC}" >&2
      return 0
    fi
    echo -e "${RED}[ERROR] Cannot download SHA-256 checksum manifest for ${label} (${asset_name}). Refusing unverified binary update.${NC}" >&2
    echo -e "${YELLOW}Pass --insecure-skip-verify ONLY if you explicitly accept installing an unverified binary.${NC}" >&2
    exit 1
  fi

  local matched_line
  matched_line="$(grep -E "[[:space:]](\./|\*)?${asset_name}$" "${sums_file}" | head -n 1 || true)"
  if [ -z "${matched_line}" ]; then
    if [ "${INSECURE_SKIP_VERIFY}" = true ]; then
      echo -e "${YELLOW}[WARNING] --insecure-skip-verify active: no SHA-256 entry found for ${asset_name} in ${label}.${NC}" >&2
      return 0
    fi
    echo -e "${RED}[ERROR] No SHA-256 entry matching '${asset_name}' in ${label} checksum manifest. Aborting update.${NC}" >&2
    exit 1
  fi

  local expected_hash
  expected_hash="$(printf '%s\n' "${matched_line}" | awk '{print $1}')"
  if ! (cd "${work_dir}" && printf '%s  %s\n' "${expected_hash}" "${asset_name}" | sha256sum -c -); then
    echo -e "${RED}[ERROR] SHA-256 verification FAILED for ${asset_name} (${label}). Aborting update.${NC}" >&2
    exit 1
  fi
}

download_verified_release_asset() {
  local asset_name="$1"
  local dest_path="$2"
  local tmp_asset="${TMP_DIR}/${asset_name}"

  if ! curl -fsSL --connect-timeout 10 -o "${TMP_DIR}/SHA256SUMS.txt" "${RELEASE_BASE}/SHA256SUMS.txt" 2>/dev/null; then
    rm -f "${TMP_DIR}/SHA256SUMS.txt"
  fi
  curl -fsSL --retry 3 --connect-timeout 10 -o "${tmp_asset}" "${RELEASE_BASE}/${asset_name}"
  verify_sha256_entry "${TMP_DIR}/SHA256SUMS.txt" "${asset_name}" "${TMP_DIR}" "FerrousPanel release"
  install -m 0755 "${tmp_asset}" "${dest_path}"
}

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

ensure_swap_for_build() {
  local ram_mb swap_mb
  ram_mb="$(awk '/MemTotal:/ {printf "%d", $2/1024}' /proc/meminfo 2>/dev/null || echo 0)"
  swap_mb="$(awk '/SwapTotal:/ {printf "%d", $2/1024}' /proc/meminfo 2>/dev/null || echo 0)"
  if [ "${ram_mb}" -gt 0 ] && [ "${ram_mb}" -lt 2500 ] && [ "${swap_mb}" -lt 512 ] && [ ! -f /swapfile ]; then
    echo -e "${YELLOW}[!] Low RAM detected (${ram_mb} MB, no swap) — provisioning 2 GB /swapfile to prevent OOM during build...${NC}"
    if fallocate -l 2G /swapfile 2>/dev/null || dd if=/dev/zero of=/swapfile bs=1M count=2048 status=none; then
      chmod 600 /swapfile
      mkswap /swapfile >/dev/null 2>&1 && swapon /swapfile >/dev/null 2>&1 || rm -f /swapfile
    fi
  fi
}

configure_fast_cargo_env() {
  local ram_mb cpus jobs
  ram_mb="$(awk '/MemTotal:/ {printf "%d", $2/1024}' /proc/meminfo 2>/dev/null || echo 2048)"
  cpus="$(nproc 2>/dev/null || echo 2)"
  if [ -z "${CARGO_BUILD_JOBS:-}" ]; then
    if [ "${ram_mb}" -lt 1500 ]; then
      jobs=1
    elif [ "${ram_mb}" -lt 2800 ]; then
      jobs=$(( cpus > 2 ? 2 : cpus ))
    else
      jobs="${cpus}"
    fi
    export CARGO_BUILD_JOBS="${jobs}"
  fi
  export CARGO_INCREMENTAL=1
  if [ -z "${RUSTFLAGS:-}" ]; then
    if command -v mold >/dev/null 2>&1; then
      export RUSTFLAGS="-C link-arg=-fuse-ld=mold"
    elif command -v lld >/dev/null 2>&1 || command -v ld.lld >/dev/null 2>&1; then
      export RUSTFLAGS="-C link-arg=-fuse-ld=lld"
    fi
  fi
}

atomic_install_binary() {
  local src_bin="$1"
  local dst_bin="$2"
  local backup_name="$3"
  local label="$4"

  if [ ! -x "${src_bin}" ]; then
    echo -e "${RED}[ERROR] Candidate binary for ${label} is missing or not executable: ${src_bin}${NC}" >&2
    exit 1
  fi
  if ! "${src_bin}" --version >/dev/null 2>&1; then
    echo -e "${RED}[ERROR] Pre-flight check failed: candidate ${label} binary could not execute '--version'. Aborting without touching running service.${NC}" >&2
    exit 1
  fi

  if [ -x "${dst_bin}" ]; then
    cp -f "${dst_bin}" "${ROLLBACK_DIR}/${backup_name}"
    chmod 0755 "${ROLLBACK_DIR}/${backup_name}"
  fi
  install -m 0755 "${src_bin}" "${dst_bin}.new"
  mv -f "${dst_bin}.new" "${dst_bin}"
}

probe_panel_health() {
  local host="$1"
  local port="$2"
  local attempts="${3:-25}"
  for _ in $(seq 1 "${attempts}"); do
    if curl -fsS --max-time 2 "http://${host}:${port}/api/health" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.2
  done
  return 1
}

read_panel_env_key() {
  local key="$1"
  local default_val="${2:-}"
  if [ -f /etc/ferrous-panel/panel.env ]; then
    local val
    val="$(grep -E "^[[:space:]]*(export[[:space:]]+)?${key}=" /etc/ferrous-panel/panel.env 2>/dev/null | tail -n 1 | cut -d= -f2 | tr -d '[:space:]\"'\''' || true)"
    if [ -n "${val}" ]; then
      echo "${val}"
      return 0
    fi
  fi
  echo "${default_val}"
}

perform_panel_rollback() {
  local prev_bin="${ROLLBACK_DIR}/ferrous-panel.prev"
  if [ ! -x "${prev_bin}" ]; then
    echo -e "${RED}[ERROR] No rollback binary found at ${prev_bin}.${NC}" >&2
    exit 1
  fi
  echo -e "${YELLOW}[!] Rolling back ${BIN_PANEL} to previous working binary...${NC}"
  install -m 0755 "${prev_bin}" "${BIN_PANEL}.new"
  mv -f "${BIN_PANEL}.new" "${BIN_PANEL}"
  if [ -f "${ROLLBACK_DIR}/ferrous-panel.prev.commit" ]; then
    cp -f "${ROLLBACK_DIR}/ferrous-panel.prev.commit" "${PANEL_COMMIT_FILE}"
  fi
  systemctl restart ferrous-panel.service 2>/dev/null || true

  local port bind host
  port="$(read_panel_env_key FERROUS_PORT 8443)"
  bind="$(read_panel_env_key FERROUS_BIND 127.0.0.1)"
  host="${bind}"
  if [ "${host}" = "0.0.0.0" ] || [ "${host}" = "::" ]; then
    host="127.0.0.1"
  fi
  if probe_panel_health "${host}" "${port}" 25; then
    echo -e "${GREEN}[+] Rollback succeeded: ferrous-panel.service is healthy on ${bind}:${port} ($(${BIN_PANEL} --version 2>/dev/null || echo restored)).${NC}"
  else
    echo -e "${RED}[ERROR] Rollback binary restarted but /api/health did not respond on ${host}:${port}. Check 'sudo fp logs'.${NC}" >&2
    exit 1
  fi
}

if [ "$DO_ROLLBACK" = true ]; then
  perform_panel_rollback
  if [ -x "${ROLLBACK_DIR}/ferrousx-bin.prev" ] && [ -x "${BIN_FERROUSX}" ]; then
    echo -e "${YELLOW}[!] Also restoring previous ferrousx-bin backup...${NC}"
    install -m 0755 "${ROLLBACK_DIR}/ferrousx-bin.prev" "${BIN_FERROUSX}.new"
    mv -f "${BIN_FERROUSX}.new" "${BIN_FERROUSX}"
    if systemctl is-active --quiet ferrousx.service 2>/dev/null; then
      systemctl restart ferrousx.service || true
    fi
  fi
  exit 0
fi

if [ "$CHECK_ONLY" = true ]; then
  LOCAL_VER="$("${BIN_PANEL}" --version 2>/dev/null || echo 'not-installed')"
  LOCAL_SHA="$(cat "${PANEL_COMMIT_FILE}" 2>/dev/null || git -C "${SRC_DIR}" rev-parse HEAD 2>/dev/null || echo 'unknown')"
  REMOTE_SHA=""
  if command -v git >/dev/null 2>&1; then
    REMOTE_SHA="$(git ls-remote --heads "https://github.com/${REPO_SLUG}.git" "refs/heads/${GIT_BRANCH}" 2>/dev/null | awk '{print $1}' | head -n 1 || true)"
  fi
  echo -e "${CYAN}FerrousPanel Update Check (${REPO_SLUG}@${GIT_BRANCH})${NC}"
  echo -e "  Installed Version : ${BOLD}${LOCAL_VER}${NC}"
  echo -e "  Installed Commit  : ${BOLD}${LOCAL_SHA:0:12}${NC}"
  echo -e "  Upstream Commit   : ${BOLD}${REMOTE_SHA:0:12}${NC}"
  echo -e "  Rollback Backup   : $([ -x "${ROLLBACK_DIR}/ferrous-panel.prev" ] && echo 'available' || echo 'none')"
  if [ -n "${REMOTE_SHA}" ] && [ "${LOCAL_SHA}" = "${REMOTE_SHA}" ]; then
    echo -e "${GREEN}[+] Already up to date.${NC}"
  elif [ -n "${REMOTE_SHA}" ]; then
    echo -e "${YELLOW}[!] New update available! Run 'sudo fp update' (fast incremental) or 'sudo fp update --prebuilt'.${NC}"
  else
    echo -e "${YELLOW}[!] Could not reach GitHub to check remote commit.${NC}"
  fi
  exit 0
fi

if [ "$PREBUILT_ONLY" = false ]; then
  ensure_rust_toolchain
fi

detect_local_ip() {
  local ip=""
  ip="$(ip -4 route get 1.1.1.1 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i=="src") {print $(i+1); exit}}' || true)"
  if [ -z "$ip" ]; then
    ip="$(hostname -I 2>/dev/null | awk '{print $1}' || true)"
  fi
  echo "${ip:-127.0.0.1}"
}

detect_public_ip() {
  local cached=""
  cached="$(read_panel_env_key FERROUS_PUBLIC_IP "")"
  if [[ "$cached" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}$ ]] && [ "$cached" != "127.0.0.1" ]; then
    echo "$cached"
    return 0
  fi
  local ip=""
  for url in "https://api.ipify.org" "https://ifconfig.me/ip" "https://ipv4.icanhazip.com"; do
    ip="$(curl -4 -fsS --connect-timeout 2 --max-time 2 "$url" 2>/dev/null | tr -d '[:space:]' || true)"
    if [[ "$ip" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}$ ]]; then
      echo "$ip"
      return 0
    fi
  done
  detect_local_ip
}

VPS_PUBLIC_IP="$(detect_public_ip)"

setup_phpmyadmin_proxy() {
  # Pure-Rust FerrousAdmin (/phpadmin/) is built directly into ferrous-panel (Actix-Web on 127.0.0.1:888).
  # Stop and disable any legacy PHP phpMyAdmin service so port 888 is exclusively owned by Rust ferrous-panel.
  if systemctl is-active --quiet ferrous-pma.service 2>/dev/null || systemctl is-enabled --quiet ferrous-pma.service 2>/dev/null; then
    systemctl disable --now ferrous-pma.service 2>/dev/null || true
  fi
  rm -f /etc/systemd/system/ferrous-pma.service /etc/ferrousx/pma-router.php 2>/dev/null || true
  systemctl daemon-reload 2>/dev/null || true
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
RuntimeDirectory=ferrousx
RuntimeDirectoryMode=0750
ExecStartPre=/usr/local/bin/ferrousx-bin --check-config --config ${FERROUSX_CONFIG}
ExecStart=/usr/local/bin/ferrousx-bin --config ${FERROUSX_CONFIG}
ExecReload=/bin/kill -HUP $MAINPID
Restart=always
RestartSec=2
TimeoutStopSec=15
LimitNOFILE=65535
NoNewPrivileges=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF_FX_SVC
  systemctl daemon-reload 2>/dev/null || true
}

update_ferrousx_from_github() {
  echo -e "${BLUE}${BOLD}[*] Checking FerrousX updates (${FERROUSX_REPO}@${FERROUSX_BRANCH})...${NC}"
  local fx_installed=false
  local fx_skipped=false
  local fx_target_sha=""

  if [ "$PREBUILT_ONLY" = false ] && command -v cargo >/dev/null 2>&1 && command -v git >/dev/null 2>&1; then
    mkdir -p "$(dirname "${FERROUSX_SRC_DIR}")"
    if (
      if [ -d "${FERROUSX_SRC_DIR}/.git" ]; then
        git -C "${FERROUSX_SRC_DIR}" fetch --depth 1 --quiet origin "${FERROUSX_BRANCH}"
      else
        rm -rf "${FERROUSX_SRC_DIR}" &&
        git clone --depth 1 --branch "${FERROUSX_BRANCH}" "https://github.com/${FERROUSX_REPO}.git" "${FERROUSX_SRC_DIR}"
      fi
    ); then
      fx_target_sha="$(git -C "${FERROUSX_SRC_DIR}" rev-parse "origin/${FERROUSX_BRANCH}" 2>/dev/null || git -C "${FERROUSX_SRC_DIR}" rev-parse HEAD 2>/dev/null || echo '')"
      local fx_prev_sha
      fx_prev_sha="$(cat "${FERROUSX_COMMIT_FILE}" 2>/dev/null || echo '')"
      if [ "$FORCE_REBUILD" = false ] && [ -x "${BIN_FERROUSX}" ] && [ -n "${fx_target_sha}" ] && [ "${fx_prev_sha}" = "${fx_target_sha}" ]; then
        echo -e "${GREEN}[+] FerrousX already up to date (${fx_target_sha:0:8}) — skipped rebuild.${NC}"
        fx_installed=true
        fx_skipped=true
      else
        git -C "${FERROUSX_SRC_DIR}" reset --hard "origin/${FERROUSX_BRANCH}" >/dev/null 2>&1 || true
      fi
    fi
    if [ "$fx_skipped" = false ] && [ -d "${FERROUSX_SRC_DIR}/.git" ]; then
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
      ensure_swap_for_build
      configure_fast_cargo_env
      if (cd "${FERROUSX_SRC_DIR}" && cargo build --release --bin ferrousx-bin --bin ferrousx-ctl); then
        if [ -x "${FERROUSX_SRC_DIR}/target/release/ferrousx-bin" ]; then
          atomic_install_binary "${FERROUSX_SRC_DIR}/target/release/ferrousx-bin" "${BIN_FERROUSX}" "ferrousx-bin.prev" "ferrousx-bin"
          fx_installed=true
          [ -n "${fx_target_sha}" ] && printf '%s\n' "${fx_target_sha}" > "${FERROUSX_COMMIT_FILE}"
        fi
        if [ -x "${FERROUSX_SRC_DIR}/target/release/ferrousx-ctl" ]; then
          install -m 0755 "${FERROUSX_SRC_DIR}/target/release/ferrousx-ctl" "${BIN_FERROUSX_CTL}.new"
          mv -f "${BIN_FERROUSX_CTL}.new" "${BIN_FERROUSX_CTL}"
          ln -sf "${BIN_FERROUSX_CTL}" /usr/local/bin/fx
        fi
        echo -e "${GREEN}[+] Installed upstream FerrousX (${fx_commit}) -> ${BIN_FERROUSX}${NC}"
      fi
    fi
  fi

  if [ "$fx_installed" = false ] && [ "$PREBUILT_ONLY" = false ] && [ -d "${SRC_DIR}/crates/ferrousx" ] && command -v cargo >/dev/null 2>&1; then
    echo -e "${YELLOW}[!] Upstream FerrousX build unavailable; building embedded workspace crate (-p ferrousx)...${NC}"
    ensure_swap_for_build
    configure_fast_cargo_env
    local fx_prof_args="--release"
    local fx_target_sub="release"
    if [ "$FULL_LTO" = false ] && grep -q '^\[profile\.vps\]' "${SRC_DIR}/Cargo.toml" 2>/dev/null; then
      fx_prof_args="--profile vps"
      fx_target_sub="vps"
    fi
    (
      cd "${SRC_DIR}"
      cargo build ${fx_prof_args} -p ferrousx
    )
    if [ -x "${SRC_DIR}/target/${fx_target_sub}/ferrousx-bin" ]; then
      atomic_install_binary "${SRC_DIR}/target/${fx_target_sub}/ferrousx-bin" "${BIN_FERROUSX}" "ferrousx-bin.prev" "ferrousx-bin"
      fx_installed=true
    fi
    if [ -x "${SRC_DIR}/target/${fx_target_sub}/ferrousx-ctl" ]; then
      install -m 0755 "${SRC_DIR}/target/${fx_target_sub}/ferrousx-ctl" "${BIN_FERROUSX_CTL}.new"
      mv -f "${BIN_FERROUSX_CTL}.new" "${BIN_FERROUSX_CTL}"
      ln -sf "${BIN_FERROUSX_CTL}" /usr/local/bin/fx
    fi
  fi

  if [ "$fx_installed" = false ]; then
    echo -e "${CYAN}[*] Downloading prebuilt ferrousx-bin release (${RELEASE_TAG}, ${ARCH})...${NC}"
    local tmp_fx="${TMP_DIR}/ferrousx-bin-candidate"
    download_verified_release_asset "ferrousx-bin-linux-${ARCH}" "${tmp_fx}"
    atomic_install_binary "${tmp_fx}" "${BIN_FERROUSX}" "ferrousx-bin.prev" "ferrousx-bin"
  fi

  ensure_ferrousx_runtime_files

  if [ -x "${BIN_FERROUSX}" ]; then
    if ! "${BIN_FERROUSX}" --check-config --config /etc/ferrousx/ferrousx.toml; then
      echo -e "${RED}[ERROR] Updated ferrousx-bin failed --check-config; rolling back to previous ferrousx-bin.${NC}" >&2
      if [ -x "${ROLLBACK_DIR}/ferrousx-bin.prev" ]; then
        install -m 0755 "${ROLLBACK_DIR}/ferrousx-bin.prev" "${BIN_FERROUSX}.new"
        mv -f "${BIN_FERROUSX}.new" "${BIN_FERROUSX}"
      fi
      return 1
    fi
  fi

  if [ "$fx_skipped" = false ] && systemctl is-active --quiet ferrousx.service 2>/dev/null; then
    if command -v ufw >/dev/null 2>&1 && ufw status 2>/dev/null | grep -qi "Status: active"; then
      ufw allow 443/udp comment 'HTTP3-QUIC' >/dev/null 2>&1 || true
    fi
    if ! systemctl restart ferrousx.service || ! systemctl is-active --quiet ferrousx.service; then
      echo -e "${RED}[ERROR] ferrousx.service failed to restart with new binary; rolling back...${NC}" >&2
      if [ -x "${ROLLBACK_DIR}/ferrousx-bin.prev" ]; then
        install -m 0755 "${ROLLBACK_DIR}/ferrousx-bin.prev" "${BIN_FERROUSX}.new"
        mv -f "${BIN_FERROUSX}.new" "${BIN_FERROUSX}"
        systemctl restart ferrousx.service || true
      fi
      return 1
    fi
    if [ -x "${BIN_FERROUSX_CTL}" ]; then
      "${BIN_FERROUSX_CTL}" sync >/dev/null 2>&1 || true
    fi
    echo -e "${GREEN}[+] Restarted ferrousx.service with updated binary & ferrousx.env.${NC}"
  elif [ "$fx_skipped" = true ]; then
    :
  else
    echo -e "${CYAN}[*] ferrousx.service is installed (inactive — switch active engine via 'sudo fp webserver ferrousx').${NC}"
  fi
}

install_zero_node_js_css_toolchain() {
  local bun_arch="x64"
  local tw_arch="x64"
  if [ "${ARCH}" = "aarch64" ]; then
    bun_arch="aarch64"
    tw_arch="arm64"
  fi

  if [ ! -x /usr/local/bin/bun ]; then
    echo -e "${CYAN}[*] Installing Bun single native binary (Zero-Node.js JS/TS runtime & bundler)...${NC}"
    if ! command -v unzip >/dev/null 2>&1 && command -v apt-get >/dev/null 2>&1; then
      DEBIAN_FRONTEND=noninteractive apt-get install -y -qq unzip >/dev/null 2>&1 || true
    fi
    mkdir -p "${TMP_DIR}/bun-dl"
    local bun_asset="bun-linux-${bun_arch}.zip"
    if curl -fsSL --retry 3 --connect-timeout 15 -o "${TMP_DIR}/bun-dl/${bun_asset}" \
      "https://github.com/oven-sh/bun/releases/latest/download/${bun_asset}"; then
      curl -fsSL --connect-timeout 15 -o "${TMP_DIR}/bun-dl/SHASUMS256.txt" \
        "https://github.com/oven-sh/bun/releases/latest/download/SHASUMS256.txt" 2>/dev/null || rm -f "${TMP_DIR}/bun-dl/SHASUMS256.txt"
      verify_sha256_entry "${TMP_DIR}/bun-dl/SHASUMS256.txt" "${bun_asset}" "${TMP_DIR}/bun-dl" "Bun release"
      if unzip -qo "${TMP_DIR}/bun-dl/${bun_asset}" -d "${TMP_DIR}/bun-dl" \
        && [ -f "${TMP_DIR}/bun-dl/bun-linux-${bun_arch}/bun" ]; then
        install -m 0755 "${TMP_DIR}/bun-dl/bun-linux-${bun_arch}/bun" /usr/local/bin/bun
        echo -e "${GREEN}[+] Verified & installed Bun $(/usr/local/bin/bun --version 2>/dev/null || echo 'latest') -> /usr/local/bin/bun (Zero-Node.js).${NC}"
      else
        echo -e "${RED}[ERROR] Failed to unpack verified Bun archive.${NC}" >&2
        exit 1
      fi
    else
      echo -e "${YELLOW}[!] Could not download Bun binary right now.${NC}"
    fi
  fi
  if [ -x /usr/local/bin/bun ] && ! command -v node >/dev/null 2>&1; then
    ln -sf /usr/local/bin/bun /usr/local/bin/node
    echo -e "${GREEN}[+] Linked /usr/local/bin/node -> /usr/local/bin/bun (Zero-Node.js shebang compatibility).${NC}"
  fi

  if [ ! -x /usr/local/bin/tailwindcss ]; then
    echo -e "${CYAN}[*] Installing Tailwind CSS v4 standalone native CLI (Rust Oxide, Zero-Node.js)...${NC}"
    mkdir -p "${TMP_DIR}/tw-dl"
    local tw_asset="tailwindcss-linux-${tw_arch}"
    if curl -fsSL --retry 3 --connect-timeout 15 -o "${TMP_DIR}/tw-dl/${tw_asset}" \
      "https://github.com/tailwindlabs/tailwindcss/releases/latest/download/${tw_asset}"; then
      if ! curl -fsSL --connect-timeout 15 -o "${TMP_DIR}/tw-dl/sha256sums.txt" \
        "https://github.com/tailwindlabs/tailwindcss/releases/latest/download/sha256sums.txt" 2>/dev/null; then
        curl -fsSL --connect-timeout 15 -o "${TMP_DIR}/tw-dl/sha256sums.txt" \
          "https://github.com/tailwindlabs/tailwindcss/releases/latest/download/SHA256SUMS.txt" 2>/dev/null || rm -f "${TMP_DIR}/tw-dl/sha256sums.txt"
      fi
      verify_sha256_entry "${TMP_DIR}/tw-dl/sha256sums.txt" "${tw_asset}" "${TMP_DIR}/tw-dl" "Tailwind CSS release"
      install -m 0755 "${TMP_DIR}/tw-dl/${tw_asset}" /usr/local/bin/tailwindcss
      echo -e "${GREEN}[+] Verified & installed Tailwind CSS v4 standalone binary -> /usr/local/bin/tailwindcss (Zero-Node.js).${NC}"
    else
      echo -e "${YELLOW}[!] Could not download Tailwind CSS standalone binary right now.${NC}"
    fi
  fi
}

ensure_native_rust_ui() {
  local work_dir="$1"
  local ui_changed="${2:-true}"
  if [ ! -f "${work_dir}/crates/ferrous-panel/src/ui/mod.rs" ]; then
    echo -e "${RED}[ERROR] Pure-Rust Native UI module (${work_dir}/crates/ferrous-panel/src/ui/mod.rs) is missing.${NC}" >&2
    return 1
  fi
  echo -e "${GREEN}[+] Pure-Rust Native Panel UI compiles straight into ferrous-panel binary (zero Node.js / npm / Vite).${NC}"
  if [ "${ui_changed}" = true ]; then
    touch "${work_dir}/crates/ferrous-panel/src/ui/mod.rs"
  fi
}

PANEL_BINARY_UPDATED=false
if [ "$UPDATE_PANEL" = true ]; then
  if [ "$PREBUILT_ONLY" = false ] && [ -d "${SRC_DIR}/.git" ] && command -v cargo >/dev/null 2>&1; then
    echo -e "${BLUE}${BOLD}[*] Fetching latest FerrousPanel (${REPO_SLUG}@${GIT_BRANCH})...${NC}"
    OLD_COMMIT="$(git -C "${SRC_DIR}" rev-parse HEAD 2>/dev/null || echo '')"
    INSTALLED_COMMIT="$(cat "${PANEL_COMMIT_FILE}" 2>/dev/null || echo '')"
    git -C "${SRC_DIR}" fetch --depth 1 --quiet origin "${GIT_BRANCH}"
    TARGET_COMMIT="$(git -C "${SRC_DIR}" rev-parse "origin/${GIT_BRANCH}" 2>/dev/null || echo '')"

    if [ "$FORCE_REBUILD" = false ] && [ -x "${BIN_PANEL}" ] && [ -n "${TARGET_COMMIT}" ] \
       && [ "${OLD_COMMIT}" = "${TARGET_COMMIT}" ] && [ "${INSTALLED_COMMIT}" = "${TARGET_COMMIT}" ]; then
      echo -e "${GREEN}[+] FerrousPanel is already up to date (${TARGET_COMMIT:0:8}) — skipped rebuild (use --force to recompile).${NC}"
    else
      UI_CHANGED=true
      if [ -n "${OLD_COMMIT}" ] && [ -n "${TARGET_COMMIT}" ] && [ "${OLD_COMMIT}" != "${TARGET_COMMIT}" ]; then
        if git -C "${SRC_DIR}" diff --quiet "${OLD_COMMIT}" "${TARGET_COMMIT}" -- crates/ferrous-panel/src/ui/assets 2>/dev/null; then
          UI_CHANGED=false
        fi
      fi
      git -C "${SRC_DIR}" reset --hard "origin/${GIT_BRANCH}"
      ensure_native_rust_ui "${SRC_DIR}" "${UI_CHANGED}"
      ensure_swap_for_build
      configure_fast_cargo_env

      BUILD_PROFILE_ARGS="--release"
      BUILD_TARGET_SUB="release"
      if [ "$FULL_LTO" = false ] && grep -q '^\[profile\.vps\]' "${SRC_DIR}/Cargo.toml" 2>/dev/null; then
        BUILD_PROFILE_ARGS="--profile vps"
        BUILD_TARGET_SUB="vps"
      fi

      echo -e "${CYAN}[*] Compiling ferrous-panel (${TARGET_COMMIT:0:8}, profile=${BUILD_TARGET_SUB}, jobs=${CARGO_BUILD_JOBS:-auto})...${NC}"
      (
        cd "${SRC_DIR}"
        cargo build ${BUILD_PROFILE_ARGS} -p ferrous-panel
      )

      if [ -f "${PANEL_COMMIT_FILE}" ]; then
        cp -f "${PANEL_COMMIT_FILE}" "${ROLLBACK_DIR}/ferrous-panel.prev.commit"
      elif [ -n "${OLD_COMMIT}" ]; then
        printf '%s\n' "${OLD_COMMIT}" > "${ROLLBACK_DIR}/ferrous-panel.prev.commit"
      fi
      atomic_install_binary "${SRC_DIR}/target/${BUILD_TARGET_SUB}/ferrous-panel" "${BIN_PANEL}" "ferrous-panel.prev" "ferrous-panel"
      [ -n "${TARGET_COMMIT}" ] && printf '%s\n' "${TARGET_COMMIT}" > "${PANEL_COMMIT_FILE}"
      PANEL_BINARY_UPDATED=true
    fi
  elif [ "$SOURCE_ONLY" = true ]; then
    echo -e "${RED}[ERROR] --source requested, but ${SRC_DIR}/.git or cargo is unavailable.${NC}" >&2
    exit 1
  else
    echo -e "${CYAN}[*] Downloading verified prebuilt FerrousPanel release (${RELEASE_TAG}, ${ARCH})...${NC}"
    TMP_PANEL_BIN="${TMP_DIR}/ferrous-panel-candidate"
    download_verified_release_asset "ferrous-panel-linux-${ARCH}" "${TMP_PANEL_BIN}"
    if [ -f "${PANEL_COMMIT_FILE}" ]; then
      cp -f "${PANEL_COMMIT_FILE}" "${ROLLBACK_DIR}/ferrous-panel.prev.commit"
    fi
    atomic_install_binary "${TMP_PANEL_BIN}" "${BIN_PANEL}" "ferrous-panel.prev" "ferrous-panel"
    printf 'release-%s\n' "${RELEASE_TAG}" > "${PANEL_COMMIT_FILE}"
    PANEL_BINARY_UPDATED=true
  fi

  if [ -f /etc/ferrous-panel/panel.env ]; then
    RAW_PANEL_PORT="$(grep -E '^[[:space:]]*(export[[:space:]]+)?FERROUS_PORT=' /etc/ferrous-panel/panel.env 2>/dev/null | tail -n 1 | cut -d= -f2 | tr -d '[:space:]\"'\''' || true)"
    if [ -z "${RAW_PANEL_PORT}" ] || ! [[ "${RAW_PANEL_PORT}" =~ ^[0-9]+$ ]] || [ "${RAW_PANEL_PORT}" -lt 1 ] || [ "${RAW_PANEL_PORT}" -gt 65535 ]; then
      RAW_PANEL_PORT="8443"
    elif [ "${RAW_PANEL_PORT}" -eq 80 ] || [ "${RAW_PANEL_PORT}" -eq 443 ]; then
      echo -e "${YELLOW}[!] FERROUS_PORT=${RAW_PANEL_PORT} is reserved exclusively for websites (Nginx/FerrousX); resetting panel port to 8443...${NC}"
      RAW_PANEL_PORT="8443"
    fi
    # Deduplicate any duplicate/export FERROUS_PORT lines so systemd always reads one clean port
    awk -v port="${RAW_PANEL_PORT}" '
      /^[[:space:]]*(export[[:space:]]+)?FERROUS_PORT=/ {
        if (!seen) { print "FERROUS_PORT=" port; seen=1 }
        next
      }
      { print }
      END { if (!seen) print "FERROUS_PORT=" port }
    ' /etc/ferrous-panel/panel.env > /etc/ferrous-panel/panel.env.tmp && \
      chmod 0600 /etc/ferrous-panel/panel.env.tmp && \
      mv /etc/ferrous-panel/panel.env.tmp /etc/ferrous-panel/panel.env

    if ! grep -Eq '^[[:space:]]*(export[[:space:]]+)?FERROUS_MODE=' /etc/ferrous-panel/panel.env 2>/dev/null; then
      echo "FERROUS_MODE=production" >> /etc/ferrous-panel/panel.env
    fi
    if [ -n "${EXPLICIT_BIND}" ]; then
      if grep -Eq '^[[:space:]]*(export[[:space:]]+)?FERROUS_BIND=' /etc/ferrous-panel/panel.env 2>/dev/null; then
        sed -i -E "s|^[[:space:]]*(export[[:space:]]+)?FERROUS_BIND=.*|FERROUS_BIND=${EXPLICIT_BIND}|" /etc/ferrous-panel/panel.env
      else
        echo "FERROUS_BIND=${EXPLICIT_BIND}" >> /etc/ferrous-panel/panel.env
      fi
      PANEL_CONFIG_CHANGED=true
      echo -e "${CYAN}[*] Updated FERROUS_BIND=${EXPLICIT_BIND} in /etc/ferrous-panel/panel.env.${NC}"
    elif ! grep -Eq '^[[:space:]]*(export[[:space:]]+)?FERROUS_BIND=' /etc/ferrous-panel/panel.env 2>/dev/null; then
      echo "FERROUS_BIND=127.0.0.1" >> /etc/ferrous-panel/panel.env
    fi
    if [ -n "${VPS_PUBLIC_IP:-}" ] && [ "${VPS_PUBLIC_IP}" != "127.0.0.1" ]; then
      if ! grep -q '^FERROUS_PUBLIC_IP=' /etc/ferrous-panel/panel.env 2>/dev/null; then
        echo "FERROUS_PUBLIC_IP=${VPS_PUBLIC_IP}" >> /etc/ferrous-panel/panel.env
      else
        sed -i "s|^FERROUS_PUBLIC_IP=.*|FERROUS_PUBLIC_IP=${VPS_PUBLIC_IP}|" /etc/ferrous-panel/panel.env
      fi
    fi
  fi

  if [ -d /etc/systemd/system/ferrous-panel.service.d ]; then
    DROPIN_CLEANED=false
    for dropin in /etc/systemd/system/ferrous-panel.service.d/*.conf; do
      [ -f "$dropin" ] || continue
      if grep -qE '(FERROUS_PORT=|--port)' "$dropin" 2>/dev/null; then
        echo -e "${YELLOW}[!] Removing stale port override in ${dropin}...${NC}"
        sed -i -E '/^[[:space:]]*Environment=.*FERROUS_PORT=/d; /^[[:space:]]*ExecStart=.*--port/d' "$dropin"
        if ! grep -qE '^[[:space:]]*[^#[:space:]\[]' "$dropin" 2>/dev/null; then
          rm -f "$dropin"
        fi
        DROPIN_CLEANED=true
      fi
    done
    if [ "$DROPIN_CLEANED" = true ]; then
      systemctl daemon-reload 2>/dev/null || true
    fi
  fi

  if [ -f /etc/systemd/system/ferrous-panel.service ]; then
    if ! grep -q '^EnvironmentFile=-/etc/ferrous-panel/panel.env$' /etc/systemd/system/ferrous-panel.service 2>/dev/null \
      || ! grep -q '^Restart=always$' /etc/systemd/system/ferrous-panel.service 2>/dev/null \
      || ! grep -q '^TimeoutStopSec=' /etc/systemd/system/ferrous-panel.service 2>/dev/null \
      || grep -qE '(^Environment=.*FERROUS_PORT=|^ExecStart=.*--port)' /etc/systemd/system/ferrous-panel.service 2>/dev/null; then
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
Restart=always
RestartSec=2
TimeoutStopSec=15
LimitNOFILE=65535
NoNewPrivileges=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF_SVC
      systemctl daemon-reload 2>/dev/null || true
    fi
  fi

  ACTIVE_PANEL_PORT="$(grep -E '^[[:space:]]*(export[[:space:]]+)?FERROUS_PORT=' /etc/ferrous-panel/panel.env 2>/dev/null | tail -n 1 | cut -d= -f2 | tr -d '[:space:]\"'\''' || true)"
  ACTIVE_PANEL_PORT="${ACTIVE_PANEL_PORT:-8443}"
  ACTIVE_PANEL_BIND="$(grep -E '^[[:space:]]*(export[[:space:]]+)?FERROUS_BIND=' /etc/ferrous-panel/panel.env 2>/dev/null | tail -n 1 | cut -d= -f2 | tr -d '[:space:]\"'\''' || true)"
  ACTIVE_PANEL_BIND="${ACTIVE_PANEL_BIND:-127.0.0.1}"
  ACTIVE_PANEL_ENTRANCE="$(grep -E '^[[:space:]]*(export[[:space:]]+)?FERROUS_SECURITY_ENTRANCE=' /etc/ferrous-panel/panel.env 2>/dev/null | tail -n 1 | cut -d= -f2 | tr -d '[:space:]\"'\''' || true)"
  ACTIVE_PANEL_ENTRANCE="${ACTIVE_PANEL_ENTRANCE:-/panel}"
  ACTIVE_PANEL_DOMAIN="$(grep -E '^[[:space:]]*(export[[:space:]]+)?FERROUS_CUSTOM_DOMAIN=' /etc/ferrous-panel/panel.env 2>/dev/null | tail -n 1 | cut -d= -f2 | tr -d '[:space:]\"'\''' || true)"
  ACTIVE_SECURE_COOKIE="$(grep -E '^[[:space:]]*(export[[:space:]]+)?FERROUS_SECURE_COOKIE=' /etc/ferrous-panel/panel.env 2>/dev/null | tail -n 1 | cut -d= -f2 | tr -d '[:space:]\"'\''' || true)"

  if [ -n "${ACTIVE_PANEL_DOMAIN}" ] && [ -f "/etc/letsencrypt/live/${ACTIVE_PANEL_DOMAIN}/fullchain.pem" ] && [ "${ACTIVE_SECURE_COOKIE}" != "true" ]; then
    sed -i 's|^FERROUS_SECURE_COOKIE=.*|FERROUS_SECURE_COOKIE=true|' /etc/ferrous-panel/panel.env
    ACTIVE_SECURE_COOKIE="true"
    echo -e "${GREEN}[+] Enabled FERROUS_SECURE_COOKIE=true for HTTPS domain ${ACTIVE_PANEL_DOMAIN}.${NC}"
  elif [ "${ACTIVE_PANEL_BIND}" != "127.0.0.1" ] && [ "${ACTIVE_PANEL_BIND}" != "::1" ] && [ "${ACTIVE_SECURE_COOKIE}" != "true" ]; then
    echo -e "${YELLOW}[WARNING] CẢNH BÁO BẢO MẬT: Panel đang chạy HTTP không mã hóa trên ${ACTIVE_PANEL_BIND}:${ACTIVE_PANEL_PORT} với FERROUS_SECURE_COOKIE=false.${NC}" >&2
  fi

  if command -v ufw >/dev/null 2>&1; then
    ufw allow 80/tcp comment 'HTTP' >/dev/null 2>&1 || true
    ufw allow 443/tcp comment 'HTTPS' >/dev/null 2>&1 || true
    if [ "${ACTIVE_PANEL_BIND}" != "127.0.0.1" ] && [ "${ACTIVE_PANEL_BIND}" != "::1" ]; then
      ufw allow "${ACTIVE_PANEL_PORT}/tcp" comment 'FerrousPanel' >/dev/null 2>&1 || true
    fi
  fi

  ln -sf "${BIN_PANEL}" /usr/local/bin/fp
  ln -sf "${BIN_PANEL}" /usr/local/bin/ferrous
  setup_phpmyadmin_proxy

  PROBE_HOST="${ACTIVE_PANEL_BIND}"
  if [ "${PROBE_HOST}" = "0.0.0.0" ] || [ "${PROBE_HOST}" = "::" ]; then
    PROBE_HOST="127.0.0.1"
  fi

  if [ "$PANEL_BINARY_UPDATED" = true ] || [ "$PANEL_CONFIG_CHANGED" = true ] || ! systemctl is-active --quiet ferrous-panel.service 2>/dev/null; then
    systemctl restart ferrous-panel.service 2>/dev/null || true
    if probe_panel_health "${PROBE_HOST}" "${ACTIVE_PANEL_PORT}" 25; then
      echo -e "${GREEN}[+] ferrous-panel.service is live on ${ACTIVE_PANEL_BIND}:${ACTIVE_PANEL_PORT} (ports 80/443 reserved exclusively for websites).${NC}"
      if [ "${ACTIVE_PANEL_BIND}" = "127.0.0.1" ] || [ "${ACTIVE_PANEL_BIND}" = "::1" ]; then
        echo -e "${GREEN}[+] Panel URL (Local)    : http://127.0.0.1:${ACTIVE_PANEL_PORT}${ACTIVE_PANEL_ENTRANCE}${NC}"
        echo -e "${YELLOW}[!] Loopback-only mode   : Panel chi nghe tren ${ACTIVE_PANEL_BIND} (bao mat mac dinh). Truy cap truc tiep IP cong khai se bao tu choi ket noi.${NC}"
        echo -e "${CYAN}[*] SSH Tunnel (Recommended): ssh -L ${ACTIVE_PANEL_PORT}:127.0.0.1:${ACTIVE_PANEL_PORT} root@${VPS_PUBLIC_IP}${NC}"
        echo -e "${CYAN}[*] Open Public IP Access   : sudo fp bind 0.0.0.0${NC}"
      else
        echo -e "${GREEN}[+] Panel URL (Direct IP): http://${VPS_PUBLIC_IP}:${ACTIVE_PANEL_PORT}${ACTIVE_PANEL_ENTRANCE}${NC}"
        if [ -n "${ACTIVE_PANEL_DOMAIN}" ]; then
          echo -e "${CYAN}[*] Panel URL (Domain)   : http://${ACTIVE_PANEL_DOMAIN}:${ACTIVE_PANEL_PORT}${ACTIVE_PANEL_ENTRANCE}${NC}"
          echo -e "${YELLOW}[!] Cloudflare Note      : Keep '${ACTIVE_PANEL_DOMAIN}' on DNS-Only (Gray Cloud) to reach HTTP :${ACTIVE_PANEL_PORT}; Cloudflare Proxy (Orange Cloud) blocks plain HTTP on :8443.${NC}"
        fi
      fi
    else
      echo -e "${RED}[ERROR] ferrous-panel.service failed /api/health on ${PROBE_HOST}:${ACTIVE_PANEL_PORT} after update!${NC}" >&2
      journalctl -u ferrous-panel.service -n 20 --no-pager >&2 || true
      if [ "$PANEL_BINARY_UPDATED" = true ] && [ -x "${ROLLBACK_DIR}/ferrous-panel.prev" ]; then
        perform_panel_rollback
        echo -e "${RED}[ERROR] Update aborted & automatically rolled back to previous working binary.${NC}" >&2
      fi
      exit 1
    fi
  else
    echo -e "${GREEN}[+] ferrous-panel.service already active on ${ACTIVE_PANEL_BIND}:${ACTIVE_PANEL_PORT} (zero restart needed).${NC}"
    if [ "${ACTIVE_PANEL_BIND}" = "127.0.0.1" ] || [ "${ACTIVE_PANEL_BIND}" = "::1" ]; then
      echo -e "${YELLOW}[!] Loopback-only mode   : Panel chi nghe tren ${ACTIVE_PANEL_BIND}. Dung 'ssh -L ${ACTIVE_PANEL_PORT}:127.0.0.1:${ACTIVE_PANEL_PORT} root@${VPS_PUBLIC_IP}' hoac 'sudo fp bind 0.0.0.0'.${NC}"
    fi
  fi
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

if [ "$SYNC_JS_TOOLCHAIN" = true ]; then
  install_zero_node_js_css_toolchain
fi

if [ "$FORCE_FERROUSX" = true ] || [ -x "${BIN_FERROUSX}" ]; then
  update_ferrousx_from_github
fi

if [ "$FORCE_NGINX" = true ] || systemctl is-active --quiet nginx.service 2>/dev/null; then
  update_nginx_engine
fi

UPDATE_ELAPSED="$(( $(date +%s) - UPDATE_START_TS ))"
echo -e "${GREEN}${BOLD}Update completed cleanly in ${UPDATE_ELAPSED}s. State in /var/lib/ferrous-panel, /etc/nginx, and /etc/ferrousx preserved.${NC}"
if [ -x /usr/local/bin/bun ] || [ -x /usr/local/bin/tailwindcss ]; then
  echo -e "${CYAN}Zero-Node.js JS/CSS Toolchain: bun=$(/usr/local/bin/bun --version 2>/dev/null || echo 'missing') | tailwindcss=$([ -x /usr/local/bin/tailwindcss ] && echo 'v4-standalone' || echo 'missing') (Node.js NOT required).${NC}"
fi
