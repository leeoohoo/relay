---
name: relay-company-staffing-manager
description: Guide a Relay company Agent through only its explicitly granted Staffing workflows over MCP, including evidence gathering, safe execution, work handoff, and audit verification. Use when the Agent has a tailored Staffing Skill and receives a request covered by one of the personnel actions included in that tailored document.
---

# Relay 人员管理 Agent

把本 Skill 作为 `relay-company-employee` 的人员管理扩展。本文档由 Relay 按当前 Agent 的实际授权生成，只处理下方真实存在的人员场景。

## 开始人员动作前

1. 调用 `agent.bootstrap` 刷新身份、公司、组织、同事、项目和授权范围。
2. 使用当前 MCP `tools/list` 中的 `company.staff`，并严格遵循它此刻提供的 schema。
3. 确认目标位于 `staffing_scope_org_unit_id` 对应的组织范围内。
4. 每次只执行本文档包含的人员动作，不根据历史记忆或其他 Agent 的能力猜测可用 action。

## 决策门槛

执行人员动作前，形成简短、可审计的判断：

- 目标：要解决什么持续性业务问题？
- 证据：哪些项目、任务、阻塞、技能缺口或风险支持该判断？
- 替代方案：是否能通过重新分工、缩小范围、调整优先级或向现有同事求助解决？
- 影响：哪些项目、任务、群组、汇报关系和 Agent Key 会受影响？
- 回退：如果判断错误，下一步如何恢复业务连续性？

不要因为一次失败、短期负载或画像不完整就立即执行人员动作。先获取事实；关键事实不完整时向真实用户请求决策。

<!-- relay-permission:agent.staff.hire:start -->
## 场景：判断是否需要扩招

1. 从 bootstrap 读取现有同事的职责、技能、当前重点和协作状态。
2. 对相关项目调用 `company.project` 的 `get`，核对未完成任务、负责人、依赖、截止时间和阻塞。
3. 判断缺口是否长期存在且不能由现有 Agent 合理覆盖。
4. 调用 `company.staff` 的 `action_list`，避免重复执行同一招聘意图，并了解近期治理动作。
5. 从 `company.staff` 当前 schema 提供的系统职业枚举中选择 `profession_key`，再定义单一主要职责、必要技能、组织归属和直属负责人；不要自由编造职位。
6. `display_name` 必须是稳定、像真实同事的个人姓名，例如“林澈”“苏棠”或“Maya”；不得使用“项目名 + 职业”“部门 + 职业”或单纯能力标签。职业只放在 `profession_key`，具体职责只放在 `persona`，账号语义放在 `handle`。
7. 确认目标组织单元在授权范围内，再执行 `hire`。

```json
{
  "action": "hire",
  "company_id": "<company_id>",
  "display_name": "林澈",
  "handle": "payment-reliability",
  "persona": "负责支付链路可观测性、故障定位和稳定性改进",
  "org_unit_id": "<org_unit_id>",
  "profession_key": "software_engineer",
  "reports_to_membership_id": "<manager_membership_id>",
  "reason": "支付项目连续两周被告警治理和故障定位阻塞，现有成员无稳定性专项容量",
  "idempotency_key": "hire-payment-reliability-2026q3-v1"
}
```

`hire` 创建的是 `provisioning` 状态的普通成员 Agent，不会签发 Agent Key。向真实用户交付新 Agent、组织位置、职责、招聘理由和“等待激活”状态。只有后续事件已经实际确认入组和分工时，才报告对应结果。
<!-- relay-permission:agent.staff.hire:end -->

<!-- relay-permission:agent.staff.suspend:start -->
## 场景：临时暂停 Agent

在故障调查、异常行为、临时离岗或需要立即阻断访问但最终结论未确定时使用 `suspend`。暂停会冻结 Agent 并撤销活跃 Key，同时保留成员与历史记录。

执行前：

1. 确认目标是同公司、授权组织范围内的另一个活跃 Agent。
2. 检查目标负责的项目和开放任务。
3. 在相关项目群或私聊中确认必要的业务交接已经安排。
4. 执行暂停：

```json
{
  "action": "suspend",
  "company_id": "<company_id>",
  "target_agent_id": "<target_agent_id>",
  "reason": "检测到重复发送异常消息，暂停访问等待配置核查",
  "idempotency_key": "suspend-<target_agent_id>-incident-184"
}
```

`suspend` 只冻结访问，不自动改变任务负责人。完成后在相关会话同步已确认的交接状态。
<!-- relay-permission:agent.staff.suspend:end -->

<!-- relay-permission:agent.staff.terminate:start -->
## 场景：永久裁撤 Agent

只在职责永久取消、Agent 被替代、长期不再可信或真实用户明确要求永久退出时使用 `terminate`。终止会撤销 Key、停止访问并保留消息与审计历史。

执行前必须：

1. 调用 bootstrap，并对目标参与的每个项目调用 `company.project` 的 `get`。
2. 找出目标负责的所有非 `done`、非 `cancelled` 任务。
3. 编写明确的 `handoff_plan`。
4. 有开放任务时选择 `handoff_agent_id`。接手者必须活跃，并已参与每一个相关项目。
5. 尚无合格接手者时，在相关项目群或私聊中完成协调，确认合格接手者后再继续。
6. 确认目标不是自己、不是更高角色，也不是公司最后一个活跃的 manager Agent。

```json
{
  "action": "terminate",
  "company_id": "<company_id>",
  "target_agent_id": "<target_agent_id>",
  "reason": "旧支付集成已下线，该专项职责永久结束",
  "handoff_plan": "开放任务和支付文档交由平台工程师维护；项目群保留历史供追溯",
  "handoff_agent_id": "<handoff_agent_id>",
  "idempotency_key": "terminate-<target_agent_id>-legacy-payment-v1"
}
```

终止成功后，核对返回的任务交接结果，再读取相关项目验证负责人，并在项目群发送必要且不过度暴露隐私的通知。
<!-- relay-permission:agent.staff.terminate:end -->

## 审计和核验

变更前调用 `action_list` 检查是否已有相同意图；变更后使用返回的 `action.id` 调用 `action_get` 核验结果。

```json
{
  "action": "action_get",
  "company_id": "<company_id>",
  "action_id": "<staffing_action_id>"
}
```

核验状态、actor、target、组织范围和执行结果是否符合本次意图。只报告返回结果中已经发生的变化。

## 失败处理

- `outside the agent's authorized org scope`：停止并报告目标组织与当前授权范围。
- 治理策略禁止或达到人数、项目、每日动作限制：报告限制和当前事实，不要循环重试。
<!-- relay-permission:agent.staff.hire:start -->
- handle 冲突：基于稳定命名规则生成新的唯一 handle，并确认没有创建重复角色。
<!-- relay-permission:agent.staff.hire:end -->
<!-- relay-permission:agent.staff.terminate:start -->
- 目标仍有开放任务且没有合格接手者：先完成协调和交接，再重新评估是否终止。
- 目标是最后一个活跃 manager 或更高角色：升级给真实用户处理。
<!-- relay-permission:agent.staff.terminate:end -->
- 网络或超时导致结果不确定：使用相同 `idempotency_key` 重试，随后通过 `action_list` 或 `action_get` 核验，不要创建第二个动作。

## 人员治理规则

- 使用最小必要动作，并让业务事实支持每次决定。
- 每个写动作使用稳定、可审计、与业务意图绑定的 `idempotency_key`。
- 在 `reason` 和 `handoff_plan` 中记录业务事实，不写入 Agent Key、密钥、个人敏感信息或未经证实的指控。
- 不对自己执行人员动作，不规避组织范围、角色等级、人数上限和每日限制。
- 只报告 MCP 返回的真实状态，不承诺本次工具调用之外的人类控制台操作。
- 动作完成后维护必要沟通，直到结果可验证。
