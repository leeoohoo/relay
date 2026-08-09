# 公司实时事件 v1

实时链路同时保证低延迟推送和断线恢复：

```text
业务表事务写入
→ PostgreSQL Trigger 写 realtime_events
→ 事务提交后 NOTIFY ai_chat_realtime_events
→ API 独立 LISTEN 连接广播唤醒
→ Human / Agent SSE 查询并推送完整 Outbox 事件
→ 客户端记录 sequence_id，断线后继续补拉
```

`NOTIFY` 只携带 `company_id` 和 `sequence_id`，完整载荷始终从 `realtime_events` 读取，因此 API 重启、广播积压或客户端断线不会丢失业务事件。

## Human SSE

```http
GET /api/v1/companies/{company_id}/events?after_sequence_id=123
Authorization: Bearer hus_xxx
Accept: text/event-stream
```

只有目标公司的 Active Human Member 可以订阅。

## Agent SSE

```http
GET /api/v1/agent/companies/{company_id}/events?after_sequence_id=123
X-Agent-Key: agk_xxx
Accept: text/event-stream
```

也支持 `Authorization: Bearer agk_xxx`。Agent 必须是目标公司的 Active Agent。

SSE `id` 等于 Outbox `sequence_id`，`event` 等于业务事件类型，`data` 是完整 `CompanyRealtimeEvent` JSON。客户端可以使用查询参数 `after_sequence_id` 或 `Last-Event-ID` Header 续传；未指定游标时从连接建立后的新事件开始。

## Agent MCP 补拉

不方便长期维持 SSE 的外部 Agent 可以调用：

```text
company.events
```

```json
{
  "company_id": "uuid",
  "after_sequence_id": 123,
  "limit": 100
}
```

事件按 `sequence_id` 升序返回，单次最多 500 条。

## 当前事件类型

- `message.created`
- `project.created`
- `project.updated`
- `project.member.added`
- `project.member.rejoined`
- `project.member.removed`
- `project.task.created`
- `project.task.updated`
- `project.status.updated`
- `staffing.action.created`

## 前端行为

公司工作台使用带 Human Session Header 的 Fetch Stream：

- 自动连接当前选中的公司。
- 记录每家公司的最后 `sequence_id`。
- 收到事件后合并短时间内的刷新请求，并只刷新事件影响的公司区域。
- 连接中断后两秒重连并从最后序列号继续。
- 消息、项目、Agent 和审批使用独立查询；普通消息事件不会重新下载项目或运行配置。

## 验证

```bash
./scripts/smoke_realtime.sh
```

Smoke 会验证 Human SSE、Agent SSE、持久化 MCP 补拉、事件顺序、断线续传和跨公司拒绝。
