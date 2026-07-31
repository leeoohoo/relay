#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-backup}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKUP_DIR="${BACKUP_DIR:-$ROOT_DIR/backups}"
DATABASE_URL="${DATABASE_URL:-}"

require_command() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "Missing required command: $1" >&2
    exit 1
  }
}

if [[ -z "$DATABASE_URL" ]]; then
  echo "DATABASE_URL is required." >&2
  exit 1
fi

case "$MODE" in
  backup)
    require_command pg_dump
    mkdir -p "$BACKUP_DIR"
    output="${2:-$BACKUP_DIR/ai_chat_$(date -u +%Y%m%dT%H%M%SZ).dump}"
    pg_dump --format=custom --compress=9 --no-owner --no-acl \
      --file "$output" "$DATABASE_URL"
    echo "Backup created: $output"
    ;;
  restore)
    require_command pg_restore
    input="${2:-}"
    if [[ -z "$input" || ! -f "$input" ]]; then
      echo "Usage: $0 restore /absolute/path/to/backup.dump" >&2
      exit 1
    fi
    if [[ "${CONFIRM_RESTORE:-}" != "yes" ]]; then
      echo "Restore replaces objects in the target database. Set CONFIRM_RESTORE=yes to continue." >&2
      exit 1
    fi
    pg_restore --clean --if-exists --no-owner --no-acl \
      --dbname "$DATABASE_URL" "$input"
    echo "Restore completed from: $input"
    ;;
  *)
    echo "Usage: $0 [backup|restore] [backup-file]" >&2
    exit 1
    ;;
esac
