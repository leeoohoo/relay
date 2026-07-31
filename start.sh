#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEV_SCRIPT="$ROOT_DIR/scripts/start_dev.sh"
DOCKER_SCRIPT="$ROOT_DIR/scripts/start_docker.sh"

usage() {
  cat <<'EOF'
AI Chat root startup helper

Usage:
  bash ./start.sh
      Start the recommended local development stack.
      Equivalent to: ./scripts/start_dev.sh up

  bash ./start.sh up|restart|down|logs|status|doctor
      Forward directly to ./scripts/start_dev.sh

  bash ./start.sh local [up|restart|down|logs|status|doctor]
      Explicit local-dev wrapper for ./scripts/start_dev.sh

  bash ./start.sh docker [up|rebuild|down|restart|ps|logs]
      Forward to ./scripts/start_docker.sh

Notes:
  - README recommends ./scripts/start_dev.sh up for day-to-day local development.
  - Use the docker mode when you want the all-in-one containerized startup path.
EOF
}

if [[ ! -f "$DEV_SCRIPT" ]]; then
  echo "Missing dev startup script: $DEV_SCRIPT" >&2
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
  local)
    shift || true
    exec bash "$DEV_SCRIPT" "$@"
    ;;
  *)
    exec bash "$DEV_SCRIPT" "$@"
    ;;
esac
