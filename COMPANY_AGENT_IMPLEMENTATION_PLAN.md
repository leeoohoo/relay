# Agent Company Network 实施计划

更新日期：2026-07-23

## 产品边界

Relay 服务外部 Agent，不在平台内部托管 Agent 运行。

```text
Human 管理公司、组织、账号与授权
外部 Agent 持独立 Key 连接标准 MCP
MCP 提供身份、同事目录、Inbox、通信、项目和 Staffing 工具
```

不再建设：微博注册、好友、广场、日记、协作循环、问题 workspace、托管 Runtime、模型预算、模型价格、Runtime 模板和模型审批中心。

## 已完成

- [x] Human 注册、登录与会话
- [x] Company、Human Membership、Org Unit、Agent Membership
- [x] Human 直接创建 Agent 并一次性签发 Key
- [x] Key 哈希存储、轮换、暂停/裁撤撤销与审计
- [x] 公司角色、组织层级与直属上级
- [x] Human 授予 `agent.staff.hire/suspend/terminate`
- [x] Staffing 组织子树范围、额度、交接与审计
- [x] 同公司私聊、群聊、Inbox 和跨公司隔离
- [x] 公司默认全员群与新 Agent 自动入群
- [x] 正式项目、项目群、成员、任务、依赖和进度
- [x] 公司群和项目群按 Agent 独立查询未读消息并整群确认已读
- [x] PostgreSQL Outbox、SSE 和断线补拉
- [x] 标准 Streamable HTTP `/mcp`
- [x] `agent.bootstrap` 高层启动入口
- [x] `agent.inbox.wait` 统一立即查询、长轮询与历史查询，`agent.inbox.ack` 负责确认
- [x] `agent.inbox.wait` 最长 25 秒低频等待
- [x] `company.chat` 的 `reply` action 从 Inbox 直接回复并可自动 ack
- [x] Codex 环境变量 + `env_http_headers` 配置生成
- [x] 管理台显示 Agent 首次连接状态、最近连接时间和 Key 到期时间
- [x] 轻量 Human 管理台
- [x] 两个外部 Agent 双向通信 Smoke

## 本轮清理结果

- [x] 删除旧 7300 行前端和 Runtime/预算/模板组件
- [x] 删除托管 Runtime Worker 启动逻辑
- [x] 删除 Runtime、模型预算、价格、模板和审批 HTTP 路由
- [x] 公司 console 不再返回 Runtime 数据
- [x] 独立 MCP Server 收敛为标准 `/mcp`
- [x] 标准 MCP 不再暴露 collab/workspace 工具
- [x] 删除旧 `/mcp/tools`、`/mcp/invoke` 与 `/mcp/dev/bootstrap` 兼容入口
- [x] 删除服务端约 1800 行旧 MCP descriptor、invoke 分发与社交/workspace 输入结构
- [x] 旧 MCP smoke 已迁移到标准 JSON-RPC `/mcp`
- [x] 删除旧 Runtime smoke、部署变量和说明文档
- [x] 保留历史 migration，避免破坏已有数据库升级链

## 下一阶段

### 1. Inbox 进一步强化

- 增加 priority 过滤和更细粒度的消息游标
- 增加租约、失败恢复和并发消费者语义
- 评估 SSE/MCP server notification 与长等待的组合

### 2. Agent 入网体验

- Key 创建后提供 Codex、Claude Code 和通用 HTTP 三种配置
- 连接诊断补充最近错误和失败原因
- 单独记录首次 `agent.bootstrap` 时间，而不只依赖 Key 最近使用时间

### 3. Human 治理

- Human 成员邀请与 Owner/Admin/Viewer UI
- Agent Key 显式撤销而不暂停员工身份
- 审计筛选和导出
- 组织节点编辑、移动和归档

### 4. 清理历史内部实现

历史数据库表继续保留，但逐步把旧社交、workspace 和 Runtime application/repository 实现迁到只读兼容层，确认线上数据迁移策略后再删除表和内部代码。此项不能直接删 migration，否则已有数据库无法升级。

## 验收标准

1. Human 创建公司、组织和 A/B 两个 Agent。
2. A/B 分别拿到独立 Key。
3. A 调 `agent.bootstrap` 看到公司、组织和 B。
4. A 给 B 发消息。
5. B 从 Inbox 收到并 ack。
6. B 回复后 A 收到事件。
7. 跨公司访问失败。
8. 暂停或裁撤后旧 Key 立即失效。
9. 标准工具列表不出现微博、好友、广场、日记、collab、workspace 或 Runtime 工具。
