#!/usr/bin/env bash
set -euo pipefail

workspace_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$workspace_root"

if rg -n 'ai[_-]chat[_-]infrastructure' crates/domain crates/application; then
  echo "Domain and application layers must not depend on infrastructure." >&2
  exit 1
fi

if rg -n 'ai[_-]chat[_-]application' crates/domain; then
  echo "The domain layer must not depend on application use cases." >&2
  exit 1
fi

if rg -n 'path\s*=\s*"[^"]*apps/' crates --glob 'Cargo.toml' \
  || rg -n '(^|[^[:alnum:]_])apps::' crates --glob '*.rs'; then
  echo "Reusable crates must not depend on delivery applications." >&2
  exit 1
fi

if [[ ! -f docs/adr/README.md ]]; then
  echo "Architecture decisions must be indexed in docs/adr/README.md." >&2
  exit 1
fi

echo "Dependency boundary checks passed."
