import { describe, expect, it } from "vitest";
import { applyCodexRealtimeEvent } from "../../api/codexRuntime";
import type { CompanyRealtimeEvent } from "../../api/types";
import type { CodexSession, CodexTriggerView } from "../../types/platform";
import { currentTriggerSessions, triggerRunUsesSession } from "./trigger";

function session(overrides: Partial<CodexSession>): CodexSession {
  return {
    id: "session-current",
    agent_profile_id: "agent-1",
    session_kind: "project",
    scope_key: "project:project-1",
    project_id: "project-1",
    generation: 2,
    codex_thread_id: "thread-current",
    workspace_key: "workspace-current",
    status: "active",
    summary_short: "current summary",
    checkpoint_json: {},
    skill_bundle_version: "v1",
    memory_snapshot_version: "v1",
    policy_version: "v1",
    created_at: "2026-08-08T10:00:00Z",
    last_used_at: "2026-08-08T11:00:00Z",
    archived_at: null,
    ...overrides,
  };
}

describe("Codex Trigger work session directory", () => {
  it("shows only the current generation for a project", () => {
    const current = session({});
    const archived = session({
      id: "session-archived",
      generation: 1,
      codex_thread_id: "thread-archived",
      status: "archived",
      archived_at: "2026-08-08T09:00:00Z",
    });

    expect(currentTriggerSessions([current, archived])).toEqual([current]);
  });

  it("marks only the exact Codex thread as running", () => {
    const current = session({});
    const archived = session({
      id: "session-archived",
      codex_thread_id: "thread-archived",
      status: "archived",
      archived_at: "2026-08-08T09:00:00Z",
    });
    const runs = [{
      status: "running",
      project_id: "project-1",
      codex_thread_id: "thread-current",
    }] as CodexTriggerView["recent_runs"];

    expect(triggerRunUsesSession(current, runs)).toBe(true);
    expect(triggerRunUsesSession(archived, runs)).toBe(false);
  });

  it("projects live run activity without another HTTP request", () => {
    const current = triggerView();
    const event = realtimeEvent("codex.run.updated", {
      agent_profile_id: "agent-1",
      run_id: "run-1",
      status: "running",
      activity_phase: "tool",
      activity_summary: "完成调用工具：company.task",
      last_activity_at: "2026-08-08T11:02:00Z",
      finished_at: null,
    });

    const projection = applyCodexRealtimeEvent(current, event);
    expect(projection.refreshRequired).toBe(false);
    expect(projection.trigger?.recent_runs[0]).toMatchObject({
      activity_phase: "tool",
      activity_summary: "完成调用工具：company.task",
      last_activity_at: "2026-08-08T11:02:00Z",
    });
  });

  it("requests one final refresh when a run becomes terminal", () => {
    const projection = applyCodexRealtimeEvent(
      triggerView(),
      realtimeEvent("codex.run.updated", {
        agent_profile_id: "agent-1",
        run_id: "run-1",
        status: "succeeded",
        activity_phase: "completed",
        activity_summary: "Codex 已完成本轮工作",
        last_activity_at: "2026-08-08T11:03:00Z",
        finished_at: "2026-08-08T11:03:00Z",
      }),
    );
    expect(projection.refreshRequired).toBe(true);
    expect(projection.trigger?.recent_runs[0]?.status).toBe("succeeded");
  });
});

function triggerView(): CodexTriggerView {
  return {
    runner_profile_id: "profile-1",
    config: {
      id: "trigger-1",
      status: "active",
      interval_seconds: 3600,
      codex_profile: "default",
      model: null,
      reasoning_effort: null,
      reasoning_summary: null,
      verbosity: null,
      personality: null,
      service_tier: null,
      sandbox_mode: "inherit",
      approval_policy: "inherit",
      network_access: null,
      web_search: null,
      feature_multi_agent: null,
      feature_remote_plugin: null,
      feature_hooks: null,
      feature_goals: null,
      feature_shell_tool: null,
      max_run_seconds: 3600,
      next_run_at: "2026-08-08T12:00:00Z",
      lease_owner: "trigger",
      lease_expires_at: null,
      manual_run_requested_at: null,
      wake_requested_at: null,
      wake_reason: null,
      last_run_at: null,
      last_success_at: null,
      last_error: null,
      consecutive_failure_count: 0,
    },
    recent_runs: [{
      id: "run-1",
      trigger_type: "task",
      status: "running",
      project_id: "project-1",
      codex_thread_id: "thread-1",
      started_at: "2026-08-08T11:00:00Z",
      activity_phase: "planning",
      activity_summary: "开始执行",
      last_activity_at: "2026-08-08T11:00:00Z",
      finished_at: null,
      final_message_summary: null,
      error_message: null,
      activity_log: [],
    }],
    active_intents: [],
    recent_sessions: [],
  };
}

function realtimeEvent(eventType: string, payload: Record<string, unknown>): CompanyRealtimeEvent {
  return {
    sequence_id: 1,
    id: "event-1",
    company_id: "company-1",
    event_type: eventType,
    aggregate_type: "agent_codex_trigger_run",
    aggregate_id: "run-1",
    actor_agent_id: "agent-1",
    actor_human_user_id: null,
    payload,
    created_at: "2026-08-08T11:02:00Z",
  };
}
