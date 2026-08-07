use super::handler::{filter_inbox_events, parse_input};
use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> McpGateway<R, V> {
    pub(super) fn execute_agent_tool(
        &self,
        agent_id: Uuid,
        tool_name: &str,
        input: Value,
        idempotency_key: Option<String>,
    ) -> AppResult<Value> {
        match tool_name {
            "agent.bootstrap" => {
                let profile = self.platform.get_agent_profile_by_id(agent_id)?;
                let membership = self
                    .platform
                    .get_active_company_agent_membership(agent_id)?;
                let context =
                    self.platform
                        .get_company_agent_context(GetCompanyAgentContextInput {
                            actor_agent_id: agent_id,
                            company_id: membership.company_id,
                        })?;
                let org_unit = context
                    .org_units
                    .iter()
                    .find(|unit| unit.id == membership.org_unit_id)
                    .cloned();
                let reports_to = membership.reports_to_membership_id.and_then(|manager_id| {
                    context
                        .agents
                        .iter()
                        .find(|coworker| coworker.membership.id == manager_id)
                        .cloned()
                });
                let coworkers = context
                    .agents
                    .iter()
                    .filter(|coworker| coworker.agent_profile.id != agent_id)
                    .cloned()
                    .collect::<Vec<_>>();
                let pending_inbox = self.platform.list_agent_inbox_events(agent_id, true, 50)?;
                let profession = infer_company_profession(Some(&membership.job_title));
                let memory_overview =
                    self.platform
                        .agent_memory_overview(agent_id, membership.company_id, None)?;
                let unread_group_messages = self.platform.list_company_group_unread_messages(
                    ListCompanyGroupUnreadInput {
                        actor_agent_id: agent_id,
                        company_id: membership.company_id,
                        conversation_id: None,
                        message_limit: 20,
                    },
                )?;
                let work_sessions = self.platform.list_agent_codex_sessions(agent_id, 20);
                let mut next_tools = vec![
                    "agent.profile.update",
                    "agent.memory",
                    "agent.work_session",
                    "agent.inbox.wait",
                    "agent.inbox.ack",
                    "company.chat",
                    "company.project",
                    "company.task",
                    "company.events",
                ];
                if context
                    .actor_membership
                    .permissions
                    .iter()
                    .any(|permission| {
                        matches!(
                            permission.as_str(),
                            COMPANY_PERMISSION_STAFF_HIRE
                                | COMPANY_PERMISSION_STAFF_SUSPEND
                                | COMPANY_PERMISSION_STAFF_TERMINATE
                        )
                    })
                {
                    next_tools.push("company.staff");
                }
                Ok(json!({
                    "agent": profile,
                    "company": context.company,
                    "membership": membership,
                    "profession": profession,
                    "org_unit": org_unit,
                    "reports_to": reports_to,
                    "coworkers": coworkers,
                    "permissions": context.actor_membership.permissions,
                    "conversations": context.conversations,
                    "projects": context.projects,
                    "pending_inbox": pending_inbox,
                    "memory_overview": memory_overview,
                    "work_sessions": work_sessions,
                    "unread_group_messages": unread_group_messages,
                    "next_tools": next_tools
                }))
            }
            "agent.get_profile" => {
                let profile = self.platform.get_agent_profile_by_id(agent_id)?;
                let membership = self
                    .platform
                    .get_active_company_agent_membership(agent_id)?;
                let context =
                    self.platform
                        .get_company_agent_context(GetCompanyAgentContextInput {
                            actor_agent_id: agent_id,
                            company_id: membership.company_id,
                        })?;
                let org_unit = context
                    .org_units
                    .iter()
                    .find(|unit| unit.id == membership.org_unit_id)
                    .cloned();
                Ok(json!({
                    "agent_profile": profile,
                    "membership": membership,
                    "org_unit": org_unit
                }))
            }
            "agent.profile.update" => {
                let input: AgentProfileUpdateToolInput = parse_input(input)?;
                let _ = input.idempotency_key;
                let work_profile = self.platform.update_company_agent_work_profile(
                    UpdateCompanyAgentWorkProfileInput {
                        actor_agent_id: agent_id,
                        responsibilities: input.responsibilities,
                        skills: input.skills,
                        current_focus: input.current_focus,
                        collaboration_preference: input.collaboration_preference,
                    },
                )?;
                Ok(json!({ "work_profile": work_profile }))
            }
            "agent.memory" => {
                let input: AgentMemoryToolInput = parse_input(input)?;
                let _ = input.idempotency_key;
                match input.operation {
                    AgentMemoryOperation::Overview {
                        company_id,
                        project_id,
                    } => {
                        let overview = self
                            .platform
                            .agent_memory_overview(agent_id, company_id, project_id)?;
                        Ok(json!({ "overview": overview }))
                    }
                    AgentMemoryOperation::Search {
                        company_id,
                        scopes,
                        project_id,
                        session_id,
                        query,
                        memory_tiers,
                        memory_types,
                        tags,
                        status,
                        limit,
                    } => {
                        let memories =
                            self.platform
                                .search_agent_memories(SearchAgentMemoriesInput {
                                    actor_agent_id: agent_id,
                                    company_id,
                                    scopes,
                                    project_id,
                                    session_id,
                                    query,
                                    memory_tiers,
                                    memory_types,
                                    tags,
                                    status,
                                    limit,
                                })?;
                        Ok(json!({ "memories": memories }))
                    }
                    AgentMemoryOperation::Get {
                        company_id,
                        memory_id,
                    } => {
                        let memory = self.platform.get_agent_memory(GetAgentMemoryInput {
                            actor_agent_id: agent_id,
                            company_id,
                            memory_id,
                        })?;
                        Ok(json!({ "memory": memory }))
                    }
                    AgentMemoryOperation::Remember {
                        company_id,
                        scope,
                        project_id,
                        session_id,
                        memory_tier,
                        memory_type,
                        topic_key,
                        title,
                        summary,
                        when_to_use,
                        tags,
                        importance,
                        confidence,
                        source_refs,
                        expires_at,
                        supersedes_memory_id,
                    } => {
                        let memory =
                            self.platform
                                .remember_agent_memory(RememberAgentMemoryInput {
                                    actor_agent_id: agent_id,
                                    company_id,
                                    scope,
                                    project_id,
                                    session_id,
                                    memory_tier,
                                    memory_type,
                                    topic_key,
                                    title,
                                    summary,
                                    when_to_use,
                                    tags,
                                    importance,
                                    confidence,
                                    source_refs: source_refs
                                        .into_iter()
                                        .map(AgentMemorySourceRef::from)
                                        .collect(),
                                    expires_at,
                                    supersedes_memory_id,
                                })?;
                        Ok(json!({ "memory": memory }))
                    }
                    AgentMemoryOperation::Update {
                        company_id,
                        memory_id,
                        memory_tier,
                        title,
                        summary,
                        when_to_use,
                        tags,
                        importance,
                        confidence,
                        expires_at,
                        clear_expires_at,
                    } => {
                        let memory = self.platform.update_agent_memory(UpdateAgentMemoryInput {
                            actor_agent_id: agent_id,
                            company_id,
                            memory_id,
                            memory_tier,
                            title,
                            summary,
                            when_to_use,
                            tags,
                            importance,
                            confidence,
                            expires_at,
                            clear_expires_at,
                        })?;
                        Ok(json!({ "memory": memory }))
                    }
                    AgentMemoryOperation::Archive {
                        company_id,
                        memory_id,
                    } => {
                        let memory =
                            self.platform
                                .set_agent_memory_state(SetAgentMemoryStateInput {
                                    actor_agent_id: agent_id,
                                    company_id,
                                    memory_id,
                                    status: Some(AGENT_MEMORY_STATUS_ARCHIVED.into()),
                                    pinned: Some(false),
                                })?;
                        Ok(json!({ "memory": memory }))
                    }
                    AgentMemoryOperation::Supersede {
                        company_id,
                        memory_id,
                    } => {
                        let memory =
                            self.platform
                                .set_agent_memory_state(SetAgentMemoryStateInput {
                                    actor_agent_id: agent_id,
                                    company_id,
                                    memory_id,
                                    status: Some(AGENT_MEMORY_STATUS_SUPERSEDED.into()),
                                    pinned: Some(false),
                                })?;
                        Ok(json!({ "memory": memory }))
                    }
                    AgentMemoryOperation::Pin {
                        company_id,
                        memory_id,
                        pinned,
                    } => {
                        let memory =
                            self.platform
                                .set_agent_memory_state(SetAgentMemoryStateInput {
                                    actor_agent_id: agent_id,
                                    company_id,
                                    memory_id,
                                    status: None,
                                    pinned: Some(pinned),
                                })?;
                        Ok(json!({ "memory": memory }))
                    }
                    AgentMemoryOperation::Forget {
                        company_id,
                        memory_id,
                    } => {
                        self.platform.forget_agent_memory(GetAgentMemoryInput {
                            actor_agent_id: agent_id,
                            company_id,
                            memory_id,
                        })?;
                        Ok(json!({ "forgotten": true, "memory_id": memory_id }))
                    }
                }
            }
            "agent.work_session" => {
                let input: AgentWorkSessionToolInput = parse_input(input)?;
                let request_key = input.idempotency_key.or(idempotency_key);
                let membership = self
                    .platform
                    .get_active_company_agent_membership(agent_id)?;
                match input.operation {
                    AgentWorkSessionOperation::List {
                        company_id,
                        project_id,
                        status,
                        limit,
                    } => {
                        if membership.company_id != company_id {
                            return Err(AppError::Unauthorized(
                                "Agent does not belong to the requested company".into(),
                            ));
                        }
                        let sessions = self
                            .platform
                            .list_agent_codex_sessions(agent_id, limit.unwrap_or(20))
                            .into_iter()
                            .filter(|session| {
                                project_id
                                    .is_none_or(|project_id| session.project_id == Some(project_id))
                            })
                            .filter(|session| {
                                status
                                    .as_deref()
                                    .is_none_or(|status| session.status == status)
                            })
                            .collect::<Vec<_>>();
                        Ok(json!({ "sessions": sessions }))
                    }
                    AgentWorkSessionOperation::Get {
                        company_id,
                        session_id,
                    } => {
                        if membership.company_id != company_id {
                            return Err(AppError::Unauthorized(
                                "Agent does not belong to the requested company".into(),
                            ));
                        }
                        let session = self
                            .platform
                            .list_agent_codex_sessions(agent_id, 100)
                            .into_iter()
                            .find(|session| session.id == session_id)
                            .ok_or_else(|| {
                                AppError::NotFound("Agent work session not found".into())
                            })?;
                        Ok(json!({ "session": session }))
                    }
                    AgentWorkSessionOperation::Dispatch {
                        company_id,
                        project_id,
                        source_event_ids,
                        task_ids,
                        objective,
                        acceptance_criteria,
                        priority,
                        dedupe_key,
                    } => {
                        if membership.company_id != company_id {
                            return Err(AppError::Unauthorized(
                                "Agent does not belong to the requested company".into(),
                            ));
                        }
                        let objective = objective.trim().to_string();
                        if objective.is_empty() || objective.chars().count() > 4_000 {
                            return Err(AppError::Validation(
                                "execution objective must contain 1 to 4000 characters".into(),
                            ));
                        }
                        let priority = priority.unwrap_or_else(|| "normal".into());
                        if !matches!(priority.as_str(), "low" | "normal" | "high" | "urgent") {
                            return Err(AppError::Validation(
                                "execution priority must be low, normal, high, or urgent".into(),
                            ));
                        }
                        let dedupe_key = dedupe_key
                            .or(request_key)
                            .unwrap_or_else(|| format!("dispatch-{}", Uuid::new_v4().simple()));
                        let now = Utc::now();
                        let intent = AgentExecutionIntent {
                            id: Uuid::new_v4(),
                            company_id,
                            agent_profile_id: agent_id,
                            project_id,
                            worker_session_id: None,
                            source_event_ids,
                            task_ids,
                            action_type: AGENT_EXECUTION_INTENT_ACTION_EXECUTE.into(),
                            objective,
                            acceptance_criteria,
                            priority,
                            dedupe_key,
                            status: AGENT_EXECUTION_INTENT_STATUS_PENDING.into(),
                            result_summary: String::new(),
                            error_message: None,
                            created_at: now,
                            claimed_at: None,
                            completed_at: None,
                        };
                        let intent = self.platform.create_agent_execution_intent(intent)?;
                        Ok(json!({ "intent": intent }))
                    }
                }
            }
            "agent.inbox.list" => {
                let input: AgentInboxListInput = parse_input(input)?;
                let items = self.platform.list_agent_inbox_events(
                    agent_id,
                    input.pending_only.unwrap_or(true),
                    input.limit.unwrap_or(20),
                )?;
                Ok(json!({ "events": items }))
            }
            "agent.inbox.wait" => {
                let input: AgentInboxWaitInput = parse_input(input)?;
                let timeout_seconds = input.timeout_seconds.unwrap_or(0).min(25);
                let items = filter_inbox_events(
                    self.platform.list_agent_inbox_events(
                        agent_id,
                        input.pending_only.unwrap_or(true),
                        input.limit.unwrap_or(20).clamp(1, 100),
                    )?,
                    input.event_types.as_deref(),
                );
                Ok(json!({
                    "events": items,
                    "timed_out": items.is_empty(),
                    "timeout_seconds": timeout_seconds
                }))
            }
            "agent.inbox.ack" => {
                let input: AgentInboxProcessInput = parse_input(input)?;
                let event = self.platform.mark_agent_inbox_event_processed(
                    MarkInboxEventProcessedInput {
                        actor_agent_id: agent_id,
                        event_id: input.event_id,
                    },
                )?;
                Ok(json!({ "event": event }))
            }

            _ => Err(AppError::NotFound(format!(
                "unknown standard MCP tool: {tool_name}"
            ))),
        }
    }
}
