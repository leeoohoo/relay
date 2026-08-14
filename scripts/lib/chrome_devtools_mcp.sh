#!/usr/bin/env bash

relay_chrome_devtools_image() {
  printf '%s\n' "${RELAY_CHROME_DEVTOOLS_MCP_IMAGE:-relay/chrome-devtools-mcp:1.6.0}"
}

relay_command_exists() {
  local configured="$1"
  if [[ "$configured" == */* || "$configured" == *\\* ]]; then
    [[ -x "$configured" ]]
  else
    command -v "$configured" >/dev/null 2>&1
  fi
}

relay_host_chrome_exists() {
  local configured="${RELAY_CHROME_EXECUTABLE:-}"
  if [[ -n "$configured" ]]; then
    [[ -x "$configured" ]]
    return
  fi
  if [[ -x "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" ]] ||
     [[ -x "/Applications/Chromium.app/Contents/MacOS/Chromium" ]] ||
     [[ -x "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary" ]]; then
    return 0
  fi
  local candidate
  for candidate in google-chrome google-chrome-stable chromium chromium-browser chrome chrome.exe; do
    if command -v "$candidate" >/dev/null 2>&1; then
      return 0
    fi
  done
  return 1
}

relay_host_browser_available() {
  relay_command_exists "${RELAY_CHROME_DEVTOOLS_MCP_HOST_COMMAND:-npx}" && relay_host_chrome_exists
}

relay_browser_requires_docker() {
  local mode="${RELAY_CHROME_DEVTOOLS_MCP_MODE:-auto}"
  mode="$(printf '%s' "$mode" | tr '[:upper:]' '[:lower:]')"
  case "$mode" in
    host|native)
      return 1
      ;;
    docker)
      return 0
      ;;
    auto|"")
      ! relay_host_browser_available
      ;;
    *)
      echo "Invalid RELAY_CHROME_DEVTOOLS_MCP_MODE: $mode (expected auto, host, or docker)" >&2
      return 2
      ;;
  esac
}

relay_cleanup_legacy_chrome_devtools_containers() {
  local profile_root="$1"
  local image container_id source matched
  command -v docker >/dev/null 2>&1 || return 0
  [[ -d "$profile_root" ]] || return 0
  profile_root="$(cd "$profile_root" && pwd -P)"
  image="$(relay_chrome_devtools_image)"
  while IFS= read -r container_id; do
    [[ -n "$container_id" ]] || continue
    matched=false
    while IFS= read -r source; do
      case "$source" in
        "$profile_root"|"$profile_root"/*)
          matched=true
          break
          ;;
      esac
    done < <(docker inspect --format '{{range .Mounts}}{{println .Source}}{{end}}' "$container_id" 2>/dev/null || true)
    if [[ "$matched" == "true" ]]; then
      echo "Stopping legacy per-session browser container $container_id..."
      docker stop --time 5 "$container_id" >/dev/null 2>&1 || true
    fi
  done < <(docker ps --quiet --filter "ancestor=$image" 2>/dev/null || true)
}

relay_ensure_chrome_devtools_image() {
  local root_dir="$1"
  local build_network="${2:-default}"
  local image

  case "${RELAY_CHROME_DEVTOOLS_MCP_ENABLED:-true}" in
    0|false|FALSE|no|NO|off|OFF) return 0 ;;
  esac

  if relay_browser_requires_docker; then
    :
  else
    local browser_mode_status=$?
    if [[ "$browser_mode_status" -eq 1 ]]; then
      return 0
    fi
    return "$browser_mode_status"
  fi

  image="$(relay_chrome_devtools_image)"
  if docker image inspect "$image" >/dev/null 2>&1; then
    return 0
  fi

  echo "Preparing managed Chrome DevTools MCP browser runtime..."
  docker build \
    --network "$build_network" \
    --build-arg CHROME_DEVTOOLS_MCP_VERSION=1.6.0 \
    --tag "$image" \
    --file "$root_dir/Dockerfile.chrome-devtools-mcp" \
    "$root_dir"
}
