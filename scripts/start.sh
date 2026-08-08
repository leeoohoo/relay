#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-up}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCAL_ENV_FILE="$ROOT_DIR/.env.local"
RUNTIME_DIR="$ROOT_DIR/.relay/runtime"
TRIGGER_PID_FILE="$RUNTIME_DIR/trigger.pid"
TRIGGER_LOG="$RUNTIME_DIR/trigger.log"
TRIGGER_LAUNCH_LABEL="com.relay.ai-chat.trigger"

# shellcheck source=scripts/lib/relay_directories.sh
source "$ROOT_DIR/scripts/lib/relay_directories.sh"

if [[ -f "$LOCAL_ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$LOCAL_ENV_FILE"
  set +a
fi

export POSTGRES_HOST_PORT="${POSTGRES_HOST_PORT:-15533}"
export WEB_HOST_PORT="${WEB_HOST_PORT:-45274}"
export API_HOST_PORT="$WEB_HOST_PORT"
export HARNESS_MODE="${HARNESS_MODE:-self_hosted}"
export HARNESS_HOST_PORT="${HARNESS_HOST_PORT:-13101}"
export HARNESS_SSH_PORT="${HARNESS_SSH_PORT:-13123}"
export RELAY_DEFAULT_WORKSPACE_ROOT="${RELAY_DEFAULT_WORKSPACE_ROOT:-$ROOT_DIR/.relay-workspace}"
export AGENT_TRIGGER_MANAGED_PROJECTS_ROOT="${AGENT_TRIGGER_MANAGED_PROJECTS_ROOT:-$RELAY_DEFAULT_WORKSPACE_ROOT}"
export AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS="${AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS:-$RELAY_DEFAULT_WORKSPACE_ROOT}"
export HUMAN_FOLDER_REFERENCE_ALLOWED_ROOTS="${HUMAN_FOLDER_REFERENCE_ALLOWED_ROOTS:-$RELAY_DEFAULT_WORKSPACE_ROOT}"
export AGENT_TRIGGER_STATE_ROOT="${AGENT_TRIGGER_STATE_ROOT:-$ROOT_DIR/.relay-agent-trigger}"
export RELAY_HARNESS_CREDENTIALS_ROOT="${RELAY_HARNESS_CREDENTIALS_ROOT:-$ROOT_DIR/.relay/harness-credentials}"
export RELAY_MESSAGE_ATTACHMENTS_ROOT="${RELAY_MESSAGE_ATTACHMENTS_ROOT:-$ROOT_DIR/.relay/attachments}"
export RELAY_HOST_UID="${RELAY_HOST_UID:-$(id -u)}"
export RELAY_HOST_GID="${RELAY_HOST_GID:-$(id -g)}"
export RELAY_CHROME_DEVTOOLS_MCP_ENABLED="${RELAY_CHROME_DEVTOOLS_MCP_ENABLED:-true}"
export RELAY_CHROME_DEVTOOLS_MCP_IMAGE="${RELAY_CHROME_DEVTOOLS_MCP_IMAGE:-relay/chrome-devtools-mcp:1.6.0}"
export RELAY_CHROME_PROFILE_ROOT="${RELAY_CHROME_PROFILE_ROOT:-$AGENT_TRIGGER_STATE_ROOT/browser-profiles}"

relay_prepare_managed_directories \
  "$RUNTIME_DIR" \
  "$RELAY_DEFAULT_WORKSPACE_ROOT" \
  "$AGENT_TRIGGER_STATE_ROOT" \
  "$RELAY_HARNESS_CREDENTIALS_ROOT" \
  "$RELAY_MESSAGE_ATTACHMENTS_ROOT"
touch "$TRIGGER_LOG"

container_host_port() {
  local container_name="$1"
  local container_port="$2"
  docker inspect \
    -f "{{with index .HostConfig.PortBindings \"$container_port\"}}{{(index . 0).HostPort}}{{end}}" \
    "$container_name" 2>/dev/null || true
}

read_trigger_pid() {
  [[ -f "$TRIGGER_PID_FILE" ]] && tr -d '[:space:]' <"$TRIGGER_PID_FILE" || true
}

trigger_is_running() {
  local pid
  pid="$(read_trigger_pid)"
  [[ -n "$pid" ]] && kill -0 "$pid" >/dev/null 2>&1
}

stop_trigger() {
  local pid
  if [[ "$(uname -s)" == "Darwin" ]]; then
    launchctl remove "$TRIGGER_LAUNCH_LABEL" >/dev/null 2>&1 || true
  fi
  pid="$(read_trigger_pid)"
  if [[ -n "$pid" ]] && kill -0 "$pid" >/dev/null 2>&1; then
    kill "$pid" >/dev/null 2>&1 || true
    for _ in {1..20}; do
      kill -0 "$pid" >/dev/null 2>&1 || break
      sleep 0.25
    done
    kill -9 "$pid" >/dev/null 2>&1 || true
  fi
  rm -f "$TRIGGER_PID_FILE"
}

resolve_trigger_binary() {
  local configured="${RELAY_TRIGGER_BIN:-}"
  if [[ -n "$configured" && -x "$configured" ]]; then
    printf '%s\n' "$configured"
    return
  fi
  if [[ -x "$ROOT_DIR/bin/ai-chat-agent-trigger" ]]; then
    printf '%s\n' "$ROOT_DIR/bin/ai-chat-agent-trigger"
    return
  fi
  if ! command -v cargo >/dev/null 2>&1; then
    echo "Codex Trigger binary is missing. Install the packaged Relay release or install Rust/Cargo to build it." >&2
    return 1
  fi
  (cd "$ROOT_DIR" && cargo build --release -p ai-chat-agent-trigger >>"$TRIGGER_LOG" 2>&1)
  printf '%s\n' "$ROOT_DIR/target/release/ai-chat-agent-trigger"
}

start_trigger() {
  local trigger_bin postgres_port server_port harness_port trigger_harness_base_url
  local trigger_harness_public_base_url attempt stable_checks
  local -a trigger_command
  stop_trigger
  trigger_bin="$(resolve_trigger_binary)"
  postgres_port="$(container_host_port ai-chat-postgres 5432/tcp)"
  server_port="$(container_host_port ai-chat-server 8080/tcp)"
  [[ -n "$postgres_port" && -n "$server_port" ]] || {
    echo "Relay containers are missing their host ports; refusing to start Trigger." >&2
    return 1
  }

  export DATABASE_URL="postgres://postgres:postgres@127.0.0.1:${postgres_port}/ai_chat"
  export AGENT_TRIGGER_MCP_URL="http://127.0.0.1:${server_port}/mcp"
  trigger_harness_base_url="${HARNESS_BASE_URL:-}"
  trigger_harness_public_base_url="${HARNESS_PUBLIC_BASE_URL:-$trigger_harness_base_url}"
  if [[ "$HARNESS_MODE" == "self_hosted" ]]; then
    harness_port="$(container_host_port ai-chat-harness 3000/tcp)"
    [[ -n "$harness_port" ]] || {
      echo "Harness container is missing its host port; refusing to start Trigger." >&2
      return 1
    }
    trigger_harness_base_url="http://127.0.0.1:${harness_port}"
    trigger_harness_public_base_url="${HARNESS_PUBLIC_BASE_URL:-$trigger_harness_base_url}"
  fi

  trigger_command=(
    bash
    "$ROOT_DIR/scripts/run_dev_service.sh"
    "$TRIGGER_PID_FILE"
    "$ROOT_DIR"
    /usr/bin/env
    "PATH=$PATH"
    "DATABASE_URL=$DATABASE_URL"
    "AGENT_TRIGGER_MCP_URL=$AGENT_TRIGGER_MCP_URL"
    "AGENT_TRIGGER_MANAGED_PROJECTS_ROOT=$AGENT_TRIGGER_MANAGED_PROJECTS_ROOT"
    "AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS=$AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS"
    "AGENT_TRIGGER_STATE_ROOT=$AGENT_TRIGGER_STATE_ROOT"
    "AGENT_TRIGGER_GIT_CREDENTIALS_ROOT=$AGENT_TRIGGER_STATE_ROOT/git-credentials"
    "AGENT_TRIGGER_CODEX_AUTO_COMPACT_TOKEN_LIMIT=${AGENT_TRIGGER_CODEX_AUTO_COMPACT_TOKEN_LIMIT:-200000}"
    "RELAY_CHROME_DEVTOOLS_MCP_ENABLED=$RELAY_CHROME_DEVTOOLS_MCP_ENABLED"
    "RELAY_CHROME_DEVTOOLS_MCP_IMAGE=$RELAY_CHROME_DEVTOOLS_MCP_IMAGE"
    "RELAY_CHROME_PROFILE_ROOT=$RELAY_CHROME_PROFILE_ROOT"
    "RELAY_HOST_UID=$RELAY_HOST_UID"
    "RELAY_HOST_GID=$RELAY_HOST_GID"
    "AGENT_TRIGGER_RUN_ONCE=false"
    "HARNESS_MODE=$HARNESS_MODE"
    "HARNESS_BASE_URL=$trigger_harness_base_url"
    "HARNESS_PUBLIC_BASE_URL=$trigger_harness_public_base_url"
    "HARNESS_SPACE_PREFIX=${HARNESS_SPACE_PREFIX:-u-}"
    "HARNESS_REQUEST_TIMEOUT_SECONDS=${HARNESS_REQUEST_TIMEOUT_SECONDS:-15}"
    "HARNESS_CREDENTIALS_ROOT=$RELAY_HARNESS_CREDENTIALS_ROOT"
    "HARNESS_ADMIN_EMAIL=${HARNESS_ADMIN_EMAIL:-admin@relay.local}"
    "HARNESS_ADMIN_PASSWORD=${HARNESS_ADMIN_PASSWORD:-change-me-harness-admin}"
    "APP_ENV=production"
    "$trigger_bin"
  )

  if [[ "$(uname -s)" == "Darwin" ]]; then
    launchctl submit \
      -l "$TRIGGER_LAUNCH_LABEL" \
      -o "$TRIGGER_LOG" \
      -e "$TRIGGER_LOG" \
      -- "${trigger_command[@]}"
  else
    nohup "${trigger_command[@]}" </dev/null >>"$TRIGGER_LOG" 2>&1 &
  fi

  stable_checks=0
  for attempt in {1..40}; do
    if trigger_is_running; then
      stable_checks=$((stable_checks + 1))
      if (( stable_checks >= 8 )); then
        return 0
      fi
    else
      stable_checks=0
    fi
    sleep 0.25
  done
  echo "Codex Trigger failed to start. See $TRIGGER_LOG" >&2
  tail -n 40 "$TRIGGER_LOG" >&2 || true
  return 1
}

print_status() {
  local server_port harness_port
  server_port="$(container_host_port ai-chat-server 8080/tcp)"
  harness_port="$(container_host_port ai-chat-harness 3000/tcp)"
  bash "$ROOT_DIR/scripts/start_docker.sh" ps --harness "$HARNESS_MODE"
  if trigger_is_running; then
    echo "Trigger: running (pid $(read_trigger_pid))"
  else
    echo "Trigger: stopped"
  fi
  [[ -n "$server_port" ]] && echo "Relay: http://127.0.0.1:${server_port}"
  [[ -n "$harness_port" && "$HARNESS_MODE" != "disabled" ]] && echo "Harness: http://127.0.0.1:${harness_port}"
  echo "Workspace: $RELAY_DEFAULT_WORKSPACE_ROOT"
}

start_all() {
  bash "$ROOT_DIR/scripts/start_dev.sh" down >/dev/null 2>&1 || true
  bash "$ROOT_DIR/scripts/start_docker.sh" up --harness "$HARNESS_MODE"
  start_trigger
  local server_port
  server_port="$(container_host_port ai-chat-server 8080/tcp)"
  curl -fsS "http://127.0.0.1:${server_port}/ready" >/dev/null
  trigger_is_running
  echo
  echo "Relay is ready: http://127.0.0.1:${server_port}"
  echo "Harness mode: $HARNESS_MODE"
  echo "Workspace: $RELAY_DEFAULT_WORKSPACE_ROOT"
}

case "$MODE" in
  up|restart)
    start_all
    ;;
  down)
    stop_trigger
    bash "$ROOT_DIR/scripts/start_docker.sh" down --harness "$HARNESS_MODE"
    ;;
  status|ps)
    print_status
    ;;
  logs)
    echo "Trigger log: $TRIGGER_LOG"
    tail -n 80 "$TRIGGER_LOG"
    bash "$ROOT_DIR/scripts/start_docker.sh" logs --harness "$HARNESS_MODE"
    ;;
  *)
    echo "Usage: $0 [up|restart|down|status|logs]" >&2
    exit 1
    ;;
esac
