#!/usr/bin/env bash
set -euo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@127.0.0.1:5432/ai_chat}"
MCP_PROTOCOL_VERSION="${MCP_PROTOCOL_VERSION:-2025-11-25}"

for bin in curl jq; do
  command -v "$bin" >/dev/null 2>&1 || {
    echo "Missing required command: $bin" >&2
    exit 1
  }
done

PG_CONTAINER="${PG_CONTAINER:-ai-chat-postgres}"
if command -v psql >/dev/null 2>&1; then
  run_psql() {
    psql "$DATABASE_URL" "$@"
  }
elif command -v docker >/dev/null 2>&1 && docker inspect "$PG_CONTAINER" >/dev/null 2>&1; then
  run_psql() {
    docker exec "$PG_CONTAINER" psql "$DATABASE_URL" "$@"
  }
else
  echo "Missing PostgreSQL client: install psql or provide a running PG_CONTAINER" >&2
  exit 1
fi

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
    email:("staffing-governance-smoke-"+$suffix+"@example.com"),
    display_name:"Staffing Governance Smoke Owner",
    password:"password-1234"
  }'
)")"
owner_token="$(jq -r '.session_token' <<<"$owner")"

company="$(post_json /api/v1/companies "$(
  jq -cn --arg suffix "$suffix" '{
    name:("Staffing Governance Smoke "+$suffix),
    slug:("staffing-governance-smoke-"+$suffix)
  }'
)" -H "authorization: Bearer $owner_token")"
company_id="$(jq -r '.company_console.company.id' <<<"$company")"

engineering="$(post_json "/api/v1/companies/$company_id/org-units" \
  '{"name":"Engineering","unit_type":"department"}' \
  -H "authorization: Bearer $owner_token")"
engineering_id="$(jq -r '.org_unit.id' <<<"$engineering")"
platform_team="$(post_json "/api/v1/companies/$company_id/org-units" "$(
  jq -cn --arg parent "$engineering_id" '{
    name:"Platform Team",
    unit_type:"team",
    parent_org_unit_id:$parent
  }'
)" -H "authorization: Bearer $owner_token")"
platform_team_id="$(jq -r '.org_unit.id' <<<"$platform_team")"
sales="$(post_json "/api/v1/companies/$company_id/org-units" \
  '{"name":"Sales","unit_type":"department"}' \
  -H "authorization: Bearer $owner_token")"
sales_id="$(jq -r '.org_unit.id' <<<"$sales")"

create_agent() {
  local display_name="$1"
  local handle="$2"
  local org_unit_id="$3"
  local role_key="$4"
  local reports_to_membership_id="${5:-}"
  post_json "/api/v1/companies/$company_id/agents" "$(
    jq -cn \
      --arg display_name "$display_name" \
      --arg handle "$handle" \
      --arg org "$org_unit_id" \
      --arg role "$role_key" \
      --arg reports_to "$reports_to_membership_id" '{
        display_name:$display_name,
        handle:$handle,
        persona:"Staffing governance smoke Agent",
        org_unit_id:$org,
        profession_key:(if $role == "company_manager" then "project_manager" else "software_engineer" end),
        role_key:$role,
        reports_to_membership_id:(if $reports_to == "" then null else $reports_to end)
      }'
  )" -H "authorization: Bearer $owner_token"
}

manager="$(create_agent "Engineering Manager $suffix" "staffing-governance-manager-$suffix" "$engineering_id" company_manager)"
manager_id="$(jq -r '.result.agent_profile.id' <<<"$manager")"
manager_key="$(jq -r '.result.agent_key_plaintext' <<<"$manager")"
manager_membership_id="$(jq -r '.result.membership.id' <<<"$manager")"
target="$(create_agent "Platform Target $suffix" "staffing-governance-target-$suffix" "$platform_team_id" member "$manager_membership_id")"
target_id="$(jq -r '.result.agent_profile.id' <<<"$target")"
handoff="$(create_agent "Platform Handoff $suffix" "staffing-governance-handoff-$suffix" "$platform_team_id" member "$manager_membership_id")"
handoff_id="$(jq -r '.result.agent_profile.id' <<<"$handoff")"
nonmember="$(create_agent "Platform Nonmember $suffix" "staffing-governance-nonmember-$suffix" "$platform_team_id" member "$manager_membership_id")"
nonmember_id="$(jq -r '.result.agent_profile.id' <<<"$nonmember")"
sales_agent="$(create_agent "Sales Agent $suffix" "staffing-governance-sales-$suffix" "$sales_id" member "$manager_membership_id")"
sales_agent_id="$(jq -r '.result.agent_profile.id' <<<"$sales_agent")"

permissions="$(post_json "/api/v1/companies/$company_id/agents/$manager_id/permissions" "$(
  jq -cn --arg scope "$engineering_id" '{
    staffing_permissions:["agent.staff.hire","agent.staff.suspend","agent.staff.terminate"],
    staffing_scope_org_unit_id:$scope,
    reason:"Limit Staffing to the Engineering subtree"
  }'
)" -H "authorization: Bearer $owner_token")"
jq -e --arg scope "$engineering_id" \
  '.membership.staffing_scope_org_unit_id == $scope' >/dev/null <<<"$permissions"

stored_scope="$(run_psql -At \
  -c "SELECT staffing_scope_org_unit_id FROM company_agent_memberships WHERE id = '$manager_membership_id'::uuid")"
[[ "$stored_scope" == "$engineering_id" ]]

foreign_company="$(post_json /api/v1/companies "$(
  jq -cn --arg suffix "$suffix" '{
    name:("Foreign Staffing Scope "+$suffix),
    slug:("foreign-staffing-scope-"+$suffix)
  }'
)" -H "authorization: Bearer $owner_token")"
foreign_root_id="$(jq -r '.company_console.org_units[0].id' <<<"$foreign_company")"
set +e
foreign_scope_error="$(run_psql -v ON_ERROR_STOP=1 \
  -c "UPDATE company_agent_memberships SET staffing_scope_org_unit_id = '$foreign_root_id'::uuid WHERE id = '$manager_membership_id'::uuid" 2>&1)"
foreign_scope_status=$?
set -e
[[ "$foreign_scope_status" -ne 0 ]]
grep -q 'fk_company_agent_staffing_scope' <<<"$foreign_scope_error"

post_json "/api/v1/companies/$company_id/governance-policy" '{
  "agent_staff_limit":20,
  "delegated_agent_hiring_enabled":true,
  "delegated_agent_suspension_enabled":true,
  "delegated_agent_termination_enabled":true,
  "max_active_projects":100,
  "max_project_members":20,
  "daily_delegated_hire_limit":1,
  "daily_delegated_suspension_limit":1,
  "daily_delegated_termination_limit":1,
  "notes":"Staffing scope, quota, and handoff smoke"
}' -H "authorization: Bearer $owner_token" >/dev/null

outside_hire="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg org "$sales_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",id:1,method:"tools/call",params:{
      name:"company.staff",arguments:{action:"hire",
        company_id:$company,
        display_name:("Out Of Scope Hire "+$suffix),
        handle:("staffing-outside-hire-"+$suffix),
        persona:"must be rejected",
        org_unit_id:$org,
        idempotency_key:("staffing-outside-hire-"+$suffix)
      }
    }
  }'
)")"
jq -e '.result.isError == true and .result.structuredContent.code == "unauthorized"' >/dev/null <<<"$outside_hire"

outside_suspend="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg target "$sales_agent_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",id:2,method:"tools/call",params:{
      name:"company.staff",arguments:{action:"suspend",company_id:$company,target_agent_id:$target,idempotency_key:("staffing-outside-suspend-"+$suffix)}
    }
  }'
)")"
jq -e '.result.isError == true and .result.structuredContent.code == "unauthorized"' >/dev/null <<<"$outside_suspend"

first_hire="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg org "$platform_team_id" --arg manager "$manager_membership_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",id:3,method:"tools/call",params:{
      name:"company.staff",arguments:{action:"hire",
        company_id:$company,
        display_name:("Scoped Hire "+$suffix),
        handle:("staffing-scoped-hire-"+$suffix),
        persona:"allowed descendant hire",
        org_unit_id:$org,
        reports_to_membership_id:$manager,
        idempotency_key:("staffing-scoped-hire-"+$suffix)
      }
    }
  }'
)")"
jq -e --arg org "$platform_team_id" \
  '.result.structuredContent.output.result.membership.org_unit_id == $org' >/dev/null <<<"$first_hire"

second_hire="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg org "$platform_team_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",id:4,method:"tools/call",params:{
      name:"company.staff",arguments:{action:"hire",
        company_id:$company,
        display_name:("Quota Hire "+$suffix),
        handle:("staffing-quota-hire-"+$suffix),
        persona:"must hit daily quota",
        org_unit_id:$org,
        idempotency_key:("staffing-quota-hire-"+$suffix)
      }
    }
  }'
)")"
jq -e '.result.isError == true and .result.structuredContent.code == "rate_limited"' >/dev/null <<<"$second_hire"

suspended="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg target "$target_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",id:5,method:"tools/call",params:{
      name:"company.staff",arguments:{action:"suspend",company_id:$company,target_agent_id:$target,idempotency_key:("staffing-suspend-one-"+$suffix)}
    }
  }'
)")"
jq -e '.result.structuredContent.output.result.membership.employment_status == "suspended"' >/dev/null <<<"$suspended"

quota_suspend="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg target "$handoff_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",id:6,method:"tools/call",params:{
      name:"company.staff",arguments:{action:"suspend",company_id:$company,target_agent_id:$target,idempotency_key:("staffing-suspend-two-"+$suffix)}
    }
  }'
)")"
jq -e '.result.isError == true and .result.structuredContent.code == "rate_limited"' >/dev/null <<<"$quota_suspend"

post_json "/api/v1/companies/$company_id/agents/$handoff_id/suspend" \
  '{"reason":"Human bypasses delegated suspension quota"}' \
  -H "authorization: Bearer $owner_token" >/dev/null
post_json "/api/v1/companies/$company_id/agents/$target_id/reactivate" \
  '{"reason":"Reactivate target for handoff test"}' \
  -H "authorization: Bearer $owner_token" >/dev/null
post_json "/api/v1/companies/$company_id/agents/$handoff_id/reactivate" \
  '{"reason":"Reactivate handoff Agent"}' \
  -H "authorization: Bearer $owner_token" >/dev/null

project="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg target "$target_id" --arg handoff "$handoff_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",id:7,method:"tools/call",params:{
      name:"company.project",arguments:{action:"create",
        company_id:$company,
        name:("Staffing Handoff Project "+$suffix),
        member_agent_ids:[$target,$handoff],
        idempotency_key:("staffing-handoff-project-"+$suffix)
      }
    }
  }'
)")"
project_id="$(jq -r '.result.structuredContent.output.project.project.id' <<<"$project")"

task="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg project "$project_id" --arg target "$target_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",id:8,method:"tools/call",params:{
      name:"company.task",arguments:{action:"create",
        company_id:$company,
        project_id:$project,
        title:("Staffing handoff task "+$suffix),
        assignee_agent_id:$target,
        idempotency_key:("staffing-handoff-task-"+$suffix)
      }
    }
  }'
)")"
task_id="$(jq -r '.result.structuredContent.output.task.id' <<<"$task")"

missing_handoff="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg target "$target_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",id:9,method:"tools/call",params:{
      name:"company.staff",arguments:{action:"terminate",
        company_id:$company,
        target_agent_id:$target,
        handoff_plan:"handoff is required",
        idempotency_key:("staffing-missing-handoff-"+$suffix)
      }
    }
  }'
)")"
jq -e '.result.isError == true and .result.structuredContent.code == "validation_error"' >/dev/null <<<"$missing_handoff"

invalid_handoff="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg target "$target_id" --arg handoff "$nonmember_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",id:10,method:"tools/call",params:{
      name:"company.staff",arguments:{action:"terminate",
        company_id:$company,
        target_agent_id:$target,
        handoff_plan:"handoff to a nonmember must fail",
        handoff_agent_id:$handoff,
        idempotency_key:("staffing-invalid-handoff-"+$suffix)
      }
    }
  }'
)")"
jq -e '.result.isError == true and .result.structuredContent.code == "validation_error"' >/dev/null <<<"$invalid_handoff"

terminated="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg target "$target_id" --arg handoff "$handoff_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",id:11,method:"tools/call",params:{
      name:"company.staff",arguments:{action:"terminate",
        company_id:$company,
        target_agent_id:$target,
        reason:"atomic task handoff",
        handoff_plan:"move all open work to the selected Agent",
        handoff_agent_id:$handoff,
        idempotency_key:("staffing-valid-handoff-"+$suffix)
      }
    }
  }'
)")"
jq -e --arg task "$task_id" '
  .result.structuredContent.output.result.membership.employment_status == "terminated" and
  .result.structuredContent.output.result.reassigned_task_count == 1 and
  .result.structuredContent.output.result.action.result_payload.reassigned_task_ids == [$task]
' >/dev/null <<<"$terminated"

quota_termination="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg target "$handoff_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",id:12,method:"tools/call",params:{
      name:"company.staff",arguments:{action:"terminate",
        company_id:$company,
        target_agent_id:$target,
        handoff_plan:"must hit daily quota",
        idempotency_key:("staffing-terminate-two-"+$suffix)
      }
    }
  }'
)")"
jq -e '.result.isError == true and .result.structuredContent.code == "rate_limited"' >/dev/null <<<"$quota_termination"

post_json "/api/v1/companies/$company_id/agents/$nonmember_id/terminate" \
  '{"reason":"Human bypasses delegated termination quota","handoff_plan":"No open tasks"}' \
  -H "authorization: Bearer $owner_token" >/dev/null

project_after="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg project "$project_id" '{
    jsonrpc:"2.0",id:13,method:"tools/call",params:{
      name:"company.project",arguments:{action:"get",company_id:$company,project_id:$project}
    }
  }'
)")"
jq -e --arg task "$task_id" --arg handoff "$handoff_id" '
  .result.structuredContent.output.project.tasks[] |
  select(.id == $task) |
  .assignee_agent_id == $handoff
' >/dev/null <<<"$project_after"

events="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" '{
    jsonrpc:"2.0",id:14,method:"tools/call",params:{
      name:"company.events",
      arguments:{company_id:$company,after_sequence_id:0,limit:500}
    }
  }'
)")"
jq -e --arg task "$task_id" --arg handoff "$handoff_id" '
  [.result.structuredContent.output.events[] |
    select(.event_type == "project.task.updated" and .aggregate_id == $task and .payload.assignee_agent_id == $handoff)
  ] | length == 1
' >/dev/null <<<"$events"

actions="$(curl -fsS \
  -H "authorization: Bearer $owner_token" \
  "$API_BASE_URL/api/v1/companies/$company_id/staffing-actions")"
jq -e --arg target "$target_id" --arg handoff "$handoff_id" --arg task "$task_id" '
  [.staffing_actions[] |
    select(.action_type == "terminate" and .target_agent_id == $target and .actor_type == "agent")
  ][0] |
  .request_payload.handoff_agent_id == $handoff and
  .result_payload.reassigned_task_count == 1 and
  .result_payload.reassigned_task_ids == [$task]
' >/dev/null <<<"$actions"

echo "Staffing governance controls smoke completed successfully."
