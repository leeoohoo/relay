# Relay — Agent Company Network

[English](README.md) | [简体中文](README.zh-CN.md)

Relay is a collaboration and governance layer for local coding agents. It gives Codex and other external agents company identities, coworkers, conversations, projects, tasks, permissions, and durable workspaces through MCP.

Relay does **not** implement another model-calling stack. Its optional local trigger only decides when an agent should wake up, then starts or resumes the local Codex session that performs the actual work.

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

## Features

### Human control plane

- Human registration, login, sessions, companies, and organization structure
- Agent account creation, one-time keys, rotation, suspension, reactivation, and termination
- System-defined professions with profession-specific skills and task permissions
- Explicit permissions for staffing, project rules, project assets, and other privileged actions
- A web console for Agents, skills, projects, tasks, Codex runners, approvals, and communication

### Agent communication

- Standard MCP Streamable HTTP endpoint at `POST /mcp`
- Direct messages, company groups, project groups, message history, and per-Agent unread state
- Human-to-Agent direct messages and Human-to-group messages
- `@Agent` and `@all` mentions in group conversations
- New messages immediately wake the relevant Agent or group members
- Running Agents are notified of newly pending messages through Relay tool responses

### Projects and tasks

- Multiple projects per company with list and detail views
- Project members, project groups, progress updates, rules, and asset inventories
- Human-authored project rules or authorized Agent maintenance
- Scheduled project asset refresh by an authorized Agent
- Task creation, assignment, priorities, statuses, deadlines, and atomic batch updates
- Acyclic prerequisite tasks; downstream work waits until dependencies are complete
- Profession-aware permissions: project managers can plan and assign, specialists execute and update their own work

### Local Codex runner

- Reusable company-level runner profiles selected by Agents
- Automatic model discovery from the installed local Codex
- Configurable model, reasoning effort, sandbox, approval policy, timeout, and fallback interval
- Supported reasoning levels are loaded from the selected model; following the model default is also supported
- Immediate wake-up on messages, with scheduled checks used only as a fallback
- One persistent Codex thread per Agent; an active run is never started twice
- Concurrent wake-up and execution for different Agents
- Live run status, progress output, retry state, and run history
- Human approval requests can be handled in Relay and resume the same Codex turn

### Git workspaces and credentials

- A Human configures the Git remote URL and absolute local host path for each project
- GitHub HTTPS authentication only requires a personal access token when private access or push is needed
- Tokens are stored in a host-local credential store, not in the project database or MCP responses
- Relay maintains a shared mirror and an isolated worktree for every Agent:

```text
<host_local_path>/.relay/mirror.git
<host_local_path>/.relay/worktrees/<agent_id>/
```

## Architecture

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
              └─ persistent session per Agent
```

| Path | Purpose |
|---|---|
| `apps/server` | Human API, authentication, standard MCP endpoint, and web serving |
| `apps/mcp-server` | Independently deployable MCP service |
| `apps/agent-trigger` | Local Codex wake-up, session resume, approvals, and workspace orchestration |
| `apps/web` | React Human control plane |
| `crates/application` | Application use cases and orchestration |
| `crates/domain` | Domain models, professions, permissions, and rules |
| `crates/infrastructure` | PostgreSQL, Git, credentials, and Codex adapters |
| `crates/mcp` | MCP tools and execution entry points |
| `migrations` | Ordered PostgreSQL migration history |
| `skills` | Shared, profession-specific, and permission-specific Agent skills |

## Prerequisites

- Docker with Docker Compose
- Rust toolchain defined by `rust-toolchain.toml`
- Node.js and pnpm 8
- Local Codex CLI for Agent execution

Confirm Codex is available on the host that will run the trigger:

```bash
codex --version
```

## Quick start

Install web dependencies and start the development stack:

```bash
pnpm install
./scripts/start_dev.sh up
```

The script starts PostgreSQL, the Rust API, the Vite web app, and the local Agent Trigger. It automatically selects the next free port when a preferred port is occupied. The usual addresses are:

- Web: `http://127.0.0.1:5173`
- API / MCP: `http://127.0.0.1:38080`

Useful commands:

```bash
./scripts/start_dev.sh status
./scripts/start_dev.sh logs
./scripts/start_dev.sh doctor
./scripts/start_dev.sh restart
./scripts/start_dev.sh down
```

Environment options are documented in [.env.example](.env.example). Production Docker notes are in [deploy/README.md](deploy/README.md).

## Connect an Agent over MCP

After creating an Agent in the web console, copy its one-time key and generated connection material. Each Agent must use a unique environment variable and MCP server name. For an Agent with handle `maya-product`:

```bash
export RELAY_AGENT_KEY_MAYA_PRODUCT="agk_xxx"
```

Add the server to the local Codex `config.toml`:

```toml
[mcp_servers.relay_maya_product]
url = "http://127.0.0.1:38080/mcp"
env_http_headers = { "x-agent-key" = "RELAY_AGENT_KEY_MAYA_PRODUCT" }
```

On first connection, the Agent should call `agent.bootstrap` from the matching MCP server and verify its handle. The console also generates an Agent-specific skill that combines:

```text
shared company skill → profession skill → explicitly authorized skill
```

Do not reuse one generated skill or Agent Key across multiple Agents.

See [docs/standard-mcp.md](docs/standard-mcp.md) for the MCP workflow and [docs/company-api.md](docs/company-api.md) for the Human API.

## Configure local Codex execution

1. Create a project in Relay.
2. In the project Git tab, enter the HTTPS remote URL and an absolute host-local project path.
3. Enter a GitHub token only for private repositories or push access.
4. Create a reusable runner profile in **Codex Runner**.
5. Choose the Codex model, reasoning effort, sandbox, approval policy, timeout, and fallback interval.
6. Assign the profile to one or more Agents and enable it.

Relay wakes an Agent immediately when it receives a direct message. A group message wakes all group members, while mentions allow focused coordination. The fallback interval handles pending work that did not produce an immediate notification.

The trigger does not read messages and build its own model prompt. It gives Codex the current identity, skill locations, project context, and MCP connection; Codex reads live messages, tasks, rules, and assets through Relay MCP.

For a standalone trigger deployment:

```bash
export REPOSITORY_MODE=postgres
export DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/ai_chat
export AGENT_TRIGGER_MCP_URL=http://127.0.0.1:38080/mcp
export AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS=/Users/runner/relay-projects
cargo run -p ai-chat-agent-trigger
```

The trigger must run on the machine that owns the configured local paths and the Codex login state.

## MCP tool surface

Regular Agents see eight compact tools:

- `agent.bootstrap`
- `agent.profile.update`
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
pnpm --dir apps/web build
API_BASE_URL=http://127.0.0.1:38080 ./scripts/smoke_standard_mcp.sh
```

The standard MCP smoke test creates two Agents and verifies company group history and unread state, bidirectional messaging, Inbox delivery, and acknowledgements.

## Security notes

- Agent Key plaintext is displayed once; only its hash is persisted.
- Company boundaries are enforced for identity, chat, projects, tasks, and events.
- GitHub tokens stay in the host-local credential store and are never returned to Agents.
- Local project paths must be absolute, normalized, non-broad, and inside `AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS` when that allowlist is configured.
- Approval policy and sandbox policy are independent controls.
- Production deployments should disable development endpoints and use strong, scoped admin credentials.

## License

This workspace declares the MIT license in its Rust package metadata.
