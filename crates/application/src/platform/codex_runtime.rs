use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn pause_company_agent_codex_trigger_for_human(
        &self,
        input: SetCompanyAgentCodexTriggerStatusForHumanInput,
    ) -> AppResult<CompanyAgentCodexTriggerView> {
        self.set_company_agent_codex_trigger_status_for_human(
            input,
            AGENT_CODEX_TRIGGER_STATUS_PAUSED,
        )
    }

    pub fn resume_company_agent_codex_trigger_for_human(
        &self,
        input: SetCompanyAgentCodexTriggerStatusForHumanInput,
    ) -> AppResult<CompanyAgentCodexTriggerView> {
        self.set_company_agent_codex_trigger_status_for_human(
            input,
            AGENT_CODEX_TRIGGER_STATUS_ACTIVE,
        )
    }

    pub fn run_company_agent_codex_trigger_now_for_human(
        &self,
        input: SetCompanyAgentCodexTriggerStatusForHumanInput,
    ) -> AppResult<CompanyAgentCodexTriggerView> {
        self.ensure_company_agent_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.agent_id,
        )?;
        let mut config = self
            .repo
            .get_agent_codex_trigger_config_by_agent_result(input.agent_id)?
            .ok_or_else(|| AppError::NotFound("Codex trigger config not found".into()))?;
        if config.status != AGENT_CODEX_TRIGGER_STATUS_ACTIVE {
            return Err(AppError::Conflict(
                "Codex trigger must be active before run-now".into(),
            ));
        }
        let now = now_utc();
        config.manual_run_requested_at = Some(now);
        config.next_run_at = now;
        config.updated_by_human_user_id = Some(input.human_user_id);
        config.updated_at = now;
        self.repo.save_agent_codex_trigger_config(config.clone())?;
        let recent_runs = self.recent_agent_codex_runs_for_human(input.agent_id, 20)?;
        let active_intents = self.active_agent_execution_intents(input.agent_id);
        let runtime = self.project_agent_runtime(&config, &recent_runs, &active_intents);
        Ok(CompanyAgentCodexTriggerView {
            recent_runs,
            active_intents,
            recent_sessions: self.recent_agent_codex_sessions_for_human(input.agent_id, 10),
            runner_profile_id: self
                .repo
                .get_agent_codex_runner_profile_assignment(input.agent_id),
            config,
            runtime,
        })
    }

    pub fn list_company_agent_codex_runs_for_human(
        &self,
        input: ListCompanyAgentCodexRunsForHumanInput,
    ) -> AppResult<Vec<AgentCodexTriggerRun>> {
        self.ensure_company_agent_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.agent_id,
        )?;
        self.recent_agent_codex_runs_for_human(input.agent_id, input.limit)
    }

    pub fn list_company_agent_codex_sessions_for_human(
        &self,
        input: ListCompanyAgentCodexSessionsForHumanInput,
    ) -> AppResult<Vec<AgentCodexSession>> {
        self.repo
            .get_company_human_member_result(input.company_id, input.human_user_id)?
            .filter(|membership| membership.status == "active")
            .ok_or_else(|| {
                AppError::Unauthorized("human user is not an active company member".into())
            })?;
        self.repo
            .get_company_agent_membership_result(input.agent_id)?
            .filter(|membership| membership.company_id == input.company_id)
            .ok_or_else(|| AppError::NotFound("company Agent not found".into()))?;
        let sessions = self
            .repo
            .list_agent_codex_sessions(input.agent_id, input.limit.clamp(1, 100))
            .into_iter()
            .filter(|session| {
                input
                    .project_id
                    .is_none_or(|project_id| session.project_id == Some(project_id))
            })
            .map(sanitize_codex_session_for_human)
            .collect();
        Ok(sessions)
    }

    pub(super) fn active_agent_execution_intents(
        &self,
        agent_id: Uuid,
    ) -> Vec<AgentExecutionIntent> {
        let mut intents = self.repo.list_agent_execution_intents(
            agent_id,
            Some(AGENT_EXECUTION_INTENT_STATUS_RUNNING),
            20,
        );
        intents.extend(self.repo.list_agent_execution_intents(
            agent_id,
            Some(AGENT_EXECUTION_INTENT_STATUS_PENDING),
            20,
        ));
        intents.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        intents
    }

    pub(super) fn recent_agent_codex_sessions_for_human(
        &self,
        agent_id: Uuid,
        limit: usize,
    ) -> Vec<AgentCodexSession> {
        self.repo
            .list_agent_codex_sessions(agent_id, limit.clamp(1, 100))
            .into_iter()
            .map(sanitize_codex_session_for_human)
            .collect()
    }

    pub(super) fn recent_agent_codex_runs_for_human(
        &self,
        agent_id: Uuid,
        limit: usize,
    ) -> AppResult<Vec<AgentCodexTriggerRun>> {
        Ok(self
            .repo
            .list_agent_codex_trigger_runs_result(agent_id, limit.clamp(1, 100))?
            .into_iter()
            .map(sanitize_codex_run_for_human)
            .collect())
    }

    pub fn claim_due_agent_codex_triggers(
        &self,
        lease_owner: &str,
        limit: usize,
    ) -> AppResult<Vec<AgentCodexTriggerConfig>> {
        if lease_owner.trim().is_empty() || lease_owner.chars().count() > 120 {
            return Err(AppError::Validation(
                "Codex trigger lease_owner must contain 1 to 120 characters".into(),
            ));
        }
        self.repo
            .claim_due_agent_codex_trigger_configs(lease_owner, now_utc(), limit.clamp(1, 100))
    }

    pub fn next_eligible_agent_codex_trigger_at(
        &self,
    ) -> AppResult<Option<chrono::DateTime<chrono::Utc>>> {
        self.repo.next_eligible_agent_codex_trigger_at(now_utc())
    }

    pub fn is_agent_codex_trigger_active(&self, agent_id: Uuid) -> AppResult<bool> {
        Ok(self
            .repo
            .get_agent_codex_trigger_config_by_agent_result(agent_id)?
            .is_some_and(|config| config.status == AGENT_CODEX_TRIGGER_STATUS_ACTIVE))
    }

    pub fn abandon_agent_codex_trigger_leases(&self, lease_owner: &str) -> AppResult<usize> {
        if lease_owner.trim().is_empty() || lease_owner.chars().count() > 120 {
            return Err(AppError::Validation(
                "Codex trigger lease_owner must contain 1 to 120 characters".into(),
            ));
        }
        self.repo
            .abandon_agent_codex_trigger_leases(lease_owner, now_utc())
    }

    pub fn decide_agent_codex_work(
        &self,
        config: &AgentCodexTriggerConfig,
    ) -> AppResult<AgentCodexWorkDecision> {
        let membership = self
            .repo
            .get_company_agent_membership_result(config.agent_profile_id)?
            .filter(|membership| {
                membership.company_id == config.company_id
                    && membership.employment_status == "active"
            })
            .ok_or_else(|| {
                AppError::Unauthorized("Codex trigger Agent is not an active company member".into())
            })?;
        self.repo
            .get_company_result(config.company_id)?
            .filter(|company| company.status == "active")
            .ok_or_else(|| AppError::NotFound("active company not found".into()))?;
        let now = now_utc();
        let mut control_snapshot =
            self.agent_control_snapshot(config.agent_profile_id, config.company_id)?;
        let projects = self
            .repo
            .list_company_projects_result(config.company_id)?
            .into_iter()
            .filter(|project| {
                project.status != PROJECT_STATUS_PAUSED
                    && self
                        .repo
                        .get_company_project_member(project.id, membership.agent_profile_id)
                        .is_some_and(|member| member.left_at.is_none())
            })
            .collect::<Vec<_>>();
        let in_progress_task_ids = control_snapshot
            .ready_tasks
            .iter()
            .filter(|task| task.status == PROJECT_TASK_STATUS_IN_PROGRESS)
            .map(|task| task.id)
            .collect::<HashSet<_>>();
        let mut recovered_intent = false;
        if !in_progress_task_ids.is_empty() {
            for intent in self.repo.list_agent_execution_intents(
                config.agent_profile_id,
                Some(AGENT_EXECUTION_INTENT_STATUS_FAILED),
                100,
            ) {
                if intent
                    .task_ids
                    .iter()
                    .any(|task_id| in_progress_task_ids.contains(task_id))
                {
                    let failure = intent.error_message.clone().unwrap_or_default();
                    recovered_intent |= self
                        .requeue_agent_execution_intent_after_retryable_failure(intent, &failure)?
                        .is_some();
                }
            }
        }
        if recovered_intent {
            control_snapshot =
                self.agent_control_snapshot(config.agent_profile_id, config.company_id)?;
        }
        let pending_events = control_snapshot.actionable_events.clone();
        let active_tasks = control_snapshot.ready_tasks.clone();
        let waiting_task_count = control_snapshot.waiting_tasks.len();
        let manual = config.manual_run_requested_at.is_some();
        let can_refresh_assets = membership
            .permissions
            .iter()
            .any(|permission| permission == COMPANY_PERMISSION_PROJECT_ASSETS_MANAGE);
        let asset_refresh = if !manual
            && pending_events.is_empty()
            && active_tasks.is_empty()
            && can_refresh_assets
        {
            self.repo
                .claim_due_company_project_asset_refresh(config.agent_profile_id, now)?
                .filter(|refresh| {
                    projects
                        .iter()
                        .any(|project| project.id == refresh.project_id)
                })
        } else {
            None
        };
        let mut event_project_id = None;
        for event in &pending_events {
            event_project_id = payload_uuid_field_optional(&event.payload_json, "project_id");
            if event_project_id.is_none() {
                if let Some(conversation_id) =
                    payload_uuid_field_optional(&event.payload_json, "conversation_id")
                {
                    event_project_id = self
                        .repo
                        .get_conversation_context_result(conversation_id)?
                        .and_then(|context| context.project_id);
                }
            }
            if event_project_id.is_some() {
                break;
            }
        }
        let selected_project_id = event_project_id
            .or_else(|| active_tasks.first().map(|task| task.project_id))
            .or_else(|| asset_refresh.as_ref().map(|refresh| refresh.project_id))
            .or_else(|| {
                projects.first().and_then(|project| {
                    (projects.len() == 1
                        && self
                            .repo
                            .get_company_project_git_config(project.id)
                            .is_some())
                    .then_some(project.id)
                })
            });
        let project = selected_project_id.and_then(|project_id| {
            projects
                .iter()
                .find(|project| project.id == project_id)
                .cloned()
        });
        let git = project
            .as_ref()
            .and_then(|project| self.repo.get_company_project_git_config(project.id));
        let active_project_ids = projects
            .iter()
            .map(|project| project.id)
            .collect::<HashSet<_>>();
        let pending_execution_intent_count = self
            .repo
            .list_agent_execution_intents(
                config.agent_profile_id,
                Some(AGENT_EXECUTION_INTENT_STATUS_PENDING),
                100,
            )
            .into_iter()
            .filter(|intent| active_project_ids.contains(&intent.project_id))
            .count();
        let covered_task_ids = control_snapshot
            .active_intents
            .iter()
            .flat_map(|intent| intent.task_ids.iter().copied())
            .collect::<HashSet<_>>();
        let resume_existing_intents_directly = !manual
            && pending_events.is_empty()
            && asset_refresh.is_none()
            && pending_execution_intent_count > 0
            && active_tasks
                .iter()
                .all(|task| covered_task_ids.contains(&task.id));
        let should_run = manual
            || !pending_events.is_empty()
            || !active_tasks.is_empty()
            || asset_refresh.is_some()
            || pending_execution_intent_count > 0;
        let trigger_type = if manual {
            AGENT_CODEX_TRIGGER_TYPE_MANUAL
        } else if !pending_events.is_empty() {
            AGENT_CODEX_TRIGGER_TYPE_MESSAGE
        } else if !active_tasks.is_empty() || pending_execution_intent_count > 0 {
            AGENT_CODEX_TRIGGER_TYPE_TASK
        } else if asset_refresh.is_some() {
            AGENT_CODEX_TRIGGER_TYPE_ASSET_REFRESH
        } else {
            AGENT_CODEX_TRIGGER_TYPE_SCHEDULED
        };
        Ok(AgentCodexWorkDecision {
            should_run,
            trigger_type: trigger_type.into(),
            project,
            git,
            pending_inbox_count: pending_events.len(),
            active_task_count: active_tasks.len(),
            waiting_task_count,
            asset_refresh_due: asset_refresh.is_some(),
            pending_execution_intent_count,
            resume_existing_intents_directly,
            control_snapshot,
        })
    }

    pub fn insert_agent_codex_trigger_run(&self, run: AgentCodexTriggerRun) -> AppResult<()> {
        self.repo.insert_agent_codex_trigger_run(run)
    }

    pub fn list_agent_codex_trigger_runs(
        &self,
        agent_id: Uuid,
        limit: usize,
    ) -> Vec<AgentCodexTriggerRun> {
        self.repo
            .list_agent_codex_trigger_runs(agent_id, limit.clamp(1, 100))
    }

    pub fn has_running_agent_codex_trigger_run(&self, agent_id: Uuid) -> bool {
        self.repo.has_running_agent_codex_trigger_run(agent_id)
    }

    pub fn update_agent_codex_trigger_run(&self, run: AgentCodexTriggerRun) -> AppResult<()> {
        self.repo.update_agent_codex_trigger_run(run)
    }

    pub fn append_agent_codex_trigger_run_activity(
        &self,
        run_id: Uuid,
        activity: AgentCodexRunActivity,
        codex_thread_id: Option<String>,
    ) -> AppResult<()> {
        self.repo
            .append_agent_codex_trigger_run_activity(run_id, activity, codex_thread_id)
    }

    pub fn heartbeat_agent_codex_trigger_run(&self, run_id: Uuid) -> AppResult<()> {
        self.repo
            .heartbeat_agent_codex_trigger_run(run_id, now_utc())
    }

    pub fn watchdog_stale_agent_codex_trigger_runs(
        &self,
        stale_after_seconds: i64,
    ) -> AppResult<usize> {
        let now = now_utc();
        self.repo.watchdog_stale_agent_codex_trigger_runs(
            now,
            now - Duration::seconds(stale_after_seconds.clamp(15, 300)),
        )
    }

    pub fn complete_agent_codex_trigger_lease(
        &self,
        input: CompleteAgentCodexTriggerLeaseInput,
    ) -> AppResult<()> {
        self.repo.complete_agent_codex_trigger_lease(input)
    }

    pub fn issue_agent_codex_run_token(
        &self,
        run_id: Uuid,
        agent_id: Uuid,
        expires_at: DateTime<Utc>,
    ) -> AppResult<AgentCodexRunTokenIssue> {
        self.ensure_agent_can_act(agent_id)?;
        if expires_at <= now_utc() {
            return Err(AppError::Validation(
                "Codex run token expiry must be in the future".into(),
            ));
        }
        let plaintext_token = format!("art_{}", Uuid::new_v4().simple());
        let token = AgentCodexRunToken {
            id: Uuid::new_v4(),
            run_id,
            agent_profile_id: agent_id,
            token_hash: hash_secret(&plaintext_token),
            expires_at,
            revoked_at: None,
            created_at: now_utc(),
        };
        self.repo.insert_agent_codex_run_token(token.clone())?;
        Ok(AgentCodexRunTokenIssue {
            token,
            plaintext_token,
        })
    }

    pub fn authenticate_agent_codex_run_token(
        &self,
        plaintext_token: &str,
    ) -> AppResult<AgentProfile> {
        let token = self
            .repo
            .find_agent_codex_run_token_by_hash(&hash_secret(plaintext_token))
            .filter(|token| token.revoked_at.is_none() && now_utc() < token.expires_at)
            .ok_or_else(|| AppError::Unauthorized("invalid or expired Agent Run Token".into()))?;
        let agent = self
            .repo
            .get_agent_profile(token.agent_profile_id)
            .ok_or_else(|| AppError::Unauthorized("Agent for Run Token not found".into()))?;
        self.ensure_agent_can_act(agent.id)?;
        Ok(agent)
    }

    pub fn revoke_agent_codex_run_tokens(&self, run_id: Uuid) -> AppResult<()> {
        self.repo.revoke_agent_codex_run_tokens(run_id, now_utc())
    }

    pub fn get_agent_codex_session(
        &self,
        agent_id: Uuid,
        scope_key: &str,
    ) -> Option<AgentCodexSession> {
        self.repo.get_agent_codex_session(agent_id, scope_key)
    }

    pub fn save_agent_codex_session(&self, session: AgentCodexSession) -> AppResult<()> {
        self.repo.save_agent_codex_session(session)
    }

    pub fn list_agent_codex_sessions(
        &self,
        agent_id: Uuid,
        limit: usize,
    ) -> Vec<AgentCodexSession> {
        self.repo
            .list_agent_codex_sessions(agent_id, limit.clamp(1, 100))
    }

    pub fn create_agent_execution_intent(
        &self,
        mut intent: AgentExecutionIntent,
    ) -> AppResult<AgentExecutionIntent> {
        normalize_execution_capabilities(&mut intent.required_capabilities)?;
        self.ensure_agent_can_act(intent.agent_profile_id)?;
        let membership = self.get_active_company_agent_membership(intent.agent_profile_id)?;
        if membership.company_id != intent.company_id {
            return Err(AppError::Unauthorized(
                "Agent does not belong to the execution intent company".into(),
            ));
        }
        let project = self.ensure_company_project_access(
            intent.company_id,
            intent.project_id,
            intent.agent_profile_id,
        )?;
        if let Some(existing) = self
            .repo
            .find_agent_execution_intent_by_dedupe_key(intent.agent_profile_id, &intent.dedupe_key)
        {
            return resolve_deduplicated_execution_intent(existing, &intent);
        }
        self.ensure_project_not_paused(&project)?;
        match self.repo.insert_agent_execution_intent(intent.clone()) {
            Ok(()) => Ok(intent),
            Err(AppError::Conflict(_)) => {
                let existing = self
                    .repo
                    .find_agent_execution_intent_by_dedupe_key(
                        intent.agent_profile_id,
                        &intent.dedupe_key,
                    )
                    .ok_or_else(|| {
                        AppError::Conflict(
                            "execution intent could not be created because its dedupe key is already in use"
                                .into(),
                        )
                    })?;
                resolve_deduplicated_execution_intent(existing, &intent)
            }
            Err(error) => Err(error),
        }
    }

    pub fn requeue_agent_execution_intent_after_retryable_failure(
        &self,
        mut intent: AgentExecutionIntent,
        failure_message: &str,
    ) -> AppResult<Option<AgentExecutionIntent>> {
        if !matches!(
            intent.status.as_str(),
            AGENT_EXECUTION_INTENT_STATUS_RUNNING | AGENT_EXECUTION_INTENT_STATUS_FAILED
        ) || !is_retryable_codex_execution_failure(failure_message)
        {
            return Ok(None);
        }
        intent.status = AGENT_EXECUTION_INTENT_STATUS_PENDING.into();
        intent.claimed_at = None;
        intent.completed_at = None;
        intent.error_message = Some(format!(
            "上次执行遇到临时服务故障，Relay 将自动重试：{}",
            truncate_execution_failure(failure_message, 800)
        ));
        self.repo.update_agent_execution_intent(intent.clone())?;
        Ok(Some(intent))
    }

    pub fn get_agent_project_git_config(
        &self,
        agent_id: Uuid,
        company_id: Uuid,
        project_id: Uuid,
    ) -> AppResult<CompanyProjectGitConfig> {
        self.ensure_company_project_access(company_id, project_id, agent_id)?;
        self.repo
            .get_company_project_git_config(project_id)
            .ok_or_else(|| AppError::NotFound("project Git configuration not found".into()))
    }

    pub fn list_agent_execution_intents(
        &self,
        agent_id: Uuid,
        status: Option<&str>,
        limit: usize,
    ) -> Vec<AgentExecutionIntent> {
        self.repo
            .list_agent_execution_intents(agent_id, status, limit.clamp(1, 100))
    }

    pub fn get_agent_execution_intent(&self, intent_id: Uuid) -> Option<AgentExecutionIntent> {
        self.repo.get_agent_execution_intent(intent_id)
    }

    pub fn request_agent_execution_intent_capability(
        &self,
        agent_id: Uuid,
        company_id: Uuid,
        intent_id: Uuid,
        capability: &str,
    ) -> AppResult<AgentExecutionIntent> {
        let capability = capability.trim().to_ascii_lowercase();
        if capability != AGENT_EXECUTION_CAPABILITY_BROWSER {
            return Err(AppError::Validation(
                "execution capability must currently be browser".into(),
            ));
        }
        let mut intent = self
            .repo
            .get_agent_execution_intent(intent_id)
            .ok_or_else(|| AppError::NotFound("execution intent not found".into()))?;
        if intent.agent_profile_id != agent_id || intent.company_id != company_id {
            return Err(AppError::Unauthorized(
                "execution intent does not belong to this Agent and company".into(),
            ));
        }
        if !matches!(
            intent.status.as_str(),
            AGENT_EXECUTION_INTENT_STATUS_PENDING | AGENT_EXECUTION_INTENT_STATUS_RUNNING
        ) {
            return Err(AppError::Conflict(
                "execution capability can only be requested for pending or running work".into(),
            ));
        }
        if !intent.required_capabilities.contains(&capability) {
            intent.required_capabilities.push(capability);
            normalize_execution_capabilities(&mut intent.required_capabilities)?;
            self.repo.update_agent_execution_intent(intent.clone())?;
        }
        Ok(intent)
    }

    pub fn update_agent_execution_intent(&self, intent: AgentExecutionIntent) -> AppResult<()> {
        self.repo.update_agent_execution_intent(intent)
    }

    pub fn list_company_realtime_events_for_human(
        &self,
        human_user_id: Uuid,
        company_id: Uuid,
        after_sequence_id: i64,
        limit: usize,
    ) -> AppResult<Vec<CompanyRealtimeEvent>> {
        self.repo
            .get_company_human_member_result(company_id, human_user_id)?
            .filter(|membership| membership.status == "active")
            .ok_or_else(|| {
                AppError::Unauthorized("human user is not an active company member".into())
            })?;
        self.read_company_realtime_events(company_id, after_sequence_id, limit)
    }

    pub fn list_company_realtime_events_for_agent(
        &self,
        actor_agent_id: Uuid,
        company_id: Uuid,
        after_sequence_id: i64,
        limit: usize,
    ) -> AppResult<Vec<CompanyRealtimeEvent>> {
        self.ensure_agent_can_act(actor_agent_id)?;
        self.ensure_active_company_conversation_member(company_id, actor_agent_id)?;
        self.read_company_realtime_events(company_id, after_sequence_id, limit)
    }

    pub fn read_company_realtime_events(
        &self,
        company_id: Uuid,
        after_sequence_id: i64,
        limit: usize,
    ) -> AppResult<Vec<CompanyRealtimeEvent>> {
        self.repo
            .get_company_result(company_id)?
            .ok_or_else(|| AppError::NotFound("company not found".into()))?;
        self.repo.list_company_realtime_events_result(
            company_id,
            after_sequence_id.max(0),
            limit.clamp(1, 500),
        )
    }

    pub fn latest_company_realtime_sequence(&self, company_id: Uuid) -> AppResult<i64> {
        self.repo
            .get_company_result(company_id)?
            .ok_or_else(|| AppError::NotFound("company not found".into()))?;
        self.repo
            .latest_company_realtime_sequence_result(company_id)
    }
}

fn resolve_deduplicated_execution_intent(
    existing: AgentExecutionIntent,
    requested: &AgentExecutionIntent,
) -> AppResult<AgentExecutionIntent> {
    let same_work = existing.company_id == requested.company_id
        && existing.project_id == requested.project_id
        && same_uuid_members(&existing.task_ids, &requested.task_ids)
        && existing.action_type == requested.action_type
        && existing.objective == requested.objective
        && existing.acceptance_criteria == requested.acceptance_criteria
        && existing.required_capabilities == requested.required_capabilities
        && existing.priority == requested.priority;
    if same_work {
        return Ok(existing);
    }
    Err(AppError::Conflict(format!(
        "dedupe_key '{}' already belongs to execution intent {} (status: {}); inspect that intent or use a new dedupe_key for different work",
        requested.dedupe_key, existing.id, existing.status
    )))
}

fn normalize_execution_capabilities(capabilities: &mut Vec<String>) -> AppResult<()> {
    for capability in capabilities.iter_mut() {
        *capability = capability.trim().to_ascii_lowercase();
    }
    capabilities.retain(|capability| !capability.is_empty());
    capabilities.sort();
    capabilities.dedup();
    if capabilities
        .iter()
        .any(|capability| capability != AGENT_EXECUTION_CAPABILITY_BROWSER)
    {
        return Err(AppError::Validation(
            "execution capabilities currently support only browser".into(),
        ));
    }
    Ok(())
}

fn same_uuid_members(left: &[Uuid], right: &[Uuid]) -> bool {
    left.iter().copied().collect::<HashSet<_>>() == right.iter().copied().collect::<HashSet<_>>()
}

fn is_retryable_codex_execution_failure(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    [
        "auth_unavailable",
        "no auth available",
        "service unavailable",
        "temporarily unavailable",
        "too many requests",
        "rate limit",
        "bad gateway",
        "gateway timeout",
        "unexpected status 502",
        "unexpected status 503",
        "unexpected status 504",
        "connection reset",
        "connection refused",
        "connection closed",
        "connection timed out",
        "network is unreachable",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

fn truncate_execution_failure(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn sanitize_codex_session_for_human(mut session: AgentCodexSession) -> AgentCodexSession {
    session.summary_short = redact_host_home_path(&session.summary_short);
    redact_json_host_paths(&mut session.checkpoint_json);
    session
}

fn sanitize_codex_run_for_human(mut run: AgentCodexTriggerRun) -> AgentCodexTriggerRun {
    if let Some(summary) = run.final_message_summary.as_mut() {
        *summary = redact_host_home_path(summary);
    }
    if let Some(error) = run.error_message.as_mut() {
        *error = redact_host_home_path(error);
    }
    if let Some(summary) = run.activity_summary.as_mut() {
        *summary = redact_host_home_path(summary);
    }
    for activity in &mut run.activity_log {
        activity.summary = redact_host_home_path(&activity.summary);
    }
    run
}

fn redact_json_host_paths(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(text) => *text = redact_host_home_path(text),
        serde_json::Value::Array(items) => {
            for item in items {
                redact_json_host_paths(item);
            }
        }
        serde_json::Value::Object(fields) => {
            for value in fields.values_mut() {
                redact_json_host_paths(value);
            }
        }
        _ => {}
    }
}

fn redact_host_home_path(value: &str) -> String {
    let redacted = redact_unix_home_segment(&redact_unix_home_segment(value, "/Users/"), "/home/");
    redact_relay_workspace_path(&redacted)
}

fn redact_relay_workspace_path(value: &str) -> String {
    let mut output = value.to_string();
    let mut search_from = 0;
    while let Some(relative_start) = output[search_from..].find(".relay-workspace/") {
        let marker_start = search_from + relative_start;
        let path_start = output[..marker_start]
            .rfind(['(', ' ', '\n', '\t'])
            .map_or(0, |index| index + 1);
        let Some(relative_worktree_end) = output[marker_start..].find("/.relay/worktrees/") else {
            search_from = marker_start + 1;
            continue;
        };
        let worktree_marker = marker_start + relative_worktree_end + "/.relay/worktrees/".len();
        let Some(relative_agent_end) = output[worktree_marker..].find('/') else {
            break;
        };
        let repository_path_start = worktree_marker + relative_agent_end + 1;
        output.replace_range(path_start..repository_path_start, "./");
        search_from = path_start + 2;
    }
    output
}

fn redact_unix_home_segment(value: &str, prefix: &str) -> String {
    let mut output = value.to_string();
    let mut search_from = 0;
    while let Some(relative_start) = output[search_from..].find(prefix) {
        let start = search_from + relative_start;
        let username_start = start + prefix.len();
        let Some(relative_end) = output[username_start..].find('/') else {
            break;
        };
        let end = username_start + relative_end;
        output.replace_range(start..end, "~");
        search_from = start + 1;
    }
    output
}

#[cfg(test)]
mod runtime_projection_tests {
    use super::redact_host_home_path;

    #[test]
    fn relay_worktree_links_are_exposed_as_project_relative_paths() {
        let value = "[evidence](/Users/alice/work/relay/.relay-workspace/companies/company/project/.relay/worktrees/agent/docs/evidence/report.md)";
        assert_eq!(
            redact_host_home_path(value),
            "[evidence](./docs/evidence/report.md)"
        );
    }
}
