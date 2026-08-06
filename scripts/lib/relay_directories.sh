#!/usr/bin/env bash

# Shared host directories are written by both the Docker Server and the
# host-native Trigger. They must remain owned by the user running Relay.
relay_prepare_managed_directories() {
  local current_user current_uid current_gid managed_dir foreign_path
  local -a invalid_paths=()

  current_user="$(id -un)"
  current_uid="$(id -u)"
  current_gid="$(id -g)"

  for managed_dir in "$@"; do
    [[ -n "$managed_dir" ]] || continue
    if [[ -e "$managed_dir" ]]; then
      foreign_path="$(find "$managed_dir" ! -user "$current_user" -print -quit 2>/dev/null || true)"
      if [[ -n "$foreign_path" || ! -O "$managed_dir" || ! -w "$managed_dir" ]]; then
        invalid_paths+=("$managed_dir")
      fi
    fi
  done

  if (( ${#invalid_paths[@]} > 0 )); then
    cat >&2 <<EOF
Relay cannot start because Docker previously created host state as another user.
This is a Relay Linux permissions issue; no project or state data needs to be deleted.

Run this command once, then run ./start.sh again:
EOF
    printf '  sudo chown -R %q' "${current_uid}:${current_gid}" >&2
    for managed_dir in "${invalid_paths[@]}"; do
      printf ' %q' "$managed_dir" >&2
    done
    printf '\n\nDo not run Relay itself with sudo.\n' >&2
    return 1
  fi

  mkdir -p "$@"
}
