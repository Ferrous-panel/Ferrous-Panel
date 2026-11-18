#!/usr/bin/env bash
# ==============================================================================
# FerrousPanel — 1-Command In-Place Updater
# Preserves /var/lib/ferrous-panel and /etc/ferrous-panel configuration.
#
# 1-Command Update:
#   curl -fsSL https://raw.githubusercontent.com/Ferrous-panel/Ferrous-Panel/main/update.sh | sudo bash
# ==============================================================================
set -euo pipefail

BOLD='\033[1m'
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

if [ "$(id -u)" -ne 0 ]; then
  echo -e "${RED}[ERROR] update.sh must be run as root (use sudo).${NC}" >&2
  exit 1
fi

REPO_SLUG="${FERROUS_REPO:-Ferrous-panel/Ferrous-Panel}"
GIT_BRANCH="${FERROUS_BRANCH:-main}"
RELEASE_TAG="${1:-${FERROUS_VERSION:-latest}}"
SRC_DIR="/usr/local/src/ferrous-panel"

export RUSTUP_HOME="${RUSTUP_HOME:-/root/.rustup}"
export CARGO_HOME="${CARGO_HOME:-/root/.cargo}"
export PATH="${CARGO_HOME}/bin:${PATH}"

if [ -d "${SRC_DIR}/.git" ] && command -v cargo >/dev/null 2>&1; then
  echo -e "${BLUE}${BOLD}[*] Pulling latest source (${REPO_SLUG}@${GIT_BRANCH}) & rebuilding with Cargo...${NC}"
  git -C "${SRC_DIR}" fetch --depth 1 origin "${GIT_BRANCH}"
  git -C "${SRC_DIR}" reset --hard "origin/${GIT_BRANCH}"

  CARGO_PKGS=(-p ferrous-panel)
  if [ -x /usr/local/bin/ferrousx-bin ]; then
    CARGO_PKGS+=(-p ferrousx)
  fi

  (
    cd "${SRC_DIR}"
    cargo build --release "${CARGO_PKGS[@]}"
  )

  install -m 0755 "${SRC_DIR}/target/release/ferrous-panel" /usr/local/bin/ferrous-panel
  if [ -x /usr/local/bin/ferrousx-bin ] && [ -x "${SRC_DIR}/target/release/ferrousx-bin" ]; then
    install -m 0755 "${SRC_DIR}/target/release/ferrousx-bin" /usr/local/bin/ferrousx-bin
    systemctl restart ferrousx.service 2>/dev/null || true
  fi
else
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

  echo -e "${CYAN}[*] Downloading updated FerrousPanel release (${RELEASE_TAG}, ${ARCH})...${NC}"
  curl -fsSL --connect-timeout 10 -o "${TMP_DIR}/SHA256SUMS.txt" "${RELEASE_BASE}/SHA256SUMS.txt" 2>/dev/null || rm -f "${TMP_DIR}/SHA256SUMS.txt"

  curl -fsSL --retry 3 --connect-timeout 10 -o "${TMP_DIR}/ferrous-panel" "${RELEASE_BASE}/ferrous-panel-linux-${ARCH}"
  if [ -f "${TMP_DIR}/SHA256SUMS.txt" ] && grep -q " ferrous-panel-linux-${ARCH}\$" "${TMP_DIR}/SHA256SUMS.txt"; then
    (cd "$TMP_DIR" && mv ferrous-panel "ferrous-panel-linux-${ARCH}" && grep " ferrous-panel-linux-${ARCH}\$" SHA256SUMS.txt | sha256sum -c - && mv "ferrous-panel-linux-${ARCH}" ferrous-panel)
  fi
  install -m 0755 "${TMP_DIR}/ferrous-panel" /usr/local/bin/ferrous-panel

  if [ -x /usr/local/bin/ferrousx-bin ]; then
    echo -e "${CYAN}[*] Updating installed ferrousx-bin...${NC}"
    curl -fsSL --retry 3 --connect-timeout 10 -o "${TMP_DIR}/ferrousx-bin" "${RELEASE_BASE}/ferrousx-bin-linux-${ARCH}"
    if [ -f "${TMP_DIR}/SHA256SUMS.txt" ] && grep -q " ferrousx-bin-linux-${ARCH}\$" "${TMP_DIR}/SHA256SUMS.txt"; then
      (cd "$TMP_DIR" && mv ferrousx-bin "ferrousx-bin-linux-${ARCH}" && grep " ferrousx-bin-linux-${ARCH}\$" SHA256SUMS.txt | sha256sum -c - && mv "ferrousx-bin-linux-${ARCH}" ferrousx-bin)
    fi
    install -m 0755 "${TMP_DIR}/ferrousx-bin" /usr/local/bin/ferrousx-bin
    systemctl restart ferrousx.service 2>/dev/null || true
  fi
fi

ln -sf /usr/local/bin/ferrous-panel /usr/local/bin/fp
ln -sf /usr/local/bin/ferrous-panel /usr/local/bin/ferrous
systemctl restart ferrous-panel.service 2>/dev/null || true

echo -e "${GREEN}${BOLD}FerrousPanel updated cleanly. State in /var/lib/ferrous-panel preserved.${NC}"
