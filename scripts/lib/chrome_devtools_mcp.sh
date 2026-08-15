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

relay_browser_install_retry_due() {
  local failure_marker="$1"
  local retry_seconds="$2"
  local failed_at now
  [[ -f "$failure_marker" ]] || return 0
  IFS= read -r failed_at <"$failure_marker" || return 0
  [[ "$failed_at" =~ ^[0-9]+$ ]] || return 0
  now="$(date +%s)"
  (( now - failed_at >= retry_seconds ))
}

relay_record_browser_install_failure() {
  local browser_root="$1"
  local failure_marker="$2"
  [[ -n "$browser_root" && "$browser_root" != "/" ]] || return 1
  rm -rf -- "$browser_root"
  mkdir -p "$browser_root"
  printf '%s\n' "$(date +%s)" >"$failure_marker"
}

relay_prepare_host_browser_executable() {
  local state_root="$1"
  local log_file="$2"
  local configured="${RELAY_CHROME_EXECUTABLE:-}"
  local enabled="${RELAY_MANAGED_HEADLESS_SHELL_ENABLED:-true}"
  local mode="${RELAY_CHROME_DEVTOOLS_MCP_MODE:-auto}"
  local browser_version="${RELAY_CHROME_HEADLESS_SHELL_VERSION:-152.0.7977.42}"
  local installer_version="${RELAY_PUPPETEER_BROWSERS_VERSION:-3.2.0}"
  local retry_seconds="${RELAY_CHROME_INSTALL_RETRY_SECONDS:-21600}"
  local installer_root installer_bin browser_root browser_bin failure_marker

  case "${RELAY_CHROME_DEVTOOLS_MCP_ENABLED:-true}" in
    0|false|FALSE|no|NO|off|OFF) return 1 ;;
  esac

  if [[ -n "$configured" ]]; then
    relay_command_exists "$configured" || return 1
    printf '%s\n' "$configured"
    return 0
  fi

  mode="$(printf '%s' "$mode" | tr '[:upper:]' '[:lower:]')"
  case "$mode" in
    docker) return 1 ;;
  esac
  case "$enabled" in
    0|false|FALSE|no|NO|off|OFF) return 1 ;;
  esac
  [[ "$retry_seconds" =~ ^[0-9]+$ ]] || retry_seconds=21600

  browser_root="$state_root/tools/chrome-headless-shell-$browser_version"
  failure_marker="$browser_root/.install-failed-at"
  browser_bin="$(
    find "$browser_root" -type f \
      \( -name chrome-headless-shell -o -name chrome-headless-shell.exe \) \
      -print -quit 2>/dev/null || true
  )"
  if [[ -n "$browser_bin" && -x "$browser_bin" ]] &&
     "$browser_bin" --version >/dev/null 2>&1; then
    rm -f "$failure_marker"
    printf '%s\n' "$browser_bin"
    return 0
  fi
  if ! relay_browser_install_retry_due "$failure_marker" "$retry_seconds"; then
    echo "Managed Chrome Headless Shell install is in retry backoff; using the available host or Docker fallback." >>"$log_file"
    return 1
  fi
  command -v npm >/dev/null 2>&1 || return 1

  installer_root="$state_root/tools/puppeteer-browsers-$installer_version"
  installer_bin="$installer_root/node_modules/.bin/browsers"
  if [[ ! -x "$installer_bin" ]]; then
    mkdir -p "$installer_root"
    echo "Preparing lightweight browser installer..." >>"$log_file"
    npm install \
      --prefix "$installer_root" \
      --no-audit \
      --no-fund \
      --no-package-lock \
      --omit=dev \
      "@puppeteer/browsers@$installer_version" >>"$log_file" 2>&1 || return 1
  fi

  rm -rf -- "$browser_root"
  mkdir -p "$browser_root"
  echo "Preparing lightweight Chrome Headless Shell $browser_version..." >&2
  echo "Preparing lightweight Chrome Headless Shell $browser_version..." >>"$log_file"
  "$installer_bin" install "chrome-headless-shell@$browser_version" \
    --path "$browser_root" >>"$log_file" 2>&1 || {
      relay_record_browser_install_failure "$browser_root" "$failure_marker" || true
      return 1
    }
  browser_bin="$(
    find "$browser_root" -type f \
      \( -name chrome-headless-shell -o -name chrome-headless-shell.exe \) \
      -print -quit 2>/dev/null || true
  )"
  if [[ -z "$browser_bin" || ! -x "$browser_bin" ]] ||
     ! "$browser_bin" --version >/dev/null 2>&1; then
    relay_record_browser_install_failure "$browser_root" "$failure_marker" || true
    return 1
  fi
  rm -f "$failure_marker"
  printf '%s\n' "$browser_bin"
}

relay_host_browser_available() {
  relay_command_exists "${RELAY_CHROME_DEVTOOLS_MCP_HOST_COMMAND:-npx}" && relay_host_chrome_exists
}

relay_prepare_host_chrome_devtools_command() {
  local state_root="$1"
  local log_file="$2"
  local configured="${RELAY_CHROME_DEVTOOLS_MCP_HOST_COMMAND:-npx}"
  local command_name tool_root tool_bin

  case "${RELAY_CHROME_DEVTOOLS_MCP_ENABLED:-true}" in
    0|false|FALSE|no|NO|off|OFF) return 1 ;;
  esac

  command_name="$(basename "$configured")"
  case "$command_name" in
    npx|npx.cmd|npx.exe) ;;
    *)
      relay_command_exists "$configured" || return 1
      printf '%s\n' "$configured"
      return 0
      ;;
  esac

  relay_host_chrome_exists || return 1
  command -v npm >/dev/null 2>&1 || return 1
  tool_root="$state_root/tools/chrome-devtools-mcp-1.6.0"
  tool_bin="$tool_root/node_modules/.bin/chrome-devtools-mcp"
  if [[ ! -x "$tool_bin" ]]; then
    mkdir -p "$tool_root"
    echo "Preparing lightweight shared Chrome DevTools MCP runtime..." >>"$log_file"
    npm install \
      --prefix "$tool_root" \
      --no-audit \
      --no-fund \
      --no-package-lock \
      --omit=dev \
      chrome-devtools-mcp@1.6.0 >>"$log_file" 2>&1 || return 1
  fi
  [[ -x "$tool_bin" ]] || return 1
  printf '%s\n' "$tool_bin"
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
