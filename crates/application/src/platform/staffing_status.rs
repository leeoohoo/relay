use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn activate_provisioned_company_agent(
        &self,
        input: HumanCompanyStaffingStatusInput,
    ) -> AppResult<CompanyAgentActivationResult> {
        self.activate_company_agent_internal(input, "provisioning", STAFFING_ACTION_ACTIVATE)
    }

    pub fn reactivate_company_agent(
        &self,
        input: HumanCompanyStaffingStatusInput,
    ) -> AppResult<CompanyAgentActivationResult> {
        self.activate_company_agent_internal(input, "suspended", STAFFING_ACTION_REACTIVATE)
    }

    pub fn suspend_company_agent_as_human(
        &self,
        input: HumanCompanyStaffingStatusInput,
    ) -> AppResult<AgentStaffingStatusResult> {
        self.ensure_company_human_manager(input.company_id, input.human_user_id)?;
        self.change_company_agent_staffing_status(
            input.company_id,
            input.target_agent_id,
            STAFFING_ACTION_SUSPEND,
            STAFFING_ACTOR_HUMAN,
            Some(input.human_user_id),
            None,
            input.reason,
            input.handoff_plan,
            input.handoff_agent_id,
            None,
        )
    }

    pub fn terminate_company_agent_as_human(
        &self,
        input: HumanCompanyStaffingStatusInput,
    ) -> AppResult<AgentStaffingStatusResult> {
        self.ensure_company_human_manager(input.company_id, input.human_user_id)?;
        self.change_company_agent_staffing_status(
            input.company_id,
            input.target_agent_id,
            STAFFING_ACTION_TERMINATE,
            STAFFING_ACTOR_HUMAN,
            Some(input.human_user_id),
            None,
            input.reason,
            input.handoff_plan,
            input.handoff_agent_id,
            None,
        )
    }

    pub fn suspend_company_agent_as_agent(
        &self,
        input: AgentStaffingStatusInput,
    ) -> AppResult<AgentStaffingStatusResult> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        let governance = self.effective_company_governance_policy_settings(input.company_id);
        if !governance.delegated_agent_suspension_enabled {
            return Err(AppError::Unauthorized(
                "company governance disables delegated agent suspension".into(),
            ));
        }
        self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_STAFF_SUSPEND,
        )?;
        self.enforce_delegated_staffing_daily_limit(
            input.company_id,
            STAFFING_ACTION_SUSPEND,
            governance.daily_delegated_suspension_limit,
        )?;
        self.change_company_agent_staffing_status(
            input.company_id,
            input.target_agent_id,
            STAFFING_ACTION_SUSPEND,
            STAFFING_ACTOR_AGENT,
            None,
            Some(input.actor_agent_id),
            input.reason,
            input.handoff_plan,
            input.handoff_agent_id,
            input.idempotency_key,
        )
    }

    pub fn terminate_company_agent_as_agent(
        &self,
        input: AgentStaffingStatusInput,
    ) -> AppResult<AgentStaffingStatusResult> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        let governance = self.effective_company_governance_policy_settings(input.company_id);
        if !governance.delegated_agent_termination_enabled {
            return Err(AppError::Unauthorized(
                "company governance disables delegated agent termination".into(),
            ));
        }
        self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_STAFF_TERMINATE,
        )?;
        self.enforce_delegated_staffing_daily_limit(
            input.company_id,
            STAFFING_ACTION_TERMINATE,
            governance.daily_delegated_termination_limit,
        )?;
        self.change_company_agent_staffing_status(
            input.company_id,
            input.target_agent_id,
            STAFFING_ACTION_TERMINATE,
            STAFFING_ACTOR_AGENT,
            None,
            Some(input.actor_agent_id),
            input.reason,
            input.handoff_plan,
            input.handoff_agent_id,
            input.idempotency_key,
        )
    }

    pub fn list_company_staffing_actions_for_human(
        &self,
        human_user_id: Uuid,
        company_id: Uuid,
    ) -> AppResult<Vec<AgentStaffingAction>> {
        self.ensure_company_human_manager(company_id, human_user_id)?;
        Ok(self.repo.list_agent_staffing_actions(company_id))
    }

    pub fn list_company_staffing_actions_for_agent(
        &self,
        actor_agent_id: Uuid,
        company_id: Uuid,
    ) -> AppResult<Vec<AgentStaffingAction>> {
        self.ensure_agent_can_act(actor_agent_id)?;
        self.ensure_company_agent_staffing_access(company_id, actor_agent_id)?;
        Ok(self.repo.list_agent_staffing_actions(company_id))
    }

    pub fn get_agent_staffing_permissions(&self, agent_id: Uuid) -> AppResult<Vec<String>> {
        self.ensure_agent_can_act(agent_id)?;
        let membership = self
            .repo
            .get_company_agent_membership(agent_id)
            .filter(|membership| membership.employment_status == "active")
            .ok_or_else(|| {
                AppError::Unauthorized("agent is not an active company member".into())
            })?;
        Ok(membership
            .permissions
            .into_iter()
            .filter(|permission| {
                matches!(
                    permission.as_str(),
                    COMPANY_PERMISSION_STAFF_HIRE
                        | COMPANY_PERMISSION_STAFF_SUSPEND
                        | COMPANY_PERMISSION_STAFF_TERMINATE
                )
            })
            .collect())
    }

    pub fn get_active_company_agent_membership(
        &self,
        agent_id: Uuid,
    ) -> AppResult<CompanyAgentMembership> {
        self.ensure_agent_can_act(agent_id)?;
        self.repo
            .get_company_agent_membership(agent_id)
            .filter(|membership| membership.employment_status == "active")
            .ok_or_else(|| AppError::Unauthorized("agent is not an active company member".into()))
    }

    pub fn get_company_staffing_action_for_agent(
        &self,
        actor_agent_id: Uuid,
        company_id: Uuid,
        action_id: Uuid,
    ) -> AppResult<AgentStaffingAction> {
        self.ensure_agent_can_act(actor_agent_id)?;
        self.ensure_company_agent_staffing_access(company_id, actor_agent_id)?;
        self.repo
            .get_agent_staffing_action(action_id)
            .filter(|action| action.company_id == company_id)
            .ok_or_else(|| AppError::NotFound("staffing action not found".into()))
    }

    pub(super) fn ensure_company_human_manager(
        &self,
        company_id: Uuid,
        human_user_id: Uuid,
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
                "company owner or admin role is required for staffing changes".into(),
            ));
        }
        Ok(membership)
    }

    pub(super) fn ensure_company_agent_permission(
        &self,
        company_id: Uuid,
        agent_id: Uuid,
        permission: &str,
    ) -> AppResult<CompanyAgentMembership> {
        let membership = self
            .repo
            .get_company_agent_membership(agent_id)
            .filter(|membership| membership.company_id == company_id)
            .ok_or_else(|| {
                AppError::Unauthorized("agent does not belong to the requested company".into())
            })?;
        if membership.employment_status != "active" {
            return Err(AppError::Unauthorized(
                "agent is not an active company member".into(),
            ));
        }
        if !membership
            .permissions
            .iter()
            .any(|candidate| candidate == permission)
        {
            return Err(AppError::Unauthorized(format!(
                "agent is missing required permission: {permission}"
            )));
        }
        Ok(membership)
    }

    pub(super) fn ensure_staffing_org_scope(
        &self,
        company_id: Uuid,
        actor_membership: &CompanyAgentMembership,
        target_org_unit_id: Uuid,
    ) -> AppResult<()> {
        let Some(scope_org_unit_id) = actor_membership.staffing_scope_org_unit_id else {
            return Ok(());
        };
        let org_units = self.repo.list_company_org_units(company_id);
        if org_unit_is_within_scope(&org_units, scope_org_unit_id, target_org_unit_id) {
            Ok(())
        } else {
            Err(AppError::Unauthorized(
                "staffing action is outside the agent's authorized org scope".into(),
            ))
        }
    }

    pub(super) fn enforce_delegated_staffing_daily_limit(
        &self,
        company_id: Uuid,
        action_type: &str,
        daily_limit: i32,
    ) -> AppResult<()> {
        let window_start = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .expect("midnight must be valid")
            .and_utc();
        let action_count = self
            .repo
            .list_agent_staffing_actions(company_id)
            .into_iter()
            .filter(|action| {
                action.actor_type == STAFFING_ACTOR_AGENT
                    && action.action_type == action_type
                    && action.status == STAFFING_STATUS_COMPLETED
                    && action.created_at >= window_start
            })
            .count();
        if action_count >= daily_limit.max(0) as usize {
            Err(AppError::RateLimited(format!(
                "company delegated {action_type} daily limit of {daily_limit} has been reached"
            )))
        } else {
            Ok(())
        }
    }

    pub(super) fn ensure_company_agent_staffing_access(
        &self,
        company_id: Uuid,
        agent_id: Uuid,
    ) -> AppResult<CompanyAgentMembership> {
        let membership = self
            .repo
            .get_company_agent_membership(agent_id)
            .filter(|membership| membership.company_id == company_id)
            .ok_or_else(|| {
                AppError::Unauthorized("agent does not belong to the requested company".into())
            })?;
        if membership.employment_status != "active"
            || !membership.permissions.iter().any(|permission| {
                matches!(
                    permission.as_str(),
                    COMPANY_PERMISSION_STAFF_HIRE
                        | COMPANY_PERMISSION_STAFF_SUSPEND
                        | COMPANY_PERMISSION_STAFF_TERMINATE
                )
            })
        {
            return Err(AppError::Unauthorized(
                "an active staffing permission is required".into(),
            ));
        }
        Ok(membership)
    }

    pub(super) fn activate_company_agent_internal(
        &self,
        input: HumanCompanyStaffingStatusInput,
        expected_employment_status: &str,
        action_type: &str,
    ) -> AppResult<CompanyAgentActivationResult> {
        self.ensure_company_human_manager(input.company_id, input.human_user_id)?;
        let mut membership = self
            .repo
            .get_company_agent_membership(input.target_agent_id)
            .filter(|membership| membership.company_id == input.company_id)
            .ok_or_else(|| AppError::NotFound("company agent membership not found".into()))?;
        if membership.employment_status != expected_employment_status {
            return Err(AppError::Conflict(format!(
                "agent employment status must be {expected_employment_status}"
            )));
        }
        if self
            .repo
            .list_agent_keys(input.target_agent_id)
            .into_iter()
            .any(|key| agent_key_is_active_record(&key))
        {
            return Err(AppError::Conflict("agent already has an active key".into()));
        }
        let mut agent_profile = self
            .repo
            .get_agent_profile(input.target_agent_id)
            .ok_or_else(|| AppError::NotFound("agent profile not found".into()))?;
        let now = now_utc();
        agent_profile.status = AgentStatus::Active;
        membership.employment_status = "active".into();
        membership.terminated_at = None;
        membership.updated_at = now;

        let plaintext_key = generate_agent_key();
        let key_prefix = plaintext_key.chars().take(12).collect::<String>();
        let key_record = AgentKeyRecord {
            id: Uuid::new_v4(),
            agent_profile_id: agent_profile.id,
            key_name: "primary".into(),
            key_prefix: key_prefix.clone(),
            key_hash: hash_secret(&plaintext_key),
            last_used_at: None,
            expires_at: Some(now + Duration::days(180)),
            revoked_at: None,
            created_at: now,
        };
        let key_issue_log = AgentKeyIssueLog {
            id: Uuid::new_v4(),
            agent_profile_id: agent_profile.id,
            agent_key_id: Some(key_record.id),
            issue_type: AgentKeyIssueType::Issued,
            issued_by_user_id: Some(input.human_user_id),
            metadata: json!({
                "agent_key_prefix": key_prefix,
                "creation_mode": "staffing_activation",
                "company_id": input.company_id,
                "action_type": action_type,
            }),
            created_at: now,
        };
        let action = AgentStaffingAction {
            id: Uuid::new_v4(),
            company_id: input.company_id,
            action_type: action_type.into(),
            actor_type: STAFFING_ACTOR_HUMAN.into(),
            actor_human_user_id: Some(input.human_user_id),
            actor_agent_id: None,
            target_agent_id: Some(agent_profile.id),
            requested_org_unit_id: Some(membership.org_unit_id),
            requested_role_key: Some(membership.role_key.clone()),
            reason: normalize_staffing_text(input.reason, 500, "reason")?,
            handoff_plan: String::new(),
            status: STAFFING_STATUS_COMPLETED.into(),
            approval_required: false,
            approved_by_human_user_id: Some(input.human_user_id),
            request_payload: json!({
                "previous_employment_status": expected_employment_status,
            }),
            result_payload: json!({
                "employment_status": "active",
                "agent_key_prefix": key_prefix,
            }),
            idempotency_key: None,
            created_at: now,
            completed_at: Some(now),
        };
        self.repo
            .complete_company_agent_activation(CompanyAgentActivationBundle {
                agent_profile: agent_profile.clone(),
                membership: membership.clone(),
                key_record,
                key_issue_log,
                action: action.clone(),
            })?;
        if action_type == STAFFING_ACTION_ACTIVATE {
            self.assign_default_codex_runner_profile_to_agent(
                input.company_id,
                agent_profile.id,
                input.human_user_id,
                now,
            )?;
        }

        Ok(CompanyAgentActivationResult {
            action,
            agent_profile,
            membership,
            agent_key_plaintext: plaintext_key,
            agent_key_prefix: key_prefix,
        })
    }

    pub(super) fn assign_default_codex_runner_profile_to_agent(
        &self,
        company_id: Uuid,
        agent_id: Uuid,
        human_user_id: Uuid,
        now: DateTime<Utc>,
    ) -> AppResult<()> {
        if self
            .repo
            .get_agent_codex_runner_profile_assignment(agent_id)
            .is_some()
            || self
                .repo
                .get_agent_codex_trigger_config_by_agent(agent_id)
                .is_some()
        {
            return Ok(());
        }
        let Some(profile) = self
            .repo
            .list_company_codex_runner_profiles(company_id)
            .into_iter()
            .find(|profile| profile.is_default)
        else {
            return Ok(());
        };
        self.repo
            .save_agent_codex_trigger_config(AgentCodexTriggerConfig {
                id: Uuid::new_v4(),
                company_id,
                agent_profile_id: agent_id,
                status: AGENT_CODEX_TRIGGER_STATUS_ACTIVE.into(),
                interval_seconds: profile.interval_seconds,
                codex_profile: profile.codex_profile.clone(),
                model: profile.model.clone(),
                reasoning_effort: profile.reasoning_effort.clone(),
                reasoning_summary: profile.reasoning_summary.clone(),
                verbosity: profile.verbosity.clone(),
                personality: profile.personality.clone(),
                service_tier: profile.service_tier.clone(),
                sandbox_mode: profile.sandbox_mode.clone(),
                approval_policy: profile.approval_policy.clone(),
                network_access: profile.network_access,
                web_search: profile.web_search.clone(),
                feature_multi_agent: profile.feature_multi_agent,
                feature_remote_plugin: profile.feature_remote_plugin,
                feature_hooks: profile.feature_hooks,
                feature_goals: profile.feature_goals,
                feature_shell_tool: profile.feature_shell_tool,
                max_run_seconds: profile.max_run_seconds,
                next_run_at: now,
                lease_owner: None,
                lease_expires_at: None,
                manual_run_requested_at: None,
                wake_requested_at: None,
                wake_reason: None,
                last_run_at: None,
                last_success_at: None,
                last_error: None,
                consecutive_failure_count: 0,
                created_by_human_user_id: human_user_id,
                updated_by_human_user_id: Some(human_user_id),
                created_at: now,
                updated_at: now,
            })?;
        self.repo
            .assign_agent_codex_runner_profile(agent_id, profile.id, human_user_id, now)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn change_company_agent_staffing_status(
        &self,
        company_id: Uuid,
        target_agent_id: Uuid,
        action_type: &str,
        actor_type: &str,
        actor_human_user_id: Option<Uuid>,
        actor_agent_id: Option<Uuid>,
        reason: Option<String>,
        handoff_plan: Option<String>,
        handoff_agent_id: Option<Uuid>,
        idempotency_key: Option<String>,
    ) -> AppResult<AgentStaffingStatusResult> {
        if actor_agent_id == Some(target_agent_id) {
            return Err(AppError::Unauthorized(
                "agent cannot suspend or terminate itself".into(),
            ));
        }
        let memberships = self.repo.list_company_agent_memberships(company_id);
        let mut target_membership = memberships
            .iter()
            .find(|membership| membership.agent_profile_id == target_agent_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound("company agent membership not found".into()))?;
        match action_type {
            STAFFING_ACTION_SUSPEND if target_membership.employment_status != "active" => {
                return Err(AppError::Conflict(
                    "only an active company agent can be suspended".into(),
                ));
            }
            STAFFING_ACTION_TERMINATE if target_membership.employment_status == "terminated" => {
                return Err(AppError::Conflict("agent is already terminated".into()));
            }
            STAFFING_ACTION_SUSPEND | STAFFING_ACTION_TERMINATE => {}
            _ => {
                return Err(AppError::Validation(
                    "unsupported staffing status action".into(),
                ));
            }
        }

        if let Some(actor_agent_id) = actor_agent_id {
            let actor_membership = memberships
                .iter()
                .find(|membership| membership.agent_profile_id == actor_agent_id)
                .ok_or_else(|| {
                    AppError::Unauthorized("staffing actor is not a company member".into())
                })?;
            self.ensure_staffing_org_scope(
                company_id,
                actor_membership,
                target_membership.org_unit_id,
            )?;
            if company_agent_role_rank(&actor_membership.role_key)
                < company_agent_role_rank(&target_membership.role_key)
            {
                return Err(AppError::Unauthorized(
                    "agent cannot suspend or terminate a higher-role agent".into(),
                ));
            }
        }
        if target_membership.role_key == COMPANY_AGENT_ROLE_MANAGER
            && target_membership.employment_status == "active"
            && memberships
                .iter()
                .filter(|membership| {
                    membership.role_key == COMPANY_AGENT_ROLE_MANAGER
                        && membership.employment_status == "active"
                })
                .count()
                <= 1
        {
            return Err(AppError::Conflict(
                "the last active company manager agent cannot be suspended or terminated".into(),
            ));
        }

        let normalized_reason = normalize_staffing_text(reason, 500, "reason")?;
        let normalized_handoff_plan = normalize_staffing_text(handoff_plan, 2_000, "handoff_plan")?;
        if action_type == STAFFING_ACTION_TERMINATE && normalized_handoff_plan.is_empty() {
            return Err(AppError::Validation(
                "handoff_plan is required when terminating an agent".into(),
            ));
        }

        let now = now_utc();
        let mut reassigned_tasks = Vec::new();
        if action_type == STAFFING_ACTION_TERMINATE {
            let mut open_tasks = Vec::new();
            for project in self.repo.list_company_projects_result(company_id)? {
                open_tasks.extend(
                    self.repo
                        .list_company_project_tasks_result(project.id)?
                        .into_iter()
                        .filter(|task| {
                            task.assignee_agent_id == Some(target_agent_id)
                                && !matches!(
                                    task.status.as_str(),
                                    PROJECT_TASK_STATUS_DONE
                                        | PROJECT_TASK_STATUS_FAILED
                                        | PROJECT_TASK_STATUS_CANCELLED
                                )
                        }),
                );
            }
            if !open_tasks.is_empty() {
                let handoff_agent_id = handoff_agent_id.ok_or_else(|| {
                    AppError::Validation(
                        "handoff_agent_id is required when the terminated agent has open tasks"
                            .into(),
                    )
                })?;
                if handoff_agent_id == target_agent_id {
                    return Err(AppError::Validation(
                        "handoff_agent_id must be different from the terminated agent".into(),
                    ));
                }
                memberships
                    .iter()
                    .find(|membership| {
                        membership.agent_profile_id == handoff_agent_id
                            && membership.employment_status == "active"
                    })
                    .ok_or_else(|| {
                        AppError::Validation("handoff agent must be an active company agent".into())
                    })?;
                for mut task in open_tasks {
                    self.ensure_active_project_member(task.project_id, handoff_agent_id)
                        .map_err(|_| {
                            AppError::Validation(format!(
                                "handoff agent is not an active member of project {}",
                                task.project_id
                            ))
                        })?;
                    task.assignee_agent_id = Some(handoff_agent_id);
                    task.updated_by_agent_id = actor_agent_id;
                    task.updated_by_human_user_id = actor_human_user_id;
                    task.updated_at = now;
                    reassigned_tasks.push(task);
                }
            }
        }

        let mut agent_profile = self
            .repo
            .get_agent_profile(target_agent_id)
            .ok_or_else(|| AppError::NotFound("agent profile not found".into()))?;
        agent_profile.status = AgentStatus::Frozen;
        target_membership.employment_status = if action_type == STAFFING_ACTION_SUSPEND {
            "suspended".into()
        } else {
            "terminated".into()
        };
        target_membership.terminated_at = if action_type == STAFFING_ACTION_TERMINATE {
            Some(now)
        } else {
            None
        };
        target_membership.updated_at = now;

        let active_keys = self
            .repo
            .list_agent_keys(target_agent_id)
            .into_iter()
            .filter(agent_key_is_active_record)
            .collect::<Vec<_>>();
        let key_issue_logs = active_keys
            .iter()
            .map(|key| AgentKeyIssueLog {
                id: Uuid::new_v4(),
                agent_profile_id: target_agent_id,
                agent_key_id: Some(key.id),
                issue_type: AgentKeyIssueType::Revoked,
                issued_by_user_id: actor_human_user_id,
                metadata: json!({
                    "agent_key_prefix": key.key_prefix,
                    "reason": action_type,
                    "company_id": company_id,
                    "actor_type": actor_type,
                    "actor_agent_id": actor_agent_id,
                    "revoked_at": now,
                }),
                created_at: now,
            })
            .collect::<Vec<_>>();
        let action = AgentStaffingAction {
            id: Uuid::new_v4(),
            company_id,
            action_type: action_type.into(),
            actor_type: actor_type.into(),
            actor_human_user_id,
            actor_agent_id,
            target_agent_id: Some(target_agent_id),
            requested_org_unit_id: Some(target_membership.org_unit_id),
            requested_role_key: Some(target_membership.role_key.clone()),
            reason: normalized_reason,
            handoff_plan: normalized_handoff_plan,
            status: STAFFING_STATUS_COMPLETED.into(),
            approval_required: false,
            approved_by_human_user_id: actor_human_user_id,
            request_payload: json!({
                "previous_employment_status": memberships
                    .iter()
                    .find(|membership| membership.agent_profile_id == target_agent_id)
                    .map(|membership| membership.employment_status.clone()),
                "handoff_agent_id": handoff_agent_id,
                "open_task_ids": reassigned_tasks.iter().map(|task| task.id).collect::<Vec<_>>(),
            }),
            result_payload: json!({
                "employment_status": target_membership.employment_status,
                "revoked_key_count": active_keys.len(),
                "history_preserved": true,
                "reassigned_task_count": reassigned_tasks.len(),
                "reassigned_task_ids": reassigned_tasks.iter().map(|task| task.id).collect::<Vec<_>>(),
            }),
            idempotency_key: normalize_optional_idempotency_key(idempotency_key)?,
            created_at: now,
            completed_at: Some(now),
        };
        self.repo
            .complete_agent_staffing_status_change(AgentStaffingStatusChangeBundle {
                agent_profile: agent_profile.clone(),
                membership: target_membership.clone(),
                key_issue_logs,
                reassigned_tasks: reassigned_tasks.clone(),
                action: action.clone(),
            })?;
        for task in &reassigned_tasks {
            if let Some(assignee_agent_id) = task.assignee_agent_id {
                let _ = self.enqueue_agent_event(
                    assignee_agent_id,
                    "company.project.task_assigned",
                    json!({
                        "company_id": company_id,
                        "project_id": task.project_id,
                        "task_id": task.id,
                        "task_title": task.title,
                        "assignment_mode": "staffing_handoff",
                        "terminated_agent_id": target_agent_id,
                        "assigned_by_agent_id": actor_agent_id,
                        "assigned_by_human_user_id": actor_human_user_id,
                    }),
                    60,
                );
            }
        }
        Ok(AgentStaffingStatusResult {
            action,
            agent_profile,
            membership: target_membership,
            revoked_key_count: active_keys.len(),
            reassigned_task_count: reassigned_tasks.len(),
        })
    }
}
