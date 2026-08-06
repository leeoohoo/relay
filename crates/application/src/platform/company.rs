use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn create_company(&self, input: CreateCompanyInput) -> AppResult<CompanyConsoleView> {
        if !self.repo.human_user_exists(input.human_user_id) {
            return Err(AppError::NotFound("human user not found".into()));
        }
        let name = input.name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(AppError::Validation(
                "company name must contain 1 to 80 characters".into(),
            ));
        }
        let slug = normalize_company_slug(input.slug.as_deref().unwrap_or(name))?;
        if self.repo.get_company_by_slug(&slug).is_some() {
            return Err(AppError::Conflict("company slug is already in use".into()));
        }

        let now = now_utc();
        let company = Company {
            id: Uuid::new_v4(),
            owner_user_id: input.human_user_id,
            name: name.to_string(),
            slug,
            description: normalize_optional_text(input.description).unwrap_or_default(),
            status: "active".into(),
            created_at: now,
            updated_at: now,
        };
        let owner_membership = CompanyHumanMember {
            id: Uuid::new_v4(),
            company_id: company.id,
            human_user_id: input.human_user_id,
            role: COMPANY_ROLE_OWNER.into(),
            status: "active".into(),
            created_at: now,
            updated_at: now,
        };
        let root_org_unit = OrgUnit {
            id: Uuid::new_v4(),
            company_id: company.id,
            parent_org_unit_id: None,
            name: company.name.clone(),
            unit_type: "company".into(),
            sort_order: 0,
            status: "active".into(),
            created_at: now,
            updated_at: now,
        };
        let default_group_id = Uuid::new_v4();
        let default_group = CompanyConversationView {
            preview: ConversationPreview {
                id: default_group_id,
                title: format!("{} 全员群", company.name),
                conversation_type: ConversationType::Group,
                last_message_preview: None,
                updated_at: now,
            },
            context: ConversationContext {
                conversation_id: default_group_id,
                company_id: Some(company.id),
                project_id: None,
                context_type: CONVERSATION_CONTEXT_COMPANY_ALL.into(),
                visibility: "members".into(),
            },
            member_agent_ids: Vec::new(),
        };
        self.repo.insert_company_bundle(CompanyCreationBundle {
            company: company.clone(),
            owner_membership: owner_membership.clone(),
            root_org_unit: root_org_unit.clone(),
            default_group: default_group.clone(),
        })?;
        let governance_policy = default_company_governance_policy_view(company.id);

        Ok(CompanyConsoleView {
            company,
            human_membership: owner_membership,
            org_units: vec![root_org_unit],
            agents: Vec::new(),
            conversations: vec![default_group],
            projects: Vec::new(),
            professions: company_profession_catalog(),
            project_types: company_project_type_catalog(),
            governance_policy,
        })
    }

    pub fn list_human_companies(&self, human_user_id: Uuid) -> AppResult<Vec<Company>> {
        if !self.repo.human_user_exists(human_user_id) {
            return Err(AppError::NotFound("human user not found".into()));
        }
        Ok(self.repo.list_human_companies(human_user_id))
    }

    pub fn get_company_console(
        &self,
        human_user_id: Uuid,
        company_id: Uuid,
    ) -> AppResult<CompanyConsoleView> {
        let company = self
            .repo
            .get_company_result(company_id)?
            .ok_or_else(|| AppError::NotFound("company not found".into()))?;
        let human_membership = self
            .repo
            .get_company_human_member_result(company_id, human_user_id)?
            .filter(|membership| membership.status == "active")
            .ok_or_else(|| {
                AppError::Unauthorized("human user is not an active company member".into())
            })?;
        let org_units = self.repo.list_company_org_units(company_id);
        let agents = self
            .repo
            .list_company_agent_memberships(company_id)
            .into_iter()
            .map(|membership| {
                let agent_profile = self
                    .repo
                    .get_agent_profile(membership.agent_profile_id)
                    .ok_or_else(|| AppError::NotFound("company agent profile not found".into()))?;
                let connection = self.company_agent_connection_view(&membership);
                let profession = infer_company_profession(Some(&membership.job_title));
                Ok(CompanyConsoleAgentView {
                    agent_profile,
                    membership,
                    profession,
                    connection,
                })
            })
            .collect::<AppResult<Vec<_>>>()?;
        let mut conversations_by_id = HashMap::new();
        for agent in &agents {
            for preview in self.repo.list_agent_conversations(agent.agent_profile.id) {
                let Some(context) = self.repo.get_conversation_context_result(preview.id)? else {
                    continue;
                };
                if context.company_id != Some(company_id)
                    || !matches!(
                        context.context_type.as_str(),
                        CONVERSATION_CONTEXT_COMPANY_DIRECT
                            | CONVERSATION_CONTEXT_COMPANY_ALL
                            | CONVERSATION_CONTEXT_COMPANY_GROUP
                            | CONVERSATION_CONTEXT_PROJECT_GROUP
                    )
                {
                    continue;
                }
                conversations_by_id
                    .entry(preview.id)
                    .or_insert_with(|| CompanyConversationView {
                        member_agent_ids: self.repo.list_conversation_member_ids(preview.id),
                        preview,
                        context,
                    });
            }
        }
        for preview in self.repo.list_company_conversations_result(company_id)? {
            let Some(context) = self.repo.get_conversation_context_result(preview.id)? else {
                continue;
            };
            if context.company_id != Some(company_id)
                || !matches!(
                    context.context_type.as_str(),
                    CONVERSATION_CONTEXT_COMPANY_ALL
                        | CONVERSATION_CONTEXT_COMPANY_DIRECT
                        | CONVERSATION_CONTEXT_COMPANY_GROUP
                        | CONVERSATION_CONTEXT_PROJECT_GROUP
                )
            {
                continue;
            }
            conversations_by_id
                .entry(preview.id)
                .or_insert_with(|| CompanyConversationView {
                    member_agent_ids: self.repo.list_conversation_member_ids(preview.id),
                    preview,
                    context,
                });
        }
        let mut conversations = conversations_by_id.into_values().collect::<Vec<_>>();
        conversations.sort_by(|left, right| {
            right
                .preview
                .updated_at
                .cmp(&left.preview.updated_at)
                .then_with(|| left.preview.id.cmp(&right.preview.id))
        });
        let projects = self
            .repo
            .list_company_projects_result(company_id)?
            .into_iter()
            .map(|project| self.company_project_view(project))
            .collect::<AppResult<Vec<_>>>()?;
        Ok(CompanyConsoleView {
            company,
            human_membership,
            org_units,
            agents,
            conversations,
            projects,
            professions: company_profession_catalog(),
            project_types: company_project_type_catalog(),
            governance_policy: self.company_governance_policy_view(company_id),
        })
    }

    pub(super) fn company_agent_connection_view(
        &self,
        membership: &CompanyAgentMembership,
    ) -> CompanyAgentConnectionView {
        let keys = self.repo.list_agent_keys(membership.agent_profile_id);
        let latest_key = keys
            .iter()
            .max_by(|left, right| left.created_at.cmp(&right.created_at));
        let active_key = keys
            .iter()
            .filter(|key| agent_key_is_active_record(key))
            .max_by(|left, right| left.created_at.cmp(&right.created_at));
        let visible_key = active_key.or(latest_key);
        let status = match membership.employment_status.as_str() {
            "provisioning" => "awaiting_activation",
            "suspended" => "suspended",
            "terminated" => "terminated",
            _ if active_key.and_then(|key| key.last_used_at).is_some() => "connected",
            _ if active_key.is_some() => "not_connected",
            _ if latest_key.is_some_and(|key| key.revoked_at.is_some()) => "key_revoked",
            _ if latest_key.is_some_and(|key| {
                key.expires_at
                    .is_some_and(|expires_at| now_utc() >= expires_at)
            }) =>
            {
                "key_expired"
            }
            _ => "no_key",
        };

        CompanyAgentConnectionView {
            status: status.into(),
            key_prefix: visible_key.map(|key| key.key_prefix.clone()),
            key_created_at: visible_key.map(|key| key.created_at),
            key_expires_at: visible_key.and_then(|key| key.expires_at),
            last_used_at: visible_key.and_then(|key| key.last_used_at),
        }
    }

    pub fn get_company_governance_policy_for_human(
        &self,
        human_user_id: Uuid,
        company_id: Uuid,
    ) -> AppResult<CompanyGovernancePolicyView> {
        self.ensure_company_human_manager(company_id, human_user_id)?;
        Ok(self.company_governance_policy_view(company_id))
    }

    pub fn publish_company_governance_policy(
        &self,
        input: PublishCompanyGovernancePolicyInput,
    ) -> AppResult<CompanyGovernancePolicyView> {
        self.ensure_company_human_manager(input.company_id, input.human_user_id)?;
        validate_company_governance_policy_settings(&input.settings)?;
        let notes = normalize_optional_text(input.notes).unwrap_or_default();
        if notes.chars().count() > 1_000 {
            return Err(AppError::Validation(
                "company governance policy notes cannot exceed 1000 characters".into(),
            ));
        }
        if self
            .repo
            .get_active_company_governance_policy_version(input.company_id)
            .is_some_and(|active| active.settings == input.settings)
        {
            return Err(AppError::Conflict(
                "the active company governance policy already has these settings".into(),
            ));
        }
        let now = now_utc();
        self.repo
            .publish_company_governance_policy_version(CompanyGovernancePolicyVersion {
                id: Uuid::new_v4(),
                company_id: input.company_id,
                version: 0,
                status: COMPANY_GOVERNANCE_POLICY_STATUS_ACTIVE.into(),
                settings: input.settings,
                notes,
                created_by_human_user_id: input.human_user_id,
                updated_by_human_user_id: Some(input.human_user_id),
                created_at: now,
                updated_at: now,
            })?;
        Ok(self.company_governance_policy_view(input.company_id))
    }

    pub fn list_agent_tool_approvals_for_human(
        &self,
        human_user_id: Uuid,
        company_id: Uuid,
        status: Option<&str>,
        limit: usize,
    ) -> AppResult<Vec<AgentToolApprovalRequest>> {
        self.ensure_human_can_manage_company_runtimes(human_user_id, company_id)?;
        if status.is_some_and(|status| !is_agent_tool_approval_status(status)) {
            return Err(AppError::Validation(
                "unsupported agent tool approval status".into(),
            ));
        }
        self.refresh_and_list_agent_tool_approvals(company_id, status, limit.clamp(1, 200))
    }

    pub fn approve_agent_tool_approval(
        &self,
        input: ReviewAgentToolApprovalInput,
    ) -> AppResult<AgentToolApprovalRequest> {
        self.ensure_human_can_manage_company_runtimes(input.human_user_id, input.company_id)?;
        let review_note = normalize_approval_review_note(input.review_note)?;
        let reviewable = self.ensure_agent_tool_approval_is_reviewable(
            input.company_id,
            input.approval_request_id,
        )?;
        if reviewable.approval_source == AGENT_TOOL_APPROVAL_SOURCE_CODEX {
            return self
                .repo
                .claim_agent_tool_approval_request(
                    input.approval_request_id,
                    input.human_user_id,
                    AGENT_TOOL_APPROVAL_STATUS_APPROVED,
                    &review_note,
                    now_utc(),
                )?
                .ok_or_else(|| {
                    AppError::Conflict(
                        "approval request is no longer pending or was claimed by another reviewer"
                            .into(),
                    )
                });
        }
        let mut request = self
            .repo
            .claim_agent_tool_approval_request(
                input.approval_request_id,
                input.human_user_id,
                AGENT_TOOL_APPROVAL_STATUS_EXECUTING,
                &review_note,
                now_utc(),
            )?
            .ok_or_else(|| {
                AppError::Conflict(
                    "approval request is no longer pending or was claimed by another reviewer"
                        .into(),
                )
            })?;
        let now = now_utc();
        match self.execute_agent_tool_approval(&request) {
            Ok(result) => {
                request.status = AGENT_TOOL_APPROVAL_STATUS_EXECUTED.into();
                request.execution_result = result;
                request.error_message = None;
            }
            Err(error) => {
                request.status = AGENT_TOOL_APPROVAL_STATUS_FAILED.into();
                request.error_message = Some(error.to_string());
            }
        }
        request.updated_at = now;
        self.repo
            .update_agent_tool_approval_request(request.clone())?;
        Ok(request)
    }

    pub fn reject_agent_tool_approval(
        &self,
        input: ReviewAgentToolApprovalInput,
    ) -> AppResult<AgentToolApprovalRequest> {
        self.ensure_human_can_manage_company_runtimes(input.human_user_id, input.company_id)?;
        let review_note = normalize_approval_review_note(input.review_note)?;
        self.ensure_agent_tool_approval_is_reviewable(input.company_id, input.approval_request_id)?;
        self.repo
            .claim_agent_tool_approval_request(
                input.approval_request_id,
                input.human_user_id,
                AGENT_TOOL_APPROVAL_STATUS_REJECTED,
                &review_note,
                now_utc(),
            )?
            .ok_or_else(|| {
                AppError::Conflict(
                    "approval request is no longer pending or was claimed by another reviewer"
                        .into(),
                )
            })
    }

    pub(super) fn refresh_and_list_agent_tool_approvals(
        &self,
        company_id: Uuid,
        status: Option<&str>,
        limit: usize,
    ) -> AppResult<Vec<AgentToolApprovalRequest>> {
        let now = now_utc();
        let pending = self.repo.list_company_agent_tool_approval_requests(
            company_id,
            Some(AGENT_TOOL_APPROVAL_STATUS_PENDING),
            1_000,
        );
        for mut request in pending {
            if request.expires_at <= now {
                request.status = AGENT_TOOL_APPROVAL_STATUS_EXPIRED.into();
                request.updated_at = now;
                self.repo.update_agent_tool_approval_request(request)?;
            }
        }
        Ok(self
            .repo
            .list_company_agent_tool_approval_requests(company_id, status, limit))
    }

    pub fn create_codex_approval_request(
        &self,
        input: CreateCodexApprovalRequestInput,
    ) -> AppResult<AgentToolApprovalRequest> {
        if !matches!(
            input.tool_name.as_str(),
            AGENT_CODEX_APPROVAL_TOOL_COMMAND
                | AGENT_CODEX_APPROVAL_TOOL_FILE_CHANGE
                | AGENT_CODEX_APPROVAL_TOOL_PERMISSIONS
        ) {
            return Err(AppError::Validation(
                "unsupported Codex approval request type".into(),
            ));
        }
        if !matches!(input.risk_level.as_str(), "low" | "medium" | "high") {
            return Err(AppError::Validation(
                "unsupported Codex approval risk level".into(),
            ));
        }
        if !input.arguments.is_object() {
            return Err(AppError::Validation(
                "Codex approval arguments must be an object".into(),
            ));
        }
        let now = now_utc();
        if input.expires_at <= now {
            return Err(AppError::Validation(
                "Codex approval request must expire in the future".into(),
            ));
        }
        let request = AgentToolApprovalRequest {
            id: Uuid::new_v4(),
            company_id: input.company_id,
            approval_source: AGENT_TOOL_APPROVAL_SOURCE_CODEX.into(),
            runtime_config_id: None,
            runtime_run_id: None,
            codex_trigger_run_id: Some(input.codex_trigger_run_id),
            requested_by_agent_id: input.requested_by_agent_id,
            tool_name: input.tool_name,
            risk_level: input.risk_level,
            reason: input.reason.trim().to_string(),
            arguments: input.arguments,
            status: AGENT_TOOL_APPROVAL_STATUS_PENDING.into(),
            expires_at: input.expires_at,
            reviewed_by_human_user_id: None,
            review_note: String::new(),
            reviewed_at: None,
            execution_result: json!({}),
            error_message: None,
            created_at: now,
            updated_at: now,
        };
        self.repo
            .insert_agent_tool_approval_request(request.clone())?;
        Ok(request)
    }

    pub fn get_codex_approval_request_for_runner(
        &self,
        approval_request_id: Uuid,
        codex_trigger_run_id: Uuid,
    ) -> AppResult<AgentToolApprovalRequest> {
        let mut request = self
            .repo
            .get_agent_tool_approval_request(approval_request_id)
            .filter(|request| {
                request.approval_source == AGENT_TOOL_APPROVAL_SOURCE_CODEX
                    && request.codex_trigger_run_id == Some(codex_trigger_run_id)
            })
            .ok_or_else(|| AppError::NotFound("Codex approval request not found".into()))?;
        if request.status == AGENT_TOOL_APPROVAL_STATUS_PENDING && request.expires_at <= now_utc() {
            request.status = AGENT_TOOL_APPROVAL_STATUS_EXPIRED.into();
            request.updated_at = now_utc();
            self.repo
                .update_agent_tool_approval_request(request.clone())?;
        }
        Ok(request)
    }

    pub(super) fn ensure_agent_tool_approval_is_reviewable(
        &self,
        company_id: Uuid,
        approval_request_id: Uuid,
    ) -> AppResult<AgentToolApprovalRequest> {
        let mut request = self
            .repo
            .get_agent_tool_approval_request(approval_request_id)
            .filter(|request| request.company_id == company_id)
            .ok_or_else(|| AppError::NotFound("agent tool approval request not found".into()))?;
        if request.status != AGENT_TOOL_APPROVAL_STATUS_PENDING {
            return Err(AppError::Conflict(format!(
                "approval request is not pending: {}",
                request.status
            )));
        }
        if request.expires_at <= now_utc() {
            request.status = AGENT_TOOL_APPROVAL_STATUS_EXPIRED.into();
            request.updated_at = now_utc();
            self.repo
                .update_agent_tool_approval_request(request.clone())?;
            return Err(AppError::Conflict("approval request has expired".into()));
        }
        Ok(request)
    }

    pub(super) fn execute_agent_tool_approval(
        &self,
        request: &AgentToolApprovalRequest,
    ) -> AppResult<serde_json::Value> {
        let proposal: AgentToolApprovalProposal = serde_json::from_value(request.arguments.clone())
            .map_err(|error| {
                AppError::Validation(format!("approval request arguments are invalid: {error}"))
            })?;
        if proposal.tool != request.tool_name {
            return Err(AppError::Validation(
                "approval request tool does not match its arguments".into(),
            ));
        }
        match request.tool_name.as_str() {
            AGENT_RUNTIME_APPROVAL_ACTION_STAFF_HIRE => {
                let result = self.hire_company_agent(AgentStaffingHireInput {
                    actor_agent_id: request.requested_by_agent_id,
                    company_id: request.company_id,
                    display_name: proposal.display_name.unwrap_or_default(),
                    handle: proposal.handle.unwrap_or_default(),
                    persona: proposal.persona.unwrap_or_default(),
                    org_unit_id: proposal.org_unit_id,
                    job_title: proposal.job_title,
                    reports_to_membership_id: proposal.reports_to_membership_id,
                    reason: Some(request.reason.clone()),
                    idempotency_key: Some(format!("approval:{}", request.id)),
                })?;
                Ok(serde_json::to_value(result).unwrap_or_default())
            }
            AGENT_RUNTIME_APPROVAL_ACTION_STAFF_SUSPEND => {
                let result = self.suspend_company_agent_as_agent(AgentStaffingStatusInput {
                    actor_agent_id: request.requested_by_agent_id,
                    company_id: request.company_id,
                    target_agent_id: proposal.target_agent_id.ok_or_else(|| {
                        AppError::Validation("target_agent_id is required".into())
                    })?,
                    reason: Some(request.reason.clone()),
                    handoff_plan: proposal.handoff_plan,
                    handoff_agent_id: proposal.handoff_agent_id,
                    idempotency_key: Some(format!("approval:{}", request.id)),
                })?;
                Ok(serde_json::to_value(result).unwrap_or_default())
            }
            AGENT_RUNTIME_APPROVAL_ACTION_STAFF_TERMINATE => {
                let result = self.terminate_company_agent_as_agent(AgentStaffingStatusInput {
                    actor_agent_id: request.requested_by_agent_id,
                    company_id: request.company_id,
                    target_agent_id: proposal.target_agent_id.ok_or_else(|| {
                        AppError::Validation("target_agent_id is required".into())
                    })?,
                    reason: Some(request.reason.clone()),
                    handoff_plan: proposal.handoff_plan,
                    handoff_agent_id: proposal.handoff_agent_id,
                    idempotency_key: Some(format!("approval:{}", request.id)),
                })?;
                Ok(serde_json::to_value(result).unwrap_or_default())
            }
            AGENT_RUNTIME_APPROVAL_ACTION_TASK_REASSIGN => {
                let result = self.update_company_project_task(UpdateCompanyProjectTaskInput {
                    actor_agent_id: request.requested_by_agent_id,
                    company_id: request.company_id,
                    project_id: proposal
                        .project_id
                        .ok_or_else(|| AppError::Validation("project_id is required".into()))?,
                    task_id: proposal
                        .task_id
                        .ok_or_else(|| AppError::Validation("task_id is required".into()))?,
                    title: None,
                    description: None,
                    status: None,
                    priority: None,
                    assignee_agent_id: Some(proposal.assignee_agent_id.ok_or_else(|| {
                        AppError::Validation("assignee_agent_id is required".into())
                    })?),
                    due_at: None,
                })?;
                Ok(serde_json::to_value(result).unwrap_or_default())
            }
            _ => Err(AppError::Unauthorized(
                "approval request tool is unsupported".into(),
            )),
        }
    }

    pub(super) fn ensure_human_can_manage_company_runtimes(
        &self,
        human_user_id: Uuid,
        company_id: Uuid,
    ) -> AppResult<CompanyHumanMember> {
        self.repo
            .get_company_result(company_id)?
            .filter(|company| company.status == "active")
            .ok_or_else(|| AppError::NotFound("active company not found".into()))?;
        let membership = self
            .repo
            .get_company_human_member_result(company_id, human_user_id)?
            .filter(|membership| membership.status == "active")
            .ok_or_else(|| {
                AppError::Unauthorized("human user is not an active company member".into())
            })?;
        if !matches!(
            membership.role.as_str(),
            COMPANY_ROLE_OWNER | COMPANY_ROLE_ADMIN
        ) {
            return Err(AppError::Unauthorized(
                "company owner or admin role is required to manage agent runtimes".into(),
            ));
        }
        Ok(membership)
    }

    pub fn ensure_human_can_manage_company_codex(
        &self,
        human_user_id: Uuid,
        company_id: Uuid,
    ) -> AppResult<()> {
        self.ensure_human_can_manage_company_runtimes(human_user_id, company_id)?;
        Ok(())
    }

    pub fn ensure_human_can_send_company_messages(
        &self,
        human_user_id: Uuid,
        company_id: Uuid,
    ) -> AppResult<CompanyHumanMember> {
        self.repo
            .get_company_result(company_id)?
            .filter(|company| company.status == "active")
            .ok_or_else(|| AppError::NotFound("active company not found".into()))?;
        let membership = self
            .repo
            .get_company_human_member_result(company_id, human_user_id)?
            .filter(|membership| membership.status == "active")
            .ok_or_else(|| {
                AppError::Unauthorized("human user is not an active company member".into())
            })?;
        if !matches!(
            membership.role.as_str(),
            COMPANY_ROLE_OWNER | COMPANY_ROLE_ADMIN
        ) {
            return Err(AppError::Unauthorized(
                "company owner or admin role is required to send messages".into(),
            ));
        }
        Ok(membership)
    }

    pub(super) fn company_governance_policy_view(
        &self,
        company_id: Uuid,
    ) -> CompanyGovernancePolicyView {
        let versions = self
            .repo
            .list_company_governance_policy_versions(company_id);
        let active_version = versions
            .iter()
            .find(|version| version.status == COMPANY_GOVERNANCE_POLICY_STATUS_ACTIVE)
            .cloned()
            .or_else(|| {
                self.repo
                    .get_active_company_governance_policy_version(company_id)
            });
        CompanyGovernancePolicyView {
            company_id,
            configured: active_version.is_some(),
            effective_settings: active_version
                .as_ref()
                .map(|version| version.settings.clone())
                .unwrap_or_else(default_company_governance_policy_settings),
            active_version,
            versions,
        }
    }

    pub(super) fn effective_company_governance_policy_settings(
        &self,
        company_id: Uuid,
    ) -> CompanyGovernancePolicySettings {
        self.repo
            .get_active_company_governance_policy_version(company_id)
            .map(|version| version.settings)
            .unwrap_or_else(default_company_governance_policy_settings)
    }

    pub fn effective_company_skill_language(&self, company_id: Uuid) -> String {
        self.effective_company_governance_policy_settings(company_id)
            .skill_language
    }
}
