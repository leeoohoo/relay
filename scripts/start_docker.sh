#!/usr/bin/env bash
set -euo pipefail

MODE="up"
HARNESS_SELECTION="${HARNESS_MODE:-}"
while (( $# > 0 )); do
  case "$1" in
    up|rebuild|down|restart|ps|logs)
      MODE="$1"
      shift
      ;;
    --harness)
      [[ $# -ge 2 ]] || { echo "--harness requires a mode" >&2; exit 1; }
      HARNESS_SELECTION="$2"
      shift 2
      ;;
    --harness=*)
      HARNESS_SELECTION="${1#*=}"
      shift
      ;;
    *)
      echo "Unknown argument: $1" >&2
      echo "Usage: $0 [up|rebuild|down|restart|ps|logs] [--harness official|self-hosted|disabled]" >&2
      exit 1
      ;;
  esac
done
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="$ROOT_DIR/docker-compose.yml"
DOCKER_DAEMON_JSON="${DOCKER_DAEMON_JSON:-$HOME/.docker/daemon.json}"
RELEASE_MARKER="$ROOT_DIR/RELAY_RELEASE"
PACKAGED_WEB_INDEX="$ROOT_DIR/apps/web/dist/index.html"

# shellcheck source=scripts/lib/relay_directories.sh
source "$ROOT_DIR/scripts/lib/relay_directories.sh"

export RELAY_HOST_UID="${RELAY_HOST_UID:-$(id -u)}"
export RELAY_HOST_GID="${RELAY_HOST_GID:-$(id -g)}"
export AGENT_TRIGGER_STATE_ROOT="${AGENT_TRIGGER_STATE_ROOT:-$ROOT_DIR/.relay-agent-trigger}"
export RELAY_DEFAULT_WORKSPACE_ROOT="${RELAY_DEFAULT_WORKSPACE_ROOT:-$ROOT_DIR/.relay-workspace}"
export RELAY_HARNESS_CREDENTIALS_ROOT="${RELAY_HARNESS_CREDENTIALS_ROOT:-$ROOT_DIR/.relay/harness-credentials}"
export RELAY_MESSAGE_ATTACHMENTS_ROOT="${RELAY_MESSAGE_ATTACHMENTS_ROOT:-$ROOT_DIR/.relay/attachments}"
export AGENT_TRIGGER_MANAGED_PROJECTS_ROOT="${AGENT_TRIGGER_MANAGED_PROJECTS_ROOT:-$RELAY_DEFAULT_WORKSPACE_ROOT}"
export AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS="${AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS:-$RELAY_DEFAULT_WORKSPACE_ROOT}"
export HUMAN_FOLDER_REFERENCE_ALLOWED_ROOTS="${HUMAN_FOLDER_REFERENCE_ALLOWED_ROOTS:-$RELAY_DEFAULT_WORKSPACE_ROOT}"
KNOWN_BAD_DOCKER_MIRRORS=(
  "hub-mirror.c.163.com"
  "mirror.baidubce.com"
)
REQUIRED_DOCKER_IMAGES=(
  "postgres:16"
  "rust:1.94-bookworm"
  "debian:bookworm-slim"
)

if ! command -v docker >/dev/null 2>&1; then
  echo "Missing required command: docker" >&2
  exit 1
fi

compose() {
  docker compose -f "$COMPOSE_FILE" --profile harness-self-hosted "$@"
}

prepare_harness_mode() {
  local selection="$HARNESS_SELECTION"

  if [[ -z "$selection" && -t 0 && "$MODE" =~ ^(up|rebuild)$ ]]; then
    echo "Harness deployment mode:"
    echo "  1) official     Use an existing hosted Harness service"
    echo "  2) self-hosted  Start Harness in this Docker stack"
    echo "  3) disabled     Do not provision Harness accounts"
    read -r -p "Select [1-3, default 2]: " selection
    case "$selection" in
      1) selection="official" ;;
      2) selection="self-hosted" ;;
      3) selection="disabled" ;;
      *) selection="self-hosted" ;;
    esac
  fi

  selection="${selection:-self_hosted}"
  selection="${selection//-/_}"
  case "$selection" in
    official|hosted|cloud)
      export HARNESS_MODE="official"
      if [[ "$MODE" =~ ^(up|rebuild)$ && -z "${HARNESS_BASE_URL:-}" ]]; then
        echo "HARNESS_BASE_URL is required for official Harness mode." >&2
        exit 1
      fi
      if [[ -n "${HARNESS_BASE_URL:-}" ]]; then
        export HARNESS_PUBLIC_BASE_URL="${HARNESS_PUBLIC_BASE_URL:-$HARNESS_BASE_URL}"
      fi
      ;;
    self_hosted|local|docker)
      export HARNESS_MODE="self_hosted"
      export HARNESS_BASE_URL="http://harness:3000"
      REQUIRED_DOCKER_IMAGES+=("${HARNESS_IMAGE:-harness/harness:latest}")
      ;;
    disabled|off|none)
      export HARNESS_MODE="disabled"
      ;;
    *)
      echo "Unsupported Harness mode: $selection" >&2
      exit 1
      ;;
  esac
}

prepare_shared_directories() {
  relay_prepare_managed_directories \
    "$AGENT_TRIGGER_STATE_ROOT" \
    "$RELAY_DEFAULT_WORKSPACE_ROOT" \
    "$RELAY_HARNESS_CREDENTIALS_ROOT" \
    "$RELAY_MESSAGE_ATTACHMENTS_ROOT"
}

build_web_assets() {
  if [[ -f "$RELEASE_MARKER" && -f "$PACKAGED_WEB_INDEX" ]]; then
    echo "Using web assets included in the Relay release package."
    return 0
  fi

  if ! command -v pnpm >/dev/null 2>&1; then
    cat >&2 <<'EOF'
Missing required command: pnpm.

Source checkouts need Node.js 22 and pnpm to build the web console.
Normal users can instead download the packaged Relay release, which already includes
the web assets and host Trigger binary.
EOF
    exit 1
  fi

  (cd "$ROOT_DIR" && pnpm --dir apps/web build)
}

prepare_docker_build_network() {
  if [[ -z "${DOCKER_BUILD_NETWORK:-}" ]]; then
    if [[ "$(uname -s)" == "Linux" ]]; then
      DOCKER_BUILD_NETWORK="host"
    else
      DOCKER_BUILD_NETWORK="default"
    fi
  fi
  export DOCKER_BUILD_NETWORK
}

existing_host_port_for() {
  local container_name="$1"
  local container_port="$2"

  docker inspect \
    -f "{{with index .HostConfig.PortBindings \"$container_port\"}}{{(index . 0).HostPort}}{{end}}" \
    "$container_name" 2>/dev/null || true
}

ensure_docker_daemon() {
  local docker_error

  if docker_error="$(docker info 2>&1)"; then
    return 0
  fi

  if [[ "$docker_error" == *"permission denied"* ]]; then
    cat >&2 <<EOF
Docker is running, but the current user (${USER:-unknown}) cannot access the Docker socket.

Fix the Linux/WSL2 Docker group once:
  getent group docker >/dev/null || sudo groupadd docker
  sudo usermod -aG docker "\$USER"
  sudo chgrp docker /var/run/docker.sock
  sudo chmod 660 /var/run/docker.sock
  newgrp docker
  docker info

If the socket is recreated with the wrong group, restart the Docker provider and repeat
the chgrp/chmod commands:
  Linux Docker Engine: sudo systemctl restart docker
  Snap Docker:         sudo snap restart docker
  WSL2 Docker Desktop: run 'wsl --shutdown' in PowerShell, then reopen Ubuntu

Then re-run:
  ./start.sh

Do not run Relay with sudo; that can create root-owned workspace and credential files.
EOF
    exit 1
  fi

  cat >&2 <<EOF
Docker is installed, but the Docker daemon is not reachable.

- Docker Desktop: start Docker Desktop and wait until it reports that Docker is running.
- Linux Docker Engine: start it with 'sudo systemctl start docker'.
- WSL2: enable Docker Desktop's WSL integration for this distribution.

Diagnostic command:
  docker info
EOF
  exit 1
}

list_known_bad_mirrors_in_daemon() {
  local mirror

  [[ -f "$DOCKER_DAEMON_JSON" ]] || return 0

  for mirror in "${KNOWN_BAD_DOCKER_MIRRORS[@]}"; do
    if grep -q "$mirror" "$DOCKER_DAEMON_JSON"; then
      printf '%s\n' "$mirror"
    fi
  done
}

list_missing_docker_images() {
  local image

  for image in "${REQUIRED_DOCKER_IMAGES[@]}"; do
    if ! docker image inspect "$image" >/dev/null 2>&1; then
      printf '%s\n' "$image"
    fi
  done
}

print_registry_mirror_fix() {
  local detected_mirrors=()
  local mirror

  while IFS= read -r mirror; do
    [[ -n "$mirror" ]] && detected_mirrors+=("$mirror")
  done < <(list_known_bad_mirrors_in_daemon)

  cat >&2 <<EOF
Detected Docker registry mirror risk in $DOCKER_DAEMON_JSON.

The Docker daemon is configured with one or more failing mirrors, and the required base images are not all available locally.
This usually prevents Docker from pulling the images needed for:
- postgres
- ai-chat-server
- built-in web assets served by ai-chat-server

Detected risky mirrors:
EOF

  if (( ${#detected_mirrors[@]} == 0 )); then
    echo "- unknown mirror from daemon config" >&2
  else
    printf -- '- %s\n' "${detected_mirrors[@]}" >&2
  fi

  cat >&2 <<EOF

Recommended fix:
1. Edit $DOCKER_DAEMON_JSON
2. Remove the failing mirror entry, for example keep:
{
  "registry-mirrors": [
    "https://dockerproxy.com"
  ]
}
3. Restart Docker Desktop
4. Re-run: ./scripts/start_docker.sh up
EOF
}

preflight_docker_images() {
  local missing_images=()
  local image

  while IFS= read -r image; do
    [[ -n "$image" ]] && missing_images+=("$image")
  done < <(list_missing_docker_images)

  if (( ${#missing_images[@]} == 0 )); then
    return 0
  fi

  if [[ -n "$(list_known_bad_mirrors_in_daemon)" ]]; then
    echo "Cannot continue Docker startup because these images are missing locally:" >&2
    printf -- '- %s\n' "${missing_images[@]}" >&2
    print_registry_mirror_fix
    exit 1
  fi
}

RESERVED_PORTS=""

port_is_reserved() {
  local port="$1"
  [[ " $RESERVED_PORTS " == *" $port "* ]]
}

reserve_port() {
  local port="$1"
  if ! port_is_reserved "$port"; then
    RESERVED_PORTS="${RESERVED_PORTS} ${port}"
  fi
}

port_is_in_use() {
  local port="$1"
  if command -v lsof >/dev/null 2>&1; then
    lsof -iTCP:"$port" -sTCP:LISTEN -n -P >/dev/null 2>&1
    return $?
  fi

  docker ps --format '{{.Ports}}' | grep -q ":$port->"
}

choose_port() {
  local preferred="$1"
  local candidate="$preferred"

  while port_is_in_use "$candidate" || port_is_reserved "$candidate"; do
    candidate=$((candidate + 1))
  done

  printf '%s\n' "$candidate"
}

load_existing_port_assignments() {
  local existing_port=""

  if [[ -z "${POSTGRES_HOST_PORT:-}" ]]; then
    existing_port="$(existing_host_port_for ai-chat-postgres 5432/tcp)"
    [[ -n "$existing_port" && "$existing_port" -ge 15533 ]] && POSTGRES_HOST_PORT="$existing_port"
  fi

  if [[ -z "${API_HOST_PORT:-}" ]]; then
    existing_port="$(existing_host_port_for ai-chat-server 8080/tcp)"
    [[ -n "$existing_port" && "$existing_port" -ge 45274 ]] && API_HOST_PORT="$existing_port"
  fi

  if [[ -z "${WEB_HOST_PORT:-}" ]]; then
    existing_port="$(existing_host_port_for ai-chat-server 8080/tcp)"
    [[ -n "$existing_port" && "$existing_port" -ge 45274 ]] && WEB_HOST_PORT="$existing_port"
  fi

  if [[ -z "${HARNESS_HOST_PORT:-}" ]]; then
    existing_port="$(existing_host_port_for ai-chat-harness 3000/tcp)"
    [[ -n "$existing_port" && "$existing_port" -ge 13101 ]] && HARNESS_HOST_PORT="$existing_port"
  fi

  if [[ -z "${HARNESS_SSH_PORT:-}" ]]; then
    existing_port="$(existing_host_port_for ai-chat-harness 3022/tcp)"
    [[ -n "$existing_port" && "$existing_port" -ge 13123 ]] && HARNESS_SSH_PORT="$existing_port"
  fi

  # A missing previous container/port is the normal first-start case. Without
  # an explicit success return, the final conditional above can make this
  # function return 1 and `set -e` aborts startup immediately after web build.
  return 0
}

choose_service_port() {
  local container_name="$1"
  local container_port="$2"
  local preferred="$3"
  local existing_port container_running

  existing_port="$(existing_host_port_for "$container_name" "$container_port")"
  container_running="$(docker inspect -f '{{.State.Running}}' "$container_name" 2>/dev/null || true)"
  if [[ "$existing_port" == "$preferred" ]]; then
    printf '%s\n' "$preferred"
  elif [[ "$container_running" == "true" \
    && -n "$existing_port" \
    && "$existing_port" -ge "$preferred" ]] \
    && ! port_is_reserved "$existing_port" \
    && port_is_in_use "$preferred"; then
    printf '%s\n' "$existing_port"
  else
    choose_port "$preferred"
  fi
}

assign_port() {
  local var_name="$1"
  local preferred="$2"
  local selected_port

  selected_port="$(choose_port "$preferred")"
  reserve_port "$selected_port"
  printf -v "$var_name" '%s' "$selected_port"
}

prepare_ports() {
  local postgres_host_port public_host_port harness_host_port harness_ssh_port

  load_existing_port_assignments

  if [[ -n "${POSTGRES_HOST_PORT:-}" ]]; then
    postgres_host_port="$(choose_service_port ai-chat-postgres 5432/tcp "$POSTGRES_HOST_PORT")"
    reserve_port "$postgres_host_port"
  else
    assign_port postgres_host_port 15533
  fi

  if [[ -n "${WEB_HOST_PORT:-}" ]]; then
    public_host_port="$(choose_service_port ai-chat-server 8080/tcp "$WEB_HOST_PORT")"
    reserve_port "$public_host_port"
  elif [[ -n "${API_HOST_PORT:-}" ]]; then
    public_host_port="$(choose_service_port ai-chat-server 8080/tcp "$API_HOST_PORT")"
    reserve_port "$public_host_port"
  else
    assign_port public_host_port 45274
  fi

  export POSTGRES_HOST_PORT="$postgres_host_port"
  export API_HOST_PORT="$public_host_port"
  export WEB_HOST_PORT="$public_host_port"
  export DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@127.0.0.1:${POSTGRES_HOST_PORT}/ai_chat}"
  export VITE_API_BASE_URL="${VITE_API_BASE_URL:-http://127.0.0.1:${WEB_HOST_PORT}}"

  if [[ "$HARNESS_MODE" == "self_hosted" ]]; then
    if [[ -n "${HARNESS_HOST_PORT:-}" ]]; then
      harness_host_port="$(choose_service_port ai-chat-harness 3000/tcp "$HARNESS_HOST_PORT")"
      reserve_port "$harness_host_port"
    else
      assign_port harness_host_port 13101
    fi
    export HARNESS_HOST_PORT="$harness_host_port"
    if [[ -n "${HARNESS_SSH_PORT:-}" ]]; then
      harness_ssh_port="$(choose_service_port ai-chat-harness 3022/tcp "$HARNESS_SSH_PORT")"
      reserve_port "$harness_ssh_port"
    else
      assign_port harness_ssh_port 13123
    fi
    export HARNESS_SSH_PORT="$harness_ssh_port"
    export HARNESS_PUBLIC_BASE_URL="${HARNESS_PUBLIC_BASE_URL:-http://127.0.0.1:${HARNESS_HOST_PORT}}"
    export HARNESS_GIT_BASE_URL="${HARNESS_GIT_BASE_URL:-http://127.0.0.1:${HARNESS_HOST_PORT}/git}"
  fi
}

start_harness_service() {
  if [[ "$HARNESS_MODE" != "self_hosted" ]]; then
    compose stop harness >/dev/null 2>&1 || true
    return 0
  fi
  compose up -d harness
  wait_for_container_health ai-chat-harness 90
}

start_application_services() {
  compose up -d --build --no-deps server
  wait_for_container_health ai-chat-server
}

wait_for_container_health() {
  local container_name="$1"
  local retries="${2:-60}"
  local attempt=1

  while (( attempt <= retries )); do
    local inspect_output container_status health_status
    inspect_output="$(docker inspect -f '{{.State.Status}} {{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' "$container_name" 2>/dev/null || true)"
    container_status="${inspect_output%% *}"
    health_status="${inspect_output#* }"

    if [[ "$health_status" == "healthy" ]]; then
      return 0
    fi

    if [[ "$health_status" == "none" && "$container_status" == "running" ]]; then
      return 0
    fi

    sleep 2
    attempt=$((attempt + 1))
  done

  echo "Container did not become ready in time: $container_name" >&2
  docker ps --filter "name=$container_name"
  return 1
}

wait_for_tcp_port() {
  local host="$1"
  local port="$2"
  local retries="${3:-60}"
  local attempt=1

  while (( attempt <= retries )); do
    if command -v nc >/dev/null 2>&1 && nc -z "$host" "$port" >/dev/null 2>&1; then
      return 0
    fi

    if (exec 3<>"/dev/tcp/${host}/${port}") >/dev/null 2>&1; then
      exec 3<&-
      exec 3>&-
      return 0
    fi

    sleep 2
    attempt=$((attempt + 1))
  done

  echo "TCP port did not become reachable in time: ${host}:${port}" >&2
  return 1
}

run_migrations() {
  local database_url="${DATABASE_URL:-postgres://postgres:postgres@127.0.0.1:15533/ai_chat}"
  local pg_container="${PG_CONTAINER:-ai-chat-postgres}"

  FORCE_DOCKER_PSQL=true \
  DATABASE_URL="$database_url" \
  PG_CONTAINER="$pg_container" \
  bash "$ROOT_DIR/scripts/run_pg_migrations.sh" ensure
}

print_summary() {
  cat <<EOF
AI Chat Docker stack is ready.

Services:
- Web/API: http://127.0.0.1:${WEB_HOST_PORT}
- PostgreSQL: 127.0.0.1:${POSTGRES_HOST_PORT} (ai_chat)
- Harness mode: ${HARNESS_MODE}

Useful commands:
- View status: ./scripts/start_docker.sh ps
- View logs: ./scripts/start_docker.sh logs
- Stop stack: ./scripts/start_docker.sh down
EOF
}

case "$MODE" in
  up)
    ensure_docker_daemon
    prepare_shared_directories
    prepare_docker_build_network
    prepare_harness_mode
    preflight_docker_images
    build_web_assets
    prepare_ports
    compose up -d postgres
    wait_for_container_health ai-chat-postgres
    wait_for_tcp_port 127.0.0.1 "$POSTGRES_HOST_PORT"
    run_migrations
    start_harness_service
    start_application_services
    print_summary
    ;;
  rebuild)
    ensure_docker_daemon
    prepare_shared_directories
    prepare_docker_build_network
    prepare_harness_mode
    preflight_docker_images
    build_web_assets
    prepare_ports
    compose down
    compose up -d postgres
    wait_for_container_health ai-chat-postgres
    wait_for_tcp_port 127.0.0.1 "$POSTGRES_HOST_PORT"
    run_migrations
    start_harness_service
    start_application_services
    print_summary
    ;;
  down)
    prepare_harness_mode
    compose down
    ;;
  restart)
    prepare_shared_directories
    prepare_harness_mode
    prepare_ports
    start_harness_service
    compose restart postgres server
    ;;
  ps)
    prepare_harness_mode
    prepare_ports
    compose ps
    ;;
  logs)
    prepare_harness_mode
    prepare_ports
    compose logs -f --tail=100
    ;;
  *)
    echo "Usage: $0 [up|rebuild|down|restart|ps|logs] [--harness official|self-hosted|disabled]" >&2
    exit 1
    ;;
esac
