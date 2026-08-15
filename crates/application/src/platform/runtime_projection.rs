use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub(super) fn project_agent_runtime(
        &self,
        config: &AgentCodexTriggerConfig,
        runs: &[AgentCodexTriggerRun],
        intents: &[AgentExecutionIntent],
    ) -> AgentRuntimeProjection {
        if config.status == AGENT_CODEX_TRIGGER_STATUS_PAUSED {
            return empty_projection(RUNTIME_STATE_PAUSED, "Agent Trigger 已暂停");
        }
        if config.status == AGENT_CODEX_TRIGGER_STATUS_ERROR {
            return empty_projection(
                RUNTIME_STATE_FAILED,
                config
                    .last_error
                    .as_deref()
                    .unwrap_or("Agent Trigger 运行异常"),
            );
        }
        let now = now_utc();
        if let Some(run) = runs
            .iter()
            .find(|run| run.status == AGENT_CODEX_RUN_STATUS_RUNNING)
        {
            let heartbeat_at = run.heartbeat_at.or(run.last_activity_at);
            let stale =
                heartbeat_at.is_none_or(|at| now.signed_duration_since(at).num_seconds() > 20);
            return AgentRuntimeProjection {
                state: if stale {
                    RUNTIME_STATE_RECOVERING
                } else {
                    state_from_run(run)
                }
                .into(),
                reason: if stale {
                    "运行心跳已停止，Relay 正在恢复或等待 Watchdog 接管".into()
                } else {
                    run.state_reason
                        .clone()
                        .or_else(|| run.activity_summary.clone())
                        .unwrap_or_else(|| "Codex 正在处理当前工作".into())
                },
                session_kind: Some(run.session_kind.clone()),
                run_id: Some(run.id),
                intent_id: run.current_intent_id,
                task_id: run.current_task_id,
                waiting_on_type: run.waiting_on_type.clone(),
                waiting_on_id: run.waiting_on_id,
                heartbeat_at,
                stale,
            };
        }
        if let Some(intent) = intents.first() {
            return AgentRuntimeProjection {
                state: RUNTIME_STATE_RECOVERING.into(),
                reason: "任务尚未完成，等待下一轮从项目会话接续".into(),
                session_kind: Some(AGENT_CODEX_SESSION_KIND_PROJECT.into()),
                run_id: None,
                intent_id: Some(intent.id),
                task_id: intent.task_ids.first().copied(),
                waiting_on_type: None,
                waiting_on_id: None,
                heartbeat_at: None,
                stale: false,
            };
        }
        if config.lease_owner.is_some()
            || config.wake_requested_at.is_some()
            || config.manual_run_requested_at.is_some()
        {
            return empty_projection(RUNTIME_STATE_TRIAGING, "工作已进入 Trigger 队列");
        }
        empty_projection(RUNTIME_STATE_IDLE, "当前空闲，等待可执行任务或消息")
    }
}

fn state_from_run(run: &AgentCodexTriggerRun) -> &'static str {
    match run.activity_phase.as_str() {
        "waiting_approval" => RUNTIME_STATE_WAITING_APPROVAL,
        "reporting" | "finishing" => RUNTIME_STATE_REPORTING,
        "preparing" | "starting" | "session" | "planning" | "dispatching" => RUNTIME_STATE_TRIAGING,
        "continuing" | "retrying" => RUNTIME_STATE_RECOVERING,
        "failed" | "timed_out" | "lease_lost" => RUNTIME_STATE_FAILED,
        _ if run.session_kind == AGENT_CODEX_SESSION_KIND_PROJECT => RUNTIME_STATE_EXECUTING,
        _ => RUNTIME_STATE_TRIAGING,
    }
}

fn empty_projection(state: &str, reason: &str) -> AgentRuntimeProjection {
    AgentRuntimeProjection {
        state: state.into(),
        reason: reason.into(),
        session_kind: None,
        run_id: None,
        intent_id: None,
        task_id: None,
        waiting_on_type: None,
        waiting_on_id: None,
        heartbeat_at: None,
        stale: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waiting_approval_is_never_projected_as_executing() {
        let run = AgentCodexTriggerRun {
            id: Uuid::new_v4(),
            trigger_config_id: Uuid::new_v4(),
            agent_profile_id: Uuid::new_v4(),
            project_id: Some(Uuid::new_v4()),
            trigger_type: "task".into(),
            status: AGENT_CODEX_RUN_STATUS_RUNNING.into(),
            codex_thread_id: None,
            codex_version: None,
            exit_code: None,
            started_at: now_utc(),
            finished_at: None,
            final_message_summary: None,
            error_message: None,
            activity_phase: "waiting_approval".into(),
            activity_summary: Some("等待审批".into()),
            last_activity_at: Some(now_utc()),
            activity_log: Vec::new(),
            process_instance_id: Some("test".into()),
            heartbeat_at: Some(now_utc()),
            state_reason: Some("等待审批".into()),
            current_intent_id: None,
            current_task_id: None,
            waiting_on_type: Some("approval".into()),
            waiting_on_id: None,
            session_kind: AGENT_CODEX_SESSION_KIND_PROJECT.into(),
            resumes_run_id: None,
        };
        assert_eq!(state_from_run(&run), RUNTIME_STATE_WAITING_APPROVAL);
    }
}
