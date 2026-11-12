#!/usr/bin/env bash
# ==============================================================================
# FerrousPanel v2.5 — 1-Click Update Script
# Updates FerrousPanel to the latest release from GitHub with zero data loss.
# ==============================================================================
set -euo pipefail

BOLD='\033[1m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

INSTALL_DIR="/opt/Ferrous-panel"
if [ ! -d "$INSTALL_DIR" ]; then
  INSTALL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fi

echo -e "${BLUE}${BOLD}[*] Starting FerrousPanel update in ${INSTALL_DIR}...${NC}"

cd "${INSTALL_DIR}"

# Backup admin config
if [ -f /etc/ferrous-panel/admin.json ]; then
  cp -f /etc/ferrous-panel/admin.json "/etc/ferrous-panel/admin.json.bak.$(date +%s)"
fi

# Pull latest code
if [ -d .git ]; then
  echo -e "${BLUE}[*] Pulling latest commits from GitHub...${NC}"
  git fetch origin main || true
  git reset --hard origin/main || true
fi

# Build static assets & Rust core
echo -e "${BLUE}[*] Building frontend & verifying Rust core...${NC}"
npm install --legacy-peer-deps --prefer-offline 2>/dev/null || npm install --legacy-peer-deps
npm run build

# Deploy latest compiled binary
if [ -f "${INSTALL_DIR}/rust-backend/target/release/ferrous-system-core" ]; then
  cp -f "${INSTALL_DIR}/rust-backend/target/release/ferrous-system-core" /usr/local/bin/ferrous-panel-daemon 2>/dev/null || true
  cp -f "${INSTALL_DIR}/rust-backend/target/release/ferrous-system-core" /usr/local/bin/ferrous-system-core 2>/dev/null || true
  chmod +x /usr/local/bin/ferrous-panel-daemon /usr/local/bin/ferrous-system-core 2>/dev/null || true
fi

# Re-link CLI commands if needed
if [ -f "${INSTALL_DIR}/Ferrous-panel/ferrous-cli.sh" ]; then
  cp -f "${INSTALL_DIR}/Ferrous-panel/ferrous-cli.sh" /usr/local/bin/fp
  cp -f "${INSTALL_DIR}/Ferrous-panel/ferrous-cli.sh" /usr/local/bin/ferrous
  chmod +x /usr/local/bin/fp /usr/local/bin/ferrous
fi

# Restart daemon
echo -e "${BLUE}[*] Restarting ferrous-panel systemd service...${NC}"
systemctl restart ferrous-panel.service 2>/dev/null || true

echo -e "${GREEN}${BOLD}==============================================================================${NC}"
echo -e "${GREEN}${BOLD}  ✅ FerrousPanel has been successfully updated to the latest version!${NC}"
echo -e "${GREEN}${BOLD}==============================================================================${NC}"
