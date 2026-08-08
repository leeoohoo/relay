$ErrorActionPreference = "Stop"

$packageDir = Split-Path -Parent $MyInvocation.MyCommand.Path

if (-not (Get-Command wsl.exe -ErrorAction SilentlyContinue)) {
    Write-Host "WSL2 is required. Run the following command in an Administrator PowerShell, restart Windows, and retry:"
    Write-Host "  wsl --install -d Ubuntu"
    exit 1
}

$wslSource = (& wsl.exe wslpath -a $packageDir).Trim()
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($wslSource)) {
    throw "Relay could not resolve the extracted package inside WSL2."
}

$commandTemplate = @'
set -euo pipefail
source_path="$RELAY_PACKAGE_SOURCE"
install_root="$HOME/.relay-app"

if ! command -v docker >/dev/null 2>&1; then
  echo "Docker Desktop with WSL Integration is required." >&2
  exit 1
fi
if ! docker info >/dev/null 2>&1; then
  echo "Docker Desktop is not ready, or WSL Integration is disabled for this distribution." >&2
  exit 1
fi

mkdir -p "$install_root"
cp -a "$source_path/." "$install_root/"
chmod +x "$install_root/start.sh" "$install_root/bin/ai-chat-agent-trigger" "$install_root/scripts/"*.sh
cd "$install_root"
exec bash ./start.sh
'@

Write-Host "Installing Relay into WSL2 at ~/.relay-app …"
& wsl.exe env "RELAY_PACKAGE_SOURCE=$wslSource" bash -lc $commandTemplate
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
