#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MIGRATIONS_ROOT="${MIGRATIONS_ROOT:-$ROOT_DIR/migrations}"

MODE="${1:-up}"
DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@127.0.0.1:15533/ai_chat}"
FORCE_DOCKER_PSQL="${FORCE_DOCKER_PSQL:-false}"

detect_pg_container() {
  command -v docker >/dev/null 2>&1 || return 0

  if docker ps --format '{{.Names}}' | grep -x 'ai-chat-postgres' >/dev/null 2>&1; then
    echo "ai-chat-postgres"
    return
  fi

  if docker ps --format '{{.Names}}' | grep -x 'postgres' >/dev/null 2>&1; then
    echo "postgres"
    return
  fi
}

PG_CONTAINER="${PG_CONTAINER:-$(detect_pg_container)}"

if [[ ! -d "$MIGRATIONS_ROOT" ]]; then
  echo "Migration directory is missing: $MIGRATIONS_ROOT" >&2
  exit 1
fi

if [[ "$DATABASE_URL" != postgres://* ]]; then
  echo "Only postgres:// DATABASE_URL values are supported: $DATABASE_URL" >&2
  exit 1
fi

uri_without_scheme="${DATABASE_URL#postgres://}"
creds_and_host="${uri_without_scheme%%/*}"
db_and_query="${uri_without_scheme#*/}"
DB_NAME="${db_and_query%%\?*}"

if [[ "$creds_and_host" != *"@"* ]]; then
  echo "DATABASE_URL must include user:password@host:port/db" >&2
  exit 1
fi

user_and_password="${creds_and_host%%@*}"
host_and_port="${creds_and_host#*@}"

if [[ "$user_and_password" != *":"* ]]; then
  echo "DATABASE_URL must include both user and password" >&2
  exit 1
fi

DB_USER="${user_and_password%%:*}"
DB_PASSWORD="${user_and_password#*:}"

if [[ "$host_and_port" == *":"* ]]; then
  DB_HOST="${host_and_port%%:*}"
  DB_PORT="${host_and_port##*:}"
else
  DB_HOST="$host_and_port"
  DB_PORT="5432"
fi

HAS_LOCAL_PSQL="false"
if command -v psql >/dev/null 2>&1; then
  HAS_LOCAL_PSQL="true"
fi

if [[ "$FORCE_DOCKER_PSQL" == "true" ]]; then
  HAS_LOCAL_PSQL="false"
fi

if [[ "$HAS_LOCAL_PSQL" != "true" && -z "$PG_CONTAINER" ]]; then
  echo "Neither local psql nor a running PostgreSQL container was found." >&2
  echo "Start one with docker compose up -d or export PG_CONTAINER=<container-name>." >&2
  exit 1
fi

if [[ -n "$PG_CONTAINER" ]]; then
  docker inspect "$PG_CONTAINER" >/dev/null 2>&1 || {
    echo "Configured PG_CONTAINER does not exist: $PG_CONTAINER" >&2
    exit 1
  }
fi

SAFE_URL="postgres://${DB_USER}:****@${DB_HOST}:${DB_PORT}/${DB_NAME}"

run_psql_cmd() {
  local database="$1"
  shift

  if [[ "$HAS_LOCAL_PSQL" == "true" ]]; then
    PGPASSWORD="$DB_PASSWORD" psql \
      -v ON_ERROR_STOP=1 \
      -h "$DB_HOST" \
      -p "$DB_PORT" \
      -U "$DB_USER" \
      -d "$database" \
      "$@"
    return
  fi

  docker exec -i -e PGPASSWORD="$DB_PASSWORD" "$PG_CONTAINER" psql \
    -v ON_ERROR_STOP=1 \
    -h 127.0.0.1 \
    -p 5432 \
    -U "$DB_USER" \
    -d "$database" \
    "$@"
}

run_psql_file() {
  local database="$1"
  local sql_file="$2"

  if [[ "$HAS_LOCAL_PSQL" == "true" ]]; then
    PGPASSWORD="$DB_PASSWORD" psql \
      -v ON_ERROR_STOP=1 \
      -h "$DB_HOST" \
      -p "$DB_PORT" \
      -U "$DB_USER" \
      -d "$database" \
      -f "$sql_file"
    return
  fi

  docker exec -i -e PGPASSWORD="$DB_PASSWORD" "$PG_CONTAINER" psql \
    -v ON_ERROR_STOP=1 \
    -h 127.0.0.1 \
    -p 5432 \
    -U "$DB_USER" \
    -d "$database" \
    -f - < "$sql_file"
}

run_psql_migration() {
  local database="$1"
  local sql_file="$2"
  local tracking_sql="$3"

  if [[ "$HAS_LOCAL_PSQL" == "true" ]]; then
    {
      printf '\\set ON_ERROR_STOP on\n'
      sed -e '$a\' "$sql_file"
      printf '%s\n' "$tracking_sql"
    } | PGPASSWORD="$DB_PASSWORD" psql \
      --single-transaction \
      -v ON_ERROR_STOP=1 \
      -h "$DB_HOST" \
      -p "$DB_PORT" \
      -U "$DB_USER" \
      -d "$database" \
      -f -
    return
  fi

  {
    printf '\\set ON_ERROR_STOP on\n'
    sed -e '$a\' "$sql_file"
    printf '%s\n' "$tracking_sql"
  } | docker exec -i -e PGPASSWORD="$DB_PASSWORD" "$PG_CONTAINER" psql \
    --single-transaction \
    -v ON_ERROR_STOP=1 \
    -h 127.0.0.1 \
    -p 5432 \
    -U "$DB_USER" \
    -d "$database" \
    -f -
}

validate_migration_version() {
  local version="$1"
  if [[ ! "$version" =~ ^[A-Za-z0-9_-]+$ ]]; then
    echo "Unsafe migration version: $version" >&2
    exit 1
  fi
}

database_exists() {
  local result
  result="$(run_psql_cmd postgres -Atqc "SELECT 1 FROM pg_database WHERE datname = '$DB_NAME';" | tr -d '[:space:]')"
  [[ "$result" == "1" ]]
}

ensure_database() {
  if database_exists; then
    return
  fi

  echo "Creating database $DB_NAME"
  run_psql_cmd postgres -c "CREATE DATABASE \"$DB_NAME\";" >/dev/null
}

ensure_schema_migrations_table() {
  run_psql_cmd "$DB_NAME" -c "
    CREATE TABLE IF NOT EXISTS schema_migrations (
      version TEXT PRIMARY KEY,
      applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );
  " >/dev/null
}

seed_legacy_baseline_if_needed() {
  local has_human_users has_schema_migrations applied_count

  has_human_users="$(run_psql_cmd "$DB_NAME" -Atqc "SELECT to_regclass('public.human_users') IS NOT NULL;" | tr -d '[:space:]')"
  has_schema_migrations="$(run_psql_cmd "$DB_NAME" -Atqc "SELECT to_regclass('public.schema_migrations') IS NOT NULL;" | tr -d '[:space:]')"

  if [[ "$has_human_users" != "t" || "$has_schema_migrations" != "t" ]]; then
    return
  fi

  applied_count="$(run_psql_cmd "$DB_NAME" -Atqc "SELECT COUNT(*) FROM schema_migrations;" | tr -d '[:space:]')"
  if [[ "$applied_count" != "0" ]]; then
    return
  fi

  if [[ -d "$MIGRATIONS_ROOT/0001_init" ]]; then
    echo "Seeding legacy baseline migration record: 0001_init"
    run_psql_cmd "$DB_NAME" -c "INSERT INTO schema_migrations(version) VALUES ('0001_init') ON CONFLICT (version) DO NOTHING;" >/dev/null
  fi
}

list_migration_versions() {
  find "$MIGRATIONS_ROOT" -mindepth 1 -maxdepth 1 -type d -exec basename {} \; | sort
}

migration_up_file() {
  local version="$1"
  echo "$MIGRATIONS_ROOT/$version/up.sql"
}

migration_down_file() {
  local version="$1"
  echo "$MIGRATIONS_ROOT/$version/down.sql"
}

is_migration_applied() {
  local version="$1"
  local result
  result="$(run_psql_cmd "$DB_NAME" -Atqc "SELECT 1 FROM schema_migrations WHERE version = '$version';" | tr -d '[:space:]')"
  [[ "$result" == "1" ]]
}

apply_migration_up() {
  local version="$1"
  validate_migration_version "$version"
  local up_file
  up_file="$(migration_up_file "$version")"

  if [[ ! -f "$up_file" ]]; then
    echo "Missing up.sql for migration $version" >&2
    exit 1
  fi

  if is_migration_applied "$version"; then
    echo "Migration $version already applied, skipping."
    return
  fi

  echo "Applying migration $version"
  run_psql_migration "$DB_NAME" "$up_file" \
    "INSERT INTO schema_migrations(version) VALUES ('$version');"
}

apply_migration_down() {
  local version="$1"
  validate_migration_version "$version"
  local down_file
  down_file="$(migration_down_file "$version")"

  if [[ ! -f "$down_file" ]]; then
    echo "Missing down.sql for migration $version" >&2
    exit 1
  fi

  if ! is_migration_applied "$version"; then
    echo "Migration $version is not applied, skipping rollback."
    return
  fi

  echo "Rolling back migration $version"
  run_psql_migration "$DB_NAME" "$down_file" \
    "DELETE FROM schema_migrations WHERE version = '$version';"
}

apply_all_up() {
  local versions=()
  local version
  while IFS= read -r version; do
    [[ -n "$version" ]] && versions+=("$version")
  done < <(list_migration_versions)

  for version in "${versions[@]}"; do
    apply_migration_up "$version"
  done
}

rollback_all_down() {
  local versions=()
  local version
  while IFS= read -r version; do
    [[ -n "$version" ]] && versions+=("$version")
  done < <(list_migration_versions)

  local index
  for (( index=${#versions[@]}-1; index>=0; index-- )); do
    apply_migration_down "${versions[$index]}"
  done
}

schema_initialized() {
  local result
  result="$(run_psql_cmd "$DB_NAME" -Atqc "SELECT to_regclass('public.schema_migrations') IS NOT NULL;" | tr -d '[:space:]')"
  [[ "$result" == "t" ]]
}

print_status() {
  if ! database_exists; then
    echo "Database $DB_NAME does not exist yet."
    return
  fi

  ensure_schema_migrations_table
  seed_legacy_baseline_if_needed

  echo "Applied migrations:"
  run_psql_cmd "$DB_NAME" -c "SELECT version, applied_at FROM schema_migrations ORDER BY version;"
  echo
  echo "Public tables:"
  run_psql_cmd "$DB_NAME" -c \
    "SELECT tablename FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename;"
}

echo "Running migration mode '$MODE' against $SAFE_URL"
if [[ -n "$PG_CONTAINER" && "$HAS_LOCAL_PSQL" != "true" ]]; then
  echo "Using PostgreSQL container: $PG_CONTAINER"
fi

case "$MODE" in
  up)
    ensure_database
    ensure_schema_migrations_table
    seed_legacy_baseline_if_needed
    apply_all_up
    ;;
  down)
    if database_exists; then
      ensure_schema_migrations_table
      seed_legacy_baseline_if_needed
      rollback_all_down
    else
      echo "Database $DB_NAME does not exist, nothing to roll back."
    fi
    ;;
  reset)
    ensure_database
    ensure_schema_migrations_table
    seed_legacy_baseline_if_needed
    rollback_all_down
    apply_all_up
    ;;
  ensure)
    ensure_database
    if ! schema_initialized; then
      ensure_schema_migrations_table
    fi
    seed_legacy_baseline_if_needed
    apply_all_up
    ;;
  status)
    print_status
    ;;
  *)
    echo "Usage: $0 [up|down|reset|ensure|status]" >&2
    exit 1
    ;;
esac

echo "Migration step '$MODE' completed."
