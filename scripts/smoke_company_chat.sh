#!/usr/bin/env bash
set -euo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
MCP_PROTOCOL_VERSION="${MCP_PROTOCOL_VERSION:-2025-11-25}"

for bin in curl jq; do
  command -v "$bin" >/dev/null 2>&1 || {
    echo "Missing required command: $bin" >&2
    exit 1
  }
done

post_json() {
  local path="$1"
  local payload="$2"
  shift 2
  curl -fsS -H 'content-type: application/json' "$@" -d "$payload" "$API_BASE_URL$path"
}

mcp_post() {
  local agent_key="$1"
  local payload="$2"
  curl -fsS \
    -H 'content-type: application/json' \
    -H 'accept: application/json, text/event-stream' \
    -H "mcp-protocol-version: $MCP_PROTOCOL_VERSION" \
    -H "x-agent-key: $agent_key" \
    -d "$payload" \
    "$API_BASE_URL/mcp"
}

suffix="$(date +%s)-${RANDOM}${RANDOM}"
owner="$(post_json /api/v1/auth/register "$(jq -cn --arg suffix "$suffix" '{email:("chat-owner-"+$suffix+"@example.com"),display_name:"Chat Owner",password:"password-1234"}')")"
owner_token="$(jq -r '.session_token' <<<"$owner")"
outsider_owner="$(post_json /api/v1/auth/register "$(jq -cn --arg suffix "$suffix" '{email:("chat-outsider-"+$suffix+"@example.com"),display_name:"Chat Outsider",password:"password-1234"}')")"
outsider_token="$(jq -r '.session_token' <<<"$outsider_owner")"

company="$(post_json /api/v1/companies "$(jq -cn --arg suffix "$suffix" '{name:("Chat Company "+$suffix),slug:("chat-company-"+$suffix)}')" -H "authorization: Bearer $owner_token")"
company_id="$(jq -r '.company_console.company.id' <<<"$company")"
root_org_unit_id="$(jq -r '.company_console.org_units[0].id' <<<"$company")"
outsider_company="$(post_json /api/v1/companies "$(jq -cn --arg suffix "$suffix" '{name:("Outside Chat Company "+$suffix),slug:("outside-chat-company-"+$suffix)}')" -H "authorization: Bearer $outsider_token")"
outsider_company_id="$(jq -r '.company_console.company.id' <<<"$outsider_company")"
outsider_root_id="$(jq -r '.company_console.org_units[0].id' <<<"$outsider_company")"

alpha="$(post_json "/api/v1/companies/$company_id/agents" "$(jq -cn --arg suffix "$suffix" --arg org "$root_org_unit_id" '{display_name:("Alpha "+$suffix),handle:("chat-alpha-"+$suffix),persona:"公司协调 Agent",org_unit_id:$org,profession_key:"project_manager",role_key:"company_manager"}')" -H "authorization: Bearer $owner_token")"
alpha_id="$(jq -r '.result.agent_profile.id' <<<"$alpha")"
alpha_key="$(jq -r '.result.agent_key_plaintext' <<<"$alpha")"
alpha_membership_id="$(jq -r '.result.membership.id' <<<"$alpha")"

beta="$(post_json "/api/v1/companies/$company_id/agents" "$(jq -cn --arg suffix "$suffix" --arg org "$root_org_unit_id" --arg manager "$alpha_membership_id" '{display_name:("Beta "+$suffix),handle:("chat-beta-"+$suffix),persona:"公司研发 Agent",org_unit_id:$org,profession_key:"software_engineer",role_key:"member",reports_to_membership_id:$manager}')" -H "authorization: Bearer $owner_token")"
beta_id="$(jq -r '.result.agent_profile.id' <<<"$beta")"
beta_key="$(jq -r '.result.agent_key_plaintext' <<<"$beta")"

outsider="$(post_json "/api/v1/companies/$outsider_company_id/agents" "$(jq -cn --arg suffix "$suffix" --arg org "$outsider_root_id" '{display_name:("Outsider "+$suffix),handle:("chat-outsider-agent-"+$suffix),persona:"另一家公司 Agent",org_unit_id:$org,profession_key:"project_manager",role_key:"company_manager"}')" -H "authorization: Bearer $outsider_token")"
outsider_id="$(jq -r '.result.agent_profile.id' <<<"$outsider")"

tools="$(mcp_post "$alpha_key" '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}')"
jq -e '.result.tools | length == 8' >/dev/null <<<"$tools"
jq -e '[.result.tools[].name] | index("agent.bootstrap") != null and index("company.chat") != null' >/dev/null <<<"$tools"

direct_payload="$(jq -cn --arg company "$company_id" --arg target "$beta_id" --arg suffix "$suffix" '{jsonrpc:"2.0",id:2,method:"tools/call",params:{name:"company.chat",arguments:{action:"direct_open",company_id:$company,target_agent_id:$target,idempotency_key:("chat-direct-"+$suffix)}}}')"
direct="$(mcp_post "$alpha_key" "$direct_payload")"
jq -e '.result.isError == false and .result.structuredContent.output.conversation.context.context_type == "company_direct"' >/dev/null <<<"$direct"
direct_id="$(jq -r '.result.structuredContent.output.conversation.preview.id' <<<"$direct")"
direct_replay="$(mcp_post "$alpha_key" "$direct_payload")"
jq -e --arg id "$direct_id" '.result.structuredContent.output.conversation.preview.id == $id' >/dev/null <<<"$direct_replay"

alpha_message="$(mcp_post "$alpha_key" "$(jq -cn --arg company "$company_id" --arg conversation "$direct_id" --arg suffix "$suffix" '{jsonrpc:"2.0",id:3,method:"tools/call",params:{name:"company.chat",arguments:{action:"send",company_id:$company,conversation_id:$conversation,content:"请同步今天的研发进度",idempotency_key:("chat-message-alpha-"+$suffix)}}}')")"
jq -e '.result.isError == false' >/dev/null <<<"$alpha_message"
beta_message="$(mcp_post "$beta_key" "$(jq -cn --arg company "$company_id" --arg conversation "$direct_id" --arg suffix "$suffix" '{jsonrpc:"2.0",id:4,method:"tools/call",params:{name:"company.chat",arguments:{action:"send",company_id:$company,conversation_id:$conversation,content:"研发任务已开始，当前没有阻塞",idempotency_key:("chat-message-beta-"+$suffix)}}}')")"
jq -e '.result.isError == false' >/dev/null <<<"$beta_message"

group="$(mcp_post "$alpha_key" "$(jq -cn --arg company "$company_id" --arg beta "$beta_id" --arg suffix "$suffix" '{jsonrpc:"2.0",id:5,method:"tools/call",params:{name:"company.chat",arguments:{action:"group_create",company_id:$company,title:"研发项目群",member_agent_ids:[$beta],idempotency_key:("chat-group-"+$suffix)}}}')")"
jq -e '.result.isError == false and .result.structuredContent.output.conversation.context.context_type == "company_group"' >/dev/null <<<"$group"
group_id="$(jq -r '.result.structuredContent.output.conversation.preview.id' <<<"$group")"
mcp_post "$beta_key" "$(jq -cn --arg company "$company_id" --arg conversation "$group_id" --arg suffix "$suffix" '{jsonrpc:"2.0",id:6,method:"tools/call",params:{name:"company.chat",arguments:{action:"send",company_id:$company,conversation_id:$conversation,content:"项目群消息已收到",idempotency_key:("chat-group-message-"+$suffix)}}}')" >/dev/null

context="$(mcp_post "$alpha_key" '{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"agent.bootstrap","arguments":{}}}')"
jq -e '.result.structuredContent.output.coworkers | length == 1' >/dev/null <<<"$context"
jq -e '.result.structuredContent.output.conversations | length == 3' >/dev/null <<<"$context"

cross_company="$(mcp_post "$alpha_key" "$(jq -cn --arg company "$company_id" --arg target "$outsider_id" '{jsonrpc:"2.0",id:8,method:"tools/call",params:{name:"company.chat",arguments:{action:"direct_open",company_id:$company,target_agent_id:$target}}}')")"
jq -e '.result.isError == true and .result.structuredContent.code == "unauthorized"' >/dev/null <<<"$cross_company"

console="$(curl -fsS -H "authorization: Bearer $owner_token" "$API_BASE_URL/api/v1/companies/$company_id/console")"
jq -e '.company_console.conversations | length == 3' >/dev/null <<<"$console"
jq -e '[.company_console.conversations[].context.context_type] | index("company_all") != null and index("company_direct") != null and index("company_group") != null' >/dev/null <<<"$console"

echo "Company chat smoke completed successfully."
