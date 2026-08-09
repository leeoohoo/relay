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
        Ok(CompanyAgentCodexTriggerView {
            recent_runs: self
                .repo
                .list_agent_codex_trigger_runs_result(input.agent_id, 20)?,
            runner_profile_id: self
                .repo
                .get_agent_codex_runner_profile_assignment(input.agent_id),
            config,
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
        Ok(self
            .repo
            .list_agent_codex_trigger_runs(input.agent_id, input.limit.clamp(1, 100)))
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
            .get_company_agent_membership(input.agent_id)
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
            .get_company_agent_membership(config.agent_profile_id)
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
        let mut pending_events = Vec::new();
        for event in self.repo.list_agent_inbox_events(
            config.agent_profile_id,
            Some(AgentInboxEventStatus::Pending),
            1_000,
        ) {
            if event.available_at > now {
                continue;
            }
            let mut project_id = payload_uuid_field_optional(&event.payload_json, "project_id");
            if project_id.is_none() {
                if let Some(conversation_id) =
                    payload_uuid_field_optional(&event.payload_json, "conversation_id")
                {
                    project_id = self
                        .repo
                        .get_conversation_context_result(conversation_id)?
                        .and_then(|context| context.project_id);
                }
            }
            let project_is_active = match project_id {
                Some(project_id) => self
                    .repo
                    .get_company_project_result(project_id)?
                    .is_none_or(|project| project.status != PROJECT_STATUS_PAUSED),
                None => true,
            };
            if project_is_active {
                pending_events.push(event);
            }
        }
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
        let mut active_tasks = Vec::new();
        let mut waiting_task_count = 0;
        for project in &projects {
            let tasks = self.repo.list_company_project_tasks_result(project.id)?;
            let dependencies = self.repo.list_company_project_task_dependencies(project.id);
            for task in tasks.iter().filter(|task| {
                task.assignee_agent_id == Some(config.agent_profile_id)
                    && matches!(
                        task.status.as_str(),
                        PROJECT_TASK_STATUS_TODO | PROJECT_TASK_STATUS_IN_PROGRESS
                    )
            }) {
                let has_unresolved_dependency = dependencies
                    .iter()
                    .filter(|dependency| dependency.task_id == task.id)
                    .any(|dependency| {
                        tasks
                            .iter()
                            .find(|candidate| candidate.id == dependency.depends_on_task_id)
                            .is_some_and(|dependency_task| {
                                !matches!(
                                    dependency_task.status.as_str(),
                                    PROJECT_TASK_STATUS_DONE | PROJECT_TASK_STATUS_CANCELLED
                                )
                            })
                    });
                if has_unresolved_dependency {
                    waiting_task_count += 1;
                } else {
                    active_tasks.push(task.clone());
                }
            }
        }
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
        let pending_execution_intent_count = self
            .repo
            .list_agent_execution_intents(
                config.agent_profile_id,
                Some(AGENT_EXECUTION_INTENT_STATUS_PENDING),
                100,
            )
            .len();
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
        })
    }

    pub fn insert_agent_codex_trigger_run(&self, run: AgentCodexTriggerRun) -> AppResult<()> {
        self.repo.insert_agent_codex_trigger_run(run)
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
        intent: AgentExecutionIntent,
    ) -> AppResult<AgentExecutionIntent> {
        self.ensure_agent_can_act(intent.agent_profile_id)?;
        let membership = self.get_active_company_agent_membership(intent.agent_profile_id)?;
        if membership.company_id != intent.company_id {
            return Err(AppError::Unauthorized(
                "Agent does not belong to the execution intent company".into(),
            ));
        }
        self.ensure_company_project_access(
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
        && existing.priority == requested.priority;
    if same_work {
        return Ok(existing);
    }
    Err(AppError::Conflict(format!(
        "dedupe_key '{}' already belongs to execution intent {} (status: {}); inspect that intent or use a new dedupe_key for different work",
        requested.dedupe_key, existing.id, existing.status
    )))
}

fn same_uuid_members(left: &[Uuid], right: &[Uuid]) -> bool {
    left.iter().copied().collect::<HashSet<_>>() == right.iter().copied().collect::<HashSet<_>>()
}

fn sanitize_codex_session_for_human(mut session: AgentCodexSession) -> AgentCodexSession {
    session.summary_short = redact_host_home_path(&session.summary_short);
    redact_json_host_paths(&mut session.checkpoint_json);
    session
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
    redact_unix_home_segment(&redact_unix_home_segment(value, "/Users/"), "/home/")
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
