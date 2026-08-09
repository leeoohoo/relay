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
