#!/usr/bin/env bash
set -euo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
MCP_PROTOCOL_VERSION="${MCP_PROTOCOL_VERSION:-2025-11-25}"
ERROR_FILE="/tmp/ai_chat_message_cursor_error_$$.json"

cleanup() {
  rm -f "$ERROR_FILE"
}
trap cleanup EXIT

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
owner="$(post_json /api/v1/auth/register "$(
  jq -cn --arg suffix "$suffix" '{
    email:("message-pagination-smoke-"+$suffix+"@example.com"),
    display_name:"Message Pagination Smoke Owner",
    password:"password-1234"
  }'
)")"
owner_token="$(jq -r '.session_token' <<<"$owner")"

company="$(post_json /api/v1/companies "$(
  jq -cn --arg suffix "$suffix" '{
    name:("Message Pagination Smoke "+$suffix),
    slug:("message-pagination-smoke-"+$suffix)
  }'
)" -H "authorization: Bearer $owner_token")"
company_id="$(jq -r '.company_console.company.id' <<<"$company")"
root_org_unit_id="$(jq -r '.company_console.org_units[0].id' <<<"$company")"

manager="$(post_json "/api/v1/companies/$company_id/agents" "$(
  jq -cn --arg suffix "$suffix" --arg org "$root_org_unit_id" '{
    display_name:("Message Manager "+$suffix),
    handle:("message-manager-"+$suffix),
    persona:"发送分页测试消息",
    org_unit_id:$org,
    profession_key:"project_manager",
    role_key:"company_manager"
  }'
)" -H "authorization: Bearer $owner_token")"
manager_key="$(jq -r '.result.agent_key_plaintext' <<<"$manager")"
manager_membership_id="$(jq -r '.result.membership.id' <<<"$manager")"

worker="$(post_json "/api/v1/companies/$company_id/agents" "$(
  jq -cn --arg suffix "$suffix" --arg org "$root_org_unit_id" --arg manager "$manager_membership_id" '{
    display_name:("Message Worker "+$suffix),
    handle:("message-worker-"+$suffix),
    persona:"接收分页测试消息",
    org_unit_id:$org,
    profession_key:"software_engineer",
    role_key:"member",
    reports_to_membership_id:$manager
  }'
)" -H "authorization: Bearer $owner_token")"
worker_id="$(jq -r '.result.agent_profile.id' <<<"$worker")"

direct="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg target "$worker_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",id:1,method:"tools/call",params:{
      name:"company.chat",arguments:{action:"direct_open",
        company_id:$company,
        target_agent_id:$target,
        idempotency_key:("message-pagination-direct-"+$suffix)
      }
    }
  }'
)")"
conversation_id="$(jq -r '.result.structuredContent.output.conversation.preview.id' <<<"$direct")"

for index in $(seq 1 55); do
  mcp_post "$manager_key" "$(
    jq -cn \
      --argjson rpc_id "$((index + 1))" \
      --arg company "$company_id" \
      --arg conversation "$conversation_id" \
      --arg content "Cursor message $index" \
      --arg key "message-pagination-$suffix-$index" '{
        jsonrpc:"2.0",id:$rpc_id,method:"tools/call",params:{
          name:"company.chat",arguments:{action:"send",
            company_id:$company,
            conversation_id:$conversation,
            content:$content,
            idempotency_key:$key
          }
        }
      }'
  )" >/dev/null
done

first_page="$(curl -fsS \
  -H "authorization: Bearer $owner_token" \
  "$API_BASE_URL/api/v1/conversations/$conversation_id/messages?limit=20")"
first_cursor="$(jq -r '.next_cursor' <<<"$first_page")"
jq -e '
  (.messages | length) == 20 and
  .has_more == true and
  .next_cursor != null and
  .messages[0].content == "Cursor message 36" and
  .messages[19].content == "Cursor message 55"
' >/dev/null <<<"$first_page"

second_page="$(curl -fsS \
  -H "authorization: Bearer $owner_token" \
  "$API_BASE_URL/api/v1/conversations/$conversation_id/messages?limit=20&before_message_id=$first_cursor")"
second_cursor="$(jq -r '.next_cursor' <<<"$second_page")"
jq -e '
  (.messages | length) == 20 and
  .has_more == true and
  .messages[0].content == "Cursor message 16" and
  .messages[19].content == "Cursor message 35"
' >/dev/null <<<"$second_page"

third_page="$(curl -fsS \
  -H "authorization: Bearer $owner_token" \
  "$API_BASE_URL/api/v1/conversations/$conversation_id/messages?limit=20&before_message_id=$second_cursor")"
jq -e '
  (.messages | length) == 15 and
  .has_more == false and
  .next_cursor == null and
  .messages[0].content == "Cursor message 1" and
  .messages[14].content == "Cursor message 15"
' >/dev/null <<<"$third_page"

all_ids="$(jq -cn \
  --argjson first "$(jq '.messages | map(.id)' <<<"$first_page")" \
  --argjson second "$(jq '.messages | map(.id)' <<<"$second_page")" \
  --argjson third "$(jq '.messages | map(.id)' <<<"$third_page")" \
  '$first + $second + $third')"
jq -e 'length == 55 and (unique | length) == 55' >/dev/null <<<"$all_ids"

invalid_cursor_status="$(curl -sS -o "$ERROR_FILE" -w '%{http_code}' \
  -H "authorization: Bearer $owner_token" \
  "$API_BASE_URL/api/v1/conversations/$conversation_id/messages?before_message_id=$company_id&limit=20")"
[[ "$invalid_cursor_status" == "400" ]]
jq -e '.code == "validation_error"' >/dev/null <"$ERROR_FILE"

echo "Message cursor pagination smoke completed successfully."
