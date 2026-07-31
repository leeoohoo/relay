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
owner="$(post_json /api/v1/auth/register "$(jq -cn --arg suffix "$suffix" '{email:("project-owner-"+$suffix+"@example.com"),display_name:"Project Owner",password:"password-1234"}')")"
owner_token="$(jq -r '.session_token' <<<"$owner")"
outsider_owner="$(post_json /api/v1/auth/register "$(jq -cn --arg suffix "$suffix" '{email:("project-outsider-"+$suffix+"@example.com"),display_name:"Project Outsider",password:"password-1234"}')")"
outsider_token="$(jq -r '.session_token' <<<"$outsider_owner")"

company="$(post_json /api/v1/companies \
  "$(jq -cn --arg suffix "$suffix" '{name:("Project Company "+$suffix),slug:("project-company-"+$suffix)}')" \
  -H "authorization: Bearer $owner_token")"
company_id="$(jq -r '.company_console.company.id' <<<"$company")"
root_org_unit_id="$(jq -r '.company_console.org_units[0].id' <<<"$company")"

outside_company="$(post_json /api/v1/companies \
  "$(jq -cn --arg suffix "$suffix" '{name:("Outside Project Company "+$suffix),slug:("outside-project-company-"+$suffix)}')" \
  -H "authorization: Bearer $outsider_token")"
outside_company_id="$(jq -r '.company_console.company.id' <<<"$outside_company")"
outside_root_id="$(jq -r '.company_console.org_units[0].id' <<<"$outside_company")"

manager="$(post_json "/api/v1/companies/$company_id/agents" \
  "$(jq -cn --arg suffix "$suffix" --arg org "$root_org_unit_id" '{display_name:("Project Manager "+$suffix),handle:("project-manager-"+$suffix),persona:"负责项目拆解和协调",org_unit_id:$org,profession_key:"project_manager",role_key:"company_manager"}')" \
  -H "authorization: Bearer $owner_token")"
manager_id="$(jq -r '.result.agent_profile.id' <<<"$manager")"
manager_key="$(jq -r '.result.agent_key_plaintext' <<<"$manager")"
manager_membership_id="$(jq -r '.result.membership.id' <<<"$manager")"

engineer="$(post_json "/api/v1/companies/$company_id/agents" \
  "$(jq -cn --arg suffix "$suffix" --arg org "$root_org_unit_id" --arg manager "$manager_membership_id" '{display_name:("Project Engineer "+$suffix),handle:("project-engineer-"+$suffix),persona:"负责项目研发",org_unit_id:$org,profession_key:"software_engineer",role_key:"member",reports_to_membership_id:$manager}')" \
  -H "authorization: Bearer $owner_token")"
engineer_id="$(jq -r '.result.agent_profile.id' <<<"$engineer")"
engineer_key="$(jq -r '.result.agent_key_plaintext' <<<"$engineer")"

outsider="$(post_json "/api/v1/companies/$outside_company_id/agents" \
  "$(jq -cn --arg suffix "$suffix" --arg org "$outside_root_id" '{display_name:("Outside Engineer "+$suffix),handle:("outside-project-engineer-"+$suffix),persona:"其他公司 Agent",org_unit_id:$org,profession_key:"project_manager",role_key:"company_manager"}')" \
  -H "authorization: Bearer $outsider_token")"
outsider_id="$(jq -r '.result.agent_profile.id' <<<"$outsider")"

tools="$(mcp_post "$manager_key" '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}')"
jq -e '.result.tools | length == 8' >/dev/null <<<"$tools"
jq -e '[.result.tools[].name] | index("company.project") != null and index("company.task") != null' >/dev/null <<<"$tools"

project="$(mcp_post "$manager_key" "$(jq -cn --arg company "$company_id" --arg engineer "$engineer_id" '{jsonrpc:"2.0",id:2,method:"tools/call",params:{name:"company.project",arguments:{action:"create",company_id:$company,name:"Agent 项目协作闭环",description:"验证正式项目、任务、状态和项目群",member_agent_ids:[$engineer],idempotency_key:"project-create-v1"}}}')")"
project_id="$(jq -r '.result.structuredContent.output.project.project.id' <<<"$project")"
project_group_id="$(jq -r '.result.structuredContent.output.project.project.project_group_conversation_id' <<<"$project")"
jq -e '.result.structuredContent.output.project.members | length == 2' >/dev/null <<<"$project"
jq -e '.result.structuredContent.output.project.project_group.context.context_type == "project_group"' >/dev/null <<<"$project"

task="$(mcp_post "$manager_key" "$(jq -cn --arg company "$company_id" --arg project "$project_id" --arg engineer "$engineer_id" '{jsonrpc:"2.0",id:3,method:"tools/call",params:{name:"company.task",arguments:{action:"create",company_id:$company,project_id:$project,title:"实现 Project API",description:"完成项目任务和状态闭环",priority:"high",assignee_agent_id:$engineer,idempotency_key:"project-task-v1"}}}')")"
task_id="$(jq -r '.result.structuredContent.output.task.id' <<<"$task")"
jq -e '.result.structuredContent.output.task.status == "todo"' >/dev/null <<<"$task"

task_update="$(mcp_post "$engineer_key" "$(jq -cn --arg company "$company_id" --arg project "$project_id" --arg task "$task_id" '{jsonrpc:"2.0",id:4,method:"tools/call",params:{name:"company.task",arguments:{action:"update",company_id:$company,project_id:$project,task_id:$task,status:"in_progress",idempotency_key:"project-task-progress-v1"}}}')")"
jq -e '.result.structuredContent.output.task.status == "in_progress"' >/dev/null <<<"$task_update"

status_update="$(mcp_post "$engineer_key" "$(jq -cn --arg company "$company_id" --arg project "$project_id" '{jsonrpc:"2.0",id:5,method:"tools/call",params:{name:"company.project",arguments:{action:"status_update",company_id:$company,project_id:$project,summary:"Project API 已进入开发阶段",progress_percent:40,blockers:[],next_steps:["完成 PostgreSQL 验收"],idempotency_key:"project-status-40-v1"}}}')")"
jq -e '.result.structuredContent.output.status_update.progress_percent == 40' >/dev/null <<<"$status_update"

group_message="$(mcp_post "$engineer_key" "$(jq -cn --arg company "$company_id" --arg conversation "$project_group_id" '{jsonrpc:"2.0",id:6,method:"tools/call",params:{name:"company.chat",arguments:{action:"send",company_id:$company,conversation_id:$conversation,content:"当前项目进度 40%，暂无阻塞",idempotency_key:"project-group-message-v1"}}}')")"
jq -e --arg conversation "$project_group_id" '.result.structuredContent.output.message.conversation_id == $conversation' >/dev/null <<<"$group_message"

cross_company="$(mcp_post "$manager_key" "$(jq -cn --arg company "$company_id" --arg project "$project_id" --arg outsider "$outsider_id" '{jsonrpc:"2.0",id:7,method:"tools/call",params:{name:"company.project",arguments:{action:"member_add",company_id:$company,project_id:$project,target_agent_id:$outsider}}}')")"
jq -e '.result.isError == true and .result.structuredContent.code == "unauthorized"' >/dev/null <<<"$cross_company"

complete="$(mcp_post "$manager_key" "$(jq -cn --arg company "$company_id" --arg project "$project_id" '{jsonrpc:"2.0",id:8,method:"tools/call",params:{name:"company.project",arguments:{action:"status_update",company_id:$company,project_id:$project,summary:"项目闭环验收完成",progress_percent:100,blockers:[],next_steps:[],project_status:"completed",idempotency_key:"project-status-100-v1"}}}')")"
jq -e '.result.structuredContent.output.status_update.project_status == "completed"' >/dev/null <<<"$complete"

remove="$(mcp_post "$manager_key" "$(jq -cn --arg company "$company_id" --arg project "$project_id" --arg engineer "$engineer_id" '{jsonrpc:"2.0",id:9,method:"tools/call",params:{name:"company.project",arguments:{action:"member_remove",company_id:$company,project_id:$project,target_agent_id:$engineer,idempotency_key:"project-member-remove-v1"}}}')")"
jq -e '.result.structuredContent.output.project.members | length == 1' >/dev/null <<<"$remove"

removed_message="$(mcp_post "$engineer_key" "$(jq -cn --arg company "$company_id" --arg conversation "$project_group_id" '{jsonrpc:"2.0",id:10,method:"tools/call",params:{name:"company.chat",arguments:{action:"send",company_id:$company,conversation_id:$conversation,content:"不应发送成功"}}}')")"
jq -e '.result.isError == true and .result.structuredContent.code == "unauthorized"' >/dev/null <<<"$removed_message"

console="$(curl -fsS -H "authorization: Bearer $owner_token" "$API_BASE_URL/api/v1/companies/$company_id/console")"
jq -e '.company_console.projects | length == 1' >/dev/null <<<"$console"
jq -e '.company_console.projects[0].project.status == "completed"' >/dev/null <<<"$console"
jq -e '.company_console.projects[0].tasks[0].status == "in_progress"' >/dev/null <<<"$console"
jq -e '[.company_console.conversations[].context.context_type] | index("project_group") != null' >/dev/null <<<"$console"

echo "Company project smoke completed successfully."
