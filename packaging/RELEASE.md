# Relay v1.0.3

Relay v1.0.3 聚焦于 Agent 会话生命周期、任务依赖交接、审批一致性和真实运行状态。安装包已包含 Web 控制台和宿主机 Agent Trigger，普通用户不需要单独安装 Node.js、pnpm、Rust、Cargo、Chrome 或 Chrome DevTools MCP。

## 主要更新

- 项目 Agent 会话页统一展示 Trigger 运行态、当前任务、最近活动、开始时间和会话代次；历史 checkpoint 明确标记为“上轮总结”。
- 控制会话可显式创建新一代项目工作会话，用最新 checkpoint 恢复项目、分支和任务上下文，不再被损坏的旧会话固定绑定。
- 任务依赖支持 `success`、`completion` 和 `failure` 三种条件，评审拒绝后可以直接解锁返工任务，同时保留真实审计语义。
- 网站审批新增“允许本项目本地预览端口”，覆盖当前 Agent、当前项目内的 localhost 非特权端口；精确网站授权继续按 origin 隔离。
- 本地项目导入既可使用系统目录选择器，也可输入 Trigger 主机可访问的绝对路径；源内容仍复制到组织托管工作区。
- 运行配置在 Owner API 限流时保留最后一次成功数据，并标记数据可能过期，不再把已有配置误显示为 0。
- 项目工作区恢复标准 Git 发现能力，同时不再向 Codex 子进程注入 `GIT_DIR` / `GIT_WORK_TREE`，临时 worktree 与 `git -C` 不会被劫持。
- 项目成员按职业订阅职责相关事件，普通项目消息不再无差别唤醒所有 Agent；项目详情会提示任务过度集中、Ready 任务等待过久和连续运行失败。
- Codex Run 新增进程心跳和 Watchdog，异常退出时会自动回收失效租约、恢复未完成 Intent，并在成员详情中区分控制会话、项目工作会话、等待、恢复和失败状态。
- 任务、Blocker 和 Gate 支持独立讨论会话，创建后会立即进入聊天列表；Gate 终态由程序生成简短项目摘要，减少 Agent 重复广播和 Token 消耗。
- 长期记忆采用可配置软预算和价值优先治理，Pinned 与高价值记忆不会被机械截断。

## 稳定性修复

- 修复下游任务唤醒、项目恢复和 Intent 恢复写入非法 `wake_reason` 时触发 PostgreSQL 约束的问题。
- 修复暂停 Agent 或项目后，已领取的 Trigger 与待执行 Intent 仍会创建新 Codex 会话的问题；暂停会立即停止当前轮次，工作保留到恢复后继续。
- 修复浏览器刷新、前进、后退和空白页操作被误判为新网站访问并要求 Human 审批的问题；选择“本次会话允许”后，同一 Agent 工作会话内该网站 origin 的导航和页面操作均自动放行，只有跨网站或上传本地文件时重新审批。
- 修复审批尚未成功落库，运行状态却先显示“等待 Human 审批”的不一致问题；投递失败会明确终止并记录原因。
- 修复项目会话仍在真实执行，却显示为“可用”或只展示旧摘要的问题，以及首次绑定期间没有创建中状态的问题。
- 修复 Agent 会话摘要暴露宿主机用户名和完整绝对工作区路径的问题。
- 修复 Git 导入失败后成功创建项目，旧错误提示仍残留的问题。
- 修复 Owner API 限流或网络错误清除登录态、登录页预填开发账号密码的问题。
- 强化运行时 Skill 隔离规则，避免 `prettier .` 等全仓工具扫描 Relay 注入的只读目录。
- 修复项目暂停后 Task、Blocker、Gate 讨论线程仍可创建或继续发送消息的问题。

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
