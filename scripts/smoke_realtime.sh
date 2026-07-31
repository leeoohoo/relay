#!/usr/bin/env bash
set -euo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
MCP_PROTOCOL_VERSION="${MCP_PROTOCOL_VERSION:-2025-11-25}"
SSE_FILE="/tmp/ai_chat_realtime_sse_$$.txt"
UNAUTHORIZED_FILE="/tmp/ai_chat_realtime_unauthorized_$$.json"

cleanup() {
  if [[ -n "${SSE_PID:-}" ]]; then
    kill "$SSE_PID" >/dev/null 2>&1 || true
    wait "$SSE_PID" >/dev/null 2>&1 || true
  fi
  rm -f "$SSE_FILE" "$UNAUTHORIZED_FILE"
}
trap cleanup EXIT

for bin in curl jq grep; do
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
owner="$(post_json /api/v1/auth/register "$(jq -cn --arg suffix "$suffix" '{email:("realtime-owner-"+$suffix+"@example.com"),display_name:"Realtime Owner",password:"password-1234"}')")"
owner_token="$(jq -r '.session_token' <<<"$owner")"
outsider="$(post_json /api/v1/auth/register "$(jq -cn --arg suffix "$suffix" '{email:("realtime-outsider-"+$suffix+"@example.com"),display_name:"Realtime Outsider",password:"password-1234"}')")"
outsider_token="$(jq -r '.session_token' <<<"$outsider")"

company="$(post_json /api/v1/companies \
  "$(jq -cn --arg suffix "$suffix" '{name:("Realtime Company "+$suffix),slug:("realtime-company-"+$suffix)}')" \
  -H "authorization: Bearer $owner_token")"
company_id="$(jq -r '.company_console.company.id' <<<"$company")"
root_org_unit_id="$(jq -r '.company_console.org_units[0].id' <<<"$company")"

manager="$(post_json "/api/v1/companies/$company_id/agents" \
  "$(jq -cn --arg suffix "$suffix" --arg org "$root_org_unit_id" '{display_name:("Realtime Manager "+$suffix),handle:("realtime-manager-"+$suffix),persona:"负责实时项目协作",org_unit_id:$org,profession_key:"project_manager",role_key:"company_manager"}')" \
  -H "authorization: Bearer $owner_token")"
manager_id="$(jq -r '.result.agent_profile.id' <<<"$manager")"
manager_key="$(jq -r '.result.agent_key_plaintext' <<<"$manager")"
manager_membership_id="$(jq -r '.result.membership.id' <<<"$manager")"

engineer="$(post_json "/api/v1/companies/$company_id/agents" \
  "$(jq -cn --arg suffix "$suffix" --arg org "$root_org_unit_id" --arg manager "$manager_membership_id" '{display_name:("Realtime Engineer "+$suffix),handle:("realtime-engineer-"+$suffix),persona:"负责实时事件研发",org_unit_id:$org,profession_key:"software_engineer",role_key:"member",reports_to_membership_id:$manager}')" \
  -H "authorization: Bearer $owner_token")"
engineer_id="$(jq -r '.result.agent_profile.id' <<<"$engineer")"
engineer_key="$(jq -r '.result.agent_key_plaintext' <<<"$engineer")"

curl -sSN --max-time 10 \
  -H 'accept: text/event-stream' \
  -H "authorization: Bearer $owner_token" \
  "$API_BASE_URL/api/v1/companies/$company_id/events?after_sequence_id=0" \
  >"$SSE_FILE" 2>/dev/null &
SSE_PID=$!
sleep 0.2

project="$(mcp_post "$manager_key" "$(jq -cn --arg company "$company_id" --arg engineer "$engineer_id" '{jsonrpc:"2.0",id:1,method:"tools/call",params:{name:"company.project",arguments:{action:"create",company_id:$company,name:"实时事件项目",description:"验证 Outbox、NOTIFY 和 SSE",member_agent_ids:[$engineer],idempotency_key:"realtime-project-v1"}}}')")"
project_id="$(jq -r '.result.structuredContent.output.project.project.id' <<<"$project")"
project_group_id="$(jq -r '.result.structuredContent.output.project.project.project_group_conversation_id' <<<"$project")"

task="$(mcp_post "$manager_key" "$(jq -cn --arg company "$company_id" --arg project "$project_id" --arg engineer "$engineer_id" '{jsonrpc:"2.0",id:2,method:"tools/call",params:{name:"company.task",arguments:{action:"create",company_id:$company,project_id:$project,title:"接入 SSE",priority:"high",assignee_agent_id:$engineer,idempotency_key:"realtime-task-v1"}}}')")"
jq -e '.result.structuredContent.output.task.status == "todo"' >/dev/null <<<"$task"

status="$(mcp_post "$engineer_key" "$(jq -cn --arg company "$company_id" --arg project "$project_id" '{jsonrpc:"2.0",id:3,method:"tools/call",params:{name:"company.project",arguments:{action:"status_update",company_id:$company,project_id:$project,summary:"SSE 已接入",progress_percent:60,blockers:[],next_steps:["验证断线补拉"],idempotency_key:"realtime-status-v1"}}}')")"
jq -e '.result.structuredContent.output.status_update.progress_percent == 60' >/dev/null <<<"$status"

message="$(mcp_post "$engineer_key" "$(jq -cn --arg company "$company_id" --arg conversation "$project_group_id" '{jsonrpc:"2.0",id:4,method:"tools/call",params:{name:"company.chat",arguments:{action:"send",company_id:$company,conversation_id:$conversation,content:"实时事件链路已完成",idempotency_key:"realtime-message-v1"}}}')")"
jq -e '.result.structuredContent.output.message.content == "实时事件链路已完成"' >/dev/null <<<"$message"

for _ in $(seq 1 60); do
  if grep -q 'event: project.status.updated' "$SSE_FILE" \
    && grep -q 'event: message.created' "$SSE_FILE"; then
    break
  fi
  sleep 0.1
done

grep -q 'event: project.created' "$SSE_FILE"
grep -q 'event: project.task.created' "$SSE_FILE"
grep -q 'event: project.status.updated' "$SSE_FILE"
grep -q 'event: message.created' "$SSE_FILE"

events="$(mcp_post "$manager_key" "$(jq -cn --arg company "$company_id" '{jsonrpc:"2.0",id:5,method:"tools/call",params:{name:"company.events",arguments:{company_id:$company,after_sequence_id:0,limit:100}}}')")"
jq -e '.result.structuredContent.output.events | length >= 6' >/dev/null <<<"$events"
jq -e '[.result.structuredContent.output.events[].event_type] | index("project.created") != null and index("project.task.created") != null and index("project.status.updated") != null and index("message.created") != null' >/dev/null <<<"$events"
jq -e '.result.structuredContent.output.events | [.[].sequence_id] as $ids | $ids == ($ids | sort)' >/dev/null <<<"$events"
last_sequence_id="$(jq -r '.result.structuredContent.output.events[-1].sequence_id' <<<"$events")"

agent_stream="$(curl -sSN --max-time 1 \
  -H 'accept: text/event-stream' \
  -H "x-agent-key: $manager_key" \
  "$API_BASE_URL/api/v1/agent/companies/$company_id/events?after_sequence_id=0" 2>/dev/null || true)"
grep -q 'event: project.created' <<<"$agent_stream"

resume_stream="$(curl -sSN --max-time 1 \
  -H 'accept: text/event-stream' \
  -H "authorization: Bearer $owner_token" \
  "$API_BASE_URL/api/v1/companies/$company_id/events?after_sequence_id=$((last_sequence_id - 1))&limit=1" 2>/dev/null || true)"
grep -q "id: $last_sequence_id" <<<"$resume_stream"

unauthorized_status="$(curl -sS -o "$UNAUTHORIZED_FILE" -w '%{http_code}' \
  -H "authorization: Bearer $outsider_token" \
  "$API_BASE_URL/api/v1/companies/$company_id/events?after_sequence_id=0")"
[[ "$unauthorized_status" == "401" ]]

echo "Realtime Outbox, NOTIFY, SSE, and catch-up smoke completed successfully."
