import { api } from "./client";
import type { CompanyRealtimeEvent } from "./types";
import type { CodexSession, CodexTriggerView } from "../types/platform";

export type AgentCodexRuntimeOverview = {
  agent_id: string;
  trigger: CodexTriggerView | null;
  sessions: CodexSession[];
};

export function fetchCodexRuntimeOverview(
  companyId: string,
  agentIds: string[],
  token: string,
  projectId?: string,
) {
  const params = new URLSearchParams({ agent_ids: agentIds.join(",") });
  if (projectId) params.set("project_id", projectId);
  return api<{ agents: AgentCodexRuntimeOverview[] }>(
    `/api/v1/companies/${companyId}/codex-runtime-overview?${params.toString()}`,
    {},
    token,
  );
}

export function applyCodexRealtimeEvent(
  current: CodexTriggerView | null,
  event: CompanyRealtimeEvent,
): { trigger: CodexTriggerView | null; refreshRequired: boolean } {
  if (!current) return { trigger: current, refreshRequired: true };

  if (event.event_type === "codex.trigger.updated") {
    const payload = event.payload;
    return {
      trigger: {
        ...current,
        config: {
          ...current.config,
          status: triggerStatusValue(payload.status) ?? current.config.status,
          lease_owner: nullableStringValue(payload.lease_owner, current.config.lease_owner),
          lease_expires_at: nullableStringValue(payload.lease_expires_at, current.config.lease_expires_at),
          wake_requested_at: nullableStringValue(payload.wake_requested_at, current.config.wake_requested_at),
          last_run_at: nullableStringValue(payload.last_run_at, current.config.last_run_at),
          last_success_at: nullableStringValue(payload.last_success_at, current.config.last_success_at),
          last_error: nullableStringValue(payload.last_error, current.config.last_error),
        },
      },
      refreshRequired: false,
    };
  }

  if (event.event_type !== "codex.run.updated") {
    return { trigger: current, refreshRequired: false };
  }

  const runId = stringValue(event.payload.run_id);
  const runIndex = runId
    ? current.recent_runs.findIndex((run) => run.id === runId)
    : -1;
  if (runIndex < 0) return { trigger: current, refreshRequired: true };

  const existing = current.recent_runs[runIndex];
  const activityPhase = stringValue(event.payload.activity_phase) ?? existing.activity_phase;
  const activitySummary = nullableStringValue(event.payload.activity_summary, existing.activity_summary);
  const lastActivityAt = nullableStringValue(event.payload.last_activity_at, existing.last_activity_at);
  const status = stringValue(event.payload.status) ?? existing.status;
  const activityLog = [...existing.activity_log];
  if (
    lastActivityAt
    && activitySummary
    && !activityLog.some((activity) => activity.at === lastActivityAt && activity.summary === activitySummary)
  ) {
    activityLog.push({ at: lastActivityAt, phase: activityPhase, summary: activitySummary });
    if (activityLog.length > 40) activityLog.splice(0, activityLog.length - 40);
  }
  const recentRuns = [...current.recent_runs];
  recentRuns[runIndex] = {
    ...existing,
    status,
    project_id: nullableStringValue(event.payload.project_id, existing.project_id),
    activity_phase: activityPhase,
    activity_summary: activitySummary,
    last_activity_at: lastActivityAt,
    finished_at: nullableStringValue(event.payload.finished_at, existing.finished_at),
    heartbeat_at: nullableStringValue(event.payload.heartbeat_at, existing.heartbeat_at ?? null),
    state_reason: nullableStringValue(event.payload.state_reason, existing.state_reason ?? null),
    current_intent_id: nullableStringValue(
      event.payload.current_intent_id,
      existing.current_intent_id ?? null,
    ),
    current_task_id: nullableStringValue(
      event.payload.current_task_id,
      existing.current_task_id ?? null,
    ),
    waiting_on_type: nullableStringValue(
      event.payload.waiting_on_type,
      existing.waiting_on_type ?? null,
    ),
    waiting_on_id: nullableStringValue(event.payload.waiting_on_id, existing.waiting_on_id ?? null),
    session_kind: sessionKindValue(event.payload.session_kind) ?? existing.session_kind,
    activity_log: activityLog,
  };
  return {
    trigger: { ...current, recent_runs: recentRuns },
    // One terminal refresh picks up the final checkpoint/session summary. Live
    // activity updates are fully projected from SSE without another request.
    refreshRequired: status !== "running",
  };
}

function stringValue(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function nullableStringValue(value: unknown, fallback: string | null): string | null {
  return value === null || typeof value === "string" ? value : fallback;
}

function triggerStatusValue(value: unknown): CodexTriggerView["config"]["status"] | null {
  return value === "active" || value === "paused" || value === "error" ? value : null;
}

function sessionKindValue(value: unknown): "control" | "project" | null {
  return value === "control" || value === "project" ? value : null;
}
