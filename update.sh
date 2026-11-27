#!/usr/bin/env bash
set -euo pipefail
BOLD='\033[1m'; RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; CYAN='\033[0;36m'; NC='\033[0m'
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
UPDATE_PANEL=true; FORCE_FERROUSX=false; FORCE_NGINX=false; SYNC_JS_TOOLCHAIN=true; INSECURE_SKIP_VERIFY=false
CHECK_ONLY=false; PREBUILT_ONLY=false; SOURCE_ONLY=false; FULL_LTO=false; FORCE_REBUILD=false; DO_ROLLBACK=false
EXPLICIT_BIND=""; EXPLICIT_DOMAIN=""; PANEL_CONFIG_CHANGED=false
usage() {
cat <<'EOF_HELP'
FerrousPanel, Nginx & FerrousX Updater — Usage: sudo fp update [OPTIONS]
  --check | --prebuilt | --source | --full-lto | --force | --rollback
  --nginx / --nginx-only       Sync, validate & reload Nginx
  --ferrousx / --ferrousx-only Sync & rebuild FerrousX
  --js-only / --no-js-toolchain Sync or skip Bun + Tailwind CSS v4
  --engine <nginx|ferrousx>    Target web engine
  --branch <B> | --ferrousx-branch <B> | --version <TAG>
  --bind <IP> | --domain <DOMAIN> | --insecure-skip-verify | -h, --help
EOF_HELP
}
while [[ $# -gt 0 ]]; do
case "$1" in
--check) CHECK_ONLY=true; SYNC_JS_TOOLCHAIN=false; shift ;;
--prebuilt|--prebuilt-binary) PREBUILT_ONLY=true; shift ;;
--source) SOURCE_ONLY=true; shift ;;
--full-lto) FULL_LTO=true; shift ;;
--force|--force-rebuild) FORCE_REBUILD=true; shift ;;
--rollback) DO_ROLLBACK=true; SYNC_JS_TOOLCHAIN=false; shift ;;
--nginx|--with-nginx) FORCE_NGINX=true; shift ;;
--nginx-only) FORCE_NGINX=true; UPDATE_PANEL=false; shift ;;
--ferrousx|--with-ferrousx) FORCE_FERROUSX=true; shift ;;
--ferrousx-only) FORCE_FERROUSX=true; UPDATE_PANEL=false; shift ;;
--js-only) SYNC_JS_TOOLCHAIN=true; UPDATE_PANEL=false; shift ;;
--no-js-toolchain) SYNC_JS_TOOLCHAIN=false; shift ;;
--insecure-skip-verify) INSECURE_SKIP_VERIFY=true; shift ;;
--engine)
case "${2:-}" in
ferrousx) FORCE_FERROUSX=true ;;
nginx) FORCE_NGINX=true ;;
*) echo -e "${RED}[ERROR] --engine must be 'nginx' or 'ferrousx' (got '${2:-}').${NC}" >&2; exit 1 ;;
esac
shift 2 ;;
--ferrousx-branch) FERROUSX_BRANCH="$2"; shift 2 ;;
--branch) GIT_BRANCH="$2"; shift 2 ;;
--version) RELEASE_TAG="$2"; shift 2 ;;
--bind)
EXPLICIT_BIND="${2:-}"
if [ -z "${EXPLICIT_BIND}" ] || ! [[ "${EXPLICIT_BIND}" =~ ^[0-9a-fA-F.:]+$ ]]; then
echo -e "${RED}[ERROR] --bind requires a valid IP address (e.g. 0.0.0.0 or 127.0.0.1).${NC}" >&2; exit 1
fi
shift 2 ;;
--domain) EXPLICIT_DOMAIN="${2:-}"; shift 2 ;;
-h|--help) usage; exit 0 ;;
-*) echo -e "${RED}[ERROR] Unknown option: $1${NC}" >&2; usage >&2; exit 1 ;;
*) RELEASE_TAG="$1"; shift ;;
esac
done
if [ "$CHECK_ONLY" = false ] && command -v flock >/dev/null 2>&1; then
mkdir -p /run
exec 9>/run/ferrous-update.lock
if ! flock -n 9; then
echo -e "${RED}[ERROR] Another FerrousPanel update is already running (/run/ferrous-update.lock).${NC}" >&2; exit 1
fi
fi
mkdir -p "${DATA_DIR}" "${ROLLBACK_DIR}"
UPDATE_START_TS="$(date +%s)"
export RUSTUP_HOME="${RUSTUP_HOME:-/root/.rustup}" CARGO_HOME="${CARGO_HOME:-/root/.cargo}"
export PATH="${CARGO_HOME}/bin:/usr/local/bin:${PATH}"
ARCH_RAW="$(uname -m)"
case "$ARCH_RAW" in
x86_64|amd64) ARCH="x86_64" ;;
aarch64|arm64) ARCH="aarch64" ;;
*) echo -e "${RED}[ERROR] Unsupported CPU architecture: ${ARCH_RAW}${NC}" >&2; exit 1 ;;
esac
if [ "$RELEASE_TAG" = "latest" ]; then
RELEASE_BASE="https://github.com/${REPO_SLUG}/releases/latest/download"
else
RELEASE_BASE="https://github.com/${REPO_SLUG}/releases/download/${RELEASE_TAG}"
fi
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT
verify_sha256_entry() {
local sums_file="$1" asset_name="$2" work_dir="$3" label="$4"
if [ ! -f "${sums_file}" ]; then
[ "${INSECURE_SKIP_VERIFY}" = true ] && return 0
echo -e "${RED}[ERROR] Cannot download SHA-256 checksum manifest for ${label} (${asset_name}).${NC}" >&2; exit 1
fi
local matched_line
matched_line="$(grep -E "[[:space:]](\./|\*)?${asset_name}$" "${sums_file}" | head -n 1 || true)"
if [ -z "${matched_line}" ]; then
[ "${INSECURE_SKIP_VERIFY}" = true ] && return 0
echo -e "${RED}[ERROR] No SHA-256 entry matching '${asset_name}' in ${label}.${NC}" >&2; exit 1
fi
local expected_hash
expected_hash="$(printf '%s\n' "${matched_line}" | awk '{print $1}')"
if ! (cd "${work_dir}" && printf '%s  %s\n' "${expected_hash}" "${asset_name}" | sha256sum -c -); then
echo -e "${RED}[ERROR] SHA-256 verification FAILED for ${asset_name} (${label}).${NC}" >&2; exit 1
fi
}
download_verified_release_asset() {
local asset_name="$1" dest_path="$2" tmp_asset="${TMP_DIR}/${asset_name}"
curl -fsSL --connect-timeout 10 -o "${TMP_DIR}/SHA256SUMS.txt" "${RELEASE_BASE}/SHA256SUMS.txt" 2>/dev/null || rm -f "${TMP_DIR}/SHA256SUMS.txt"
curl -fsSL --retry 3 --connect-timeout 10 -o "${tmp_asset}" "${RELEASE_BASE}/${asset_name}"
verify_sha256_entry "${TMP_DIR}/SHA256SUMS.txt" "${asset_name}" "${TMP_DIR}" "FerrousPanel release"
install -m 0755 "${tmp_asset}" "${dest_path}"
}
ensure_rust_toolchain() {
local min_ver_code=189
if command -v rustc >/dev/null 2>&1; then
local ver_code
ver_code="$(rustc --version | awk '{print $2}' | awk -F'.' '{print ($1 * 100) + $2}' 2>/dev/null || echo 0)"
[ "${ver_code:-0}" -ge "${min_ver_code}" ] && return 0
else
return 0
fi
if command -v rustup >/dev/null 2>&1; then
rustup toolchain install "${RUST_TOOLCHAIN}" --profile minimal && rustup default "${RUST_TOOLCHAIN}"
else
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "${RUST_TOOLCHAIN}"
fi
for _b in cargo rustc rustup; do
[ -x "${CARGO_HOME}/bin/${_b}" ] && ln -sf "${CARGO_HOME}/bin/${_b}" "/usr/local/bin/${_b}"
done
}
ensure_swap_for_build() {
local ram_mb swap_mb
ram_mb="$(awk '/MemTotal:/ {printf "%d", $2/1024}' /proc/meminfo 2>/dev/null || echo 0)"
swap_mb="$(awk '/SwapTotal:/ {printf "%d", $2/1024}' /proc/meminfo 2>/dev/null || echo 0)"
if [ "${ram_mb}" -gt 0 ] && [ "${ram_mb}" -lt 2500 ] && [ "${swap_mb}" -lt 512 ] && [ ! -f /swapfile ]; then
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
if [ "${ram_mb}" -lt 1500 ]; then jobs=1
elif [ "${ram_mb}" -lt 2800 ]; then jobs=$(( cpus > 2 ? 2 : cpus ))
else jobs="${cpus}"; fi
export CARGO_BUILD_JOBS="${jobs}"
fi
export CARGO_INCREMENTAL=1
if [ -z "${RUSTFLAGS:-}" ]; then
if command -v mold >/dev/null 2>&1; then export RUSTFLAGS="-C link-arg=-fuse-ld=mold"
elif command -v lld >/dev/null 2>&1 || command -v ld.lld >/dev/null 2>&1; then export RUSTFLAGS="-C link-arg=-fuse-ld=lld"; fi
fi
}
atomic_install_binary() {
local src_bin="$1" dst_bin="$2" backup_name="$3" label="$4"
if [ ! -x "${src_bin}" ] || ! "${src_bin}" --version >/dev/null 2>&1; then
echo -e "${RED}[ERROR] Candidate binary for ${label} failed pre-flight '--version': ${src_bin}${NC}" >&2; exit 1
fi
if [ -x "${dst_bin}" ]; then
cp -f "${dst_bin}" "${ROLLBACK_DIR}/${backup_name}" && chmod 0755 "${ROLLBACK_DIR}/${backup_name}"
fi
install -m 0755 "${src_bin}" "${dst_bin}.new" && mv -f "${dst_bin}.new" "${dst_bin}"
}
probe_panel_health() {
local host="$1" port="$2" attempts="${3:-25}"
for _ in $(seq 1 "${attempts}"); do
if curl -fsS --max-time 2 "http://${host}:${port}/api/health" >/dev/null 2>&1 \
|| curl -kfsS --max-time 2 "https://${host}:${port}/api/health" >/dev/null 2>&1; then
return 0
fi
sleep 0.2
done
return 1
}
read_panel_env_key() {
local key="$1" default_val="${2:-}"
if [ -f /etc/ferrous-panel/panel.env ]; then
local val
val="$(grep -E "^[[:space:]]*(export[[:space:]]+)?${key}=" /etc/ferrous-panel/panel.env 2>/dev/null | tail -n 1 | cut -d= -f2 | tr -d '[:space:]\"'\''' || true)"
if [ -n "${val}" ]; then echo "${val}"; return 0; fi
fi
echo "${default_val}"
}
perform_panel_rollback() {
local prev_bin="${ROLLBACK_DIR}/ferrous-panel.prev"
if [ ! -x "${prev_bin}" ]; then
echo -e "${RED}[ERROR] No rollback binary found at ${prev_bin}.${NC}" >&2; exit 1
fi
install -m 0755 "${prev_bin}" "${BIN_PANEL}.new" && mv -f "${BIN_PANEL}.new" "${BIN_PANEL}"
[ -f "${ROLLBACK_DIR}/ferrous-panel.prev.commit" ] && cp -f "${ROLLBACK_DIR}/ferrous-panel.prev.commit" "${PANEL_COMMIT_FILE}"
systemctl restart ferrous-panel.service 2>/dev/null || true
local port bind host
port="$(read_panel_env_key FERROUS_PORT 8443)"
bind="$(read_panel_env_key FERROUS_BIND 127.0.0.1)"
host="${bind}"
if [ "${host}" = "0.0.0.0" ] || [ "${host}" = "::" ]; then host="127.0.0.1"; fi
if probe_panel_health "${host}" "${port}" 25; then
echo -e "${GREEN}[+] Rollback succeeded: ferrous-panel.service is healthy on ${bind}:${port}.${NC}"
else
echo -e "${RED}[ERROR] Rollback binary restarted but /api/health did not respond on ${host}:${port}.${NC}" >&2; exit 1
fi
}
if [ "$DO_ROLLBACK" = true ]; then
perform_panel_rollback
if [ -x "${ROLLBACK_DIR}/ferrousx-bin.prev" ] && [ -x "${BIN_FERROUSX}" ]; then
install -m 0755 "${ROLLBACK_DIR}/ferrousx-bin.prev" "${BIN_FERROUSX}.new" && mv -f "${BIN_FERROUSX}.new" "${BIN_FERROUSX}"
systemctl is-active --quiet ferrousx.service 2>/dev/null && systemctl restart ferrousx.service || true
fi
exit 0
fi
if [ "$CHECK_ONLY" = true ]; then
LOCAL_VER="$("${BIN_PANEL}" --version 2>/dev/null || echo 'not-installed')"
LOCAL_SHA="$(cat "${PANEL_COMMIT_FILE}" 2>/dev/null || git -C "${SRC_DIR}" rev-parse HEAD 2>/dev/null || echo 'unknown')"
REMOTE_SHA=""
command -v git >/dev/null 2>&1 && REMOTE_SHA="$(git ls-remote --heads "https://github.com/${REPO_SLUG}.git" "refs/heads/${GIT_BRANCH}" 2>/dev/null | awk '{print $1}' | head -n 1 || true)"
echo -e "${CYAN}FerrousPanel Update Check (${REPO_SLUG}@${GIT_BRANCH})${NC}"
echo -e "  Installed Version : ${BOLD}${LOCAL_VER}${NC}"
echo -e "  Installed Commit  : ${BOLD}${LOCAL_SHA:0:12}${NC}"
echo -e "  Upstream Commit   : ${BOLD}${REMOTE_SHA:0:12}${NC}"
[ -n "${REMOTE_SHA}" ] && [ "${LOCAL_SHA}" = "${REMOTE_SHA}" ] && echo -e "${GREEN}[+] Already up to date.${NC}" || echo -e "${YELLOW}[!] Run 'sudo fp update' to update.${NC}"
exit 0
fi
[ "$PREBUILT_ONLY" = false ] && ensure_rust_toolchain
detect_local_ip() {
local ip=""
ip="$(ip -4 route get 1.1.1.1 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i=="src") {print $(i+1); exit}}' || true)"
[ -z "$ip" ] && ip="$(hostname -I 2>/dev/null | awk '{print $1}' || true)"
echo "${ip:-127.0.0.1}"
}
detect_public_ip() {
local cached=""
cached="$(read_panel_env_key FERROUS_PUBLIC_IP "")"
if [[ "$cached" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}$ ]] && [ "$cached" != "127.0.0.1" ]; then echo "$cached"; return 0; fi
local ip=""
for url in "https://api.ipify.org" "https://ifconfig.me/ip" "https://ipv4.icanhazip.com"; do
ip="$(curl -4 -fsS --connect-timeout 2 --max-time 2 "$url" 2>/dev/null | tr -d '[:space:]' || true)"
if [[ "$ip" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}$ ]]; then echo "$ip"; return 0; fi
done
detect_local_ip
}
VPS_PUBLIC_IP="$(detect_public_ip)"
ensure_default_landing_page() {
mkdir -p /www/server/default /www/wwwroot
chmod 0755 /www /www/server /www/server/default /www/wwwroot 2>/dev/null || true
if [ -f "${SRC_DIR}/crates/ferrous-panel/src/modules/default_pages.rs" ]; then
awk '/^const UNCONFIGURED_HOST_HTML:/{f=1;next} f&&/^"##;$/{exit} f{print}' "${SRC_DIR}/crates/ferrous-panel/src/modules/default_pages.rs" > /www/server/default/index.html.tmp 2>/dev/null || true
if [ -s /www/server/default/index.html.tmp ] && grep -q 'FerrousPanel-Managed-Default-Index' /www/server/default/index.html.tmp 2>/dev/null; then
chmod 0644 /www/server/default/index.html.tmp && mv -f /www/server/default/index.html.tmp /www/server/default/index.html
else rm -f /www/server/default/index.html.tmp; fi
fi
if [ ! -f /www/server/default/index.html ]; then
printf '<!DOCTYPE html><!-- FerrousPanel-Managed-Default-Index --><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Ferrous Edge Node</title><style>body{margin:0;min-height:100vh;display:grid;place-items:center;background:#060913;color:#f8fafc;font-family:system-ui,sans-serif}.c{max-width:560px;padding:28px;border-radius:16px;background:#0f172a;border:1px solid #334155}code{color:#fb923c}</style></head><body><main class="c"><h2>Ferrous Edge Server Ready</h2><p><code>{{HOST}}</code> is online. Add this domain in <b>FerrousPanel → Websites &amp; Apps</b> to deploy your site.</p></main></body></html>\n' > /www/server/default/index.html
chmod 0644 /www/server/default/index.html 2>/dev/null || true
fi
if [ ! -f /www/wwwroot/index.html ] || grep -qE 'Welcome to nginx|FerrousPanel-Managed-Default-Index' /www/wwwroot/index.html 2>/dev/null; then
cp -f /www/server/default/index.html /www/wwwroot/index.html 2>/dev/null || true
chmod 0644 /www/wwwroot/index.html 2>/dev/null || true
fi
}
setup_phpmyadmin_proxy() {
ensure_default_landing_page
if systemctl is-active --quiet ferrous-pma.service 2>/dev/null || systemctl is-enabled --quiet ferrous-pma.service 2>/dev/null; then
systemctl disable --now ferrous-pma.service 2>/dev/null || true
fi
rm -f /etc/systemd/system/ferrous-pma.service /etc/ferrousx/pma-router.php 2>/dev/null || true
local pma_dir="/www/server/phpmyadmin" pma_router="/www/server/ferrous-pma-router.php" pma_tmp="/var/lib/phpmyadmin/tmp" pma_port="18889"
mkdir -p /www/server "${pma_tmp}"
chmod 0755 /www /www/server /var/lib/phpmyadmin 2>/dev/null || true
chmod 0770 "${pma_tmp}" 2>/dev/null || true
local php_bin="" php_ver=""
for cand in /usr/bin/php8.3 /usr/bin/php8.2 /usr/bin/php8.1 /usr/bin/php8.4 /usr/bin/php /usr/bin/php7.4; do
if [ -x "$cand" ]; then
php_bin="$cand"
case "$cand" in /usr/bin/php[0-9].[0-9]) php_ver="${cand#/usr/bin/php}" ;; esac
break
fi
done
if { [ -z "$php_bin" ] || ! "$php_bin" -m 2>/dev/null | grep -qi '^mysqli$' || ! "$php_bin" -m 2>/dev/null | grep -qi '^mbstring$' || ! "$php_bin" -m 2>/dev/null | grep -qi '^xml$'; } && command -v apt-get >/dev/null 2>&1; then
DEBIAN_FRONTEND=noninteractive apt-get update -qq >/dev/null 2>&1 || true
[ -n "$php_ver" ] && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold \
"php${php_ver}-cli" "php${php_ver}-mysql" "php${php_ver}-mbstring" "php${php_ver}-xml" "php${php_ver}-zip" "php${php_ver}-curl" "php${php_ver}-gd" "php${php_ver}-intl" "php${php_ver}-bz2" >/dev/null 2>&1 || true
DEBIAN_FRONTEND=noninteractive apt-get install -y -qq -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold \
php-cli php-mysql php-mbstring php-xml php-zip php-curl php-gd php-intl php-bz2 >/dev/null 2>&1 || true
for cand in /usr/bin/php8.3 /usr/bin/php8.2 /usr/bin/php8.1 /usr/bin/php8.4 /usr/bin/php /usr/bin/php7.4; do
if [ -x "$cand" ]; then php_bin="$cand"; break; fi
done
fi
if [ ! -f "${pma_dir}/index.php" ]; then
mkdir -p "${pma_dir}"
local pma_tar="${TMP_DIR}/phpMyAdmin-official.tar.gz"
for _v in 5.2.3 5.2.2 5.2.1; do
if curl -fsSL --retry 2 --connect-timeout 15 -o "${pma_tar}" "https://files.phpmyadmin.net/phpMyAdmin/${_v}/phpMyAdmin-${_v}-all-languages.tar.gz"; then
tar -xzf "${pma_tar}" --strip-components=1 -C "${pma_dir}" || true
rm -f "${pma_tar}"
break
fi
done
fi
if [ -f "${pma_dir}/index.php" ] && [ -n "$php_bin" ]; then
local sso_file="/var/lib/phpmyadmin/myphpadmin.sso.key" legacy_sso_file="/etc/ferrous-panel/myphpadmin.sso.key"
mkdir -p /var/lib/phpmyadmin /etc/ferrous-panel
chmod 0755 /var/lib/phpmyadmin 2>/dev/null || true
if [ ! -s "${sso_file}" ]; then
if [ -s "${legacy_sso_file}" ]; then cp -f "${legacy_sso_file}" "${sso_file}"
else openssl rand -hex 16 2>/dev/null > "${sso_file}" || printf 'FerrousMyphpadminSsoSecretKey32x' > "${sso_file}"; fi
fi
cp -f "${sso_file}" "${legacy_sso_file}" 2>/dev/null || true
chown root:www-data "${sso_file}" 2>/dev/null || true
chmod 0640 "${sso_file}" 2>/dev/null || true
chmod 0600 "${legacy_sso_file}" 2>/dev/null || true
local sso_pw
sso_pw="$(tr -d '[:space:]' < "${sso_file}" 2>/dev/null || true)"
if [ -n "${sso_pw}" ] && command -v mysql >/dev/null 2>&1; then
mysql --batch --raw -e "CREATE DATABASE IF NOT EXISTS phpmyadmin CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci; CREATE USER IF NOT EXISTS 'fp_myphpadmin'@'localhost' IDENTIFIED BY '${sso_pw}'; CREATE USER IF NOT EXISTS 'fp_myphpadmin'@'127.0.0.1' IDENTIFIED BY '${sso_pw}'; ALTER USER 'fp_myphpadmin'@'localhost' IDENTIFIED BY '${sso_pw}'; ALTER USER 'fp_myphpadmin'@'127.0.0.1' IDENTIFIED BY '${sso_pw}'; GRANT ALL PRIVILEGES ON *.* TO 'fp_myphpadmin'@'localhost' WITH GRANT OPTION; GRANT ALL PRIVILEGES ON *.* TO 'fp_myphpadmin'@'127.0.0.1' WITH GRANT OPTION; FLUSH PRIVILEGES;" >/dev/null 2>&1 || true
[ -f "${pma_dir}/sql/create_tables.sql" ] && mysql phpmyadmin < "${pma_dir}/sql/create_tables.sql" >/dev/null 2>&1 || true
fi
if [ ! -f "${pma_dir}/config.inc.php" ] || ! grep -q "PmaNoRelation_DisableWarning" "${pma_dir}/config.inc.php" 2>/dev/null; then
local bf_secret
bf_secret="$(openssl rand -hex 16 2>/dev/null || echo 'FerrousPanelOfficialPmaSecretKey')"
cat > "${pma_dir}/config.inc.php" <<EOF_PMA_CFG
<?php
declare(strict_types=1);
\$cfg['blowfish_secret'] = '${bf_secret}';
\$ferrousSsoFile = '${sso_file}';
\$ferrousSsoLegacyFile = '${legacy_sso_file}';
\$ferrousSsoActive = !empty(\$_SERVER['HTTP_X_FERROUS_PMA_AUTH']) && \$_SERVER['HTTP_X_FERROUS_PMA_AUTH'] === '1';
\$ferrousSsoPass = '';
if (\$ferrousSsoActive) {
  if (!empty(\$_SERVER['HTTP_X_FERROUS_PMA_PASS'])) \$ferrousSsoPass = trim((string) \$_SERVER['HTTP_X_FERROUS_PMA_PASS']);
  elseif (is_readable(\$ferrousSsoFile)) \$ferrousSsoPass = trim((string) @file_get_contents(\$ferrousSsoFile));
  elseif (is_readable(\$ferrousSsoLegacyFile)) \$ferrousSsoPass = trim((string) @file_get_contents(\$ferrousSsoLegacyFile));
}
for (\$i = 1; \$i <= 2; \$i++) {
  \$cfg['Servers'][\$i]['verbose'] = \$i === 1 ? 'FPSql & Database — MariaDB / MySQL (Socket)' : '127.0.0.1:3306 (TCP)';
  if (\$ferrousSsoPass !== '') {
    \$cfg['Servers'][\$i]['auth_type'] = 'config';
    \$cfg['Servers'][\$i]['user'] = 'fp_myphpadmin';
    \$cfg['Servers'][\$i]['password'] = \$ferrousSsoPass;
    \$cfg['Servers'][\$i]['controluser'] = 'fp_myphpadmin';
    \$cfg['Servers'][\$i]['controlpass'] = \$ferrousSsoPass;
  } else {
    \$cfg['Servers'][\$i]['auth_type'] = 'cookie';
  }
  \$cfg['Servers'][\$i]['host'] = \$i === 1 ? 'localhost' : '127.0.0.1';
  if (\$i === 2) \$cfg['Servers'][\$i]['port'] = '3306';
  elseif (file_exists('/run/mysqld/mysqld.sock')) \$cfg['Servers'][\$i]['socket'] = '/run/mysqld/mysqld.sock';
  \$cfg['Servers'][\$i]['compress'] = false;
  \$cfg['Servers'][\$i]['AllowNoPassword'] = false;
  \$cfg['Servers'][\$i]['pmadb'] = 'phpmyadmin';
  \$cfg['Servers'][\$i]['bookmarktable'] = 'pma__bookmark';
  foreach (['relation','table_info','table_coords','pdf_pages','column_info','history','table_uiprefs','tracking','userconfig','recent','favorite','users','usergroups','navigationhiding','savedsearches','central_columns','designer_settings','export_templates'] as \$t) \$cfg['Servers'][\$i][\$t] = 'pma__' . \$t;
}
\$cfg['PmaNoRelation_DisableWarning'] = true;
\$cfg['ZeroConf'] = true;
\$cfg['VersionCheck'] = false;
\$cfg['TempDir'] = '${pma_tmp}';
\$cfg['AllowThirdPartyFraming'] = 'sameorigin';
\$cfg['ExecTimeLimit'] = 300;
\$cfg['SendErrorReports'] = 'never';
EOF_PMA_CFG
chmod 0640 "${pma_dir}/config.inc.php"
fi
cat > "${pma_router}" <<'EOF_PMA_ROUTER'
<?php
if (!empty($_SERVER['HTTP_X_FORWARDED_HOST'])) $_SERVER['HTTP_HOST'] = $_SERVER['HTTP_X_FORWARDED_HOST'];
if (!empty($_SERVER['HTTP_X_FORWARDED_PROTO']) && strtolower($_SERVER['HTTP_X_FORWARDED_PROTO']) === 'https') { $_SERVER['HTTPS'] = 'on'; $_SERVER['SERVER_PORT'] = '443'; }
if (!empty($_SERVER['HTTP_HOST']) && strpos($_SERVER['HTTP_HOST'], ':') !== false) { [$hN, $hP] = explode(':', $_SERVER['HTTP_HOST'], 2); if ($hN !== '') $_SERVER['SERVER_NAME'] = $hN; if (ctype_digit($hP)) $_SERVER['SERVER_PORT'] = $hP; }
$uri = parse_url($_SERVER['REQUEST_URI'] ?? '/', PHP_URL_PATH) ?: '/';
if ($uri === '/phpmyadmin') { header('Location: /phpmyadmin/', true, 302); exit; }
if (strpos($uri, '/phpmyadmin/') !== 0) { http_response_code(403); exit('Forbidden'); }
$rel = substr($uri, strlen('/phpmyadmin'));
if ($rel === '' || $rel === '/') $rel = '/index.php';
$relLower = strtolower($rel);
foreach (['/setup','/libraries','/templates','/vendor','/sql','/locale','/.git','/test'] as $pfx) {
  if ($relLower === $pfx || strpos($relLower, $pfx . '/') === 0) { http_response_code(403); exit('Forbidden'); }
}
if (in_array($relLower, ['/config.inc.php','/config.sample.inc.php','/composer.json','/composer.lock','/package.json','/yarn.lock'], true)) { http_response_code(403); exit('Forbidden'); }
$base = realpath('/www/server/phpmyadmin');
if ($base === false) { http_response_code(503); exit('Myphpadmin directory missing'); }
$real = realpath($base . $rel);
if ($real === false || strpos($real, $base) !== 0) { http_response_code(404); exit('Not Found'); }
if (is_dir($real)) {
  $idx = $real . '/index.php';
  if (is_file($idx)) { $real = $idx; $rel = rtrim($rel, '/') . '/index.php'; }
  else { http_response_code(403); exit('Forbidden'); }
}
if (strtolower(pathinfo($real, PATHINFO_EXTENSION)) !== 'php') return false;
chdir(dirname($real));
$_SERVER['SCRIPT_FILENAME'] = $real;
$_SERVER['SCRIPT_NAME'] = '/phpmyadmin' . $rel;
$_SERVER['PHP_SELF'] = '/phpmyadmin' . $rel;
require $real;
EOF_PMA_ROUTER
chmod 0644 "${pma_router}"
chmod 0755 "${pma_dir}" 2>/dev/null || true
chown -R www-data:www-data "${pma_dir}" "${pma_tmp}" 2>/dev/null || true
cat > /etc/systemd/system/ferrous-phpmyadmin.service <<EOF_PMA_UNIT
[Unit]
Description=FerrousPanel Myphpadmin Worker (127.0.0.1:${pma_port})
After=network.target mariadb.service mysql.service
[Service]
Type=simple
User=www-data
Group=www-data
WorkingDirectory=${pma_dir}
Environment=PHP_CLI_SERVER_WORKERS=4
ExecStart=${php_bin} -S 127.0.0.1:${pma_port} -d upload_max_filesize=128M -d post_max_size=128M -d memory_limit=256M -d max_execution_time=300 -t /www/server ${pma_router}
Restart=always
RestartSec=2
NoNewPrivileges=true
PrivateTmp=true
[Install]
WantedBy=multi-user.target
EOF_PMA_UNIT
systemctl daemon-reload 2>/dev/null || true
systemctl enable ferrous-phpmyadmin.service >/dev/null 2>&1 || true
systemctl restart ferrous-phpmyadmin.service >/dev/null 2>&1 || true
else
systemctl daemon-reload 2>/dev/null || true
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
grep -q '^FERROUSX_ENABLE_PHPMYADMIN=' "${FERROUSX_ENV_FILE}" 2>/dev/null || echo "FERROUSX_ENABLE_PHPMYADMIN=true" >> "${FERROUSX_ENV_FILE}"
grep -q '^FERROUSX_PHPMYADMIN_UPSTREAM=' "${FERROUSX_ENV_FILE}" 2>/dev/null || echo "FERROUSX_PHPMYADMIN_UPSTREAM=127.0.0.1:888" >> "${FERROUSX_ENV_FILE}"
fi
if [ ! -f /etc/ferrousx/ferrousx.toml ]; then
cat > /etc/ferrousx/ferrousx.toml <<'EOF_FX'
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
grep -Eq '^[[:space:]]*enable_phpmyadmin_route[[:space:]]*=' /etc/ferrousx/ferrousx.toml 2>/dev/null || echo 'enable_phpmyadmin_route = true' >> /etc/ferrousx/ferrousx.toml
grep -Eq '^[[:space:]]*phpmyadmin_upstream[[:space:]]*=' /etc/ferrousx/ferrousx.toml 2>/dev/null || echo 'phpmyadmin_upstream = "127.0.0.1:888"' >> /etc/ferrousx/ferrousx.toml
fi
setup_phpmyadmin_proxy
if [ -f /etc/profile.d/ferrous-panel.sh ] && ! grep -q '/etc/ferrousx/ferrousx.env' /etc/profile.d/ferrous-panel.sh 2>/dev/null; then
cat >> /etc/profile.d/ferrous-panel.sh <<'EOF_FX_PROF'
export FERROUSX_ENV_FILE="/etc/ferrousx/ferrousx.env"
if [ -r "/etc/ferrousx/ferrousx.env" ]; then set -a; . /etc/ferrousx/ferrousx.env; set +a; fi
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
local fx_installed=false fx_skipped=false fx_target_sha="" fx_dir="${FERROUSX_SRC_DIR}"
if [ "$PREBUILT_ONLY" = false ] && command -v cargo >/dev/null 2>&1 && command -v git >/dev/null 2>&1; then
mkdir -p "$(dirname "${fx_dir}")"
if (
if [ -d "${fx_dir}/.git" ]; then
git -C "${fx_dir}" fetch --depth 1 --force --quiet origin "${FERROUSX_BRANCH}:refs/remotes/origin/${FERROUSX_BRANCH}" || \
git -C "${fx_dir}" fetch --depth 1 --quiet origin "${FERROUSX_BRANCH}"
else
rm -rf "${fx_dir}" && git clone --depth 1 --branch "${FERROUSX_BRANCH}" "https://github.com/${FERROUSX_REPO}.git" "${fx_dir}"
fi
); then
fx_target_sha="$(git -C "${fx_dir}" rev-parse "origin/${FERROUSX_BRANCH}" 2>/dev/null || git -C "${fx_dir}" rev-parse FETCH_HEAD 2>/dev/null || git -C "${fx_dir}" rev-parse HEAD 2>/dev/null || echo '')"
local fx_cache_tag="${fx_target_sha:+${fx_target_sha}-fp-ssl-v6}" fx_prev_sha
fx_prev_sha="$(cat "${FERROUSX_COMMIT_FILE}" 2>/dev/null || echo '')"
if [ "$FORCE_REBUILD" = false ] && [ -x "${BIN_FERROUSX}" ] && [ -n "${fx_cache_tag}" ] && [ "${fx_prev_sha}" = "${fx_cache_tag}" ]; then
echo -e "${GREEN}[+] FerrousX already up to date (${fx_target_sha:0:8}).${NC}"
fx_installed=true; fx_skipped=true
else
git -C "${fx_dir}" reset --hard "origin/${FERROUSX_BRANCH}" >/dev/null 2>&1 || git -C "${fx_dir}" reset --hard FETCH_HEAD >/dev/null 2>&1 || true
fi
fi
if [ "$fx_skipped" = false ] && [ -d "${fx_dir}/.git" ]; then
local fx_commit
fx_commit="$(git -C "${fx_dir}" rev-parse --short HEAD 2>/dev/null || echo 'main')"
[ -f "${fx_dir}/Cargo.toml" ] && sed -i '/^rust-version = /d' "${fx_dir}/Cargo.toml"
if [ -f "${fx_dir}/crates/ferrousx-core/Cargo.toml" ] && ! grep -Eq '^[[:space:]]*anyhow[[:space:]]*[.=]' "${fx_dir}/crates/ferrousx-core/Cargo.toml"; then
awk '/^\[dependencies\]$/ { print; print "anyhow.workspace = true"; next } { print }' "${fx_dir}/crates/ferrousx-core/Cargo.toml" > "${fx_dir}/crates/ferrousx-core/Cargo.toml.tmp" && mv "${fx_dir}/crates/ferrousx-core/Cargo.toml.tmp" "${fx_dir}/crates/ferrousx-core/Cargo.toml"
fi
if [ -f "${fx_dir}/crates/ferrousx-bin/Cargo.toml" ]; then
for _dep in tokio-rustls toml; do
if ! grep -Eq "^[[:space:]]*${_dep}[[:space:]]*[.=]" "${fx_dir}/crates/ferrousx-bin/Cargo.toml"; then
awk -v dep="${_dep}.workspace = true" '/^\[dependencies\]$/ { print; print dep; next } { print }' "${fx_dir}/crates/ferrousx-bin/Cargo.toml" > "${fx_dir}/crates/ferrousx-bin/Cargo.toml.tmp" && mv "${fx_dir}/crates/ferrousx-bin/Cargo.toml.tmp" "${fx_dir}/crates/ferrousx-bin/Cargo.toml"
fi
done
fi
if [ -f "${fx_dir}/Cargo.toml" ] && grep -q 'h3-quinn = "0\.0\.7"' "${fx_dir}/Cargo.toml" && grep -q 'quinn = { version = "0\.11\.6"' "${fx_dir}/Cargo.toml"; then
sed -i 's/quinn = { version = "0\.11\.6"/quinn = { version = "=0.11.6"/' "${fx_dir}/Cargo.toml"
git -C "${fx_dir}" ls-files --error-unmatch Cargo.lock >/dev/null 2>&1 || rm -f "${fx_dir}/Cargo.lock"
(cd "${fx_dir}" && cargo generate-lockfile && cargo update -p quinn-proto --precise 0.11.9) || true
fi
local srv_rs="${fx_dir}/crates/ferrousx-http/src/server.rs"
if [ -f "${srv_rs}" ]; then
sed -i 's/if is_panel_request(req, \&norm_path, cfg)/if false \&\& is_panel_request(req, \&norm_path, cfg)/g; s/if cfg\.enable_phpmyadmin_route && is_admin_or_panel_host(host_no_port, cfg)/if cfg.enable_phpmyadmin_route/; s|norm_path == "/phpadmin" || norm_path == "/phpmyadmin"|norm_path == "/phpadmin" \|\| norm_path == "/phpmyadmin" \|\| norm_path == "/pgadmin"|; s|norm_path.starts_with("/phpadmin/") || norm_path.starts_with("/phpmyadmin/")|norm_path.starts_with("/phpadmin/") \|\| norm_path.starts_with("/phpmyadmin/") \|\| norm_path.starts_with("/pgadmin/")|; s/mut stream: up_stream,/stream: mut up_stream,/g' "${srv_rs}"
if grep -q 'while let Some(chunk) = match decoder\.next_decoded()' "${srv_rs}"; then
awk '/while let Some\(chunk\) = match decoder\.next_decoded\(\) \{/ { print "let mut stream_err = false; loop { match decoder.next_decoded() { Ok(Some(chunk)) => { write_http1_stream_chunk(stream, &chunk, use_chunked, stats).await?; } Ok(None) => break, Err(_) => { stream_err = true; break; } } } if stream_err || decoder.is_done() { break; }"; skip = 8; next } skip > 0 { skip--; next } { print }' "${srv_rs}" > "${srv_rs}.tmp" && mv "${srv_rs}.tmp" "${srv_rs}"
fi
if grep -q 'let payload = serde_json::json!({' "${srv_rs}" && ! grep -q '/www/server/default/index.html' "${srv_rs}"; then
awk '/let payload = serde_json::json!\(\{/ { print "if req.method == \"GET\" || req.method == \"HEAD\" { for cand in [\"/www/server/default/index.html\", \"/www/wwwroot/index.html\"] { if let Ok(tpl) = tokio::fs::read_to_string(cand).await { let sh = host_no_port.replace(\x27&\x27, \"&amp;\").replace(\x27<\x27, \"&lt;\").replace(\x27>\x27, \"&gt;\").replace(\x27\"\x27, \"&quot;\"); let html = tpl.replace(\"{{HOST}}\", &sh); return DispatchResponse::Buffered(HttpResponsePayload { status: if norm_path == \"/\" || norm_path == \"/index.html\" { StatusCode::OK } else { StatusCode::NOT_FOUND }, headers: vec![(\"Content-Type\".to_string(), \"text/html; charset=utf-8\".to_string()), (\"Cache-Control\".to_string(), \"no-store\".to_string())], body: if req.method == \"HEAD\" { Bytes::new() } else { Bytes::from(html) } }); } } }" } { print }' "${srv_rs}" > "${srv_rs}.tmp" && mv "${srv_rs}.tmp" "${srv_rs}"
fi
fi
[ -f "${fx_dir}/crates/ferrousx-proxy/src/fastcgi.rs" ] && sed -i 's/let mut saw_end_request = false;/let saw_end_request: bool;/' "${fx_dir}/crates/ferrousx-proxy/src/fastcgi.rs"
local cli_rs="${fx_dir}/crates/ferrousx-bin/src/cli.rs"
[ -f "${cli_rs}" ] && sed -i "s|<Server-IP>|${ACTIVE_PANEL_DOMAIN:-${VPS_PUBLIC_IP}}|g" "${cli_rs}"
if [ -f "${fx_dir}/crates/ferrousx-bin/src/main.rs" ]; then
sed -i 's/if meta\.file_type()\.is_symlink() {/if false \&\& meta.file_type().is_symlink() {/; s/let certified = load_certified_key(cert_path, key_path)?;/let Ok(certified) = load_certified_key(cert_path, key_path) else { warn!(domain = %vhost.server_name, "Skipping unreadable TLS certificate for vhost"); continue; };/' "${fx_dir}/crates/ferrousx-bin/src/main.rs"
fi
for _cfg_rs in "${fx_dir}/crates/ferrousx-config/src/config.rs" "${fx_dir}/crates/ferrousx-core/src/config.rs"; do
[ -f "${_cfg_rs}" ] || continue
sed -i 's/Some(load_certified_key(&clean_cert, &clean_key)?)/if clean_cert.is_file() \&\& clean_key.is_file() { Some(load_certified_key(\&clean_cert, \&clean_key)?) } else { None }/' "${_cfg_rs}"
if grep -q 'validate_pem_cert_and_key_files(&clean_cert, &clean_key)?;' "${_cfg_rs}"; then
awk '
/validate_pem_cert_and_key_files\(&clean_cert, &clean_key\)\?;/ {
if ((getline next_line) > 0 && next_line ~ /\(Some\(clean_cert\), Some\(clean_key\)\)/) {
print "if clean_cert.is_file() && clean_key.is_file() { validate_pem_cert_and_key_files(&clean_cert, &clean_key)?; (Some(clean_cert), Some(clean_key)) } else { (None, None) }"; next
} else {
print "if clean_cert.is_file() && clean_key.is_file() { validate_pem_cert_and_key_files(&clean_cert, &clean_key)?; }"
if (next_line != "") print next_line; next
}
}
/ssl_enabled:[[:space:]]*raw\.ssl\.enabled,/ { print "ssl_enabled: raw.ssl.enabled && ssl_cert_path.is_some() && ssl_key_path.is_some(),"; next }
/redirect_https:[[:space:]]*raw\.ssl\.redirect_https,/ { print "redirect_https: raw.ssl.redirect_https && ssl_cert_path.is_some() && ssl_key_path.is_some(),"; next }
{ print }
' "${_cfg_rs}" > "${_cfg_rs}.tmp" && mv "${_cfg_rs}.tmp" "${_cfg_rs}"
fi
done
ensure_swap_for_build
configure_fast_cargo_env
if (cd "${fx_dir}" && cargo build --release --bin ferrousx-bin --bin ferrousx-ctl); then
if [ -x "${fx_dir}/target/release/ferrousx-bin" ]; then
atomic_install_binary "${fx_dir}/target/release/ferrousx-bin" "${BIN_FERROUSX}" "ferrousx-bin.prev" "ferrousx-bin"
fx_installed=true
[ -n "${fx_cache_tag}" ] && printf '%s\n' "${fx_cache_tag}" > "${FERROUSX_COMMIT_FILE}"
fi
if [ -x "${fx_dir}/target/release/ferrousx-ctl" ]; then
install -m 0755 "${fx_dir}/target/release/ferrousx-ctl" "${BIN_FERROUSX_CTL}.new" && mv -f "${BIN_FERROUSX_CTL}.new" "${BIN_FERROUSX_CTL}"
ln -sf "${BIN_FERROUSX_CTL}" /usr/local/bin/fx
fi
fi
fi
fi
if [ "$fx_installed" = false ] && [ "$PREBUILT_ONLY" = false ] && [ -d "${SRC_DIR}/crates/ferrousx" ] && command -v cargo >/dev/null 2>&1; then
ensure_swap_for_build
configure_fast_cargo_env
local fx_prof_args="--release" fx_target_sub="release"
if [ "$FULL_LTO" = false ] && grep -q '^\[profile\.vps\]' "${SRC_DIR}/Cargo.toml" 2>/dev/null; then
fx_prof_args="--profile vps"; fx_target_sub="vps"
fi
(cd "${SRC_DIR}" && cargo build ${fx_prof_args} -p ferrousx)
if [ -x "${SRC_DIR}/target/${fx_target_sub}/ferrousx-bin" ]; then
atomic_install_binary "${SRC_DIR}/target/${fx_target_sub}/ferrousx-bin" "${BIN_FERROUSX}" "ferrousx-bin.prev" "ferrousx-bin"
fx_installed=true
fi
if [ -x "${SRC_DIR}/target/${fx_target_sub}/ferrousx-ctl" ]; then
install -m 0755 "${SRC_DIR}/target/${fx_target_sub}/ferrousx-ctl" "${BIN_FERROUSX_CTL}.new" && mv -f "${BIN_FERROUSX_CTL}.new" "${BIN_FERROUSX_CTL}"
ln -sf "${BIN_FERROUSX_CTL}" /usr/local/bin/fx
fi
fi
if [ "$fx_installed" = false ]; then
local tmp_fx="${TMP_DIR}/ferrousx-bin-candidate"
download_verified_release_asset "ferrousx-bin-linux-${ARCH}" "${tmp_fx}"
atomic_install_binary "${tmp_fx}" "${BIN_FERROUSX}" "ferrousx-bin.prev" "ferrousx-bin"
fi
ensure_ferrousx_runtime_files
if [ -d /etc/ferrousx/sites ]; then
for site_toml in /etc/ferrousx/sites/*.toml; do
[ -f "$site_toml" ] || continue
local cert_val key_val
cert_val="$(grep -E '^[[:space:]]*cert_path[[:space:]]*=' "$site_toml" 2>/dev/null | head -n 1 | cut -d= -f2- | tr -d '[:space:]\"'\''' || true)"
key_val="$(grep -E '^[[:space:]]*key_path[[:space:]]*=' "$site_toml" 2>/dev/null | head -n 1 | cut -d= -f2- | tr -d '[:space:]\"'\''' || true)"
if { [ -n "$cert_val" ] && [ ! -f "$cert_val" ]; } || { [ -n "$key_val" ] && [ ! -f "$key_val" ]; }; then
if grep -Eq '^[[:space:]]*(enabled|redirect_https)[[:space:]]*=[[:space:]]*true' "$site_toml" 2>/dev/null; then
awk '/^\[ssl\]/ { in_ssl=1; print; next } /^\[/ { in_ssl=0 } in_ssl && /^[[:space:]]*enabled[[:space:]]*=[[:space:]]*true/ { print "enabled = false"; next } in_ssl && /^[[:space:]]*redirect_https[[:space:]]*=[[:space:]]*true/ { print "redirect_https = false"; next } { print }' "$site_toml" > "${site_toml}.tmp" && chmod 0644 "${site_toml}.tmp" && mv -f "${site_toml}.tmp" "$site_toml"
fi
else
if [ -n "$cert_val" ] && [ -L "$cert_val" ]; then
local real_cert
real_cert="$(readlink -f "$cert_val" 2>/dev/null || true)"
[ -n "$real_cert" ] && [ -f "$real_cert" ] && sed -i "s|^cert_path[[:space:]]*=.*|cert_path = \"${real_cert}\"|" "$site_toml"
fi
if [ -n "$key_val" ] && [ -L "$key_val" ]; then
local real_key
real_key="$(readlink -f "$key_val" 2>/dev/null || true)"
if [ -n "$real_key" ] && [ -f "$real_key" ]; then
chmod 0600 "$real_key" 2>/dev/null || true
sed -i "s|^key_path[[:space:]]*=.*|key_path = \"${real_key}\"|" "$site_toml"
fi
fi
fi
done
fi
if [ -x "${BIN_FERROUSX}" ] && ! "${BIN_FERROUSX}" --check-config --config /etc/ferrousx/ferrousx.toml; then
if [ -x "${ROLLBACK_DIR}/ferrousx-bin.prev" ]; then
install -m 0755 "${ROLLBACK_DIR}/ferrousx-bin.prev" "${BIN_FERROUSX}.new" && mv -f "${BIN_FERROUSX}.new" "${BIN_FERROUSX}"
fi
return 1
fi
if [ "$fx_skipped" = false ] && systemctl is-active --quiet ferrousx.service 2>/dev/null; then
command -v ufw >/dev/null 2>&1 && ufw status 2>/dev/null | grep -qi "Status: active" && ufw allow 443/udp comment 'HTTP3-QUIC' >/dev/null 2>&1 || true
if ! systemctl restart ferrousx.service || ! systemctl is-active --quiet ferrousx.service; then
if [ -x "${ROLLBACK_DIR}/ferrousx-bin.prev" ]; then
install -m 0755 "${ROLLBACK_DIR}/ferrousx-bin.prev" "${BIN_FERROUSX}.new" && mv -f "${BIN_FERROUSX}.new" "${BIN_FERROUSX}"
systemctl restart ferrousx.service || true
fi
return 1
fi
[ -x "${BIN_FERROUSX_CTL}" ] && "${BIN_FERROUSX_CTL}" sync >/dev/null 2>&1 || true
fi
}
install_zero_node_js_css_toolchain() {
local bun_arch="x64" tw_arch="x64" gh_bun="https://github.com/oven-sh/bun/releases/latest/download" gh_tw="https://github.com/tailwindlabs/tailwindcss/releases/latest/download"
if [ "${ARCH}" = "aarch64" ]; then bun_arch="aarch64"; tw_arch="arm64"; fi
if [ ! -x /usr/local/bin/bun ]; then
echo -e "${CYAN}[*] Installing Bun (Zero-Node.js)...${NC}"
! command -v unzip >/dev/null 2>&1 && command -v apt-get >/dev/null 2>&1 && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq unzip >/dev/null 2>&1 || true
mkdir -p "${TMP_DIR}/bun-dl"
local bun_asset="bun-linux-${bun_arch}.zip"
if curl -fsSL --retry 3 --connect-timeout 15 -o "${TMP_DIR}/bun-dl/${bun_asset}" "${gh_bun}/${bun_asset}"; then
curl -fsSL --connect-timeout 15 -o "${TMP_DIR}/bun-dl/SHASUMS256.txt" "${gh_bun}/SHASUMS256.txt" 2>/dev/null || rm -f "${TMP_DIR}/bun-dl/SHASUMS256.txt"
verify_sha256_entry "${TMP_DIR}/bun-dl/SHASUMS256.txt" "${bun_asset}" "${TMP_DIR}/bun-dl" "Bun"
if unzip -qo "${TMP_DIR}/bun-dl/${bun_asset}" -d "${TMP_DIR}/bun-dl" && [ -f "${TMP_DIR}/bun-dl/bun-linux-${bun_arch}/bun" ]; then
install -m 0755 "${TMP_DIR}/bun-dl/bun-linux-${bun_arch}/bun" /usr/local/bin/bun
else echo -e "${RED}[ERROR] Failed to unpack Bun.${NC}" >&2; exit 1; fi
fi
fi
[ -x /usr/local/bin/bun ] && ! command -v node >/dev/null 2>&1 && ln -sf /usr/local/bin/bun /usr/local/bin/node || true
if [ ! -x /usr/local/bin/tailwindcss ]; then
echo -e "${CYAN}[*] Installing Tailwind CSS v4 CLI...${NC}"
mkdir -p "${TMP_DIR}/tw-dl"
local tw_asset="tailwindcss-linux-${tw_arch}"
if curl -fsSL --retry 3 --connect-timeout 15 -o "${TMP_DIR}/tw-dl/${tw_asset}" "${gh_tw}/${tw_asset}"; then
curl -fsSL --connect-timeout 15 -o "${TMP_DIR}/tw-dl/sha256sums.txt" "${gh_tw}/sha256sums.txt" 2>/dev/null \
|| curl -fsSL --connect-timeout 15 -o "${TMP_DIR}/tw-dl/sha256sums.txt" "${gh_tw}/SHA256SUMS.txt" 2>/dev/null \
|| rm -f "${TMP_DIR}/tw-dl/sha256sums.txt"
verify_sha256_entry "${TMP_DIR}/tw-dl/sha256sums.txt" "${tw_asset}" "${TMP_DIR}/tw-dl" "Tailwind"
install -m 0755 "${TMP_DIR}/tw-dl/${tw_asset}" /usr/local/bin/tailwindcss
fi
fi
if { ! command -v wg >/dev/null 2>&1 || { ! command -v podman >/dev/null 2>&1 && ! command -v docker >/dev/null 2>&1; }; } && command -v apt-get >/dev/null 2>&1; then
DEBIAN_FRONTEND=noninteractive apt-get update -qq >/dev/null 2>&1 || true
DEBIAN_FRONTEND=noninteractive apt-get install -y -qq wireguard-tools podman >/dev/null 2>&1 || true
fi
[ ! -x /usr/local/bin/wasmtime ] && [ -x /root/.wasmtime/bin/wasmtime ] && install -m 0755 /root/.wasmtime/bin/wasmtime /usr/local/bin/wasmtime || true
if [ ! -x /usr/local/bin/wasmtime ] && command -v curl >/dev/null 2>&1 && command -v tar >/dev/null 2>&1; then
local wt_arch="x86_64"; [ "${ARCH}" = "aarch64" ] && wt_arch="aarch64"
local wt_url
wt_url="$(curl -fsSL --connect-timeout 10 -H "Accept: application/vnd.github+json" "https://api.github.com/repos/bytecodealliance/wasmtime/releases/latest" 2>/dev/null | grep -oE "https://[^\"]+wasmtime-[^\"]+-${wt_arch}-linux\.tar\.xz" | head -n 1 || true)"
if [ -n "$wt_url" ]; then
mkdir -p "${TMP_DIR}/wt-dl"
if curl -fsSL --retry 2 --connect-timeout 15 -o "${TMP_DIR}/wt-dl/wt.tar.xz" "$wt_url" && tar -xJf "${TMP_DIR}/wt-dl/wt.tar.xz" -C "${TMP_DIR}/wt-dl" 2>/dev/null; then
local wt_bin; wt_bin="$(find "${TMP_DIR}/wt-dl" -type f -name wasmtime | head -n 1 || true)"
[ -n "$wt_bin" ] && install -m 0755 "$wt_bin" /usr/local/bin/wasmtime || true
fi
fi
fi
}
ensure_native_rust_ui() {
local work_dir="$1" ui_changed="${2:-true}"
if [ ! -f "${work_dir}/crates/ferrous-panel/src/ui/mod.rs" ]; then
echo -e "${RED}[ERROR] Pure-Rust Native UI module (${work_dir}/crates/ferrous-panel/src/ui/mod.rs) is missing.${NC}" >&2
return 1
fi
[ "${ui_changed}" = true ] && touch "${work_dir}/crates/ferrous-panel/src/ui/mod.rs" || true
}
PANEL_BINARY_UPDATED=false
if [ "$UPDATE_PANEL" = true ]; then
if [ "$PREBUILT_ONLY" = false ] && [ ! -d "${SRC_DIR}/.git" ] && command -v git >/dev/null 2>&1 && command -v cargo >/dev/null 2>&1; then
echo -e "${BLUE}${BOLD}[*] Cloning FerrousPanel source (${REPO_SLUG}@${GIT_BRANCH}) into ${SRC_DIR}...${NC}"
mkdir -p "$(dirname "${SRC_DIR}")"
rm -rf "${SRC_DIR}"
git clone --depth 1 --branch "${GIT_BRANCH}" "https://github.com/${REPO_SLUG}.git" "${SRC_DIR}" || true
fi
if [ "$PREBUILT_ONLY" = false ] && [ -d "${SRC_DIR}/.git" ] && command -v cargo >/dev/null 2>&1; then
echo -e "${BLUE}${BOLD}[*] Fetching latest FerrousPanel (${REPO_SLUG}@${GIT_BRANCH})...${NC}"
OLD_COMMIT="$(git -C "${SRC_DIR}" rev-parse HEAD 2>/dev/null || echo '')"
INSTALLED_COMMIT="$(cat "${PANEL_COMMIT_FILE}" 2>/dev/null || echo '')"
git -C "${SRC_DIR}" fetch --depth 1 --force --quiet origin "${GIT_BRANCH}:refs/remotes/origin/${GIT_BRANCH}" || \
git -C "${SRC_DIR}" fetch --depth 1 --quiet origin "${GIT_BRANCH}"
TARGET_COMMIT="$(git -C "${SRC_DIR}" rev-parse "origin/${GIT_BRANCH}" 2>/dev/null || git -C "${SRC_DIR}" rev-parse FETCH_HEAD 2>/dev/null || echo '')"
if [ "$FORCE_REBUILD" = false ] && [ -x "${BIN_PANEL}" ] && [ -n "${TARGET_COMMIT}" ] \
&& [ "${OLD_COMMIT}" = "${TARGET_COMMIT}" ] && [ "${INSTALLED_COMMIT}" = "${TARGET_COMMIT}" ]; then
echo -e "${GREEN}[+] FerrousPanel is already up to date (${TARGET_COMMIT:0:8}) — skipped rebuild (use --force to recompile).${NC}"
else
UI_CHANGED=true
if [ -n "${OLD_COMMIT}" ] && [ -n "${TARGET_COMMIT}" ] && [ "${OLD_COMMIT}" != "${TARGET_COMMIT}" ]; then
git -C "${SRC_DIR}" diff --quiet "${OLD_COMMIT}" "${TARGET_COMMIT}" -- crates/ferrous-panel/src/ui/assets 2>/dev/null && UI_CHANGED=false || true
fi
git -C "${SRC_DIR}" reset --hard "origin/${GIT_BRANCH}" >/dev/null 2>&1 || \
git -C "${SRC_DIR}" reset --hard FETCH_HEAD >/dev/null 2>&1 || true
ensure_native_rust_ui "${SRC_DIR}" "${UI_CHANGED}"
ensure_swap_for_build
configure_fast_cargo_env
BUILD_PROFILE_ARGS="--release"; BUILD_TARGET_SUB="release"
if [ "$FULL_LTO" = false ] && grep -q '^\[profile\.vps\]' "${SRC_DIR}/Cargo.toml" 2>/dev/null; then
BUILD_PROFILE_ARGS="--profile vps"; BUILD_TARGET_SUB="vps"
fi
echo -e "${CYAN}[*] Compiling ferrous-panel (${TARGET_COMMIT:0:8}, profile=${BUILD_TARGET_SUB}, jobs=${CARGO_BUILD_JOBS:-auto})...${NC}"
(cd "${SRC_DIR}" && cargo build ${BUILD_PROFILE_ARGS} -p ferrous-panel)
if [ -f "${PANEL_COMMIT_FILE}" ]; then cp -f "${PANEL_COMMIT_FILE}" "${ROLLBACK_DIR}/ferrous-panel.prev.commit"
elif [ -n "${OLD_COMMIT}" ]; then printf '%s\n' "${OLD_COMMIT}" > "${ROLLBACK_DIR}/ferrous-panel.prev.commit"; fi
atomic_install_binary "${SRC_DIR}/target/${BUILD_TARGET_SUB}/ferrous-panel" "${BIN_PANEL}" "ferrous-panel.prev" "ferrous-panel"
[ -n "${TARGET_COMMIT}" ] && printf '%s\n' "${TARGET_COMMIT}" > "${PANEL_COMMIT_FILE}"
PANEL_BINARY_UPDATED=true
fi
elif [ "$SOURCE_ONLY" = true ]; then
echo -e "${RED}[ERROR] --source requested, but ${SRC_DIR}/.git or cargo is unavailable.${NC}" >&2; exit 1
else
echo -e "${CYAN}[*] Downloading verified prebuilt FerrousPanel release (${RELEASE_TAG}, ${ARCH})...${NC}"
TMP_PANEL_BIN="${TMP_DIR}/ferrous-panel-candidate"
download_verified_release_asset "ferrous-panel-linux-${ARCH}" "${TMP_PANEL_BIN}"
[ -f "${PANEL_COMMIT_FILE}" ] && cp -f "${PANEL_COMMIT_FILE}" "${ROLLBACK_DIR}/ferrous-panel.prev.commit"
atomic_install_binary "${TMP_PANEL_BIN}" "${BIN_PANEL}" "ferrous-panel.prev" "ferrous-panel"
printf 'release-%s\n' "${RELEASE_TAG}" > "${PANEL_COMMIT_FILE}"
PANEL_BINARY_UPDATED=true
fi
if [ -f /etc/ferrous-panel/panel.env ]; then
RAW_PANEL_PORT="$(grep -E '^[[:space:]]*(export[[:space:]]+)?FERROUS_PORT=' /etc/ferrous-panel/panel.env 2>/dev/null | tail -n 1 | cut -d= -f2 | tr -d '[:space:]\"'\''' || true)"
if [ -z "${RAW_PANEL_PORT}" ] || ! [[ "${RAW_PANEL_PORT}" =~ ^[0-9]+$ ]] || [ "${RAW_PANEL_PORT}" -lt 1 ] || [ "${RAW_PANEL_PORT}" -gt 65535 ] || [ "${RAW_PANEL_PORT}" -eq 80 ] || [ "${RAW_PANEL_PORT}" -eq 443 ]; then
RAW_PANEL_PORT="8443"
fi
awk -v port="${RAW_PANEL_PORT}" '/^[[:space:]]*(export[[:space:]]+)?FERROUS_PORT=/ { if (!seen) { print "FERROUS_PORT=" port; seen=1 } next } { print } END { if (!seen) print "FERROUS_PORT=" port }' \
/etc/ferrous-panel/panel.env > /etc/ferrous-panel/panel.env.tmp && chmod 0600 /etc/ferrous-panel/panel.env.tmp && mv /etc/ferrous-panel/panel.env.tmp /etc/ferrous-panel/panel.env
grep -Eq '^[[:space:]]*(export[[:space:]]+)?FERROUS_MODE=' /etc/ferrous-panel/panel.env 2>/dev/null || echo "FERROUS_MODE=production" >> /etc/ferrous-panel/panel.env
if [ -n "${EXPLICIT_BIND}" ]; then
if grep -Eq '^[[:space:]]*(export[[:space:]]+)?FERROUS_BIND=' /etc/ferrous-panel/panel.env 2>/dev/null; then
sed -i -E "s|^[[:space:]]*(export[[:space:]]+)?FERROUS_BIND=.*|FERROUS_BIND=${EXPLICIT_BIND}|" /etc/ferrous-panel/panel.env
else
echo "FERROUS_BIND=${EXPLICIT_BIND}" >> /etc/ferrous-panel/panel.env
fi
PANEL_CONFIG_CHANGED=true
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
sed -i -E '/^[[:space:]]*Environment=.*FERROUS_PORT=/d; /^[[:space:]]*ExecStart=.*--port/d' "$dropin"
grep -qE '^[[:space:]]*[^#[:space:]\[]' "$dropin" 2>/dev/null || rm -f "$dropin"
DROPIN_CLEANED=true
fi
done
[ "$DROPIN_CLEANED" = true ] && systemctl daemon-reload 2>/dev/null || true
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
ACTIVE_PANEL_PORT="$(read_panel_env_key FERROUS_PORT "8443")"
ACTIVE_PANEL_BIND="$(read_panel_env_key FERROUS_BIND "127.0.0.1")"
ACTIVE_PANEL_ENTRANCE="$(read_panel_env_key FERROUS_SECURITY_ENTRANCE "/panel")"
ACTIVE_PANEL_DOMAIN="$(read_panel_env_key FERROUS_CUSTOM_DOMAIN "")"
if [ -n "${EXPLICIT_DOMAIN}" ]; then
ACTIVE_PANEL_DOMAIN="${EXPLICIT_DOMAIN}"
if grep -qE '^[[:space:]]*FERROUS_CUSTOM_DOMAIN=' /etc/ferrous-panel/panel.env 2>/dev/null; then
sed -i "s|^[[:space:]]*FERROUS_CUSTOM_DOMAIN=.*|FERROUS_CUSTOM_DOMAIN=${ACTIVE_PANEL_DOMAIN}|" /etc/ferrous-panel/panel.env
else
echo "FERROUS_CUSTOM_DOMAIN=${ACTIVE_PANEL_DOMAIN}" >> /etc/ferrous-panel/panel.env
fi
PANEL_CONFIG_CHANGED=true
fi
if [ -z "${ACTIVE_PANEL_DOMAIN}" ]; then
if [ -f "${DATA_DIR}/panel.db" ] && command -v strings >/dev/null 2>&1; then
RECOVERED_DOM="$(strings "${DATA_DIR}/panel.db" 2>/dev/null | grep -oE '"customDomain":"[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}"' | tail -n 1 | cut -d'"' -f4 || true)"
[ -n "${RECOVERED_DOM}" ] && ACTIVE_PANEL_DOMAIN="${RECOVERED_DOM}"
fi
if [ -z "${ACTIVE_PANEL_DOMAIN}" ] && [ -f "/etc/ferrous-panel/ssl/fullchain.pem" ] && command -v openssl >/dev/null 2>&1; then
CERT_CN="$(openssl x509 -in /etc/ferrous-panel/ssl/fullchain.pem -noout -subject 2>/dev/null | sed -n 's/.*CN[[:space:]]*=[[:space:]]*\([a-zA-Z0-9.-]*\).*/\1/p' | head -n 1 || true)"
if [[ "${CERT_CN}" == *.* ]] && [[ "${CERT_CN}" != *.local ]] && [[ ! "${CERT_CN}" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
ACTIVE_PANEL_DOMAIN="${CERT_CN}"
fi
fi
if [ -z "${ACTIVE_PANEL_DOMAIN}" ] && [ -d "/etc/letsencrypt/live" ]; then
for _le_d in /etc/letsencrypt/live/*; do
[ -d "$_le_d" ] || continue
_cand="$(basename "$_le_d")"; _cand="${_cand%-000*}"
if [[ "$_cand" == *.* ]] && [[ "$_cand" != *.local ]] && [ ! -f "/etc/ferrousx/sites/${_cand}.toml" ] && [ ! -f "/etc/nginx/sites-enabled/${_cand}.conf" ]; then
ACTIVE_PANEL_DOMAIN="$_cand"; break
fi
done
fi
if [ -n "${ACTIVE_PANEL_DOMAIN}" ] && [ -f /etc/ferrous-panel/panel.env ]; then
if grep -qE '^[[:space:]]*FERROUS_CUSTOM_DOMAIN=' /etc/ferrous-panel/panel.env 2>/dev/null; then
sed -i "s|^[[:space:]]*FERROUS_CUSTOM_DOMAIN=.*|FERROUS_CUSTOM_DOMAIN=${ACTIVE_PANEL_DOMAIN}|" /etc/ferrous-panel/panel.env
else
echo "FERROUS_CUSTOM_DOMAIN=${ACTIVE_PANEL_DOMAIN}" >> /etc/ferrous-panel/panel.env
fi
PANEL_CONFIG_CHANGED=true
echo -e "${GREEN}[+] Restored saved Panel Custom Domain: ${ACTIVE_PANEL_DOMAIN}${NC}"
fi
fi
ACTIVE_SECURE_COOKIE="$(read_panel_env_key FERROUS_SECURE_COOKIE "false")"
if [ -n "${ACTIVE_PANEL_DOMAIN}" ] && [ -f "/etc/letsencrypt/live/${ACTIVE_PANEL_DOMAIN}/fullchain.pem" ] && [ -f "/etc/letsencrypt/live/${ACTIVE_PANEL_DOMAIN}/privkey.pem" ]; then
mkdir -p /etc/ferrous-panel/ssl
cp -fL "/etc/letsencrypt/live/${ACTIVE_PANEL_DOMAIN}/fullchain.pem" /etc/ferrous-panel/ssl/fullchain.pem 2>/dev/null || true
cp -fL "/etc/letsencrypt/live/${ACTIVE_PANEL_DOMAIN}/privkey.pem" /etc/ferrous-panel/ssl/privkey.pem 2>/dev/null || true
chmod 0644 /etc/ferrous-panel/ssl/fullchain.pem 2>/dev/null || true
chmod 0600 /etc/ferrous-panel/ssl/privkey.pem 2>/dev/null || true
if ! grep -q '^FERROUS_SSL_CERT=' /etc/ferrous-panel/panel.env 2>/dev/null; then
echo "FERROUS_SSL_CERT=/etc/ferrous-panel/ssl/fullchain.pem" >> /etc/ferrous-panel/panel.env
echo "FERROUS_SSL_KEY=/etc/ferrous-panel/ssl/privkey.pem" >> /etc/ferrous-panel/panel.env
PANEL_CONFIG_CHANGED=true
fi
if [ "${ACTIVE_SECURE_COOKIE}" != "true" ]; then
sed -i 's|^FERROUS_SECURE_COOKIE=.*|FERROUS_SECURE_COOKIE=true|' /etc/ferrous-panel/panel.env
ACTIVE_SECURE_COOKIE="true"; PANEL_CONFIG_CHANGED=true
fi
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
if [ "${PROBE_HOST}" = "0.0.0.0" ] || [ "${PROBE_HOST}" = "::" ]; then PROBE_HOST="127.0.0.1"; fi
if [ "$PANEL_BINARY_UPDATED" = true ] || [ "$PANEL_CONFIG_CHANGED" = true ] || ! systemctl is-active --quiet ferrous-panel.service 2>/dev/null; then
systemctl restart ferrous-panel.service 2>/dev/null || true
if probe_panel_health "${PROBE_HOST}" "${ACTIVE_PANEL_PORT}" 25; then
echo -e "${GREEN}[+] ferrous-panel.service is live on ${ACTIVE_PANEL_BIND}:${ACTIVE_PANEL_PORT}.${NC}"
else
echo -e "${RED}[ERROR] ferrous-panel.service failed /api/health on ${PROBE_HOST}:${ACTIVE_PANEL_PORT} after update!${NC}" >&2
journalctl -u ferrous-panel.service -n 20 --no-pager >&2 || true
if [ "$PANEL_BINARY_UPDATED" = true ] && [ -x "${ROLLBACK_DIR}/ferrous-panel.prev" ]; then
perform_panel_rollback
fi
exit 1
fi
else
echo -e "${GREEN}[+] ferrous-panel.service already active on ${ACTIVE_PANEL_BIND}:${ACTIVE_PANEL_PORT}.${NC}"
fi
fi
update_nginx_engine() {
echo -e "${BLUE}${BOLD}[*] Syncing & verifying Nginx web server on VPS...${NC}"
if ! command -v nginx >/dev/null 2>&1; then
if [ "$FORCE_NGINX" = true ] && command -v apt-get >/dev/null 2>&1; then
export DEBIAN_FRONTEND=noninteractive
apt-get update -y -q && apt-get install -y -q nginx
else
return 0
fi
fi
mkdir -p /etc/nginx/sites-available /etc/nginx/sites-enabled /etc/nginx/conf.d /etc/nginx/snippets /www/acme/.well-known/acme-challenge
chown -R www-data:www-data /www/acme 2>/dev/null || true
if [ ! -f /etc/nginx/snippets/fastcgi-php.conf ]; then
cat > /etc/nginx/snippets/fastcgi-php.conf <<'EOF_NGX_PHP'
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
echo "# Managed by FerrousPanel"
grep -Eq '^[[:space:]]*client_max_body_size[[:space:]]' /etc/nginx/nginx.conf 2>/dev/null || echo "client_max_body_size 128m;"
grep -Eq '^[[:space:]]*server_names_hash_bucket_size[[:space:]]' /etc/nginx/nginx.conf 2>/dev/null || echo "server_names_hash_bucket_size 128;"
} > /etc/nginx/conf.d/ferrous-defaults.conf
nginx -t >/dev/null 2>&1 || rm -f /etc/nginx/conf.d/ferrous-defaults.conf
fi
local ngx_default_target
ngx_default_target="$(readlink -f /etc/nginx/sites-enabled/default 2>/dev/null || true)"
if [ "$ngx_default_target" = "/etc/nginx/sites-available/default" ] || \
{ [ "$ngx_default_target" = "/etc/nginx/sites-available/ferrous-default.conf" ] && \
head -n 1 /etc/nginx/sites-available/ferrous-default.conf 2>/dev/null | grep -q 'Managed by FerrousPanel' && \
{ ! grep -q '/pgadmin/' /etc/nginx/sites-available/ferrous-default.conf 2>/dev/null || \
! grep -q '/www/server/default' /etc/nginx/sites-available/ferrous-default.conf 2>/dev/null; }; }; then
cat > /etc/nginx/sites-available/ferrous-default.conf <<'EOF_NGX_DEF'
# Managed by FerrousPanel — Default catch-all vhost (/phpadmin/ /phpmyadmin/ /pgadmin/)
server {
    listen 80 default_server;
    listen [::]:80 default_server;
    server_name _;
    root /www/server/default;
    index index.html;
    server_tokens off;
    proxy_set_header Host $http_host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
    location ^~ /.well-known/acme-challenge/ { root /www/acme; default_type "text/plain"; try_files $uri =404; }
    location ~ ^/(phpadmin|phpmyadmin|pgadmin)$ { return 301 $scheme://$http_host$uri/; }
    location ~ ^/(phpadmin|phpmyadmin|pgadmin)/ { proxy_pass http://127.0.0.1:888; }
    location / { try_files $uri $uri/ /index.html; }
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
fi
fi
}
[ "$SYNC_JS_TOOLCHAIN" = true ] && install_zero_node_js_css_toolchain
if [ "$FORCE_FERROUSX" = true ] || [ -x "${BIN_FERROUSX}" ]; then update_ferrousx_from_github; fi
if [ "$FORCE_NGINX" = true ] || systemctl is-active --quiet nginx.service 2>/dev/null; then
update_nginx_engine
else
setup_phpmyadmin_proxy
fi
ACTIVE_PANEL_PORT="$(read_panel_env_key FERROUS_PORT "8443")"
if [ -z "$ACTIVE_PANEL_PORT" ] || [ "$ACTIVE_PANEL_PORT" = "80" ] || [ "$ACTIVE_PANEL_PORT" = "443" ]; then ACTIVE_PANEL_PORT="8443"; fi
ACTIVE_PANEL_ENTRANCE="$(read_panel_env_key FERROUS_SECURITY_ENTRANCE "/panel")"
ACTIVE_PANEL_DOMAIN="$(read_panel_env_key FERROUS_CUSTOM_DOMAIN "")"
for _vh in /etc/ferrousx/sites/*.toml /etc/nginx/sites-enabled/*.conf /etc/nginx/sites-available/*.conf; do
[ -f "$_vh" ] || continue
if grep -qE "127\.0\.0\.1:${ACTIVE_PANEL_PORT}|127\.0\.0\.1:8443" "$_vh" 2>/dev/null; then
rm -f "$_vh"
fi
done
is_self_signed_pem() {
command -v openssl >/dev/null 2>&1 || return 1
local i s
i="$(openssl x509 -in "$1" -noout -issuer 2>/dev/null | sed 's/^issuer=//' | tr -d '[:space:]' || true)"
s="$(openssl x509 -in "$1" -noout -subject 2>/dev/null | sed 's/^subject=//' | tr -d '[:space:]' || true)"
[ -n "$i" ] && [ "$i" = "$s" ]
}
auto_upgrade_self_signed_sites_to_letsencrypt() {
mkdir -p /www/acme/.well-known/acme-challenge
chmod 0755 /www /www/acme /www/acme/.well-known /www/acme/.well-known/acme-challenge 2>/dev/null || true
local site_domains=()
if [ -n "${ACTIVE_PANEL_DOMAIN:-}" ] && [[ "${ACTIVE_PANEL_DOMAIN}" == *.* ]] && [[ "${ACTIVE_PANEL_DOMAIN}" != *.local ]] && [ "${ACTIVE_PANEL_DOMAIN}" != "localhost" ]; then
site_domains+=("${ACTIVE_PANEL_DOMAIN}")
fi
for f in /etc/ferrousx/sites/*.toml /etc/nginx/sites-enabled/*.conf; do
[ -e "$f" ] || continue
local dom="${f##*/}"; dom="${dom%.toml}"; dom="${dom%.conf}"
if [[ "$dom" == *.* ]] && [[ "$dom" != *.local ]] && [ "$dom" != "localhost" ] && [ "$dom" != "ferrous-default" ]; then
local seen=false; for d in "${site_domains[@]:-}"; do [ "$d" = "$dom" ] && seen=true; done
[ "$seen" = false ] && site_domains+=("$dom")
fi
done
[ "${#site_domains[@]}" -gt 0 ] || return 0
local upgraded_any=false panel_ssl_upgraded=false
for dom in "${site_domains[@]}"; do
local cert_p="/etc/letsencrypt/live/${dom}/fullchain.pem" key_p="/etc/letsencrypt/live/${dom}/privkey.pem" need_le=false
if [ ! -f "$cert_p" ] || [ ! -f "$key_p" ] || is_self_signed_pem "$cert_p"; then need_le=true; fi
if [ "$dom" = "${ACTIVE_PANEL_DOMAIN:-}" ] && [ -f "/etc/ferrous-panel/ssl/fullchain.pem" ] && is_self_signed_pem "/etc/ferrous-panel/ssl/fullchain.pem"; then need_le=true; fi
[ "$need_le" = true ] || continue
if ! command -v certbot >/dev/null 2>&1 && command -v apt-get >/dev/null 2>&1; then
DEBIAN_FRONTEND=noninteractive apt-get update -qq >/dev/null 2>&1 || true
DEBIAN_FRONTEND=noninteractive apt-get install -y -qq certbot >/dev/null 2>&1 || true
fi
command -v certbot >/dev/null 2>&1 || return 0
local bak_dir="${TMP_DIR}/ssl-bak-${dom}" cb_ok=false
rm -rf "$bak_dir"
if [ -f "$cert_p" ] && [ -f "$key_p" ] && { [ ! -L "$cert_p" ] || is_self_signed_pem "$cert_p"; }; then
mkdir -p "$bak_dir" && cp -fL "$cert_p" "$bak_dir/fullchain.pem" 2>/dev/null && cp -fL "$key_p" "$bak_dir/privkey.pem" 2>/dev/null || true
rm -rf "/etc/letsencrypt/live/${dom}" "/etc/letsencrypt/archive/${dom}" "/etc/letsencrypt/renewal/${dom}.conf" 2>/dev/null || true
fi
local cb_flags=(certonly --non-interactive --agree-tos --register-unsafely-without-email -d "${dom}" --cert-name "${dom}" --expand --force-renewal)
if certbot "${cb_flags[@]}" --webroot -w /www/acme >/dev/null 2>&1 || certbot "${cb_flags[@]}" --standalone >/dev/null 2>&1; then
if [ ! -f "$cert_p" ]; then
for alt in /etc/letsencrypt/live/"${dom}"-*; do
if [ -d "$alt" ] && [ -f "$alt/fullchain.pem" ] && [ -f "$alt/privkey.pem" ]; then
mkdir -p "/etc/letsencrypt/live/${dom}"
ln -sf "$(readlink -f "$alt/fullchain.pem")" "$cert_p"; ln -sf "$(readlink -f "$alt/privkey.pem")" "$key_p"; break
fi
done
fi
if [ -f "$cert_p" ] && [ -f "$key_p" ]; then
cb_ok=true
local real_c real_k
real_c="$(readlink -f "$cert_p" 2>/dev/null || echo "$cert_p")"; real_k="$(readlink -f "$key_p" 2>/dev/null || echo "$key_p")"
chmod 0600 "$real_k" 2>/dev/null || true
if [ -f "/etc/ferrousx/sites/${dom}.toml" ]; then
awk -v rc="$real_c" -v rk="$real_k" '/^\[ssl\]/{s=1;print;next} /^\[/{s=0} s&&/^[[:space:]]*enabled[[:space:]]*=/{print "enabled = true";next} s&&/^[[:space:]]*cert_path[[:space:]]*=/{print "cert_path = \"" rc "\"";next} s&&/^[[:space:]]*key_path[[:space:]]*=/{print "key_path = \"" rk "\"";next} {print}' "/etc/ferrousx/sites/${dom}.toml" > "/etc/ferrousx/sites/${dom}.toml.tmp" && chmod 0644 "/etc/ferrousx/sites/${dom}.toml.tmp" && mv -f "/etc/ferrousx/sites/${dom}.toml.tmp" "/etc/ferrousx/sites/${dom}.toml"
fi
if [ "$dom" = "${ACTIVE_PANEL_DOMAIN:-}" ]; then
mkdir -p /etc/ferrous-panel/ssl && cp -f "$real_c" /etc/ferrous-panel/ssl/fullchain.pem && cp -f "$real_k" /etc/ferrous-panel/ssl/privkey.pem
chmod 0644 /etc/ferrous-panel/ssl/fullchain.pem && chmod 0600 /etc/ferrous-panel/ssl/privkey.pem
grep -q '^FERROUS_SSL_CERT=' /etc/ferrous-panel/panel.env 2>/dev/null && sed -i 's|^FERROUS_SSL_CERT=.*|FERROUS_SSL_CERT=/etc/ferrous-panel/ssl/fullchain.pem|' /etc/ferrous-panel/panel.env || echo "FERROUS_SSL_CERT=/etc/ferrous-panel/ssl/fullchain.pem" >> /etc/ferrous-panel/panel.env
grep -q '^FERROUS_SSL_KEY=' /etc/ferrous-panel/panel.env 2>/dev/null && sed -i 's|^FERROUS_SSL_KEY=.*|FERROUS_SSL_KEY=/etc/ferrous-panel/ssl/privkey.pem|' /etc/ferrous-panel/panel.env || echo "FERROUS_SSL_KEY=/etc/ferrous-panel/ssl/privkey.pem" >> /etc/ferrous-panel/panel.env
sed -i 's|^FERROUS_SECURE_COOKIE=.*|FERROUS_SECURE_COOKIE=true|' /etc/ferrous-panel/panel.env
panel_ssl_upgraded=true
fi
upgraded_any=true
echo -e "${GREEN}[+] Issued Let's Encrypt SSL for https://${dom}${NC}"
fi
fi
if [ "$cb_ok" = false ] && [ -d "$bak_dir" ]; then
mkdir -p "/etc/letsencrypt/live/${dom}" && cp -f "$bak_dir/fullchain.pem" "$cert_p" && cp -f "$bak_dir/privkey.pem" "$key_p" && chmod 0600 "$key_p" 2>/dev/null || true
fi
done
if [ "$upgraded_any" = true ]; then
systemctl is-active --quiet ferrousx.service 2>/dev/null && systemctl restart ferrousx.service 2>/dev/null || true
systemctl is-active --quiet nginx.service 2>/dev/null && nginx -t >/dev/null 2>&1 && systemctl reload nginx.service 2>/dev/null || true
fi
[ "$panel_ssl_upgraded" = true ] && systemctl restart ferrous-panel.service 2>/dev/null || true
}
auto_upgrade_self_signed_sites_to_letsencrypt
PANEL_SCHEME="http"
[ -f "/etc/ferrous-panel/ssl/fullchain.pem" ] && [ -f "/etc/ferrous-panel/ssl/privkey.pem" ] && PANEL_SCHEME="https"
UPDATE_ELAPSED="$(( $(date +%s) - UPDATE_START_TS ))"
echo -e "${GREEN}${BOLD}Update completed in ${UPDATE_ELAPSED}s.${NC} Panel URL: ${BOLD}${PANEL_SCHEME}://${ACTIVE_PANEL_DOMAIN:-${VPS_PUBLIC_IP}}:${ACTIVE_PANEL_PORT}${ACTIVE_PANEL_ENTRANCE}${NC}"
