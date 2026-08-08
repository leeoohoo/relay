# Relay for Windows 10 / 11 through WSL2

Prerequisites:

- WSL2 with Ubuntu
- Docker Desktop with **Use the WSL 2 based engine** enabled
- Docker Desktop WSL Integration enabled for Ubuntu

Extract the ZIP, open PowerShell in the extracted Relay directory, then run:

```powershell
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

The installer copies Relay to `~/.relay-app` inside WSL2 and starts it. The Release already includes the web console and WSL2 Agent Trigger. Node.js, pnpm, Rust, and Cargo are not required.

If WSL2 is missing, first run this in an Administrator PowerShell and restart Windows:

```powershell
wsl --install -d Ubuntu
winget install -e --id Docker.DockerDesktop
```
