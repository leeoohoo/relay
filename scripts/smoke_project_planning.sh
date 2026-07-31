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
owner="$(post_json /api/v1/auth/register "$(
  jq -cn --arg suffix "$suffix" '{
    email:("planning-smoke-"+$suffix+"@example.com"),
    display_name:"Planning Smoke Owner",
    password:"password-1234"
  }'
)")"
owner_token="$(jq -r '.session_token' <<<"$owner")"

company="$(post_json /api/v1/companies "$(
  jq -cn --arg suffix "$suffix" '{
    name:("Planning Smoke "+$suffix),
    slug:("planning-smoke-"+$suffix)
  }'
)" -H "authorization: Bearer $owner_token")"
company_id="$(jq -r '.company_console.company.id' <<<"$company")"
root_org_unit_id="$(jq -r '.company_console.org_units[0].id' <<<"$company")"

manager="$(post_json "/api/v1/companies/$company_id/agents" "$(
  jq -cn --arg suffix "$suffix" --arg org "$root_org_unit_id" '{
    display_name:("Planning Manager "+$suffix),
    handle:("planning-manager-"+$suffix),
    persona:"负责项目计划、依赖和批量任务更新",
    org_unit_id:$org,
    profession_key:"project_manager",
    role_key:"company_manager"
  }'
)" -H "authorization: Bearer $owner_token")"
manager_key="$(jq -r '.result.agent_key_plaintext' <<<"$manager")"
manager_id="$(jq -r '.result.agent_profile.id' <<<"$manager")"
manager_membership_id="$(jq -r '.result.membership.id' <<<"$manager")"

engineer="$(post_json "/api/v1/companies/$company_id/agents" "$(
  jq -cn \
    --arg suffix "$suffix" \
    --arg org "$root_org_unit_id" \
    --arg manager "$manager_membership_id" '{
      display_name:("Planning Engineer "+$suffix),
      handle:("planning-engineer-"+$suffix),
      persona:"按依赖顺序完成任务",
      org_unit_id:$org,
      profession_key:"software_engineer",
      role_key:"member",
      reports_to_membership_id:$manager
    }'
)" -H "authorization: Bearer $owner_token")"
engineer_id="$(jq -r '.result.agent_profile.id' <<<"$engineer")"
engineer_key="$(jq -r '.result.agent_key_plaintext' <<<"$engineer")"

tools="$(mcp_post "$manager_key" '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}')"
jq -e '.result.tools | length == 8' >/dev/null <<<"$tools"
jq -e '[.result.tools[].name] |
  index("company.project") != null and
  index("company.task") != null
' >/dev/null <<<"$tools"

project="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg engineer "$engineer_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",
    id:2,
    method:"tools/call",
    params:{
      name:"company.project",arguments:{action:"create",
        company_id:$company,
        name:("Planning MVP "+$suffix),
        description:"initial metadata",
        member_agent_ids:[$engineer],
        idempotency_key:("planning-project-"+$suffix)
      }
    }
  }'
)")"
project_id="$(jq -r '.result.structuredContent.output.project.project.id' <<<"$project")"

updated_project="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg project "$project_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",
    id:3,
    method:"tools/call",
    params:{
      name:"company.project",arguments:{action:"update",
        company_id:$company,
        project_id:$project,
        name:("Planning Controls "+$suffix),
        description:"metadata, dependencies, and batch updates",
        due_at:"2026-12-31T12:00:00Z",
        idempotency_key:("planning-project-update-"+$suffix)
      }
    }
  }'
)")"
jq -e --arg manager "$manager_id" '
  .result.structuredContent.output.project.project.due_at == "2026-12-31T12:00:00Z" and
  .result.structuredContent.output.project.project.updated_by_agent_id == $manager and
  (.result.structuredContent.output.project.project_group.preview.title | startswith("项目 · Planning Controls"))
' >/dev/null <<<"$updated_project"

create_task() {
  local rpc_id="$1"
  local title="$2"
  local key="$3"
  mcp_post "$manager_key" "$(
    jq -cn \
      --argjson rpc_id "$rpc_id" \
      --arg company "$company_id" \
      --arg project "$project_id" \
      --arg engineer "$engineer_id" \
      --arg title "$title" \
      --arg key "$key" '{
        jsonrpc:"2.0",
        id:$rpc_id,
        method:"tools/call",
        params:{
          name:"company.task",arguments:{action:"create",
            company_id:$company,
            project_id:$project,
            title:$title,
            priority:"normal",
            assignee_agent_id:$engineer,
            idempotency_key:$key
          }
        }
      }'
  )"
}

foundation="$(create_task 4 "Foundation $suffix" "planning-foundation-$suffix")"
delivery="$(create_task 5 "Delivery $suffix" "planning-delivery-$suffix")"
launch="$(create_task 6 "Launch $suffix" "planning-launch-$suffix")"
foundation_id="$(jq -r '.result.structuredContent.output.task.id' <<<"$foundation")"
delivery_id="$(jq -r '.result.structuredContent.output.task.id' <<<"$delivery")"
launch_id="$(jq -r '.result.structuredContent.output.task.id' <<<"$launch")"

add_dependency() {
  local rpc_id="$1"
  local task_id="$2"
  local depends_on_task_id="$3"
  local key="$4"
  mcp_post "$manager_key" "$(
    jq -cn \
      --argjson rpc_id "$rpc_id" \
      --arg company "$company_id" \
      --arg project "$project_id" \
      --arg task "$task_id" \
      --arg depends "$depends_on_task_id" \
      --arg key "$key" '{
        jsonrpc:"2.0",
        id:$rpc_id,
        method:"tools/call",
        params:{
          name:"company.task",arguments:{action:"dependency_add",
            company_id:$company,
            project_id:$project,
            task_id:$task,
            depends_on_task_id:$depends,
            idempotency_key:$key
          }
        }
      }'
  )"
}

delivery_dependency="$(add_dependency 7 "$delivery_id" "$foundation_id" "planning-dependency-delivery-$suffix")"
launch_dependency="$(add_dependency 8 "$launch_id" "$delivery_id" "planning-dependency-launch-$suffix")"
jq -e '.result.structuredContent.output.dependency.id != null' >/dev/null <<<"$delivery_dependency"
jq -e '.result.structuredContent.output.dependency.id != null' >/dev/null <<<"$launch_dependency"

cycle="$(add_dependency 9 "$foundation_id" "$launch_id" "planning-cycle-$suffix")"
jq -e '.result.isError == true and .result.structuredContent.code == "validation_error"' >/dev/null <<<"$cycle"

blocked_delivery="$(mcp_post "$engineer_key" "$(
  jq -cn --arg company "$company_id" --arg project "$project_id" --arg task "$delivery_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",
    id:10,
    method:"tools/call",
    params:{
      name:"company.task",arguments:{action:"update",
        company_id:$company,
        project_id:$project,
        task_id:$task,
        status:"in_progress",
        idempotency_key:("planning-blocked-delivery-"+$suffix)
      }
    }
  }'
)")"
jq -e '.result.isError == true and .result.structuredContent.code == "conflict"' >/dev/null <<<"$blocked_delivery"

foundation_done="$(mcp_post "$engineer_key" "$(
  jq -cn --arg company "$company_id" --arg project "$project_id" --arg task "$foundation_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",
    id:11,
    method:"tools/call",
    params:{
      name:"company.task",arguments:{action:"update",
        company_id:$company,
        project_id:$project,
        task_id:$task,
        status:"done",
        idempotency_key:("planning-foundation-done-"+$suffix)
      }
    }
  }'
)")"
jq -e '.result.structuredContent.output.task.status == "done"' >/dev/null <<<"$foundation_done"

delivery_done="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg project "$project_id" --arg task "$delivery_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",
    id:12,
    method:"tools/call",
    params:{
      name:"company.task",arguments:{action:"batch_update",
        company_id:$company,
        project_id:$project,
        task_ids:[$task],
        status:"done",
        priority:"urgent",
        idempotency_key:("planning-delivery-batch-"+$suffix)
      }
    }
  }'
)")"
jq -e '.result.structuredContent.output.tasks[0].status == "done" and .result.structuredContent.output.tasks[0].priority == "urgent"' >/dev/null <<<"$delivery_done"

launch_started="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg project "$project_id" --arg task "$launch_id" --arg manager "$manager_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",
    id:13,
    method:"tools/call",
    params:{
      name:"company.task",arguments:{action:"batch_update",
        company_id:$company,
        project_id:$project,
        task_ids:[$task],
        status:"in_progress",
        priority:"high",
        assignee_agent_id:$manager,
        due_at:"2026-12-20T12:00:00Z",
        idempotency_key:("planning-launch-batch-"+$suffix)
      }
    }
  }'
)")"
jq -e --arg manager "$manager_id" '
  .result.structuredContent.output.tasks[0].status == "in_progress" and
  .result.structuredContent.output.tasks[0].priority == "high" and
  .result.structuredContent.output.tasks[0].assignee_agent_id == $manager and
  .result.structuredContent.output.tasks[0].updated_by_agent_id == $manager
' >/dev/null <<<"$launch_started"

invalid_task_id="00000000-0000-0000-0000-000000000001"
invalid_batch="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg project "$project_id" --arg launch "$launch_id" --arg invalid "$invalid_task_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",
    id:14,
    method:"tools/call",
    params:{
      name:"company.task",arguments:{action:"batch_update",
        company_id:$company,
        project_id:$project,
        task_ids:[$launch,$invalid],
        priority:"low",
        idempotency_key:("planning-invalid-batch-"+$suffix)
      }
    }
  }'
)")"
jq -e '.result.isError == true and .result.structuredContent.code == "not_found"' >/dev/null <<<"$invalid_batch"

removed_dependency="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg project "$project_id" --arg task "$launch_id" --arg depends "$delivery_id" --arg suffix "$suffix" '{
    jsonrpc:"2.0",
    id:15,
    method:"tools/call",
    params:{
      name:"company.task",arguments:{action:"dependency_remove",
        company_id:$company,
        project_id:$project,
        task_id:$task,
        depends_on_task_id:$depends,
        idempotency_key:("planning-remove-dependency-"+$suffix)
      }
    }
  }'
)")"
jq -e '.result.structuredContent.output.removed == true' >/dev/null <<<"$removed_dependency"

project_view="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" --arg project "$project_id" '{
    jsonrpc:"2.0",
    id:16,
    method:"tools/call",
    params:{name:"company.project",arguments:{action:"get",company_id:$company,project_id:$project}}
  }'
)")"
jq -e '
  (.result.structuredContent.output.project.tasks | length) == 3 and
  (.result.structuredContent.output.project.task_dependencies | length) == 1 and
  .result.structuredContent.output.project.project.due_at == "2026-12-31T12:00:00Z" and
  ([.result.structuredContent.output.project.tasks[] | select(.id == "'"$launch_id"'")][0].priority == "high")
' >/dev/null <<<"$project_view"

events="$(mcp_post "$manager_key" "$(
  jq -cn --arg company "$company_id" '{
    jsonrpc:"2.0",
    id:17,
    method:"tools/call",
    params:{name:"company.events",arguments:{company_id:$company,after_sequence_id:0,limit:200}}
  }'
)")"
jq -e '[.result.structuredContent.output.events[].event_type] |
  index("project.updated") != null and
  index("project.task.dependency.added") != null and
  index("project.task.dependency.removed") != null and
  index("project.task.updated") != null
' >/dev/null <<<"$events"

echo "Project planning metadata, dependencies, and batch update smoke completed successfully."
