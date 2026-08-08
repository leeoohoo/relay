#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ "$(uname -s)" != "Darwin" || "$(uname -m)" != "arm64" ]]; then
  echo "This Relay package requires an Apple Silicon Mac." >&2
  exit 1
fi

if ! command -v docker >/dev/null 2>&1; then
  cat >&2 <<'EOF'
Docker Desktop is required.
Install it from https://www.docker.com/products/docker-desktop/ and run this installer again.
EOF
  exit 1
fi

# GitHub release downloads can inherit the browser quarantine attribute. Removing it
# from this already user-selected package prevents Gatekeeper from blocking the bundled
# command-line Trigger. A signed/notarized package does not need this fallback.
xattr -dr com.apple.quarantine "$ROOT_DIR" >/dev/null 2>&1 || true

if ! docker info >/dev/null 2>&1; then
  echo "Starting Docker Desktop…"
  open -a Docker >/dev/null 2>&1 || true
  for _ in {1..90}; do
    docker info >/dev/null 2>&1 && break
    sleep 2
  done
fi

docker info >/dev/null 2>&1 || {
  echo "Docker Desktop is not ready. Start it, wait for Docker to report Running, then retry." >&2
  exit 1
}

cd "$ROOT_DIR"
exec bash ./start.sh
