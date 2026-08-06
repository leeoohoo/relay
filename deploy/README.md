# Relay 生产部署

生产环境使用 `docker-compose.prod.yml`，唯一应用数据库为 PostgreSQL。部署栈包含数据库迁移、Rust Server、Web，以及可选的自建 Harness；Agent Trigger 默认运行在宿主机，以便访问宿主机 Codex 登录态、本地项目与 Git worktree。

## 1. 准备配置

```bash
cp deploy/.env.production.example .env.production
```

至少修改以下配置：

- `POSTGRES_PASSWORD`
- `DATABASE_URL`
- `PUBLIC_BASE_URL`
- `API_ALLOWED_ORIGINS`
- `MCP_ALLOWED_HOSTS`
- `AGENT_TRIGGER_STATE_ROOT`
- `RELAY_DEFAULT_WORKSPACE_ROOT`
- `RELAY_HARNESS_CREDENTIALS_ROOT`
- `RELAY_MESSAGE_ATTACHMENTS_ROOT`
- `RELAY_HOST_UID` 与 `RELAY_HOST_GID`（分别填写部署用户的 `id -u`、`id -g`）
- 管理员 Token 或 `ADMIN_API_TOKENS_JSON`

不要把 `.env.production` 提交到 Git。

`RELAY_DEFAULT_WORKSPACE_ROOT` 必须填写宿主机绝对路径；该路径会以相同位置挂载到 Server 容器，使 Server 创建的项目和宿主机 Trigger 看到的是同一目录。

首次启动前创建共享目录。Server 会使用上述宿主 UID/GID 运行，确保宿主机 Trigger 与 Docker Server 可以安全读写同一份状态：

```bash
mkdir -p .relay-agent-trigger .relay-workspace .relay/harness-credentials .relay/attachments
```

如果这些目录曾由旧版本的 root 容器创建，只需修复所有权，不要删除数据：

```bash
sudo chown -R "$(id -u):$(id -g)" \
  .relay-agent-trigger .relay-workspace .relay
```

## 2. 启动 PostgreSQL、迁移、API 与 Web

```bash
docker compose --env-file .env.production -f docker-compose.prod.yml up -d --build
```

启动顺序由 Compose 保证：

1. PostgreSQL 健康检查通过。
2. `migrate` 执行 `scripts/run_pg_migrations.sh ensure`。
3. Server 启动并连接 PostgreSQL。
4. Web 等待 Server 健康后启动。

查看状态与日志：

```bash
docker compose --env-file .env.production -f docker-compose.prod.yml ps
docker compose --env-file .env.production -f docker-compose.prod.yml logs -f server web migrate
```

## 3. Harness 模式

默认不启动 Harness。连接已有服务时设置：

```env
HARNESS_MODE=official
HARNESS_BASE_URL=https://harness.example.com
HARNESS_PUBLIC_BASE_URL=https://harness.example.com
```

在同一 Docker 栈中启动自建 Harness：

```bash
docker compose --env-file .env.production -f docker-compose.prod.yml \
  --profile harness-self-hosted up -d harness
```

同时设置：

```env
HARNESS_MODE=self_hosted
HARNESS_BASE_URL=http://harness:3000
HARNESS_PUBLIC_BASE_URL=https://git.example.com
HARNESS_ADMIN_PASSWORD=replace-with-a-long-random-password
```

## 4. 宿主机 Agent Trigger

Trigger 必须：

- 使用与 Server 相同的 `DATABASE_URL`；
- 能访问 `AGENT_TRIGGER_STATE_ROOT` 与托管项目目录；
- 能调用宿主机 Codex CLI；
- 使用与 Server 一致的 Git/Harness 配置。

```bash
set -a
source .env.production
set +a
cargo run --release -p ai-chat-agent-trigger
```

## 5. PostgreSQL 备份与恢复

备份：

```bash
docker compose --env-file .env.production -f docker-compose.prod.yml \
  exec -T postgres pg_dump -U "$POSTGRES_USER" -d "${POSTGRES_DB:-ai_chat}" -Fc \
  > relay-postgres.dump
```

恢复前先停止写入，再执行：

```bash
docker compose --env-file .env.production -f docker-compose.prod.yml stop server
docker compose --env-file .env.production -f docker-compose.prod.yml \
  exec -T postgres pg_restore -U "$POSTGRES_USER" -d "${POSTGRES_DB:-ai_chat}" \
  --clean --if-exists < relay-postgres.dump
docker compose --env-file .env.production -f docker-compose.prod.yml start server
```

生产环境不要暴露 PostgreSQL 端口到公网，并定期验证备份可恢复。
