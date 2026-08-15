use super::*;

pub(crate) fn next_trigger_run_at(
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
