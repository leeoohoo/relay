#!/usr/bin/env bash
set -euo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
MCP_PROTOCOL_VERSION="${MCP_PROTOCOL_VERSION:-2025-11-25}"
ERROR_FILE="/tmp/ai_chat_governance_error_$$.json"

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
    email:("governance-smoke-"+$suffix+"@example.com"),
    display_name:"Governance Smoke Owner",
    password:"password-1234"
  }'
)")"
owner_token="$(jq -r '.session_token' <<<"$owner")"

company="$(post_json /api/v1/companies "$(
  jq -cn --arg suffix "$suffix" '{
    name:("Governance Smoke "+$suffix),
    slug:("governance-smoke-"+$suffix)
  }'
)" -H "authorization: Bearer $owner_token")"
company_id="$(jq -r '.company_console.company.id' <<<"$company")"
root_org_unit_id="$(jq -r '.company_console.org_units[0].id' <<<"$company")"

default_policy="$(curl -fsS \
  -H "authorization: Bearer $owner_token" \
  "$API_BASE_URL/api/v1/companies/$company_id/governance-policy")"
jq -e '
  .governance_policy.configured == false and
  .governance_policy.active_version == null and
  (.governance_policy.versions | length) == 0 and
  .governance_policy.effective_settings.agent_staff_limit == 100 and
  .governance_policy.effective_settings.delegated_agent_hiring_enabled == true and
  .governance_policy.effective_settings.max_active_projects == 1000 and
  .governance_policy.effective_settings.max_project_members == 50 and
  .governance_policy.effective_settings.daily_delegated_hire_limit == 20 and
  .governance_policy.effective_settings.daily_delegated_suspension_limit == 50 and
  .governance_policy.effective_settings.daily_delegated_termination_limit == 20
' >/dev/null <<<"$default_policy"

manager="$(post_json "/api/v1/companies/$company_id/agents" "$(
  jq -cn --arg suffix "$suffix" --arg org "$root_org_unit_id" '{
    display_name:("Governance Manager "+$suffix),
    handle:("governance-manager-"+$suffix),
    persona:"负责执行治理策略",
    org_unit_id:$org,
    profession_key:"project_manager",
    role_key:"company_manager"
  }'
)" -H "authorization: Bearer $owner_token")"
manager_id="$(jq -r '.result.agent_profile.id' <<<"$manager")"
manager_key="$(jq -r '.result.agent_key_plaintext' <<<"$manager")"
manager_membership_id="$(jq -r '.result.membership.id' <<<"$manager")"

worker="$(post_json "/api/v1/companies/$company_id/agents" "$(
  jq -cn \
    --arg suffix "$suffix" \
    --arg org "$root_org_unit_id" \
    --arg manager "$manager_membership_id" '{
      display_name:("Governance Worker "+$suffix),
      handle:("governance-worker-"+$suffix),
      persona:"参与公司项目",
      org_unit_id:$org,
      profession_key:"software_engineer",
      role_key:"member",
      reports_to_membership_id:$manager
    }'
)" -H "authorization: Bearer $owner_token")"
worker_id="$(jq -r '.result.agent_profile.id' <<<"$worker")"

post_json "/api/v1/companies/$company_id/agents/$manager_id/permissions" \
  '{"staffing_permissions":["agent.staff.hire","agent.staff.suspend","agent.staff.terminate"],"reason":"Governance smoke"}' \
  -H "authorization: Bearer $owner_token" >/dev/null

v1="$(post_json "/api/v1/companies/$company_id/governance-policy" '{
  "agent_staff_limit":2,
  "delegated_agent_hiring_enabled":false,
  "delegated_agent_suspension_enabled":false,
  "delegated_agent_termination_enabled":false,
  "max_active_projects":1,
  "max_project_members":1,
  "notes":"v1 closes delegated staffing"
}' -H "authorization: Bearer $owner_token")"
jq -e '
  .governance_policy.configured == true and
  .governance_policy.active_version.version == 1 and
  .governance_policy.active_version.status == "active" and
  (.governance_policy.versions | length) == 1
' >/dev/null <<<"$v1"

over_limit_status="$(curl -sS -o "$ERROR_FILE" -w '%{http_code}' \
  -H 'content-type: application/json' \
  -H "authorization: Bearer $owner_token" \
  -d "$(jq -cn --arg suffix "$suffix" --arg org "$root_org_unit_id" --arg manager "$manager_membership_id" '{
    display_name:("Over Limit Worker "+$suffix),
    handle:("over-limit-worker-"+$suffix),
    persona:"must be rejected",
    org_unit_id:$org,
    profession_key:"software_engineer",
    role_key:"member",
    reports_to_membership_id:$manager
  }')" \
  "$API_BASE_URL/api/v1/companies/$company_id/agents")"
[[ "$over_limit_status" == "429" ]]
jq -e '.code == "rate_limited"' >/dev/null <"$ERROR_FILE"

disabled_hire="$(mcp_post "$manager_key" "$(
  jq -cn \
    --arg company "$company_id" \
    --arg suffix "$suffix" \
    --arg org "$root_org_unit_id" \
    --arg manager "$manager_membership_id" '{
      jsonrpc:"2.0",
      id:1,
      method:"tools/call",
      params:{
        name:"company.staff",arguments:{action:"hire",
          company_id:$company,
          display_name:("Disabled Hire "+$suffix),
          handle:("disabled-hire-"+$suffix),
          persona:"must be rejected",
          org_unit_id:$org,
          reports_to_membership_id:$manager,
          idempotency_key:("governance-disabled-hire-"+$suffix)
        }
      }
    }'
)")"
jq -e '.result.isError == true and .result.structuredContent.code == "unauthorized"' >/dev/null <<<"$disabled_hire"

disabled_suspend="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg worker "$worker_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",
    id:2,
    method:"tools/call",
    params:{
      name:"company.staff",arguments:{action:"suspend",
        company_id:$company,
        target_agent_id:$worker,
        idempotency_key:("governance-disabled-suspend-"+$suffix)
      }
    }
  }'
)")"
jq -e '.result.isError == true and .result.structuredContent.code == "unauthorized"' >/dev/null <<<"$disabled_suspend"

oversized_project="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg worker "$worker_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",
    id:3,
    method:"tools/call",
    params:{
      name:"company.project",arguments:{action:"create",
        company_id:$company,
        name:("Oversized Governance Project "+$suffix),
        member_agent_ids:[$worker],
        idempotency_key:("governance-oversized-project-"+$suffix)
      }
    }
  }'
)")"
jq -e '.result.isError == true and .result.structuredContent.code == "validation_error"' >/dev/null <<<"$oversized_project"

first_project="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",
    id:4,
    method:"tools/call",
    params:{
      name:"company.project",arguments:{action:"create",
        company_id:$company,
        name:("First Governance Project "+$suffix),
        member_agent_ids:[],
        idempotency_key:("governance-first-project-"+$suffix)
      }
    }
  }'
)")"
first_project_id="$(jq -r '.result.structuredContent.output.project.project.id' <<<"$first_project")"

blocked_second_project="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",
    id:5,
    method:"tools/call",
    params:{
      name:"company.project",arguments:{action:"create",
        company_id:$company,
        name:("Blocked Second Project "+$suffix),
        member_agent_ids:[],
        idempotency_key:("governance-blocked-second-project-"+$suffix)
      }
    }
  }'
)")"
jq -e '.result.isError == true and .result.structuredContent.code == "rate_limited"' >/dev/null <<<"$blocked_second_project"

v2="$(post_json "/api/v1/companies/$company_id/governance-policy" '{
  "agent_staff_limit":3,
  "delegated_agent_hiring_enabled":true,
  "delegated_agent_suspension_enabled":true,
  "delegated_agent_termination_enabled":true,
  "max_active_projects":2,
  "max_project_members":2,
  "notes":"v2 opens delegated staffing"
}' -H "authorization: Bearer $owner_token")"
jq -e '
  .governance_policy.active_version.version == 2 and
  .governance_policy.active_version.status == "active" and
  (.governance_policy.versions | length) == 2 and
  .governance_policy.versions[0].version == 2 and
  .governance_policy.versions[0].status == "active" and
  .governance_policy.versions[1].version == 1 and
  .governance_policy.versions[1].status == "archived"
' >/dev/null <<<"$v2"

expanded_project="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg project "$first_project_id" --arg worker "$worker_id" '{
    jsonrpc:"2.0",
    id:6,
    method:"tools/call",
    params:{
      name:"company.project",arguments:{action:"member_add",company_id:$company,project_id:$project,target_agent_id:$worker}
    }
  }'
)")"
jq -e '.result.structuredContent.output.project.members | length == 2' >/dev/null <<<"$expanded_project"

second_project="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",
    id:7,
    method:"tools/call",
    params:{
      name:"company.project",arguments:{action:"create",
        company_id:$company,
        name:("Second Governance Project "+$suffix),
        member_agent_ids:[],
        idempotency_key:("governance-second-project-"+$suffix)
      }
    }
  }'
)")"
jq -e '.result.structuredContent.output.project.project.status == "active"' >/dev/null <<<"$second_project"

delegated_hire="$(mcp_post "$manager_key" "$(
  jq -cn \
    --arg company "$company_id" \
    --arg suffix "$suffix" \
    --arg org "$root_org_unit_id" \
    --arg manager "$manager_membership_id" '{
      jsonrpc:"2.0",
      id:8,
      method:"tools/call",
      params:{
        name:"company.staff",arguments:{action:"hire",
          company_id:$company,
          display_name:("Delegated Hire "+$suffix),
          handle:("delegated-hire-"+$suffix),
          persona:"hired under v2",
          org_unit_id:$org,
          reports_to_membership_id:$manager,
          idempotency_key:("governance-v2-hire-"+$suffix)
        }
      }
    }'
)")"
jq -e '.result.structuredContent.output.result.membership.employment_status == "provisioning"' >/dev/null <<<"$delegated_hire"

suspended="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg worker "$worker_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",
    id:9,
    method:"tools/call",
    params:{
      name:"company.staff",arguments:{action:"suspend",
        company_id:$company,
        target_agent_id:$worker,
        reason:"v2 suspension",
        idempotency_key:("governance-v2-suspend-"+$suffix)
      }
    }
  }'
)")"
jq -e '.result.structuredContent.output.result.membership.employment_status == "suspended"' >/dev/null <<<"$suspended"

post_json "/api/v1/companies/$company_id/agents/$worker_id/reactivate" \
  '{"reason":"continue v2 termination smoke"}' \
  -H "authorization: Bearer $owner_token" >/dev/null

terminated="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg worker "$worker_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",
    id:10,
    method:"tools/call",
    params:{
      name:"company.staff",arguments:{action:"terminate",
        company_id:$company,
        target_agent_id:$worker,
        reason:"v2 termination",
        handoff_plan:"return work to manager",
        idempotency_key:("governance-v2-terminate-"+$suffix)
      }
    }
  }'
)")"
jq -e '.result.structuredContent.output.result.membership.employment_status == "terminated"' >/dev/null <<<"$terminated"

console="$(curl -fsS \
  -H "authorization: Bearer $owner_token" \
  "$API_BASE_URL/api/v1/companies/$company_id/console")"
jq -e '
  .company_console.governance_policy.active_version.version == 2 and
  (.company_console.governance_policy.versions | length) == 2 and
  .company_console.governance_policy.effective_settings.max_project_members == 2
' >/dev/null <<<"$console"

events="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" '{
    jsonrpc:"2.0",
    id:11,
    method:"tools/call",
    params:{
      name:"company.events",
      arguments:{company_id:$company,after_sequence_id:0,limit:200}
    }
  }'
)")"
jq -e '[.result.structuredContent.output.events[].event_type | select(. == "company.governance_policy.published")] | length == 2' >/dev/null <<<"$events"
jq -e '[.result.structuredContent.output.events[].event_type | select(. == "company.governance_policy.archived")] | length == 1' >/dev/null <<<"$events"

echo "Company governance policy version smoke completed successfully."
