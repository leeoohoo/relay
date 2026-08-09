#!/usr/bin/env bash
set -euo pipefail

workspace_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$workspace_root"

if rg -n 'list_company_console_(agents|conversations|projects)_for_human' apps/server/src/company.rs; then
  echo "Console handlers must call cursor-page use cases, not unbounded list use cases." >&2
  exit 1
fi

for method in list_company_agent_membership_page list_company_conversation_page list_company_project_page; do
  if ! rg -q "fn ${method}" crates/infrastructure/src/postgres; then
    echo "PostgreSQL is missing the bounded ${method} query." >&2
    exit 1
  fi
done

for repository_file in company chat project; do
  if ! rg -q 'limit\.clamp\(1, 100\)' "crates/infrastructure/src/postgres/${repository_file}.rs"; then
    echo "Console PostgreSQL ${repository_file} pages must retain a hard maximum limit." >&2
    exit 1
  fi
done

if ! rg -q 'MAX_CONSOLE_PAGE_RESPONSE_BYTES' apps/server/src/company.rs; then
  echo "Console responses must retain a serialized payload budget." >&2
  exit 1
fi

echo "Console pagination boundary checks passed."
