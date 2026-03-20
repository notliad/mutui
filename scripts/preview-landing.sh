#!/usr/bin/env bash
set -euo pipefail

PORT="${1:-4173}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$ROOT_DIR/docs"
echo "Serving mutui landing page at http://localhost:${PORT}"
python3 -m http.server "$PORT"
