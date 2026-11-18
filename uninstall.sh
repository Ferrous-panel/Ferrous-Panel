#!/usr/bin/env bash
# ==============================================================================
# FerrousPanel — 1-Command Clean Uninstaller
# Preserves /www/wwwroot, /var/lib/ferrous-panel, and SQL databases.
#
# 1-Command Uninstall:
#   curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/uninstall.sh | sudo bash
# ==============================================================================
set -euo pipefail

RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
NC='\033[0m'

PROMPT_CONFIRM=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --interactive) PROMPT_CONFIRM=true; shift ;;
    -y|--yes) PROMPT_CONFIRM=false; shift ;;
    *) shift ;;
  esac
done

if [ "$(id -u)" -ne 0 ]; then
  echo -e "${RED}[ERROR] uninstall.sh must be run as root (use sudo).${NC}" >&2
  exit 1
fi

if [ "$PROMPT_CONFIRM" = true ] && [ -t 0 ]; then
  read -rp "Stop and remove ferrous-panel/ferrousx services & binaries? (y/N): " confirm
  if [[ ! "${confirm:-N}" =~ ^[yY]([eE][sS])?$ ]]; then
    echo "Aborted."
    exit 0
  fi
fi

echo -e "${YELLOW}[*] Stopping services...${NC}"
systemctl disable --now ferrous-panel.service 2>/dev/null || true
systemctl disable --now ferrousx.service 2>/dev/null || true

rm -f /etc/systemd/system/ferrous-panel.service
rm -f /etc/systemd/system/ferrousx.service
rm -f /etc/systemd/system/ferrous-certbot-renew.service
rm -f /etc/systemd/system/ferrous-certbot-renew.timer
systemctl daemon-reload

rm -f /usr/local/bin/ferrous-panel /usr/local/bin/ferrousx-bin /usr/local/bin/fp /usr/local/bin/ferrous
rm -f /etc/profile.d/ferrous-panel.sh

echo -e "${GREEN}FerrousPanel binaries and systemd services removed.${NC}"
echo "Website files (/www/wwwroot), panel state (/var/lib/ferrous-panel), and SQL databases were preserved."
