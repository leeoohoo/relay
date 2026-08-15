#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PG_CONTAINER="${PG_CONTAINER:-ai-chat-postgres}"
SERVER_PORT="${RELAY_LOAD_TEST_PORT:-18089}"
FAKE_CODEX_SECONDS="${RELAY_LOAD_FAKE_CODEX_SECONDS:-1.0}"
TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/relay-trigger-load.XXXXXX")"
DATABASE_NAME="relay_load_$(date +%s)_${RANDOM}"
PG_HOST_PORT="${PG_HOST_PORT:-}"
DATABASE_URL=""
API_BASE_URL="http://127.0.0.1:${SERVER_PORT}"
SERVER_PID=""
TRIGGER_PID=""

if [[ ! "$DATABASE_NAME" =~ ^relay_load_[0-9]+_[0-9]+$ ]]; then
  echo "Refusing to use unsafe load-test database name: $DATABASE_NAME" >&2
  exit 1
fi

cleanup() {
  local pid
  for pid in "$TRIGGER_PID" "$SERVER_PID"; do
    if [[ -n "$pid" ]] && kill -0 "$pid" >/dev/null 2>&1; then
      kill "$pid" >/dev/null 2>&1 || true
      wait "$pid" 2>/dev/null || true
    fi
  done
  docker exec "$PG_CONTAINER" dropdb --if-exists --force -U postgres "$DATABASE_NAME" \
    >/dev/null 2>&1 || true
  if [[ -d "$TEMP_ROOT" && "$TEMP_ROOT" == *"/relay-trigger-load."* ]]; then
    rm -rf -- "$TEMP_ROOT"
  fi
}
trap cleanup EXIT INT TERM

for command in awk cargo curl docker grep jq lsof pgrep ps sed; do
  command -v "$command" >/dev/null 2>&1 || {
    echo "Missing required command: $command" >&2
    exit 1
  }
done
docker inspect "$PG_CONTAINER" >/dev/null 2>&1 || {
  echo "PostgreSQL container is unavailable: $PG_CONTAINER" >&2
  exit 1
}
if [[ -z "$PG_HOST_PORT" ]]; then
  PG_HOST_PORT="$(docker inspect -f '{{with index .HostConfig.PortBindings "5432/tcp"}}{{(index . 0).HostPort}}{{end}}' "$PG_CONTAINER")"
fi
[[ "$PG_HOST_PORT" =~ ^[0-9]+$ ]] || {
  echo "Cannot resolve PostgreSQL host port for $PG_CONTAINER" >&2
  exit 1
}
DATABASE_URL="postgres://postgres:postgres@127.0.0.1:${PG_HOST_PORT}/${DATABASE_NAME}"
if lsof -nP -iTCP:"$SERVER_PORT" -sTCP:LISTEN >/dev/null 2>&1; then
  echo "Load-test port is already in use: $SERVER_PORT" >&2
  exit 1
fi

pg_query() {
  local sql="$1"
  docker exec "$PG_CONTAINER" psql -U postgres -d "$DATABASE_NAME" -Atqc "$sql"
}

post_json() {
  local path="$1"
  local payload="$2"
  shift 2
  curl -fsS -H 'content-type: application/json' "$@" -d "$payload" "$API_BASE_URL$path"
}

wait_for_http() {
  local url="$1"
  local pid="$2"
  local log="$3"
  local attempt
  for attempt in {1..100}; do
    if curl -fsS "$url" >/dev/null 2>&1; then
      return 0
    fi
    if ! kill -0 "$pid" >/dev/null 2>&1; then
      echo "Process exited before becoming ready: $url" >&2
      tail -n 80 "$log" >&2 || true
      return 1
    fi
    sleep 0.1
  done
  echo "Timed out waiting for $url" >&2
  tail -n 80 "$log" >&2 || true
  return 1
}

collect_descendants() {
  local parent_pid="$1"
  local child_pid
  while IFS= read -r child_pid; do
    [[ -n "$child_pid" ]] || continue
    printf '%s\n' "$child_pid"
    collect_descendants "$child_pid"
  done < <(pgrep -P "$parent_pid" 2>/dev/null || true)
}

sample_runtime() {
  local output_file="$1"
  local pids="$TRIGGER_PID"
  local descendants
  descendants="$(collect_descendants "$TRIGGER_PID")"
  if [[ -n "$descendants" ]]; then
    pids+=" $(tr '\n' ' ' <<<"$descendants")"
  fi
  local csv
  csv="$(tr ' ' ',' <<<"$pids" | sed -E 's/,+/,/g; s/^,//; s/,$//')"
  local process_sample
  process_sample="$(ps -o %cpu=,rss= -p "$csv" 2>/dev/null | awk '
    { cpu += $1; rss += $2; processes += 1 }
    END { printf "%.2f %d %d", cpu + 0, rss + 0, processes + 0 }
  ')"
  local connections
  connections="$(pg_query "select count(*) from pg_stat_activity where datname = current_database();")"
  printf '%s %s\n' "$process_sample" "$connections" >>"$output_file"
}

wait_for_runs() {
  local marker="$1"
  local expected="$2"
  local sample_file="$3"
  local attempt completed
  for attempt in {1..600}; do
    sample_runtime "$sample_file"
    completed="$(pg_query "
      select count(*)
      from agent_codex_trigger_runs
      where started_at >= timestamptz '$marker'
        and status in ('succeeded', 'failed', 'cancelled', 'timed_out');
    ")"
    if (( completed >= expected )); then
      return 0
    fi
    if ! kill -0 "$TRIGGER_PID" >/dev/null 2>&1; then
      echo "Trigger exited during the load scenario" >&2
      tail -n 120 "$TEMP_ROOT/trigger.log" >&2 || true
      return 1
    fi
    sleep 0.1
  done
  echo "Timed out waiting for $expected Trigger runs" >&2
  return 1
}

run_scenario() {
  local requested="$1"
  local marker before_transactions after_transactions
  local sample_file="$TEMP_ROOT/scenario-${requested}.samples"
  : >"$sample_file"
  sleep 1.2
  pg_query "
    select pg_terminate_backend(pid)
    from pg_stat_activity
    where datname = current_database()
      and pid <> pg_backend_pid()
      and state = 'idle'
      and application_name in ('relay-server', 'relay-trigger');
  " >/dev/null
  sleep 0.5
  pg_query 'select pg_stat_reset();' >/dev/null
  marker="$(pg_query 'select clock_timestamp();')"
  before_transactions="$(pg_query "
    select xact_commit + xact_rollback
    from pg_stat_database
    where datname = current_database();
  ")"
  local index agent_id
  for ((index = 0; index < requested; index++)); do
    agent_id="${AGENT_IDS[$index]}"
    post_json "/api/v1/companies/$COMPANY_ID/agents/$agent_id/codex-trigger/run-now" \
      '{}' -H "authorization: Bearer $OWNER_TOKEN" >/dev/null
  done
  wait_for_runs "$marker" "$requested" "$sample_file"
  sleep 1.2
  after_transactions="$(pg_query "
    select xact_commit + xact_rollback
    from pg_stat_database
    where datname = current_database();
  ")"

  local run_metrics peak_concurrency resource_metrics
  run_metrics="$(pg_query "
    with selected as (
      select status, started_at, finished_at
      from agent_codex_trigger_runs
      where started_at >= timestamptz '$marker'
      order by started_at
      limit $requested
    )
    select
      count(*) filter (where status = 'succeeded'),
      round(extract(epoch from (max(finished_at) - timestamptz '$marker')) * 1000),
      round(extract(epoch from (max(started_at) - timestamptz '$marker')) * 1000),
      round(extract(epoch from (max(finished_at) - min(started_at))) * 1000)
    from selected;
  ")"
  peak_concurrency="$(pg_query "
    with selected as (
      select started_at, finished_at
      from agent_codex_trigger_runs
      where started_at >= timestamptz '$marker'
      order by started_at
      limit $requested
    ), events as (
      select started_at as event_at, 1 as delta from selected
      union all
      select finished_at as event_at, -1 as delta from selected
    ), levels as (
      select sum(delta) over (order by event_at, delta desc rows unbounded preceding) as running
      from events
    )
    select coalesce(max(running), 0) from levels;
  ")"
  resource_metrics="$(awk '
    { cpu += $1; if ($1 > max_cpu) max_cpu = $1; if ($2 > max_rss) max_rss = $2; if ($3 > max_processes) max_processes = $3; if ($4 > max_connections) max_connections = $4; samples += 1 }
    END { printf "%.2f|%.2f|%d|%d|%d|%d", cpu / (samples ? samples : 1), max_cpu + 0, max_rss + 0, max_processes + 0, max_connections + 0, samples + 0 }
  ' "$sample_file")"

  local succeeded end_to_end_ms wake_ms active_ms
  IFS='|' read -r succeeded end_to_end_ms wake_ms active_ms <<<"$run_metrics"
  local average_cpu peak_cpu peak_rss peak_processes peak_connections samples
  IFS='|' read -r average_cpu peak_cpu peak_rss peak_processes peak_connections samples \
    <<<"$resource_metrics"
  if [[ "$succeeded" != "$requested" ]]; then
    echo "Scenario $requested completed with only $succeeded successful runs" >&2
    return 1
  fi

  jq -cn \
    --argjson agents "$requested" \
    --argjson succeeded "$succeeded" \
    --argjson end_to_end_ms "$end_to_end_ms" \
    --argjson wake_ms "$wake_ms" \
    --argjson active_ms "$active_ms" \
    --argjson peak_concurrency "$peak_concurrency" \
    --argjson average_cpu_percent "$average_cpu" \
    --argjson peak_cpu_percent "$peak_cpu" \
    --argjson peak_rss_kib "$peak_rss" \
    --argjson peak_processes "$peak_processes" \
    --argjson peak_db_connections "$peak_connections" \
    --argjson database_transactions "$((after_transactions - before_transactions))" \
    --argjson samples "$samples" \
    '{agents:$agents,succeeded:$succeeded,end_to_end_ms:$end_to_end_ms,wake_ms:$wake_ms,active_window_ms:$active_ms,peak_concurrency:$peak_concurrency,trigger_tree_cpu_percent:{average:$average_cpu_percent,peak:$peak_cpu_percent},trigger_tree_peak_rss_kib:$peak_rss_kib,trigger_tree_peak_processes:$peak_processes,peak_db_connections:$peak_db_connections,database_transactions:$database_transactions,samples:$samples}'
}

echo "Building release Server and Trigger..."
if [[ "${RELAY_LOAD_TEST_SKIP_BUILD:-false}" != "true" ]]; then
  cargo build --release -p ai-chat-server -p ai-chat-agent-trigger \
    --manifest-path "$ROOT_DIR/Cargo.toml" >"$TEMP_ROOT/build.log" 2>&1
fi

FAKE_CODEX="$TEMP_ROOT/fake-codex.sh"
cat >"$FAKE_CODEX" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
for argument in "$@"; do
  if [[ "$argument" == "--version" ]]; then
    echo "codex-load-fixture 1.0"
    exit 0
  fi
done
if [[ " $* " == *" login status "* ]]; then
  echo "Logged in using load fixture"
  exit 0
fi
if [[ " $* " != *" exec "* ]]; then
  printf '%s\n' '[]'
  exit 0
fi
thread_id="load-thread-$$"
previous=""
for argument in "$@"; do
  if [[ "$previous" == "resume" ]]; then
    thread_id="$argument"
    break
  fi
  previous="$argument"
done
sleep "${RELAY_LOAD_FAKE_CODEX_SECONDS:-1.0}"
printf '{"type":"thread.started","thread_id":"%s"}\n' "$thread_id"
printf '%s\n' '{"type":"turn.started"}'
printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"load fixture completed"}}'
printf '%s\n' '{"type":"turn.completed"}'
EOF
chmod 700 "$FAKE_CODEX"

echo "Creating isolated database $DATABASE_NAME..."
docker exec "$PG_CONTAINER" createdb -U postgres "$DATABASE_NAME"
DATABASE_URL="$DATABASE_URL" FORCE_DOCKER_PSQL=true PG_CONTAINER="$PG_CONTAINER" \
  "$ROOT_DIR/scripts/run_pg_migrations.sh" up >"$TEMP_ROOT/migrations.log"

mkdir -p "$TEMP_ROOT/server-state" "$TEMP_ROOT/trigger-state" "$TEMP_ROOT/workspaces"
env \
  API_HOST=127.0.0.1 \
  API_PORT="$SERVER_PORT" \
  PUBLIC_BASE_URL="$API_BASE_URL" \
  DATABASE_URL="$DATABASE_URL" \
  APP_ENV=development \
  ENABLE_DEV_ENDPOINTS=true \
  REQUIRE_EMAIL_VERIFICATION=false \
  HARNESS_MODE=disabled \
  AGENT_TRIGGER_STATE_ROOT="$TEMP_ROOT/trigger-state" \
  RELAY_DEFAULT_WORKSPACE_ROOT="$TEMP_ROOT/workspaces" \
  AGENT_TRIGGER_MANAGED_PROJECTS_ROOT="$TEMP_ROOT/workspaces" \
  AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS="$TEMP_ROOT/workspaces" \
  MESSAGE_ATTACHMENTS_ROOT="$TEMP_ROOT/server-state/attachments" \
  HARNESS_CREDENTIALS_ROOT="$TEMP_ROOT/server-state/harness-credentials" \
  RUST_LOG=warn \
  "$ROOT_DIR/target/release/ai-chat-server" >"$TEMP_ROOT/server.log" 2>&1 &
SERVER_PID=$!
wait_for_http "$API_BASE_URL/health" "$SERVER_PID" "$TEMP_ROOT/server.log"

suffix="$(date +%s)-${RANDOM}"
owner="$(post_json /api/v1/auth/register "$(jq -cn --arg suffix "$suffix" '{email:("load-"+$suffix+"@example.com"),display_name:"Load Owner",password:"load-password-123"}')")"
OWNER_TOKEN="$(jq -r '.session_token' <<<"$owner")"
company="$(post_json /api/v1/companies "$(jq -cn --arg suffix "$suffix" '{name:("Load Company "+$suffix),slug:("load-company-"+$suffix)}')" -H "authorization: Bearer $OWNER_TOKEN")"
COMPANY_ID="$(jq -r '.company_console.company.id' <<<"$company")"
ORG_ID="$(jq -r '.company_console.org_units[0].id' <<<"$company")"
AGENT_IDS=()
manager_membership_id=""
for index in {1..10}; do
  if (( index == 1 )); then
    payload="$(jq -cn --arg suffix "$suffix" --arg org "$ORG_ID" '{display_name:"Load Manager",handle:("load-manager-"+$suffix),persona:"Load fixture manager",org_unit_id:$org,profession_key:"project_manager",role_key:"company_manager"}')"
  else
    payload="$(jq -cn --arg suffix "$suffix" --arg org "$ORG_ID" --arg manager "$manager_membership_id" --argjson index "$index" '{display_name:("Load Agent "+($index|tostring)),handle:("load-agent-"+($index|tostring)+"-"+$suffix),persona:"Load fixture worker",org_unit_id:$org,profession_key:"software_engineer",role_key:"member",reports_to_membership_id:$manager}')"
  fi
  agent="$(post_json "/api/v1/companies/$COMPANY_ID/agents" "$payload" -H "authorization: Bearer $OWNER_TOKEN")"
  agent_id="$(jq -r '.result.agent_profile.id' <<<"$agent")"
  AGENT_IDS+=("$agent_id")
  if (( index == 1 )); then
    manager_membership_id="$(jq -r '.result.membership.id' <<<"$agent")"
  fi
  post_json "/api/v1/companies/$COMPANY_ID/agents/$agent_id/codex-trigger" \
    '{"interval_seconds":604800,"max_run_seconds":60,"sandbox_mode":"read_only","approval_policy":"never","network_access":false,"web_search":"disabled"}' \
    -X PUT -H "authorization: Bearer $OWNER_TOKEN" >/dev/null
done

env \
  DATABASE_URL="$DATABASE_URL" \
  AGENT_TRIGGER_MCP_URL="$API_BASE_URL/mcp" \
  AGENT_TRIGGER_CODEX_BIN="$FAKE_CODEX" \
  AGENT_TRIGGER_CODEX_AUTO_INSTALL=false \
  AGENT_TRIGGER_STATE_ROOT="$TEMP_ROOT/trigger-state" \
  AGENT_TRIGGER_MANAGED_PROJECTS_ROOT="$TEMP_ROOT/workspaces" \
  AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS="$TEMP_ROOT/workspaces" \
  AGENT_TRIGGER_BATCH_SIZE=10 \
  AGENT_TRIGGER_RESOURCE_CONCURRENCY_LIMIT=10 \
  AGENT_TRIGGER_MIN_AVAILABLE_MEMORY_MB=64 \
  AGENT_TRIGGER_RUN_ONCE=false \
  RELAY_CHROME_DEVTOOLS_MCP_ENABLED=false \
  HARNESS_MODE=disabled \
  HARNESS_CREDENTIALS_ROOT="$TEMP_ROOT/trigger-state/harness-credentials" \
  RELAY_LOAD_FAKE_CODEX_SECONDS="$FAKE_CODEX_SECONDS" \
  RUST_LOG=info \
  "$ROOT_DIR/target/release/ai-chat-agent-trigger" >"$TEMP_ROOT/trigger.log" 2>&1 &
TRIGGER_PID=$!
trigger_started=false
for attempt in {1..100}; do
  if grep -q 'local Codex Agent Trigger started' "$TEMP_ROOT/trigger.log"; then
    trigger_started=true
    break
  fi
  if ! kill -0 "$TRIGGER_PID" >/dev/null 2>&1; then
    echo "Trigger exited during startup" >&2
    tail -n 120 "$TEMP_ROOT/trigger.log" >&2 || true
    exit 1
  fi
  sleep 0.1
done
if [[ "$trigger_started" != "true" ]]; then
  echo "Timed out waiting for Trigger startup" >&2
  tail -n 120 "$TEMP_ROOT/trigger.log" >&2 || true
  exit 1
fi

echo "Running isolated 1/3/10-Agent control-session load scenarios..."
run_scenario 1
run_scenario 3
run_scenario 10

browser_processes="$(ps -axo command | grep -E '[c]hrome-devtools-mcp|[C]hromium.*relay-trigger-load' | grep -c "$TEMP_ROOT" || true)"
if [[ "$browser_processes" != "0" ]]; then
  echo "Control-only load unexpectedly started managed browser processes" >&2
  exit 1
fi

echo "Trigger load benchmark completed successfully without real model calls."
