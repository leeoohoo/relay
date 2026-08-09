#!/usr/bin/env bash
set -euo pipefail

workspace_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$workspace_root"

if rg -n '/api/v1/companies/\$\{[^}]+\}/console|/api/v1/companies/[^"` ]+/console' apps/web/src; then
  echo "Web must use region-specific company queries instead of the legacy /console endpoint." >&2
  exit 1
fi

if [[ -e crates/application/src/repositories.rs || -d crates/application/src/services ]]; then
  echo "Do not reintroduce the unused repositories/services facade beside PlatformApp." >&2
  exit 1
fi

echo "Architecture boundary checks passed."
