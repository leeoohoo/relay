use super::*;

pub(super) fn persist_codex_stage_session(
    platform: &TriggerPlatform,
    agent_id: Uuid,
    scope_key: &str,
    session_kind: &str,
    project_id: Option<Uuid>,
    workspace: &PreparedGitWorkspace,
    skills: &PreparedRelaySkills,
    memories: &[AgentMemory],
    result: &CodexRunResult,
    replace_session: bool,
) -> AppResult<AgentCodexSession> {
    let thread_id = result.thread_id.clone().ok_or_else(|| {
        AppError::Validation("successful Codex run did not return a thread ID".into())
    })?;
    let existing = platform.get_agent_codex_session(agent_id, scope_key);
    let now = now_utc();
    let replace_session = replace_session || result.replaced_failed_session;
    if replace_session {
        if let Some(mut archived) = existing.clone() {
            archived.status = AGENT_CODEX_SESSION_STATUS_ARCHIVED.into();
            archived.archived_at = Some(now);
            archived.last_used_at = now;
            platform.save_agent_codex_session(archived)?;
        }
    }
    let existing = (!replace_session).then_some(existing).flatten();
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

pub(crate) fn sanitize_workspace_output(value: &str, workspace_path: &Path) -> String {
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

pub(super) fn empty_stage_session(agent_id: Uuid, project_id: Uuid) -> AgentCodexSession {
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

pub(super) fn apply_codex_result_to_run(run: &mut AgentCodexTriggerRun, result: &CodexRunResult) {
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

pub(crate) fn protect_trigger_decision<T>(decision: impl FnOnce() -> AppResult<T>) -> AppResult<T> {
    catch_unwind(AssertUnwindSafe(decision)).map_err(|_| {
        AppError::Validation(
            "Codex trigger could not decide Agent work because the decision handler panicked"
                .into(),
        )
    })?
}

pub(crate) fn codex_session_key(workspace: &PreparedGitWorkspace) -> String {
    format!("{CODEX_SESSION_POLICY_VERSION}:{}", workspace.worktree_key)
}

pub(crate) fn codex_session_key_matches(saved_key: &str, current_key: &str) -> bool {
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
