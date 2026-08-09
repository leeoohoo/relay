# Relay 前端架构

Relay Web 是面向 Human 的多 Agent 协作与执行控制台，不是只读治理页面。它同时承担组织治理、项目协作、Agent 运行观察和 Codex 执行配置，但各功能必须按页面按需查询，不能重新汇总成一个不断膨胀的 Company Console 对象。

## 产品区域

1. **Identity & Governance**：公司、Human、Agent、组织、权限、策略与审批。
2. **Collaboration Hub**：聊天、项目、任务、项目记忆和项目上下文。
3. **Execution Control**：Codex Runner、会话、运行记录、CLI、插件、MCP 与认证环境。
4. **Integration**：Harness、Git、实时事件与外部工具状态。

这四项是代码和产品职责边界，不代表四个独立部署服务。

## 应用壳职责

`pages/App.tsx` 只负责：

- Human Session 与公司选择；
- 顶层导航和全局弹窗；
- Company Summary 的组合；
- SSE 事件到局部查询刷新的路由。

业务页面拥有自己的请求、分页、错误恢复和局部刷新状态。应用壳不得重新保存项目详情、消息历史、运行日志或插件目录等页面级大对象。

## Company 查询模型

前端使用以下区域接口并行构建初始页面：

- `/api/v1/companies/{company_id}/summary`
- `/api/v1/companies/{company_id}/agents`
- `/api/v1/companies/{company_id}/conversations`
- `/api/v1/companies/{company_id}/projects`
- `/api/v1/companies/{company_id}/approvals`

旧 `/console` 仅为兼容接口，Web 不得调用。SSE 按事件类型只使对应区域失效：消息刷新会话，任务刷新项目，招聘刷新 Agent，审批刷新审批。

项目列表只应保存摘要。项目成员、任务、资产、历史、Rule、Git 和会话应逐步迁移到项目详情的独立懒加载接口。

## 分页规则

- 消息、任务、项目、会话、运行记录和审计记录使用服务端游标或页码分页。
- 前端分页只能用于已经有明确服务端上限的小型目录，不能用来掩盖全量接口。
- 新列表必须定义默认 `limit`、最大 `limit`、稳定排序键和下一页游标。

## 架构检查

新增页面、接口或实体前必须回答：

1. 它属于治理、协作、执行还是集成？
2. 是否必须进入首页级导航？
3. 是否扩大现有聚合对象？如果是，为什么不能独立查询或懒加载？

CI 通过 `scripts/check_architecture_boundaries.sh` 阻止 Web 重新调用 `/console`，并阻止应用层重新出现未使用的双轨 Service Facade。
