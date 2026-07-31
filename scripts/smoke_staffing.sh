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
registered="$(post_json /api/v1/auth/register "$(
  jq -cn --arg suffix "$suffix" '{
    email:("staffing-smoke-"+$suffix+"@example.com"),
    display_name:"Staffing Smoke Owner",
    password:"password-1234"
  }'
)")"
human_token="$(jq -r '.session_token' <<<"$registered")"

company="$(post_json /api/v1/companies "$(
  jq -cn --arg suffix "$suffix" '{
    name:("Staffing Smoke "+$suffix),
    slug:("staffing-smoke-"+$suffix)
  }'
)" -H "authorization: Bearer $human_token")"
company_id="$(jq -r '.company_console.company.id' <<<"$company")"
root_org_unit_id="$(jq -r '.company_console.org_units[0].id' <<<"$company")"

manager="$(post_json "/api/v1/companies/$company_id/agents" "$(
  jq -cn --arg suffix "$suffix" --arg org "$root_org_unit_id" '{
    display_name:("Staffing Manager "+$suffix),
    handle:("staffing-manager-"+$suffix),
    persona:"负责扩招和人员调整",
    org_unit_id:$org,
    profession_key:"project_manager",
    role_key:"company_manager"
  }'
)" -H "authorization: Bearer $human_token")"
manager_id="$(jq -r '.result.agent_profile.id' <<<"$manager")"
manager_key="$(jq -r '.result.agent_key_plaintext' <<<"$manager")"

permissions="$(post_json "/api/v1/companies/$company_id/agents/$manager_id/permissions" \
  '{"staffing_permissions":["agent.staff.hire","agent.staff.suspend","agent.staff.terminate"],"reason":"CI Staffing smoke"}' \
  -H "authorization: Bearer $human_token")"
jq -e '.membership.permissions | index("agent.staff.hire") != null and index("agent.staff.suspend") != null and index("agent.staff.terminate") != null' >/dev/null <<<"$permissions"

staffing_tools="$(mcp_post "$manager_key" '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}')"
jq -e '.result.tools | length == 9' >/dev/null <<<"$staffing_tools"
jq -e '[.result.tools[].name] | index("company.staff") != null and index("company.staff.hire") == null' >/dev/null <<<"$staffing_tools"

hire_payload="$(jq -cn --arg company "$company_id" --arg org "$root_org_unit_id" --arg suffix "$suffix" '{
  jsonrpc:"2.0",id:2,method:"tools/call",params:{
    name:"company.staff",arguments:{action:"hire",
      company_id:$company,
      display_name:("Staffing Engineer "+$suffix),
      handle:("staffing-engineer-"+$suffix),
      persona:"负责工程实现",
      org_unit_id:$org,
      job_title:"工程师",
      reason:"CI 扩招",
      idempotency_key:("staffing-hire-"+$suffix)
    }
  }
}')"
first_hire="$(mcp_post "$manager_key" "$hire_payload")"
second_hire="$(mcp_post "$manager_key" "$hire_payload")"
engineer_id="$(jq -r '.result.structuredContent.output.result.agent_profile.id' <<<"$first_hire")"
jq -e --arg id "$engineer_id" '.result.structuredContent.output.result.agent_profile.id == $id and .result.structuredContent.output.result.membership.employment_status == "provisioning" and (.result.structuredContent.output.result | has("agent_key_plaintext") | not)' >/dev/null <<<"$second_hire"

activated="$(post_json "/api/v1/companies/$company_id/agents/$engineer_id/activate" \
  '{"reason":"CI Runtime ready"}' \
  -H "authorization: Bearer $human_token")"
engineer_key="$(jq -r '.result.agent_key_plaintext' <<<"$activated")"

suspend_payload="$(jq -cn --arg company "$company_id" --arg target "$engineer_id" --arg suffix "$suffix" '{
  jsonrpc:"2.0",id:3,method:"tools/call",params:{
    name:"company.staff",arguments:{action:"suspend",company_id:$company,target_agent_id:$target,reason:"CI 暂停",idempotency_key:("staffing-suspend-"+$suffix)}
  }
}')"
suspended="$(mcp_post "$manager_key" "$suspend_payload")"
jq -e '.result.structuredContent.output.result.membership.employment_status == "suspended" and .result.structuredContent.output.result.revoked_key_count == 1' >/dev/null <<<"$suspended"

old_key_response="$(mcp_post "$engineer_key" \
  '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"agent.bootstrap","arguments":{}}}')"
jq -e '.result.isError == true and .result.structuredContent.code == "unauthorized"' \
  >/dev/null <<<"$old_key_response"

reactivated="$(post_json "/api/v1/companies/$company_id/agents/$engineer_id/reactivate" \
  '{"reason":"CI 恢复"}' \
  -H "authorization: Bearer $human_token")"
new_engineer_key="$(jq -r '.result.agent_key_plaintext' <<<"$reactivated")"

terminate_payload="$(jq -cn --arg company "$company_id" --arg target "$engineer_id" --arg suffix "$suffix" '{
  jsonrpc:"2.0",id:5,method:"tools/call",params:{
    name:"company.staff",arguments:{action:"terminate",
      company_id:$company,
      target_agent_id:$target,
      reason:"CI 裁撤",
      handoff_plan:"所有上下文和未完成工作交回直属管理 Agent",
      idempotency_key:("staffing-terminate-"+$suffix)
    }
  }
}')"
terminated="$(mcp_post "$manager_key" "$terminate_payload")"
jq -e '.result.structuredContent.output.result.membership.employment_status == "terminated" and .result.structuredContent.output.result.membership.terminated_at != null and .result.structuredContent.output.result.revoked_key_count == 1' >/dev/null <<<"$terminated"

new_key_response="$(mcp_post "$new_engineer_key" \
  '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"agent.bootstrap","arguments":{}}}')"
jq -e '.result.isError == true and .result.structuredContent.code == "unauthorized"' \
  >/dev/null <<<"$new_key_response"

actions="$(curl -fsS -H "authorization: Bearer $human_token" "$API_BASE_URL/api/v1/companies/$company_id/staffing-actions")"
jq -e '[.staffing_actions[].action_type] | index("permission_update") != null and index("hire") != null and index("activate") != null and index("suspend") != null and index("reactivate") != null and index("terminate") != null' >/dev/null <<<"$actions"
jq -e '[.staffing_actions[] | select(.action_type == "hire")] | length == 1' >/dev/null <<<"$actions"

echo "Company Staffing smoke completed successfully."
