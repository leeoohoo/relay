# 定时触发 Codex + 多项目 Git 协作实施方案

更新日期：2026-07-27

## 1. 最终产品边界

Relay 不实现新的大模型调用层，也不把消息拼成 OpenAI API 请求。

Relay Trigger 只做以下事情：

1. 定时检查某个 Agent 是否有 pending Inbox、已分配未完成任务或 Human 手动唤醒请求。
2. 按项目配置准备本地 Git mirror 和该 Agent 的隔离 worktree。
3. 为本次运行签发短期 Relay MCP Run Token。
4. 启动本机 `codex exec --json`，或恢复该 Agent 已有的 Codex thread。
5. 记录进程成功、失败、超时、thread ID 和审计信息。

Codex 自己负责：

- 调用 Relay MCP 的 `agent.bootstrap`、`agent.inbox.wait` 获取真实上下文。
- 理解消息和任务。
- 修改代码、运行测试、commit/push。
- 通过 Relay MCP 回复消息、更新任务和 ack Inbox。

明确不做：

- 不调用 Responses API。
- 不复用 `AgentRuntimeModelProvider`。
- 不要求 Codex 输出 Relay 自定义行动 JSON。
- 不解析 Codex 最终回答后代替 Agent 执行动作。
- 不由 Trigger 自动 ack 消息。

## 2. 多项目配置模型

Human 管理台的“项目 Git”是项目列表。公司中的每个项目独立配置：

| 字段 | 填写者 | Agent 可见 | 用途 |
| --- | --- | --- | --- |
| `remote_url` | Human Owner/Admin | 是 | Git 远程地址 |
| `default_branch` | Human Owner/Admin | 是 | 默认分支 |
| `host_local_path` | Human Owner/Admin | 否 | Trigger 宿主机上的项目根目录 |
| `auth_profile` | Human Owner/Admin | 否 | 宿主机预注册的凭证标签 |
| `allow_agent_push` | Human Owner/Admin | 是 | 项目 push 策略 |
| `branch_prefix` | Human Owner/Admin | 是 | Agent 工作分支前缀 |

Git 配置只能通过 Human API 写入：

```text
GET    /api/v1/companies/{company_id}/projects/{project_id}/git
PUT    /api/v1/companies/{company_id}/projects/{project_id}/git
DELETE /api/v1/companies/{company_id}/projects/{project_id}/git
```

Agent MCP 只能读取安全 Git 坐标，不提供修改 Git 配置的入口，也不返回 `host_local_path` 或 `auth_profile`。

## 3. 宿主机路径设计

Human 为每个项目填写绝对路径，例如：

```text
/Users/runner/relay-projects/payment-service
```

Trigger 在该路径下维护：

```text
<host_local_path>/.relay/workspace.lock
<host_local_path>/.relay/mirror.git
<host_local_path>/.relay/worktrees/<agent_id>/
```

安全规则：

- 必须是绝对路径。
- 拒绝 `.`、`..`、控制字符和前后空白。
- 拒绝文件系统根目录和 Trigger 用户的 HOME。
- 可通过 `AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS` 限制允许的父目录。
- Trigger 创建目录后再次 canonicalize，防止符号链接逃逸允许根目录。
- 多个 Agent 不共享可写 checkout；每个 Agent 使用自己的 worktree。

没有绑定 Git 项目的消息处理使用独立 general workspace，并初始化为本地 Git 仓库，以满足 Codex 非交互模式的 Git 工作目录要求。

## 4. 每个 Agent 固定一个 Codex 会话

会话表按 Agent 唯一保存，不按项目拆分：

```text
agent_codex_sessions
  agent_profile_id PRIMARY KEY
  current_project_id UUID NULL
  codex_thread_id TEXT
  worktree_key TEXT
  last_used_at TIMESTAMPTZ
```

每次触发：

1. 查询 `agent_profile_id` 对应的唯一 session。
2. 存在 thread ID 时执行 `codex exec ... resume <thread_id> <prompt>`。
3. 如果恢复在 turn 开始前明确报告 session/thread 无法恢复，只回退一次新建 thread。
4. 如果恢复后已经开始执行 turn，即使后续失败也不新建 thread，避免同一工作重复执行。
5. 新 thread 成功后覆盖保存，以后的项目切换继续使用这个 thread。
6. `current_project_id` 和 `worktree_key` 只记录最近一次运行位置，不决定 session 粒度。

## 5. Codex 启动方式

采用官方稳定的非交互入口：

```text
codex exec --json
codex exec --json ... resume <SESSION_ID>
```

Trigger 动态加入：

- `--sandbox read-only` 或 `--sandbox workspace-write`
- `--ask-for-approval never`
- required Relay MCP Server
- `env_http_headers` 注入短期 `x-agent-run-token`
- `shell_environment_policy.exclude`，避免 Run Token 进入模型启动的 Shell 子进程

`codex_profile=default` 表示使用本机 Codex 基础配置，不传 `--profile`；填写其他值时，必须对应宿主机 `$CODEX_HOME/<profile>.config.toml`。

Trigger 解析 JSONL 事件：

```text
thread.started
turn.started
turn.completed
turn.failed
item.completed(agent_message)
error
```

JSONL 仅用于生命周期、thread ID、最终摘要和错误审计，不用于解释业务行动。

## 6. 调度与租约

独立服务：

```text
apps/agent-trigger
```

运行循环：

1. PostgreSQL 使用 `FOR UPDATE SKIP LOCKED` claim 到期 Trigger。
2. 租约长度为 `max_run_seconds + 60`。
3. 检查 Agent/公司是否 active。
4. 检查 pending Inbox 和 active assigned tasks。
5. 无工作则释放租约并安排下一次检查，不启动 Codex。
6. 有工作则创建 run 审计记录、准备工作区、签发短期 Token、运行 Codex。
7. 成功时保存 Agent 固定 session。
8. 无论成功失败都撤销 Token、更新 run、释放租约。
9. 连续失败 3 次后 Trigger 进入 `error`，由 Human 修复后恢复。

Human API：

```text
GET  /api/v1/companies/{company_id}/agents/{agent_id}/codex-trigger
PUT  /api/v1/companies/{company_id}/agents/{agent_id}/codex-trigger
POST /api/v1/companies/{company_id}/agents/{agent_id}/codex-trigger/pause
POST /api/v1/companies/{company_id}/agents/{agent_id}/codex-trigger/resume
POST /api/v1/companies/{company_id}/agents/{agent_id}/codex-trigger/run-now
GET  /api/v1/companies/{company_id}/agents/{agent_id}/codex-runs
```

## 7. 配置

主要环境变量：

```text
AGENT_TRIGGER_MCP_URL
AGENT_TRIGGER_MCP_SERVER_NAME
AGENT_TRIGGER_CODEX_BIN
AGENT_TRIGGER_CODEX_PREFIX_ARGS_JSON
AGENT_TRIGGER_CODEX_ENV_ALLOWLIST
AGENT_TRIGGER_CODEX_SENSITIVE_ENV_NAMES
AGENT_TRIGGER_POLL_INTERVAL_SECONDS
AGENT_TRIGGER_BATCH_SIZE
AGENT_TRIGGER_INSTANCE_ID
AGENT_TRIGGER_GENERAL_WORKSPACE_ROOT
AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS
AGENT_TRIGGER_ALLOWED_GIT_HOSTS
AGENT_TRIGGER_GIT_AUTH_PROFILES_JSON
```

启动示例：

```bash
export DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/ai_chat
export AGENT_TRIGGER_MCP_URL=http://127.0.0.1:38080/mcp
export AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS=/Users/runner/relay-projects
cargo run -p ai-chat-agent-trigger
```

Trigger 应运行在宿主机，不建议放入默认 Docker 服务，因为它需要访问 Human 配置的宿主机绝对路径、本地 Git 凭证和本机 Codex 登录态。

## 8. 已实施文件

- `migrations/0033_project_git_and_codex_triggers/`
- `crates/domain/src/company.rs`
- `crates/application/src/service.rs`
- `crates/application/src/memory.rs`
- `crates/infrastructure/src/postgres.rs`
- `crates/infrastructure/src/git_workspace.rs`
- `crates/infrastructure/src/codex_trigger.rs`
- `apps/agent-trigger/`
- `apps/server/src/main.rs`
- `apps/web/src/pages/App.tsx`
- `apps/web/src/styles.css`
- `.env.example`
- `README.md`

## 9. 验证方案

自动验证：

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
npm run build --prefix apps/web
```

Fake Codex 测试覆盖：

- JSONL thread/turn/final message 解析。
- 恢复失败时只创建一次新 thread。
- turn 已开始后的失败不误判为 session 恢复失败。
- general workspace 自动初始化 Git。
- 项目路径必须位于允许的宿主机根目录之下。

真实 smoke 前置条件：

```bash
codex --version
codex exec --json "return ok"
```

当前开发机的 npm `codex` 入口存在，但对应 macOS arm64 平台二进制缺失并报 `ENOENT`。因此实现使用 Fake Codex 完成自动测试；安装完整 Codex CLI 后再执行真实 session resume smoke。
