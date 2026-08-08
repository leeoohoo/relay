#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/package_release.sh <version> <platform> <trigger-binary> <output-dir>

Platforms:
  macos-apple-silicon
  windows-wsl2-x86_64
EOF
}

[[ $# -eq 4 ]] || { usage >&2; exit 1; }

VERSION="$1"
PLATFORM="$2"
TRIGGER_BINARY="$3"
OUTPUT_DIR="$4"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR="$(cd "$OUTPUT_DIR" && pwd)"

[[ -x "$TRIGGER_BINARY" ]] || {
  echo "Trigger binary is missing or is not executable: $TRIGGER_BINARY" >&2
  exit 1
}
[[ -f "$ROOT_DIR/apps/web/dist/index.html" ]] || {
  echo "Packaged web assets are missing. Run pnpm --dir apps/web build first." >&2
  exit 1
}

case "$PLATFORM" in
  macos-apple-silicon|windows-wsl2-x86_64) ;;
  *) echo "Unsupported release platform: $PLATFORM" >&2; usage >&2; exit 1 ;;
esac

TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMP_DIR"' EXIT

PACKAGE_NAME="relay-${VERSION}-${PLATFORM}"
PACKAGE_ROOT="$TEMP_DIR/$PACKAGE_NAME"
mkdir -p "$PACKAGE_ROOT"

git -C "$ROOT_DIR" archive --format=tar HEAD | tar -xf - -C "$PACKAGE_ROOT"
mkdir -p "$PACKAGE_ROOT/bin" "$PACKAGE_ROOT/apps/web/dist"
cp "$TRIGGER_BINARY" "$PACKAGE_ROOT/bin/ai-chat-agent-trigger"
chmod +x "$PACKAGE_ROOT/bin/ai-chat-agent-trigger"
cp -R "$ROOT_DIR/apps/web/dist/." "$PACKAGE_ROOT/apps/web/dist/"

cat >"$PACKAGE_ROOT/RELAY_RELEASE" <<EOF
version=$VERSION
platform=$PLATFORM
EOF

case "$PLATFORM" in
  macos-apple-silicon)
    cp "$ROOT_DIR/packaging/install-macos.sh" "$PACKAGE_ROOT/install.sh"
    cp "$ROOT_DIR/packaging/INSTALL-macos.md" "$PACKAGE_ROOT/INSTALL.md"
    chmod +x "$PACKAGE_ROOT/install.sh" "$PACKAGE_ROOT/start.sh" "$PACKAGE_ROOT/scripts/"*.sh
    tar -C "$TEMP_DIR" -czf "$OUTPUT_DIR/relay-macos-apple-silicon.tar.gz" "$PACKAGE_NAME"
    ;;
  windows-wsl2-x86_64)
    command -v zip >/dev/null 2>&1 || { echo "Missing required command: zip" >&2; exit 1; }
    cp "$ROOT_DIR/packaging/install-windows.ps1" "$PACKAGE_ROOT/install.ps1"
    cp "$ROOT_DIR/packaging/INSTALL-windows.md" "$PACKAGE_ROOT/INSTALL.md"
    chmod +x "$PACKAGE_ROOT/start.sh" "$PACKAGE_ROOT/scripts/"*.sh
    (cd "$TEMP_DIR" && zip -q -r "$OUTPUT_DIR/relay-windows-wsl2-x86_64.zip" "$PACKAGE_NAME")
    ;;
esac

echo "Created $PACKAGE_NAME in $OUTPUT_DIR"
