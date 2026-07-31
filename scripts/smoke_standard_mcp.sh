#!/usr/bin/env bash
set -euo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
MCP_PROTOCOL_VERSION="${MCP_PROTOCOL_VERSION:-2025-11-25}"

for bin in curl jq; do
  command -v "$bin" >/dev/null 2>&1 || { echo "Missing required command: $bin" >&2; exit 1; }
done

post_json() {
  local path="$1" payload="$2"
  shift 2
  curl -fsS -H 'content-type: application/json' "$@" -d "$payload" "$API_BASE_URL$path"
}

mcp_post() {
  local agent_key="$1" payload="$2"
  curl -fsS \
    -H 'content-type: application/json' \
    -H 'accept: application/json, text/event-stream' \
    -H "mcp-protocol-version: $MCP_PROTOCOL_VERSION" \
    -H "x-agent-key: $agent_key" \
    -d "$payload" \
    "$API_BASE_URL/mcp"
}

assert_json() {
  local payload="$1" filter="$2" message="$3"
  if ! jq -e "$filter" >/dev/null <<<"$payload"; then
    echo "$message" >&2
    jq . <<<"$payload" >&2
    exit 1
  fi
}

unauthenticated_tools="$(curl -fsS \
  -H 'content-type: application/json' \
  -H 'accept: application/json, text/event-stream' \
  -H "mcp-protocol-version: $MCP_PROTOCOL_VERSION" \
  -d '{"jsonrpc":"2.0","id":0,"method":"tools/list","params":{}}' \
  "$API_BASE_URL/mcp")"
assert_json "$unauthenticated_tools" \
  '.error.code == -32600 and .error.data.code == "unauthorized"' \
  "MCP tools/list must require an Agent Key"

suffix="$(date +%s)-${RANDOM}${RANDOM}"
owner="$(post_json /api/v1/auth/register "$(jq -cn --arg suffix "$suffix" '{email:("mcp-owner-"+$suffix+"@example.com"),display_name:"MCP Owner",password:"password-1234"}')")"
owner_token="$(jq -r '.session_token' <<<"$owner")"
company="$(post_json /api/v1/companies "$(jq -cn --arg suffix "$suffix" '{name:("MCP Company "+$suffix),slug:("mcp-company-"+$suffix)}')" -H "authorization: Bearer $owner_token")"
company_id="$(jq -r '.company_console.company.id' <<<"$company")"
org_id="$(jq -r '.company_console.org_units[0].id' <<<"$company")"

alpha="$(post_json "/api/v1/companies/$company_id/agents" "$(jq -cn --arg suffix "$suffix" --arg org "$org_id" '{display_name:"Alpha",handle:("alpha-"+$suffix),persona:"负责协调公司通信",org_unit_id:$org,profession_key:"project_manager",role_key:"company_manager"}')" -H "authorization: Bearer $owner_token")"
alpha_id="$(jq -r '.result.agent_profile.id' <<<"$alpha")"
alpha_key="$(jq -r '.result.agent_key_plaintext' <<<"$alpha")"
alpha_membership_id="$(jq -r '.result.membership.id' <<<"$alpha")"

beta="$(post_json "/api/v1/companies/$company_id/agents" "$(jq -cn --arg suffix "$suffix" --arg org "$org_id" --arg manager "$alpha_membership_id" '{display_name:"Beta",handle:("beta-"+$suffix),persona:"负责执行与反馈",org_unit_id:$org,profession_key:"software_engineer",role_key:"member",reports_to_membership_id:$manager}')" -H "authorization: Bearer $owner_token")"
beta_id="$(jq -r '.result.agent_profile.id' <<<"$beta")"
beta_key="$(jq -r '.result.agent_key_plaintext' <<<"$beta")"

initialize="$(mcp_post "$alpha_key" "$(jq -cn --arg version "$MCP_PROTOCOL_VERSION" '{jsonrpc:"2.0",id:1,method:"initialize",params:{protocolVersion:$version,capabilities:{},clientInfo:{name:"agent-company-smoke",version:"0.1.0"}}}')")"
assert_json "$initialize" '.result.serverInfo.name == "ai-chat"' "MCP initialize failed"

tools="$(mcp_post "$alpha_key" '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}')"
assert_json "$tools" '.result.tools | length == 8' "Expected the compact eight-tool Agent surface"
assert_json "$tools" '[.result.tools[].name] | index("agent.bootstrap") != null and index("agent.profile.update") != null and index("agent.inbox.wait") != null and index("agent.inbox.ack") != null and index("company.chat") != null and index("company.project") != null and index("company.task") != null and index("company.events") != null' "Compact Agent Company tools are missing"
assert_json "$tools" '[.result.tools[].name] | index("company.chat.message.send") == null and index("company.project.task.update") == null and index("company.staff.hire") == null' "Legacy granular tools must not be exposed"
legacy_call="$(mcp_post "$alpha_key" '{"jsonrpc":"2.0","id":20,"method":"tools/call","params":{"name":"company.chat.message.send","arguments":{}}}')"
assert_json "$legacy_call" '.error.code == -32601' "Legacy granular tool names must be rejected"

bootstrap="$(mcp_post "$alpha_key" '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"agent.bootstrap","arguments":{}}}')"
assert_json "$bootstrap" ".result.isError == false and .result.structuredContent.output.agent.id == \"$alpha_id\"" "agent.bootstrap returned the wrong identity"
assert_json "$bootstrap" ".result.structuredContent.output.company.id == \"$company_id\" and (.result.structuredContent.output.coworkers | length) == 1" "agent.bootstrap company context is incomplete"
company_group_id="$(jq -r '.result.structuredContent.output.conversations | map(select(.context.context_type == "company_all")) | first | .preview.id' <<<"$bootstrap")"
[[ -n "$company_group_id" && "$company_group_id" != "null" ]]

work_profile="$(mcp_post "$beta_key" "$(jq -cn --arg suffix "$suffix" '{jsonrpc:"2.0",id:36,method:"tools/call",params:{name:"agent.profile.update",arguments:{responsibilities:["实现公司 MCP 接口","维护消息投递链路"],skills:["Rust","PostgreSQL"],current_focus:"完成 Agent 通信闭环",collaboration_preference:"low_cost_only",idempotency_key:("work-profile-"+$suffix)}}}')")"
assert_json "$work_profile" '.result.isError == false and .result.structuredContent.output.work_profile.membership.current_focus == "完成 Agent 通信闭环" and (.result.structuredContent.output.work_profile.membership.skills | length) == 2' "Beta could not update its structured work profile"
alpha_context="$(mcp_post "$alpha_key" '{"jsonrpc":"2.0","id":37,"method":"tools/call","params":{"name":"agent.bootstrap","arguments":{}}}')"
assert_json "$alpha_context" ".result.structuredContent.output.coworkers | map(select(.agent_profile.id == \"$beta_id\" and .membership.current_focus == \"完成 Agent 通信闭环\" and (.membership.skills | index(\"Rust\")) != null)) | length == 1" "Alpha could not discover Beta's structured work profile"

empty_wait="$(mcp_post "$beta_key" '{"jsonrpc":"2.0","id":30,"method":"tools/call","params":{"name":"agent.inbox.wait","arguments":{"timeout_seconds":1,"event_types":["message.received"],"limit":20}}}')"
assert_json "$empty_wait" '.result.structuredContent.output.timed_out == true and (.result.structuredContent.output.events | length) == 0' "Empty inbox wait did not time out cleanly"

mcp_post "$alpha_key" "$(jq -cn --arg company "$company_id" --arg conversation "$company_group_id" --arg suffix "$suffix" '{jsonrpc:"2.0",id:31,method:"tools/call",params:{name:"company.chat",arguments:{action:"send",company_id:$company,conversation_id:$conversation,content:"欢迎加入公司全员群。",idempotency_key:("company-all-message-"+$suffix)}}}')" >/dev/null
group_unread="$(mcp_post "$beta_key" "$(jq -cn --arg company "$company_id" --arg conversation "$company_group_id" '{jsonrpc:"2.0",id:32,method:"tools/call",params:{name:"company.chat",arguments:{action:"unread",company_id:$company,conversation_id:$conversation,message_limit:20}}}')")"
assert_json "$group_unread" '.result.structuredContent.output.unread.total_unread_count == 1 and .result.structuredContent.output.unread.groups[0].unread_messages[0].content == "欢迎加入公司全员群。"' "Beta did not see the unread default company group message"
group_read="$(mcp_post "$beta_key" "$(jq -cn --arg company "$company_id" --arg conversation "$company_group_id" --arg suffix "$suffix" '{jsonrpc:"2.0",id:33,method:"tools/call",params:{name:"company.chat",arguments:{action:"mark_read",company_id:$company,conversation_id:$conversation,idempotency_key:("company-all-read-"+$suffix)}}}')")"
assert_json "$group_read" '.result.structuredContent.output.result.marked_read_count == 1' "Beta could not mark the default company group as read"
group_unread_after_read="$(mcp_post "$beta_key" "$(jq -cn --arg company "$company_id" --arg conversation "$company_group_id" '{jsonrpc:"2.0",id:34,method:"tools/call",params:{name:"company.chat",arguments:{action:"unread",company_id:$company,conversation_id:$conversation,message_limit:20}}}')")"
assert_json "$group_unread_after_read" '.result.structuredContent.output.unread.total_unread_count == 0' "Beta still saw the company group message as unread after marking it read"
group_history="$(mcp_post "$beta_key" "$(jq -cn --arg company "$company_id" --arg conversation "$company_group_id" '{jsonrpc:"2.0",id:35,method:"tools/call",params:{name:"company.chat",arguments:{action:"history",company_id:$company,conversation_id:$conversation,limit:20}}}')")"
assert_json "$group_history" '.result.structuredContent.output.messages | map(select(.content == "欢迎加入公司全员群。")) | length == 1' "Beta could not read the company group history after marking the message read"

direct="$(mcp_post "$alpha_key" "$(jq -cn --arg company "$company_id" --arg target "$beta_id" --arg suffix "$suffix" '{jsonrpc:"2.0",id:4,method:"tools/call",params:{name:"company.chat",arguments:{action:"direct_open",company_id:$company,target_agent_id:$target,idempotency_key:("direct-"+$suffix)}}}')")"
conversation_id="$(jq -r '.result.structuredContent.output.conversation.preview.id' <<<"$direct")"

mcp_post "$alpha_key" "$(jq -cn --arg company "$company_id" --arg conversation "$conversation_id" --arg suffix "$suffix" '{jsonrpc:"2.0",id:5,method:"tools/call",params:{name:"company.chat",arguments:{action:"send",company_id:$company,conversation_id:$conversation,content:"请确认你已接入公司 MCP。",idempotency_key:("alpha-message-"+$suffix)}}}')" >/dev/null

beta_inbox="$(mcp_post "$beta_key" '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"agent.inbox.wait","arguments":{"timeout_seconds":2,"event_types":["message.received"],"limit":20}}}')"
assert_json "$beta_inbox" '.result.structuredContent.output.timed_out == false and (.result.structuredContent.output.events | length) >= 1' "Beta did not receive Alpha's message through inbox wait"
beta_event_id="$(jq -r '.result.structuredContent.output.events | map(select(.event_type == "message.received")) | first | .id' <<<"$beta_inbox")"
beta_reply="$(mcp_post "$beta_key" "$(jq -cn --arg event "$beta_event_id" --arg suffix "$suffix" '{jsonrpc:"2.0",id:7,method:"tools/call",params:{name:"company.chat",arguments:{action:"reply",event_id:$event,content:"已接入，可以开始协作。",auto_ack:true,idempotency_key:("beta-reply-"+$suffix)}}}')")"
assert_json "$beta_reply" '.result.isError == false and .result.structuredContent.output.event.status == "processed"' "Beta reply did not auto-ack the inbox event"

alpha_inbox="$(mcp_post "$alpha_key" '{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"agent.inbox.wait","arguments":{"timeout_seconds":2,"event_types":["message.received"],"limit":20}}}')"
assert_json "$alpha_inbox" '.result.structuredContent.output.events | map(select(.event_type == "message.received")) | length >= 1' "Alpha did not receive Beta's reply"

jq -n --arg company_id "$company_id" --arg alpha_id "$alpha_id" --arg beta_id "$beta_id" --arg conversation_id "$conversation_id" --arg company_group_id "$company_group_id" '{status:"ok",company_id:$company_id,agents:[$alpha_id,$beta_id],company_group_id:$company_group_id,conversation_id:$conversation_id,tool_count:8}'
