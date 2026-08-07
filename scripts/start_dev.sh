#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-up}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCAL_ENV_FILE="$ROOT_DIR/.env.local"
if [[ -f "$LOCAL_ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$LOCAL_ENV_FILE"
  set +a
fi
PID_DIR="${PID_DIR:-/tmp/ai_chat_dev}"
API_LOG="$PID_DIR/api.log"
WEB_LOG="$PID_DIR/web.log"
WATCH_LOG="$PID_DIR/watch.log"
TRIGGER_LOG="$PID_DIR/trigger.log"
STATE_FILE="$PID_DIR/dev.env"
WATCH_INTERVAL_SECONDS="${WATCH_INTERVAL_SECONDS:-2}"
CURRENT_STEP="initializing"
PORT_NOTES=()

mkdir -p "$PID_DIR"
touch "$API_LOG" "$WEB_LOG" "$WATCH_LOG" "$TRIGGER_LOG"

if ! command -v docker >/dev/null 2>&1; then
  echo "Missing required command: docker" >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "Missing required command: cargo" >&2
  exit 1
fi

if ! command -v pnpm >/dev/null 2>&1; then
  echo "Missing required command: pnpm" >&2
  exit 1
fi

if ! command -v node >/dev/null 2>&1; then
  echo "Missing required command: node" >&2
  exit 1
fi

compose() {
  docker compose -f "$ROOT_DIR/docker-compose.yml" --profile harness-self-hosted "$@"
}

existing_container_port() {
  local container_name="$1"
  local container_port="$2"
  docker port "$container_name" "$container_port" 2>/dev/null \
    | head -n 1 \
    | awk -F: '{print $NF}' \
    || true
}

set_step() {
  CURRENT_STEP="$1"
}

stop_docker_app_services() {
  echo "Stopping docker app service (server) to avoid stale web and API conflicts..."
  compose stop server >/dev/null 2>&1 || true
}

stop_local_dev_processes() {
  stop_pid trigger
  stop_pid watcher
  stop_pid web
  stop_pid api
}

port_is_in_use() {
  local port="$1"
  lsof -iTCP:"$port" -sTCP:LISTEN -n -P >/dev/null 2>&1
}

choose_port() {
  local preferred="$1"
  local candidate="$preferred"

  while port_is_in_use "$candidate"; do
    candidate=$((candidate + 1))
  done

  printf '%s\n' "$candidate"
}

write_pid() {
  local name="$1"
  local pid="$2"
  printf '%s\n' "$pid" > "$PID_DIR/$name.pid"
}

launch_label() {
  local name="$1"
  printf 'com.relay.ai-chat.dev.%s\n' "$name"
}

stop_launch_job() {
  local name="$1"

  if [[ "$(uname -s)" == "Darwin" ]]; then
    launchctl remove "$(launch_label "$name")" >/dev/null 2>&1 || true
  fi
}

start_detached_service() {
  local name="$1"
  local log_file="$2"
  local work_dir="$3"
  local pid_file="$PID_DIR/$name.pid"
  local attempt pid
  shift 3

  rm -f "$pid_file"

  if [[ "$(uname -s)" == "Darwin" ]]; then
    launchctl submit \
      -l "$(launch_label "$name")" \
      -o "$log_file" \
      -e "$log_file" \
      -- "$ROOT_DIR/scripts/run_dev_service.sh" "$pid_file" "$work_dir" "$@"
  else
    nohup "$ROOT_DIR/scripts/run_dev_service.sh" \
      "$pid_file" "$work_dir" "$@" </dev/null >>"$log_file" 2>&1 &
  fi

  attempt=1
  while (( attempt <= 50 )); do
    pid="$(read_pid "$name")"
    if is_pid_running "$pid"; then
      return 0
    fi
    sleep 0.1
    attempt=$((attempt + 1))
  done

  echo "Detached service failed to start: $name" >&2
  return 1
}

read_pid() {
  local name="$1"
  local file="$PID_DIR/$name.pid"
  if [[ -f "$file" ]]; then
    cat "$file"
  fi
  return 0
}

load_dev_env() {
  if [[ -f "$STATE_FILE" ]]; then
    # shellcheck disable=SC1090
    source "$STATE_FILE"
  fi
}

is_pid_running() {
  local pid="$1"
  [[ -n "$pid" ]] && kill -0 "$pid" >/dev/null 2>&1
}

stop_pid() {
  local name="$1"
  local pid
  pid="$(read_pid "$name")"

  stop_launch_job "$name"

  if is_pid_running "$pid"; then
    kill "$pid" >/dev/null 2>&1 || true
    sleep 1
    if is_pid_running "$pid"; then
      kill -9 "$pid" >/dev/null 2>&1 || true
    fi
  fi

  rm -f "$PID_DIR/$name.pid"
}

stop_port_listener() {
  local port="$1"
  local pid

  while IFS= read -r pid; do
    [[ -n "$pid" ]] || continue
    kill "$pid" >/dev/null 2>&1 || true
    sleep 1
    if kill -0 "$pid" >/dev/null 2>&1; then
      kill -9 "$pid" >/dev/null 2>&1 || true
    fi
  done < <(lsof -tiTCP:"$port" -sTCP:LISTEN -n -P 2>/dev/null || true)
}

file_signature() {
  local file="$1"

  if stat -f '%m %N' "$file" >/dev/null 2>&1; then
    stat -f '%m %N' "$file"
  else
    stat -c '%Y %n' "$file"
  fi
}

persist_dev_env() {
  cat >"$STATE_FILE" <<EOF
POSTGRES_HOST_PORT=${POSTGRES_HOST_PORT}
API_HOST_PORT=${API_HOST_PORT}
WEB_HOST_PORT=${WEB_HOST_PORT}
DATABASE_URL=${DATABASE_URL}
ENABLE_DEV_ENDPOINTS=${ENABLE_DEV_ENDPOINTS}
HARNESS_MODE=${HARNESS_MODE}
HARNESS_BASE_URL=${HARNESS_BASE_URL:-}
HARNESS_PUBLIC_BASE_URL=${HARNESS_PUBLIC_BASE_URL:-}
HARNESS_HOST_PORT=${HARNESS_HOST_PORT:-}
HARNESS_SSH_PORT=${HARNESS_SSH_PORT:-}
AGENT_TRIGGER_MCP_URL=${AGENT_TRIGGER_MCP_URL}
AGENT_TRIGGER_MANAGED_PROJECTS_ROOT=${AGENT_TRIGGER_MANAGED_PROJECTS_ROOT}
AGENT_TRIGGER_CODEX_AUTO_COMPACT_TOKEN_LIMIT=${AGENT_TRIGGER_CODEX_AUTO_COMPACT_TOKEN_LIMIT:-200000}
EOF
}

record_port_shift() {
  local service_name="$1"
  local preferred_port="$2"
  local actual_port="$3"

  if [[ "$preferred_port" != "$actual_port" ]]; then
    PORT_NOTES+=("${service_name} port ${preferred_port} was busy, switched to ${actual_port}")
  fi
}

collect_api_watch_files() {
  printf '%s\n' \
    "$ROOT_DIR/Cargo.toml" \
    "$ROOT_DIR/Cargo.lock"
  find \
    "$ROOT_DIR/apps/server" \
    "$ROOT_DIR/crates" \
    -type f \
    \( -name '*.rs' -o -name 'Cargo.toml' \) \
    -print
}

compute_api_fingerprint() {
  while IFS= read -r file; do
    [[ -f "$file" ]] || continue
    file_signature "$file"
  done < <(collect_api_watch_files | sort) | shasum | awk '{print $1}'
}

wait_for_http() {
  local url="$1"
  local retries="${2:-60}"
  local attempt=1

  while (( attempt <= retries )); do
    if curl -fsS "$url" >/dev/null 2>&1; then
      return 0
    fi

    sleep 1
    attempt=$((attempt + 1))
  done

  echo "Service did not become ready in time: $url" >&2
  return 1
}

wait_for_postgres() {
  local retries="${1:-60}"
  local attempt=1

  while (( attempt <= retries )); do
    if docker exec "$PG_CONTAINER" pg_isready -U postgres -d postgres >/dev/null 2>&1; then
      return 0
    fi

    sleep 1
    attempt=$((attempt + 1))
  done

  echo "PostgreSQL did not become ready in time: ${PG_CONTAINER}" >&2
  docker logs --tail 40 "$PG_CONTAINER" >&2 || true
  return 1
}

print_recent_log() {
  local label="$1"
  local file="$2"

  if [[ -s "$file" ]]; then
    echo
    echo "Recent ${label} log:"
    tail -n 20 "$file" || true
  fi
}

print_failure_help() {
  local exit_code="$1"
  local line_no="$2"

  trap - ERR
  set +e
  load_dev_env

  echo >&2
  echo "AI Chat local dev failed." >&2
  echo "Step: ${CURRENT_STEP}" >&2
  echo "Line: ${line_no}" >&2
  echo "Exit code: ${exit_code}" >&2

  if [[ -n "${WEB_HOST_PORT:-}" ]]; then
    echo "Expected Web URL: http://127.0.0.1:${WEB_HOST_PORT}" >&2
  fi

  if [[ -n "${API_HOST_PORT:-}" ]]; then
    echo "Expected API URL: http://127.0.0.1:${API_HOST_PORT}" >&2
  fi

  echo "Logs:" >&2
  echo "- API: ${API_LOG}" >&2
  echo "- Web: ${WEB_LOG}" >&2
  echo "- Trigger: ${TRIGGER_LOG}" >&2
  echo "- Watcher: ${WATCH_LOG}" >&2

  if ! docker info >/dev/null 2>&1; then
    echo >&2
    echo "Docker daemon is not reachable. Start Docker Desktop first, then retry." >&2
  fi

  print_recent_log "API" "$API_LOG" >&2
  print_recent_log "Web" "$WEB_LOG" >&2
  print_recent_log "Trigger" "$TRIGGER_LOG" >&2

  echo >&2
  echo "Quick checks:" >&2
  echo "- Re-run: ./scripts/start_dev.sh up" >&2
  echo "- View logs: ./scripts/start_dev.sh logs" >&2
  echo "- Diagnose: ./scripts/start_dev.sh doctor" >&2
  exit "$exit_code"
}

trap 'print_failure_help $? $LINENO' ERR

prepare_env() {
  local preferred_postgres_port="${POSTGRES_HOST_PORT:-15533}"
  local preferred_api_port="${API_HOST_PORT:-48181}"
  local preferred_web_port="${WEB_HOST_PORT:-15274}"
  local existing_harness_port existing_harness_ssh_port

  export POSTGRES_HOST_PORT="$preferred_postgres_port"
  export API_HOST_PORT="$preferred_api_port"
  export WEB_HOST_PORT="$preferred_web_port"

  if port_is_in_use "$API_HOST_PORT"; then
    API_HOST_PORT="$(choose_port "$API_HOST_PORT")"
    export API_HOST_PORT
  fi

  if port_is_in_use "$WEB_HOST_PORT"; then
    WEB_HOST_PORT="$(choose_port "$WEB_HOST_PORT")"
    export WEB_HOST_PORT
  fi

  export DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@127.0.0.1:${POSTGRES_HOST_PORT}/ai_chat}"
  export PG_CONTAINER="${PG_CONTAINER:-ai-chat-postgres}"
  export ENABLE_DEV_ENDPOINTS="${ENABLE_DEV_ENDPOINTS:-true}"
  export HARNESS_MODE="${HARNESS_MODE:-self_hosted}"
  HARNESS_MODE="${HARNESS_MODE//-/_}"
  export HARNESS_MODE
  case "$HARNESS_MODE" in
    self_hosted)
      existing_harness_port="$(existing_container_port ai-chat-harness 3000/tcp)"
      existing_harness_ssh_port="$(existing_container_port ai-chat-harness 3022/tcp)"
      export HARNESS_HOST_PORT="${HARNESS_HOST_PORT:-${existing_harness_port:-13101}}"
      export HARNESS_SSH_PORT="${HARNESS_SSH_PORT:-${existing_harness_ssh_port:-13123}}"
      if [[ -z "$existing_harness_port" ]] && port_is_in_use "$HARNESS_HOST_PORT"; then
        HARNESS_HOST_PORT="$(choose_port "$HARNESS_HOST_PORT")"
        export HARNESS_HOST_PORT
      fi
      if [[ -z "$existing_harness_ssh_port" ]] && port_is_in_use "$HARNESS_SSH_PORT"; then
        HARNESS_SSH_PORT="$(choose_port "$HARNESS_SSH_PORT")"
        export HARNESS_SSH_PORT
      fi
      export HARNESS_BASE_URL="${HARNESS_BASE_URL:-http://127.0.0.1:${HARNESS_HOST_PORT}}"
      export HARNESS_PUBLIC_BASE_URL="${HARNESS_PUBLIC_BASE_URL:-$HARNESS_BASE_URL}"
      export HARNESS_GIT_BASE_URL="${HARNESS_GIT_BASE_URL:-${HARNESS_PUBLIC_BASE_URL}/git}"
      ;;
    official)
      if [[ -z "${HARNESS_BASE_URL:-}" ]]; then
        echo "HARNESS_BASE_URL is required when HARNESS_MODE=official" >&2
        return 1
      fi
      export HARNESS_PUBLIC_BASE_URL="${HARNESS_PUBLIC_BASE_URL:-$HARNESS_BASE_URL}"
      ;;
    disabled)
      ;;
    *)
      echo "Unsupported HARNESS_MODE: $HARNESS_MODE (use official, self_hosted, or disabled)" >&2
      return 1
      ;;
  esac
  export ADMIN_API_TOKEN="${ADMIN_API_TOKEN:-dev-admin-token}"
  export API_ALLOWED_ORIGINS="${API_ALLOWED_ORIGINS:-http://127.0.0.1:${WEB_HOST_PORT},http://localhost:${WEB_HOST_PORT}}"
  export AGENT_TRIGGER_MCP_URL="${AGENT_TRIGGER_MCP_URL:-http://127.0.0.1:${API_HOST_PORT}/mcp}"
  export AGENT_TRIGGER_MANAGED_PROJECTS_ROOT="${AGENT_TRIGGER_MANAGED_PROJECTS_ROOT:-${ROOT_DIR}/.relay-managed-projects}"
  record_port_shift "API" "$preferred_api_port" "$API_HOST_PORT"
  record_port_shift "Web" "$preferred_web_port" "$WEB_HOST_PORT"
  persist_dev_env
}

start_dependencies() {
  set_step "starting docker dependencies"
  if [[ "$HARNESS_MODE" == "self_hosted" ]]; then
    compose up -d postgres harness
  else
    compose up -d postgres
    compose stop harness >/dev/null 2>&1 || true
  fi
  set_step "waiting for postgres readiness"
  wait_for_postgres
  if [[ "$HARNESS_MODE" != "disabled" ]]; then
    set_step "waiting for harness readiness"
    wait_for_http "${HARNESS_BASE_URL}/api/v1/system/health" 90
  fi
  set_step "running postgres migrations"
  bash "$ROOT_DIR/scripts/run_pg_migrations.sh" ensure
}

start_api() {
  set_step "starting rust api service"
  stop_pid api
  stop_port_listener "$API_HOST_PORT"
  (
    cd "$ROOT_DIR"
    cargo build -p ai-chat-server >>"$API_LOG" 2>&1
  )
  start_detached_service api "$API_LOG" "$ROOT_DIR" \
    /usr/bin/env \
    PATH="$PATH" \
    API_HOST=0.0.0.0 \
    API_PORT="$API_HOST_PORT" \
    DATABASE_URL="$DATABASE_URL" \
    ENABLE_DEV_ENDPOINTS="$ENABLE_DEV_ENDPOINTS" \
    ADMIN_API_TOKEN="$ADMIN_API_TOKEN" \
    API_ALLOWED_ORIGINS="$API_ALLOWED_ORIGINS" \
    HARNESS_MODE="$HARNESS_MODE" \
    HARNESS_BASE_URL="${HARNESS_BASE_URL:-}" \
    HARNESS_PUBLIC_BASE_URL="${HARNESS_PUBLIC_BASE_URL:-}" \
    HARNESS_SPACE_PREFIX="${HARNESS_SPACE_PREFIX:-u-}" \
    HARNESS_REQUEST_TIMEOUT_SECONDS="${HARNESS_REQUEST_TIMEOUT_SECONDS:-15}" \
    HARNESS_CREDENTIALS_ROOT="${HARNESS_CREDENTIALS_ROOT:-.relay/harness-credentials}" \
    HARNESS_ADMIN_EMAIL="${HARNESS_ADMIN_EMAIL:-admin@relay.local}" \
    HARNESS_ADMIN_PASSWORD="${HARNESS_ADMIN_PASSWORD:-change-me-harness-admin}" \
    AGENT_TRIGGER_MANAGED_PROJECTS_ROOT="$AGENT_TRIGGER_MANAGED_PROJECTS_ROOT" \
    APP_ENV=development \
    "$ROOT_DIR/target/debug/ai-chat-server"
  wait_for_http "http://127.0.0.1:${API_HOST_PORT}/ready"
}

run_api_watcher() {
  cd "$ROOT_DIR"
  local previous current
  previous="$(compute_api_fingerprint)"
  echo "[watch] watching Rust API sources" >>"$WATCH_LOG"

  while true; do
    sleep "$WATCH_INTERVAL_SECONDS"
    current="$(compute_api_fingerprint)"

    if [[ "$current" != "$previous" ]]; then
      echo "[watch] change detected, restarting ai-chat-server at $(date '+%Y-%m-%d %H:%M:%S')" >>"$WATCH_LOG"
      if start_api >>"$WATCH_LOG" 2>&1; then
        previous="$current"
        echo "[watch] ai-chat-server restarted successfully" >>"$WATCH_LOG"
      else
        echo "[watch] ai-chat-server restart failed, keeping watcher alive" >>"$WATCH_LOG"
      fi
    fi
  done
}

start_api_watcher() {
  set_step "starting rust api watcher"
  stop_pid watcher
  : >"$WATCH_LOG"
  start_detached_service watcher "$WATCH_LOG" "$ROOT_DIR" \
    /usr/bin/env \
    PATH="$PATH" \
    PID_DIR="$PID_DIR" \
    WATCH_INTERVAL_SECONDS="$WATCH_INTERVAL_SECONDS" \
    POSTGRES_HOST_PORT="$POSTGRES_HOST_PORT" \
    API_HOST_PORT="$API_HOST_PORT" \
    WEB_HOST_PORT="$WEB_HOST_PORT" \
    DATABASE_URL="$DATABASE_URL" \
    ENABLE_DEV_ENDPOINTS="$ENABLE_DEV_ENDPOINTS" \
    ADMIN_API_TOKEN="$ADMIN_API_TOKEN" \
    API_ALLOWED_ORIGINS="$API_ALLOWED_ORIGINS" \
    HARNESS_MODE="$HARNESS_MODE" \
    HARNESS_BASE_URL="${HARNESS_BASE_URL:-}" \
    HARNESS_PUBLIC_BASE_URL="${HARNESS_PUBLIC_BASE_URL:-}" \
    HARNESS_SPACE_PREFIX="${HARNESS_SPACE_PREFIX:-u-}" \
    HARNESS_REQUEST_TIMEOUT_SECONDS="${HARNESS_REQUEST_TIMEOUT_SECONDS:-15}" \
    HARNESS_CREDENTIALS_ROOT="${HARNESS_CREDENTIALS_ROOT:-.relay/harness-credentials}" \
    HARNESS_ADMIN_EMAIL="${HARNESS_ADMIN_EMAIL:-admin@relay.local}" \
    HARNESS_ADMIN_PASSWORD="${HARNESS_ADMIN_PASSWORD:-change-me-harness-admin}" \
    AGENT_TRIGGER_MCP_URL="$AGENT_TRIGGER_MCP_URL" \
    AGENT_TRIGGER_MANAGED_PROJECTS_ROOT="$AGENT_TRIGGER_MANAGED_PROJECTS_ROOT" \
    "$ROOT_DIR/scripts/start_dev.sh" watch-api
}

start_trigger() {
  set_step "starting local Codex Agent Trigger"
  stop_pid trigger
  (
    cd "$ROOT_DIR"
    cargo build -p ai-chat-agent-trigger >>"$TRIGGER_LOG" 2>&1
  )
  start_detached_service trigger "$TRIGGER_LOG" "$ROOT_DIR" \
    /usr/bin/env \
    PATH="$PATH" \
    DATABASE_URL="$DATABASE_URL" \
    AGENT_TRIGGER_MCP_URL="$AGENT_TRIGGER_MCP_URL" \
    AGENT_TRIGGER_MANAGED_PROJECTS_ROOT="$AGENT_TRIGGER_MANAGED_PROJECTS_ROOT" \
    AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS="$AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS" \
    AGENT_TRIGGER_GIT_CREDENTIALS_ROOT="${AGENT_TRIGGER_GIT_CREDENTIALS_ROOT:-.relay-agent-trigger/git-credentials}" \
    AGENT_TRIGGER_CODEX_AUTO_COMPACT_TOKEN_LIMIT="${AGENT_TRIGGER_CODEX_AUTO_COMPACT_TOKEN_LIMIT:-200000}" \
    AGENT_TRIGGER_RUN_ONCE=false \
    HARNESS_MODE="$HARNESS_MODE" \
    HARNESS_BASE_URL="${HARNESS_BASE_URL:-}" \
    HARNESS_PUBLIC_BASE_URL="${HARNESS_PUBLIC_BASE_URL:-}" \
    HARNESS_SPACE_PREFIX="${HARNESS_SPACE_PREFIX:-u-}" \
    HARNESS_REQUEST_TIMEOUT_SECONDS="${HARNESS_REQUEST_TIMEOUT_SECONDS:-15}" \
    HARNESS_CREDENTIALS_ROOT="${HARNESS_CREDENTIALS_ROOT:-.relay/harness-credentials}" \
    HARNESS_ADMIN_EMAIL="${HARNESS_ADMIN_EMAIL:-admin@relay.local}" \
    HARNESS_ADMIN_PASSWORD="${HARNESS_ADMIN_PASSWORD:-change-me-harness-admin}" \
    APP_ENV=development \
    "$ROOT_DIR/target/debug/ai-chat-agent-trigger"
  sleep 1
  local trigger_pid
  trigger_pid="$(read_pid trigger)"
  if ! is_pid_running "$trigger_pid"; then
    rm -f "$PID_DIR/trigger.pid"
    echo "Local Codex Agent Trigger exited during startup" >&2
    print_recent_log "Trigger" "$TRIGGER_LOG"
    return 1
  fi
}

start_web() {
  set_step "starting react web service"
  stop_pid web
  stop_port_listener "$WEB_HOST_PORT"
  start_detached_service web "$WEB_LOG" "$ROOT_DIR/apps/web" \
    /usr/bin/env \
    PATH="$PATH" \
    VITE_API_BASE_URL="http://127.0.0.1:${API_HOST_PORT}" \
    node "$ROOT_DIR/apps/web/node_modules/vite/bin/vite.js" \
    --host 0.0.0.0 --port "$WEB_HOST_PORT"
  wait_for_http "http://127.0.0.1:${WEB_HOST_PORT}"
}

print_port_notes() {
  local note

  if (( ${#PORT_NOTES[@]} == 0 )); then
    return
  fi

  echo
  echo "Port notes:"
  for note in "${PORT_NOTES[@]}"; do
    echo "- ${note}"
  done
}

print_process_status() {
  local name="$1"
  local pid
  pid="$(read_pid "$name")"

  if is_pid_running "$pid"; then
    echo "- ${name}: running (pid ${pid})"
  else
    echo "- ${name}: stopped"
  fi
}

print_http_status() {
  local label="$1"
  local url="$2"

  if curl -fsS "$url" >/dev/null 2>&1; then
    echo "- ${label}: reachable (${url})"
  else
    echo "- ${label}: unreachable (${url})"
  fi
}

print_status() {
  load_dev_env

  if [[ ! -f "$STATE_FILE" ]]; then
    echo "No local dev state found. Start it with: ./scripts/start_dev.sh up"
    return
  fi

  cat <<EOF
AI Chat local dev status

Processes:
EOF
  print_process_status api
  print_process_status web
  print_process_status trigger
  print_process_status watcher

  cat <<EOF

Health:
EOF
  print_http_status "Web" "http://127.0.0.1:${WEB_HOST_PORT}"
  print_http_status "API readiness" "http://127.0.0.1:${API_HOST_PORT}/ready"

  echo "- Harness mode: ${HARNESS_MODE:-self_hosted}"
  if [[ "${HARNESS_MODE:-self_hosted}" != "disabled" ]]; then
    echo "- Harness URL: ${HARNESS_PUBLIC_BASE_URL:-${HARNESS_BASE_URL:-not configured}}"
  fi

  cat <<EOF

Logs:
- API: ${API_LOG}
- Web: ${WEB_LOG}
- Trigger: ${TRIGGER_LOG}
- Watcher: ${WATCH_LOG}
EOF
}

print_doctor() {
  load_dev_env

  echo "AI Chat local dev doctor"
  echo

  if docker info >/dev/null 2>&1; then
    echo "- Docker daemon: reachable"
  else
    echo "- Docker daemon: unreachable"
  fi

  if [[ -f "$STATE_FILE" ]]; then
    echo "- Dev state: ${STATE_FILE}"
  else
    echo "- Runtime state: missing"
  fi

  echo
  print_status
  print_recent_log "API" "$API_LOG"
  print_recent_log "Web" "$WEB_LOG"
  print_recent_log "Trigger" "$TRIGGER_LOG"
}

print_summary() {
  cat <<EOF
AI Chat local dev is ready.

Services:
- Web: http://127.0.0.1:${WEB_HOST_PORT}
- API: http://127.0.0.1:${API_HOST_PORT}
- Trigger: local ai-chat-agent-trigger process
- PostgreSQL: 127.0.0.1:${POSTGRES_HOST_PORT} (ai_chat)
- Harness mode: ${HARNESS_MODE:-self_hosted}
- Harness URL: ${HARNESS_PUBLIC_BASE_URL:-${HARNESS_BASE_URL:-not configured}}

Logs:
- API: $API_LOG
- Web: $WEB_LOG
- Trigger: $TRIGGER_LOG
- Watcher: $WATCH_LOG

Commands:
- Stop local services: ./scripts/start_dev.sh down
- Restart local services: ./scripts/start_dev.sh restart
- Tail local logs: ./scripts/start_dev.sh logs
- Current status: ./scripts/start_dev.sh status
- Diagnose startup: ./scripts/start_dev.sh doctor
- Keep the data container only: docker compose up -d postgres

Notes:
- ai-chat-server will auto-restart when Rust files under apps/server or crates/ change
- ai-chat-agent-trigger runs as a separate host process and starts the configured local Codex sessions
- Open the exact Web URL shown above. If 15274 is occupied by another project, this script will switch to the next free port.
EOF
  print_port_notes
}

case "$MODE" in
  up)
    set_step "stopping docker app services"
    stop_docker_app_services
    stop_local_dev_processes
    set_step "preparing local environment"
    prepare_env
    start_dependencies
    start_api
    start_trigger
    start_web
    start_api_watcher
    set_step "completed"
    print_summary
    ;;
  restart)
    set_step "stopping docker app services"
    stop_docker_app_services
    stop_local_dev_processes
    set_step "preparing local environment"
    prepare_env
    start_api
    start_trigger
    start_web
    start_api_watcher
    set_step "completed"
    print_summary
    ;;
  down)
    stop_local_dev_processes
    rm -f "$STATE_FILE"
    ;;
  logs)
    touch "$API_LOG" "$WEB_LOG" "$TRIGGER_LOG" "$WATCH_LOG"
    tail -n 100 -f "$API_LOG" "$WEB_LOG" "$TRIGGER_LOG" "$WATCH_LOG"
    ;;
  status)
    print_status
    ;;
  doctor)
    print_doctor
    ;;
  watch-api)
    run_api_watcher
    ;;
  *)
    echo "Usage: $0 [up|restart|down|logs|status|doctor]" >&2
    exit 1
    ;;
esac
