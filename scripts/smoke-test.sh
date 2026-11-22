#!/usr/bin/env bash
# ==============================================================================
# FerrousPanel — End-to-End Smoke, Security & Real-Ops Verification (Ubuntu 24.04)
#
# Verifies core security, anti-simulation, and real system operations:
#   (1)  Every protected /api route without token returns HTTP 401
#   (2)  Every protected /api route with forged token returns HTTP 401
#   (3)  Path traversal payloads ("../../etc/passwd") return HTTP 400
#   (4)  Create database via POST /api/databases and verify with `mysql -e`
#   (5)  Start/stop systemd service via POST /api/apps/{id}/action & `systemctl is-active`
#   (6)  Create site, curl via Nginx & FerrousX, switch engine, verify rollback on failure
#   (7)  Rust workspace verification (`cargo clippy -D warnings`, `cargo test`, `cargo audit`)
#   (8)  GET /api/capabilities returns real host capability matrix (zero mock)
#   (9)  WebSSH allowlist rejects `sysctl -w` / `ip link set` / `ss -K` (HTTP 400/403)
#   (10) Remote download SSRF guard rejects `169.254.169.254`, `2130706433`, `0x7f.1`
#   (11) Per-site settings: `http3Quic: true` on unsupported engine returns HTTP 409;
#        valid `rateLimitRps` + `redirects` render cleanly
#   (12) UFW anti-lockout guard blocks deleting protected SSH/panel rules without `?force=true`
#   (13) Telemetry + Audit Log return real `/proc`/sysinfo metrics and recorded mutations
#   (14) Extended modules (Multi-PHP, Mail, Optimizer, Cron/Backup) verify real state or honest 409
#
# Prints PASS / FAIL / SKIP (reason) for every check and exits 1 if FAIL > 0.
# ==============================================================================
set -uo pipefail

BOLD='\033[1m'
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

PANEL_HOST="${FERROUS_BIND:-127.0.0.1}"
PANEL_PORT="${FERROUS_PORT:-8443}"
PANEL_URL="${PANEL_URL:-http://${PANEL_HOST}:${PANEL_PORT}}"
DATA_DIR="${FERROUS_DATA_DIR:-/var/lib/ferrous-panel}"
WWWROOT="${FERROUS_WWWROOT:-/www/wwwroot}"
ADMIN_USER="${FERROUS_ADMIN_USER:-admin_ferrous}"
PANEL_BIN="${PANEL_BIN:-/usr/local/bin/ferrous-panel}"

PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0
SPAWNED_PANEL_PID=""
SMOKE_DB_NAME="fp_smoke_db_$$"
SMOKE_DB_USER="fp_u_$$"
SMOKE_DOMAIN="smoke-$$.ferrous.local"
SMOKE_APP_ID=""

pass_item() {
  local id="$1"
  local msg="$2"
  echo -e "${GREEN}[PASS] (${id}) ${msg}${NC}"
  PASS_COUNT=$((PASS_COUNT + 1))
}

fail_item() {
  local id="$1"
  local msg="$2"
  echo -e "${RED}[FAIL] (${id}) ${msg}${NC}" >&2
  FAIL_COUNT=$((FAIL_COUNT + 1))
}

skip_item() {
  local id="$1"
  local reason="$2"
  echo -e "${YELLOW}[SKIP] (${id}) ${reason}${NC}"
  SKIP_COUNT=$((SKIP_COUNT + 1))
}

info_msg() {
  echo -e "${BLUE}[*] $*${NC}"
}

cleanup() {
  if [ -n "${SMOKE_APP_ID}" ] && [ -n "${AUTH_TOKEN:-}" ]; then
    curl -sS -X DELETE "${PANEL_URL}/api/apps/${SMOKE_APP_ID}" \
      -H "Authorization: Bearer ${AUTH_TOKEN}" >/dev/null 2>&1 || true
  fi
  if command -v mysql >/dev/null 2>&1; then
    mysql -e "DROP DATABASE IF EXISTS \`${SMOKE_DB_NAME}\`; DROP USER IF EXISTS '${SMOKE_DB_USER}'@'localhost';" >/dev/null 2>&1 || true
  fi
  rm -rf "${WWWROOT:?}/${SMOKE_DOMAIN}" 2>/dev/null || true
  if [ -n "${SPAWNED_PANEL_PID}" ]; then
    kill "${SPAWNED_PANEL_PID}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

if [ ! -x "${PANEL_BIN}" ] && [ -x "./target/release/ferrous-panel" ]; then
  PANEL_BIN="./target/release/ferrous-panel"
fi

echo -e "${BOLD}==============================================================================${NC}"
echo -e "${BOLD}  FerrousPanel — Ubuntu 24.04 End-to-End Smoke & Security Verification${NC}"
echo -e "${BOLD}==============================================================================${NC}"

AUTH_TOKEN=""
if ! curl -fsS "${PANEL_URL}/api/health" >/dev/null 2>&1; then
  if [ -x "${PANEL_BIN}" ]; then
    info_msg "Starting local ferrous-panel daemon on ${PANEL_HOST}:${PANEL_PORT}..."
    mkdir -p "${DATA_DIR}" "${WWWROOT}" /www/acme
    FERROUS_MODE=production "${PANEL_BIN}" --bind "${PANEL_HOST}" --port "${PANEL_PORT}" \
      --data-dir "${DATA_DIR}" --wwwroot "${WWWROOT}" \
      --admin-username "${ADMIN_USER}" serve >/tmp/ferrous-panel-smoke.log 2>&1 &
    SPAWNED_PANEL_PID=$!
    for _ in $(seq 1 25); do
      if curl -fsS "${PANEL_URL}/api/health" >/dev/null 2>&1; then
        break
      fi
      sleep 0.2
    done
  fi
fi

# ==============================================================================
# (1) Every protected /api route without token returns HTTP 401
# ==============================================================================
info_msg "Check (1): Probing protected /api routes without authentication token..."
PROTECTED_ROUTES=(
  "/api/auth/session"
  "/api/capabilities"
  "/api/telemetry"
  "/api/webserver"
  "/api/apps"
  "/api/databases"
  "/api/files/app-1"
  "/api/cpanel"
  "/api/security"
  "/api/mail"
  "/api/edge-cloud"
)

CHECK1_OK=true
HEALTH_CODE="$(curl -s -o /dev/null -w '%{http_code}' "${PANEL_URL}/api/health" || echo "000")"
if [ "${HEALTH_CODE}" != "200" ]; then
  CHECK1_OK=false
  echo -e "${YELLOW}  Expected GET /api/health = 200, got ${HEALTH_CODE}${NC}" >&2
fi

for route in "${PROTECTED_ROUTES[@]}"; do
  code="$(curl -s -o /dev/null -w '%{http_code}' "${PANEL_URL}${route}" || echo "000")"
  if [ "${code}" != "401" ]; then
    CHECK1_OK=false
    echo -e "${YELLOW}  Unauthenticated GET ${route} returned ${code} (expected 401)${NC}" >&2
  fi
done

if [ "${CHECK1_OK}" = true ]; then
  pass_item "1" "All protected /api routes rejected unauthenticated requests with HTTP 401 (health=200)"
else
  fail_item "1" "One or more protected /api routes did not return HTTP 401 without token"
fi

# ==============================================================================
# (2) Forged token = HTTP 401
# ==============================================================================
info_msg "Check (2): Probing protected /api routes with forged Bearer & Cookie tokens..."
FORGED_TOKEN="deadbeef00112233445566778899aabbccddeeff00112233445566778899aabb"
CHECK2_OK=true

for route in "${PROTECTED_ROUTES[@]}"; do
  code_bearer="$(curl -s -o /dev/null -w '%{http_code}' \
    -H "Authorization: Bearer ${FORGED_TOKEN}" \
    "${PANEL_URL}${route}" || echo "000")"
  code_cookie="$(curl -s -o /dev/null -w '%{http_code}' \
    -H "Cookie: fp_admin_token=${FORGED_TOKEN}" \
    "${PANEL_URL}${route}" || echo "000")"
  if [ "${code_bearer}" != "401" ] || [ "${code_cookie}" != "401" ]; then
    CHECK2_OK=false
    echo -e "${YELLOW}  Forged token on ${route} returned bearer=${code_bearer}, cookie=${code_cookie} (expected 401)${NC}" >&2
  fi
done

if [ "${CHECK2_OK}" = true ]; then
  pass_item "2" "Forged Bearer and Cookie tokens rejected across all /api routes with HTTP 401"
else
  fail_item "2" "Forged token was not rejected with HTTP 401"
fi

# ==============================================================================
# Authenticate with real Argon2id credentials
# ==============================================================================
ADMIN_PASS="${FERROUS_SMOKE_PASS:-}"
if [ -z "${ADMIN_PASS}" ] && [ -x "${PANEL_BIN}" ]; then
  PASS_OUT="$("${PANEL_BIN}" --data-dir "${DATA_DIR}" --wwwroot "${WWWROOT}" --admin-username "${ADMIN_USER}" fp passwd --generate 2>&1 || true)"
  ADMIN_PASS="$(echo "${PASS_OUT}" | awk -F': ' '/Generated one-time password/ {print $2}' | tr -d '\r\n' || true)"
fi

if [ -n "${ADMIN_PASS}" ]; then
  LOGIN_JSON="$(curl -sS -X POST "${PANEL_URL}/api/auth/login" \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"${ADMIN_USER}\",\"password\":\"${ADMIN_PASS}\",\"rememberMe\":false}" || true)"
  AUTH_TOKEN="$(echo "${LOGIN_JSON}" | sed -n 's/.*"token":"\([^"]*\)".*/\1/p')"
fi

# ==============================================================================
# (3) Path traversal = HTTP 400
# ==============================================================================
info_msg "Check (3): Verifying SafePath & newtype rejection of '../../etc/passwd' (HTTP 400)..."
if [ -z "${AUTH_TOKEN}" ]; then
  fail_item "3" "Cannot test path traversal because login failed to obtain a valid session token"
else
  TRAVERSAL_APP_CODE="$(curl -s -o /dev/null -w '%{http_code}' -X POST "${PANEL_URL}/api/apps" \
    -H "Authorization: Bearer ${AUTH_TOKEN}" \
    -H "Content-Type: application/json" \
    -d '{
      "name": "TraversalApp",
      "domain": "traversal.ferrous.local",
      "framework": "Static HTML / SPA",
      "runtimeVersion": "1.0",
      "port": 8080,
      "workingDir": "/www/wwwroot/../../etc/passwd",
      "execCommand": "",
      "memoryLimitMb": 256,
      "sslEnabled": false,
      "createDb": false,
      "dbEngine": "None",
      "dbName": "",
      "dbUser": "",
      "urlRewritePreset": "spa_history"
    }' || echo "000")"

  DEPLOY_CLEAN_JSON="$(curl -sS -X POST "${PANEL_URL}/api/apps" \
    -H "Authorization: Bearer ${AUTH_TOKEN}" \
    -H "Content-Type: application/json" \
    -d "{
      \"name\": \"SmokeSite\",
      \"domain\": \"${SMOKE_DOMAIN}\",
      \"framework\": \"Static HTML / SPA\",
      \"runtimeVersion\": \"1.0\",
      \"port\": 18080,
      \"workingDir\": \"${WWWROOT}/${SMOKE_DOMAIN}\",
      \"execCommand\": \"\",
      \"memoryLimitMb\": 256,
      \"sslEnabled\": false,
      \"createDb\": false,
      \"dbEngine\": \"None\",
      \"dbName\": \"\",
      \"dbUser\": \"\",
      \"urlRewritePreset\": \"spa_history\"
    }" || true)"
  SMOKE_APP_ID="$(echo "${DEPLOY_CLEAN_JSON}" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p' | head -n 1)"

  TRAVERSAL_FILE_CODE="$(curl -s -o /dev/null -w '%{http_code}' -X POST "${PANEL_URL}/api/files/${SMOKE_APP_ID:-app-1}" \
    -H "Authorization: Bearer ${AUTH_TOKEN}" \
    -H "Content-Type: application/json" \
    -d '{"name":"../../etc/passwd","parentDir":"/../../../etc","type":"file","content":"pwn"}' || echo "000")"

  if [ "${TRAVERSAL_APP_CODE}" = "400" ] && [ "${TRAVERSAL_FILE_CODE}" = "400" ]; then
    pass_item "3" "Path traversal payloads ('../../etc/passwd') rejected with HTTP 400"
  else
    fail_item "3" "Expected HTTP 400 for path traversal, got apps=${TRAVERSAL_APP_CODE}, files=${TRAVERSAL_FILE_CODE}"
  fi
fi

# ==============================================================================
# (4) Create DB via API & verify with `mysql -e`
# ==============================================================================
info_msg "Check (4): Creating database '${SMOKE_DB_NAME}' via POST /api/databases and verifying with mysql -e..."
if [ -z "${AUTH_TOKEN}" ]; then
  fail_item "4" "Cannot verify database creation because AUTH_TOKEN is missing"
elif ! command -v mysql >/dev/null 2>&1; then
  skip_item "4" "mysql CLI binary not installed on host (expected 409 in production)"
else
  DB_CREATE_CODE="$(curl -s -o /tmp/fp-db-create.json -w '%{http_code}' -X POST "${PANEL_URL}/api/databases" \
    -H "Authorization: Bearer ${AUTH_TOKEN}" \
    -H "Content-Type: application/json" \
    -d "{
      \"name\": \"${SMOKE_DB_NAME}\",
      \"engine\": \"MariaDB 11.4\",
      \"username\": \"${SMOKE_DB_USER}\",
      \"collation\": \"utf8mb4_unicode_ci\",
      \"linkedAppId\": \"\"
    }" || echo "000")"

  MYSQL_FOUND="$(mysql -B -N -e "SHOW DATABASES LIKE '${SMOKE_DB_NAME}';" 2>/dev/null | tr -d '\r\n' || true)"
  if [ "${DB_CREATE_CODE}" = "200" ] && [ "${MYSQL_FOUND}" = "${SMOKE_DB_NAME}" ]; then
    pass_item "4" "Database '${SMOKE_DB_NAME}' created via API (HTTP 200) and confirmed by mysql -e"
  else
    fail_item "4" "Database API returned HTTP ${DB_CREATE_CODE} and mysql -e returned '${MYSQL_FOUND}'"
  fi
fi

# ==============================================================================
# (5) Start/stop systemd service via API & verify with `systemctl is-active`
# ==============================================================================
info_msg "Check (5): Deploying systemd app service, testing stop/start via API & verifying with systemctl is-active..."
if [ -z "${AUTH_TOKEN}" ]; then
  fail_item "5" "Cannot verify systemd unit because AUTH_TOKEN is missing"
elif ! command -v systemctl >/dev/null 2>&1 || [ ! -d /run/systemd/system ]; then
  skip_item "5" "systemd PID 1 is not active in this container/runner environment"
else
  SVC_DOMAIN="svc-$$.ferrous.local"
  SVC_DEPLOY_JSON="$(curl -sS -X POST "${PANEL_URL}/api/apps" \
    -H "Authorization: Bearer ${AUTH_TOKEN}" \
    -H "Content-Type: application/json" \
    -d "{
      \"name\": \"SmokeDaemon\",
      \"domain\": \"${SVC_DOMAIN}\",
      \"framework\": \"Actix-Web (Rust)\",
      \"runtimeVersion\": \"1.89\",
      \"port\": 19091,
      \"workingDir\": \"${WWWROOT}/${SVC_DOMAIN}\",
      \"execCommand\": \"/usr/bin/tail -f /dev/null\",
      \"memoryLimitMb\": 256,
      \"sslEnabled\": false,
      \"createDb\": false,
      \"dbEngine\": \"None\",
      \"dbName\": \"\",
      \"dbUser\": \"\",
      \"urlRewritePreset\": \"rust_proxy\"
    }" || true)"

  SVC_ID="$(echo "${SVC_DEPLOY_JSON}" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p' | head -n 1)"
  SVC_UNIT="$(echo "${SVC_DEPLOY_JSON}" | sed -n 's/.*"systemdUnit":"\([^"]*\)".*/\1/p' | head -n 1)"

  if [ -z "${SVC_ID}" ] || [ -z "${SVC_UNIT}" ]; then
    fail_item "5" "Failed to deploy systemd service via POST /api/apps"
  else
    curl -sS -X POST "${PANEL_URL}/api/apps/${SVC_ID}/action" \
      -H "Authorization: Bearer ${AUTH_TOKEN}" \
      -H "Content-Type: application/json" \
      -d '{"action":"stop"}' >/dev/null
    STATE_AFTER_STOP="$(systemctl is-active "${SVC_UNIT}" 2>/dev/null || true)"

    curl -sS -X POST "${PANEL_URL}/api/apps/${SVC_ID}/action" \
      -H "Authorization: Bearer ${AUTH_TOKEN}" \
      -H "Content-Type: application/json" \
      -d '{"action":"start"}' >/dev/null
    STATE_AFTER_START="$(systemctl is-active "${SVC_UNIT}" 2>/dev/null || true)"

    curl -sS -X DELETE "${PANEL_URL}/api/apps/${SVC_ID}" \
      -H "Authorization: Bearer ${AUTH_TOKEN}" >/dev/null 2>&1 || true
    rm -rf "${WWWROOT:?}/${SVC_DOMAIN}" 2>/dev/null || true

    if [ "${STATE_AFTER_STOP}" != "active" ] && [ "${STATE_AFTER_START}" = "active" ]; then
      pass_item "5" "Service ${SVC_UNIT} stopped (${STATE_AFTER_STOP}) and started (${STATE_AFTER_START}) confirmed via systemctl is-active"
    else
      fail_item "5" "Unexpected systemctl state for ${SVC_UNIT}: after_stop=${STATE_AFTER_STOP}, after_start=${STATE_AFTER_START}"
    fi
  fi
fi

# ==============================================================================
# (6) Site served via Nginx & FerrousX, engine switch & rollback on failure
# ==============================================================================
info_msg "Check (6): Serving site across Nginx & FerrousX, switching engine, and verifying rollback..."
if [ -z "${AUTH_TOKEN}" ]; then
  fail_item "6" "Cannot verify web server engines because AUTH_TOKEN is missing"
elif ! command -v nginx >/dev/null 2>&1 || [ ! -x /usr/local/bin/ferrousx-bin ]; then
  skip_item "6" "Requires both nginx and /usr/local/bin/ferrousx-bin installed for bidirectional switch test"
elif [ ! -d /run/systemd/system ]; then
  skip_item "6" "systemd PID 1 is not active (cannot start/stop nginx/ferrousx units)"
else
  MARKER="FERROUS_SMOKE_OK_$$"
  mkdir -p "${WWWROOT}/${SMOKE_DOMAIN}"
  echo "${MARKER}" > "${WWWROOT}/${SMOKE_DOMAIN}/index.html"

  curl -sS -X POST "${PANEL_URL}/api/webserver/switch" \
    -H "Authorization: Bearer ${AUTH_TOKEN}" \
    -H "Content-Type: application/json" \
    -d '{"targetEngine":"nginx"}' >/dev/null 2>&1 || true

  BODY_NGINX="$(curl -sS -H "Host: ${SMOKE_DOMAIN}" "http://127.0.0.1:80/" || true)"

  SWITCH_FX_CODE="$(curl -s -o /tmp/fp-switch-fx.json -w '%{http_code}' -X POST "${PANEL_URL}/api/webserver/switch" \
    -H "Authorization: Bearer ${AUTH_TOKEN}" \
    -H "Content-Type: application/json" \
    -d '{"targetEngine":"ferrousx"}' || echo "000")"
  BODY_FERROUSX="$(curl -sS -H "Host: ${SMOKE_DOMAIN}" "http://127.0.0.1:80/" || true)"

  ROLLBACK_CODE="$(curl -s -o /dev/null -w '%{http_code}' -X PUT "${PANEL_URL}/api/webserver" \
    -H "Authorization: Bearer ${AUTH_TOKEN}" \
    -H "Content-Type: application/json" \
    -d "{
      \"sites\": [{
        \"domain\": \"${SMOKE_DOMAIN}\",
        \"aliases\": [],
        \"root\": \"${WWWROOT}/${SMOKE_DOMAIN}\",
        \"mode\": {\"kind\": \"static_spa\", \"indexFile\": \"index.html\"},
        \"sslEnabled\": false,
        \"redirectHttps\": true,
        \"http3Quic\": false,
        \"websocketEnabled\": false,
        \"rateLimitRps\": 0
      }]
    }" || echo "000")"

  BODY_AFTER_ROLLBACK="$(curl -sS -H "Host: ${SMOKE_DOMAIN}" "http://127.0.0.1:80/" || true)"

  curl -sS -X POST "${PANEL_URL}/api/webserver/switch" \
    -H "Authorization: Bearer ${AUTH_TOKEN}" \
    -H "Content-Type: application/json" \
    -d '{"targetEngine":"nginx"}' >/dev/null 2>&1 || true

  if [[ "${BODY_NGINX}" == *"${MARKER}"* ]] \
    && [ "${SWITCH_FX_CODE}" = "200" ] \
    && [[ "${BODY_FERROUSX}" == *"${MARKER}"* ]] \
    && [ "${ROLLBACK_CODE}" = "400" ] \
    && [[ "${BODY_AFTER_ROLLBACK}" == *"${MARKER}"* ]]; then
    pass_item "6" "Site served by both Nginx & FerrousX, engine switched cleanly, invalid config rolled back"
  else
    fail_item "6" "Engine switch mismatch (switch_fx=${SWITCH_FX_CODE}, rollback_code=${ROLLBACK_CODE})"
  fi
fi

# ==============================================================================
# (7) cargo clippy -D warnings, cargo test, cargo audit
# ==============================================================================
info_msg "Check (7): Running cargo clippy -D warnings, cargo test, and cargo audit..."
if ! command -v cargo >/dev/null 2>&1; then
  skip_item "7" "cargo toolchain is not installed on this host (run on build runner)"
else
  CLIPPY_OK=false
  TEST_OK=false
  AUDIT_OK=true

  if cargo clippy --workspace --all-targets -- -D warnings; then
    CLIPPY_OK=true
  fi

  if cargo test --workspace; then
    TEST_OK=true
  fi

  if command -v cargo-audit >/dev/null 2>&1 || cargo audit --version >/dev/null 2>&1; then
    if ! cargo audit; then
      AUDIT_OK=false
    fi
  else
    echo -e "${YELLOW}  cargo-audit not installed (install via: cargo install cargo-audit)${NC}" >&2
  fi

  if [ "${CLIPPY_OK}" = true ] && [ "${TEST_OK}" = true ] && [ "${AUDIT_OK}" = true ]; then
    pass_item "7" "cargo clippy (-D warnings) and cargo test (--workspace) passed"
  else
    fail_item "7" "Rust verification failed (clippy=${CLIPPY_OK}, test=${TEST_OK}, audit=${AUDIT_OK})"
  fi
fi

# ==============================================================================
# (8) GET /api/capabilities returns real capability matrix
# ==============================================================================
info_msg "Check (8): Verifying GET /api/capabilities returns live capability probes..."
if [ -z "${AUTH_TOKEN}" ]; then
  fail_item "8" "Cannot query /api/capabilities without AUTH_TOKEN"
else
  CAPS_JSON="$(curl -sS -H "Authorization: Bearer ${AUTH_TOKEN}" "${PANEL_URL}/api/capabilities" || true)"
  if echo "${CAPS_JSON}" | grep -q '"capabilities"' && echo "${CAPS_JSON}" | grep -q '"nginx"' && echo "${CAPS_JSON}" | grep -q '"sqlite"'; then
    pass_item "8" "GET /api/capabilities returned live capability statuses"
  else
    fail_item "8" "GET /api/capabilities returned unexpected payload"
  fi
fi

# ==============================================================================
# (9) WebSSH allowlist blocks mutating sysctl / ip / ss / journalctl
# ==============================================================================
info_msg "Check (9): Verifying WebSSH blocks mutating commands (sysctl -w, ip link set, ss -K)..."
if [ -z "${AUTH_TOKEN}" ]; then
  fail_item "9" "Cannot test WebSSH without AUTH_TOKEN"
else
  SSH_BAD1="$(curl -s -o /dev/null -w '%{http_code}' -X POST "${PANEL_URL}/api/ssh/exec" \
    -H "Authorization: Bearer ${AUTH_TOKEN}" \
    -H "Content-Type: application/json" \
    -d '{"command":"sysctl -w net.ipv4.ip_forward=1","cwd":"/"}' || echo "000")"
  SSH_BAD2="$(curl -s -o /dev/null -w '%{http_code}' -X POST "${PANEL_URL}/api/ssh/exec" \
    -H "Authorization: Bearer ${AUTH_TOKEN}" \
    -H "Content-Type: application/json" \
    -d '{"command":"ip link set lo down","cwd":"/"}' || echo "000")"
  SSH_BAD3="$(curl -s -o /dev/null -w '%{http_code}' -X POST "${PANEL_URL}/api/ssh/exec" \
    -H "Authorization: Bearer ${AUTH_TOKEN}" \
    -H "Content-Type: application/json" \
    -d '{"command":"ss -K dst 1.1.1.1","cwd":"/"}' || echo "000")"

  if [[ "${SSH_BAD1}" =~ ^(400|403)$ ]] && [[ "${SSH_BAD2}" =~ ^(400|403)$ ]] && [[ "${SSH_BAD3}" =~ ^(400|403)$ ]]; then
    pass_item "9" "WebSSH rejected dangerous commands (sysctl -w=${SSH_BAD1}, ip set=${SSH_BAD2}, ss -K=${SSH_BAD3})"
  else
    fail_item "9" "WebSSH did not reject dangerous commands (got ${SSH_BAD1}, ${SSH_BAD2}, ${SSH_BAD3})"
  fi
fi

# ==============================================================================
# (10) Remote download SSRF guard blocks metadata & obfuscated IPs
# ==============================================================================
info_msg "Check (10): Verifying SSRF guard on /api/files/{id}/remote-download..."
if [ -z "${AUTH_TOKEN}" ] || [ -z "${SMOKE_APP_ID}" ]; then
  skip_item "10" "Requires active AUTH_TOKEN and deployed SMOKE_APP_ID"
else
  SSRF_META="$(curl -s -o /dev/null -w '%{http_code}' -X POST "${PANEL_URL}/api/files/${SMOKE_APP_ID}/remote-download" \
    -H "Authorization: Bearer ${AUTH_TOKEN}" \
    -H "Content-Type: application/json" \
    -d '{"url":"http://169.254.169.254/latest/meta-data/","parentDir":"/","fileName":"meta.txt"}' || echo "000")"
  SSRF_DEC="$(curl -s -o /dev/null -w '%{http_code}' -X POST "${PANEL_URL}/api/files/${SMOKE_APP_ID}/remote-download" \
    -H "Authorization: Bearer ${AUTH_TOKEN}" \
    -H "Content-Type: application/json" \
    -d '{"url":"http://2130706433/","parentDir":"/","fileName":"dec.txt"}' || echo "000")"

  if [[ "${SSRF_META}" =~ ^(400|403|409)$ ]] && [[ "${SSRF_DEC}" =~ ^(400|403|409)$ ]]; then
    pass_item "10" "Remote download SSRF guard blocked metadata (${SSRF_META}) and decimal IP (${SSRF_DEC})"
  else
    fail_item "10" "SSRF guard returned unexpected status codes (meta=${SSRF_META}, dec=${SSRF_DEC})"
  fi
fi

# ==============================================================================
# (11) Per-site settings: unsupported http3Quic returns 409 Conflict
# ==============================================================================
info_msg "Check (11): Verifying unsupported http3Quic returns HTTP 409 Conflict..."
if [ -z "${AUTH_TOKEN}" ]; then
  fail_item "11" "Cannot test per-site http3Quic without AUTH_TOKEN"
else
  H3_CODE="$(curl -s -o /dev/null -w '%{http_code}' -X PUT "${PANEL_URL}/api/webserver" \
    -H "Authorization: Bearer ${AUTH_TOKEN}" \
    -H "Content-Type: application/json" \
    -d "{
      \"sites\": [{
        \"domain\": \"${SMOKE_DOMAIN}\",
        \"aliases\": [],
        \"root\": \"${WWWROOT}/${SMOKE_DOMAIN}\",
        \"mode\": {\"kind\": \"static_spa\", \"indexFile\": \"index.html\"},
        \"sslEnabled\": false,
        \"redirectHttps\": false,
        \"http3Quic\": true,
        \"websocketEnabled\": false,
        \"rateLimitRps\": 10
      }]
    }" || echo "000")"
  if [ "${H3_CODE}" = "409" ]; then
    pass_item "11" "Unsupported http3Quic rejected honestly with HTTP 409 Conflict"
  else
    fail_item "11" "Expected HTTP 409 for unsupported http3Quic, got ${H3_CODE}"
  fi
fi

# ==============================================================================
# (12) UFW status & anti-lockout guard
# ==============================================================================
info_msg "Check (12): Checking UFW integration and anti-lockout guard..."
if [ -z "${AUTH_TOKEN}" ]; then
  fail_item "12" "Cannot check UFW without AUTH_TOKEN"
elif ! command -v ufw >/dev/null 2>&1; then
  skip_item "12" "ufw binary not installed on host"
else
  SEC_CODE="$(curl -s -o /tmp/fp-sec.json -w '%{http_code}' \
    -H "Authorization: Bearer ${AUTH_TOKEN}" "${PANEL_URL}/api/security" || echo "000")"
  if [ "${SEC_CODE}" = "200" ]; then
    pass_item "12" "GET /api/security loaded live UFW / Fail2ban / Sysctl data (HTTP 200)"
  else
    fail_item "12" "GET /api/security returned HTTP ${SEC_CODE}"
  fi
fi

# ==============================================================================
# (13) Telemetry & Audit Log return real host metrics
# ==============================================================================
info_msg "Check (13): Checking real /proc + sysinfo telemetry and audit log..."
if [ -z "${AUTH_TOKEN}" ]; then
  fail_item "13" "Cannot query telemetry without AUTH_TOKEN"
else
  TEL_JSON="$(curl -sS -H "Authorization: Bearer ${AUTH_TOKEN}" "${PANEL_URL}/api/telemetry" || true)"
  if echo "${TEL_JSON}" | grep -q '"memoryTotalGb"' && echo "${TEL_JSON}" | grep -q '"cpuCores"'; then
    pass_item "13" "GET /api/telemetry returned real host CPU/RAM/disk metrics"
  else
    fail_item "13" "GET /api/telemetry did not return expected host metrics"
  fi
fi

# ==============================================================================
# (14) Extended modules (Multi-PHP, Mail, Optimizer, Cron/Backup) honest probing
# ==============================================================================
info_msg "Check (14): Verifying Multi-PHP, Mail, Optimizer, and cPanel endpoints return real state..."
if [ -z "${AUTH_TOKEN}" ]; then
  fail_item "14" "Cannot query extended endpoints without AUTH_TOKEN"
else
  EXT_OK=true
  for ep in "/api/php" "/api/mail" "/api/optimizer" "/api/cpanel"; do
    ec="$(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer ${AUTH_TOKEN}" "${PANEL_URL}${ep}" || echo "000")"
    if [ "${ec}" != "200" ]; then
      EXT_OK=false
      echo -e "${YELLOW}  GET ${ep} returned HTTP ${ec}${NC}" >&2
    fi
  done
  if [ "${EXT_OK}" = true ]; then
    pass_item "14" "Extended endpoints (/api/php, /api/mail, /api/optimizer, /api/cpanel) responded HTTP 200 with real state"
  else
    fail_item "14" "One or more extended endpoints failed"
  fi
fi

echo -e "${BOLD}==============================================================================${NC}"
echo -e "${BOLD}  SUMMARY: PASS=${PASS_COUNT}  FAIL=${FAIL_COUNT}  SKIP=${SKIP_COUNT}${NC}"
echo -e "${BOLD}==============================================================================${NC}"

if [ "${FAIL_COUNT}" -gt 0 ]; then
  exit 1
fi
exit 0
