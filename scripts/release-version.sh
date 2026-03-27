#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "Usage: $0 <patch|minor|major>"
  exit 1
fi

BUMP_TYPE="$1"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST_PATH="${ROOT_DIR}/Cargo.toml"

case "${BUMP_TYPE}" in
  patch|minor|major)
    ;;
  *)
    echo "Invalid bump type: ${BUMP_TYPE}"
    echo "Expected one of: patch, minor, major"
    exit 1
    ;;
esac

if ! command -v cargo >/dev/null 2>&1; then
  echo "[error] cargo not found in PATH"
  exit 1
fi

CURRENT_VERSION="$(awk '
  /^\[workspace\.package\]$/ { in_section = 1; next }
  /^\[/ { if (in_section) exit }
  in_section && /^version[[:space:]]*=/ {
    gsub(/"/, "", $3)
    print $3
    exit
  }
' "${MANIFEST_PATH}")"

if [ -z "${CURRENT_VERSION}" ]; then
  echo "[error] Could not find [workspace.package].version in ${MANIFEST_PATH}"
  exit 1
fi

IFS='.' read -r MAJOR MINOR PATCH_EXTRA <<< "${CURRENT_VERSION}"
PATCH="${PATCH_EXTRA%%-*}"

if ! [[ "${MAJOR}" =~ ^[0-9]+$ && "${MINOR}" =~ ^[0-9]+$ && "${PATCH}" =~ ^[0-9]+$ ]]; then
  echo "[error] Unsupported version format: ${CURRENT_VERSION}"
  echo "Expected semantic version like 1.2.3"
  exit 1
fi

case "${BUMP_TYPE}" in
  patch)
    PATCH=$((PATCH + 1))
    ;;
  minor)
    MINOR=$((MINOR + 1))
    PATCH=0
    ;;
  major)
    MAJOR=$((MAJOR + 1))
    MINOR=0
    PATCH=0
    ;;
esac

NEW_VERSION="${MAJOR}.${MINOR}.${PATCH}"
TMP_FILE="$(mktemp)"

awk -v new_version="${NEW_VERSION}" '
  BEGIN { in_section = 0; replaced = 0 }
  /^\[workspace\.package\]$/ { in_section = 1; print; next }
  /^\[/ {
    if (in_section) in_section = 0
    print
    next
  }
  {
    if (in_section && /^version[[:space:]]*=/) {
      print "version = \"" new_version "\""
      replaced = 1
      next
    }
    print
  }
  END {
    if (!replaced) exit 2
  }
' "${MANIFEST_PATH}" > "${TMP_FILE}" || {
  rm -f "${TMP_FILE}"
  echo "[error] Failed updating workspace version in ${MANIFEST_PATH}"
  exit 1
}

mv "${TMP_FILE}" "${MANIFEST_PATH}"

echo "Version bumped: ${CURRENT_VERSION} -> ${NEW_VERSION}"
echo "Running cargo build --release --workspace ..."

cargo build --release --workspace

echo "Release build complete for version ${NEW_VERSION}"
