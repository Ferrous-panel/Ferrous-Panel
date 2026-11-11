#!/usr/bin/env bash
# ==============================================================================
# FerrousPanel v2.5 — Clean Uninstaller
# Safely uninstalls FerrousPanel service and system components.
# ==============================================================================
set -euo pipefail

BOLD='\033[1m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
NC='\033[0m'

if [ "$(id -u)" -ne 0 ]; then
  echo -e "${RED}[ERROR] This script must be run as root.${NC}" >&2
  exit 1
fi

echo -e "${YELLOW}${BOLD}==============================================================================${NC}"
echo -e "${YELLOW}${BOLD}  ⚠️  WARNING: FerrousPanel Uninstallation${NC}"
echo -e "${YELLOW}  This will stop and remove the FerrousPanel daemon, systemd services, and CLI tools.${NC}"
echo -e "${YELLOW}${BOLD}==============================================================================${NC}"

if [ -t 0 ]; then
  read -rp "Are you sure you want to proceed with uninstallation? (y/N): " confirm
  if [[ ! "$confirm" =~ ^[yY]([eE][sS])?$ ]]; then
    echo "Aborted."
    exit 0
  fi
fi

echo -e "${RED}[*] Stopping FerrousPanel systemd services...${NC}"
systemctl stop ferrous-panel.service 2>/dev/null || true
systemctl stop ferrous-panel-web.service 2>/dev/null || true
systemctl disable ferrous-panel.service 2>/dev/null || true
systemctl disable ferrous-panel-web.service 2>/dev/null || true

echo -e "${RED}[*] Removing systemd service definitions...${NC}"
rm -f /etc/systemd/system/ferrous-panel.service
rm -f /etc/systemd/system/ferrous-panel-web.service
systemctl daemon-reload

echo -e "${RED}[*] Removing CLI toolbox (/usr/local/bin/fp & /usr/local/bin/ferrous)...${NC}"
rm -f /usr/local/bin/fp /usr/local/bin/ferrous /usr/local/bin/ferrous-panel-daemon

echo -e "${RED}[*] Removing installation files in /opt/Ferrous-panel...${NC}"
rm -rf /opt/Ferrous-panel

echo -e "${GREEN}==============================================================================${NC}"
echo -e "${GREEN}  ✅ FerrousPanel daemon has been uninstalled.${NC}"
echo -e "  Note: Your website files in /www/wwwroot and databases in MariaDB/PostgreSQL"
echo -e "  have been safely preserved.${NC}"
echo -e "${GREEN}==============================================================================${NC}"
