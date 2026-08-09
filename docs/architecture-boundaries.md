# Architecture Boundaries

Relay 保持一个部署友好的模块化单体，同时使用四个逻辑边界控制依赖与产品复杂度。

## Identity & Governance

拥有 Human、Company、Agent、组织、权限、治理策略和审批。对外查询默认只返回身份与策略摘要，不携带项目任务、消息或运行日志。

## Collaboration Hub

拥有会话、消息、项目、任务和项目记忆。列表返回摘要，详情和历史按需加载。它可以请求 Execution Control 执行工作，但不直接管理 Codex 进程。

## Execution Control

拥有 Trigger、Codex Session、Run、Runner Profile、CLI、插件、MCP、Hooks 和运行诊断。执行状态不能塞入所有协作列表；页面按 Agent 或项目独立查询。

## Integration

拥有 Harness、Git、浏览器 MCP、Ownership Proof、SSE Outbox 和外部凭证。所有外部副作用必须具备：

- 幂等请求标识；
- 超时与可重试错误；
- 成功后的审计信息；
- 失败补偿或持久化的待清理状态；
- 不在数据库或日志中暴露秘密。

## 应用层主路径

当前唯一业务入口为 `PlatformApp`。`service/` 保存按领域拆分的 Repository Port，`platform/` 保存 Use Case 编排。未被生产代码使用的第二套 `services/` Facade 已删除。

后续迁移以真实 Use Case 为单位进行；只有当模块同时包含授权、校验、事务和事件编排时，才值得形成独立 Service。仅代理一次 Repository 调用的薄包装不允许进入公共 API。
