use super::*;

const PROGRESS_CONTINUATION_DELAY_SECONDS: i64 = 10;
const NO_PROGRESS_DELAYS_SECONDS: [i64; 4] = [30, 120, 300, 900];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct IntentProgressState {
    pub fingerprint: String,
    pub executable_task_count: usize,
}

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

pub(super) fn intent_progress_state(
    platform: &TriggerPlatform,
    trigger: &AgentCodexTriggerConfig,
    intent: &AgentExecutionIntent,
) -> AppResult<IntentProgressState> {
    if intent.task_ids.is_empty() {
        return Ok(IntentProgressState {
            fingerprint: hash_secret("no-formal-tasks"),
            executable_task_count: 0,
        });
    }
    let control = platform.agent_control_snapshot(trigger.agent_profile_id, trigger.company_id)?;
    let task_ids = intent.task_ids.iter().copied().collect::<HashSet<_>>();
    let mut ready_tasks = control
        .ready_tasks
        .into_iter()
        .filter(|task| task_ids.contains(&task.id))
        .map(|task| {
            serde_json::json!({
                "id": task.id,
                "status": task.status,
                "assignee_agent_id": task.assignee_agent_id,
                "updated_at": task.updated_at,
            })
        })
        .collect::<Vec<_>>();
    ready_tasks.sort_by_key(|task| task["id"].as_str().unwrap_or_default().to_string());
    let mut waiting_tasks = control
        .waiting_tasks
        .into_iter()
        .filter(|task| task_ids.contains(&task.id))
        .map(|task| {
            serde_json::json!({
                "id": task.id,
                "status": task.status,
                "assignee_agent_id": task.assignee_agent_id,
                "updated_at": task.updated_at,
            })
        })
        .collect::<Vec<_>>();
    waiting_tasks.sort_by_key(|task| task["id"].as_str().unwrap_or_default().to_string());

    let mut execution = Vec::with_capacity(intent.task_ids.len());
    for task_id in &intent.task_ids {
        execution.push(serde_json::json!({
            "task_id": task_id,
            "execution": platform.get_project_task_execution(
                trigger.agent_profile_id,
                trigger.company_id,
                intent.project_id,
                *task_id,
            )?,
        }));
    }
    execution.sort_by_key(|task| task["task_id"].as_str().unwrap_or_default().to_string());
    let snapshot = serde_json::to_string(&(
        ready_tasks.as_slice(),
        waiting_tasks.as_slice(),
        execution.as_slice(),
    ))
    .map_err(|error| AppError::Internal(format!("cannot encode progress state: {error}")))?;
    Ok(IntentProgressState {
        fingerprint: hash_secret(&snapshot),
        executable_task_count: ready_tasks.len(),
    })
}

pub(super) fn continuation_retry_delay(
    recent_runs: &[AgentCodexTriggerRun],
    current_run_id: Uuid,
    intent_id: Uuid,
    made_progress: bool,
) -> i64 {
    if made_progress {
        return PROGRESS_CONTINUATION_DELAY_SECONDS;
    }
    let mut consecutive_no_progress = 0usize;
    for run in recent_runs.iter().filter(|run| run.id != current_run_id) {
        if run.current_intent_id != Some(intent_id) {
            continue;
        }
        if run.activity_phase == "backing_off" {
            consecutive_no_progress += 1;
        } else {
            break;
        }
    }
    NO_PROGRESS_DELAYS_SECONDS[consecutive_no_progress.min(NO_PROGRESS_DELAYS_SECONDS.len() - 1)]
}

pub(super) fn made_structured_progress(
    before: Option<&IntentProgressState>,
    after: Option<&IntentProgressState>,
) -> bool {
    matches!((before, after), (Some(before), Some(after)) if before.fingerprint != after.fingerprint)
}

pub(super) fn executable_tasks_remain(after: Option<&IntentProgressState>) -> bool {
    after.is_none_or(|state| state.executable_task_count > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(intent_id: Uuid, phase: &str) -> AgentCodexTriggerRun {
        let now = now_utc();
        AgentCodexTriggerRun {
            id: Uuid::new_v4(),
            trigger_config_id: Uuid::new_v4(),
            agent_profile_id: Uuid::new_v4(),
            project_id: Some(Uuid::new_v4()),
            trigger_type: "task".into(),
            status: AGENT_CODEX_RUN_STATUS_TIMED_OUT.into(),
            codex_thread_id: Some("thread".into()),
            codex_version: None,
            exit_code: None,
            started_at: now,
            finished_at: Some(now),
            final_message_summary: None,
            error_message: None,
            activity_phase: phase.into(),
            activity_summary: None,
            last_activity_at: Some(now),
            activity_log: Vec::new(),
            process_instance_id: None,
            heartbeat_at: Some(now),
            state_reason: None,
            current_intent_id: Some(intent_id),
            current_task_id: None,
            waiting_on_type: None,
            waiting_on_id: None,
            session_kind: AGENT_CODEX_SESSION_KIND_PROJECT.into(),
            resumes_run_id: None,
        }
    }

    #[test]
    fn continuation_backoff_grows_only_after_repeated_no_progress() {
        let intent_id = Uuid::new_v4();
        let current_run_id = Uuid::new_v4();
        assert_eq!(
            continuation_retry_delay(&[], current_run_id, intent_id, true),
            10
        );
        assert_eq!(
            continuation_retry_delay(&[], current_run_id, intent_id, false),
            30
        );
        let prior = vec![run(intent_id, "backing_off"), run(intent_id, "backing_off")];
        assert_eq!(
            continuation_retry_delay(&prior, current_run_id, intent_id, false),
            300
        );
    }

    #[test]
    fn process_success_does_not_complete_unfinished_formal_tasks() {
        assert!(executable_tasks_remain(None));
        assert!(executable_tasks_remain(Some(&IntentProgressState {
            fingerprint: "same".into(),
            executable_task_count: 1,
        })));
        assert!(!executable_tasks_remain(Some(&IntentProgressState {
            fingerprint: "done".into(),
            executable_task_count: 0,
        })));
    }
}
