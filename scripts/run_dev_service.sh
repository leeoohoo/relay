#!/usr/bin/env bash
set -euo pipefail

PID_FILE="$1"
WORK_DIR="$2"
shift 2

mkdir -p "$(dirname "$PID_FILE")"
printf '%s\n' "$$" >"$PID_FILE"
cd "$WORK_DIR"
exec "$@"
