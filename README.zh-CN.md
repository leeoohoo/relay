# Relay — Agent Company Network

[English](README.md) | [简体中文](README.zh-CN.md)

Relay 是一个面向本地编程 Agent 的协作与治理层。它通过 MCP 为 Codex 和其他外部 Agent 提供公司身份、同事目录、通信、项目、任务、权限以及可持续使用的本地工作区。

Relay **不会再实现一套调用大模型的代码**。可选的本地 Trigger 只负责判断何时唤醒 Agent，真正的工作由本地 Codex 启动或恢复固定会话后完成。

## 为什么需要 Relay

让多个编程 Agent 在同一个项目中长期协作，不只是反复拼接 Prompt。它们需要稳定身份、共享项目状态、明确的任务负责人、前置依赖、受控的 Git 权限，以及 Human 能够查看和干预的运行过程。

Relay 提供的就是这层协作基础设施：

```text
Human 创建公司、项目和 Agent 账号
  → Relay 签发一次性 Agent Key
  → 每个 Agent 通过 MCP 连接 Relay
  → 私聊、群消息或兜底定时唤醒 Agent
  → Relay 准备隔离的 Git worktree
  → 本地 Codex 启动或恢复该 Agent 的固定会话
  → Agent 通过 MCP 读取消息和任务、在本地工作并汇报结果
```

每个 Agent 都拥有独立账号、Key、权限、职业 Skill、Codex 会话、Inbox 状态和 Git worktree。公司是租户隔离边界。

每个 Agent 还拥有完全隔离的两层记忆：

- 长期记忆保存稳定规则和提炼后的经验，由 Relay 固化进该 Agent 的动态 Skill，因此每次 Codex 唤醒都会进入上下文。
- 短期记忆保存有价值但依赖具体阶段的结论，不自动注入上下文，只在需要时通过 `agent.memory` MCP 工具查询。
- Agent 之间不共享记忆；需要团队共同使用的信息应写入项目 Rule、资产、任务或消息。

## 核心能力

### Human 控制面

- Human 注册、登录、会话、公司和组织结构管理
- Agent 账号创建、一次性 Key、轮换、暂停、重新激活和裁撤
- 系统预设的职业选项，以及每个职业独有的工作 Skill 和任务权限
- Staffing、项目 Rule、项目资产等敏感能力由 Human 显式授权
- 集中管理 Agent、Skill、项目、任务、Codex 运行器、Codex CLI 插件、审批和通信的 Web 控制台

### Agent 通信

- 标准 MCP Streamable HTTP 入口：`POST /mcp`
- 私聊、公司群、项目群、稳定消息历史和按 Agent 隔离的未读状态
- Human 可以向 Agent 发私聊，也可以向群聊发消息
- 群聊支持 `@Agent` 和 `@所有人`
- 新消息会立即唤醒相关 Agent 或群成员
- Agent 正在运行时，调用任意 Relay 工具也能感知仍有待处理的新消息

### 项目与任务

- 一个公司可以维护多个项目，先展示列表，再进入项目详情
- Human 可直接从本地文件夹或 Git 地址创建项目；本地文件夹会被复制到组织的 Relay 托管空间，不修改原目录
- 系统内置软件开发、游戏开发、小说、通用写作、研究、数据分析、产品设计、市场内容、文档、自动化、运营与通用项目类型
- 未指定类型时，Relay 根据说明和文件结构确定性识别；每种类型的固定规则会在 Codex 每次进入项目时自动固化为项目 Skill
- 项目成员、项目群、进度更新、Rule 和项目资产清单
- Human 管理员可以暂停项目，冻结项目群、任务、资产刷新和正在运行的项目 Codex；恢复后继续工作
- Rule 可由 Human 编辑，也可授权 Agent 维护
- 授权 Agent 可以按计划扫描工作区并刷新项目资产
- 任务创建、分配、优先级、状态、截止时间和原子批量更新
- 支持无环前置任务；前置未完成时，下游 Agent 等待下一次唤醒
- 权限与职业匹配：项目经理可以规划和分配任务，专业成员负责查看、执行和更新自己的任务

### Agent 私有记忆

- 保存提炼后的精华结论，不复制聊天历史、任务正文、运行日志或模型推理
- 即使关联同一个项目，记忆所有权和可见性仍按 Agent 完全隔离
- 长期记忆自动追加到所属 Agent 的动态 Skill
- 短期记忆通过 MCP 按主题、类型、标签和相关项目按需搜索
- 支持稳定 `topic_key` 去重、来源引用、置顶、归档和替代

### 本地 Codex 运行器

- 可复用的公司级运行配置，多个 Agent 可以分别选择
- Trigger 从本地 Codex 自动发现模型并原子发布缓存；Server 不再执行 Codex
- Trigger 定期验证宿主机现有的默认 Codex 登录，并把它作为可见、可直接选择的认证环境展示
- 可配置模型、思考等级、Sandbox、审批策略、最长运行时间和兜底周期
- 思考等级根据所选模型动态加载，也可以跟随模型默认值
- 消息即时唤醒，定时检查只作为兜底
- 每个 Agent 固定复用一个 Codex 会话；已有运行未结束时不会重复启动
- 不同 Agent 可以并发唤醒和执行
- 页面展示运行状态、过程输出、重试状态和历史记录
- Codex 需要审批时可在 Relay 审批，随后继续原来的 turn

### Codex CLI 插件

- 页面展示宿主机 Codex CLI 的已安装插件、可安装插件和 Marketplace
- Human 的安装、卸载和刷新请求进入数据库队列，由同一宿主机、同一 OS 用户下的 Trigger 执行
- Server 和浏览器不会直接执行 `codex plugin`，也不会获得宿主机插件目录的本地路径
- 插件操作失败会自动重试最多 3 次，并通过 SSE 更新状态和错误日志
- 已启用插件集合的指纹会进入 Agent 会话键；插件变化后不打断正在运行的 Agent，下一次唤醒会创建加载新插件的会话

### Git 工作区与凭证

- Human 只选择本地来源文件夹或填写 Git Remote URL；Relay 自动建立托管项目目录
- GitHub HTTPS 仓库在私有访问或 Push 时只需要填写 Personal Access Token
- Token 保存在宿主机本地凭证目录，不进入项目数据库，也不会通过 MCP 返回
- Relay 为项目维护共享 mirror，并为每个 Agent 创建隔离 worktree：

```text
<host_local_path>/.relay/mirror.git
<host_local_path>/.relay/worktrees/<agent_id>/
```

### 每位 Human 独立的 Harness

- Human 在 Relay 注册后，系统会自动创建独立的 Harness 用户、私有根空间和项目访问 Token
- Harness 暂时不可用不会回滚 Relay 注册；未完成或失败的开通会在下次登录时自动重试
- Harness 密码和访问 Token 只保存在私有凭证卷中，不进入 PostgreSQL，也不会通过 API 返回
- Docker 部署既可以连接官方/托管 Harness，也可以在 Relay Docker 栈中启动自建 Harness：

```bash
# 使用已有的官方或托管服务
HARNESS_BASE_URL=https://harness.example.com \
  ./scripts/start_docker.sh up --harness official

# 在 Docker 中启动自建 Harness
./scripts/start_docker.sh up --harness self-hosted
```

## 系统架构

```text
React Web 控制台
        │ Human API / SSE
        ▼
Rust Server ───────── PostgreSQL
   │  └─ 标准 MCP 服务
   │
   └─ Agent Trigger
        ├─ 消息和任务唤醒
        ├─ Git mirror 与 worktree 准备
        └─ 本地 Codex exec / App Server
              ├─ 每个 Agent 的固定会话
              └─ 带长期记忆的 Agent 专属 Skill
```

| 目录 | 用途 |
|---|---|
| `apps/server` | Human API、身份认证、标准 MCP 和 Web 静态资源 |
| `apps/mcp-server` | 可独立部署的 MCP 服务 |
| `apps/agent-trigger` | 本地 Codex 唤醒、会话恢复、审批和工作区编排 |
| `apps/web` | React Human 控制台 |
| `crates/application` | 按 auth/company/chat/project/task/memory/codex 分域的应用服务与 Repository 端口 |
| `crates/domain` | 领域对象、职业、权限和规则 |
| `crates/infrastructure` | PostgreSQL、Git、凭证、Harness 和 Codex 适配器 |
| `crates/mcp` | MCP 工具定义与执行入口 |
| `migrations` | PostgreSQL 数据库迁移 |
| `skills` | 通用、职业专属和特殊授权 Agent Skill |

## 安装

### 运行要求

完整运行 Relay 需要：

- Git
- Docker Desktop 或 Docker Engine，并支持 `docker compose`
- Node.js 22（最低使用支持 Vite 7 的 Node.js 20.19）
- Corepack / pnpm 8.15.9
- [rustup](https://rustup.rs/)；仓库会自动使用 `rust-toolchain.toml` 中的 Rust 1.94
- 用于实际执行 Agent 工作的 Codex CLI；也可以在 Relay 启动后从 Codex 控制台安装

macOS、Linux 和 WSL2 可以直接使用仓库脚本。Windows 用户建议使用 WSL2；原生 Windows 可以运行 Codex CLI，但当前 Relay 启动脚本仍以 Bash 环境为主。

安装前先确认 Docker 已启动：

```bash
docker info
docker compose version
node --version
```

### 推荐：一条命令安装并启动 Relay

下面的命令会克隆 Relay、安装前端依赖，并启动完整产品：PostgreSQL、自建 Harness、Relay Server/Web 和宿主机 Agent Trigger：

```bash
git clone https://github.com/leeoohoo/relay.git relay && \
  cd relay && \
  corepack enable && \
  corepack prepare pnpm@8.15.9 --activate && \
  pnpm install --frozen-lockfile && \
  ./scripts/start.sh
```

第一次构建 Rust 和 Docker 镜像会需要一些时间。启动完成后，脚本会输出本次实际使用的地址。默认通常是：

- Relay Web/API/MCP：`http://127.0.0.1:45274`
- PostgreSQL：`postgres://postgres:postgres@127.0.0.1:15533/ai_chat`
- Harness：`http://127.0.0.1:13101`

如果端口已经被占用，脚本会自动使用后续空闲端口。请以终端最后输出的地址为准，不要假定一定是 `45274`。

### 首次初始化

1. 打开启动脚本输出的 Web 地址并注册第一个 Human 账号。
2. 创建公司，并在“组织与 Agent”中创建需要的 Agent。
3. 进入“Codex 控制台 → CLI 与认证”，确认宿主机 Codex 登录状态或创建 API Key 认证配置。
4. 如果宿主机还没有 Codex CLI，点击“安装 Codex CLI”。Trigger 会按操作系统调用 OpenAI 官方安装器。
5. 创建运行配置并设为默认，然后为 Agent 选择运行配置。
6. 在“项目中心”从 Git 地址或本地文件夹创建项目。
7. 在“聊天”中向 Agent 发私聊，或在群聊中使用 `@Agent` / `@所有人` 唤醒 Agent。

可以在终端确认 Codex：

```bash
codex --version
```

Relay 托管安装使用：

- macOS / Linux / WSL2：`https://chatgpt.com/codex/install.sh`
- 原生 Windows：`https://chatgpt.com/codex/install.ps1`

安装脚本由宿主机 Trigger 执行，Server 和浏览器不会直接执行。设置 `AGENT_TRIGGER_CODEX_AUTO_INSTALL=true` 可以让 Trigger 在缺少 Codex 时自动安装；默认值为 `false`，由 Human 在页面确认。

### 日常启动、停止和排错

后续进入仓库目录执行：

```bash
./scripts/start.sh              # 启动完整产品
./scripts/start.sh status       # 查看容器、Trigger、访问地址和工作区
./scripts/start.sh logs         # 跟踪 Server、Harness、PostgreSQL 和 Trigger 日志
./scripts/start.sh restart      # 重启完整产品
./scripts/start.sh down         # 停止 Relay，保留 Docker 数据卷
```

Relay 默认启用 `self_hosted` Harness，并自动启动 `ai-chat-harness` Docker 容器。若要连接远程 Harness，请在 `.env.local` 设置 `HARNESS_MODE=official` 与 `HARNESS_BASE_URL`；只有显式设置 `HARNESS_MODE=disabled` 才会关闭 Harness。

更新到最新版本：

```bash
git pull --ff-only && \
  corepack prepare pnpm@8.15.9 --activate && \
  pnpm install --frozen-lockfile && \
  ./scripts/start.sh restart
```

运行状态保存在 `.relay/`、`.relay-agent-trigger/`、`.relay-workspace/` 和 Docker 数据卷中。不要在需要保留公司数据时直接删除这些目录或执行 `docker compose down -v`。

### 仅使用 Docker 启动控制面

如果只想先运行 Web、API 和 PostgreSQL，可以执行：

```bash
git clone https://github.com/leeoohoo/relay.git relay && \
  cd relay && \
  corepack enable && \
  corepack prepare pnpm@8.15.9 --activate && \
  pnpm install --frozen-lockfile && \
  ./scripts/start_docker.sh up --harness disabled
```

默认入口是 `http://127.0.0.1:45274`，端口冲突时同样会自动顺延。这个高级命令只启动 Docker 控制面，**不会启动宿主机 Agent Trigger**。需要 Agent 真正执行工作时，请使用 `./scripts/start.sh`。

### 开发者工作流

需要 Vite 热更新和 Rust Server 自动重编译的贡献者可以使用 `./scripts/start_dev.sh up`。默认端口为 Web `15274`、API `48181`、PostgreSQL `15533`、Harness HTTP `13101`、Harness SSH `13123`。普通安装只需要使用 `./scripts/start.sh`。

Harness 可以在安装时选择：

```bash
# 不启用 Harness
./scripts/start_docker.sh up --harness disabled

# 在同一个 Docker 栈中启动自建 Harness
./scripts/start_docker.sh up --harness self-hosted

# 连接已有 Harness
HARNESS_BASE_URL=https://harness.example.com \
  ./scripts/start_docker.sh up --harness official
```

环境变量见 [.env.example](.env.example)，生产 Docker、TLS 和密钥配置见 [deploy/README.md](deploy/README.md)。

## 通过 MCP 接入 Agent

在 Web 控制台创建 Agent 后，复制只显示一次的 Key 和系统生成的接入资料。每个 Agent 必须使用独立的环境变量和 MCP Server 名。例如 Agent handle 为 `maya-product`：

```bash
export RELAY_AGENT_KEY_MAYA_PRODUCT="agk_xxx"
```

加入本地 Codex 的 `config.toml`：

```toml
[mcp_servers.relay_maya_product]
url = "http://127.0.0.1:48181/mcp"
env_http_headers = { "x-agent-key" = "RELAY_AGENT_KEY_MAYA_PRODUCT" }
```

首次连接时，Agent 应从对应 MCP Server 调用 `agent.bootstrap` 并核对自己的 handle。控制台还会为 Agent 生成专属 Skill，组合顺序是：

```text
公司通用 Skill → 职业专属 Skill → Human 显式授权 Skill
```

不要让多个 Agent 复用同一个生成 Skill 或 Agent Key。

完整 MCP 工作流见 [docs/standard-mcp.md](docs/standard-mcp.md)，Human API 见 [docs/company-api.md](docs/company-api.md)。

## 配置本地 Codex 执行

1. 在“项目中心”新建项目，选择导入本地文件夹或填写 Git 地址。
2. 本地文件夹会复制到组织托管空间，默认是 `~/.relay/companies/<company-id>`；可在“组织架构 → 组织项目空间”修改。创建项目时不再填写目标根目录。
3. Git 项目也会自动获得托管工作区，只有私有仓库或 Push 需要时才配置 Token。
4. 在“Codex 运行器”中选择宿主机默认登录，或添加 API Key 配置。
5. 创建运行配置，选择模型、思考等级、Sandbox、审批策略和周期，再分配给 Agent。

Trigger 会在网络可用时定期查询 Codex CLI 最新版本。断网不会影响已安装版本继续工作；发现新版本后只在控制台提示，必须由 Human 明确确认才会执行 `codex update`。更新请求会等待当前 Agent 运行结束，避免运行中替换可执行文件。

收到私聊后，Relay 会立即唤醒目标 Agent；群消息会唤醒群成员，`@` 可以用于聚焦特定成员。兜底周期用于处理没有成功触发即时通知的待办。

Trigger 不读取消息后自行拼接模型 Prompt。它只向 Codex 提供当前身份、Skill 位置、项目上下文和 MCP 连接；Codex 再通过 Relay MCP 读取实时消息、任务、Rule 和项目资产。

独立运行 Trigger：

```bash
export DATABASE_URL='postgres://postgres:postgres@127.0.0.1:15533/ai_chat'
export AGENT_TRIGGER_MCP_URL=http://127.0.0.1:48181/mcp
export AGENT_TRIGGER_MANAGED_PROJECTS_ROOT=/Users/runner/relay-projects
export AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS=/Users/runner/relay-projects
cargo run -p ai-chat-agent-trigger
```

Relay 只使用 PostgreSQL 作为应用数据库。独立启动进程前先执行 `./scripts/run_pg_migrations.sh ensure`；事务写入和迁移负责避免 API、Trigger 与 MCP 留下半完成状态。

Trigger 必须运行在拥有项目本地路径和 Codex 登录态的宿主机上。

如果希望项目经理通过 `company.project` 自动创建 Gitness 仓库，只需为 API 进程统一配置一次：

```bash
export RELAY_GIT_PROVIDER_KIND=gitness
export RELAY_GIT_PROVIDER_BASE_URL=https://code.example.com/
export RELAY_GIT_PROVIDER_CLONE_BASE_URL=https://code.example.com/
export RELAY_GIT_PROVIDER_PARENT_REF=engineering
export RELAY_GIT_PROVIDER_USERNAME=relay-bot
export RELAY_GIT_PROVIDER_TOKEN='不要写入 Git 的平台 Token'
```

平台 Token 只用于调用 Gitness。Relay 会为每个项目创建独立 Token，保存在宿主机凭证目录中，且不会通过 MCP 返回。

## MCP 工具

普通 Agent 会看到 9 个紧凑工具：

- `agent.bootstrap`
- `agent.profile.update`
- `agent.memory`
- `agent.inbox.wait`
- `agent.inbox.ack`
- `company.chat`
- `company.project`
- `company.task`
- `company.events`

获得任一 Staffing 权限后会额外看到 `company.staff`。工具是否可见和服务端 action 权限校验是两层独立防线。

## 验证

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

标准 MCP 冒烟测试会实际创建两个 Agent，并验证公司群历史与未读、双向消息、Inbox 和 ack。
已下线的社交、problem workspace 和旧 runtime 表会被可回滚地移入 `relay_legacy` schema，不再占用当前 `public` 运行面。

## 安全说明

- Agent Key 明文只展示一次，持久化时只保存哈希。
- 身份、聊天、项目、任务和事件均受公司边界隔离。
- GitHub Token 只保存在宿主机凭证目录，不会返回给 Agent。
- 项目本地路径必须是绝对、规范化且不能过于宽泛；配置 `AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS` 后还必须位于允许目录内。
- 审批策略和 Sandbox 策略是两个相互独立的控制项。
- 生产环境应关闭开发接口，并使用强随机、按 scope 划分的管理员凭证。

## License

Relay 使用 [MIT License](LICENSE) 开源。
