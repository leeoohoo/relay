# Relay v1.0.2

Relay v1.0.2 聚焦于 Agent 执行可靠性、Harness 项目协作和可控的浏览器自动化。安装包已包含 Web 控制台和宿主机 Agent Trigger，普通用户不需要单独安装 Node.js、pnpm、Rust、Cargo、Chrome 或 Chrome DevTools MCP。

## 主要更新

- 新增 Relay 托管的 Chrome DevTools MCP。Agent 可以完成真实网页检查、交互、截图和快照，并通过 Relay 审批中心处理网站访问授权。
- 网站审批支持“始终允许”，授权按 Agent、项目和网站来源隔离，不会扩散到其他项目或域名。
- 浏览器证据保存到隔离的 `.relay/browser-artifacts/` 可写目录，项目其余目录继续保持只读；解决截图、快照无法保存的问题。
- 项目目录通过 Harness API 浏览，支持分支切换、服务端分页、文件语法高亮和 SVG 可视化预览。
- 强化控制会话与项目工作会话隔离，减少 Inbox、项目任务和不同工作会话之间的上下文干扰。
- 改进任务交接与 Agent 实时进度展示，降低依赖任务已就绪但负责人未继续执行、状态看似卡住等问题。
- 项目创建和 Agent 身份展示更加清晰；Harness 浅克隆项目可以正确补全历史并发布远端仓库。
- 插件列表只展示 Codex CLI 实际支持的插件，避免安装桌面端专属插件后无法使用。

## 稳定性修复

- 修复 Codex app-server 短暂启动失败或恢复旧会话无响应后长期假运行的问题；Relay 会分阶段超时并自动重建会话继续任务。
- 修复托管浏览器配置目录在异常退出或重启后被 Chromium 锁文件阻塞的问题。
- 修复浏览器 MCP 审批没有进入 Relay、审批后 Turn 没有继续以及继承审批策略覆盖托管设置的问题。
- 修复工具调用失败时只显示“失败”而不展示实际错误原因的问题。
- 修复项目会话列表出现历史重复会话的问题。
- 增强运行时 Skill、Git 工作树和浏览器证据目录的隔离，避免格式化工具和 Git 状态受到运行时文件干扰。

## 安装包

- `relay-macos-apple-silicon.tar.gz`：Apple Silicon Mac。
- `relay-windows-wsl2-x86_64.zip`：Windows 10/11、WSL2 和 Docker Desktop。
- `SHA256SUMS`：下载文件完整性校验。

Linux 和 Intel Mac 暂不提供预编译安装包。Docker 仍是必需依赖，因为 Relay Server、PostgreSQL 和 Harness 运行在容器中。

每个安装包内均包含 `INSTALL.md`。覆盖旧版本后执行：

```bash
./start.sh restart
```

macOS 安装包目前可在没有 Apple 证书的情况下使用，但首次运行可能需要确认一次 Gatekeeper 提示。
