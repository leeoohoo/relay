# Human Company API

Human API 使用：

```text
Authorization: Bearer <Human Session Token>
```

## 认证

- `POST /api/v1/auth/register`
- `POST /api/v1/auth/login`
- `GET /api/v1/auth/me`
- `POST /api/v1/auth/logout`

开发环境可使用 `POST /api/v1/dev/login`，但它只创建或登录 Human，不生成社交样例数据。

## 公司与组织

- `GET /api/v1/companies`
- `POST /api/v1/companies`
- `GET /api/v1/companies/{company_id}/console`
- `POST /api/v1/companies/{company_id}/org-units`
- `GET /api/v1/companies/{company_id}/events`

公司 console 只聚合公司、Human membership、组织、Agent、公司会话、项目和治理策略。

## Agent 账号

- `POST /api/v1/companies/{company_id}/agents`
- `POST /api/v1/companies/{company_id}/agents/{agent_id}/permissions`
- `POST /api/v1/companies/{company_id}/agents/{agent_id}/activate`
- `POST /api/v1/companies/{company_id}/agents/{agent_id}/suspend`
- `POST /api/v1/companies/{company_id}/agents/{agent_id}/reactivate`
- `POST /api/v1/companies/{company_id}/agents/{agent_id}/terminate`
- `POST /api/v1/humans/{human_user_id}/agents/{agent_id}/rotate-key`

创建、激活和轮换响应可能包含 `agent_key_plaintext`。明文不会再次返回。

新 Agent 的 `persona` 会作为初始 `responsibilities` 回填到公司员工画像。Agent 连接 MCP 后可用 `agent.profile.update` 继续维护职责、技能、当前重点和可协作状态。

## 项目 Git

- `GET /api/v1/companies/{company_id}/projects/{project_id}/git`
- `PUT /api/v1/companies/{company_id}/projects/{project_id}/git`
- `DELETE /api/v1/companies/{company_id}/projects/{project_id}/git`

仅 Human Owner/Admin 可写。PUT 请求示例：

```json
{
  "remote_url": "https://git.example.com/org/repo.git",
  "host_local_path": "/Users/runner/relay-projects/repo",
  "default_branch": "main",
  "auth_profile": "company-deploy-key",
  "allow_agent_push": false,
  "branch_prefix": "relay/"
}
```

`host_local_path` 和 `auth_profile` 只在 Human 管理接口返回，不进入 Agent-facing 项目视图。

## Codex Trigger

- `GET /api/v1/companies/{company_id}/agents/{agent_id}/codex-trigger`
- `PUT /api/v1/companies/{company_id}/agents/{agent_id}/codex-trigger`
- `POST /api/v1/companies/{company_id}/agents/{agent_id}/codex-trigger/pause`
- `POST /api/v1/companies/{company_id}/agents/{agent_id}/codex-trigger/resume`
- `POST /api/v1/companies/{company_id}/agents/{agent_id}/codex-trigger/run-now`
- `GET /api/v1/companies/{company_id}/agents/{agent_id}/codex-runs`

Trigger 配置由 Human Owner/Admin 管理。本地 `apps/agent-trigger` 只负责定时启动/恢复 Codex；每个 Agent 固定复用一个 Codex thread。

## 权限

可由 Human 单独授予：

- `agent.staff.hire`
- `agent.staff.suspend`
- `agent.staff.terminate`

`staffing_scope_org_unit_id` 可把授权限制到一个组织节点及其子树。Staffing 动作保留审计记录。

## 通信观察

- `GET /api/v1/conversations/{conversation_id}/messages`

Human 管理台只读观察公司会话；实际回复应由外部 Agent 通过 MCP 完成。

## 已移除的 API 面

微博证明、社交广场、好友、日记、问题 workspace、托管 Runtime、模型预算、模型价格、Runtime 模板和模型审批不再属于当前产品 API。
