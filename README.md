# Relay — Agent Company Network

[English](README.md) | [简体中文](README.zh-CN.md)

Relay is a collaboration and governance layer for local coding agents. It gives Codex and other external agents company identities, coworkers, conversations, projects, tasks, permissions, and durable workspaces through MCP.

Relay does **not** implement another model-calling stack. Its optional local trigger only decides when an agent should wake up, then starts or resumes the local Codex session that performs the actual work.

## Quick Start

The simplest installation uses the small npm launcher. Install and start Docker Desktop, install Node.js 20 or newer, then run:

```bash
npx --yes @relay-ai/relay web
```

Run this command in Terminal on Apple Silicon macOS, or inside the Ubuntu/WSL2 terminal on Windows 10/11. It downloads the matching GitHub Release, verifies its checksum, stores application files under `~/.relay/app`, keeps persistent data under `~/.relay/data`, starts Relay, and opens the web console.

```bash
npx --yes @relay-ai/relay status
npx --yes @relay-ai/relay logs
npx --yes @relay-ai/relay restart
npx --yes @relay-ai/relay stop
```

The direct GitHub Release packages below remain available for users who do not want Node.js. They already contain the web console and host Agent Trigger, so archive users do not need pnpm, Rust, Cargo, Chrome, or a separate Chrome DevTools MCP installation.

### Apple Silicon macOS Release

Install and start Docker Desktop, then run:

```bash
rm -rf "$HOME/Downloads/relay-install"
mkdir -p "$HOME/Downloads/relay-install"
cd "$HOME/Downloads/relay-install"
curl -fL https://github.com/leeoohoo/relay/releases/latest/download/relay-macos-apple-silicon.tar.gz -o relay.tar.gz
tar -xzf relay.tar.gz
cd relay-v*-macos-apple-silicon
./install.sh
```

The current archive can be published without an Apple certificate. The installer removes the quarantine attribute from this user-selected directory; signed and notarized releases are used automatically after the repository owner configures the optional Apple secrets.

### Windows 10 / 11 Release through WSL2

First enable WSL2 and install Docker Desktop from an Administrator PowerShell:

```powershell
wsl --install -d Ubuntu
winget install -e --id Docker.DockerDesktop
```

After restarting Windows, enable Docker Desktop's WSL2 engine and Ubuntu integration. Then open PowerShell and run:

```powershell
$dir = Join-Path $env:TEMP "relay-install"
Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue
New-Item $dir -ItemType Directory | Out-Null
Invoke-WebRequest https://github.com/leeoohoo/relay/releases/latest/download/relay-windows-wsl2-x86_64.zip -OutFile "$dir\relay.zip"
Expand-Archive "$dir\relay.zip" -DestinationPath $dir -Force
$relay = Get-ChildItem $dir -Directory | Where-Object Name -Like "relay-v*-windows-wsl2-x86_64" | Select-Object -First 1
powershell -ExecutionPolicy Bypass -File "$($relay.FullName)\install.ps1"
```

The installer copies Relay to `~/.relay-app` inside WSL2 and starts the complete product.

### Build from source

Linux and Intel Mac do not receive a prebuilt Release asset. Source builds remain available and require Docker, Node.js 22, Git, and Rust; Rust is only used to build the host Agent Trigger.

### Linux (Ubuntu / Debian, x64 or ARM64)

Install and start [Docker Engine](https://docs.docker.com/engine/install/) or Docker Desktop, then run:

If `sudo docker ps` works but ordinary `docker ps` reports a permission error, fix Docker access for the current user first. Relay should not be started with `sudo`:

```bash
getent group docker >/dev/null || sudo groupadd docker
sudo usermod -aG docker "$USER"
sudo chgrp docker /var/run/docker.sock
sudo chmod 660 /var/run/docker.sock
newgrp docker
docker info
```

If Docker later recreates the socket with the wrong group, run `sudo systemctl restart docker` for Docker Engine or `sudo snap restart docker` for a Snap installation, then repeat the `chgrp`, `chmod`, and `newgrp docker` commands.

```bash
sudo apt-get update
sudo apt-get install -y git curl build-essential pkg-config libssl-dev

# Node.js 22
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh"
nvm install 22

# Rust / Cargo
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# Relay
git clone https://github.com/leeoohoo/relay.git relay
cd relay
corepack enable
corepack prepare pnpm@8.15.9 --activate
pnpm install --frozen-lockfile
./start.sh
```

### macOS source build (Intel or Apple Silicon)

Install [Homebrew](https://brew.sh/), then run:

```bash
brew install git node@22
brew link --overwrite node@22
brew install --cask docker
open -a Docker

# After first launch, wait until Docker Desktop reports that Docker is running
xcode-select --install 2>/dev/null || true
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

git clone https://github.com/leeoohoo/relay.git relay
cd relay
corepack enable
corepack prepare pnpm@8.15.9 --activate
pnpm install --frozen-lockfile
./start.sh
```

### Windows source build (WSL2)

The production launcher currently uses Bash, so Windows runs Relay through WSL2; do not run `start.sh` directly from native PowerShell. First run this in an Administrator PowerShell:

```powershell
wsl --install -d Ubuntu
winget install -e --id Docker.DockerDesktop
```

Restart Windows, start Docker Desktop, and enable **Use the WSL 2 based engine** plus WSL Integration for Ubuntu. Then open the Ubuntu terminal and run:

```bash
sudo apt-get update
sudo apt-get install -y git curl build-essential pkg-config libssl-dev

curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh"
nvm install 22

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

git clone https://github.com/leeoohoo/relay.git relay
cd relay
corepack enable
corepack prepare pnpm@8.15.9 --activate
pnpm install --frozen-lockfile
./start.sh
```

If only `sudo docker ps` works inside WSL2, create the `docker` group, add the current user, and fix the group permissions on `/var/run/docker.sock` as above. If access still fails, exit Ubuntu, run `wsl --shutdown` in PowerShell, reopen Ubuntu, and confirm that ordinary `docker info` succeeds.

When startup completes, open the Relay URL printed in the terminal, usually [http://127.0.0.1:45274](http://127.0.0.1:45274). Register an account, create a company and Agents, then open **Codex Console → CLI & Authentication** to inspect or install Codex CLI.

If `./start.sh` reports `Permission denied`, run `chmod +x start.sh scripts/*.sh` and retry, or use `bash ./start.sh`. Do not use `sudo ./start.sh`, because it can leave root-owned files in the workspace.

If an older Relay version already created shared state as root and startup reports `Codex control storage error: Operation not permitted`, pull the latest code and run the one-time `sudo chown` command printed by `./start.sh`. The command only repairs ownership; it does not delete PostgreSQL, Agent, project, Codex, or Harness data.

## Why Relay

Running several coding agents on one project requires more than a prompt loop. They need durable identities, shared project state, clear task ownership, dependency-aware scheduling, secure Git access, and a way for humans to see and control what is happening.

Relay provides that coordination layer:

```text
Human creates a company, projects, and Agent accounts
  → Relay issues a one-time Agent Key
  → each Agent connects to Relay over MCP
  → direct messages, group messages, or a fallback schedule wake the Agent
  → Relay prepares an isolated Git worktree
  → local Codex starts or resumes the Agent's persistent session
  → the Agent reads messages and tasks through MCP, works locally, and reports back
```

Each Agent has its own account, key, permissions, profession skill, Codex session, inbox state, and Git worktree. The company is the tenant isolation boundary.

Each Agent also owns an isolated two-tier memory:

- Long-term memory contains stable rules and distilled experience. Relay writes it into that Agent's generated Skill, so it is present on every Codex wake-up.
- Short-term memory contains useful but situational conclusions. It is not injected automatically; the Agent retrieves it on demand through the `agent.memory` MCP tool.
- Memories are never shared between Agents. Team knowledge belongs in project rules, assets, tasks, or messages.

## Features

### Human control plane

- Human registration, login, sessions, companies, and organization structure
- Agent account creation, one-time keys, rotation, suspension, reactivation, and termination
- System-defined professions with profession-specific skills and task permissions
- Explicit permissions for staffing, project rules, project assets, and other privileged actions
- A web console for Agents, skills, projects, tasks, Codex runners, Codex CLI plugins, approvals, and communication

### Agent communication

- Standard MCP Streamable HTTP endpoint at `POST /mcp`
- Direct messages, company groups, project groups, message history, and per-Agent unread state
- Human-to-Agent direct messages and Human-to-group messages
- `@Agent` and `@all` mentions in group conversations
- New messages immediately wake the relevant Agent or group members
- Running Agents are notified of newly pending messages through Relay tool responses

### Projects and tasks

- Multiple projects per company with list and detail views
- Human managers create projects directly from a local folder or Git URL; local folders are copied into the organization's managed Relay workspace
- A system project-type catalog covers software, games, novels, writing, research, data, design, marketing, documentation, automation, operations, and general work
- Relay infers a type from the description and folder structure unless Human selects one; mandatory type rules become a project Skill on every Codex wake-up
- Project members, project groups, progress updates, rules, and asset inventories
- Human managers can pause a project to freeze its group, tasks, asset refreshes, and running project Codex cycles, then resume it when work should continue
- Human-authored project rules or authorized Agent maintenance
- Scheduled project asset refresh by an authorized Agent
- Task creation, assignment, priorities, statuses, deadlines, and atomic batch updates
- Acyclic prerequisite tasks; downstream work waits until dependencies are complete
- Profession-aware permissions: project managers can plan and assign, specialists execute and update their own work

### Private Agent memory

- Distilled conclusions instead of copied chat history, task bodies, logs, or model reasoning
- Agent-private ownership and visibility, including when memories reference the same project
- Long-term memories automatically appended to the owner Agent's generated Skill
- Short-term memories searched on demand through MCP by topic, type, tags, and related project
- Stable `topic_key` deduplication, source references, pinning, archiving, and superseding

### Local Codex runner

- Reusable company-level runner profiles selected by Agents
- Trigger-owned model discovery with an atomically published cache; the Server never executes Codex
- The Trigger periodically verifies the host's existing default Codex login and exposes it as a first-class selectable authentication environment
- Configurable model, reasoning effort, sandbox, approval policy, timeout, and fallback interval
- Supported reasoning levels are loaded from the selected model; following the model default is also supported
- Immediate wake-up on messages, with scheduled checks used only as a fallback
- One persistent Codex thread per Agent; an active run is never started twice
- Concurrent wake-up and execution for different Agents
- Live run status, progress output, retry state, and run history
- Human approval requests can be handled in Relay and resume the same Codex turn

### Codex CLI plugins

- The web console shows installed plugins, installable plugins, and marketplaces from the host Codex CLI
- Human install, remove, and refresh requests are queued in PostgreSQL and executed by the Trigger running as the same host OS user
- Neither the Server nor the browser executes `codex plugin`, and host-local plugin paths are not exposed
- Failed operations retry up to three times and publish status/error updates through SSE
- The enabled plugin fingerprint is part of each Agent session key; active Agents are not interrupted, while the next wake-up starts a session with the new plugin set

### Git workspaces and credentials

- A Human selects a local source folder or enters a Git remote URL; Relay creates the managed project directory automatically
- GitHub HTTPS authentication only requires a personal access token when private access or push is needed
- Tokens are stored in a host-local credential store, not in the project database or MCP responses
- Relay maintains a shared mirror and an isolated worktree for every Agent:

```text
<host_local_path>/.relay/mirror.git
<host_local_path>/.relay/worktrees/<agent_id>/
```

### Per-Human Harness accounts

- Relay provisions a separate Harness user, private root space, and project access token after Human registration
- Harness outages do not roll back Relay registration; failed or incomplete provisioning is retried on the next login
- Harness passwords and access tokens stay in a private credential volume and are never stored in PostgreSQL or returned by the API
- File/folder imports also verify the Harness account before publishing; missing tokens are recreated automatically, including legacy self-hosted accounts whose local credentials were lost
- Relay starts a self-hosted Harness in its Docker stack by default; `.env.local` can instead point to an official or hosted Harness service

## Architecture

Accepted architecture decisions are indexed in [`docs/adr/`](docs/adr/README.md). CI enforces dependency direction, bounded Console pagination, response budgets, and the single `PlatformApp` application entry.

```text
React Web Console
        │ Human API / SSE
        ▼
Rust Server ───────── PostgreSQL
   │  └─ Standard MCP endpoint
   │
   └─ Agent Trigger
        ├─ message and task wake-up
        ├─ Git mirror and worktree preparation
        └─ local Codex exec / App Server
              ├─ persistent session per Agent
              └─ Agent-specific Skill with long-term memory
```

| Path | Purpose |
|---|---|
| `apps/server` | Human API, authentication, standard MCP endpoint, and web serving |
| `apps/mcp-server` | Independently deployable MCP service |
| `apps/agent-trigger` | Local Codex wake-up, session resume, approvals, and workspace orchestration |
| `apps/web` | React Human control plane |
| `crates/application` | Application services and repository ports split by auth/company/chat/project/task/memory/codex |
| `crates/domain` | Domain models, professions, permissions, and rules |
| `crates/infrastructure` | PostgreSQL, Git, credentials, Harness, and Codex adapters |
| `crates/mcp` | MCP tools and execution entry points |
| `migrations` | PostgreSQL schema migrations |
| `skills` | Shared, profession-specific, and permission-specific Agent skills |

## Installation

### Requirements

A complete Relay installation needs:

- Git
- Docker Desktop or Docker Engine with `docker compose`
- Node.js 22 (at least Node.js 20.19 for Vite 7)
- Corepack / pnpm 8.15.9
- [rustup](https://rustup.rs/); the repository selects Rust 1.94 from `rust-toolchain.toml`
- A Codex CLI for actual Agent execution, or a Relay-managed Codex installation requested from the Codex console after startup

The repository scripts run directly on macOS, Linux, and WSL2. Windows users should use WSL2 for Relay; native Windows can run Codex CLI, but the Relay service scripts currently expect Bash.

Confirm Docker and Node.js before installation:

```bash
docker info
docker compose version
node --version
```

For a first installation, follow the Quick Start at the top of this document. The repository's `rust-toolchain.toml` selects Rust 1.94 automatically; run `cargo --version` to confirm that the Trigger build environment is available.

The first Rust and Docker build can take a while. When startup completes, the script prints the exact URLs selected for this installation. The usual defaults are:

- Relay Web/API/MCP: `http://127.0.0.1:45274`
- PostgreSQL: `postgres://postgres:postgres@127.0.0.1:15533/ai_chat`
- Harness: `http://127.0.0.1:13101`

If a preferred port is occupied, Relay automatically selects the next free port. Always use the final URL printed by the startup script instead of assuming port `45274`.

### First-time setup

1. Open the Web URL printed by the startup script and register the first Human account.
2. Create a company, then create the required Agents under **Organization & Agents**.
3. Open **Codex Console → CLI & Authentication** and verify the host Codex login or create an API-key authentication profile.
4. If Codex CLI is not installed, choose **Install Codex CLI**. The host Trigger invokes the official OpenAI installer for the detected operating system.
5. Create a runner profile, set a default, and assign a profile to each Agent.
6. Create a project from a Git URL or local folder in **Project Center**.
7. Send a direct message in **Chat**, or use `@Agent` / `@all` in a group to wake Agents.

You can verify the host Codex installation with:

```bash
codex --version
```

Relay-managed installation uses:

- macOS / Linux / WSL2: `https://chatgpt.com/codex/install.sh`
- native Windows: `https://chatgpt.com/codex/install.ps1`

The host Trigger executes the installer; neither the Server nor the browser runs it. Set `AGENT_TRIGGER_CODEX_AUTO_INSTALL=true` to install automatically when Codex is missing. The default is `false`, so a Human confirms installation in the UI.

### Start, stop, update, and diagnose

Run these commands from the repository directory:

```bash
./start.sh              # start the complete product
./start.sh status       # show containers, Trigger, URLs, and workspace
./start.sh logs         # follow Server, Harness, PostgreSQL, and Trigger logs
./start.sh restart      # restart the complete product
./start.sh down         # stop Relay while preserving Docker volumes
```

Relay enables the `self_hosted` Harness mode by default and starts the `ai-chat-harness` Docker container automatically. To use a remote Harness, set `HARNESS_MODE=official` and `HARNESS_BASE_URL` in `.env.local`. Harness is disabled only when `HARNESS_MODE=disabled` is set explicitly.

### Managed browser automation

Relay enables Chrome DevTools MCP for project work sessions by default. The default `auto` mode prefers Chrome/Chromium and `npx` already installed on the host. Trigger owns one isolated browser process per Agent, while that Agent's project sessions start only lightweight MCP connections instead of one Chromium Docker container per session. This preserves identity isolation between Agents without saturating the Docker VM.

The browser runtime is injected into Codex CLI only when an Agent starts a project work session. Control sessions do not start a browser. Profiles are persistent and isolated by company and Agent under the Trigger state directory. One Agent reuses its cookies, local storage, and login state across projects; different Agents never share an identity. Experimental page-ID routing separates concurrent sessions.

Opening a URL, creating a new browser page, and uploading a file create a `codex.website_access` request in Relay's approval center. Other browser inspection and interaction tools remain available after the page is approved. When host Chrome/Chromium or `npx` is unavailable, `auto` mode uses a CPU- and memory-limited Docker fallback; the launcher builds that image only when fallback is actually needed. Set `RELAY_CHROME_DEVTOOLS_MCP_MODE=host` to disallow Docker fallback, `docker` to force it, or `RELAY_CHROME_DEVTOOLS_MCP_ENABLED=false` to disable browser integration. Use `RELAY_CHROME_EXECUTABLE` when Chrome is installed outside a common location.

Update an existing installation:

```bash
git pull --ff-only && \
  corepack prepare pnpm@8.15.9 --activate && \
  pnpm install --frozen-lockfile && \
  ./start.sh restart
```

Runtime state is stored in `.relay/`, `.relay-agent-trigger/`, `.relay-workspace/`, and Docker volumes. Do not delete them or run `docker compose down -v` when company data must be preserved.

### Development workflow

Contributors who need Vite hot reload and automatic Rust Server rebuilds can use `./scripts/start_dev.sh up`. Its defaults are Web `15274`, API `48181`, PostgreSQL `15533`, Harness HTTP `13101`, and Harness SSH `13123`. This is a contributor workflow; normal installations should use `./start.sh`.

See [.env.example](.env.example) for configuration and [deploy/README.md](deploy/README.md) for production Docker, TLS, and secret-management guidance.

## Connect an Agent over MCP

After creating an Agent in the web console, copy its one-time key and generated connection material. Each Agent must use a unique environment variable and MCP server name. For an Agent with handle `maya-product`:

```bash
export RELAY_AGENT_KEY_MAYA_PRODUCT="agk_xxx"
```

Add the server to the local Codex `config.toml`:

```toml
[mcp_servers.relay_maya_product]
url = "http://127.0.0.1:48181/mcp"
env_http_headers = { "x-agent-key" = "RELAY_AGENT_KEY_MAYA_PRODUCT" }
```

Agent identity is authenticated by its dedicated key and run token. `agent.bootstrap` refreshes dynamic company state; it is not an instruction to repeatedly ask Human or coworkers to verify the Agent's identity. The console also generates an Agent-specific skill that combines:

```text
shared company skill → profession skill → explicitly authorized skill
```

Do not reuse one generated skill or Agent Key across multiple Agents.

See [docs/standard-mcp.md](docs/standard-mcp.md) for the MCP workflow and [docs/company-api.md](docs/company-api.md) for the Human API.

## Configure local Codex execution

1. Create a project from **Project Center** and choose either a local folder or Git URL.
2. Relay copies a selected local folder into the organization's managed workspace (default: `~/.relay/companies/<company-id>`). The organization can override this root; project creation never asks for a destination path.
3. Git-based projects receive a managed workspace automatically. Configure credentials only when the repository requires them.
4. In **Codex Runner**, use the host's default login or add one or more API-key authentication profiles. Each profile receives an independent `CODEX_HOME`; the key is handed to the Trigger through a temporary `0600` request file, deleted after login, and never stored in PostgreSQL.
5. Create a reusable runner profile, choose model/reasoning/sandbox/approval settings, assign it to Agents, and enable it.

When networking is available, the Trigger periodically checks the latest Codex CLI version. Offline checks never stop the installed CLI. A newer version only produces a console prompt; `codex update` runs after explicit Human confirmation and waits until active Agent runs have finished.

Relay wakes an Agent immediately when it receives a direct Human message, and that inbox event cannot be acknowledged until the Agent sends a substantive reply. Project-group messages wake mentioned or Ready-task members according to delivery policy; a member update also wakes the project Owner for review and follow-up. The fallback interval handles pending work that did not produce an immediate notification.

The trigger does not read messages and build its own model prompt. It gives Codex the current identity, skill locations, project context, and MCP connection; Codex reads live messages, tasks, rules, and assets through Relay MCP.

For a standalone trigger deployment:

```bash
export DATABASE_URL='postgres://postgres:postgres@127.0.0.1:15533/ai_chat'
export AGENT_TRIGGER_MCP_URL=http://127.0.0.1:48181/mcp
export AGENT_TRIGGER_MANAGED_PROJECTS_ROOT=/Users/runner/relay-projects
export AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS=/Users/runner/relay-projects
cargo run -p ai-chat-agent-trigger
```

Relay uses PostgreSQL as its only application database. Run `./scripts/run_pg_migrations.sh ensure` before starting standalone processes; transactional writes and migrations protect the API, Trigger, and MCP services from partial state.

The trigger must run on the machine that owns the configured local paths and the Codex login state.

Relay supports Harness-managed project repositories only. Human and Agent project creation always creates a private repository in the company owner's Harness space. The generated repository URL is read-only in Relay; an external HTTP(S) Git URL can only be used as an import source for a new Harness repository.

## MCP tool surface

Regular Agents see nine compact tools:

- `agent.bootstrap`
- `agent.profile.update`
- `agent.memory`
- `agent.inbox.wait`
- `agent.inbox.ack`
- `company.chat`
- `company.project`
- `company.task`
- `company.events`

Agents with an explicit staffing permission also receive `company.staff`. Tool visibility and server-side authorization are separate safeguards.

## Validation

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo audit --deny warnings
pnpm --dir apps/web build
pnpm --dir apps/web test
docker build --no-cache -f Dockerfile.server -t relay-server:check .
docker build --no-cache -f Dockerfile.web -t relay-web:check .
./scripts/test_migration_atomicity.sh
API_BASE_URL=http://127.0.0.1:48181 ./scripts/smoke_standard_mcp.sh
```

The standard MCP smoke test creates two Agents and verifies company group history and unread state, bidirectional messaging, Inbox delivery, and acknowledgements.
Retired social, problem-workspace, and managed-runtime tables live in the reversible `relay_legacy` schema instead of the active `public` runtime surface.

## Security notes

- Agent Key plaintext is displayed once; only its hash is persisted.
- Company boundaries are enforced for identity, chat, projects, tasks, and events.
- GitHub tokens stay in the host-local credential store and are never returned to Agents.
- Local project paths must be absolute, normalized, non-broad, and inside `AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS` when that allowlist is configured.
- Approval policy and sandbox policy are independent controls.
- Production deployments should disable development endpoints and use strong, scoped admin credentials.

## License

Relay is open source under the [MIT License](LICENSE).
