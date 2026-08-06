#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@127.0.0.1:15533/ai_chat}"
PROBE_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/relay-migration-atomicity.XXXXXX")"
PROBE_VERSION="9999_atomicity_probe"
PROBE_TABLE="relay_migration_atomicity_probe"

cleanup() {
  if [[ -n "$PROBE_ROOT" && "$PROBE_ROOT" == *relay-migration-atomicity.* ]]; then
    rm -rf "$PROBE_ROOT"
  fi
}
trap cleanup EXIT

query_database() {
  local sql="$1"
  if command -v psql >/dev/null 2>&1; then
    psql "$DATABASE_URL" -Atqc "$sql"
    return
  fi

  local container="${PG_CONTAINER:-}"
  if [[ -z "$container" ]] && docker ps --format '{{.Names}}' | grep -x 'ai-chat-postgres' >/dev/null 2>&1; then
    container="ai-chat-postgres"
  fi
  if [[ -z "$container" ]] && docker ps --format '{{.Names}}' | grep -x 'postgres' >/dev/null 2>&1; then
    container="postgres"
  fi
  if [[ -z "$container" ]]; then
    echo "Neither psql nor a PostgreSQL container is available for verification." >&2
    return 1
  fi

  local uri_without_scheme="${DATABASE_URL#postgres://}"
  local creds_and_host="${uri_without_scheme%%/*}"
  local database_and_query="${uri_without_scheme#*/}"
  local database_name="${database_and_query%%\?*}"
  local user_and_password="${creds_and_host%%@*}"
  local database_user="${user_and_password%%:*}"
  local database_password="${user_and_password#*:}"
  docker exec -i -e PGPASSWORD="$database_password" "$container" \
    psql -U "$database_user" -d "$database_name" -Atqc "$sql"
}

mkdir -p "$PROBE_ROOT/$PROBE_VERSION"
cat > "$PROBE_ROOT/$PROBE_VERSION/up.sql" <<SQL
CREATE TABLE $PROBE_TABLE (id INTEGER PRIMARY KEY);
SELECT relay_intentionally_missing_function();
SQL
cat > "$PROBE_ROOT/$PROBE_VERSION/down.sql" <<SQL
DROP TABLE IF EXISTS $PROBE_TABLE;
SQL

if MIGRATIONS_ROOT="$PROBE_ROOT" "$ROOT_DIR/scripts/run_pg_migrations.sh" up; then
  echo "Expected the probe migration to fail, but it succeeded." >&2
  exit 1
fi

table_exists="$(query_database "SELECT to_regclass('public.$PROBE_TABLE') IS NOT NULL;")"
record_exists="$(query_database "SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = '$PROBE_VERSION');")"

if [[ "$table_exists" != "f" ]]; then
  echo "Failed migration left table $PROBE_TABLE behind." >&2
  exit 1
fi
if [[ "$record_exists" != "f" ]]; then
  echo "Failed migration left schema_migrations record $PROBE_VERSION behind." >&2
  exit 1
fi

echo "Migration failure rolled back schema changes and tracking record."
