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
        },
    };
    let next_run_at = next_trigger_run_at(&trigger, finished_at, completion.succeeded);
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
        });
    }
    let agent = platform.get_agent_profile_by_id(trigger.agent_profile_id)?;
    let membership = platform.get_active_company_agent_membership(trigger.agent_profile_id)?;
    let workspace = match (decision.project.as_ref(), decision.git.as_ref()) {
        (Some(project), Some(git)) => {
            prepare_project_workspace_with_credential_recovery(
                harness,
                workspace_manager,
                trigger.company_id,
                project.id,
                trigger.agent_profile_id,
                agent.owner_user_id,
                &agent.handle,
                git,
            )
            .await?
        }
        _ => workspace_manager
            .prepare_general_workspace(trigger.company_id, trigger.agent_profile_id)?,
    };
    let long_term_memories =
        platform.agent_long_term_memories(trigger.agent_profile_id, trigger.company_id)?;
    let project_view = decision
        .project
        .as_ref()
        .map(|project| {
            platform.get_company_project(GetCompanyProjectInput {
                actor_agent_id: trigger.agent_profile_id,
                company_id: trigger.company_id,
                project_id: project.id,
            })
        })
        .transpose()?;
    let skill_language = platform.effective_company_skill_language(trigger.company_id);
    let relay_skills = prepare_relay_skills(
        &workspace.path,
        &agent,
        &membership.job_title,
        &membership.permissions,
        &long_term_memories,
        project_view.as_ref().map(|view| &view.project),
        project_view.as_ref().and_then(|view| view.rule.as_ref()),
        &skill_language,
    )?;
    let started_at = now_utc();
    let initial_activity = AgentCodexRunActivity {
        at: started_at,
        phase: "preparing".into(),
        summary: format!("正在准备工作区：{}", workspace.branch),
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
    let token_expiry = started_at + Duration::seconds(i64::from(trigger.max_run_seconds) + 60);
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
    let session_key = codex_session_key(&workspace);
    let existing_thread_id = platform
        .get_agent_codex_session(trigger.agent_profile_id)
        .and_then(|session| {
            if codex_session_key_matches(&session.worktree_key, &session_key) {
                Some(session.codex_thread_id)
            } else {
                tracing::info!(
                    agent_id = %trigger.agent_profile_id,
                    "starting a new Codex session because the saved session uses an older workspace or execution policy"
                );
                None
            }
        });
    let request = CodexRunRequest {
        cwd: workspace.path.clone(),
        codex_profile: trigger.codex_profile.clone(),
        model: effective_settings.model.clone(),
        reasoning_effort: effective_settings.reasoning_effort.clone(),
        reasoning_summary: effective_settings.reasoning_summary.clone(),
        verbosity: effective_settings.verbosity.clone(),
        personality: effective_settings.personality.clone(),
        service_tier: effective_settings.service_tier.clone(),
        sandbox_mode: effective_settings.sandbox_mode.clone(),
        approval_policy: effective_settings.approval_policy.clone(),
        network_access: effective_settings.network_access,
        web_search: effective_settings.web_search.clone(),
        feature_multi_agent: effective_settings.feature_multi_agent,
        feature_remote_plugin: effective_settings.feature_remote_plugin,
        feature_hooks: effective_settings.feature_hooks,
        feature_goals: effective_settings.feature_goals,
        feature_shell_tool: effective_settings.feature_shell_tool,
        max_run_seconds: trigger.max_run_seconds as u64,
        prompt: build_wakeup_prompt(WakeupPromptContext {
            agent: &agent,
            project_name: decision
                .project
                .as_ref()
                .map(|project| project.name.as_str()),
            pending_inbox_count: decision.pending_inbox_count,
            active_task_count: decision.active_task_count,
            waiting_task_count: decision.waiting_task_count,
            asset_refresh_due: decision.asset_refresh_due,
            workspace: &workspace,
            relay_skills: &relay_skills,
        }),
        existing_thread_id,
        run_token: token.plaintext_token,
        environment: workspace.auth_environment.clone(),
        approval_handler: (effective_settings.approval_policy == "on-request").then(|| {
            Arc::new(PlatformCodexApprovalHandler {
                platform: platform.clone(),
                company_id: trigger.company_id,
                run_id: run.id,
                agent_id: trigger.agent_profile_id,
                expires_at: started_at + Duration::seconds(i64::from(trigger.max_run_seconds)),
            }) as Arc<dyn CodexApprovalHandler>
        }),
        progress_handler: Some(Arc::new(PlatformCodexProgressHandler {
            platform: platform.clone(),
            run_id: run.id,
        }) as Arc<dyn CodexProgressHandler>),
        cancellation_handler: decision.project.as_ref().map(|project| {
            Arc::new(PlatformProjectCancellationHandler {
                platform: platform.clone(),
                project_id: project.id,
            }) as Arc<dyn CodexCancellationHandler>
        }),
    };
    let result = codex_runner.run(request).await;
    let revoke_result = platform.revoke_agent_codex_run_tokens(run.id);
    if let Err(error) = revoke_result {
        tracing::error!(run_id = %run.id, error = %sanitize_error(&error.to_string()), "failed to revoke Agent Run Token");
    }
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            fail_run(platform, &mut run, None, error.to_string())?;
            return Err(error);
        }
    };
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
    if result.status == CodexRunStatus::Succeeded {
        let Some(thread_id) = result.thread_id else {
            let error =
                AppError::Validation("successful Codex run did not return a thread ID".into());
            fail_run(platform, &mut run, result.exit_code, error.to_string())?;
            return Err(error);
        };
        if let Err(error) = platform.save_agent_codex_session(AgentCodexSession {
            agent_profile_id: trigger.agent_profile_id,
            current_project_id: decision.project.as_ref().map(|project| project.id),
            codex_thread_id: thread_id,
            worktree_key: session_key,
            last_used_at: now_utc(),
        }) {
            fail_run(platform, &mut run, result.exit_code, error.to_string())?;
            return Err(error);
        }
    }
    platform.update_agent_codex_trigger_run(run.clone())?;
    let (final_phase, final_summary) = match result.status {
        CodexRunStatus::Succeeded => ("completed", "Codex 已完成本轮工作"),
        CodexRunStatus::Failed => ("failed", "Codex 本轮执行失败"),
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
        succeeded: matches!(
            result.status,
            CodexRunStatus::Succeeded | CodexRunStatus::Cancelled
        ),
        error_message: (result.status != CodexRunStatus::Cancelled)
            .then_some(result.error_message)
            .flatten(),
    })
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
