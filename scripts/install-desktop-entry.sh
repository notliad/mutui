#!/usr/bin/env bash
set -euo pipefail

APP_DIR="${HOME}/.local/share/applications"
BIN_DIR="${HOME}/.local/bin"
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

mkdir -p "${APP_DIR}" "${BIN_DIR}"

# Install mutui binary into a user-local path that desktop launchers can find.
cargo build --release --manifest-path "${PROJECT_DIR}/Cargo.toml"
cp -f "${PROJECT_DIR}/target/release/mutui" "${BIN_DIR}/mutui"

# Install desktop entry.
cp -f "${PROJECT_DIR}/packaging/mutui.desktop" "${APP_DIR}/mutui.desktop"

# Use absolute binary path to avoid PATH differences in GUI launchers.
sed -i "s|^Exec=.*|Exec=${BIN_DIR}/mutui|" "${APP_DIR}/mutui.desktop"

# Refresh desktop database when available.
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "${APP_DIR}" || true
fi

echo "Installed mutui launcher at ${APP_DIR}/mutui.desktop"
echo "If it does not appear immediately, log out/in or run: gtk-update-icon-cache -f ~/.local/share/icons 2>/dev/null || true"
