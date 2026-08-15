use super::*;

mod capabilities;
mod run_token_guard;
mod scheduling;
mod session_state;
mod settings;
mod workspace;

use capabilities::*;
use run_token_guard::ManagedBrowserRunTokenGuard;
pub(super) use scheduling::next_trigger_run_at;
use scheduling::{
    continuation_retry_delay, executable_tasks_remain, intent_progress_state,
    made_structured_progress,
};
use session_state::{
    apply_codex_result_to_run, empty_stage_session, fail_run, persist_codex_stage_session,
};
pub(super) use session_state::{
    codex_session_key, codex_session_key_matches, protect_trigger_decision,
    sanitize_workspace_output,
};
pub(super) use settings::resolve_effective_cli_settings;
use workspace::prepare_project_workspace_with_credential_recovery;

pub(super) async fn process_claimed_trigger(
    platform: &TriggerPlatform,
    harness: &TriggerHarnessProvisioner,
    workspace_manager: &GitWorkspaceManager,
    codex_runner: &CodexTriggerRunner,
    codex_control: &CodexControlStore,
    service_config: &TriggerServiceConfig,
    trigger: AgentCodexTriggerConfig,
) -> Uuid {
    let execution = execute_trigger(
        platform,
        harness,
        workspace_manager,
        codex_runner,
        codex_control,
        service_config,
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
    trigger.agent_profile_id
}

pub(super) async fn execute_trigger(
    platform: &TriggerPlatform,
    harness: &TriggerHarnessProvisioner,
    workspace_manager: &GitWorkspaceManager,
    codex_runner: &CodexTriggerRunner,
    codex_control: &CodexControlStore,
    service_config: &TriggerServiceConfig,
    trigger: &AgentCodexTriggerConfig,
) -> AppResult<TriggerExecution> {
    if !platform.is_agent_codex_trigger_active(trigger.agent_profile_id)? {
        return Ok(TriggerExecution {
            succeeded: true,
            error_message: None,
            retry_after_seconds: None,
        });
    }
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
        process_instance_id: Some(service_config.lease_owner.clone()),
        heartbeat_at: Some(started_at),
        state_reason: Some("正在准备 Agent 控制会话".into()),
        current_intent_id: None,
        current_task_id: None,
        waiting_on_type: None,
        waiting_on_id: None,
        session_kind: AGENT_CODEX_SESSION_KIND_CONTROL.into(),
        resumes_run_id: platform
            .list_agent_codex_trigger_runs(trigger.agent_profile_id, 5)
            .into_iter()
            .find(|candidate| candidate.status == AGENT_CODEX_RUN_STATUS_RESTARTED)
            .map(|candidate| candidate.id),
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
    let _browser_token_guard =
        ManagedBrowserRunTokenGuard::new(codex_runner, &token.plaintext_token);
    if decision.resume_existing_intents_directly {
        record_run_activity(
            platform,
            run.id,
            "continuing",
            "已有项目工作已完成分诊，正在从原工作会话继续",
            None,
        );
    } else {
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
            control_snapshot: &decision.control_snapshot,
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
            false,
            false,
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
        if !platform.is_agent_codex_trigger_active(trigger.agent_profile_id)? {
            let _ = platform.revoke_agent_codex_run_tokens(run.id);
            run.status = AGENT_CODEX_RUN_STATUS_CANCELLED.into();
            run.finished_at = Some(now_utc());
            run.error_message = Some("Agent Trigger was paused by Human".into());
            platform.update_agent_codex_trigger_run(run.clone())?;
            record_run_activity(
                platform,
                run.id,
                "cancelled",
                "Agent 已暂停，本轮已停止；待处理工作将在恢复后继续",
                run.codex_thread_id.clone(),
            );
            return Ok(TriggerExecution {
                succeeded: true,
                error_message: None,
                retry_after_seconds: None,
            });
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
            false,
        )?;
        run.codex_thread_id = Some(control_session.codex_thread_id.clone());
        run.final_message_summary = control_result
            .final_message
            .as_deref()
            .map(|message| truncate(message, 2_000));
    }
    let mut worker_failure = None;
    let mut worker_retry_requested = false;
    let mut worker_retry_counts_as_failure = false;
    let mut worker_capability_upgrade_requested = false;
    let mut worker_retry_after_seconds = 10;
    let mut worker_continuation_summary = None;
    let mut worker_no_progress = false;
    let mut worker_completed_intent = false;
    let intents = platform.list_agent_execution_intents(
        trigger.agent_profile_id,
        Some(AGENT_EXECUTION_INTENT_STATUS_PENDING),
        1,
    );
    for mut intent in intents {
        if !platform.is_agent_codex_trigger_active(trigger.agent_profile_id)? {
            break;
        }
        if platform.is_company_project_paused(intent.project_id)? {
            continue;
        }
        intent.status = AGENT_EXECUTION_INTENT_STATUS_RUNNING.into();
        intent.claimed_at = Some(now_utc());
        platform.update_agent_execution_intent(intent.clone())?;
        run.project_id = Some(intent.project_id);
        run.current_intent_id = Some(intent.id);
        run.current_task_id = intent.task_ids.first().copied();
        run.session_kind = AGENT_CODEX_SESSION_KIND_PROJECT.into();
        run.state_reason = Some(intent.objective.clone());
        platform.update_agent_codex_trigger_run(run.clone())?;
        record_run_activity(
            platform,
            run.id,
            "dispatching",
            &format!("正在进入项目工作会话：{}", intent.project_id),
            None,
        );
        let progress_before = intent_progress_state(platform, trigger, &intent).ok();
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
        let worker_result = match handle_browser_capability_upgrade(
            platform,
            &mut run,
            &mut intent,
            worker_result,
        )? {
            BrowserCapabilityUpgrade::Unchanged(result) => result,
            BrowserCapabilityUpgrade::RetryAfterResult(result) => {
                worker_retry_requested = true;
                worker_capability_upgrade_requested = true;
                worker_retry_after_seconds = 1;
                worker_failure = Some(result);
                break;
            }
            BrowserCapabilityUpgrade::RetryAfterError => {
                return Ok(TriggerExecution {
                    succeeded: true,
                    error_message: None,
                    retry_after_seconds: Some(1),
                });
            }
        };
        match worker_result {
            Ok((result, session)) if result.status == CodexRunStatus::Succeeded => {
                intent.worker_session_id = Some(session.id);
                intent.result_summary = result
                    .final_message
                    .as_deref()
                    .map(|message| truncate(message, 4_000))
                    .unwrap_or_default();
                run.project_id = session.project_id;
                run.codex_thread_id = Some(session.codex_thread_id);
                run.final_message_summary = result
                    .final_message
                    .as_deref()
                    .map(|message| truncate(message, 2_000));
                let progress_after = intent_progress_state(platform, trigger, &intent).ok();
                if executable_tasks_remain(progress_after.as_ref()) {
                    let made_progress =
                        made_structured_progress(progress_before.as_ref(), progress_after.as_ref());
                    worker_retry_after_seconds = continuation_retry_delay(
                        &platform.list_agent_codex_trigger_runs(trigger.agent_profile_id, 20),
                        run.id,
                        intent.id,
                        made_progress,
                    );
                    worker_no_progress = !made_progress;
                    worker_continuation_summary = Some(if made_progress {
                        "Codex 本轮已结束，但正式任务仍未完成；已保存进展并将从同一项目会话继续"
                            .into()
                    } else {
                        format!(
                            "正式任务仍未完成，且本轮未检测到任务、Attempt 或 Evidence 变化；已退避 {} 秒后继续",
                            worker_retry_after_seconds
                        )
                    });
                    intent.status = AGENT_EXECUTION_INTENT_STATUS_PENDING.into();
                    intent.error_message = worker_continuation_summary.clone();
                    intent.claimed_at = None;
                    intent.completed_at = None;
                    platform.update_agent_execution_intent(intent)?;
                    run.waiting_on_type = worker_no_progress.then(|| "progress_backoff".into());
                    run.state_reason = worker_continuation_summary.clone();
                    worker_retry_requested = true;
                    worker_failure = Some(result);
                    break;
                }
                intent.status = AGENT_EXECUTION_INTENT_STATUS_COMPLETED.into();
                intent.error_message = None;
                intent.completed_at = Some(now_utc());
                platform.update_agent_execution_intent(intent)?;
                worker_completed_intent = true;
            }
            Ok((result, session)) if result.status == CodexRunStatus::TimedOut => {
                intent.status = AGENT_EXECUTION_INTENT_STATUS_PENDING.into();
                intent.worker_session_id = Some(session.id);
                intent.result_summary = result
                    .final_message
                    .as_deref()
                    .map(|message| truncate(message, 4_000))
                    .unwrap_or_default();
                intent.claimed_at = None;
                intent.completed_at = None;
                run.project_id = session.project_id;
                run.codex_thread_id = Some(session.codex_thread_id);
                run.final_message_summary = result
                    .final_message
                    .as_deref()
                    .map(|message| truncate(message, 2_000));
                let progress_after = intent_progress_state(platform, trigger, &intent).ok();
                let made_progress =
                    made_structured_progress(progress_before.as_ref(), progress_after.as_ref());
                worker_retry_after_seconds = continuation_retry_delay(
                    &platform.list_agent_codex_trigger_runs(trigger.agent_profile_id, 20),
                    run.id,
                    intent.id,
                    made_progress,
                );
                worker_no_progress = !made_progress;
                worker_continuation_summary = Some(if made_progress {
                    "本轮达到运行时间上限，已保存结构化进展并将从同一项目会话继续".into()
                } else {
                    format!(
                        "本轮达到运行时间上限，但未检测到任务、Attempt 或 Evidence 变化；已退避 {} 秒后继续",
                        worker_retry_after_seconds
                    )
                });
                intent.error_message = worker_continuation_summary.clone();
                platform.update_agent_execution_intent(intent)?;
                run.waiting_on_type = worker_no_progress.then(|| "progress_backoff".into());
                run.state_reason = worker_continuation_summary.clone();
                worker_retry_requested = true;
                worker_failure = Some(result);
                break;
            }
            Ok((result, session)) => {
                if session.id != Uuid::nil() {
                    intent.worker_session_id = Some(session.id);
                }
                intent.result_summary = result
                    .final_message
                    .as_deref()
                    .map(|message| truncate(message, 4_000))
                    .unwrap_or_default();
                let failure_message = result.error_message.clone().unwrap_or_default();
                if result.status == CodexRunStatus::Failed
                    && platform
                        .requeue_agent_execution_intent_after_retryable_failure(
                            intent.clone(),
                            &failure_message,
                        )?
                        .is_some()
                {
                    run.project_id = session.project_id;
                    run.codex_thread_id = Some(session.codex_thread_id);
                    run.final_message_summary = result
                        .final_message
                        .as_deref()
                        .map(|message| truncate(message, 2_000));
                    record_run_activity(
                        platform,
                        run.id,
                        "retrying",
                        "项目工作会话遇到临时服务故障，已保留进度并将在 10 秒后重试",
                        run.codex_thread_id.clone(),
                    );
                    worker_retry_requested = true;
                    worker_retry_counts_as_failure = true;
                    worker_failure = Some(result);
                    break;
                }
                intent.status = if result.status == CodexRunStatus::Cancelled {
                    AGENT_EXECUTION_INTENT_STATUS_PENDING
                } else {
                    AGENT_EXECUTION_INTENT_STATUS_FAILED
                }
                .into();
                intent.error_message = result.error_message.clone();
                intent.result_summary = result.final_message.clone().unwrap_or_default();
                intent.claimed_at = None;
                intent.completed_at = (result.status != CodexRunStatus::Cancelled).then(now_utc);
                platform.update_agent_execution_intent(intent)?;
                worker_failure = Some(result);
                break;
            }
            Err(error) => {
                let failure_message = sanitize_error(&error.to_string());
                let retry_requested = platform
                    .requeue_agent_execution_intent_after_retryable_failure(
                        intent.clone(),
                        &failure_message,
                    )?
                    .is_some();
                if !retry_requested {
                    intent.status = AGENT_EXECUTION_INTENT_STATUS_FAILED.into();
                    intent.error_message = Some(failure_message.clone());
                    intent.completed_at = Some(now_utc());
                    platform.update_agent_execution_intent(intent)?;
                }
                let _ = platform.revoke_agent_codex_run_tokens(run.id);
                fail_run(platform, &mut run, None, error.to_string())?;
                if retry_requested {
                    record_run_activity(
                        platform,
                        run.id,
                        "retrying",
                        "项目工作会话遇到临时服务故障，将在 10 秒后重试",
                        run.codex_thread_id.clone(),
                    );
                    return Ok(TriggerExecution {
                        succeeded: false,
                        error_message: Some(failure_message),
                        retry_after_seconds: Some(10),
                    });
                }
                return Err(error);
            }
        }
    }
    if worker_completed_intent
        && !worker_retry_requested
        && !platform
            .list_agent_execution_intents(
                trigger.agent_profile_id,
                Some(AGENT_EXECUTION_INTENT_STATUS_PENDING),
                1,
            )
            .is_empty()
    {
        worker_retry_requested = true;
        worker_retry_after_seconds = 1;
        worker_continuation_summary =
            Some("本轮正式任务已完成，队列中仍有其他工作，将公平释放运行槽位后继续".into());
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
    let (final_phase, final_summary) = if worker_capability_upgrade_requested {
        ("continuing", "已按需启用浏览器能力，即将从同一项目会话继续")
    } else if let Some(summary) = worker_continuation_summary.as_deref() {
        (
            if worker_no_progress {
                "backing_off"
            } else {
                "continuing"
            },
            summary,
        )
    } else {
        match final_status {
            CodexRunStatus::Succeeded => ("completed", "Codex 已完成本轮工作"),
            CodexRunStatus::Failed if worker_retry_requested => {
                ("retrying", "临时服务故障，已保留工作进度并即将重试")
            }
            CodexRunStatus::Failed => ("failed", "Codex 本轮执行失败"),
            CodexRunStatus::TimedOut if worker_retry_requested => {
                ("continuing", "已保存当前进度，即将从同一项目会话继续")
            }
            CodexRunStatus::TimedOut => ("timed_out", "Codex 本轮执行超时"),
            CodexRunStatus::Cancelled => ("cancelled", "项目已暂停，Codex 本轮已停止"),
        }
    };
    record_run_activity(
        platform,
        run.id,
        final_phase,
        final_summary,
        run.codex_thread_id.clone(),
    );
    Ok(TriggerExecution {
        succeeded: worker_failure.is_none()
            || (worker_retry_requested && !worker_retry_counts_as_failure),
        error_message: (!worker_retry_requested || worker_retry_counts_as_failure)
            .then_some(run.error_message)
            .flatten(),
        retry_after_seconds: worker_retry_requested.then_some(worker_retry_after_seconds),
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
        &project_view.project.name,
        &project_view.project.description,
        &git,
    )
    .await?;
    let scope_key = format!("project:{}", intent.project_id);
    let replace_session = intent.action_type == AGENT_EXECUTION_INTENT_ACTION_REPLACE_SESSION;
    let existing_session = platform.get_agent_codex_session(trigger.agent_profile_id, &scope_key);
    let worker_session_id = (!replace_session)
        .then(|| existing_session.as_ref().map(|session| session.id))
        .flatten();
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
        previous_checkpoint: replace_session
            .then(|| {
                existing_session
                    .as_ref()
                    .map(|session| session.summary_short.as_str())
            })
            .flatten(),
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
        replace_session,
        intent
            .required_capabilities
            .iter()
            .any(|capability| capability == AGENT_EXECUTION_CAPABILITY_BROWSER),
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
            replace_session,
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
    replace_session: bool,
    browser_enabled: bool,
) -> AppResult<CodexRunResult> {
    let session_key = codex_session_key(workspace);
    let existing_thread_id = (!replace_session)
        .then(|| platform.get_agent_codex_session(trigger.agent_profile_id, scope_key))
        .flatten()
        .and_then(|session| {
            codex_session_key_matches(&session.workspace_key, &session_key)
                .then_some(session.codex_thread_id)
        });
    let managed_mcp_servers = project_id
        .filter(|_| browser_enabled)
        .map(|project_id| {
            codex_runner.managed_browser_mcp_server(
                trigger.company_id,
                trigger.agent_profile_id,
                project_id,
                &workspace.path,
                run_token,
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
    let mut result = run_with_heartbeat(
        platform,
        run.id,
        codex_runner.run(CodexRunRequest {
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
            prompt: normalize_codex_prompt(&prompt),
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
                        session_website_grants: Arc::new(Mutex::new(HashSet::new())),
                    }) as Arc<dyn CodexApprovalHandler>
                }),
            progress_handler: Some(Arc::new(PlatformCodexProgressHandler {
                platform: platform.clone(),
                run_id: run.id,
            }) as Arc<dyn CodexProgressHandler>),
            cancellation_handler: Some(Arc::new(PlatformRunCancellationHandler {
                platform: platform.clone(),
                agent_id: trigger.agent_profile_id,
                project_id,
            }) as Arc<dyn CodexCancellationHandler>),
        }),
    )
    .await??;
    if let Some(message) = result.final_message.as_mut() {
        *message = sanitize_workspace_output(message, &workspace.path);
    }
    if let Some(message) = result.error_message.as_mut() {
        *message = sanitize_workspace_output(message, &workspace.path);
    }
    Ok(result)
}
