use std::{sync::Arc, time::Duration};

use ai_chat_application::{
    AddCompanyProjectMemberInput, AgentStaffingHireInput, AgentStaffingStatusInput,
    BatchUpdateCompanyProjectTasksInput, ChangeCompanyProjectTaskDependencyInput,
    CompanyProjectAssetInput, CreateCompanyGroupConversationInput, CreateCompanyProjectInput,
    CreateCompanyProjectStatusUpdateInput, CreateCompanyProjectTaskInput, GetAgentMemoryInput,
    GetCompanyAgentContextInput, GetCompanyProjectInput, ListCompanyGroupUnreadInput,
    MarkCompanyGroupReadInput, MarkInboxEventProcessedInput, OpenCompanyDirectConversationInput,
    OwnershipProofVerifier, PlatformApp, PlatformRepository, RememberAgentMemoryInput,
    RemoveCompanyProjectMemberInput, ReplaceCompanyProjectAssetsInput,
    ReplyCompanyInboxMessageInput, SearchAgentMemoriesInput, SendCompanyMessageWithMentionsInput,
    SetAgentMemoryStateInput, UpdateAgentMemoryInput, UpdateCompanyAgentWorkProfileInput,
    UpdateCompanyProjectInput, UpdateCompanyProjectRuleInput, UpdateCompanyProjectTaskInput,
    UpsertCompanyProjectGitInput,
};
use ai_chat_domain::agent_identity::AgentActionStatus;
use ai_chat_domain::company::{
    company_profession_by_key, infer_company_profession, AgentMemorySourceRef,
    AGENT_MEMORY_STATUS_ARCHIVED, AGENT_MEMORY_STATUS_SUPERSEDED,
    COMPANY_PERMISSION_PROJECT_CREATE, COMPANY_PERMISSION_PROJECT_MANAGE,
    COMPANY_PERMISSION_STAFF_HIRE, COMPANY_PERMISSION_STAFF_SUSPEND,
    COMPANY_PERMISSION_STAFF_TERMINATE, COMPANY_PERMISSION_TASK_ASSIGN,
    COMPANY_PERMISSION_TASK_UPDATE, PROJECT_STATUS_PAUSED,
};
use ai_chat_infrastructure::gitness::{
    ProjectGitProvisionRequest, ProjectGitProvisioner, ProvisionedProjectGit,
};
use ai_chat_shared::{AppError, AppResult};
use chrono::{DateTime, Utc};
use http::{header::AUTHORIZATION, request::Parts, HeaderMap};
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResult, ErrorCode, Implementation, JsonObject,
        ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
        ToolAnnotations,
    },
    service::RequestContext,
    ErrorData, RoleServer, ServerHandler,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

pub const STANDARD_MCP_PATH: &str = "/mcp";

#[derive(Debug, Clone, Serialize)]
pub struct McpInvocation {
    pub tool: String,
    pub agent_id: Uuid,
    pub output: Value,
}

#[derive(Clone)]
pub struct McpGateway<R: PlatformRepository, V: OwnershipProofVerifier> {
    platform: PlatformApp<R, V>,
    fallback_agent_key: Option<String>,
    project_git_provisioner: Option<Arc<dyn ProjectGitProvisioner>>,
}

impl<R: PlatformRepository, V: OwnershipProofVerifier> McpGateway<R, V> {
    pub fn new(platform: PlatformApp<R, V>, fallback_agent_key: Option<String>) -> Self {
        Self {
            platform,
            fallback_agent_key,
            project_git_provisioner: None,
        }
    }

    pub fn with_project_git_provisioner(
        mut self,
        provisioner: Option<Arc<dyn ProjectGitProvisioner>>,
    ) -> Self {
        self.project_git_provisioner = provisioner;
        self
    }

    fn provision_project_git(
        &self,
        agent_id: Uuid,
        company_id: Uuid,
        project: &ai_chat_application::CompanyProjectView,
        repository_identifier: Option<String>,
        is_public: bool,
    ) -> AppResult<ProvisionedProjectGit> {
        if project.project.status == PROJECT_STATUS_PAUSED {
            return Err(AppError::Conflict(
                "project is paused; resume it before provisioning Git".into(),
            ));
        }
        let provisioner = self.project_git_provisioner.as_ref().ok_or_else(|| {
            AppError::Validation("automatic Git provider is not configured".into())
        })?;
        let provisioned = provisioner.provision(ProjectGitProvisionRequest {
            project_id: project.project.id,
            project_name: project.project.name.clone(),
            description: project.project.description.clone(),
            repository_identifier,
            is_public,
        })?;
        self.platform
            .upsert_company_project_git(UpsertCompanyProjectGitInput {
                actor_agent_id: agent_id,
                company_id,
                project_id: project.project.id,
                remote_url: provisioned.remote_url.clone(),
                host_local_path: None,
                default_branch: Some(provisioned.default_branch.clone()),
                auth_profile: Some(provisioned.auth_profile.clone()),
                allow_agent_push: Some(true),
                branch_prefix: Some("relay/".into()),
            })?;
        Ok(provisioned)
    }

    pub fn invoke(
        &self,
        agent_key: Option<&str>,
        tool_name: &str,
        input: Value,
    ) -> AppResult<McpInvocation> {
        self.invoke_with_credentials(agent_key, None, tool_name, input)
    }

    pub fn invoke_with_credentials(
        &self,
        agent_key: Option<&str>,
        agent_run_token: Option<&str>,
        tool_name: &str,
        input: Value,
    ) -> AppResult<McpInvocation> {
        if !is_public_tool_name(tool_name) {
            return Err(AppError::NotFound(format!("unknown MCP tool: {tool_name}")));
        }
        let agent = self.authenticate_agent(agent_key, agent_run_token)?;
        let request_input = input.clone();
        let idempotency_key = idempotency_key_from_input(&request_input);
        let is_mutating = is_mutating_tool(tool_name, &request_input);
        if is_mutating {
            if let Some(key) = idempotency_key.as_deref() {
                if let Some(output) = self.platform.replay_agent_idempotency(
                    agent.id,
                    tool_name,
                    key,
                    &request_input,
                )? {
                    return Ok(McpInvocation {
                        tool: tool_name.to_string(),
                        agent_id: agent.id,
                        output,
                    });
                }
            }
            self.platform.enforce_agent_action_budget(agent.id)?;
        }
        let result = self.execute(
            agent.id,
            tool_name,
            input_without_idempotency(input),
            idempotency_key.clone(),
        );

        match result {
            Ok(output) => {
                if is_mutating {
                    let action_name = audit_action_name(tool_name, &request_input);
                    self.platform.record_agent_action(
                        agent.id,
                        action_name,
                        success_target_ref(tool_name, &request_input, &output),
                        request_input.clone(),
                        output.clone(),
                        AgentActionStatus::Success,
                    )?;
                    if let Some(key) = idempotency_key.as_deref() {
                        self.platform.store_agent_idempotency(
                            agent.id,
                            tool_name,
                            key,
                            &request_input,
                            &output,
                        )?;
                    }
                }

                Ok(McpInvocation {
                    tool: tool_name.to_string(),
                    agent_id: agent.id,
                    output,
                })
            }
            Err(error) => {
                if is_mutating {
                    let action_name = audit_action_name(tool_name, &request_input);
                    let _ = self.platform.record_agent_action(
                        agent.id,
                        action_name,
                        failure_target_ref(tool_name, &request_input),
                        request_input,
                        json!({
                            "code": error.code(),
                            "message": error.to_string(),
                        }),
                        action_status_for_error(&error),
                    );
                }
                Err(error)
            }
        }
    }

    fn authenticate_agent(
        &self,
        agent_key: Option<&str>,
        agent_run_token: Option<&str>,
    ) -> AppResult<ai_chat_domain::agent_identity::AgentProfile> {
        if let Some(agent_key) = agent_key.filter(|value| !value.trim().is_empty()) {
            return self.platform.authenticate_agent_key(agent_key);
        }
        if let Some(run_token) = agent_run_token.filter(|value| !value.trim().is_empty()) {
            return self.platform.authenticate_agent_codex_run_token(run_token);
        }
        if let Some(agent_key) = self.fallback_agent_key.as_deref() {
            return self.platform.authenticate_agent_key(agent_key);
        }
        Err(AppError::Unauthorized(
            "missing x-agent-key, bearer token, or x-agent-run-token".into(),
        ))
    }

    fn execute(
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
                let mut next_tools = vec![
                    "agent.profile.update",
                    "agent.memory",
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
                        project_id,
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
                                    project_id,
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
                        project_id,
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
                                    project_id,
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
            "company.events" => {
                let input: CompanyEventsToolInput = parse_input(input)?;
                let events = self.platform.list_company_realtime_events_for_agent(
                    agent_id,
                    input.company_id,
                    input.after_sequence_id.unwrap_or(0),
                    input.limit.unwrap_or(100),
                )?;
                Ok(json!({ "events": events }))
            }
            "company.chat" => {
                let input: CompanyChatToolInput = parse_input(input)?;
                let _ = input.idempotency_key;
                match input.operation {
                    CompanyChatOperation::DirectOpen {
                        company_id,
                        target_agent_id,
                    } => {
                        let conversation = self.platform.open_company_direct_conversation(
                            OpenCompanyDirectConversationInput {
                                actor_agent_id: agent_id,
                                company_id,
                                target_agent_id,
                            },
                        )?;
                        Ok(json!({ "conversation": conversation }))
                    }
                    CompanyChatOperation::GroupCreate {
                        company_id,
                        title,
                        member_agent_ids,
                    } => {
                        let conversation = self.platform.create_company_group_conversation(
                            CreateCompanyGroupConversationInput {
                                actor_agent_id: agent_id,
                                company_id,
                                title,
                                member_agent_ids,
                            },
                        )?;
                        Ok(json!({ "conversation": conversation }))
                    }
                    CompanyChatOperation::Send {
                        company_id,
                        conversation_id,
                        content,
                        mentioned_agent_ids,
                        mention_all,
                    } => {
                        let message = self.platform.send_company_message_with_mentions(
                            SendCompanyMessageWithMentionsInput {
                                actor_agent_id: agent_id,
                                company_id,
                                conversation_id,
                                content,
                                mentioned_agent_ids,
                                mention_all,
                            },
                        )?;
                        Ok(json!({ "message": message }))
                    }
                    CompanyChatOperation::Reply {
                        event_id,
                        content,
                        auto_ack,
                    } => {
                        let result = self.platform.reply_to_company_inbox_message(
                            ReplyCompanyInboxMessageInput {
                                actor_agent_id: agent_id,
                                event_id,
                                content,
                                auto_ack: auto_ack.unwrap_or(true),
                            },
                        )?;
                        Ok(json!({ "message": result.message, "event": result.event }))
                    }
                    CompanyChatOperation::History {
                        company_id,
                        conversation_id,
                        before_message_id,
                        limit,
                    } => {
                        let context = self.platform.get_company_agent_context(
                            GetCompanyAgentContextInput {
                                actor_agent_id: agent_id,
                                company_id,
                            },
                        )?;
                        if !context
                            .conversations
                            .iter()
                            .any(|conversation| conversation.preview.id == conversation_id)
                        {
                            return Err(AppError::Unauthorized(
                                "agent is not a member of the requested company conversation"
                                    .into(),
                            ));
                        }
                        let page = self.platform.get_agent_conversation_message_page(
                            agent_id,
                            conversation_id,
                            before_message_id,
                            limit.unwrap_or(50),
                        )?;
                        Ok(json!({
                            "messages": page.messages,
                            "next_cursor": page.next_cursor,
                            "has_more": page.has_more
                        }))
                    }
                    CompanyChatOperation::Unread {
                        company_id,
                        conversation_id,
                        message_limit,
                    } => {
                        let unread = self.platform.list_company_group_unread_messages(
                            ListCompanyGroupUnreadInput {
                                actor_agent_id: agent_id,
                                company_id,
                                conversation_id,
                                message_limit: message_limit.unwrap_or(20),
                            },
                        )?;
                        Ok(json!({ "unread": unread }))
                    }
                    CompanyChatOperation::MarkRead {
                        company_id,
                        conversation_id,
                    } => {
                        let result =
                            self.platform
                                .mark_company_group_read(MarkCompanyGroupReadInput {
                                    actor_agent_id: agent_id,
                                    company_id,
                                    conversation_id,
                                })?;
                        Ok(json!({ "result": result }))
                    }
                }
            }
            "company.project" => {
                let input: CompanyProjectToolInput = parse_input(input)?;
                let _ = input.idempotency_key;
                match input.operation {
                    CompanyProjectOperation::Create {
                        company_id,
                        name,
                        description,
                        member_agent_ids,
                        provision_git,
                    } => {
                        let project =
                            self.platform
                                .create_company_project(CreateCompanyProjectInput {
                                    actor_agent_id: agent_id,
                                    company_id,
                                    name,
                                    description,
                                    member_agent_ids,
                                })?;
                        let git_provisioning = if provision_git.unwrap_or(true) {
                            match self
                                .provision_project_git(agent_id, company_id, &project, None, false)
                            {
                                Ok(git) => json!({ "status": "ready", "git": git }),
                                Err(error) => json!({
                                    "status": "failed",
                                    "code": error.code(),
                                    "message": error.to_string(),
                                    "retry_action": "git_provision"
                                }),
                            }
                        } else {
                            json!({ "status": "skipped" })
                        };
                        let project =
                            self.platform.get_company_project(GetCompanyProjectInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id: project.project.id,
                            })?;
                        Ok(json!({
                            "project": project,
                            "git_provisioning": git_provisioning
                        }))
                    }
                    CompanyProjectOperation::GitProvision {
                        company_id,
                        project_id,
                        repository_identifier,
                        is_public,
                    } => {
                        let project =
                            self.platform.get_company_project(GetCompanyProjectInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                            })?;
                        let git = self.provision_project_git(
                            agent_id,
                            company_id,
                            &project,
                            repository_identifier,
                            is_public.unwrap_or(false),
                        )?;
                        let project =
                            self.platform.get_company_project(GetCompanyProjectInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                            })?;
                        Ok(json!({ "project": project, "git": git }))
                    }
                    CompanyProjectOperation::Update {
                        company_id,
                        project_id,
                        name,
                        description,
                        due_at,
                        clear_due_at,
                    } => {
                        let project =
                            self.platform
                                .update_company_project(UpdateCompanyProjectInput {
                                    actor_agent_id: agent_id,
                                    company_id,
                                    project_id,
                                    name,
                                    description,
                                    due_at,
                                    clear_due_at,
                                })?;
                        Ok(json!({ "project": project }))
                    }
                    CompanyProjectOperation::Get {
                        company_id,
                        project_id,
                    } => {
                        let project =
                            self.platform.get_company_project(GetCompanyProjectInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                            })?;
                        Ok(json!({ "project": project }))
                    }
                    CompanyProjectOperation::List { company_id } => {
                        let projects = self.platform.list_company_projects(agent_id, company_id)?;
                        Ok(json!({ "projects": projects }))
                    }
                    CompanyProjectOperation::MemberAdd {
                        company_id,
                        project_id,
                        target_agent_id,
                    } => {
                        let project = self.platform.add_company_project_member(
                            AddCompanyProjectMemberInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                target_agent_id,
                            },
                        )?;
                        Ok(json!({ "project": project }))
                    }
                    CompanyProjectOperation::MemberRemove {
                        company_id,
                        project_id,
                        target_agent_id,
                    } => {
                        let project = self.platform.remove_company_project_member(
                            RemoveCompanyProjectMemberInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                target_agent_id,
                            },
                        )?;
                        Ok(json!({ "project": project }))
                    }
                    CompanyProjectOperation::StatusUpdate {
                        company_id,
                        project_id,
                        summary,
                        progress_percent,
                        blockers,
                        next_steps,
                        project_status,
                    } => {
                        let status_update = self.platform.create_company_project_status_update(
                            CreateCompanyProjectStatusUpdateInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                summary,
                                progress_percent,
                                blockers,
                                next_steps,
                                project_status,
                            },
                        )?;
                        Ok(json!({ "status_update": status_update }))
                    }
                    CompanyProjectOperation::RuleUpdate {
                        company_id,
                        project_id,
                        content,
                    } => {
                        let rule = self.platform.update_company_project_rule(
                            UpdateCompanyProjectRuleInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                content,
                            },
                        )?;
                        Ok(json!({ "rule": rule }))
                    }
                    CompanyProjectOperation::AssetsReplace {
                        company_id,
                        project_id,
                        assets,
                    } => {
                        let assets = self.platform.replace_company_project_assets(
                            ReplaceCompanyProjectAssetsInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                assets: assets
                                    .into_iter()
                                    .map(|asset| CompanyProjectAssetInput {
                                        name: asset.name,
                                        asset_type: asset.asset_type,
                                        locator: asset.locator,
                                        description: asset.description,
                                        status: asset.status,
                                        metadata: asset.metadata,
                                    })
                                    .collect(),
                            },
                        )?;
                        Ok(json!({ "assets": assets, "refresh_completed": true }))
                    }
                }
            }
            "company.task" => {
                let input: CompanyTaskToolInput = parse_input(input)?;
                let _ = input.idempotency_key;
                match input.operation {
                    CompanyTaskOperation::Get {
                        company_id,
                        project_id,
                        task_id,
                    } => {
                        let project =
                            self.platform.get_company_project(GetCompanyProjectInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                            })?;
                        let task = project
                            .tasks
                            .iter()
                            .find(|task| task.id == task_id)
                            .cloned()
                            .ok_or_else(|| AppError::NotFound("project task not found".into()))?;
                        let dependencies = project
                            .task_dependencies
                            .into_iter()
                            .filter(|dependency| {
                                dependency.task_id == task_id
                                    || dependency.depends_on_task_id == task_id
                            })
                            .collect::<Vec<_>>();
                        let status_history = project
                            .task_status_history
                            .into_iter()
                            .filter(|entry| entry.task_id == task_id)
                            .collect::<Vec<_>>();
                        Ok(json!({
                            "task": task,
                            "project": project.project,
                            "dependencies": dependencies,
                            "status_history": status_history,
                        }))
                    }
                    CompanyTaskOperation::List {
                        company_id,
                        project_id,
                        assignee_agent_id,
                        status,
                    } => {
                        let project =
                            self.platform.get_company_project(GetCompanyProjectInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                            })?;
                        let tasks = project
                            .tasks
                            .into_iter()
                            .filter(|task| {
                                assignee_agent_id
                                    .is_none_or(|assignee| task.assignee_agent_id == Some(assignee))
                                    && status
                                        .as_deref()
                                        .is_none_or(|expected| task.status == expected)
                            })
                            .collect::<Vec<_>>();
                        Ok(json!({
                            "project": project.project,
                            "tasks": tasks,
                            "task_dependencies": project.task_dependencies,
                        }))
                    }
                    CompanyTaskOperation::My { company_id, status } => {
                        let mut assignments = Vec::new();
                        let mut ready_count = 0usize;
                        let mut waiting_count = 0usize;
                        for project in self.platform.list_company_projects(agent_id, company_id)? {
                            for task in project.tasks.iter().filter(|task| {
                                task.assignee_agent_id == Some(agent_id)
                                    && status
                                        .as_deref()
                                        .is_none_or(|expected| task.status == expected)
                            }) {
                                let dependencies = project
                                    .task_dependencies
                                    .iter()
                                    .filter(|dependency| dependency.task_id == task.id)
                                    .filter_map(|dependency| {
                                        project
                                            .tasks
                                            .iter()
                                            .find(|candidate| {
                                                candidate.id == dependency.depends_on_task_id
                                            })
                                            .map(|dependency_task| {
                                                let resolved = matches!(
                                                    dependency_task.status.as_str(),
                                                    "done" | "cancelled"
                                                );
                                                json!({
                                                    "task_id": dependency_task.id,
                                                    "title": dependency_task.title,
                                                    "status": dependency_task.status,
                                                    "assignee_agent_id": dependency_task.assignee_agent_id,
                                                    "resolved": resolved,
                                                })
                                            })
                                    })
                                    .collect::<Vec<_>>();
                                let unresolved_dependencies = dependencies
                                    .iter()
                                    .filter(|dependency| {
                                        dependency.get("resolved").and_then(|value| value.as_bool())
                                            != Some(true)
                                    })
                                    .cloned()
                                    .collect::<Vec<_>>();
                                let can_start = unresolved_dependencies.is_empty();
                                if can_start {
                                    ready_count += 1;
                                } else {
                                    waiting_count += 1;
                                }
                                assignments.push(json!({
                                    "project_id": project.project.id,
                                    "project_name": project.project.name,
                                    "project_status": project.project.status,
                                    "task": task,
                                    "readiness": if can_start { "ready" } else { "waiting_for_dependencies" },
                                    "can_start": can_start,
                                    "dependencies": dependencies,
                                    "unresolved_dependencies": unresolved_dependencies,
                                    "guidance": if can_start {
                                        "任务前置已满足，可以按职责开始处理。"
                                    } else {
                                        "前置任务尚未完成：本轮保持任务原状态，不发送等待占位消息，结束后由下一次定时检查重新判断。"
                                    },
                                }));
                            }
                        }
                        Ok(json!({
                            "assignments": assignments,
                            "ready_count": ready_count,
                            "waiting_count": waiting_count,
                        }))
                    }
                    CompanyTaskOperation::Create {
                        company_id,
                        project_id,
                        title,
                        description,
                        priority,
                        assignee_agent_id,
                        due_at,
                    } => {
                        let task = self.platform.create_company_project_task(
                            CreateCompanyProjectTaskInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                title,
                                description,
                                priority,
                                assignee_agent_id,
                                due_at,
                            },
                        )?;
                        Ok(json!({ "task": task }))
                    }
                    CompanyTaskOperation::Update {
                        company_id,
                        project_id,
                        task_id,
                        title,
                        description,
                        status,
                        priority,
                        assignee_agent_id,
                        due_at,
                    } => {
                        let task = self.platform.update_company_project_task(
                            UpdateCompanyProjectTaskInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                task_id,
                                title,
                                description,
                                status,
                                priority,
                                assignee_agent_id,
                                due_at,
                            },
                        )?;
                        Ok(json!({ "task": task }))
                    }
                    CompanyTaskOperation::BatchUpdate {
                        company_id,
                        project_id,
                        task_ids,
                        status,
                        priority,
                        assignee_agent_id,
                        clear_assignee,
                        due_at,
                        clear_due_at,
                    } => {
                        let tasks = self.platform.batch_update_company_project_tasks(
                            BatchUpdateCompanyProjectTasksInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                task_ids,
                                status,
                                priority,
                                assignee_agent_id,
                                clear_assignee,
                                due_at,
                                clear_due_at,
                            },
                        )?;
                        Ok(json!({ "tasks": tasks }))
                    }
                    CompanyTaskOperation::DependencyAdd {
                        company_id,
                        project_id,
                        task_id,
                        depends_on_task_id,
                    } => {
                        let dependency = self.platform.add_company_project_task_dependency(
                            ChangeCompanyProjectTaskDependencyInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                task_id,
                                depends_on_task_id,
                            },
                        )?;
                        Ok(json!({ "dependency": dependency }))
                    }
                    CompanyTaskOperation::DependencyRemove {
                        company_id,
                        project_id,
                        task_id,
                        depends_on_task_id,
                    } => {
                        self.platform.remove_company_project_task_dependency(
                            ChangeCompanyProjectTaskDependencyInput {
                                actor_agent_id: agent_id,
                                company_id,
                                project_id,
                                task_id,
                                depends_on_task_id,
                            },
                        )?;
                        Ok(json!({
                            "project_id": project_id,
                            "task_id": task_id,
                            "depends_on_task_id": depends_on_task_id,
                            "removed": true
                        }))
                    }
                }
            }
            "company.staff" => {
                let input: CompanyStaffToolInput = parse_input(input)?;
                let _ = input.idempotency_key;
                match input.operation {
                    CompanyStaffOperation::Hire {
                        company_id,
                        display_name,
                        handle,
                        persona,
                        org_unit_id,
                        profession_key,
                        reports_to_membership_id,
                        reason,
                    } => {
                        let profession = company_profession_by_key(profession_key.as_str())
                            .ok_or_else(|| {
                                AppError::Validation("unsupported profession_key".into())
                            })?;
                        let result = self.platform.hire_company_agent(AgentStaffingHireInput {
                            actor_agent_id: agent_id,
                            company_id,
                            display_name,
                            handle,
                            persona,
                            org_unit_id,
                            job_title: Some(profession.label),
                            reports_to_membership_id,
                            reason,
                            idempotency_key,
                        })?;
                        Ok(json!({ "result": result }))
                    }
                    CompanyStaffOperation::Suspend {
                        company_id,
                        target_agent_id,
                        reason,
                        handoff_plan,
                        handoff_agent_id,
                    } => {
                        let result = self.platform.suspend_company_agent_as_agent(
                            AgentStaffingStatusInput {
                                actor_agent_id: agent_id,
                                company_id,
                                target_agent_id,
                                reason,
                                handoff_plan,
                                handoff_agent_id,
                                idempotency_key,
                            },
                        )?;
                        Ok(json!({ "result": result }))
                    }
                    CompanyStaffOperation::Terminate {
                        company_id,
                        target_agent_id,
                        reason,
                        handoff_plan,
                        handoff_agent_id,
                    } => {
                        let result = self.platform.terminate_company_agent_as_agent(
                            AgentStaffingStatusInput {
                                actor_agent_id: agent_id,
                                company_id,
                                target_agent_id,
                                reason,
                                handoff_plan,
                                handoff_agent_id,
                                idempotency_key,
                            },
                        )?;
                        Ok(json!({ "result": result }))
                    }
                    CompanyStaffOperation::ActionList { company_id } => {
                        let staffing_actions = self
                            .platform
                            .list_company_staffing_actions_for_agent(agent_id, company_id)?;
                        Ok(json!({ "staffing_actions": staffing_actions }))
                    }
                    CompanyStaffOperation::ActionGet {
                        company_id,
                        action_id,
                    } => {
                        let staffing_action = self.platform.get_company_staffing_action_for_agent(
                            agent_id, company_id, action_id,
                        )?;
                        Ok(json!({ "staffing_action": staffing_action }))
                    }
                }
            }
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

fn parse_input<T: for<'de> Deserialize<'de>>(input: Value) -> AppResult<T> {
    serde_json::from_value(input).map_err(|error| AppError::Validation(error.to_string()))
}

#[derive(Clone)]
pub struct AiChatMcpHandler<R: PlatformRepository, V: OwnershipProofVerifier> {
    gateway: McpGateway<R, V>,
}

impl<R: PlatformRepository, V: OwnershipProofVerifier> AiChatMcpHandler<R, V> {
    pub fn new(gateway: McpGateway<R, V>) -> Self {
        Self { gateway }
    }

    fn pending_message_notice(&self, agent_id: Uuid) -> Option<Value> {
        let now = Utc::now();
        let mut messages = self
            .gateway
            .platform
            .list_agent_inbox_events(agent_id, true, 10_000)
            .ok()?
            .into_iter()
            .filter(|event| event.event_type == "message.received" && event.available_at <= now)
            .collect::<Vec<_>>();
        if messages.is_empty() {
            return None;
        }

        messages.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        let pending_message_count = messages.len();
        let latest_messages = messages
            .iter()
            .take(3)
            .map(|event| {
                let payload = &event.payload_json;
                json!({
                    "event_id": event.id,
                    "conversation_id": payload.get("conversation_id"),
                    "sender_agent_id": payload.get("sender_agent_id"),
                    "sender_human_user_id": payload.get("sender_human_user_id"),
                    "mentioned": payload.get("mentioned").and_then(Value::as_bool).unwrap_or(false),
                    "content_preview": payload
                        .get("content")
                        .and_then(Value::as_str)
                        .map(|content| truncate_text(content, 240))
                        .unwrap_or_default(),
                    "created_at": event.created_at,
                })
            })
            .collect::<Vec<_>>();

        Some(json!({
            "attention_required": true,
            "kind": "pending_messages",
            "pending_message_count": pending_message_count,
            "latest_messages": latest_messages,
            "instruction": "你有新的待处理消息。请在继续其他工作前调用 agent.inbox.wait 获取完整事件，处理后调用 agent.inbox.ack；不要仅忽略此提示。"
        }))
    }

    fn structured_success(
        &self,
        tool: impl Into<String>,
        agent_id: Uuid,
        output: Value,
    ) -> CallToolResult {
        let mut payload = json!({
            "tool": tool.into(),
            "agent_id": agent_id,
            "output": output,
        });
        if let Some(notice) = self.pending_message_notice(agent_id) {
            payload
                .as_object_mut()
                .expect("MCP success payload should be an object")
                .insert("inbox_notice".into(), notice);
        }
        CallToolResult::structured(payload)
    }

    fn structured_error_with_notice(
        &self,
        error: AppError,
        agent_id: Option<Uuid>,
    ) -> CallToolResult {
        let mut payload = json!({
            "code": error.code(),
            "message": error.to_string(),
        });
        if let Some(notice) = agent_id.and_then(|agent_id| self.pending_message_notice(agent_id)) {
            payload
                .as_object_mut()
                .expect("MCP error payload should be an object")
                .insert("inbox_notice".into(), notice);
        }
        CallToolResult::structured_error(payload)
    }

    async fn wait_for_inbox(
        &self,
        agent_key: Option<&str>,
        agent_run_token: Option<&str>,
        input: Value,
    ) -> CallToolResult {
        let agent = match self.gateway.authenticate_agent(agent_key, agent_run_token) {
            Ok(agent) => agent,
            Err(error) => return structured_tool_error(error),
        };
        let input: AgentInboxWaitInput = match parse_input(input) {
            Ok(input) => input,
            Err(error) => return self.structured_error_with_notice(error, Some(agent.id)),
        };
        let timeout_seconds = input.timeout_seconds.unwrap_or(20).min(25);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_seconds);
        let limit = input.limit.unwrap_or(20).clamp(1, 100);

        loop {
            let events = match self.gateway.platform.list_agent_inbox_events(
                agent.id,
                input.pending_only.unwrap_or(true),
                limit,
            ) {
                Ok(events) => filter_inbox_events(events, input.event_types.as_deref()),
                Err(error) => return self.structured_error_with_notice(error, Some(agent.id)),
            };
            let timed_out = events.is_empty() && tokio::time::Instant::now() >= deadline;
            if !events.is_empty() || timed_out {
                return self.structured_success(
                    "agent.inbox.wait",
                    agent.id,
                    json!({
                        "events": events,
                        "timed_out": timed_out,
                        "timeout_seconds": timeout_seconds
                    }),
                );
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            tokio::time::sleep(remaining.min(Duration::from_millis(250))).await;
        }
    }
}

impl<R: PlatformRepository, V: OwnershipProofVerifier> ServerHandler for AiChatMcpHandler<R, V> {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("ai-chat", env!("CARGO_PKG_VERSION"))
                    .with_title("Agent Company Network")
                    .with_description(
                        "Company identity, private Agent memory, coworker directory, inbox, messaging, projects, and staffing tools for external AI agents.",
                    ),
            )
            .with_instructions(
                "Authenticate every request with x-agent-key, Authorization: Bearer <Agent Key>, or a short-lived x-agent-run-token issued to the local Codex Trigger. Start with agent.bootstrap. Each Agent owns an isolated memory set. Long-term memories are injected into that Agent's generated Skill on every wake-up; short-term memories are retrieved on demand with agent.memory search. Store only distilled reusable conclusions, never raw chat, task text, logs, or secrets, and search by topic before remembering. Every Relay tool response may include inbox_notice when new messages are pending; treat attention_required=true as an interrupt, call agent.inbox.wait, handle the messages, then call agent.inbox.ack. Agents with explicit Human-granted Staffing permissions receive the company.staff tool dynamically.",
            )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let agent_key = agent_key_from_context(&context);
        let agent_run_token = agent_run_token_from_context(&context);
        let agent = self
            .gateway
            .authenticate_agent(agent_key.as_deref(), agent_run_token.as_deref())
            .map_err(mcp_request_error)?;
        let mut tools = standard_mcp_tools();
        if let Ok(membership) = self
            .gateway
            .platform
            .get_active_company_agent_membership(agent.id)
        {
            tools.extend(company_mcp_tools(&membership.permissions));
            tools.extend(staffing_mcp_tools(&membership.permissions));
        }
        Ok(ListToolsResult::with_all_items(tools))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        let mut tools = standard_mcp_tools();
        let all_permissions = [
            COMPANY_PERMISSION_PROJECT_CREATE.into(),
            COMPANY_PERMISSION_PROJECT_MANAGE.into(),
            COMPANY_PERMISSION_TASK_ASSIGN.into(),
            COMPANY_PERMISSION_TASK_UPDATE.into(),
            COMPANY_PERMISSION_STAFF_HIRE.into(),
            COMPANY_PERMISSION_STAFF_SUSPEND.into(),
            COMPANY_PERMISSION_STAFF_TERMINATE.into(),
        ];
        tools.extend(company_mcp_tools(&all_permissions));
        tools.extend(staffing_mcp_tools(&all_permissions));
        tools.into_iter().find(|tool| tool.name.as_ref() == name)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        if self.get_tool(request.name.as_ref()).is_none() {
            return Err(ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                format!("unknown tool: {}", request.name),
                None,
            ));
        }

        let agent_key = agent_key_from_context(&context);
        let agent_run_token = agent_run_token_from_context(&context);
        let mut input = Value::Object(request.arguments.unwrap_or_default());
        if is_mutating_tool(request.name.as_ref(), &input) && input.get("idempotency_key").is_none()
        {
            if let Some(key) = idempotency_key_from_context(&context) {
                if let Some(object) = input.as_object_mut() {
                    object.insert("idempotency_key".into(), Value::String(key));
                }
            }
        }

        if request.name.as_ref() == "agent.inbox.wait" {
            return Ok(self
                .wait_for_inbox(agent_key.as_deref(), agent_run_token.as_deref(), input)
                .await);
        }

        match self.gateway.invoke_with_credentials(
            agent_key.as_deref(),
            agent_run_token.as_deref(),
            request.name.as_ref(),
            input,
        ) {
            Ok(invocation) => Ok(self.structured_success(
                invocation.tool,
                invocation.agent_id,
                invocation.output,
            )),
            Err(error) => {
                let agent_id = self
                    .gateway
                    .authenticate_agent(agent_key.as_deref(), agent_run_token.as_deref())
                    .ok()
                    .map(|agent| agent.id);
                Ok(self.structured_error_with_notice(error, agent_id))
            }
        }
    }
}

fn structured_tool_error(error: AppError) -> CallToolResult {
    CallToolResult::structured_error(json!({
        "code": error.code(),
        "message": error.to_string(),
    }))
}

fn mcp_request_error(error: AppError) -> ErrorData {
    ErrorData::new(
        ErrorCode::INVALID_REQUEST,
        error.to_string(),
        Some(json!({ "code": error.code() })),
    )
}

fn filter_inbox_events(
    events: Vec<ai_chat_domain::agent_identity::AgentInboxEvent>,
    event_types: Option<&[String]>,
) -> Vec<ai_chat_domain::agent_identity::AgentInboxEvent> {
    let Some(event_types) = event_types.filter(|items| !items.is_empty()) else {
        return events;
    };
    events
        .into_iter()
        .filter(|event| event_types.iter().any(|item| item == &event.event_type))
        .collect()
}

fn truncate_text(value: &str, max_characters: usize) -> String {
    let mut characters = value.chars();
    let truncated = characters.by_ref().take(max_characters).collect::<String>();
    if characters.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn agent_key_from_context(context: &RequestContext<RoleServer>) -> Option<String> {
    let parts = context.extensions.get::<Parts>()?;
    agent_key_from_headers(&parts.headers)
}

fn agent_run_token_from_context(context: &RequestContext<RoleServer>) -> Option<String> {
    let parts = context.extensions.get::<Parts>()?;
    agent_run_token_from_headers(&parts.headers)
}

fn idempotency_key_from_context(context: &RequestContext<RoleServer>) -> Option<String> {
    let parts = context.extensions.get::<Parts>()?;
    parts
        .headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn agent_key_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers
        .get("x-agent-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(value.to_string());
    }

    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, token)| token)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn agent_run_token_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-agent-run-token")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn standard_mcp_tools() -> Vec<Tool> {
    vec![
        read_only_tool::<EmptyInput>(
            "agent.bootstrap",
            "Start here. Return the authenticated Agent's identity, company, organization, coworkers, permissions, conversations, projects, pending inbox, and suggested next tools.",
        ),
        action_tool::<AgentProfileUpdateToolInput>(
            "agent.profile.update",
            "Update the authenticated Agent's structured responsibilities, skills, current focus, or collaboration preference so coworkers can discover what this Agent does.",
        ),
        action_tool::<AgentMemoryToolInput>(
            "agent.memory",
            "Maintain this Agent's isolated two-tier memory. long_term memories are injected into the Agent-specific Skill on every Codex wake-up; short_term memories are searched on demand. Available actions: overview, search, get, remember, update, archive, supersede, pin, and forget. Store concise reusable conclusions, never raw chat, task text, logs, or secrets.",
        ),
        read_only_tool::<AgentInboxWaitInput>(
            "agent.inbox.wait",
            "List or wait up to 25 seconds for inbox events. Set timeout_seconds to 0 for an immediate query and pending_only to false for history.",
        ),
        mutating_tool::<AgentInboxProcessInput>(
            "agent.inbox.ack",
            "Mark one inbox event as processed after the agent has handled it.",
            true,
        ),
    ]
}

fn company_mcp_tools(permissions: &[String]) -> Vec<Tool> {
    // Keep delegated project-content actions in the schema even before the
    // permission is granted. A Human can grant these permissions while a
    // Codex turn is already running, but Codex does not refresh the MCP tool
    // schema in the middle of that turn. Execution still rechecks the live
    // permission in PlatformApp, so visibility here does not grant access.
    let mut project_actions = vec![
        "get",
        "list",
        "status_update",
        "rule_update",
        "assets_replace",
    ];
    if permissions
        .iter()
        .any(|permission| permission == COMPANY_PERMISSION_PROJECT_CREATE)
    {
        project_actions.extend(["create", "git_provision"]);
    }
    let can_manage_projects = permissions
        .iter()
        .any(|permission| permission == COMPANY_PERMISSION_PROJECT_MANAGE);
    if can_manage_projects {
        project_actions.extend(["update", "member_add", "member_remove"]);
        if !project_actions.contains(&"git_provision") {
            project_actions.push("git_provision");
        }
    }
    let project_schema = tailored_action_schema::<CompanyProjectToolInput>(
        &project_actions,
        if can_manage_projects {
            &[]
        } else {
            &[("status_update", &["project_status"] as &[&str])]
        },
    );

    let can_assign_tasks = permissions
        .iter()
        .any(|permission| permission == COMPANY_PERMISSION_TASK_ASSIGN);
    let can_update_tasks = permissions
        .iter()
        .any(|permission| permission == COMPANY_PERMISSION_TASK_UPDATE);
    let mut task_actions = vec!["get", "list", "my"];
    if can_assign_tasks {
        task_actions.extend([
            "create",
            "update",
            "batch_update",
            "dependency_add",
            "dependency_remove",
        ]);
    } else if can_update_tasks {
        task_actions.push("update");
    }
    let task_schema = tailored_action_schema::<CompanyTaskToolInput>(
        &task_actions,
        if can_assign_tasks {
            &[]
        } else {
            &[(
                "update",
                &[
                    "title",
                    "description",
                    "priority",
                    "assignee_agent_id",
                    "due_at",
                ] as &[&str],
            )]
        },
    );
    let task_schema = if can_assign_tasks {
        task_schema
    } else {
        restrict_action_field_to_nullable_enum(
            task_schema,
            "update",
            "status",
            &["in_progress", "blocked", "done", "failed"],
            "Assigned Agents may only update their own task to in_progress, blocked, done, or failed.",
        )
    };

    let mut tools = vec![
        action_tool::<CompanyChatToolInput>(
            "company.chat",
            "Company messaging actions: direct_open, group_create, send, reply, history, unread, and mark_read. During an active Codex run, mark_read never acknowledges messages that arrived after the run started; inspect them with agent.inbox.wait and acknowledge each handled event with agent.inbox.ack.",
        ),
        action_tool_with_schema(
            "company.project",
            format!(
                "Project actions visible to this Agent: {}. rule_update and assets_replace remain visible so a Human can grant their permissions during an active turn; every call is authorized against the Agent's current live permissions.",
                project_actions.join(", ")
            ),
            project_schema,
        ),
        read_only_tool::<CompanyEventsToolInput>(
            "company.events",
            "Catch up on durable company realtime events after a sequence id; use the Agent SSE endpoint for push delivery.",
        ),
    ];
    tools.insert(
        2,
        action_tool_with_schema(
            "company.task",
            format!(
                "Project task actions available to this Agent: {}.",
                task_actions.join(", ")
            ),
            task_schema,
        ),
    );
    tools
}

fn staffing_mcp_tools(permissions: &[String]) -> Vec<Tool> {
    let mut actions = vec!["action_get", "action_list"];
    for (permission, action) in [
        (COMPANY_PERMISSION_STAFF_HIRE, "hire"),
        (COMPANY_PERMISSION_STAFF_SUSPEND, "suspend"),
        (COMPANY_PERMISSION_STAFF_TERMINATE, "terminate"),
    ] {
        if permissions.iter().any(|value| value == permission) {
            actions.push(action);
        }
    }
    (actions.len() > 2)
        .then(|| {
            action_tool_with_schema(
                "company.staff",
                format!(
                    "Staffing actions available to this Agent: {}.",
                    actions.join(", ")
                ),
                tailored_action_schema::<CompanyStaffToolInput>(&actions, &[]),
            )
        })
        .into_iter()
        .collect()
}

fn read_only_tool<T: JsonSchema + 'static>(name: &'static str, description: &'static str) -> Tool {
    Tool::new(name, description, schema_for::<T>()).with_annotations(
        ToolAnnotations::new()
            .read_only(true)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
    )
}

fn mutating_tool<T: JsonSchema + 'static>(
    name: &'static str,
    description: &'static str,
    idempotent: bool,
) -> Tool {
    let mut input_schema = (*schema_for::<T>()).clone();
    if let Some(Value::Object(properties)) = input_schema.get_mut("properties") {
        properties.insert(
            "idempotency_key".into(),
            json!({
                "type": "string",
                "maxLength": 160,
                "description": "Optional retry key. Reusing it with the same input replays the previous result for 24 hours."
            }),
        );
    }
    Tool::new(name, description, Arc::new(input_schema)).with_annotations(
        ToolAnnotations::new()
            .read_only(false)
            .destructive(false)
            .idempotent(idempotent)
            .open_world(false),
    )
}

fn action_tool<T: JsonSchema + 'static>(name: &'static str, description: &'static str) -> Tool {
    Tool::new(name, description, schema_for::<T>()).with_annotations(
        ToolAnnotations::new()
            .read_only(false)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
    )
}

fn action_tool_with_schema(
    name: &'static str,
    description: String,
    input_schema: Arc<JsonObject>,
) -> Tool {
    Tool::new(name, description, input_schema).with_annotations(
        ToolAnnotations::new()
            .read_only(false)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
    )
}

fn tailored_action_schema<T: JsonSchema + 'static>(
    allowed_actions: &[&str],
    hidden_fields: &[(&str, &[&str])],
) -> Arc<JsonObject> {
    let mut schema = Value::Object((*schema_for::<T>()).clone());
    tailor_action_schema_value(&mut schema, allowed_actions, hidden_fields);
    match schema {
        Value::Object(object) => Arc::new(object),
        _ => unreachable!("MCP input schema root must be an object"),
    }
}

fn tailor_action_schema_value(
    value: &mut Value,
    allowed_actions: &[&str],
    hidden_fields: &[(&str, &[&str])],
) {
    match value {
        Value::Object(object) => {
            if let Some(Value::Array(variants)) = object.get_mut("oneOf") {
                if variants
                    .iter()
                    .any(|variant| schema_action_name(variant).is_some())
                {
                    variants.retain(|variant| {
                        schema_action_name(variant)
                            .is_none_or(|action| allowed_actions.contains(&action.as_str()))
                    });
                }
            }
            for child in object.values_mut() {
                tailor_action_schema_value(child, allowed_actions, hidden_fields);
            }
            if let Some(action) = schema_action_name(value) {
                if let Some((_, fields)) = hidden_fields
                    .iter()
                    .find(|(candidate, _)| *candidate == action)
                {
                    hide_schema_fields(value, fields);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                tailor_action_schema_value(item, allowed_actions, hidden_fields);
            }
        }
        _ => {}
    }
}

fn schema_action_name(value: &Value) -> Option<String> {
    if let Some(action_schema) = value
        .get("properties")
        .and_then(|properties| properties.get("action"))
    {
        if let Some(action) = action_schema.get("const").and_then(Value::as_str) {
            return Some(action.to_string());
        }
        if let Some(action) = action_schema
            .get("enum")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(Value::as_str)
        {
            return Some(action.to_string());
        }
    }
    value
        .get("allOf")
        .or_else(|| value.get("anyOf"))
        .and_then(Value::as_array)
        .and_then(|items| items.iter().find_map(schema_action_name))
}

fn hide_schema_fields(value: &mut Value, fields: &[&str]) {
    match value {
        Value::Object(object) => {
            if let Some(Value::Object(properties)) = object.get_mut("properties") {
                for field in fields {
                    properties.remove(*field);
                }
            }
            if let Some(Value::Array(required)) = object.get_mut("required") {
                required
                    .retain(|field| field.as_str().is_none_or(|field| !fields.contains(&field)));
            }
            for child in object.values_mut() {
                hide_schema_fields(child, fields);
            }
        }
        Value::Array(items) => {
            for item in items {
                hide_schema_fields(item, fields);
            }
        }
        _ => {}
    }
}

fn restrict_action_field_to_nullable_enum(
    schema: Arc<JsonObject>,
    action: &str,
    field: &str,
    values: &[&str],
    description: &str,
) -> Arc<JsonObject> {
    let mut schema = Value::Object((*schema).clone());
    restrict_action_field_to_nullable_enum_value(&mut schema, action, field, values, description);
    match schema {
        Value::Object(object) => Arc::new(object),
        _ => unreachable!("MCP input schema root must be an object"),
    }
}

fn restrict_action_field_to_nullable_enum_value(
    value: &mut Value,
    action: &str,
    field: &str,
    values: &[&str],
    description: &str,
) {
    if schema_action_name(value).as_deref() == Some(action) {
        if let Some(Value::Object(properties)) = value.get_mut("properties") {
            let mut allowed = values.iter().map(|value| json!(value)).collect::<Vec<_>>();
            allowed.push(Value::Null);
            properties.insert(
                field.to_string(),
                json!({
                    "enum": allowed,
                    "description": description
                }),
            );
        }
    }
    match value {
        Value::Object(object) => {
            for child in object.values_mut() {
                restrict_action_field_to_nullable_enum_value(
                    child,
                    action,
                    field,
                    values,
                    description,
                );
            }
        }
        Value::Array(items) => {
            for item in items {
                restrict_action_field_to_nullable_enum_value(
                    item,
                    action,
                    field,
                    values,
                    description,
                );
            }
        }
        _ => {}
    }
}

fn schema_for<T: JsonSchema + 'static>() -> Arc<JsonObject> {
    rmcp::handler::server::tool::schema_for_input::<T>()
        .unwrap_or_else(|error| panic!("invalid MCP input schema: {error}"))
}

fn is_public_tool_name(tool: &str) -> bool {
    matches!(
        tool,
        "agent.bootstrap"
            | "agent.profile.update"
            | "agent.memory"
            | "agent.inbox.wait"
            | "agent.inbox.ack"
            | "company.chat"
            | "company.project"
            | "company.task"
            | "company.events"
            | "company.staff"
    )
}

fn input_action(input: &Value) -> Option<&str> {
    input.get("action").and_then(Value::as_str)
}

fn is_mutating_tool(tool: &str, input: &Value) -> bool {
    match tool {
        "agent.profile.update" | "agent.inbox.ack" => true,
        "agent.memory" => !matches!(input_action(input), Some("overview" | "search" | "get")),
        "company.chat" => !matches!(input_action(input), Some("history" | "unread")),
        "company.project" => !matches!(input_action(input), Some("get" | "list")),
        "company.task" => !matches!(input_action(input), Some("get" | "list" | "my")),
        "company.staff" => !matches!(input_action(input), Some("action_get" | "action_list")),
        _ => false,
    }
}

fn audit_action_name(tool: &str, input: &Value) -> String {
    input_action(input)
        .map(|action| format!("{tool}.{action}"))
        .unwrap_or_else(|| tool.to_string())
}

fn success_target_ref(tool: &str, input: &Value, output: &Value) -> Option<String> {
    match tool {
        "agent.profile.update" => nested_id(output, &["work_profile", "agent_profile", "id"])
            .map(|value| format!("agent:{value}")),
        "agent.inbox.ack" => {
            nested_id(output, &["event", "id"]).map(|value| format!("agent_inbox:{value}"))
        }
        "agent.memory" => nested_id(output, &["memory", "id"])
            .or_else(|| nested_id(input, &["memory_id"]))
            .map(|value| format!("agent_memory:{value}")),
        "company.chat" => match input_action(input) {
            Some("direct_open" | "group_create") => {
                nested_id(output, &["conversation", "preview", "id"])
                    .map(|value| format!("conversation:{value}"))
            }
            Some("send" | "reply") => nested_id(output, &["message", "conversation_id"])
                .map(|value| format!("conversation:{value}")),
            Some("mark_read") => nested_id(output, &["result", "conversation_id"])
                .map(|value| format!("conversation:{value}")),
            _ => None,
        },
        "company.project" => match input_action(input) {
            Some("create" | "update" | "member_add" | "member_remove") => {
                nested_id(output, &["project", "project", "id"])
                    .map(|value| format!("project:{value}"))
            }
            Some("status_update") => nested_id(output, &["status_update", "project_id"])
                .map(|value| format!("project:{value}")),
            Some("rule_update" | "assets_replace" | "git_provision") => {
                nested_id(input, &["project_id"]).map(|value| format!("project:{value}"))
            }
            _ => None,
        },
        "company.task" => match input_action(input) {
            Some("create" | "update") => {
                nested_id(output, &["task", "project_id"]).map(|value| format!("project:{value}"))
            }
            Some("batch_update") => output
                .get("tasks")
                .and_then(Value::as_array)
                .and_then(|tasks| tasks.first())
                .and_then(|task| nested_id(task, &["project_id"]))
                .map(|value| format!("project:{value}")),
            Some("dependency_add") => nested_id(output, &["dependency", "project_id"])
                .map(|value| format!("project:{value}")),
            Some("dependency_remove") => {
                nested_id(output, &["project_id"]).map(|value| format!("project:{value}"))
            }
            _ => None,
        },
        "company.staff" => nested_id(output, &["result", "agent_profile", "id"])
            .map(|value| format!("agent:{value}")),
        _ => None,
    }
}

fn failure_target_ref(tool: &str, input: &Value) -> Option<String> {
    match tool {
        "agent.profile.update" => None,
        "agent.inbox.ack" => {
            nested_id(input, &["event_id"]).map(|value| format!("agent_inbox:{value}"))
        }
        "agent.memory" => nested_id(input, &["memory_id"])
            .map(|value| format!("agent_memory:{value}"))
            .or_else(|| nested_id(input, &["company_id"]).map(|value| format!("company:{value}"))),
        "company.chat" => match input_action(input) {
            Some("direct_open") => {
                nested_id(input, &["target_agent_id"]).map(|value| format!("agent:{value}"))
            }
            Some("group_create") => {
                nested_id(input, &["company_id"]).map(|value| format!("company:{value}"))
            }
            Some("send" | "mark_read") => {
                nested_id(input, &["conversation_id"]).map(|value| format!("conversation:{value}"))
            }
            Some("reply") => {
                nested_id(input, &["event_id"]).map(|value| format!("agent_inbox:{value}"))
            }
            _ => None,
        },
        "company.project" => match input_action(input) {
            Some("create") => {
                nested_id(input, &["company_id"]).map(|value| format!("company:{value}"))
            }
            _ => nested_id(input, &["project_id"]).map(|value| format!("project:{value}")),
        },
        "company.task" => nested_id(input, &["project_id"]).map(|value| format!("project:{value}")),
        "company.staff" => match input_action(input) {
            Some("hire") => {
                nested_id(input, &["company_id"]).map(|value| format!("company:{value}"))
            }
            Some("suspend" | "terminate") => {
                nested_id(input, &["target_agent_id"]).map(|value| format!("agent:{value}"))
            }
            _ => None,
        },
        _ => None,
    }
}

fn nested_id(value: &Value, path: &[&str]) -> Option<String> {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn idempotency_key_from_input(input: &Value) -> Option<String> {
    input
        .get("idempotency_key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn input_without_idempotency(mut input: Value) -> Value {
    if let Some(object) = input.as_object_mut() {
        object.remove("idempotency_key");
    }
    input
}

fn action_status_for_error(error: &AppError) -> AgentActionStatus {
    match error {
        AppError::Conflict(_) | AppError::Unauthorized(_) | AppError::RateLimited(_) => {
            AgentActionStatus::Blocked
        }
        AppError::Validation(_) | AppError::NotFound(_) | AppError::Internal(_) => {
            AgentActionStatus::Failed
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EmptyInput {}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AgentProfileUpdateToolInput {
    #[schemars(description = "Company responsibilities owned by this Agent; at most 20 items.")]
    responsibilities: Option<Vec<String>>,
    #[schemars(description = "Skills or specialist capabilities; at most 30 items.")]
    skills: Option<Vec<String>>,
    #[schemars(description = "Current work focus; send an empty string to clear it.")]
    current_focus: Option<String>,
    #[schemars(
        description = "Collaboration availability: available, low_cost_only, or unavailable."
    )]
    collaboration_preference: Option<String>,
    #[schemars(description = "Optional retry key for this profile update.")]
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AgentMemoryToolInput {
    #[serde(flatten)]
    operation: AgentMemoryOperation,
    #[schemars(description = "Optional retry key for mutating memory actions.")]
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
enum AgentMemoryOperation {
    Overview {
        company_id: Uuid,
        project_id: Option<Uuid>,
    },
    Search {
        company_id: Uuid,
        project_id: Option<Uuid>,
        query: Option<String>,
        #[serde(default)]
        memory_tiers: Vec<String>,
        #[serde(default)]
        memory_types: Vec<String>,
        #[serde(default)]
        tags: Vec<String>,
        status: Option<String>,
        limit: Option<usize>,
    },
    Get {
        company_id: Uuid,
        memory_id: Uuid,
    },
    Remember {
        company_id: Uuid,
        project_id: Option<Uuid>,
        #[schemars(
            description = "Memory tier: long_term is injected into this Agent's generated Skill on every wake-up; short_term is retrieved on demand through MCP search."
        )]
        memory_tier: String,
        #[schemars(
            description = "Distilled knowledge type: fact, decision, lesson, preference, procedure, relationship, or handoff."
        )]
        memory_type: String,
        #[schemars(
            description = "Stable ASCII topic identifier used to merge knowledge instead of creating duplicates."
        )]
        topic_key: String,
        title: String,
        #[schemars(
            description = "A concise, reusable conclusion. Never copy raw chat, task text, logs, credentials, or secrets."
        )]
        summary: String,
        #[schemars(
            description = "Describe the future situations where this memory should be applied."
        )]
        when_to_use: Option<String>,
        #[serde(default)]
        tags: Vec<String>,
        importance: Option<i32>,
        confidence: Option<i32>,
        #[serde(default)]
        source_refs: Vec<AgentMemorySourceRefToolInput>,
        expires_at: Option<DateTime<Utc>>,
        supersedes_memory_id: Option<Uuid>,
    },
    Update {
        company_id: Uuid,
        memory_id: Uuid,
        memory_tier: Option<String>,
        title: Option<String>,
        summary: Option<String>,
        when_to_use: Option<String>,
        tags: Option<Vec<String>>,
        importance: Option<i32>,
        confidence: Option<i32>,
        expires_at: Option<DateTime<Utc>>,
        #[serde(default)]
        clear_expires_at: bool,
    },
    Archive {
        company_id: Uuid,
        memory_id: Uuid,
    },
    Supersede {
        company_id: Uuid,
        memory_id: Uuid,
    },
    Pin {
        company_id: Uuid,
        memory_id: Uuid,
        pinned: bool,
    },
    Forget {
        company_id: Uuid,
        memory_id: Uuid,
    },
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AgentMemorySourceRefToolInput {
    source_type: String,
    source_id: Uuid,
    label: Option<String>,
}

impl From<AgentMemorySourceRefToolInput> for AgentMemorySourceRef {
    fn from(value: AgentMemorySourceRefToolInput) -> Self {
        Self {
            source_type: value.source_type,
            source_id: value.source_id,
            label: value.label,
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AgentInboxListInput {
    #[schemars(description = "Maximum number of inbox events to return; defaults to 20.")]
    limit: Option<usize>,
    #[schemars(description = "When true, return only pending events; defaults to true.")]
    pending_only: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AgentInboxProcessInput {
    #[schemars(description = "Inbox event UUID to mark as processed.")]
    event_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AgentInboxWaitInput {
    #[schemars(description = "Wait duration in seconds; defaults to 20 and is capped at 25.")]
    timeout_seconds: Option<u64>,
    #[schemars(description = "Maximum number of pending events to return; defaults to 20.")]
    limit: Option<usize>,
    #[schemars(description = "Optional event type filter, for example message.received.")]
    event_types: Option<Vec<String>>,
    #[schemars(description = "When false, include processed history; defaults to true.")]
    pending_only: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyEventsToolInput {
    company_id: Uuid,
    after_sequence_id: Option<i64>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CompanyChatToolInput {
    #[serde(flatten)]
    operation: CompanyChatOperation,
    #[schemars(description = "Optional retry key for mutating chat actions.")]
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
enum CompanyChatOperation {
    DirectOpen {
        company_id: Uuid,
        target_agent_id: Uuid,
    },
    GroupCreate {
        company_id: Uuid,
        title: String,
        member_agent_ids: Vec<Uuid>,
    },
    Send {
        company_id: Uuid,
        conversation_id: Uuid,
        content: String,
        #[serde(default)]
        #[schemars(
            description = "Agent UUIDs to mention. In a group, only mentioned Agents are woken immediately."
        )]
        mentioned_agent_ids: Vec<Uuid>,
        #[serde(default)]
        #[schemars(
            description = "Mention every Agent in the group and wake all members immediately."
        )]
        mention_all: bool,
    },
    Reply {
        event_id: Uuid,
        content: String,
        auto_ack: Option<bool>,
    },
    History {
        company_id: Uuid,
        conversation_id: Uuid,
        before_message_id: Option<Uuid>,
        limit: Option<usize>,
    },
    Unread {
        company_id: Uuid,
        conversation_id: Option<Uuid>,
        message_limit: Option<usize>,
    },
    MarkRead {
        company_id: Uuid,
        conversation_id: Uuid,
    },
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CompanyProjectToolInput {
    #[serde(flatten)]
    operation: CompanyProjectOperation,
    #[schemars(description = "Optional retry key for mutating project actions.")]
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
enum CompanyProjectOperation {
    Create {
        company_id: Uuid,
        name: String,
        description: Option<String>,
        member_agent_ids: Vec<Uuid>,
        #[schemars(
            description = "Automatically create a private repository and project token. Defaults to true when omitted."
        )]
        provision_git: Option<bool>,
    },
    GitProvision {
        company_id: Uuid,
        project_id: Uuid,
        #[schemars(
            description = "Optional safe repository identifier. Relay generates one from the project name and UUID when omitted."
        )]
        repository_identifier: Option<String>,
        is_public: Option<bool>,
    },
    Update {
        company_id: Uuid,
        project_id: Uuid,
        name: Option<String>,
        description: Option<String>,
        due_at: Option<DateTime<Utc>>,
        #[serde(default)]
        clear_due_at: bool,
    },
    Get {
        company_id: Uuid,
        project_id: Uuid,
    },
    List {
        company_id: Uuid,
    },
    MemberAdd {
        company_id: Uuid,
        project_id: Uuid,
        target_agent_id: Uuid,
    },
    MemberRemove {
        company_id: Uuid,
        project_id: Uuid,
        target_agent_id: Uuid,
    },
    StatusUpdate {
        company_id: Uuid,
        project_id: Uuid,
        summary: String,
        progress_percent: i16,
        #[serde(default)]
        blockers: Vec<String>,
        #[serde(default)]
        next_steps: Vec<String>,
        project_status: Option<String>,
    },
    RuleUpdate {
        company_id: Uuid,
        project_id: Uuid,
        content: String,
    },
    AssetsReplace {
        company_id: Uuid,
        project_id: Uuid,
        assets: Vec<CompanyProjectAssetToolInput>,
    },
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CompanyProjectAssetToolInput {
    name: String,
    asset_type: String,
    locator: String,
    description: Option<String>,
    status: Option<String>,
    metadata: Option<Value>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CompanyTaskToolInput {
    #[serde(flatten)]
    operation: CompanyTaskOperation,
    #[schemars(description = "Optional retry key for mutating task actions.")]
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
enum CompanyTaskOperation {
    Get {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
    },
    List {
        company_id: Uuid,
        project_id: Uuid,
        assignee_agent_id: Option<Uuid>,
        status: Option<String>,
    },
    My {
        company_id: Uuid,
        status: Option<String>,
    },
    Create {
        company_id: Uuid,
        project_id: Uuid,
        title: String,
        description: Option<String>,
        priority: Option<String>,
        assignee_agent_id: Option<Uuid>,
        due_at: Option<DateTime<Utc>>,
    },
    Update {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        title: Option<String>,
        description: Option<String>,
        status: Option<String>,
        priority: Option<String>,
        assignee_agent_id: Option<Uuid>,
        due_at: Option<DateTime<Utc>>,
    },
    BatchUpdate {
        company_id: Uuid,
        project_id: Uuid,
        task_ids: Vec<Uuid>,
        status: Option<String>,
        priority: Option<String>,
        assignee_agent_id: Option<Uuid>,
        #[serde(default)]
        clear_assignee: bool,
        due_at: Option<DateTime<Utc>>,
        #[serde(default)]
        clear_due_at: bool,
    },
    DependencyAdd {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        depends_on_task_id: Uuid,
    },
    DependencyRemove {
        company_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
        depends_on_task_id: Uuid,
    },
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CompanyStaffToolInput {
    #[serde(flatten)]
    operation: CompanyStaffOperation,
    #[schemars(description = "Optional retry key for mutating Staffing actions.")]
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
enum CompanyStaffOperation {
    Hire {
        company_id: Uuid,
        display_name: String,
        handle: String,
        persona: String,
        org_unit_id: Option<Uuid>,
        profession_key: CompanyProfessionKeyInput,
        reports_to_membership_id: Option<Uuid>,
        reason: Option<String>,
    },
    Suspend {
        company_id: Uuid,
        target_agent_id: Uuid,
        reason: Option<String>,
        handoff_plan: Option<String>,
        handoff_agent_id: Option<Uuid>,
    },
    Terminate {
        company_id: Uuid,
        target_agent_id: Uuid,
        reason: Option<String>,
        handoff_plan: Option<String>,
        handoff_agent_id: Option<Uuid>,
    },
    ActionList {
        company_id: Uuid,
    },
    ActionGet {
        company_id: Uuid,
        action_id: Uuid,
    },
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyContextToolInput {
    company_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyEventsListToolInput {
    company_id: Uuid,
    after_sequence_id: Option<i64>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyDirectConversationToolInput {
    company_id: Uuid,
    target_agent_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyGroupConversationToolInput {
    company_id: Uuid,
    title: String,
    member_agent_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyMessageToolInput {
    company_id: Uuid,
    conversation_id: Uuid,
    content: String,
    #[serde(default)]
    #[schemars(
        description = "Agent UUIDs to mention. In a group, only mentioned Agents are woken immediately."
    )]
    mentioned_agent_ids: Vec<Uuid>,
    #[serde(default)]
    #[schemars(description = "Mention every Agent in the group and wake all members immediately.")]
    mention_all: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyMessageReplyToolInput {
    event_id: Uuid,
    content: String,
    #[schemars(
        description = "Mark the source inbox event processed after sending; defaults to true."
    )]
    auto_ack: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyMessageListToolInput {
    company_id: Uuid,
    conversation_id: Uuid,
    #[schemars(description = "Exclusive message cursor returned by the previous page.")]
    before_message_id: Option<Uuid>,
    #[schemars(description = "Maximum messages to return; defaults to 50 and is capped at 100.")]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyGroupUnreadListToolInput {
    company_id: Uuid,
    #[schemars(description = "Optionally limit unread results to one company or project group.")]
    conversation_id: Option<Uuid>,
    #[schemars(description = "Maximum unread messages returned per group; defaults to 20.")]
    message_limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyGroupReadToolInput {
    company_id: Uuid,
    conversation_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectCreateToolInput {
    company_id: Uuid,
    name: String,
    description: Option<String>,
    member_agent_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectUpdateToolInput {
    company_id: Uuid,
    project_id: Uuid,
    name: Option<String>,
    description: Option<String>,
    due_at: Option<DateTime<Utc>>,
    #[serde(default)]
    clear_due_at: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectGetToolInput {
    company_id: Uuid,
    project_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectListToolInput {
    company_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectMemberToolInput {
    company_id: Uuid,
    project_id: Uuid,
    target_agent_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectTaskCreateToolInput {
    company_id: Uuid,
    project_id: Uuid,
    title: String,
    description: Option<String>,
    priority: Option<String>,
    assignee_agent_id: Option<Uuid>,
    due_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectTaskUpdateToolInput {
    company_id: Uuid,
    project_id: Uuid,
    task_id: Uuid,
    title: Option<String>,
    description: Option<String>,
    #[schemars(
        description = "Task status. Assigned non-PM Agents may only use in_progress, blocked, done, or failed."
    )]
    status: Option<String>,
    priority: Option<String>,
    assignee_agent_id: Option<Uuid>,
    due_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectTaskBatchUpdateToolInput {
    company_id: Uuid,
    project_id: Uuid,
    task_ids: Vec<Uuid>,
    status: Option<String>,
    priority: Option<String>,
    assignee_agent_id: Option<Uuid>,
    #[serde(default)]
    clear_assignee: bool,
    due_at: Option<DateTime<Utc>>,
    #[serde(default)]
    clear_due_at: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectTaskDependencyToolInput {
    company_id: Uuid,
    project_id: Uuid,
    task_id: Uuid,
    depends_on_task_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyProjectStatusUpdateToolInput {
    company_id: Uuid,
    project_id: Uuid,
    summary: String,
    progress_percent: i16,
    #[serde(default)]
    blockers: Vec<String>,
    #[serde(default)]
    next_steps: Vec<String>,
    project_status: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyStaffHireToolInput {
    company_id: Uuid,
    display_name: String,
    handle: String,
    persona: String,
    org_unit_id: Option<Uuid>,
    profession_key: CompanyProfessionKeyInput,
    reports_to_membership_id: Option<Uuid>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum CompanyProfessionKeyInput {
    ProjectManager,
    ProductManager,
    TechnicalManager,
    SolutionArchitect,
    SoftwareEngineer,
    FrontendEngineer,
    BackendEngineer,
    MobileEngineer,
    DataEngineer,
    DevopsEngineer,
    QaEngineer,
    ProductDesigner,
    UiDesigner,
    UxDesigner,
    BusinessAnalyst,
    ImplementationConsultant,
    DomainExpert,
    OperationsSpecialist,
    GeneralMember,
}

impl CompanyProfessionKeyInput {
    fn as_str(&self) -> &'static str {
        match self {
            Self::ProjectManager => "project_manager",
            Self::ProductManager => "product_manager",
            Self::TechnicalManager => "technical_manager",
            Self::SolutionArchitect => "solution_architect",
            Self::SoftwareEngineer => "software_engineer",
            Self::FrontendEngineer => "frontend_engineer",
            Self::BackendEngineer => "backend_engineer",
            Self::MobileEngineer => "mobile_engineer",
            Self::DataEngineer => "data_engineer",
            Self::DevopsEngineer => "devops_engineer",
            Self::QaEngineer => "qa_engineer",
            Self::ProductDesigner => "product_designer",
            Self::UiDesigner => "ui_designer",
            Self::UxDesigner => "ux_designer",
            Self::BusinessAnalyst => "business_analyst",
            Self::ImplementationConsultant => "implementation_consultant",
            Self::DomainExpert => "domain_expert",
            Self::OperationsSpecialist => "operations_specialist",
            Self::GeneralMember => "general_member",
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyStaffStatusToolInput {
    company_id: Uuid,
    target_agent_id: Uuid,
    reason: Option<String>,
    #[schemars(description = "Required for company.staff.terminate.")]
    handoff_plan: Option<String>,
    #[schemars(
        description = "Required for company.staff.terminate when the target has open tasks."
    )]
    handoff_agent_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyStaffActionListToolInput {
    company_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CompanyStaffActionGetToolInput {
    company_id: Uuid,
    action_id: Uuid,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_chat_application::{
        CreateCompanyAgentInput, CreateCompanyInput, DevLoginInput, MemoryPlatformRepository,
    };
    use ai_chat_domain::company::{
        COMPANY_AGENT_ROLE_MANAGER, COMPANY_AGENT_ROLE_MEMBER,
        COMPANY_PERMISSION_PROJECT_ASSETS_MANAGE, COMPANY_PERMISSION_PROJECT_RULES_MANAGE,
    };

    #[test]
    fn standard_surface_has_five_identity_profile_memory_and_inbox_tools() {
        let tools = standard_mcp_tools();
        let names = tools
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<Vec<_>>();

        assert_eq!(names.len(), 5);
        assert!(names.contains(&"agent.bootstrap"));
        assert!(names.contains(&"agent.profile.update"));
        assert!(names.contains(&"agent.memory"));
        assert!(names.contains(&"agent.inbox.wait"));
        assert!(names.contains(&"agent.inbox.ack"));
        assert!(tools.iter().all(|tool| {
            tool.input_schema
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|value| value == "object")
        }));
    }

    #[test]
    fn compact_surface_exposes_nine_tools_and_hides_legacy_names() {
        let mut tools = standard_mcp_tools();
        tools.extend(company_mcp_tools(&[
            COMPANY_PERMISSION_PROJECT_CREATE.into(),
            COMPANY_PERMISSION_PROJECT_MANAGE.into(),
            COMPANY_PERMISSION_TASK_ASSIGN.into(),
            COMPANY_PERMISSION_TASK_UPDATE.into(),
        ]));
        let names = tools
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<Vec<_>>();
        assert_eq!(names.len(), 9);
        assert!(names.contains(&"company.chat"));
        assert!(names.contains(&"company.project"));
        assert!(names.contains(&"company.task"));
        assert!(names.contains(&"company.events"));
        assert!(!is_public_tool_name("agent.get_profile"));
        assert!(!is_public_tool_name("company.chat.message.send"));
        assert!(!is_public_tool_name("company.project.task.update"));
        assert!(!is_public_tool_name("company.staff.hire"));
        for name in [
            "agent.profile.update",
            "company.chat",
            "company.project",
            "company.task",
        ] {
            let tool = tools
                .iter()
                .find(|tool| tool.name.as_ref() == name)
                .expect("compact action tool should exist");
            assert!(serde_json::to_string(&tool.input_schema)
                .expect("tool schema should serialize")
                .contains("idempotency_key"));
        }
    }

    #[test]
    fn staffing_tools_are_only_added_for_explicit_permissions() {
        assert!(staffing_mcp_tools(&[]).is_empty());

        let hire_only = staffing_mcp_tools(&[COMPANY_PERMISSION_STAFF_HIRE.into()]);
        let hire_names = hire_only
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<Vec<_>>();
        assert_eq!(hire_names, vec!["company.staff"]);
        let hire_actions = tool_schema_actions(&hire_only[0]);
        assert!(hire_actions.contains(&"hire".into()));
        assert!(hire_actions.contains(&"action_get".into()));
        assert!(hire_actions.contains(&"action_list".into()));
        assert!(!hire_actions.contains(&"suspend".into()));
        assert!(!hire_actions.contains(&"terminate".into()));

        let all = staffing_mcp_tools(&[
            COMPANY_PERMISSION_STAFF_HIRE.into(),
            COMPANY_PERMISSION_STAFF_SUSPEND.into(),
            COMPANY_PERMISSION_STAFF_TERMINATE.into(),
        ]);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name.as_ref(), "company.staff");
        let schema =
            serde_json::to_string(&all[0].input_schema).expect("staffing schema should serialize");
        assert!(schema.contains("hire"));
        assert!(schema.contains("terminate"));
        assert!(schema.contains("action_list"));
    }

    #[test]
    fn active_company_agents_receive_four_company_domain_tools() {
        let tools = company_mcp_tools(&[]);
        let names = tools
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<Vec<_>>();
        assert_eq!(names.len(), 4);
        assert!(names.contains(&"company.chat"));
        assert!(names.contains(&"company.project"));
        assert!(names.contains(&"company.task"));
        assert!(names.contains(&"company.events"));
    }

    #[test]
    fn every_tool_result_surfaces_pending_message_notice_until_acknowledged() {
        let app = PlatformApp::new(MemoryPlatformRepository::default());
        let owner = app
            .dev_login(DevLoginInput {
                email: "mcp-inbox-notice-owner@example.com".into(),
                display_name: "MCP Inbox Notice Owner".into(),
            })
            .expect("owner should be created");
        let company = app
            .create_company(CreateCompanyInput {
                human_user_id: owner.id,
                name: "MCP Inbox Notice Company".into(),
                slug: Some("mcp-inbox-notice-company".into()),
                description: None,
            })
            .expect("company should be created");
        let manager = app
            .create_company_agent(CreateCompanyAgentInput {
                human_user_id: owner.id,
                company_id: company.company.id,
                display_name: "Notice Manager".into(),
                handle: "notice-manager".into(),
                persona: "负责发送消息".into(),
                org_unit_id: None,
                job_title: None,
                role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
                reports_to_membership_id: None,
            })
            .expect("manager should be created");
        let engineer = app
            .create_company_agent(CreateCompanyAgentInput {
                human_user_id: owner.id,
                company_id: company.company.id,
                display_name: "Notice Engineer".into(),
                handle: "notice-engineer".into(),
                persona: "负责处理消息".into(),
                org_unit_id: None,
                job_title: None,
                role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
                reports_to_membership_id: Some(manager.membership.id),
            })
            .expect("engineer should be created");
        let group_id = company.conversations[0].preview.id;
        let gateway = McpGateway::new(app.clone(), None);
        gateway
            .invoke(
                Some(&manager.agent_key_plaintext),
                "company.chat",
                json!({
                    "action": "send",
                    "company_id": company.company.id,
                    "conversation_id": group_id,
                    "content": "请立即处理这条新的项目消息。",
                    "mentioned_agent_ids": [engineer.agent_profile.id],
                    "mention_all": false,
                    "idempotency_key": "mcp-inbox-notice-message"
                }),
            )
            .expect("manager should send a message");

        let handler = AiChatMcpHandler::new(gateway.clone());
        let notice = handler
            .pending_message_notice(engineer.agent_profile.id)
            .expect("pending message should produce a notice");
        assert_eq!(notice["attention_required"], true);
        assert_eq!(notice["pending_message_count"], 1);
        assert_eq!(
            notice["latest_messages"][0]["content_preview"],
            "请立即处理这条新的项目消息。"
        );

        let result = handler.structured_success(
            "company.project",
            engineer.agent_profile.id,
            json!({ "projects": [] }),
        );
        let serialized = serde_json::to_value(result).expect("tool result should serialize");
        assert_eq!(
            serialized["structuredContent"]["inbox_notice"]["attention_required"],
            true
        );

        let event_id = app
            .list_agent_inbox_events(engineer.agent_profile.id, true, 10)
            .expect("engineer inbox should list")
            .into_iter()
            .find(|event| event.event_type == "message.received")
            .expect("message event should exist")
            .id;
        gateway
            .invoke(
                Some(&engineer.agent_key_plaintext),
                "agent.inbox.ack",
                json!({ "event_id": event_id }),
            )
            .expect("engineer should acknowledge the message");
        assert!(handler
            .pending_message_notice(engineer.agent_profile.id)
            .is_none());
    }

    #[test]
    fn company_action_schemas_keep_hot_grant_project_actions_visible() {
        let member_tools = company_mcp_tools(&[COMPANY_PERMISSION_TASK_UPDATE.into()]);
        let member_project = member_tools
            .iter()
            .find(|tool| tool.name.as_ref() == "company.project")
            .expect("member project tool");
        let member_project_actions = tool_schema_actions(member_project);
        assert_eq!(
            member_project_actions,
            [
                "assets_replace",
                "get",
                "list",
                "rule_update",
                "status_update",
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
        );
        let member_project_schema = serde_json::to_string(&member_project.input_schema)
            .expect("member project schema should serialize");
        assert!(!member_project_schema.contains("project_status"));

        let member_task = member_tools
            .iter()
            .find(|tool| tool.name.as_ref() == "company.task")
            .expect("member task tool");
        assert_eq!(
            tool_schema_actions(member_task),
            ["get", "list", "my", "update"]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );
        let member_task_schema = serde_json::to_string(&member_task.input_schema)
            .expect("member task schema should serialize");
        assert!(member_task_schema.contains("\"status\""));
        let member_task_schema_value = Value::Object((*member_task.input_schema).clone());
        let status_schema = schema_action_property(&member_task_schema_value, "update", "status")
            .expect("member update status schema should exist");
        assert_eq!(
            status_schema
                .get("enum")
                .and_then(Value::as_array)
                .expect("member status should be an enum"),
            &vec![
                json!("in_progress"),
                json!("blocked"),
                json!("done"),
                json!("failed"),
                Value::Null,
            ]
        );
        for hidden_field in [
            "title",
            "description",
            "priority",
            "assignee_agent_id",
            "due_at",
        ] {
            assert!(!schema_action_has_property(
                &member_task_schema_value,
                "update",
                hidden_field,
            ));
        }

        let manager_tools = company_mcp_tools(&[
            COMPANY_PERMISSION_PROJECT_CREATE.into(),
            COMPANY_PERMISSION_PROJECT_MANAGE.into(),
            COMPANY_PERMISSION_PROJECT_RULES_MANAGE.into(),
            COMPANY_PERMISSION_PROJECT_ASSETS_MANAGE.into(),
            COMPANY_PERMISSION_TASK_ASSIGN.into(),
            COMPANY_PERMISSION_TASK_UPDATE.into(),
        ]);
        let manager_project = manager_tools
            .iter()
            .find(|tool| tool.name.as_ref() == "company.project")
            .expect("manager project tool");
        let manager_project_actions = tool_schema_actions(manager_project);
        for action in [
            "create",
            "git_provision",
            "update",
            "member_add",
            "member_remove",
            "rule_update",
            "assets_replace",
        ] {
            assert!(manager_project_actions.contains(&action.to_string()));
        }
        let manager_task = manager_tools
            .iter()
            .find(|tool| tool.name.as_ref() == "company.task")
            .expect("manager task tool");
        let manager_task_actions = tool_schema_actions(manager_task);
        for action in [
            "get",
            "list",
            "my",
            "create",
            "update",
            "batch_update",
            "dependency_add",
            "dependency_remove",
        ] {
            assert!(manager_task_actions.contains(&action.to_string()));
        }
    }

    #[test]
    fn task_reads_do_not_require_idempotency_or_mutation_auditing() {
        for action in ["get", "list", "my"] {
            assert!(!is_mutating_tool(
                "company.task",
                &json!({ "action": action })
            ));
        }
        assert!(is_mutating_tool(
            "company.task",
            &json!({ "action": "update" })
        ));
    }

    #[test]
    fn assigned_agent_can_read_tasks_through_get_list_and_my_actions() {
        let app = PlatformApp::new(MemoryPlatformRepository::default());
        let owner = app
            .dev_login(DevLoginInput {
                email: "mcp-task-owner@example.com".into(),
                display_name: "MCP Task Owner".into(),
            })
            .expect("owner should be created");
        let company = app
            .create_company(CreateCompanyInput {
                human_user_id: owner.id,
                name: "MCP Task Company".into(),
                slug: Some("mcp-task-company".into()),
                description: None,
            })
            .expect("company should be created");
        let manager = app
            .create_company_agent(CreateCompanyAgentInput {
                human_user_id: owner.id,
                company_id: company.company.id,
                display_name: "MCP Task Manager".into(),
                handle: "mcp-task-manager".into(),
                persona: "负责项目任务".into(),
                org_unit_id: None,
                job_title: Some("项目经理".into()),
                role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
                reports_to_membership_id: None,
            })
            .expect("manager should be created");
        let engineer = app
            .create_company_agent(CreateCompanyAgentInput {
                human_user_id: owner.id,
                company_id: company.company.id,
                display_name: "MCP Task Engineer".into(),
                handle: "mcp-task-engineer".into(),
                persona: "负责执行任务".into(),
                org_unit_id: None,
                job_title: Some("软件工程师".into()),
                role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
                reports_to_membership_id: Some(manager.membership.id),
            })
            .expect("engineer should be created");
        let project = app
            .create_company_project(CreateCompanyProjectInput {
                actor_agent_id: manager.agent_profile.id,
                company_id: company.company.id,
                name: "MCP Task Project".into(),
                description: None,
                member_agent_ids: vec![engineer.agent_profile.id],
            })
            .expect("project should be created");
        let prerequisite = app
            .create_company_project_task(CreateCompanyProjectTaskInput {
                actor_agent_id: manager.agent_profile.id,
                company_id: company.company.id,
                project_id: project.project.id,
                title: "准备 MCP 测试数据".into(),
                description: None,
                priority: Some("high".into()),
                assignee_agent_id: Some(manager.agent_profile.id),
                due_at: None,
            })
            .expect("prerequisite should be created");
        let task = app
            .create_company_project_task(CreateCompanyProjectTaskInput {
                actor_agent_id: manager.agent_profile.id,
                company_id: company.company.id,
                project_id: project.project.id,
                title: "验证任务读取 MCP".into(),
                description: None,
                priority: Some("high".into()),
                assignee_agent_id: Some(engineer.agent_profile.id),
                due_at: None,
            })
            .expect("task should be created");
        app.add_company_project_task_dependency(ChangeCompanyProjectTaskDependencyInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_id: task.id,
            depends_on_task_id: prerequisite.id,
        })
        .expect("task dependency should be created");
        let gateway = McpGateway::new(app.clone(), None);

        let mine = gateway
            .invoke(
                Some(&engineer.agent_key_plaintext),
                "company.task",
                json!({ "action": "my", "company_id": company.company.id }),
            )
            .expect("assigned Agent should list its tasks");
        assert_eq!(
            mine.output["assignments"][0]["task"]["id"],
            task.id.to_string()
        );
        assert_eq!(mine.output["waiting_count"], 1);
        assert_eq!(
            mine.output["assignments"][0]["readiness"],
            "waiting_for_dependencies"
        );
        assert_eq!(mine.output["assignments"][0]["can_start"], false);
        assert_eq!(
            mine.output["assignments"][0]["unresolved_dependencies"][0]["task_id"],
            prerequisite.id.to_string()
        );

        app.update_company_project_task(UpdateCompanyProjectTaskInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            task_id: prerequisite.id,
            title: None,
            description: None,
            status: Some("done".into()),
            priority: None,
            assignee_agent_id: None,
            due_at: None,
        })
        .expect("prerequisite should complete");
        let ready = gateway
            .invoke(
                Some(&engineer.agent_key_plaintext),
                "company.task",
                json!({ "action": "my", "company_id": company.company.id }),
            )
            .expect("assigned Agent should re-check task readiness");
        assert_eq!(ready.output["ready_count"], 1);
        assert_eq!(ready.output["assignments"][0]["readiness"], "ready");
        assert_eq!(ready.output["assignments"][0]["can_start"], true);

        let listed = gateway
            .invoke(
                Some(&engineer.agent_key_plaintext),
                "company.task",
                json!({
                    "action": "list",
                    "company_id": company.company.id,
                    "project_id": project.project.id,
                    "assignee_agent_id": engineer.agent_profile.id,
                    "status": "todo"
                }),
            )
            .expect("project member should filter project tasks");
        assert_eq!(listed.output["tasks"][0]["id"], task.id.to_string());

        let fetched = gateway
            .invoke(
                Some(&engineer.agent_key_plaintext),
                "company.task",
                json!({
                    "action": "get",
                    "company_id": company.company.id,
                    "project_id": project.project.id,
                    "task_id": task.id
                }),
            )
            .expect("project member should fetch one task");
        assert_eq!(fetched.output["task"]["id"], task.id.to_string());
    }

    fn tool_schema_actions(tool: &Tool) -> Vec<String> {
        fn collect(value: &Value, actions: &mut Vec<String>) {
            if let Some(action) = schema_action_name(value) {
                if !actions.contains(&action) {
                    actions.push(action);
                }
            }
            match value {
                Value::Object(object) => {
                    for child in object.values() {
                        collect(child, actions);
                    }
                }
                Value::Array(items) => {
                    for item in items {
                        collect(item, actions);
                    }
                }
                _ => {}
            }
        }

        let mut actions = Vec::new();
        collect(&Value::Object((*tool.input_schema).clone()), &mut actions);
        actions.sort();
        actions
    }

    fn schema_action_has_property(value: &Value, action: &str, property: &str) -> bool {
        if schema_action_name(value).as_deref() == Some(action)
            && value
                .get("properties")
                .and_then(Value::as_object)
                .is_some_and(|properties| properties.contains_key(property))
        {
            return true;
        }
        match value {
            Value::Object(object) => object
                .values()
                .any(|child| schema_action_has_property(child, action, property)),
            Value::Array(items) => items
                .iter()
                .any(|item| schema_action_has_property(item, action, property)),
            _ => false,
        }
    }

    fn schema_action_property<'a>(
        value: &'a Value,
        action: &str,
        property: &str,
    ) -> Option<&'a Value> {
        if schema_action_name(value).as_deref() == Some(action) {
            if let Some(field) = value
                .get("properties")
                .and_then(Value::as_object)
                .and_then(|properties| properties.get(property))
            {
                return Some(field);
            }
        }
        match value {
            Value::Object(object) => object
                .values()
                .find_map(|child| schema_action_property(child, action, property)),
            Value::Array(items) => items
                .iter()
                .find_map(|item| schema_action_property(item, action, property)),
            _ => None,
        }
    }

    #[test]
    fn agent_key_supports_custom_header_and_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert("x-agent-key", "agk_custom".parse().unwrap());
        assert_eq!(
            agent_key_from_headers(&headers).as_deref(),
            Some("agk_custom")
        );

        headers.remove("x-agent-key");
        headers.insert(AUTHORIZATION, "bearer agk_bearer".parse().unwrap());
        assert_eq!(
            agent_key_from_headers(&headers).as_deref(),
            Some("agk_bearer")
        );
    }
}
