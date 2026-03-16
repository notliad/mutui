#!/usr/bin/env bash
set -euo pipefail

APP_DIR="${HOME}/.local/share/applications"
BIN_DIR="${HOME}/.local/bin"
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DESKTOP_FILE="${APP_DIR}/mutui.desktop"
LAUNCHER="${BIN_DIR}/mutui-launch"
ICON_SRC="${PROJECT_DIR}/assets/icon.png"
ICON_DIR="${HOME}/.local/share/icons/hicolor/256x256/apps"
ICON_FILE="${ICON_DIR}/mutui.png"

need_cmd() {
  command -v "$1" >/dev/null 2>&1
}

ensure_path_in_file() {
  local file="$1"
  local line='export PATH="$HOME/.local/bin:$PATH"'
  [ -f "${file}" ] || touch "${file}"
  if ! grep -Fq "$line" "${file}"; then
    printf '\n%s\n' "$line" >> "${file}"
  fi
}

detect_pkg_manager() {
  if need_cmd pacman; then
    echo "pacman"
    return
  fi
  if need_cmd apt-get; then
    echo "apt"
    return
  fi
  if need_cmd dnf; then
    echo "dnf"
    return
  fi
  echo ""
}

install_system_deps() {
  local missing=()
  need_cmd mpv || missing+=("mpv")
  need_cmd yt-dlp || missing+=("yt-dlp")

  if [ ${#missing[@]} -eq 0 ]; then
    echo "[deps] mpv and yt-dlp are already installed"
    return
  fi

  local pm
  pm="$(detect_pkg_manager)"
  if [ -z "${pm}" ]; then
    echo "[deps] Could not detect a supported package manager."
    echo "[deps] Please install manually: mpv yt-dlp"
    return
  fi

  echo "[deps] Installing system dependencies: ${missing[*]}"
  case "${pm}" in
    pacman)
      sudo pacman -Sy --needed --noconfirm "${missing[@]}"
      ;;
    apt)
      sudo apt-get update
      sudo apt-get install -y "${missing[@]}"
      ;;
    dnf)
      sudo dnf install -y "${missing[@]}"
      ;;
  esac
}

if ! need_cmd cargo; then
  echo "[error] cargo not found. Install Rust (https://rustup.rs) and try again."
  exit 1
fi

mkdir -p "${APP_DIR}" "${BIN_DIR}" "${ICON_DIR}"

install_system_deps

# Build required binaries explicitly.
cargo build --release --manifest-path "${PROJECT_DIR}/Cargo.toml" \
  --bin mutui --bin mutuid --bin mutui-tray

require_file() {
  if [ ! -f "$1" ]; then
    echo "[error] Missing build output: $1"
    exit 1
  fi
}

require_file "${PROJECT_DIR}/target/release/mutui"
require_file "${PROJECT_DIR}/target/release/mutuid"
require_file "${PROJECT_DIR}/target/release/mutui-tray"

# Install binaries into a user-local path that desktop launchers can find.
cp -f "${PROJECT_DIR}/target/release/mutui" "${BIN_DIR}/mutui"
cp -f "${PROJECT_DIR}/target/release/mutuid" "${BIN_DIR}/mutuid"
cp -f "${PROJECT_DIR}/target/release/mutui-tray" "${BIN_DIR}/mutui-tray"

chmod +x "${BIN_DIR}/mutui" "${BIN_DIR}/mutuid"
chmod +x "${BIN_DIR}/mutui-tray"

cat > "${LAUNCHER}" <<EOF
#!/usr/bin/env bash
set -euo pipefail

if [ -x "${BIN_DIR}/mutui-tray" ]; then
  nohup "${BIN_DIR}/mutui-tray" >/dev/null 2>&1 &
fi

exec "${BIN_DIR}/mutui"
EOF

chmod +x "${LAUNCHER}"

# Keep CLI usage simple: `mutui` in terminal uses the launcher behavior.
ln -sf "${LAUNCHER}" "${BIN_DIR}/mutui-app"

# Install desktop entry.
cp -f "${PROJECT_DIR}/packaging/mutui.desktop" "${DESKTOP_FILE}"

# Install app icon in user icon theme.
if [ -f "${ICON_SRC}" ]; then
  cp -f "${ICON_SRC}" "${ICON_FILE}"
fi

# Use absolute launcher path to avoid PATH differences in GUI launchers.
sed -i "s|^Exec=.*|Exec=${LAUNCHER}|" "${DESKTOP_FILE}"
sed -i "s|^Terminal=.*|Terminal=true|" "${DESKTOP_FILE}"
sed -i "s|^Icon=.*|Icon=mutui|" "${DESKTOP_FILE}"

# Refresh desktop database when available.
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "${APP_DIR}" || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f "${HOME}/.local/share/icons/hicolor" || true
fi

echo "Installation completed."
echo "- Launcher: ${DESKTOP_FILE}"
echo "- Binaries: ${BIN_DIR}/mutui ${BIN_DIR}/mutuid ${BIN_DIR}/mutui-tray"
echo "- Terminal command alias: ${BIN_DIR}/mutui-app"
echo "- Icon: ${ICON_FILE}"

if [[ ":${PATH}:" != *":${BIN_DIR}:"* ]]; then
  echo
  echo "[warning] ${BIN_DIR} is not in PATH for this shell session."
  ensure_path_in_file "${HOME}/.bashrc"
  ensure_path_in_file "${HOME}/.profile"
  echo "Added PATH export to ~/.bashrc and ~/.profile."
  echo "Open a new terminal and run: command -v mutui"
fi
