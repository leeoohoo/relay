# Relay v1.0.4

Relay v1.0.4 重点改善首次安装、项目导入和 Agent 工作会话恢复。Apple Silicon macOS 与 Windows WSL2 用户可以通过轻量 npm 启动器完成安装、更新、启动和日常运维。

## 一条命令安装

安装并启动 Docker Desktop 与 Node.js 20+ 后执行：

```bash
npx --yes @relay-ai/relay web
```

启动器会下载匹配平台的 GitHub Release、校验 `SHA256SUMS`、把程序安装到 `~/.relay/app`，把公司、Agent、项目、Trigger 和工作区数据持久化到 `~/.relay/data`，然后启动 Relay 并打开控制台。

同时支持：

```bash
npx --yes @relay-ai/relay install
npx --yes @relay-ai/relay status
npx --yes @relay-ai/relay logs
npx --yes @relay-ai/relay restart
npx --yes @relay-ai/relay stop
npx --yes @relay-ai/relay update
```

## 主要修复

- 修复本地目录导入和 Git 地址导入失败，项目记录、Harness 仓库、Git 配置与补偿清理重新保持一致。
- 修复数据库查询异常被错误转换成“Agent 不属于公司”，导致正常项目成员无法调用项目和任务工具的问题。
- 补齐 `replace_session` 数据库约束，使损坏或权限缓存过期的项目 worker 可以创建新一代工作会话。
- 项目 worker 现在直接获得完整 `company_id` 与 `project_id`，不再扫描工作区或从 Git 命名空间猜测运行参数。
- 项目 Gate 与 Environment 已接入 MCP 执行链，Agent 可以读取真实门禁、环境要求与 readiness，不再只展示不用。
- 浏览器运行时改为按 Agent 复用宿主机 Chrome，减少重复 Chromium 容器造成的 CPU 和内存压力，并保留受限 Docker 兜底。
- Codex Prompt 上限提高至 100,000 字符；控制快照会去除重复消息、按预算分页，并统一清理不安全控制字符，避免 Agent 因上下文增长而无法启动。
- 安装目录和持久化数据目录彻底分离，npm 启动器更新应用文件时不会覆盖公司、Agent、项目、PostgreSQL、Harness 或工作区数据。

## 发布与平台

- GitHub Release 完成后自动发布带 provenance 的 `@relay-ai/relay` npm 包。
- `relay-macos-apple-silicon.tar.gz`：Apple Silicon macOS。
- `relay-windows-wsl2-x86_64.zip`：Windows PowerShell 手动安装。
- `relay-windows-wsl2-x86_64.tar.gz`：Windows WSL2 npm 启动器。
- `SHA256SUMS`：所有 Release 附件的完整性校验。

Linux 和 Intel Mac 暂不发布 npm/预编译安装包。Docker 仍是 Relay Server、PostgreSQL、Harness 和浏览器运行时的必需依赖。
