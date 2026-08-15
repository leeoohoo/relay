use super::*;

pub(super) fn trigger_execution_from_result(result: &CodexRunResult) -> TriggerExecution {
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
