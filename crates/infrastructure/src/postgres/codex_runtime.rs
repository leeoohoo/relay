use super::mapping::*;
use super::*;
use ai_chat_domain::company::is_agent_codex_wake_reason;

impl CodexRuntimePlatformRepository for PostgresPlatformRepository {
    fn save_agent_codex_trigger_config(&self, config: AgentCodexTriggerConfig) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_codex_trigger_configs (
                    id, company_id, agent_profile_id, status, interval_seconds,
                    codex_profile, model, reasoning_effort, reasoning_summary, verbosity,
                    personality, service_tier, sandbox_mode, approval_policy, network_access,
                    web_search, feature_multi_agent, feature_remote_plugin, feature_hooks,
                    feature_goals, feature_shell_tool, max_run_seconds, next_run_at,
                    lease_owner, lease_expires_at, manual_run_requested_at,
                    last_run_at, last_success_at, last_error,
                    consecutive_failure_count, created_by_human_user_id,
                    updated_by_human_user_id, created_at, updated_at,
                    wake_requested_at, wake_reason
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                        $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                        $21, $22, $23, $24, $25, $26, $27, $28, $29, $30,
                        $31, $32, $33, $34, $35, $36)
                ON CONFLICT (agent_profile_id) DO UPDATE
                SET company_id = EXCLUDED.company_id,
                    status = EXCLUDED.status,
                    interval_seconds = EXCLUDED.interval_seconds,
                    codex_profile = EXCLUDED.codex_profile,
                    model = EXCLUDED.model,
                    reasoning_effort = EXCLUDED.reasoning_effort,
                    reasoning_summary = EXCLUDED.reasoning_summary,
                    verbosity = EXCLUDED.verbosity,
                    personality = EXCLUDED.personality,
                    service_tier = EXCLUDED.service_tier,
                    sandbox_mode = EXCLUDED.sandbox_mode,
                    approval_policy = EXCLUDED.approval_policy,
                    network_access = EXCLUDED.network_access,
                    web_search = EXCLUDED.web_search,
                    feature_multi_agent = EXCLUDED.feature_multi_agent,
                    feature_remote_plugin = EXCLUDED.feature_remote_plugin,
                    feature_hooks = EXCLUDED.feature_hooks,
                    feature_goals = EXCLUDED.feature_goals,
                    feature_shell_tool = EXCLUDED.feature_shell_tool,
                    max_run_seconds = EXCLUDED.max_run_seconds,
                    next_run_at = EXCLUDED.next_run_at,
                    lease_owner = EXCLUDED.lease_owner,
                    lease_expires_at = EXCLUDED.lease_expires_at,
                    manual_run_requested_at = EXCLUDED.manual_run_requested_at,
                    last_run_at = EXCLUDED.last_run_at,
                    last_success_at = EXCLUDED.last_success_at,
                    last_error = EXCLUDED.last_error,
                    consecutive_failure_count = EXCLUDED.consecutive_failure_count,
                    wake_requested_at = EXCLUDED.wake_requested_at,
                    wake_reason = EXCLUDED.wake_reason,
                    updated_by_human_user_id = EXCLUDED.updated_by_human_user_id,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &config.id,
                    &config.company_id,
                    &config.agent_profile_id,
                    &config.status,
                    &config.interval_seconds,
                    &config.codex_profile,
                    &config.model,
                    &config.reasoning_effort,
                    &config.reasoning_summary,
                    &config.verbosity,
                    &config.personality,
                    &config.service_tier,
                    &config.sandbox_mode,
                    &config.approval_policy,
                    &config.network_access,
                    &config.web_search,
                    &config.feature_multi_agent,
                    &config.feature_remote_plugin,
                    &config.feature_hooks,
                    &config.feature_goals,
                    &config.feature_shell_tool,
                    &config.max_run_seconds,
                    &config.next_run_at,
                    &config.lease_owner,
                    &config.lease_expires_at,
                    &config.manual_run_requested_at,
                    &config.last_run_at,
                    &config.last_success_at,
                    &config.last_error,
                    &config.consecutive_failure_count,
                    &config.created_by_human_user_id,
                    &config.updated_by_human_user_id,
                    &config.created_at,
                    &config.updated_at,
                    &config.wake_requested_at,
                    &config.wake_reason,
                ],
            )?;
            Ok(())
        })
    }

    fn get_agent_codex_trigger_config_by_agent(
        &self,
        agent_id: Uuid,
    ) -> Option<AgentCodexTriggerConfig> {
        self.get_agent_codex_trigger_config_by_agent_result(agent_id)
            .ok()
            .flatten()
    }

    fn get_agent_codex_trigger_config_by_agent_result(
        &self,
        agent_id: Uuid,
    ) -> AppResult<Option<AgentCodexTriggerConfig>> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, agent_profile_id, status, interval_seconds,
                       codex_profile, model, reasoning_effort, reasoning_summary, verbosity,
                       personality, service_tier, sandbox_mode, approval_policy, network_access,
                       web_search, feature_multi_agent, feature_remote_plugin, feature_hooks,
                       feature_goals, feature_shell_tool, max_run_seconds, next_run_at,
                       lease_owner, lease_expires_at, manual_run_requested_at,
                       wake_requested_at, wake_reason,
                       last_run_at, last_success_at, last_error,
                       consecutive_failure_count, created_by_human_user_id,
                       updated_by_human_user_id, created_at, updated_at
                FROM agent_codex_trigger_configs
                WHERE agent_profile_id = $1
                "#,
                &[&agent_id],
            )
        })
        .map(|row| row.map(map_agent_codex_trigger_config))
    }

    fn claim_due_agent_codex_trigger_configs(
        &self,
        lease_owner: &str,
        now: chrono::DateTime<chrono::Utc>,
        limit: usize,
    ) -> AppResult<Vec<AgentCodexTriggerConfig>> {
        self.with_client(|client| {
            client.query(
                r#"
                WITH stale_runs AS (
                    UPDATE agent_codex_trigger_runs run
                    SET status = 'lease_lost',
                        finished_at = $2,
                        activity_phase = 'lease_lost',
                        activity_summary = 'Trigger 进程中断，本轮已停止',
                        last_activity_at = $2,
                        error_message = COALESCE(
                            run.error_message,
                            'Codex trigger process stopped before the run completed'
                        )
                    FROM agent_codex_trigger_configs stale_config
                    WHERE run.trigger_config_id = stale_config.id
                      AND run.status = 'running'
                      AND run.started_at
                          + make_interval(secs => stale_config.max_run_seconds + 60) <= $2
                    RETURNING run.agent_profile_id
                ),
                recovered_intents AS (
                    UPDATE agent_execution_intents intent
                    SET status = 'pending',
                        worker_session_id = NULL,
                        claimed_at = NULL,
                        completed_at = NULL,
                        error_message = NULL
                    WHERE intent.status = 'running'
                      AND NOT EXISTS (
                          SELECT 1
                          FROM agent_codex_trigger_runs active_run
                          WHERE active_run.agent_profile_id = intent.agent_profile_id
                            AND active_run.status = 'running'
                      )
                    RETURNING intent.id
                ),
                due AS (
                    SELECT config.id
                    FROM agent_codex_trigger_configs config
                    WHERE config.status = 'active'
                      AND config.next_run_at <= $2
                      AND (config.lease_expires_at IS NULL OR config.lease_expires_at <= $2)
                      AND EXISTS (
                          SELECT 1
                          FROM company_human_members human_member
                          INNER JOIN human_sessions human_session
                              ON human_session.human_user_id = human_member.human_user_id
                          WHERE human_member.company_id = config.company_id
                            AND human_member.status = 'active'
                            AND human_session.revoked_at IS NULL
                            AND human_session.expires_at > $2
                            AND COALESCE(human_session.last_used_at, human_session.created_at)
                                > $2 - INTERVAL '60 seconds'
                      )
                      AND NOT EXISTS (
                          SELECT 1
                          FROM agent_codex_trigger_runs active_run
                          WHERE active_run.agent_profile_id = config.agent_profile_id
                            AND active_run.status = 'running'
                            AND active_run.started_at
                                + make_interval(secs => config.max_run_seconds + 60) > $2
                      )
                    ORDER BY config.next_run_at, config.id
                    FOR UPDATE SKIP LOCKED
                    LIMIT $3
                )
                UPDATE agent_codex_trigger_configs config
                SET lease_owner = $1,
                    lease_expires_at = $2 + make_interval(secs => config.max_run_seconds + 60),
                    last_run_at = $2,
                    updated_at = $2
                FROM due
                WHERE config.id = due.id
                RETURNING config.id, config.company_id, config.agent_profile_id,
                          config.status, config.interval_seconds, config.codex_profile,
                          config.model, config.reasoning_effort, config.reasoning_summary,
                          config.verbosity, config.personality, config.service_tier,
                          config.sandbox_mode, config.approval_policy, config.network_access,
                          config.web_search, config.feature_multi_agent, config.feature_remote_plugin,
                          config.feature_hooks, config.feature_goals, config.feature_shell_tool,
                          config.max_run_seconds, config.next_run_at,
                          config.lease_owner, config.lease_expires_at,
                          config.manual_run_requested_at,
                          config.wake_requested_at, config.wake_reason, config.last_run_at,
                          config.last_success_at, config.last_error,
                          config.consecutive_failure_count,
                          config.created_by_human_user_id,
                          config.updated_by_human_user_id, config.created_at,
                          config.updated_at
                "#,
                &[&lease_owner, &now, &(limit as i64)],
            )
        })
        .map(|rows| {
            rows.into_iter()
                .map(map_agent_codex_trigger_config)
                .collect()
        })
    }

    fn abandon_agent_codex_trigger_leases(
        &self,
        lease_owner: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        self.with_client(|client| {
            client.query_one(
                r#"
                WITH abandoned_configs AS (
                    UPDATE agent_codex_trigger_configs
                    SET lease_owner = NULL,
                        lease_expires_at = NULL,
                        next_run_at = LEAST(next_run_at, $2),
                        updated_at = $2
                    WHERE lease_owner = $1
                    RETURNING id
                ),
                abandoned_runs AS (
                    UPDATE agent_codex_trigger_runs run
                    SET status = 'restarted',
                        finished_at = $2,
                        error_message = NULL,
                        activity_phase = 'continuing',
                        activity_summary = 'Trigger 服务重启，本轮工作已保存并等待接续',
                        last_activity_at = $2
                    WHERE run.status = 'running'
                      AND run.trigger_config_id IN (SELECT id FROM abandoned_configs)
                    RETURNING run.id, run.agent_profile_id
                ),
                recovered_intents AS (
                    UPDATE agent_execution_intents intent
                    SET status = 'pending',
                        worker_session_id = NULL,
                        claimed_at = NULL,
                        completed_at = NULL,
                        error_message = NULL
                    WHERE intent.status = 'running'
                      AND intent.agent_profile_id IN (
                          SELECT agent_profile_id FROM abandoned_runs
                      )
                    RETURNING intent.id
                )
                SELECT COUNT(*)::BIGINT AS abandoned_run_count
                FROM abandoned_runs
                "#,
                &[&lease_owner, &now],
            )
        })
        .map(|row| row.get::<_, i64>("abandoned_run_count") as usize)
    }

    fn request_agent_codex_trigger_wake(
        &self,
        agent_id: Uuid,
        requested_at: chrono::DateTime<chrono::Utc>,
        reason: &str,
    ) -> AppResult<bool> {
        if !is_agent_codex_wake_reason(reason) {
            return Err(AppError::Validation(format!(
                "unsupported Codex trigger wake reason: {reason}"
            )));
        }
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_codex_trigger_configs
                SET next_run_at = LEAST(next_run_at, $2),
                    wake_requested_at = GREATEST(wake_requested_at, $2),
                    wake_reason = $3,
                    updated_at = $2
                WHERE agent_profile_id = $1
                  AND status = 'active'
                "#,
                &[&agent_id, &requested_at, &reason],
            )
        })
        .map(|updated| updated > 0)
    }

    fn complete_agent_codex_trigger_lease(
        &self,
        input: CompleteAgentCodexTriggerLeaseInput,
    ) -> AppResult<()> {
        let updated = self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_codex_trigger_configs
                SET status = CASE
                        WHEN NOT $5 AND consecutive_failure_count + 1 >= 3 AND status = 'active'
                            THEN 'error'
                        ELSE status
                    END,
                    next_run_at = CASE
                        WHEN (manual_run_requested_at IS NOT NULL
                              AND last_run_at IS NOT NULL
                              AND manual_run_requested_at > last_run_at)
                          OR (wake_requested_at IS NOT NULL
                              AND last_run_at IS NOT NULL
                              AND wake_requested_at > last_run_at)
                            THEN LEAST(
                                COALESCE(
                                    CASE
                                        WHEN manual_run_requested_at > last_run_at
                                            THEN manual_run_requested_at
                                    END,
                                    $4
                                ),
                                COALESCE(
                                    CASE
                                        WHEN wake_requested_at > last_run_at
                                            THEN wake_requested_at
                                    END,
                                    $4
                                ),
                                $4
                            )
                        ELSE $4
                    END,
                    lease_owner = NULL,
                    lease_expires_at = NULL,
                    manual_run_requested_at = CASE
                        WHEN manual_run_requested_at IS NOT NULL
                         AND last_run_at IS NOT NULL
                         AND manual_run_requested_at > last_run_at
                            THEN manual_run_requested_at
                        ELSE NULL
                    END,
                    wake_requested_at = CASE
                        WHEN wake_requested_at IS NOT NULL
                         AND last_run_at IS NOT NULL
                         AND wake_requested_at > last_run_at
                            THEN wake_requested_at
                        ELSE NULL
                    END,
                    wake_reason = CASE
                        WHEN wake_requested_at IS NOT NULL
                         AND last_run_at IS NOT NULL
                         AND wake_requested_at > last_run_at
                            THEN wake_reason
                        ELSE NULL
                    END,
                    last_run_at = $3,
                    last_success_at = CASE WHEN $5 THEN $3 ELSE last_success_at END,
                    last_error = CASE WHEN $5 THEN NULL ELSE $6 END,
                    consecutive_failure_count = CASE
                        WHEN $5 THEN 0
                        ELSE consecutive_failure_count + 1
                    END,
                    updated_at = $3
                WHERE id = $1 AND lease_owner = $2
                "#,
                &[
                    &input.trigger_config_id,
                    &input.lease_owner,
                    &input.finished_at,
                    &input.next_run_at,
                    &input.succeeded,
                    &input.error_message,
                ],
            )
        })?;
        if updated == 0 {
            return Err(AppError::Conflict("Codex trigger lease was lost".into()));
        }
        Ok(())
    }

    fn insert_agent_codex_trigger_run(&self, run: AgentCodexTriggerRun) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_codex_trigger_runs (
                    id, trigger_config_id, agent_profile_id, project_id,
                    trigger_type, status, codex_thread_id, codex_version,
                    exit_code, started_at, finished_at, final_message_summary,
                    error_message, activity_phase, activity_summary,
                    last_activity_at, activity_log, process_instance_id,
                    heartbeat_at, state_reason, current_intent_id, current_task_id,
                    waiting_on_type, waiting_on_id, session_kind, resumes_run_id
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                        $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24,
                        $25, $26)
                "#,
                &[
                    &run.id,
                    &run.trigger_config_id,
                    &run.agent_profile_id,
                    &run.project_id,
                    &run.trigger_type,
                    &run.status,
                    &run.codex_thread_id,
                    &run.codex_version,
                    &run.exit_code,
                    &run.started_at,
                    &run.finished_at,
                    &run.final_message_summary,
                    &run.error_message,
                    &run.activity_phase,
                    &run.activity_summary,
                    &run.last_activity_at,
                    &Json(&run.activity_log),
                    &run.process_instance_id,
                    &run.heartbeat_at,
                    &run.state_reason,
                    &run.current_intent_id,
                    &run.current_task_id,
                    &run.waiting_on_type,
                    &run.waiting_on_id,
                    &run.session_kind,
                    &run.resumes_run_id,
                ],
            )?;
            Ok(())
        })
    }

    fn has_running_agent_codex_trigger_run(&self, agent_id: Uuid) -> bool {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT 1
                FROM agent_codex_trigger_runs
                WHERE agent_profile_id = $1 AND status = 'running'
                LIMIT 1
                "#,
                &[&agent_id],
            )
        })
        .ok()
        .flatten()
        .is_some()
    }

    fn update_agent_codex_trigger_run(&self, run: AgentCodexTriggerRun) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_codex_trigger_runs
                SET project_id = $2,
                    trigger_type = $3,
                    status = $4,
                    codex_thread_id = $5,
                    codex_version = $6,
                    exit_code = $7,
                    finished_at = $8,
                    final_message_summary = $9,
                    error_message = $10,
                    activity_phase = $11,
                    activity_summary = $12,
                    last_activity_at = $13,
                    process_instance_id = $14,
                    heartbeat_at = $15,
                    state_reason = $16,
                    current_intent_id = $17,
                    current_task_id = $18,
                    waiting_on_type = $19,
                    waiting_on_id = $20,
                    session_kind = $21,
                    resumes_run_id = $22
                WHERE id = $1
                "#,
                &[
                    &run.id,
                    &run.project_id,
                    &run.trigger_type,
                    &run.status,
                    &run.codex_thread_id,
                    &run.codex_version,
                    &run.exit_code,
                    &run.finished_at,
                    &run.final_message_summary,
                    &run.error_message,
                    &run.activity_phase,
                    &run.activity_summary,
                    &run.last_activity_at,
                    &run.process_instance_id,
                    &run.heartbeat_at,
                    &run.state_reason,
                    &run.current_intent_id,
                    &run.current_task_id,
                    &run.waiting_on_type,
                    &run.waiting_on_id,
                    &run.session_kind,
                    &run.resumes_run_id,
                ],
            )?;
            Ok(())
        })
    }

    fn append_agent_codex_trigger_run_activity(
        &self,
        run_id: Uuid,
        activity: AgentCodexRunActivity,
        codex_thread_id: Option<String>,
    ) -> AppResult<()> {
        let updated = self.with_client(|client| {
            let mut tx = client.transaction()?;
            let Some(row) = tx.query_opt(
                "SELECT activity_log FROM agent_codex_trigger_runs WHERE id = $1 FOR UPDATE",
                &[&run_id],
            )?
            else {
                return Ok(false);
            };
            let Json(mut activity_log): Json<Vec<AgentCodexRunActivity>> = row.get("activity_log");
            activity_log.push(activity.clone());
            if activity_log.len() > 40 {
                let excess = activity_log.len() - 40;
                activity_log.drain(0..excess);
            }
            tx.execute(
                r#"
                UPDATE agent_codex_trigger_runs
                SET activity_phase = $2,
                    activity_summary = $3,
                    last_activity_at = $4,
                    heartbeat_at = $4,
                    state_reason = $3,
                    codex_thread_id = COALESCE($5, codex_thread_id),
                    activity_log = $6
                WHERE id = $1
                "#,
                &[
                    &run_id,
                    &activity.phase,
                    &activity.summary,
                    &activity.at,
                    &codex_thread_id,
                    &Json(&activity_log),
                ],
            )?;
            tx.commit()?;
            Ok(true)
        })?;
        if !updated {
            return Err(AppError::NotFound("Codex trigger run not found".into()));
        }
        Ok(())
    }

    fn heartbeat_agent_codex_trigger_run(
        &self,
        run_id: Uuid,
        heartbeat_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let updated = self.with_client(|client| {
            client.execute(
                "UPDATE agent_codex_trigger_runs SET heartbeat_at = $2 WHERE id = $1 AND status = 'running'",
                &[&run_id, &heartbeat_at],
            )
        })?;
        if updated == 0 {
            return Err(AppError::Conflict(
                "Codex trigger run is no longer running".into(),
            ));
        }
        Ok(())
    }

    fn watchdog_stale_agent_codex_trigger_runs(
        &self,
        now: chrono::DateTime<chrono::Utc>,
        stale_before: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        self.with_client(|client| {
            client.query_one(
                r#"
                WITH stale_runs AS (
                    UPDATE agent_codex_trigger_runs
                    SET status = 'lease_lost', finished_at = $1,
                        activity_phase = 'lease_lost',
                        activity_summary = '运行心跳已停止，Watchdog 已回收本轮',
                        last_activity_at = $1,
                        state_reason = 'run_heartbeat_lost: Trigger 进程没有继续报告心跳',
                        error_message = 'run_heartbeat_lost: Trigger process heartbeat stopped'
                    WHERE status = 'running'
                      AND GREATEST(
                          COALESCE(heartbeat_at, started_at),
                          COALESCE(last_activity_at, started_at)
                      ) < $2
                    RETURNING id, trigger_config_id, agent_profile_id
                ),
                released_configs AS (
                    UPDATE agent_codex_trigger_configs config
                    SET lease_owner = NULL, lease_expires_at = NULL,
                        next_run_at = LEAST(config.next_run_at, $1), updated_at = $1
                    WHERE config.id IN (SELECT trigger_config_id FROM stale_runs)
                    RETURNING config.id
                ),
                recovered_intents AS (
                    UPDATE agent_execution_intents intent
                    SET status = 'pending', claimed_at = NULL, completed_at = NULL,
                        error_message = '上一个运行心跳丢失，Relay 已安排从原项目会话恢复'
                    WHERE intent.status = 'running'
                      AND intent.agent_profile_id IN (SELECT agent_profile_id FROM stale_runs)
                    RETURNING intent.id
                )
                SELECT COUNT(*)::BIGINT AS stale_count FROM stale_runs
                "#,
                &[&now, &stale_before],
            )
        })
        .map(|row| row.get::<_, i64>("stale_count") as usize)
    }

    fn list_agent_codex_trigger_runs(
        &self,
        agent_id: Uuid,
        limit: usize,
    ) -> Vec<AgentCodexTriggerRun> {
        self.list_agent_codex_trigger_runs_result(agent_id, limit)
            .unwrap_or_default()
    }

    fn list_agent_codex_trigger_runs_result(
        &self,
        agent_id: Uuid,
        limit: usize,
    ) -> AppResult<Vec<AgentCodexTriggerRun>> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, trigger_config_id, agent_profile_id, project_id,
                       trigger_type, status, codex_thread_id, codex_version,
                       exit_code, started_at, finished_at, final_message_summary,
                       error_message, activity_phase, activity_summary,
                       last_activity_at, activity_log, process_instance_id,
                       heartbeat_at, state_reason, current_intent_id, current_task_id,
                       waiting_on_type, waiting_on_id, session_kind, resumes_run_id
                FROM agent_codex_trigger_runs
                WHERE agent_profile_id = $1
                ORDER BY started_at DESC, id DESC
                LIMIT $2
                "#,
                &[&agent_id, &(limit as i64)],
            )
        })
        .map(|rows| rows.into_iter().map(map_agent_codex_trigger_run).collect())
    }

    fn save_agent_codex_session(&self, session: AgentCodexSession) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_codex_sessions (
                    id, agent_profile_id, session_kind, scope_key, project_id, generation,
                    codex_thread_id, workspace_key, status, summary_short, checkpoint_json,
                    skill_bundle_version, memory_snapshot_version, policy_version,
                    created_at, last_used_at, archived_at
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                    $13, $14, $15, $16, $17
                )
                ON CONFLICT (agent_profile_id, scope_key, generation) DO UPDATE
                SET session_kind = EXCLUDED.session_kind,
                    project_id = EXCLUDED.project_id,
                    codex_thread_id = EXCLUDED.codex_thread_id,
                    workspace_key = EXCLUDED.workspace_key,
                    status = EXCLUDED.status,
                    summary_short = EXCLUDED.summary_short,
                    checkpoint_json = EXCLUDED.checkpoint_json,
                    skill_bundle_version = EXCLUDED.skill_bundle_version,
                    memory_snapshot_version = EXCLUDED.memory_snapshot_version,
                    policy_version = EXCLUDED.policy_version,
                    last_used_at = EXCLUDED.last_used_at,
                    archived_at = EXCLUDED.archived_at
                "#,
                &[
                    &session.id,
                    &session.agent_profile_id,
                    &session.session_kind,
                    &session.scope_key,
                    &session.project_id,
                    &session.generation,
                    &session.codex_thread_id,
                    &session.workspace_key,
                    &session.status,
                    &session.summary_short,
                    &session.checkpoint_json,
                    &session.skill_bundle_version,
                    &session.memory_snapshot_version,
                    &session.policy_version,
                    &session.created_at,
                    &session.last_used_at,
                    &session.archived_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_agent_codex_session(
        &self,
        agent_id: Uuid,
        scope_key: &str,
    ) -> Option<AgentCodexSession> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, agent_profile_id, session_kind, scope_key, project_id, generation,
                       codex_thread_id, workspace_key, status, summary_short, checkpoint_json,
                       skill_bundle_version, memory_snapshot_version, policy_version,
                       created_at, last_used_at, archived_at
                FROM agent_codex_sessions
                WHERE agent_profile_id = $1 AND scope_key = $2 AND status = 'active'
                "#,
                &[&agent_id, &scope_key],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_codex_session)
    }

    fn list_agent_codex_sessions(&self, agent_id: Uuid, limit: usize) -> Vec<AgentCodexSession> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, agent_profile_id, session_kind, scope_key, project_id, generation,
                       codex_thread_id, workspace_key, status, summary_short, checkpoint_json,
                       skill_bundle_version, memory_snapshot_version, policy_version,
                       created_at, last_used_at, archived_at
                FROM agent_codex_sessions
                WHERE agent_profile_id = $1
                ORDER BY (status = 'active') DESC, last_used_at DESC, id DESC
                LIMIT $2
                "#,
                &[&agent_id, &(limit as i64)],
            )
        })
        .map(|rows| rows.into_iter().map(map_agent_codex_session).collect())
        .unwrap_or_default()
    }

    fn insert_agent_execution_intent(&self, intent: AgentExecutionIntent) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_execution_intents (
                    id, company_id, agent_profile_id, project_id, worker_session_id,
                    source_event_ids, task_ids, action_type, objective, acceptance_criteria,
                    priority, dedupe_key, status, result_summary, error_message,
                    created_at, claimed_at, completed_at
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                    $13, $14, $15, $16, $17, $18
                )
                "#,
                &[
                    &intent.id,
                    &intent.company_id,
                    &intent.agent_profile_id,
                    &intent.project_id,
                    &intent.worker_session_id,
                    &Json(&intent.source_event_ids),
                    &Json(&intent.task_ids),
                    &intent.action_type,
                    &intent.objective,
                    &Json(&intent.acceptance_criteria),
                    &intent.priority,
                    &intent.dedupe_key,
                    &intent.status,
                    &intent.result_summary,
                    &intent.error_message,
                    &intent.created_at,
                    &intent.claimed_at,
                    &intent.completed_at,
                ],
            )?;
            Ok(())
        })
    }

    fn update_agent_execution_intent(&self, intent: AgentExecutionIntent) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_execution_intents
                SET worker_session_id = $2,
                    status = $3,
                    result_summary = $4,
                    error_message = $5,
                    claimed_at = $6,
                    completed_at = $7
                WHERE id = $1
                "#,
                &[
                    &intent.id,
                    &intent.worker_session_id,
                    &intent.status,
                    &intent.result_summary,
                    &intent.error_message,
                    &intent.claimed_at,
                    &intent.completed_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_agent_execution_intent(&self, intent_id: Uuid) -> Option<AgentExecutionIntent> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, agent_profile_id, project_id, worker_session_id,
                       source_event_ids, task_ids, action_type, objective, acceptance_criteria,
                       priority, dedupe_key, status, result_summary, error_message,
                       created_at, claimed_at, completed_at
                FROM agent_execution_intents
                WHERE id = $1
                "#,
                &[&intent_id],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_execution_intent)
    }

    fn find_agent_execution_intent_by_dedupe_key(
        &self,
        agent_id: Uuid,
        dedupe_key: &str,
    ) -> Option<AgentExecutionIntent> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, agent_profile_id, project_id, worker_session_id,
                       source_event_ids, task_ids, action_type, objective, acceptance_criteria,
                       priority, dedupe_key, status, result_summary, error_message,
                       created_at, claimed_at, completed_at
                FROM agent_execution_intents
                WHERE agent_profile_id = $1 AND dedupe_key = $2
                "#,
                &[&agent_id, &dedupe_key],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_execution_intent)
    }

    fn list_agent_execution_intents(
        &self,
        agent_id: Uuid,
        status: Option<&str>,
        limit: usize,
    ) -> Vec<AgentExecutionIntent> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, company_id, agent_profile_id, project_id, worker_session_id,
                       source_event_ids, task_ids, action_type, objective, acceptance_criteria,
                       priority, dedupe_key, status, result_summary, error_message,
                       created_at, claimed_at, completed_at
                FROM agent_execution_intents
                WHERE agent_profile_id = $1
                  AND ($2::TEXT IS NULL OR status = $2)
                ORDER BY created_at ASC, id ASC
                LIMIT $3
                "#,
                &[&agent_id, &status, &(limit as i64)],
            )
        })
        .map(|rows| rows.into_iter().map(map_agent_execution_intent).collect())
        .unwrap_or_default()
    }

    fn insert_agent_codex_run_token(&self, token: AgentCodexRunToken) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_codex_run_tokens (
                    id, run_id, agent_profile_id, token_hash,
                    expires_at, revoked_at, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
                &[
                    &token.id,
                    &token.run_id,
                    &token.agent_profile_id,
                    &token.token_hash,
                    &token.expires_at,
                    &token.revoked_at,
                    &token.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn find_agent_codex_run_token_by_hash(&self, token_hash: &str) -> Option<AgentCodexRunToken> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, run_id, agent_profile_id, token_hash,
                       expires_at, revoked_at, created_at
                FROM agent_codex_run_tokens
                WHERE token_hash = $1
                "#,
                &[&token_hash],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_codex_run_token)
    }

    fn revoke_agent_codex_run_tokens(
        &self,
        run_id: Uuid,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_codex_run_tokens
                SET revoked_at = $2
                WHERE run_id = $1 AND revoked_at IS NULL
                "#,
                &[&run_id, &revoked_at],
            )?;
            Ok(())
        })
    }

    fn delete_expired_agent_codex_run_tokens(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        self.with_client(|client| {
            client
                .execute(
                    r#"
                    DELETE FROM agent_codex_run_tokens
                    WHERE expires_at <= $1 OR revoked_at IS NOT NULL
                    "#,
                    &[&now],
                )
                .map(|count| count as usize)
        })
    }
}
