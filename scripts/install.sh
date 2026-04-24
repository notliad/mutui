#!/usr/bin/env bash
set -euo pipefail

BIN_DIR="${HOME}/.local/bin"
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
WITH_TRAY=0

for arg in "$@"; do
  case "${arg}" in
    --with-tray) WITH_TRAY=1 ;;
    -h|--help)
      echo "Usage: install.sh [--with-tray]"
      echo "  --with-tray  Also build and install the mutui-tray system-tray binary"
      exit 0
      ;;
    *)
      echo "[error] Unknown option: ${arg}"
      echo "Usage: install.sh [--with-tray]"
      exit 1
      ;;
  esac
done

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
  if need_cmd pacman; then echo "pacman"; return; fi
  if need_cmd apt-get; then echo "apt"; return; fi
  if need_cmd dnf;     then echo "dnf"; return; fi
  if need_cmd brew;    then echo "brew"; return; fi
  echo ""
}

install_system_deps() {
  local missing=()

  # mpv is checked as a command; libmpv (the shared library) ships with it.
  need_cmd mpv        || missing+=("mpv")
  need_cmd yt-dlp     || missing+=("yt-dlp")
  need_cmd pkg-config || missing+=("pkg-config")

  if [ ${#missing[@]} -eq 0 ]; then
    echo "[deps] All runtime dependencies are satisfied."
    return
  fi

  local pm
  pm="$(detect_pkg_manager)"

  if [ -z "${pm}" ]; then
    echo "[deps] Could not detect a supported package manager."
    echo "[deps] Please install the following packages manually: ${missing[*]}"
    exit 1
  fi

  echo "[deps] Installing missing dependencies: ${missing[*]}"
  case "${pm}" in
    pacman) sudo pacman -Sy --needed --noconfirm "${missing[@]}" ;;
    apt)    sudo apt-get update && sudo apt-get install -y "${missing[@]}" ;;
    dnf)    sudo dnf install -y "${missing[@]}" ;;
    brew)   brew install "${missing[@]}" ;;
  esac
}

# --- Preflight checks ---

if ! need_cmd cargo; then
  echo "[error] cargo not found. Install Rust via https://rustup.rs and try again."
  exit 1
fi

install_system_deps

mkdir -p "${BIN_DIR}"

# --- Build ---

bins=(mutui mutuid)
if [[ "${WITH_TRAY}" -eq 1 ]]; then
  bins+=(mutui-tray)
fi

echo "[build] Compiling ${bins[*]} (release)..."
bin_args=()
for b in "${bins[@]}"; do
  bin_args+=(--bin "$b")
done
cargo build --release --manifest-path "${PROJECT_DIR}/Cargo.toml" \
  "${bin_args[@]}"

# --- Install ---

install -Dm0755 "${PROJECT_DIR}/target/release/mutui"  "${BIN_DIR}/mutui"
install -Dm0755 "${PROJECT_DIR}/target/release/mutuid" "${BIN_DIR}/mutuid"

if [[ "${WITH_TRAY}" -eq 1 ]]; then
  install -Dm0755 "${PROJECT_DIR}/target/release/mutui-tray" "${BIN_DIR}/mutui-tray"
  echo "[install] Installed to ${BIN_DIR}/mutui, ${BIN_DIR}/mutuid, ${BIN_DIR}/mutui-tray"
else
  echo "[install] Installed to ${BIN_DIR}/mutui and ${BIN_DIR}/mutuid"
fi

# --- PATH setup ---

if [[ ":${PATH}:" != *":${BIN_DIR}:"* ]]; then
  echo
  echo "[warning] ${BIN_DIR} is not in your PATH for this session."
  ensure_path_in_file "${HOME}/.bashrc"
  ensure_path_in_file "${HOME}/.profile"
  # macOS defaults to zsh; add to .zshrc and .zprofile as well.
  if [[ "$(uname)" == "Darwin" ]]; then
    ensure_path_in_file "${HOME}/.zshrc"
    ensure_path_in_file "${HOME}/.zprofile"
  fi
  echo "Added PATH export to shell profile(s)."
  echo "Reload your shell or run: export PATH=\"\$HOME/.local/bin:\$PATH\""
fi

echo
echo "Done. Run 'mutui' to start."
