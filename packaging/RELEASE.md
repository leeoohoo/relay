# Relay v1.0.4

Relay v1.0.4 重点改善首次安装、项目导入、Agent 工作会话恢复与本地运行性能。

## 性能与稳定性修复

- 默认同时运行的 Agent Codex 会话降为 2，并根据主机 CPU 自动设置 1–4 的资源上限，避免并发任务把普通设备压满。
- Codex 运行心跳改为独立线程，并将默认心跳容错提高到 180 秒，降低资源拥塞造成的误中断。
- Watchdog 以心跳和最近活动中的最新时间判断存活状态，不再回收仍有实际进展的会话。
- 租约失效、暂停、取消和 Trigger 重启时清理完整 Codex 进程树，避免残留的幽灵进程持续消耗 CPU。
- 宿主机 Chrome 空闲 15 分钟后自动退出，同时保留浏览器 Profile 供下次恢复。
- 修复 macOS 停止 Trigger 时先卸载服务导致子进程来不及清理的问题。
- Agent 运行状态投影增加短暂抖动容错，减少正常会话切换被展示成异常恢复。

## 其他主要修复

- 修复本地目录导入和 Git 地址导入失败，项目记录、Harness 仓库、Git 配置与补偿清理重新保持一致。
- 修复数据库查询异常被错误转换成“Agent 不属于公司”，导致正常项目成员无法调用项目和任务工具的问题。
- 补齐 `replace_session` 数据库约束，使损坏或权限缓存过期的项目工作会话可以创建新一代会话。
- 项目工作会话直接获得完整 `company_id` 与 `project_id`，不再扫描工作区或从 Git 命名空间猜测运行参数。
- 项目 Gate 与 Environment 接入 MCP 执行链，Agent 可以读取真实门禁、环境要求与 readiness。
- 浏览器运行时按 Agent 复用宿主机 Chrome，并保留受限 Docker 兜底。
- Codex Prompt 上限提高至 100,000 字符；控制快照会去除重复消息、按预算分页并清理不安全控制字符。

## 安装包

- `relay-macos-apple-silicon.tar.gz`：Apple Silicon macOS。
- `relay-windows-wsl2-x86_64.zip`：Windows PowerShell 安装包。
- `relay-windows-wsl2-x86_64.tar.gz`：Windows WSL2 安装包。
- `SHA256SUMS`：所有附件的 SHA-256 校验值。

Linux 原生版与 Intel Mac 暂不提供预编译安装包。
