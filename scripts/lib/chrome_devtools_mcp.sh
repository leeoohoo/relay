#!/usr/bin/env bash

relay_chrome_devtools_image() {
  printf '%s\n' "${RELAY_CHROME_DEVTOOLS_MCP_IMAGE:-relay/chrome-devtools-mcp:1.6.0}"
}

relay_ensure_chrome_devtools_image() {
  local root_dir="$1"
  local build_network="${2:-default}"
  local image

  if [[ "${RELAY_CHROME_DEVTOOLS_MCP_ENABLED:-true}" != "true" ]]; then
    return 0
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
