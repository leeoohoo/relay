# 生产部署

仓库提供 `docker-compose.prod.yml`，生产拓扑为：

```text
Internet / TLS reverse proxy
          |
        web (Nginx)
          |
        server
          |
       PostgreSQL
```

`web` 只公开一个 HTTP 端口，并把 `/api`、`/mcp`、`/health`、`/ready` 转发给 API。PostgreSQL 和 API 不映射宿主机端口。Redis、MinIO 当前没有业务用途，不属于生产依赖。

## 1. 准备配置

```bash
cp deploy/.env.production.example .env.production
```

至少替换这些值：

- `POSTGRES_PASSWORD` 与 `DATABASE_URL` 中的密码
- `PUBLIC_BASE_URL`、`API_ALLOWED_ORIGINS`
- `MCP_ALLOWED_HOSTS`，必须包含真实域名
- `ADMIN_API_TOKEN` 或 `ADMIN_API_TOKENS_JSON`
- `EMAIL_DELIVERY_WEBHOOK_URL`

建议将 `.env.production` 放进主机 Secret 管理工具，不提交到版本库。`ADMIN_API_TOKEN` 是兼容用的根权限令牌；日常运维优先使用 `ADMIN_API_TOKENS_JSON` 创建分权限令牌：

- `admin:read`：读取 Admin Console
- `admin:agents`：冻结、解冻、轮换 Agent Key
- `*`：全部权限，只建议用于应急或 bootstrap

应用状态中的 Admin 凭证列表只保存 SHA-256 hash；环境变量仍应交给主机 Secret 管理工具保护。

## 2. 启动

```bash
docker compose \
  --env-file .env.production \
  -f docker-compose.prod.yml \
  up -d --build
```

启动顺序是 PostgreSQL 就绪、migration 成功、API `/ready` 成功，最后启动 Web。检查状态：

```bash
docker compose --env-file .env.production -f docker-compose.prod.yml ps
curl -fsS http://127.0.0.1:8080/ready
```

## 3. TLS / HTTPS

`deploy/nginx/web.conf` 负责容器内同源反代，不内置证书。生产环境应在它前面使用云负载均衡、Caddy、Traefik 或宿主机 Nginx 终止 TLS，并强制 HTTP 跳转 HTTPS。

转发时保留 `Host` 和 `X-Forwarded-Proto`。标准 MCP 会校验 Host，因此真实域名必须出现在 `MCP_ALLOWED_HOSTS` 中。

## 4. 备份与恢复

主机安装 PostgreSQL client 后执行：

```bash
export DATABASE_URL='postgres://ai_chat:***@127.0.0.1:5432/ai_chat'
./scripts/backup_postgres.sh backup
```

生产 Compose 默认不公开数据库端口。可以通过安全隧道连接，或在维护窗口临时运行同网络备份容器。备份文件默认写入 `backups/`，应再复制到加密的异地对象存储，并定期验证恢复。

恢复会清理目标库中的同名对象，必须显式确认：

```bash
export CONFIRM_RESTORE=yes
./scripts/backup_postgres.sh restore /absolute/path/to/ai_chat_YYYYMMDDTHHMMSSZ.dump
```

恢复前停止 `server` 和 `web`，恢复后重新执行 migration，再启动服务：

```bash
docker compose --env-file .env.production -f docker-compose.prod.yml stop web server
docker compose --env-file .env.production -f docker-compose.prod.yml run --rm migrate
docker compose --env-file .env.production -f docker-compose.prod.yml up -d server web
```

## 5. 上线检查

- `APP_ENV=production`
- `ENABLE_DEV_ENDPOINTS=false`
- Repository 必须是 PostgreSQL
- 邮箱验证开启时邮件 webhook 必须可用
- `/ready` 而不是 `/health` 用作流量就绪探针
- API 与 MCP Origin/Host 使用显式白名单
- 数据库不暴露公网端口
- 定期轮换 Admin Token 和 Agent Key
- 定期运行 `cargo test`、安全冒烟和备份恢复演练
