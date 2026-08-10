#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVICE_FILE="$ROOT_DIR/crates/application/src/service.rs"

for bin in rg; do
  command -v "$bin" >/dev/null 2>&1 || {
    echo "Missing required command: $bin" >&2
    exit 1
  }
done

legacy_read_pattern='self\s*\.repo\s*\.\s*(get_human_user|get_company|get_company_human_member|get_conversation_context|get_conversation_messages|get_company_project|list_company_projects|list_company_project_tasks)\('

if rg --multiline --line-number "$legacy_read_pattern" "$SERVICE_FILE"; then
  echo >&2
  echo "Application services must use the Result-returning repository reads." >&2
  echo "Use the corresponding *_result method so database failures become Internal Error responses." >&2
  exit 1
fi

echo "Application service repository reads use Result-returning methods."
