#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PRODUCT_SCRIPT="$ROOT_DIR/scripts/start.sh"
DEV_SCRIPT="$ROOT_DIR/scripts/start_dev.sh"
DOCKER_SCRIPT="$ROOT_DIR/scripts/start_docker.sh"

usage() {
  cat <<'EOF'
Relay startup helper

Usage:
  ./start.sh [up|restart|down|status|logs]
      Start or manage the complete Relay product.
      Equivalent to: ./scripts/start.sh

  ./start.sh dev [up|restart|down|logs|status|doctor]
      Start the contributor development environment.

  ./start.sh docker [up|rebuild|down|restart|ps|logs]
      Manage only the Docker control plane. This does not start the host Trigger.

Notes:
  - Normal users should use the default command so the host Agent Trigger starts.
  - If execution permission was lost, run: chmod +x start.sh scripts/*.sh
  - You can always invoke this wrapper explicitly with: bash ./start.sh
EOF
}

if [[ ! -f "$PRODUCT_SCRIPT" ]]; then
  echo "Missing product startup script: $PRODUCT_SCRIPT" >&2
  exit 1
fi

if [[ ! -f "$DOCKER_SCRIPT" ]]; then
  echo "Missing Docker startup script: $DOCKER_SCRIPT" >&2
  exit 1
fi

case "${1:-up}" in
  -h|--help|help)
    usage
    ;;
  docker)
    shift || true
    exec bash "$DOCKER_SCRIPT" "$@"
    ;;
  dev|local)
    shift || true
    exec bash "$DEV_SCRIPT" "$@"
    ;;
  *)
    exec bash "$PRODUCT_SCRIPT" "$@"
    ;;
esac
