use super::*;

pub(super) async fn process_claimed_trigger(
    platform: &TriggerPlatform,
    harness: &TriggerHarnessProvisioner,
    workspace_manager: &GitWorkspaceManager,
    codex_runner: &CodexTriggerRunner,
    codex_control: &CodexControlStore,
    service_config: &TriggerServiceConfig,
    trigger: AgentCodexTriggerConfig,
) {
    let execution = execute_trigger(
        platform,
        harness,
        workspace_manager,
        codex_runner,
        codex_control,
        &trigger,
    )
    .await;
    let finished_at = now_utc();
    let completion = match execution {
        Ok(execution) => execution,
        Err(error) => TriggerExecution {
            succeeded: false,
            error_message: Some(sanitize_error(&error.to_string())),
            retry_after_seconds: None,
        },
    };
    let next_run_at = completion
        .retry_after_seconds
        .map(|seconds| finished_at + Duration::seconds(seconds))
        .unwrap_or_else(|| next_trigger_run_at(&trigger, finished_at, completion.succeeded));
    if let Err(error) =
        platform.complete_agent_codex_trigger_lease(CompleteAgentCodexTriggerLeaseInput {
            trigger_config_id: trigger.id,
            lease_owner: service_config.lease_owner.clone(),
            finished_at,
            next_run_at,
            succeeded: completion.succeeded,
            error_message: completion.error_message.clone(),
        })
    {
        tracing::error!(
            trigger_id = %trigger.id,
            agent_id = %trigger.agent_profile_id,
            error = %sanitize_error(&error.to_string()),
            "failed to release Codex trigger lease"
        );
    } else if completion.succeeded {
        tracing::info!(
            trigger_id = %trigger.id,
            agent_id = %trigger.agent_profile_id,
            "Codex trigger cycle completed"
        );
    } else {
        tracing::warn!(
            trigger_id = %trigger.id,
            agent_id = %trigger.agent_profile_id,
            error = completion.error_message.as_deref().unwrap_or("unknown error"),
            "Codex trigger cycle failed"
        );
    }
}

pub(super) fn resolve_effective_cli_settings(
    trigger: &AgentCodexTriggerConfig,
    company: &CompanyCodexCliSettings,
) -> EffectiveCodexCliSettings {
    EffectiveCodexCliSettings {
        model: trigger.model.clone().or_else(|| company.model.clone()),
        reasoning_effort: trigger
            .reasoning_effort
            .clone()
            .or_else(|| company.reasoning_effort.clone()),
        reasoning_summary: trigger
            .reasoning_summary
            .clone()
            .or_else(|| Some(company.reasoning_summary.clone())),
        verbosity: trigger
            .verbosity
            .clone()
            .or_else(|| company.verbosity.clone()),
        personality: trigger
            .personality
            .clone()
            .or_else(|| company.personality.clone()),
        service_tier: company.service_tier.clone(),
        sandbox_mode: if trigger.sandbox_mode == AGENT_CODEX_SETTING_INHERIT {
            company.sandbox_mode.clone()
        } else {
            trigger.sandbox_mode.clone()
        },
        approval_policy: if trigger.approval_policy == AGENT_CODEX_SETTING_INHERIT {
            company.approval_policy.clone()
        } else {
            trigger.approval_policy.clone()
        },
        network_access: company.network_access,
        web_search: company.web_search.clone(),
        feature_multi_agent: company.feature_multi_agent,
        feature_remote_plugin: company.feature_remote_plugin,
        feature_hooks: company.feature_hooks,
        feature_goals: company.feature_goals,
        feature_shell_tool: company.feature_shell_tool,
    }
}

pub(super) fn next_trigger_run_at(
    trigger: &AgentCodexTriggerConfig,
    finished_at: chrono::DateTime<chrono::Utc>,
    succeeded: bool,
) -> chrono::DateTime<chrono::Utc> {
    if succeeded {
        return finished_at + Duration::seconds(i64::from(trigger.interval_seconds));
    }
    let retry_delay_seconds = match trigger.consecutive_failure_count {
        0 => 10,
        1 => 30,
        _ => i64::from(trigger.interval_seconds),
    };
    finished_at + Duration::seconds(retry_delay_seconds)
}

pub(super) async fn execute_trigger(
    platform: &TriggerPlatform,
    harness: &TriggerHarnessProvisioner,
    workspace_manager: &GitWorkspaceManager,
    codex_runner: &CodexTriggerRunner,
    codex_control: &CodexControlStore,
    trigger: &AgentCodexTriggerConfig,
) -> AppResult<TriggerExecution> {
    let company_settings = codex_control.company_cli_settings(trigger.company_id)?;
    let effective_settings = resolve_effective_cli_settings(trigger, &company_settings);
    let decision = protect_trigger_decision(|| platform.decide_agent_codex_work(trigger))?;
    if !decision.should_run {
        return Ok(TriggerExecution {
            succeeded: true,
            error_message: None,
            retry_after_seconds: None,
        });
    }
    if platform.has_running_agent_codex_trigger_run(trigger.agent_profile_id) {
        tracing::info!(
            trigger_id = %trigger.id,
            agent_id = %trigger.agent_profile_id,
            "skipping duplicate Codex wake-up because the Agent already has a running cycle"
        );
        return Ok(TriggerExecution {
            succeeded: true,
            error_message: None,
            retry_after_seconds: None,
        });
    }
    let agent = platform.get_agent_profile_by_id(trigger.agent_profile_id)?;
    let membership = platform.get_active_company_agent_membership(trigger.agent_profile_id)?;
    let control_workspace = workspace_manager
        .prepare_general_workspace(trigger.company_id, trigger.agent_profile_id)?;
    let control_session_id = platform
        .get_agent_codex_session(trigger.agent_profile_id, "control")
        .map(|session| session.id);
    let control_memories = platform.agent_long_term_memories_for_control_session(
        trigger.agent_profile_id,
        trigger.company_id,
        control_session_id,
    )?;
    let skill_language = platform.effective_company_skill_language(trigger.company_id);
    let control_skills = prepare_relay_skills(
        &control_workspace.path,
        RELAY_SKILL_BUNDLE_CONTROL,
        &agent,
        &membership.job_title,
        &membership.permissions,
        &control_memories,
        None,
        None,
        &skill_language,
    )?;
    let started_at = now_utc();
    let initial_activity = AgentCodexRunActivity {
        at: started_at,
        phase: "preparing".into(),
        summary: "正在准备 Agent 控制会话".into(),
    };
    let mut run = AgentCodexTriggerRun {
        id: Uuid::new_v4(),
        trigger_config_id: trigger.id,
        agent_profile_id: trigger.agent_profile_id,
        project_id: decision.project.as_ref().map(|project| project.id),
        trigger_type: decision.trigger_type.clone(),
        status: AGENT_CODEX_RUN_STATUS_RUNNING.into(),
        codex_thread_id: None,
        codex_version: codex_runner.detect_version(),
        exit_code: None,
        started_at,
        finished_at: None,
        final_message_summary: None,
        error_message: None,
        activity_phase: initial_activity.phase.clone(),
        activity_summary: Some(initial_activity.summary.clone()),
        last_activity_at: Some(initial_activity.at),
        activity_log: vec![initial_activity],
    };
    platform.insert_agent_codex_trigger_run(run.clone())?;
    let token_expiry =
        started_at + Duration::seconds(i64::from(trigger.max_run_seconds).saturating_mul(4) + 60);
    let token = match platform.issue_agent_codex_run_token(
        run.id,
        trigger.agent_profile_id,
        token_expiry,
    ) {
        Ok(token) => token,
        Err(error) => {
            fail_run(platform, &mut run, None, error.to_string())?;
            return Err(error);
        }
    };
    let mut control_settings = effective_settings.clone();
    control_settings.sandbox_mode = AGENT_CODEX_SANDBOX_READ_ONLY.into();
    control_settings.approval_policy = AGENT_CODEX_APPROVAL_POLICY_NEVER.into();
    control_settings.network_access = false;
    control_settings.web_search = "disabled".into();
    control_settings.feature_multi_agent = false;
    control_settings.feature_shell_tool = false;
    let control_prompt = build_wakeup_prompt(WakeupPromptContext {
        agent: &agent,
        job_title: &membership.job_title,
        project_name: decision
            .project
            .as_ref()
            .map(|project| project.name.as_str()),
        pending_inbox_count: decision.pending_inbox_count,
        active_task_count: decision.active_task_count,
        waiting_task_count: decision.waiting_task_count,
        asset_refresh_due: decision.asset_refresh_due,
        workspace: &control_workspace,
        relay_skills: &control_skills,
    });
    let control_result = run_codex_stage(
        platform,
        codex_runner,
        trigger,
        &run,
        &control_workspace,
        "control",
        None,
        control_prompt,
        &control_skills,
        &control_memories,
        &control_settings,
        &token.plaintext_token,
    )
    .await;
    let control_result = match control_result {
        Ok(result) => result,
        Err(error) => {
            let _ = platform.revoke_agent_codex_run_tokens(run.id);
            fail_run(platform, &mut run, None, error.to_string())?;
            return Err(error);
        }
    };
    if control_result.status != CodexRunStatus::Succeeded {
        let _ = platform.revoke_agent_codex_run_tokens(run.id);
        apply_codex_result_to_run(&mut run, &control_result);
        platform.update_agent_codex_trigger_run(run.clone())?;
        return Ok(trigger_execution_from_result(&control_result));
    }
    let control_session = persist_codex_stage_session(
        platform,
        trigger.agent_profile_id,
        "control",
        AGENT_CODEX_SESSION_KIND_CONTROL,
        None,
        &control_workspace,
        &control_skills,
        &control_memories,
        &control_result,
    )?;
    run.codex_thread_id = Some(control_session.codex_thread_id.clone());
    run.final_message_summary = control_result
        .final_message
        .as_deref()
        .map(|message| truncate(message, 2_000));

    let mut worker_failure = None;
    let mut worker_retry_requested = false;
    let intents = platform.list_agent_execution_intents(
        trigger.agent_profile_id,
        Some(AGENT_EXECUTION_INTENT_STATUS_PENDING),
        3,
    );
    for mut intent in intents {
        intent.status = AGENT_EXECUTION_INTENT_STATUS_RUNNING.into();
        intent.claimed_at = Some(now_utc());
        platform.update_agent_execution_intent(intent.clone())?;
        record_run_activity(
            platform,
            run.id,
            "dispatching",
            &format!("正在进入项目工作会话：{}", intent.project_id),
            None,
        );
        let worker_result = execute_project_intent(
            platform,
            harness,
            workspace_manager,
            codex_runner,
            trigger,
            &run,
            &agent,
            &membership,
            &skill_language,
            &effective_settings,
            &token.plaintext_token,
            &intent,
        )
        .await;
        match worker_result {
            Ok((result, session)) if result.status == CodexRunStatus::Succeeded => {
                intent.status = AGENT_EXECUTION_INTENT_STATUS_COMPLETED.into();
                intent.worker_session_id = Some(session.id);
                intent.result_summary = result
                    .final_message
                    .as_deref()
                    .map(|message| truncate(message, 4_000))
                    .unwrap_or_default();
                intent.completed_at = Some(now_utc());
                platform.update_agent_execution_intent(intent)?;
                run.project_id = session.project_id;
                run.codex_thread_id = Some(session.codex_thread_id);
                run.final_message_summary = result
                    .final_message
                    .as_deref()
                    .map(|message| truncate(message, 2_000));
            }
            Ok((result, session)) if result.status == CodexRunStatus::TimedOut => {
                intent.status = AGENT_EXECUTION_INTENT_STATUS_PENDING.into();
                intent.worker_session_id = Some(session.id);
                intent.result_summary = result
                    .final_message
                    .as_deref()
                    .map(|message| truncate(message, 4_000))
                    .unwrap_or_default();
                intent.error_message =
                    Some("本轮达到运行时间上限，Relay 将从当前项目会话继续执行".into());
                intent.claimed_at = None;
                intent.completed_at = None;
                platform.update_agent_execution_intent(intent)?;
                run.project_id = session.project_id;
                run.codex_thread_id = Some(session.codex_thread_id);
                run.final_message_summary = result
                    .final_message
                    .as_deref()
                    .map(|message| truncate(message, 2_000));
                record_run_activity(
                    platform,
                    run.id,
                    "continuing",
                    "本轮达到运行时间上限，已保存项目会话，将自动继续",
                    run.codex_thread_id.clone(),
                );
                worker_retry_requested = true;
                worker_failure = Some(result);
                break;
            }
            Ok((result, _)) => {
                intent.status = if result.status == CodexRunStatus::Cancelled {
                    AGENT_EXECUTION_INTENT_STATUS_CANCELLED
                } else {
                    AGENT_EXECUTION_INTENT_STATUS_FAILED
                }
                .into();
                intent.error_message = result.error_message.clone();
                intent.result_summary = result.final_message.clone().unwrap_or_default();
                intent.completed_at = Some(now_utc());
                platform.update_agent_execution_intent(intent)?;
                worker_failure = Some(result);
                break;
            }
            Err(error) => {
                intent.status = AGENT_EXECUTION_INTENT_STATUS_FAILED.into();
                intent.error_message = Some(sanitize_error(&error.to_string()));
                intent.completed_at = Some(now_utc());
                platform.update_agent_execution_intent(intent)?;
                let _ = platform.revoke_agent_codex_run_tokens(run.id);
                fail_run(platform, &mut run, None, error.to_string())?;
                return Err(error);
            }
        }
    }
    if let Err(error) = platform.revoke_agent_codex_run_tokens(run.id) {
        tracing::error!(run_id = %run.id, error = %sanitize_error(&error.to_string()), "failed to revoke Agent Run Token");
    }
    if let Some(result) = worker_failure.as_ref() {
        apply_codex_result_to_run(&mut run, result);
    } else {
        run.status = AGENT_CODEX_RUN_STATUS_SUCCEEDED.into();
        run.exit_code = Some(0);
        run.finished_at = Some(now_utc());
        run.error_message = None;
    }
    platform.update_agent_codex_trigger_run(run.clone())?;
    let final_status = worker_failure
        .as_ref()
        .map(|result| result.status)
        .unwrap_or(CodexRunStatus::Succeeded);
    let (final_phase, final_summary) = match final_status {
        CodexRunStatus::Succeeded => ("completed", "Codex 已完成本轮工作"),
        CodexRunStatus::Failed => ("failed", "Codex 本轮执行失败"),
        CodexRunStatus::TimedOut if worker_retry_requested => {
            ("continuing", "已保存当前进度，即将从同一项目会话继续")
        }
        CodexRunStatus::TimedOut => ("timed_out", "Codex 本轮执行超时"),
        CodexRunStatus::Cancelled => ("cancelled", "项目已暂停，Codex 本轮已停止"),
    };
    record_run_activity(
        platform,
        run.id,
        final_phase,
        final_summary,
        run.codex_thread_id.clone(),
    );
    Ok(TriggerExecution {
        succeeded: worker_failure.is_none() || worker_retry_requested,
        error_message: (!worker_retry_requested)
            .then_some(run.error_message)
            .flatten(),
        retry_after_seconds: worker_retry_requested.then_some(10),
    })
}

#[allow(clippy::too_many_arguments)]
async fn execute_project_intent(
    platform: &TriggerPlatform,
    harness: &TriggerHarnessProvisioner,
    workspace_manager: &GitWorkspaceManager,
    codex_runner: &CodexTriggerRunner,
    trigger: &AgentCodexTriggerConfig,
    run: &AgentCodexTriggerRun,
    agent: &AgentProfile,
    membership: &ai_chat_domain::company::CompanyAgentMembership,
    skill_language: &str,
    settings: &EffectiveCodexCliSettings,
    run_token: &str,
    intent: &AgentExecutionIntent,
) -> AppResult<(CodexRunResult, AgentCodexSession)> {
    let project_view = platform.get_company_project(GetCompanyProjectInput {
        actor_agent_id: trigger.agent_profile_id,
        company_id: trigger.company_id,
        project_id: intent.project_id,
    })?;
    let git = platform.get_agent_project_git_config(
        trigger.agent_profile_id,
        trigger.company_id,
        intent.project_id,
    )?;
    let workspace = prepare_project_workspace_with_credential_recovery(
        harness,
        workspace_manager,
        trigger.company_id,
        intent.project_id,
        trigger.agent_profile_id,
        agent.owner_user_id,
        &agent.handle,
        &git,
    )
    .await?;
    let scope_key = format!("project:{}", intent.project_id);
    let worker_session_id = platform
        .get_agent_codex_session(trigger.agent_profile_id, &scope_key)
        .map(|session| session.id);
    let memories = platform.agent_long_term_memories_for_project_session(
        trigger.agent_profile_id,
        trigger.company_id,
        intent.project_id,
        worker_session_id,
    )?;
    let skills = prepare_relay_skills(
        &workspace.path,
        RELAY_SKILL_BUNDLE_PROJECT,
        agent,
        &membership.job_title,
        &membership.permissions,
        &memories,
        Some(&project_view.project),
        project_view.rule.as_ref(),
        skill_language,
    )?;
    let prompt = build_worker_prompt(WorkerPromptContext {
        agent,
        job_title: &membership.job_title,
        project: &project_view.project,
        intent,
        workspace: &workspace,
        relay_skills: &skills,
    });
    let result = run_codex_stage(
        platform,
        codex_runner,
        trigger,
        run,
        &workspace,
        &scope_key,
        Some(intent.project_id),
        prompt,
        &skills,
        &memories,
        settings,
        run_token,
    )
    .await?;
    let session = if matches!(
        result.status,
        CodexRunStatus::Succeeded | CodexRunStatus::TimedOut
    ) {
        persist_codex_stage_session(
            platform,
            trigger.agent_profile_id,
            &scope_key,
            AGENT_CODEX_SESSION_KIND_PROJECT,
            Some(intent.project_id),
            &workspace,
            &skills,
            &memories,
            &result,
        )?
    } else {
        platform
            .get_agent_codex_session(trigger.agent_profile_id, &scope_key)
            .unwrap_or_else(|| empty_stage_session(trigger.agent_profile_id, intent.project_id))
    };
    Ok((result, session))
}

#[allow(clippy::too_many_arguments)]
async fn run_codex_stage(
    platform: &TriggerPlatform,
    codex_runner: &CodexTriggerRunner,
    trigger: &AgentCodexTriggerConfig,
    run: &AgentCodexTriggerRun,
    workspace: &PreparedGitWorkspace,
    scope_key: &str,
    project_id: Option<Uuid>,
    prompt: String,
    _skills: &PreparedRelaySkills,
    _memories: &[AgentMemory],
    settings: &EffectiveCodexCliSettings,
    run_token: &str,
) -> AppResult<CodexRunResult> {
    let session_key = codex_session_key(workspace);
    let existing_thread_id = platform
        .get_agent_codex_session(trigger.agent_profile_id, scope_key)
        .and_then(|session| {
            codex_session_key_matches(&session.workspace_key, &session_key)
                .then_some(session.codex_thread_id)
        });
    let managed_mcp_servers = project_id
        .map(|project_id| {
            codex_runner.managed_browser_mcp_server(
                trigger.company_id,
                trigger.agent_profile_id,
                project_id,
                &workspace.path,
            )
        })
        .transpose()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let managed_mcp_requires_approval = managed_mcp_servers
        .iter()
        .any(|server| server.requires_human_approval());
    let elapsed_seconds = now_utc()
        .signed_duration_since(run.started_at)
        .num_seconds()
        .max(0) as u64;
    let configured_run_seconds = u64::try_from(trigger.max_run_seconds)
        .map_err(|_| AppError::Validation("Codex max_run_seconds must be positive".into()))?;
    let remaining_run_seconds = configured_run_seconds
        .saturating_sub(elapsed_seconds)
        .max(1);
    let mut result = codex_runner
        .run(CodexRunRequest {
            cwd: workspace.path.clone(),
            codex_profile: trigger.codex_profile.clone(),
            model: settings.model.clone(),
            reasoning_effort: settings.reasoning_effort.clone(),
            reasoning_summary: settings.reasoning_summary.clone(),
            verbosity: settings.verbosity.clone(),
            personality: settings.personality.clone(),
            service_tier: settings.service_tier.clone(),
            sandbox_mode: settings.sandbox_mode.clone(),
            approval_policy: settings.approval_policy.clone(),
            network_access: settings.network_access,
            web_search: settings.web_search.clone(),
            feature_multi_agent: settings.feature_multi_agent,
            feature_remote_plugin: settings.feature_remote_plugin,
            feature_hooks: settings.feature_hooks,
            feature_goals: settings.feature_goals,
            feature_shell_tool: settings.feature_shell_tool,
            max_run_seconds: remaining_run_seconds,
            prompt,
            existing_thread_id,
            run_token: run_token.into(),
            session_kind: if project_id.is_some() {
                AGENT_CODEX_SESSION_KIND_PROJECT.into()
            } else {
                AGENT_CODEX_SESSION_KIND_CONTROL.into()
            },
            environment: workspace.auth_environment.clone(),
            managed_mcp_servers,
            approval_handler: (settings.approval_policy == "on-request"
                || managed_mcp_requires_approval)
                .then(|| {
                    Arc::new(PlatformCodexApprovalHandler {
                        platform: platform.clone(),
                        company_id: trigger.company_id,
                        run_id: run.id,
                        agent_id: trigger.agent_profile_id,
                        project_id,
                        expires_at: now_utc()
                            + Duration::seconds(i64::from(trigger.max_run_seconds)),
                        general_approval_required: settings.approval_policy == "on-request",
                    }) as Arc<dyn CodexApprovalHandler>
                }),
            progress_handler: Some(Arc::new(PlatformCodexProgressHandler {
                platform: platform.clone(),
                run_id: run.id,
            }) as Arc<dyn CodexProgressHandler>),
            cancellation_handler: project_id.map(|project_id| {
                Arc::new(PlatformProjectCancellationHandler {
                    platform: platform.clone(),
                    project_id,
                }) as Arc<dyn CodexCancellationHandler>
            }),
        })
        .await?;
    if let Some(message) = result.final_message.as_mut() {
        *message = sanitize_workspace_output(message, &workspace.path);
    }
    if let Some(message) = result.error_message.as_mut() {
        *message = sanitize_workspace_output(message, &workspace.path);
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn persist_codex_stage_session(
    platform: &TriggerPlatform,
    agent_id: Uuid,
    scope_key: &str,
    session_kind: &str,
    project_id: Option<Uuid>,
    workspace: &PreparedGitWorkspace,
    skills: &PreparedRelaySkills,
    memories: &[AgentMemory],
    result: &CodexRunResult,
) -> AppResult<AgentCodexSession> {
    let thread_id = result.thread_id.clone().ok_or_else(|| {
        AppError::Validation("successful Codex run did not return a thread ID".into())
    })?;
    let existing = platform.get_agent_codex_session(agent_id, scope_key);
    let now = now_utc();
    if result.replaced_failed_session {
        if let Some(mut archived) = existing.clone() {
            archived.status = AGENT_CODEX_SESSION_STATUS_ARCHIVED.into();
            archived.archived_at = Some(now);
            archived.last_used_at = now;
            platform.save_agent_codex_session(archived)?;
        }
    }
    let existing = (!result.replaced_failed_session)
        .then_some(existing)
        .flatten();
    let latest_generation = platform
        .list_agent_codex_sessions(agent_id, 100)
        .into_iter()
        .filter(|session| session.scope_key == scope_key)
        .map(|session| session.generation)
        .max()
        .unwrap_or(0);
    let summary = result
        .final_message
        .as_deref()
        .map(|message| truncate(message, 1_000))
        .unwrap_or_default();
    let session = AgentCodexSession {
        id: existing
            .as_ref()
            .map(|session| session.id)
            .unwrap_or_else(Uuid::new_v4),
        agent_profile_id: agent_id,
        session_kind: session_kind.into(),
        scope_key: scope_key.into(),
        project_id,
        generation: existing
            .as_ref()
            .map(|session| session.generation)
            .unwrap_or(latest_generation + 1),
        codex_thread_id: thread_id,
        workspace_key: codex_session_key(workspace),
        status: AGENT_CODEX_SESSION_STATUS_ACTIVE.into(),
        summary_short: summary.clone(),
        checkpoint_json: serde_json::json!({
            "summary": summary,
            "project_id": project_id,
            "branch": workspace.branch,
            "last_turn_status": match result.status {
                CodexRunStatus::Succeeded => "succeeded",
                CodexRunStatus::Failed => "failed",
                CodexRunStatus::TimedOut => "timed_out",
                CodexRunStatus::Cancelled => "cancelled",
            },
            "continuation_expected": result.status == CodexRunStatus::TimedOut,
            "updated_at": now,
        }),
        skill_bundle_version: skills.version_hash.clone(),
        memory_snapshot_version: memory_snapshot_version(memories),
        policy_version: CODEX_SESSION_POLICY_VERSION.into(),
        created_at: existing
            .as_ref()
            .map(|session| session.created_at)
            .unwrap_or(now),
        last_used_at: now,
        archived_at: None,
    };
    platform.save_agent_codex_session(session.clone())?;
    Ok(session)
}

fn memory_snapshot_version(memories: &[AgentMemory]) -> String {
    let source = memories
        .iter()
        .map(|memory| format!("{}:{}:{}", memory.id, memory.updated_at, memory.topic_key))
        .collect::<Vec<_>>()
        .join("\n");
    hash_secret(&source).chars().take(16).collect()
}

pub(super) fn sanitize_workspace_output(value: &str, workspace_path: &Path) -> String {
    let mut sanitized = value.replace(workspace_path.to_string_lossy().as_ref(), ".");
    if let Ok(canonical_path) = workspace_path.canonicalize() {
        let canonical_path = canonical_path.to_string_lossy();
        if canonical_path.as_ref() != workspace_path.to_string_lossy().as_ref() {
            sanitized = sanitized.replace(canonical_path.as_ref(), ".");
        }
    }
    sanitized = redact_home_user_segment(&sanitized, "/Users/", '/');
    sanitized = redact_home_user_segment(&sanitized, "/home/", '/');
    sanitized
}

fn redact_home_user_segment(value: &str, prefix: &str, separator: char) -> String {
    let mut output = value.to_string();
    let mut search_from = 0;
    while let Some(relative_start) = output[search_from..].find(prefix) {
        let start = search_from + relative_start;
        let username_start = start + prefix.len();
        let Some(relative_end) = output[username_start..].find(separator) else {
            break;
        };
        let end = username_start + relative_end;
        output.replace_range(start..end, "~");
        search_from = start + 1;
    }
    output
}

fn empty_stage_session(agent_id: Uuid, project_id: Uuid) -> AgentCodexSession {
    let now = now_utc();
    AgentCodexSession {
        id: Uuid::nil(),
        agent_profile_id: agent_id,
        session_kind: AGENT_CODEX_SESSION_KIND_PROJECT.into(),
        scope_key: format!("project:{project_id}"),
        project_id: Some(project_id),
        generation: 1,
        codex_thread_id: String::new(),
        workspace_key: String::new(),
        status: AGENT_CODEX_SESSION_STATUS_ACTIVE.into(),
        summary_short: String::new(),
        checkpoint_json: serde_json::json!({}),
        skill_bundle_version: String::new(),
        memory_snapshot_version: String::new(),
        policy_version: CODEX_SESSION_POLICY_VERSION.into(),
        created_at: now,
        last_used_at: now,
        archived_at: None,
    }
}

fn apply_codex_result_to_run(run: &mut AgentCodexTriggerRun, result: &CodexRunResult) {
    run.codex_thread_id = result.thread_id.clone();
    run.exit_code = result.exit_code;
    run.finished_at = Some(now_utc());
    run.final_message_summary = result
        .final_message
        .as_deref()
        .map(|message| truncate(message, 2_000));
    run.error_message = result
        .error_message
        .as_deref()
        .map(|message| truncate(&sanitize_error(message), 2_000));
    run.status = match result.status {
        CodexRunStatus::Succeeded => AGENT_CODEX_RUN_STATUS_SUCCEEDED,
        CodexRunStatus::Failed => AGENT_CODEX_RUN_STATUS_FAILED,
        CodexRunStatus::TimedOut => AGENT_CODEX_RUN_STATUS_TIMED_OUT,
        CodexRunStatus::Cancelled => AGENT_CODEX_RUN_STATUS_CANCELLED,
    }
    .into();
}

fn trigger_execution_from_result(result: &CodexRunResult) -> TriggerExecution {
    TriggerExecution {
        succeeded: matches!(
            result.status,
            CodexRunStatus::Succeeded | CodexRunStatus::Cancelled
        ),
        error_message: (result.status != CodexRunStatus::Cancelled)
            .then_some(result.error_message.clone())
            .flatten(),
        retry_after_seconds: None,
    }
}

#[allow(clippy::too_many_arguments)]
async fn prepare_project_workspace_with_credential_recovery(
    harness: &TriggerHarnessProvisioner,
    workspace_manager: &GitWorkspaceManager,
    company_id: Uuid,
    project_id: Uuid,
    agent_id: Uuid,
    fallback_human_user_id: Uuid,
    agent_handle: &str,
    git: &ai_chat_domain::company::CompanyProjectGitConfig,
) -> AppResult<PreparedGitWorkspace> {
    let prepare = || {
        workspace_manager.prepare_project_workspace(
            company_id,
            project_id,
            agent_id,
            agent_handle,
            git,
        )
    };
    match prepare() {
        Ok(workspace) => Ok(workspace),
        Err(error) if should_refresh_managed_git_credentials(git, &error) => {
            let human_user_id = git
                .created_by_human_user_id
                .unwrap_or(fallback_human_user_id);
            tracing::warn!(
                project_id = %project_id,
                agent_id = %agent_id,
                human_user_id = %human_user_id,
                "managed Git credentials were rejected; refreshing the project token once"
            );
            harness
                .refresh_project_git_credentials(
                    human_user_id,
                    project_id,
                    workspace_manager.credential_store(),
                )
                .await?;
            prepare().map_err(|retry_error| {
                AppError::Validation(format!(
                    "Git authentication still failed after refreshing the managed project credential: {retry_error}"
                ))
            })
        }
        Err(error) => Err(error),
    }
}

fn should_refresh_managed_git_credentials(
    git: &ai_chat_domain::company::CompanyProjectGitConfig,
    error: &AppError,
) -> bool {
    git.auth_profile
        .as_deref()
        .is_some_and(is_managed_token_profile)
        && is_git_authentication_error(error)
}

pub(super) fn protect_trigger_decision<T>(decision: impl FnOnce() -> AppResult<T>) -> AppResult<T> {
    catch_unwind(AssertUnwindSafe(decision)).map_err(|_| {
        AppError::Validation(
            "Codex trigger could not decide Agent work because the decision handler panicked"
                .into(),
        )
    })?
}

pub(super) fn codex_session_key(workspace: &PreparedGitWorkspace) -> String {
    format!("{CODEX_SESSION_POLICY_VERSION}:{}", workspace.worktree_key)
}

pub(super) fn codex_session_key_matches(saved_key: &str, current_key: &str) -> bool {
    saved_key == current_key || saved_key.starts_with(&format!("{current_key}:"))
}

pub(super) fn fail_run(
    platform: &TriggerPlatform,
    run: &mut AgentCodexTriggerRun,
    exit_code: Option<i32>,
    error_message: String,
) -> AppResult<()> {
    run.status = AGENT_CODEX_RUN_STATUS_FAILED.into();
    run.exit_code = exit_code;
    run.finished_at = Some(now_utc());
    run.error_message = Some(truncate(&sanitize_error(&error_message), 2_000));
    platform.update_agent_codex_trigger_run(run.clone())?;
    record_run_activity(
        platform,
        run.id,
        "failed",
        &format!("本轮失败：{}", sanitize_error(&error_message)),
        run.codex_thread_id.clone(),
    );
    Ok(())
}

pub(super) fn record_run_activity(
    platform: &TriggerPlatform,
    run_id: Uuid,
    phase: &str,
    summary: &str,
    codex_thread_id: Option<String>,
) {
    if let Err(error) = platform.append_agent_codex_trigger_run_activity(
        run_id,
        AgentCodexRunActivity {
            at: now_utc(),
            phase: phase.into(),
            summary: truncate(&sanitize_error(summary), 500),
        },
        codex_thread_id,
    ) {
        tracing::warn!(
            run_id = %run_id,
            error = %sanitize_error(&error.to_string()),
            "failed to persist Codex activity"
        );
    }
}
