#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PG_CONTAINER="${PG_CONTAINER:-ai-chat-postgres}"
SERVER_PORT="${RELAY_LOAD_TEST_PORT:-18089}"
FAKE_CODEX_SECONDS="${RELAY_LOAD_FAKE_CODEX_SECONDS:-1.0}"
LOAD_SCENARIOS="${RELAY_LOAD_TEST_SCENARIOS:-1 3 10}"
IDLE_SECONDS="${RELAY_LOAD_TEST_IDLE_SECONDS:-0}"
RUN_MIXED_SCENARIO="${RELAY_LOAD_TEST_MIXED_SCENARIO:-false}"
LOG_STATEMENTS="${RELAY_LOAD_TEST_LOG_STATEMENTS:-false}"
VERIFY_CANCELLATION="${RELAY_LOAD_TEST_VERIFY_CANCELLATION:-false}"
MCP_PROTOCOL_VERSION="${MCP_PROTOCOL_VERSION:-2025-11-25}"
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
if [[ ! "$FAKE_CODEX_SECONDS" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
  echo "Invalid fake Codex duration: $FAKE_CODEX_SECONDS" >&2
  exit 1
fi
if [[ ! "$IDLE_SECONDS" =~ ^[0-9]+$ ]]; then
  echo "Invalid idle observation duration: $IDLE_SECONDS" >&2
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

for command in awk cargo curl docker git grep jq lsof pgrep ps sed; do
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

mcp_post() {
  local agent_key="$1"
  local payload="$2"
  curl -fsS \
    -H 'content-type: application/json' \
    -H 'accept: application/json, text/event-stream' \
    -H "mcp-protocol-version: $MCP_PROTOCOL_VERSION" \
    -H "x-agent-key: $agent_key" \
    -d "$payload" \
    "$API_BASE_URL/mcp"
}

source "$ROOT_DIR/scripts/lib/trigger_load_fairness.sh"

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

sample_trigger_process() {
  local output_file="$1"
  local pids="$TRIGGER_PID"
  local descendants
  descendants="$(collect_descendants "$TRIGGER_PID")"
  if [[ -n "$descendants" ]]; then
    pids+=" $(tr '\n' ' ' <<<"$descendants")"
  fi
  local csv
  csv="$(tr ' ' ',' <<<"$pids" | sed -E 's/,+/,/g; s/^,//; s/,$//')"
  ps -o %cpu=,rss= -p "$csv" 2>/dev/null | awk '
    { cpu += $1; rss += $2; processes += 1 }
    END { printf "%.2f %d %d\n", cpu + 0, rss + 0, processes + 0 }
  ' >>"$output_file"
}

wait_for_trigger_idle() {
  local stable_samples=0
  local descendants
  for _ in {1..300}; do
    descendants="$(collect_descendants "$TRIGGER_PID")"
    if [[ -z "$descendants" ]]; then
      stable_samples=$((stable_samples + 1))
      if (( stable_samples >= 20 )); then
        return 0
      fi
    else
      stable_samples=0
    fi
    sleep 0.1
  done
  echo "Trigger did not become idle after startup" >&2
  ps -axo pid,ppid,%cpu,rss,etime,command | awk -v pid="$TRIGGER_PID" '$2 == pid {print}' >&2
  return 1
}

run_idle_scenario() {
  local seconds="$1"
  local sample_file="$TEMP_ROOT/idle.samples"
  : >"$sample_file"
  wait_for_trigger_idle
  # Let one-time discovery persistence finish before taking the idle baseline.
  sleep 2
  # Flush transaction counters accumulated while the fixture was being created,
  # then reset only after the long-lived pool connections have released them.
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
  sleep 0.5
  local before_transactions after_transactions
  before_transactions="$(pg_query "
    select xact_commit + xact_rollback
    from pg_stat_database
    where datname = current_database();
  ")"
  local elapsed
  for ((elapsed = 0; elapsed < seconds; elapsed++)); do
    sample_trigger_process "$sample_file"
    sleep 1
  done
  after_transactions="$(pg_query "
    select xact_commit + xact_rollback
    from pg_stat_database
    where datname = current_database();
  ")"

  local resource_metrics average_cpu peak_cpu peak_rss peak_processes samples
  resource_metrics="$(awk '
    { cpu += $1; if ($1 > max_cpu) max_cpu = $1; if ($2 > max_rss) max_rss = $2; if ($3 > max_processes) max_processes = $3; samples += 1 }
    END { printf "%.2f|%.2f|%d|%d|%d", cpu / (samples ? samples : 1), max_cpu + 0, max_rss + 0, max_processes + 0, samples + 0 }
  ' "$sample_file")"
  IFS='|' read -r average_cpu peak_cpu peak_rss peak_processes samples <<<"$resource_metrics"
  local browser_processes
  browser_processes="$(ps -axo command | grep -E '[c]hrome-devtools-mcp|[C]hromium.*relay-trigger-load' | grep -c "$TEMP_ROOT" || true)"
  if (( peak_processes > 1 )); then
    echo "Idle Trigger started an unexpected child process" >&2
    return 1
  fi
  if [[ "$browser_processes" != "0" ]]; then
    echo "Idle Trigger unexpectedly started managed browser processes" >&2
    return 1
  fi
  if ! awk -v cpu="$average_cpu" 'BEGIN { exit !(cpu <= 1.0) }'; then
    echo "Idle Trigger average CPU is too high: ${average_cpu}%" >&2
    return 1
  fi

  jq -cn \
    --argjson idle_seconds "$seconds" \
    --argjson average_cpu_percent "$average_cpu" \
    --argjson peak_cpu_percent "$peak_cpu" \
    --argjson peak_rss_kib "$peak_rss" \
    --argjson peak_processes "$peak_processes" \
    --argjson browser_processes "$browser_processes" \
    --argjson database_transactions "$((after_transactions - before_transactions))" \
    --argjson samples "$samples" \
    '{idle_seconds:$idle_seconds,trigger_tree_cpu_percent:{average:$average_cpu_percent,peak:$peak_cpu_percent},trigger_tree_peak_rss_kib:$peak_rss_kib,trigger_tree_peak_processes:$peak_processes,browser_processes:$browser_processes,database_transactions:$database_transactions,samples:$samples}'
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

run_cancellation_scenario() {
  if ! awk -v seconds="$FAKE_CODEX_SECONDS" 'BEGIN { exit !(seconds >= 2) }'; then
    echo "Cancellation verification requires fake Codex duration >= 2 seconds" >&2
    return 1
  fi
  local agent_id="${AGENT_IDS[0]}"
  local marker pause_requested_at state result cancel_latency_ms
  marker="$(pg_query 'select clock_timestamp();')"
  post_json "/api/v1/companies/$COMPANY_ID/agents/$agent_id/codex-trigger/run-now" \
    '{}' -H "authorization: Bearer $OWNER_TOKEN" >/dev/null
  state=""
  for _ in {1..100}; do
    state="$(pg_query "
      select status
      from agent_codex_trigger_runs
      where agent_profile_id = '$agent_id'
        and started_at >= timestamptz '$marker'
      order by started_at desc
      limit 1;
    ")"
    [[ "$state" == "running" ]] && break
    if [[ -n "$state" ]]; then
      echo "Cancellation fixture became terminal before pause: $state" >&2
      return 1
    fi
    sleep 0.05
  done
  if [[ "$state" != "running" ]]; then
    echo "Cancellation fixture did not start" >&2
    return 1
  fi
  pause_requested_at="$(pg_query 'select clock_timestamp();')"
  post_json "/api/v1/companies/$COMPANY_ID/agents/$agent_id/codex-trigger/pause" \
    '{}' -H "authorization: Bearer $OWNER_TOKEN" >/dev/null
  result=""
  for _ in {1..100}; do
    result="$(pg_query "
      select status || '|' || coalesce(
        round(extract(epoch from (finished_at - timestamptz '$pause_requested_at')) * 1000)::text,
        ''
      )
      from agent_codex_trigger_runs
      where agent_profile_id = '$agent_id'
        and started_at >= timestamptz '$marker'
      order by started_at desc
      limit 1;
    ")"
    [[ "${result%%|*}" != "running" ]] && break
    sleep 0.05
  done
  IFS='|' read -r state cancel_latency_ms <<<"$result"
  if [[ "$state" != "cancelled" ]]; then
    echo "Cancellation fixture ended with unexpected status: ${state:-missing}" >&2
    return 1
  fi
  if [[ ! "$cancel_latency_ms" =~ ^[0-9]+$ ]] || (( cancel_latency_ms > 2000 )); then
    echo "Cancellation took too long: ${cancel_latency_ms:-missing}ms" >&2
    return 1
  fi
  jq -cn \
    --arg status "$state" \
    --argjson cancel_latency_ms "$cancel_latency_ms" \
    '{cancellation_status:$status,cancel_latency_ms:$cancel_latency_ms}'
}

require_mcp_success() {
  local label="$1"
  local response="$2"
  if ! jq -e '.result.isError != true and .result.structuredContent.output != null' \
    >/dev/null <<<"$response"; then
    echo "$label failed: $response" >&2
    return 1
  fi
}

run_mixed_project_browser_scenario() {
  local managed_agents="'${MIXED_AGENT_IDS[0]}','${MIXED_AGENT_IDS[1]}','${MIXED_AGENT_IDS[2]}','${MIXED_AGENT_IDS[3]}','${MIXED_AGENT_IDS[4]}'"
  pg_query "
    update agent_codex_trigger_configs
    set status = 'paused', next_run_at = clock_timestamp() + interval '7 days',
        manual_run_requested_at = null, wake_requested_at = null, wake_reason = null,
        lease_owner = null, lease_expires_at = null
    where agent_profile_id in ($managed_agents);
  " >/dev/null

  local project_response project_id project_root remote_root remote_url
  project_response="$(mcp_post "${MIXED_AGENT_KEYS[0]}" "$(jq -cn \
    --arg company "$MIXED_COMPANY_ID" \
    --arg first "${MIXED_AGENT_IDS[1]}" \
    --arg second "${MIXED_AGENT_IDS[2]}" \
    --arg prerequisite "${MIXED_AGENT_IDS[3]}" \
    --arg waiting "${MIXED_AGENT_IDS[4]}" \
    '{jsonrpc:"2.0",id:100,method:"tools/call",params:{name:"company.project",arguments:{action:"create",company_id:$company,name:"Trigger mixed load fixture",description:"Project worker, browser sharing, waiting task, and backoff verification",member_agent_ids:[$first,$second,$prerequisite,$waiting],idempotency_key:"trigger-mixed-project-v1"}}}')")"
  require_mcp_success "mixed project creation" "$project_response"
  project_id="$(jq -r '.result.structuredContent.output.project.project.id' <<<"$project_response")"
  local project_status_response
  project_status_response="$(mcp_post "${MIXED_AGENT_KEYS[0]}" "$(jq -cn \
    --arg company "$MIXED_COMPANY_ID" --arg project "$project_id" \
    '{jsonrpc:"2.0",id:101,method:"tools/call",params:{name:"company.project",arguments:{action:"status_update",company_id:$company,project_id:$project,summary:"Activate mixed load fixture",progress_percent:0,blockers:[],next_steps:["Run worker load"],project_status:"active",idempotency_key:"trigger-mixed-project-active"}}}')")"
  require_mcp_success "mixed project activation" "$project_status_response"
  project_root="$TEMP_ROOT/workspaces/projects/$project_id"
  remote_root="$TEMP_ROOT/remotes/$project_id.git"
  remote_url="$remote_root"
  mkdir -p "$project_root" "$(dirname "$remote_root")"
  git init --bare --initial-branch=main "$remote_root" >"$TEMP_ROOT/git-fixture.log" 2>&1
  git init --initial-branch=main "$project_root" >>"$TEMP_ROOT/git-fixture.log" 2>&1
  git -C "$project_root" config user.name "Relay Load Fixture"
  git -C "$project_root" config user.email "relay-load@example.com"
  git -C "$project_root" commit --allow-empty -m "Initialize mixed load fixture" \
    >>"$TEMP_ROOT/git-fixture.log" 2>&1
  git -C "$project_root" remote add origin "$remote_url"
  git -C "$project_root" push -u origin main >>"$TEMP_ROOT/git-fixture.log" 2>&1
  pg_query "
    insert into company_project_git_configs (
      project_id, remote_url, default_branch, git_host, host_local_path,
      auth_profile, allow_agent_push, branch_prefix, created_by_agent_id,
      updated_by_agent_id
    ) values (
      '$project_id', '$remote_url', 'main', 'local-fixture', '$project_root',
      null, true, 'relay/', '${MIXED_AGENT_IDS[0]}', '${MIXED_AGENT_IDS[0]}'
    );
  " >/dev/null

  local browser_task_ids=()
  local task_response task_id index
  for index in 0 1 2; do
    task_response="$(mcp_post "${MIXED_AGENT_KEYS[0]}" "$(jq -cn \
      --argjson rpc_id "$((110 + index))" \
      --arg company "$MIXED_COMPANY_ID" \
      --arg project "$project_id" \
      --arg assignee "${MIXED_AGENT_IDS[$index]}" \
      --argjson sequence "$((index + 1))" \
      '{jsonrpc:"2.0",id:$rpc_id,method:"tools/call",params:{name:"company.task",arguments:{action:"create",company_id:$company,project_id:$project,title:("Browser worker "+($sequence|tostring)),description:"Exercise a real project-bound worker with shared browser capability",priority:"high",assignee_agent_id:$assignee,depends_on_task_ids:[],idempotency_key:("trigger-browser-task-"+($sequence|tostring))}}}')")"
    require_mcp_success "browser task creation" "$task_response"
    task_id="$(jq -r '.result.structuredContent.output.task.id' <<<"$task_response")"
    browser_task_ids+=("$task_id")
  done

  local prerequisite_response prerequisite_task_id waiting_response waiting_task_id waiting_wake
  prerequisite_response="$(mcp_post "${MIXED_AGENT_KEYS[0]}" "$(jq -cn \
    --arg company "$MIXED_COMPANY_ID" --arg project "$project_id" \
    --arg assignee "${MIXED_AGENT_IDS[3]}" \
    '{jsonrpc:"2.0",id:120,method:"tools/call",params:{name:"company.task",arguments:{action:"create",company_id:$company,project_id:$project,title:"Waiting prerequisite",description:"Keep the dependent task blocked",priority:"normal",assignee_agent_id:$assignee,depends_on_task_ids:[],idempotency_key:"trigger-waiting-prerequisite"}}}')")"
  require_mcp_success "waiting prerequisite creation" "$prerequisite_response"
  prerequisite_task_id="$(jq -r '.result.structuredContent.output.task.id' <<<"$prerequisite_response")"
  pg_query "
    update agent_codex_trigger_configs
    set next_run_at = clock_timestamp() + interval '7 days',
        manual_run_requested_at = null, wake_requested_at = null, wake_reason = null
    where agent_profile_id = '${MIXED_AGENT_IDS[4]}';
  " >/dev/null
  waiting_response="$(mcp_post "${MIXED_AGENT_KEYS[0]}" "$(jq -cn \
    --arg company "$MIXED_COMPANY_ID" --arg project "$project_id" \
    --arg assignee "${MIXED_AGENT_IDS[4]}" --arg dependency "$prerequisite_task_id" \
    '{jsonrpc:"2.0",id:121,method:"tools/call",params:{name:"company.task",arguments:{action:"create",company_id:$company,project_id:$project,title:"Waiting dependent",description:"Must not wake until its prerequisite is complete",priority:"normal",assignee_agent_id:$assignee,depends_on_task_ids:[$dependency],idempotency_key:"trigger-waiting-dependent"}}}')")"
  require_mcp_success "waiting dependent creation" "$waiting_response"
  waiting_task_id="$(jq -r '.result.structuredContent.output.task.id' <<<"$waiting_response")"
  waiting_wake="$(pg_query "
    select case
      when wake_requested_at is null and next_run_at > clock_timestamp() + interval '1 day'
        then 'quiet'
      else 'woken'
    end
    from agent_codex_trigger_configs
    where agent_profile_id = '${MIXED_AGENT_IDS[4]}';
  ")"
  if [[ "$waiting_wake" != "quiet" ]]; then
    echo "Waiting task unexpectedly woke its assignee" >&2
    return 1
  fi

  local intent_ids=()
  local intent_response intent_id
  for index in 0 1 2; do
    intent_response="$(mcp_post "${MIXED_AGENT_KEYS[$index]}" "$(jq -cn \
      --argjson rpc_id "$((130 + index))" \
      --arg company "$MIXED_COMPANY_ID" --arg project "$project_id" \
      --arg task "${browser_task_ids[$index]}" --argjson sequence "$((index + 1))" \
      '{jsonrpc:"2.0",id:$rpc_id,method:"tools/call",params:{name:"agent.work_session",arguments:{action:"dispatch",company_id:$company,project_id:$project,source_event_ids:[],task_ids:[$task],objective:("Run mixed browser worker "+($sequence|tostring)),acceptance_criteria:["Project session starts","Shared browser page is assigned"],required_capabilities:["browser"],priority:"high",dedupe_key:("trigger-mixed-intent-"+($sequence|tostring))}}}')")"
    require_mcp_success "browser intent dispatch" "$intent_response"
    intent_id="$(jq -r '.result.structuredContent.output.intent.id' <<<"$intent_response")"
    intent_ids+=("$intent_id")
  done

  local browser_agents="'${MIXED_AGENT_IDS[0]}','${MIXED_AGENT_IDS[1]}','${MIXED_AGENT_IDS[2]}'"
  pg_query "
    update agent_codex_trigger_configs
    set status = 'active', next_run_at = clock_timestamp() + interval '7 days',
        manual_run_requested_at = null, wake_requested_at = null, wake_reason = null,
        lease_owner = null, lease_expires_at = null
    where agent_profile_id in ($browser_agents)
       or agent_profile_id = '${MIXED_AGENT_IDS[4]}';
  " >/dev/null

  local marker sample_file
  marker="$(pg_query 'select clock_timestamp();')"
  sample_file="$TEMP_ROOT/mixed.samples"
  : >"$sample_file"
  for index in 0 1 2; do
    post_json "/api/v1/companies/$MIXED_COMPANY_ID/agents/${MIXED_AGENT_IDS[$index]}/codex-trigger/run-now" \
      '{}' -H "authorization: Bearer $OWNER_TOKEN" >/dev/null
  done
  wait_for_runs "$marker" 3 "$sample_file"
  sleep 0.5

  local project_runs pending_intents backing_off_runs waiting_runs
  project_runs="$(pg_query "
    select count(*) from agent_codex_trigger_runs
    where started_at >= timestamptz '$marker' and project_id = '$project_id'
      and session_kind = 'project' and current_intent_id is not null;
  ")"
  pending_intents="$(pg_query "
    select count(*) from agent_execution_intents
    where id in ('${intent_ids[0]}','${intent_ids[1]}','${intent_ids[2]}')
      and status = 'pending';
  ")"
  backing_off_runs="$(pg_query "
    select count(*) from agent_codex_trigger_runs
    where started_at >= timestamptz '$marker' and project_id = '$project_id'
      and activity_phase = 'backing_off';
  ")"
  waiting_runs="$(pg_query "
    select count(*) from agent_codex_trigger_runs
    where started_at >= timestamptz '$marker'
      and agent_profile_id = '${MIXED_AGENT_IDS[4]}';
  ")"
  if [[ "$project_runs" != "3" || "$pending_intents" != "3" || "$backing_off_runs" != "3" ]]; then
    echo "Mixed project execution did not preserve unfinished work: project_runs=$project_runs pending_intents=$pending_intents backing_off_runs=$backing_off_runs" >&2
    pg_query "
      select 'task', id, status, assignee_agent_id from company_project_tasks
      where id in ('${browser_task_ids[0]}','${browser_task_ids[1]}','${browser_task_ids[2]}')
      union all
      select 'intent', id, status, agent_profile_id from agent_execution_intents
      where id in ('${intent_ids[0]}','${intent_ids[1]}','${intent_ids[2]}');
    " >&2 || true
    pg_query "
      select id, status, activity_phase, coalesce(state_reason, ''),
             coalesce(error_message, '')
      from agent_codex_trigger_runs
      where started_at >= timestamptz '$marker' and project_id = '$project_id'
      order by started_at;
    " >&2 || true
    return 1
  fi
  if [[ "$waiting_runs" != "0" ]]; then
    echo "Waiting-only Agent unexpectedly started $waiting_runs runs" >&2
    return 1
  fi

  local endpoint_file="$TEMP_ROOT/trigger-state/browser-profiles/shared/native/.relay-devtools-endpoint"
  for _ in {1..100}; do
    [[ -s "$endpoint_file" ]] && break
    sleep 0.1
  done
  [[ -s "$endpoint_file" ]] || {
    echo "Shared browser endpoint was not created" >&2
    return 1
  }
  local endpoint pages initial_pages pruned_pages missing_agent=""
  endpoint="$(sed -n '1p' "$endpoint_file")"
  pages="$(curl -fsS "$endpoint/json/list")"
  initial_pages="$(jq '[.[] | select(.type == "page" and (.url | startswith("about:blank#relay-agent-")))] | length' <<<"$pages")"
  if [[ "$initial_pages" != "3" ]]; then
    echo "Expected three Agent browser pages, found $initial_pages" >&2
    return 1
  fi

  pruned_pages="$initial_pages"
  for _ in {1..75}; do
    pages="$(curl -fsS "$endpoint/json/list")"
    pruned_pages="$(jq '[.[] | select(.type == "page" and (.url | startswith("about:blank#relay-agent-")))] | length' <<<"$pages")"
    (( pruned_pages <= 2 )) && break
    sleep 1
  done
  if [[ "$pruned_pages" != "2" ]]; then
    echo "Shared browser LRU did not reduce three idle Agent pages to two" >&2
    return 1
  fi
  for index in 0 1 2; do
    if ! jq -e --arg url "about:blank#relay-agent-${MIXED_AGENT_IDS[$index]}" \
      '[.[] | select(.type == "page") | .url] | index($url) != null' >/dev/null <<<"$pages"; then
      missing_agent="${MIXED_AGENT_IDS[$index]}"
      break
    fi
  done
  [[ -n "$missing_agent" ]] || {
    echo "Could not identify the LRU-reclaimed Agent page" >&2
    return 1
  }

  local rerun_marker="$marker" rerun_sample="$TEMP_ROOT/mixed-rerun.samples"
  rerun_marker="$(pg_query 'select clock_timestamp();')"
  : >"$rerun_sample"
  post_json "/api/v1/companies/$MIXED_COMPANY_ID/agents/$missing_agent/codex-trigger/run-now" \
    '{}' -H "authorization: Bearer $OWNER_TOKEN" >/dev/null
  wait_for_runs "$rerun_marker" 1 "$rerun_sample"
  pages="$(curl -fsS "$endpoint/json/list")"
  if ! jq -e --arg url "about:blank#relay-agent-$missing_agent" \
    '[.[] | select(.type == "page") | .url] | index($url) != null' >/dev/null <<<"$pages"; then
    echo "LRU-reclaimed Agent page was not recreated on demand" >&2
    return 1
  fi
  local second_backoff_seconds
  second_backoff_seconds="$(pg_query "
    select floor(extract(epoch from (next_run_at - clock_timestamp())))::int
    from agent_codex_trigger_configs where agent_profile_id = '$missing_agent';
  ")"
  if (( second_backoff_seconds < 100 )); then
    echo "Repeated no-progress work did not advance to the two-minute backoff" >&2
    return 1
  fi

  jq -cn \
    --arg project_id "$project_id" \
    --arg waiting_task_id "$waiting_task_id" \
    --arg recreated_agent_id "$missing_agent" \
    --argjson project_runs "$project_runs" \
    --argjson pending_intents "$pending_intents" \
    --argjson backing_off_runs "$backing_off_runs" \
    --argjson initial_browser_pages "$initial_pages" \
    --argjson pruned_browser_pages "$pruned_pages" \
    --argjson second_backoff_seconds "$second_backoff_seconds" \
    '{project_id:$project_id,project_runs:$project_runs,pending_intents:$pending_intents,backing_off_runs:$backing_off_runs,waiting_task_id:$waiting_task_id,waiting_agent_runs:0,shared_browser_pages:{before_lru:$initial_browser_pages,after_lru:$pruned_browser_pages},recreated_agent_id:$recreated_agent_id,second_backoff_seconds:$second_backoff_seconds}'
}

echo "Building release Server and Trigger..."
if [[ "${RELAY_LOAD_TEST_SKIP_BUILD:-false}" != "true" ]]; then
  cargo build --release -p ai-chat-server -p ai-chat-agent-trigger \
    --manifest-path "$ROOT_DIR/Cargo.toml" >"$TEMP_ROOT/build.log" 2>&1
fi

FAKE_CODEX="$TEMP_ROOT/fake-codex.sh"
printf '%s\n' "$FAKE_CODEX_SECONDS" >"$TEMP_ROOT/fake-codex-seconds"
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
if [[ " $* " == *" app-server "* ]]; then
  IFS= read -r initialize
  printf '%s\n' '{"id":0,"result":{"userAgent":"load-fixture","platformFamily":"unix","platformOs":"test","codexHome":"/tmp"}}'
  IFS= read -r initialized
  IFS= read -r thread_request
  thread_id="$(printf '%s' "$thread_request" | jq -r '.params.threadId // empty')"
  if [[ -z "$thread_id" ]]; then
    thread_id="load-app-thread-$$"
  fi
  printf '{"id":1,"result":{"thread":{"id":"%s"},"model":"load-fixture","modelProvider":"load-fixture","cwd":"/tmp","approvalPolicy":"never","approvalsReviewer":"user","sandbox":{"type":"workspaceWrite","writableRoots":[],"readOnlyAccess":{"type":"fullAccess"},"networkAccess":false,"excludeTmpdirEnvVar":false,"excludeSlashTmp":false}}}\n' "$thread_id"
  IFS= read -r turn_request
  turn_id="load-turn-$$"
  printf '{"id":2,"result":{"turn":{"id":"%s","items":[],"status":"inProgress"}}}\n' "$turn_id"
  printf '{"method":"turn/started","params":{"threadId":"%s","turn":{"id":"%s","items":[],"status":"inProgress"}}}\n' "$thread_id" "$turn_id"
  sleep "$(sed -n '1p' "$(dirname "$0")/fake-codex-seconds")"
  printf '{"method":"item/completed","params":{"threadId":"%s","turnId":"%s","item":{"id":"message-1","type":"agentMessage","text":"load fixture completed"}}}\n' "$thread_id" "$turn_id"
  printf '{"method":"turn/completed","params":{"threadId":"%s","turn":{"id":"%s","items":[{"id":"message-1","type":"agentMessage","text":"load fixture completed"}],"status":"completed"}}}\n' "$thread_id" "$turn_id"
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
sleep "$(sed -n '1p' "$(dirname "$0")/fake-codex-seconds")"
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
if [[ "$LOG_STATEMENTS" == "true" ]]; then
  docker exec "$PG_CONTAINER" psql -U postgres -d postgres -v ON_ERROR_STOP=1 \
    -c "ALTER DATABASE \"$DATABASE_NAME\" SET log_statement = 'all';" >/dev/null
fi

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
OWNER_EMAIL="load-$suffix@example.com"
owner="$(post_json /api/v1/auth/register "$(jq -cn --arg email "$OWNER_EMAIL" '{email:$email,display_name:"Load Owner",password:"load-password-123"}')")"
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

fair_company="$(post_json /api/v1/companies "$(jq -cn --arg suffix "$suffix" '{name:("Fair Load Company "+$suffix),slug:("fair-load-company-"+$suffix)}')" -H "authorization: Bearer $OWNER_TOKEN")"
FAIR_COMPANY_ID="$(jq -r '.company_console.company.id' <<<"$fair_company")"
fair_org_id="$(jq -r '.company_console.org_units[0].id' <<<"$fair_company")"
FAIR_AGENT_IDS=()
fair_manager_membership_id=""
for index in {1..2}; do
  if (( index == 1 )); then
    fair_payload="$(jq -cn --arg org "$fair_org_id" '{display_name:"Fair Load Manager",persona:"Fairness fixture manager",org_unit_id:$org,profession_key:"project_manager",role_key:"company_manager"}')"
  else
    fair_payload="$(jq -cn --arg org "$fair_org_id" --arg manager "$fair_manager_membership_id" '{display_name:"Fair Load Engineer",persona:"Fairness fixture worker",org_unit_id:$org,profession_key:"software_engineer",role_key:"member",reports_to_membership_id:$manager}')"
  fi
  fair_agent="$(post_json "/api/v1/companies/$FAIR_COMPANY_ID/agents" "$fair_payload" -H "authorization: Bearer $OWNER_TOKEN")"
  fair_agent_id="$(jq -r '.result.agent_profile.id' <<<"$fair_agent")"
  FAIR_AGENT_IDS+=("$fair_agent_id")
  if (( index == 1 )); then
    fair_manager_membership_id="$(jq -r '.result.membership.id' <<<"$fair_agent")"
  fi
  post_json "/api/v1/companies/$FAIR_COMPANY_ID/agents/$fair_agent_id/codex-trigger" \
    '{"interval_seconds":604800,"max_run_seconds":60,"sandbox_mode":"read_only","approval_policy":"never","network_access":false,"web_search":"disabled"}' \
    -X PUT -H "authorization: Bearer $OWNER_TOKEN" >/dev/null
done

MIXED_AGENT_IDS=()
MIXED_AGENT_KEYS=()
MIXED_COMPANY_ID=""
for index in {1..5}; do
  bootstrap="$(post_json /api/v1/dev/bootstrap "$(jq -cn \
    --arg email "$OWNER_EMAIL" --arg suffix "$suffix" --argjson index "$index" \
    '{email:$email,display_name:"Load Owner",desired_handle:("mixed-agent-"+($index|tostring)+"-"+$suffix),desired_agent_name:("Mixed Agent "+($index|tostring)),persona:"Mixed project and browser load fixture"}')")"
  mixed_agent_id="$(jq -r '.agent_profile.id' <<<"$bootstrap")"
  mixed_agent_key="$(jq -r '.agent_key' <<<"$bootstrap")"
  mixed_company_id="$(jq -r '.company.id' <<<"$bootstrap")"
  if [[ -z "$MIXED_COMPANY_ID" ]]; then
    MIXED_COMPANY_ID="$mixed_company_id"
  elif [[ "$MIXED_COMPANY_ID" != "$mixed_company_id" ]]; then
    echo "Development bootstrap placed mixed Agents in different companies" >&2
    exit 1
  fi
  MIXED_AGENT_IDS+=("$mixed_agent_id")
  MIXED_AGENT_KEYS+=("$mixed_agent_key")
  profession_key="software_engineer"
  if (( index == 1 )); then
    profession_key="project_manager"
    post_json "/api/v1/companies/$MIXED_COMPANY_ID/agents/$mixed_agent_id/role" \
      '{"role_key":"company_manager","reason":"mixed load fixture manager"}' \
      -H "authorization: Bearer $OWNER_TOKEN" >/dev/null
  fi
  post_json "/api/v1/companies/$MIXED_COMPANY_ID/agents/$mixed_agent_id/profession" \
    "$(jq -cn --arg profession "$profession_key" '{profession_key:$profession,reason:"mixed load fixture"}')" \
    -H "authorization: Bearer $OWNER_TOKEN" >/dev/null
  post_json "/api/v1/companies/$MIXED_COMPANY_ID/agents/$mixed_agent_id/codex-trigger" \
    '{"interval_seconds":604800,"max_run_seconds":60,"sandbox_mode":"read_only","approval_policy":"never","network_access":false,"web_search":"disabled"}' \
    -X PUT -H "authorization: Bearer $OWNER_TOKEN" >/dev/null
done

# Trigger configurations are intentionally created as immediately due. Keep the
# fixture agents dormant until each scenario explicitly requests a manual run,
# otherwise startup work from all ten agents contaminates the 1/3-Agent samples.
pg_query "
  update agent_codex_trigger_configs
  set next_run_at = clock_timestamp() + interval '7 days',
      manual_run_requested_at = null,
      wake_requested_at = null,
      wake_reason = null,
      lease_owner = null,
      lease_expires_at = null;
" >/dev/null

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
  RELAY_CHROME_DEVTOOLS_MCP_ENABLED="$RUN_MIXED_SCENARIO" \
  RELAY_CHROME_DEVTOOLS_MCP_MODE=host \
  RELAY_CHROME_MAX_IDLE_PAGES=2 \
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

echo "Running isolated Agent control-session load scenarios: $LOAD_SCENARIOS"
if (( IDLE_SECONDS > 0 )); then
  echo "Observing an idle Trigger for ${IDLE_SECONDS}s"
  run_idle_scenario "$IDLE_SECONDS"
fi
for scenario in $LOAD_SCENARIOS; do
  if [[ ! "$scenario" =~ ^(1|3|10)$ ]]; then
    echo "Unsupported load scenario: $scenario (allowed: 1, 3, 10)" >&2
    exit 1
  fi
  run_scenario "$scenario"
done
echo "Running cross-company fairness scenario"
run_company_fairness_scenario
if [[ "$VERIFY_CANCELLATION" == "true" ]]; then
  run_cancellation_scenario
fi

browser_processes="$(ps -axo command | grep -E '[c]hrome-devtools-mcp|[C]hromium.*relay-trigger-load' | grep -c "$TEMP_ROOT" || true)"
if [[ "$browser_processes" != "0" ]]; then
  echo "Control-only load unexpectedly started managed browser processes" >&2
  exit 1
fi
if [[ "$RUN_MIXED_SCENARIO" == "true" ]]; then
  echo "Running project-worker and shared-browser mixed scenario"
  run_mixed_project_browser_scenario
fi

echo "Trigger load benchmark completed successfully without real model calls."
