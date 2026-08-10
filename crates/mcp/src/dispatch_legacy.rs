use super::handler::parse_input;
use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> McpGateway<R, V> {
    pub(super) fn execute_legacy_tool(
        &self,
        agent_id: Uuid,
        tool_name: &str,
        input: Value,
        idempotency_key: Option<String>,
    ) -> AppResult<Value> {
        match tool_name {
            "company.context.get" => {
                let input: CompanyContextToolInput = parse_input(input)?;
                let context =
                    self.platform
                        .get_company_agent_context(GetCompanyAgentContextInput {
                            actor_agent_id: agent_id,
                            company_id: input.company_id,
                        })?;
                Ok(json!({ "company_context": context }))
            }
            "company.events.list" => {
                let input: CompanyEventsListToolInput = parse_input(input)?;
                let events = self.platform.list_company_realtime_events_for_agent(
                    agent_id,
                    input.company_id,
                    input.after_sequence_id.unwrap_or(0),
                    input.limit.unwrap_or(100),
                )?;
                Ok(json!({ "events": events }))
            }
            "company.chat.direct.open" => {
                let input: CompanyDirectConversationToolInput = parse_input(input)?;
                let conversation = self.platform.open_company_direct_conversation(
                    OpenCompanyDirectConversationInput {
                        actor_agent_id: agent_id,
                        company_id: input.company_id,
                        target_agent_id: input.target_agent_id,
                    },
                )?;
                Ok(json!({ "conversation": conversation }))
            }
            "company.chat.group.create" => {
                let input: CompanyGroupConversationToolInput = parse_input(input)?;
                let conversation = self.platform.create_company_group_conversation(
                    CreateCompanyGroupConversationInput {
                        actor_agent_id: agent_id,
                        company_id: input.company_id,
                        title: input.title,
                        member_agent_ids: input.member_agent_ids,
                    },
                )?;
                Ok(json!({ "conversation": conversation }))
            }
            "company.chat.message.send" => {
                let input: CompanyMessageToolInput = parse_input(input)?;
                let message = self.platform.send_company_message_with_mentions(
                    SendCompanyMessageWithMentionsInput {
                        actor_agent_id: agent_id,
                        company_id: input.company_id,
                        conversation_id: input.conversation_id,
                        content: input.content,
                        mentioned_agent_ids: input.mentioned_agent_ids,
                        mention_all: input.mention_all,
                    },
                )?;
                Ok(json!({ "message": message }))
            }
            "company.chat.message.reply" => {
                let input: CompanyMessageReplyToolInput = parse_input(input)?;
                let result = self.platform.reply_to_company_inbox_message(
                    ReplyCompanyInboxMessageInput {
                        actor_agent_id: agent_id,
                        event_id: input.event_id,
                        content: input.content,
                        auto_ack: input.auto_ack.unwrap_or(true),
                    },
                )?;
                Ok(json!({ "message": result.message, "event": result.event }))
            }
            "company.chat.message.list" => {
                let input: CompanyMessageListToolInput = parse_input(input)?;
                let context =
                    self.platform
                        .get_company_agent_context(GetCompanyAgentContextInput {
                            actor_agent_id: agent_id,
                            company_id: input.company_id,
                        })?;
                if !context
                    .conversations
                    .iter()
                    .any(|conversation| conversation.preview.id == input.conversation_id)
                {
                    return Err(AppError::Unauthorized(
                        "agent is not a member of the requested company conversation".into(),
                    ));
                }
                let page = self.platform.get_agent_conversation_message_page(
                    agent_id,
                    input.conversation_id,
                    input.before_message_id,
                    input.limit.unwrap_or(50),
                )?;
                Ok(json!({
                    "messages": page.messages,
                    "next_cursor": page.next_cursor,
                    "has_more": page.has_more
                }))
            }
            "company.chat.group.unread.list" => {
                let input: CompanyGroupUnreadListToolInput = parse_input(input)?;
                let unread = self.platform.list_company_group_unread_messages(
                    ListCompanyGroupUnreadInput {
                        actor_agent_id: agent_id,
                        company_id: input.company_id,
                        conversation_id: input.conversation_id,
                        message_limit: input.message_limit.unwrap_or(20),
                    },
                )?;
                Ok(json!({ "unread": unread }))
            }
            "company.chat.group.read" => {
                let input: CompanyGroupReadToolInput = parse_input(input)?;
                let result = self
                    .platform
                    .mark_company_group_read(MarkCompanyGroupReadInput {
                        actor_agent_id: agent_id,
                        company_id: input.company_id,
                        conversation_id: input.conversation_id,
                    })?;
                Ok(json!({ "result": result }))
            }
            "company.project.create" => {
                let input: CompanyProjectCreateToolInput = parse_input(input)?;
                let project = self
                    .platform
                    .create_company_project(CreateCompanyProjectInput {
                        actor_agent_id: agent_id,
                        company_id: input.company_id,
                        name: input.name,
                        description: input.description,
                        member_agent_ids: input.member_agent_ids,
                    })?;
                Ok(json!({ "project": project }))
            }
            "company.project.update" => {
                let input: CompanyProjectUpdateToolInput = parse_input(input)?;
                let project = self
                    .platform
                    .update_company_project(UpdateCompanyProjectInput {
                        actor_agent_id: agent_id,
                        company_id: input.company_id,
                        project_id: input.project_id,
                        name: input.name,
                        description: input.description,
                        due_at: input.due_at,
                        clear_due_at: input.clear_due_at,
                    })?;
                Ok(json!({ "project": project }))
            }
            "company.project.get" => {
                let input: CompanyProjectGetToolInput = parse_input(input)?;
                let project = self.platform.get_company_project(GetCompanyProjectInput {
                    actor_agent_id: agent_id,
                    company_id: input.company_id,
                    project_id: input.project_id,
                })?;
                Ok(json!({ "project": project }))
            }
            "company.project.list" => {
                let input: CompanyProjectListToolInput = parse_input(input)?;
                let projects = self
                    .platform
                    .list_company_projects(agent_id, input.company_id)?;
                Ok(json!({ "projects": projects }))
            }
            "company.project.member.add" => {
                let input: CompanyProjectMemberToolInput = parse_input(input)?;
                let project =
                    self.platform
                        .add_company_project_member(AddCompanyProjectMemberInput {
                            actor_agent_id: agent_id,
                            company_id: input.company_id,
                            project_id: input.project_id,
                            target_agent_id: input.target_agent_id,
                        })?;
                Ok(json!({ "project": project }))
            }
            "company.project.member.remove" => {
                let input: CompanyProjectMemberToolInput = parse_input(input)?;
                let project = self.platform.remove_company_project_member(
                    RemoveCompanyProjectMemberInput {
                        actor_agent_id: agent_id,
                        company_id: input.company_id,
                        project_id: input.project_id,
                        target_agent_id: input.target_agent_id,
                    },
                )?;
                Ok(json!({ "project": project }))
            }
            "company.project.task.create" => {
                let input: CompanyProjectTaskCreateToolInput = parse_input(input)?;
                let task =
                    self.platform
                        .create_company_project_task(CreateCompanyProjectTaskInput {
                            actor_agent_id: agent_id,
                            company_id: input.company_id,
                            project_id: input.project_id,
                            title: input.title,
                            description: input.description,
                            priority: input.priority,
                            assignee_agent_id: input.assignee_agent_id,
                            due_at: input.due_at,
                        })?;
                Ok(json!({ "task": task }))
            }
            "company.project.task.update" => {
                let input: CompanyProjectTaskUpdateToolInput = parse_input(input)?;
                let task =
                    self.platform
                        .update_company_project_task(UpdateCompanyProjectTaskInput {
                            actor_agent_id: agent_id,
                            company_id: input.company_id,
                            project_id: input.project_id,
                            task_id: input.task_id,
                            title: input.title,
                            description: input.description,
                            status: input.status,
                            priority: input.priority,
                            assignee_agent_id: input.assignee_agent_id,
                            due_at: input.due_at,
                        })?;
                Ok(json!({ "task": task }))
            }
            "company.project.task.batch_update" => {
                let input: CompanyProjectTaskBatchUpdateToolInput = parse_input(input)?;
                let tasks = self.platform.batch_update_company_project_tasks(
                    BatchUpdateCompanyProjectTasksInput {
                        actor_agent_id: agent_id,
                        company_id: input.company_id,
                        project_id: input.project_id,
                        task_ids: input.task_ids,
                        status: input.status,
                        priority: input.priority,
                        assignee_agent_id: input.assignee_agent_id,
                        clear_assignee: input.clear_assignee,
                        due_at: input.due_at,
                        clear_due_at: input.clear_due_at,
                    },
                )?;
                Ok(json!({ "tasks": tasks }))
            }
            "company.project.task.dependency.add" => {
                let input: CompanyProjectTaskDependencyToolInput = parse_input(input)?;
                let dependency = self.platform.add_company_project_task_dependency(
                    ChangeCompanyProjectTaskDependencyInput {
                        actor_agent_id: agent_id,
                        company_id: input.company_id,
                        project_id: input.project_id,
                        task_id: input.task_id,
                        depends_on_task_id: input.depends_on_task_id,
                        dependency_condition: input.dependency_condition,
                    },
                )?;
                Ok(json!({ "dependency": dependency }))
            }
            "company.project.task.dependency.remove" => {
                let input: CompanyProjectTaskDependencyToolInput = parse_input(input)?;
                self.platform.remove_company_project_task_dependency(
                    ChangeCompanyProjectTaskDependencyInput {
                        actor_agent_id: agent_id,
                        company_id: input.company_id,
                        project_id: input.project_id,
                        task_id: input.task_id,
                        depends_on_task_id: input.depends_on_task_id,
                        dependency_condition: None,
                    },
                )?;
                Ok(json!({
                    "project_id": input.project_id,
                    "task_id": input.task_id,
                    "depends_on_task_id": input.depends_on_task_id,
                    "removed": true
                }))
            }
            "company.project.status.update" => {
                let input: CompanyProjectStatusUpdateToolInput = parse_input(input)?;
                let update = self.platform.create_company_project_status_update(
                    CreateCompanyProjectStatusUpdateInput {
                        actor_agent_id: agent_id,
                        company_id: input.company_id,
                        project_id: input.project_id,
                        summary: input.summary,
                        progress_percent: input.progress_percent,
                        blockers: input.blockers,
                        next_steps: input.next_steps,
                        project_status: input.project_status,
                    },
                )?;
                Ok(json!({ "status_update": update }))
            }
            "company.staff.hire" => {
                let input: CompanyStaffHireToolInput = parse_input(input)?;
                let profession = company_profession_by_key(input.profession_key.as_str())
                    .ok_or_else(|| AppError::Validation("unsupported profession_key".into()))?;
                let result = self.platform.hire_company_agent(AgentStaffingHireInput {
                    actor_agent_id: agent_id,
                    company_id: input.company_id,
                    display_name: input.display_name,
                    handle: input.handle,
                    persona: input.persona,
                    org_unit_id: input.org_unit_id,
                    job_title: Some(profession.label),
                    reports_to_membership_id: input.reports_to_membership_id,
                    reason: input.reason,
                    idempotency_key,
                })?;
                Ok(json!({ "result": result }))
            }
            "company.staff.suspend" => {
                let input: CompanyStaffStatusToolInput = parse_input(input)?;
                let result =
                    self.platform
                        .suspend_company_agent_as_agent(AgentStaffingStatusInput {
                            actor_agent_id: agent_id,
                            company_id: input.company_id,
                            target_agent_id: input.target_agent_id,
                            reason: input.reason,
                            handoff_plan: input.handoff_plan,
                            handoff_agent_id: input.handoff_agent_id,
                            idempotency_key,
                        })?;
                Ok(json!({ "result": result }))
            }
            "company.staff.terminate" => {
                let input: CompanyStaffStatusToolInput = parse_input(input)?;
                let result =
                    self.platform
                        .terminate_company_agent_as_agent(AgentStaffingStatusInput {
                            actor_agent_id: agent_id,
                            company_id: input.company_id,
                            target_agent_id: input.target_agent_id,
                            reason: input.reason,
                            handoff_plan: input.handoff_plan,
                            handoff_agent_id: input.handoff_agent_id,
                            idempotency_key,
                        })?;
                Ok(json!({ "result": result }))
            }
            "company.staff.action.list" => {
                let input: CompanyStaffActionListToolInput = parse_input(input)?;
                let actions = self
                    .platform
                    .list_company_staffing_actions_for_agent(agent_id, input.company_id)?;
                Ok(json!({ "staffing_actions": actions }))
            }
            "company.staff.action.get" => {
                let input: CompanyStaffActionGetToolInput = parse_input(input)?;
                let action = self.platform.get_company_staffing_action_for_agent(
                    agent_id,
                    input.company_id,
                    input.action_id,
                )?;
                Ok(json!({ "staffing_action": action }))
            }

            _ => Err(AppError::NotFound(format!(
                "unknown standard MCP tool: {tool_name}"
            ))),
        }
    }
}
