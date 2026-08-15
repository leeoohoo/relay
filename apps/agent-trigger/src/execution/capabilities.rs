use super::*;

pub(super) enum BrowserCapabilityUpgrade {
    Unchanged(AppResult<(CodexRunResult, AgentCodexSession)>),
    RetryAfterResult(CodexRunResult),
    RetryAfterError,
}

pub(super) fn handle_browser_capability_upgrade(
    platform: &TriggerPlatform,
    run: &mut AgentCodexTriggerRun,
    intent: &mut AgentExecutionIntent,
    worker_result: AppResult<(CodexRunResult, AgentCodexSession)>,
) -> AppResult<BrowserCapabilityUpgrade> {
    let latest_intent = platform
        .get_agent_execution_intent(intent.id)
        .unwrap_or_else(|| intent.clone());
    let requested = !intent
        .required_capabilities
        .iter()
        .any(|capability| capability == AGENT_EXECUTION_CAPABILITY_BROWSER)
        && latest_intent
            .required_capabilities
            .iter()
            .any(|capability| capability == AGENT_EXECUTION_CAPABILITY_BROWSER);
    if !requested {
        return Ok(BrowserCapabilityUpgrade::Unchanged(worker_result));
    }

    *intent = latest_intent;
    intent.status = AGENT_EXECUTION_INTENT_STATUS_PENDING.into();
    intent.claimed_at = None;
    intent.completed_at = None;
    match worker_result {
        Ok((result, session)) => {
            intent.worker_session_id = Some(session.id);
            intent.result_summary = result
                .final_message
                .as_deref()
                .map(|message| truncate(message, 4_000))
                .unwrap_or_default();
            intent.error_message =
                Some("工作会话已申请浏览器能力，Relay 将从同一项目会话继续".into());
            platform.update_agent_execution_intent(intent.clone())?;
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
                "项目工作会话已按需申请浏览器能力，即将从同一会话继续",
                run.codex_thread_id.clone(),
            );
            Ok(BrowserCapabilityUpgrade::RetryAfterResult(result))
        }
        Err(error) => {
            tracing::warn!(
                intent_id = %intent.id,
                error = %sanitize_error(&error.to_string()),
                "browser capability was requested before the worker turn returned an error"
            );
            intent.error_message =
                Some("浏览器能力申请已保存；上一个无浏览器 Turn 已结束，Relay 将继续".into());
            platform.update_agent_execution_intent(intent.clone())?;
            let _ = platform.revoke_agent_codex_run_tokens(run.id);
            run.status = AGENT_CODEX_RUN_STATUS_RESTARTED.into();
            run.finished_at = Some(now_utc());
            run.error_message = None;
            platform.update_agent_codex_trigger_run(run.clone())?;
            record_run_activity(
                platform,
                run.id,
                "continuing",
                "浏览器能力申请已保存，即将重新进入同一项目工作会话",
                run.codex_thread_id.clone(),
            );
            Ok(BrowserCapabilityUpgrade::RetryAfterError)
        }
    }
}
