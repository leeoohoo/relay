use super::*;
use ai_chat_domain::company::is_agent_codex_wake_reason;

impl CodexControlPlatformRepository for MemoryPlatformRepository {
    fn save_company_codex_runner_profile(
        &self,
        profile: CompanyCodexRunnerProfile,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard
            .company_codex_runner_profiles
            .values()
            .any(|existing| {
                existing.company_id == profile.company_id
                    && existing.id != profile.id
                    && existing.name.eq_ignore_ascii_case(&profile.name)
            })
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "Codex runner profile name already exists".into(),
            ));
        }
        guard
            .company_codex_runner_profiles
            .insert(profile.id, profile);
        Ok(())
    }

    fn get_company_codex_runner_profile(
        &self,
        profile_id: Uuid,
    ) -> Option<CompanyCodexRunnerProfile> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .company_codex_runner_profiles
            .get(&profile_id)
            .cloned()
    }

    fn list_company_codex_runner_profiles(
        &self,
        company_id: Uuid,
    ) -> Vec<CompanyCodexRunnerProfile> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut profiles = guard
            .company_codex_runner_profiles
            .values()
            .filter(|profile| profile.company_id == company_id)
            .cloned()
            .collect::<Vec<_>>();
        profiles.sort_by(|left, right| {
            right
                .is_default
                .cmp(&left.is_default)
                .then_with(|| left.name.cmp(&right.name))
        });
        profiles
    }

    fn delete_company_codex_runner_profile(&self, profile_id: Uuid) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.company_codex_runner_profiles.remove(&profile_id);
        Ok(())
    }

    fn clear_company_codex_runner_profile_defaults(
        &self,
        company_id: Uuid,
        except_profile_id: Uuid,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        for profile in guard.company_codex_runner_profiles.values_mut() {
            if profile.company_id == company_id && profile.id != except_profile_id {
                profile.is_default = false;
            }
        }
        Ok(())
    }

    fn assign_agent_codex_runner_profile(
        &self,
        agent_id: Uuid,
        profile_id: Uuid,
        _human_user_id: Uuid,
        _assigned_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard
            .company_codex_runner_profiles
            .contains_key(&profile_id)
        {
            return Err(ai_chat_shared::AppError::NotFound(
                "Codex runner profile not found".into(),
            ));
        }
        guard
            .agent_codex_runner_profile_assignments
            .insert(agent_id, profile_id);
        Ok(())
    }

    fn get_agent_codex_runner_profile_assignment(&self, agent_id: Uuid) -> Option<Uuid> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .agent_codex_runner_profile_assignments
            .get(&agent_id)
            .copied()
    }

    fn list_codex_runner_profile_agent_ids(&self, profile_id: Uuid) -> Vec<Uuid> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .agent_codex_runner_profile_assignments
            .iter()
            .filter_map(|(agent_id, assigned_profile_id)| {
                (*assigned_profile_id == profile_id).then_some(*agent_id)
            })
            .collect()
    }

    fn save_codex_plugin_catalog_snapshot(
        &self,
        snapshot: CodexPluginCatalogSnapshot,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let key = format!("{}\n{}", snapshot.runner_id, snapshot.target_selector);
        guard.codex_plugin_catalogs.insert(key, snapshot);
        Ok(())
    }

    fn list_codex_plugin_catalog_snapshots(&self) -> AppResult<Vec<CodexPluginCatalogSnapshot>> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut catalogs = guard
            .codex_plugin_catalogs
            .values()
            .cloned()
            .collect::<Vec<_>>();
        catalogs.sort_by(|left, right| right.discovered_at.cmp(&left.discovered_at));
        Ok(catalogs)
    }

    fn insert_codex_plugin_operation(&self, operation: CodexPluginOperation) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.codex_plugin_operations.values().any(|existing| {
            existing.target_runner_id == operation.target_runner_id
                && existing.target_selector == operation.target_selector
                && existing.operation == operation.operation
                && existing.plugin_id == operation.plugin_id
                && matches!(
                    existing.status.as_str(),
                    CODEX_PLUGIN_OPERATION_STATUS_QUEUED | CODEX_PLUGIN_OPERATION_STATUS_RUNNING
                )
        }) {
            return Err(ai_chat_shared::AppError::Conflict(
                "the same Codex plugin operation is already queued".into(),
            ));
        }
        guard
            .codex_plugin_operations
            .insert(operation.id, operation);
        Ok(())
    }

    fn list_company_codex_plugin_operations(
        &self,
        company_id: Uuid,
        limit: usize,
    ) -> AppResult<Vec<CodexPluginOperation>> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut operations = guard
            .codex_plugin_operations
            .values()
            .filter(|operation| operation.company_id == company_id)
            .cloned()
            .collect::<Vec<_>>();
        operations.sort_by(|left, right| right.requested_at.cmp(&left.requested_at));
        operations.truncate(limit);
        Ok(operations)
    }

    fn claim_codex_plugin_operations(
        &self,
        target_runner_id: &str,
        lease_owner: &str,
        now: chrono::DateTime<chrono::Utc>,
        limit: usize,
    ) -> AppResult<Vec<CodexPluginOperation>> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let mut ids = guard
            .codex_plugin_operations
            .values()
            .filter(|operation| operation.target_runner_id == target_runner_id)
            .filter(|operation| {
                operation.status == CODEX_PLUGIN_OPERATION_STATUS_QUEUED
                    || (operation.status == CODEX_PLUGIN_OPERATION_STATUS_RUNNING
                        && operation
                            .lease_expires_at
                            .is_some_and(|expires_at| expires_at <= now))
            })
            .map(|operation| (operation.requested_at, operation.id))
            .collect::<Vec<_>>();
        ids.sort_by_key(|(requested_at, _)| *requested_at);
        ids.truncate(limit);
        let mut claimed = Vec::new();
        for (_, id) in ids {
            if let Some(operation) = guard.codex_plugin_operations.get_mut(&id) {
                operation.status = CODEX_PLUGIN_OPERATION_STATUS_RUNNING.into();
                operation.lease_owner = Some(lease_owner.into());
                operation.lease_expires_at = Some(now + chrono::Duration::minutes(2));
                operation.attempt_count += 1;
                operation.started_at.get_or_insert(now);
                operation.updated_at = now;
                claimed.push(operation.clone());
            }
        }
        Ok(claimed)
    }

    fn finish_codex_plugin_operation(
        &self,
        operation_id: Uuid,
        lease_owner: &str,
        succeeded: bool,
        result: serde_json::Value,
        error_message: Option<String>,
        finished_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let operation = guard
            .codex_plugin_operations
            .get_mut(&operation_id)
            .ok_or_else(|| {
                ai_chat_shared::AppError::NotFound("plugin operation not found".into())
            })?;
        if operation.lease_owner.as_deref() != Some(lease_owner) {
            return Err(ai_chat_shared::AppError::Conflict(
                "plugin operation lease is no longer owned by this Trigger".into(),
            ));
        }
        operation.result = result;
        operation.error_message = error_message;
        operation.lease_owner = None;
        operation.lease_expires_at = None;
        operation.updated_at = finished_at;
        if succeeded {
            operation.status = CODEX_PLUGIN_OPERATION_STATUS_SUCCEEDED.into();
            operation.finished_at = Some(finished_at);
        } else if operation.attempt_count < 3 {
            operation.status = CODEX_PLUGIN_OPERATION_STATUS_QUEUED.into();
        } else {
            operation.status = CODEX_PLUGIN_OPERATION_STATUS_FAILED.into();
            operation.finished_at = Some(finished_at);
        }
        Ok(())
    }
}

impl CodexRuntimePlatformRepository for MemoryPlatformRepository {
    fn save_agent_codex_trigger_config(&self, config: AgentCodexTriggerConfig) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.agent_profiles.contains_key(&config.agent_profile_id) {
            return Err(ai_chat_shared::AppError::NotFound("Agent not found".into()));
        }
        guard
            .agent_codex_trigger_configs
            .insert(config.agent_profile_id, config);
        Ok(())
    }

    fn get_agent_codex_trigger_config_by_agent(
        &self,
        agent_id: Uuid,
    ) -> Option<AgentCodexTriggerConfig> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.agent_codex_trigger_configs.get(&agent_id).cloned()
    }

    fn claim_due_agent_codex_trigger_configs(
        &self,
        lease_owner: &str,
        now: chrono::DateTime<chrono::Utc>,
        limit: usize,
    ) -> AppResult<Vec<AgentCodexTriggerConfig>> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let stale_run_ids = guard
            .agent_codex_trigger_runs
            .values()
            .filter(|run| run.status == AGENT_CODEX_RUN_STATUS_RUNNING)
            .filter_map(|run| {
                let config = guard
                    .agent_codex_trigger_configs
                    .get(&run.agent_profile_id)?;
                (run.started_at + chrono::Duration::seconds(i64::from(config.max_run_seconds) + 60)
                    <= now)
                    .then_some(run.id)
            })
            .collect::<Vec<_>>();
        for run_id in stale_run_ids {
            if let Some(run) = guard.agent_codex_trigger_runs.get_mut(&run_id) {
                run.status = AGENT_CODEX_RUN_STATUS_LEASE_LOST.into();
                run.finished_at = Some(now);
                run.error_message =
                    Some("Codex trigger process stopped before the run completed".into());
                run.activity_phase = "lease_lost".into();
                run.activity_summary = Some("Trigger 进程中断，本轮已停止".into());
                run.last_activity_at = Some(now);
            }
        }
        let running_agent_ids = guard
            .agent_codex_trigger_runs
            .values()
            .filter(|run| run.status == AGENT_CODEX_RUN_STATUS_RUNNING)
            .map(|run| run.agent_profile_id)
            .collect::<std::collections::HashSet<_>>();
        for intent in guard.agent_execution_intents.values_mut().filter(|intent| {
            intent.status == AGENT_EXECUTION_INTENT_STATUS_RUNNING
                && !running_agent_ids.contains(&intent.agent_profile_id)
        }) {
            intent.status = AGENT_EXECUTION_INTENT_STATUS_PENDING.into();
            intent.worker_session_id = None;
            intent.claimed_at = None;
            intent.completed_at = None;
            intent.error_message = None;
        }
        let logged_in_human_ids = guard
            .human_sessions
            .values()
            .filter(|session| {
                session.revoked_at.is_none()
                    && session.expires_at > now
                    && session.last_used_at.unwrap_or(session.created_at)
                        > now - chrono::Duration::seconds(60)
            })
            .map(|session| session.human_user_id)
            .collect::<std::collections::HashSet<_>>();
        let logged_in_company_ids = guard
            .company_human_members
            .values()
            .filter(|membership| {
                membership.status == "active"
                    && logged_in_human_ids.contains(&membership.human_user_id)
            })
            .map(|membership| membership.company_id)
            .collect::<std::collections::HashSet<_>>();
        let mut agent_ids = guard
            .agent_codex_trigger_configs
            .iter()
            .filter(|(_, config)| {
                config.status == AGENT_CODEX_TRIGGER_STATUS_ACTIVE
                    && config.next_run_at <= now
                    && logged_in_company_ids.contains(&config.company_id)
                    && !running_agent_ids.contains(&config.agent_profile_id)
                    && config
                        .lease_expires_at
                        .is_none_or(|lease_expires_at| lease_expires_at <= now)
            })
            .map(|(agent_id, config)| (*agent_id, config.next_run_at))
            .collect::<Vec<_>>();
        agent_ids.sort_by_key(|(_, next_run_at)| *next_run_at);
        agent_ids.truncate(limit);
        Ok(agent_ids
            .into_iter()
            .filter_map(|(agent_id, _)| {
                let config = guard.agent_codex_trigger_configs.get_mut(&agent_id)?;
                config.lease_owner = Some(lease_owner.to_string());
                config.lease_expires_at =
                    Some(now + chrono::Duration::seconds(i64::from(config.max_run_seconds) + 60));
                config.last_run_at = Some(now);
                config.updated_at = now;
                Some(config.clone())
            })
            .collect())
    }

    fn abandon_agent_codex_trigger_leases(
        &self,
        lease_owner: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let abandoned_agent_ids = guard
            .agent_codex_trigger_configs
            .iter_mut()
            .filter_map(|(agent_id, config)| {
                (config.lease_owner.as_deref() == Some(lease_owner)).then(|| {
                    config.lease_owner = None;
                    config.lease_expires_at = None;
                    config.next_run_at = config.next_run_at.min(now);
                    config.updated_at = now;
                    *agent_id
                })
            })
            .collect::<std::collections::HashSet<_>>();
        let mut abandoned_runs = 0;
        for run in guard.agent_codex_trigger_runs.values_mut().filter(|run| {
            run.status == AGENT_CODEX_RUN_STATUS_RUNNING
                && abandoned_agent_ids.contains(&run.agent_profile_id)
        }) {
            run.status = AGENT_CODEX_RUN_STATUS_RESTARTED.into();
            run.finished_at = Some(now);
            run.error_message = None;
            run.activity_phase = "continuing".into();
            run.activity_summary = Some("Trigger 服务重启，本轮工作已保存并等待接续".into());
            run.last_activity_at = Some(now);
            abandoned_runs += 1;
        }
        for intent in guard.agent_execution_intents.values_mut().filter(|intent| {
            intent.status == AGENT_EXECUTION_INTENT_STATUS_RUNNING
                && abandoned_agent_ids.contains(&intent.agent_profile_id)
        }) {
            intent.status = AGENT_EXECUTION_INTENT_STATUS_PENDING.into();
            intent.claimed_at = None;
            intent.completed_at = None;
            intent.error_message = None;
        }
        Ok(abandoned_runs)
    }

    fn request_agent_codex_trigger_wake(
        &self,
        agent_id: Uuid,
        requested_at: chrono::DateTime<chrono::Utc>,
        reason: &str,
    ) -> AppResult<bool> {
        if !is_agent_codex_wake_reason(reason) {
            return Err(ai_chat_shared::AppError::Validation(format!(
                "unsupported Codex trigger wake reason: {reason}"
            )));
        }
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let Some(config) = guard.agent_codex_trigger_configs.get_mut(&agent_id) else {
            return Ok(false);
        };
        if config.status != AGENT_CODEX_TRIGGER_STATUS_ACTIVE {
            return Ok(false);
        }
        config.next_run_at = config.next_run_at.min(requested_at);
        config.wake_requested_at = Some(
            config
                .wake_requested_at
                .map_or(requested_at, |existing| existing.max(requested_at)),
        );
        config.wake_reason = Some(reason.to_string());
        config.updated_at = requested_at;
        Ok(true)
    }

    fn complete_agent_codex_trigger_lease(
        &self,
        input: CompleteAgentCodexTriggerLeaseInput,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let config = guard
            .agent_codex_trigger_configs
            .values_mut()
            .find(|config| {
                config.id == input.trigger_config_id
                    && config.lease_owner.as_deref() == Some(input.lease_owner.as_str())
            })
            .ok_or_else(|| {
                ai_chat_shared::AppError::Conflict("Codex trigger lease was lost".into())
            })?;
        let has_new_wake = config
            .wake_requested_at
            .zip(config.last_run_at)
            .is_some_and(|(wake_requested_at, claimed_at)| wake_requested_at > claimed_at);
        let has_new_manual = config
            .manual_run_requested_at
            .zip(config.last_run_at)
            .is_some_and(|(requested_at, claimed_at)| requested_at > claimed_at);
        config.lease_owner = None;
        config.lease_expires_at = None;
        if has_new_wake || has_new_manual {
            config.next_run_at = [
                has_new_wake.then_some(config.wake_requested_at).flatten(),
                has_new_manual
                    .then_some(config.manual_run_requested_at)
                    .flatten(),
                Some(input.next_run_at),
            ]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(input.next_run_at);
        } else {
            config.next_run_at = input.next_run_at;
            config.wake_requested_at = None;
            config.wake_reason = None;
        }
        if !has_new_manual {
            config.manual_run_requested_at = None;
        }
        config.last_run_at = Some(input.finished_at);
        config.updated_at = input.finished_at;
        if input.succeeded {
            config.last_success_at = Some(input.finished_at);
            config.last_error = None;
            config.consecutive_failure_count = 0;
        } else {
            config.last_error = input.error_message;
            config.consecutive_failure_count += 1;
            if config.consecutive_failure_count >= 3
                && config.status == AGENT_CODEX_TRIGGER_STATUS_ACTIVE
            {
                config.status = AGENT_CODEX_TRIGGER_STATUS_ERROR.into();
            }
        }
        Ok(())
    }

    fn insert_agent_codex_trigger_run(&self, run: AgentCodexTriggerRun) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.agent_codex_trigger_runs.contains_key(&run.id) {
            return Err(ai_chat_shared::AppError::Conflict(
                "Codex trigger run already exists".into(),
            ));
        }
        if run.status == AGENT_CODEX_RUN_STATUS_RUNNING
            && guard.agent_codex_trigger_runs.values().any(|existing| {
                existing.agent_profile_id == run.agent_profile_id
                    && existing.status == AGENT_CODEX_RUN_STATUS_RUNNING
            })
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "Agent already has a running Codex trigger run".into(),
            ));
        }
        guard.agent_codex_trigger_runs.insert(run.id, run);
        Ok(())
    }

    fn has_running_agent_codex_trigger_run(&self, agent_id: Uuid) -> bool {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.agent_codex_trigger_runs.values().any(|run| {
            run.agent_profile_id == agent_id && run.status == AGENT_CODEX_RUN_STATUS_RUNNING
        })
    }

    fn update_agent_codex_trigger_run(&self, mut run: AgentCodexTriggerRun) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let Some(existing) = guard.agent_codex_trigger_runs.get(&run.id) else {
            return Err(ai_chat_shared::AppError::NotFound(
                "Codex trigger run not found".into(),
            ));
        };
        run.activity_phase = existing.activity_phase.clone();
        run.activity_summary = existing.activity_summary.clone();
        run.last_activity_at = existing.last_activity_at;
        run.activity_log = existing.activity_log.clone();
        guard.agent_codex_trigger_runs.insert(run.id, run);
        Ok(())
    }

    fn append_agent_codex_trigger_run_activity(
        &self,
        run_id: Uuid,
        activity: AgentCodexRunActivity,
        codex_thread_id: Option<String>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let run = guard
            .agent_codex_trigger_runs
            .get_mut(&run_id)
            .ok_or_else(|| {
                ai_chat_shared::AppError::NotFound("Codex trigger run not found".into())
            })?;
        run.activity_phase = activity.phase.clone();
        run.activity_summary = Some(activity.summary.clone());
        run.last_activity_at = Some(activity.at);
        run.heartbeat_at = Some(activity.at);
        run.state_reason = Some(activity.summary.clone());
        if codex_thread_id.is_some() {
            run.codex_thread_id = codex_thread_id;
        }
        run.activity_log.push(activity);
        if run.activity_log.len() > 40 {
            let excess = run.activity_log.len() - 40;
            run.activity_log.drain(0..excess);
        }
        Ok(())
    }

    fn heartbeat_agent_codex_trigger_run(
        &self,
        run_id: Uuid,
        heartbeat_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let run = guard
            .agent_codex_trigger_runs
            .get_mut(&run_id)
            .filter(|run| run.status == AGENT_CODEX_RUN_STATUS_RUNNING)
            .ok_or_else(|| {
                ai_chat_shared::AppError::Conflict("Codex trigger run is no longer running".into())
            })?;
        run.heartbeat_at = Some(heartbeat_at);
        Ok(())
    }

    fn watchdog_stale_agent_codex_trigger_runs(
        &self,
        now: chrono::DateTime<chrono::Utc>,
        stale_before: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let stale_agent_ids = guard
            .agent_codex_trigger_runs
            .values_mut()
            .filter(|run| run.status == AGENT_CODEX_RUN_STATUS_RUNNING)
            .filter(|run| latest_run_liveness(run) < stale_before)
            .map(|run| {
                run.status = AGENT_CODEX_RUN_STATUS_LEASE_LOST.into();
                run.finished_at = Some(now);
                run.activity_phase = "lease_lost".into();
                run.activity_summary = Some("运行心跳已停止，Watchdog 已回收本轮".into());
                run.state_reason = Some("run_heartbeat_lost: Trigger 进程没有继续报告心跳".into());
                run.error_message =
                    Some("run_heartbeat_lost: Trigger process heartbeat stopped".into());
                run.agent_profile_id
            })
            .collect::<std::collections::HashSet<_>>();
        for agent_id in &stale_agent_ids {
            if let Some(config) = guard.agent_codex_trigger_configs.get_mut(agent_id) {
                config.lease_owner = None;
                config.lease_expires_at = None;
                config.next_run_at = config.next_run_at.min(now);
                config.updated_at = now;
            }
        }
        for intent in guard.agent_execution_intents.values_mut().filter(|intent| {
            intent.status == AGENT_EXECUTION_INTENT_STATUS_RUNNING
                && stale_agent_ids.contains(&intent.agent_profile_id)
        }) {
            intent.status = AGENT_EXECUTION_INTENT_STATUS_PENDING.into();
            intent.claimed_at = None;
            intent.completed_at = None;
            intent.error_message = Some("上一个运行心跳丢失，Relay 已安排从原项目会话恢复".into());
        }
        Ok(stale_agent_ids.len())
    }

    fn list_agent_codex_trigger_runs(
        &self,
        agent_id: Uuid,
        limit: usize,
    ) -> Vec<AgentCodexTriggerRun> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut runs = guard
            .agent_codex_trigger_runs
            .values()
            .filter(|run| run.agent_profile_id == agent_id)
            .cloned()
            .collect::<Vec<_>>();
        runs.sort_by(|left, right| right.started_at.cmp(&left.started_at));
        runs.truncate(limit);
        runs
    }

    fn save_agent_codex_session(&self, session: AgentCodexSession) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.agent_codex_sessions.insert(
            (session.agent_profile_id, session.scope_key.clone()),
            session,
        );
        Ok(())
    }

    fn get_agent_codex_session(
        &self,
        agent_id: Uuid,
        scope_key: &str,
    ) -> Option<AgentCodexSession> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .agent_codex_sessions
            .get(&(agent_id, scope_key.to_string()))
            .cloned()
    }

    fn list_agent_codex_sessions(&self, agent_id: Uuid, limit: usize) -> Vec<AgentCodexSession> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut sessions = guard
            .agent_codex_sessions
            .values()
            .filter(|session| session.agent_profile_id == agent_id)
            .cloned()
            .collect::<Vec<_>>();
        sessions.sort_by(|left, right| right.last_used_at.cmp(&left.last_used_at));
        sessions.truncate(limit);
        sessions
    }

    fn insert_agent_execution_intent(&self, intent: AgentExecutionIntent) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.agent_execution_intents.values().any(|existing| {
            existing.agent_profile_id == intent.agent_profile_id
                && existing.dedupe_key == intent.dedupe_key
        }) {
            return Err(ai_chat_shared::AppError::Conflict(
                "Agent execution intent dedupe key already exists".into(),
            ));
        }
        guard.agent_execution_intents.insert(intent.id, intent);
        Ok(())
    }

    fn update_agent_execution_intent(&self, intent: AgentExecutionIntent) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.agent_execution_intents.contains_key(&intent.id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "Agent execution intent not found".into(),
            ));
        }
        guard.agent_execution_intents.insert(intent.id, intent);
        Ok(())
    }

    fn get_agent_execution_intent(&self, intent_id: Uuid) -> Option<AgentExecutionIntent> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.agent_execution_intents.get(&intent_id).cloned()
    }

    fn find_agent_execution_intent_by_dedupe_key(
        &self,
        agent_id: Uuid,
        dedupe_key: &str,
    ) -> Option<AgentExecutionIntent> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .agent_execution_intents
            .values()
            .find(|intent| intent.agent_profile_id == agent_id && intent.dedupe_key == dedupe_key)
            .cloned()
    }

    fn list_agent_execution_intents(
        &self,
        agent_id: Uuid,
        status: Option<&str>,
        limit: usize,
    ) -> Vec<AgentExecutionIntent> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut intents = guard
            .agent_execution_intents
            .values()
            .filter(|intent| intent.agent_profile_id == agent_id)
            .filter(|intent| status.is_none_or(|status| intent.status == status))
            .cloned()
            .collect::<Vec<_>>();
        intents.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        intents.truncate(limit);
        intents
    }

    fn insert_agent_codex_run_token(&self, token: AgentCodexRunToken) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.agent_codex_run_tokens.contains_key(&token.token_hash) {
            return Err(ai_chat_shared::AppError::Conflict(
                "Codex run token already exists".into(),
            ));
        }
        guard
            .agent_codex_run_tokens
            .insert(token.token_hash.clone(), token);
        Ok(())
    }

    fn find_agent_codex_run_token_by_hash(&self, token_hash: &str) -> Option<AgentCodexRunToken> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.agent_codex_run_tokens.get(token_hash).cloned()
    }

    fn revoke_agent_codex_run_tokens(
        &self,
        run_id: Uuid,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        for token in guard.agent_codex_run_tokens.values_mut() {
            if token.run_id == run_id && token.revoked_at.is_none() {
                token.revoked_at = Some(revoked_at);
            }
        }
        Ok(())
    }

    fn delete_expired_agent_codex_run_tokens(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let before = guard.agent_codex_run_tokens.len();
        guard.agent_codex_run_tokens.retain(|_, token| {
            token.expires_at > now && token.revoked_at.is_none_or(|revoked_at| revoked_at > now)
        });
        Ok(before - guard.agent_codex_run_tokens.len())
    }
}

fn latest_run_liveness(run: &AgentCodexTriggerRun) -> chrono::DateTime<chrono::Utc> {
    match (run.heartbeat_at, run.last_activity_at) {
        (Some(heartbeat), Some(activity)) => heartbeat.max(activity),
        (Some(heartbeat), None) => heartbeat,
        (None, Some(activity)) => activity,
        (None, None) => run.started_at,
    }
}
