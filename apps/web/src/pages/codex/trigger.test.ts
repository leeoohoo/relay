import { describe, expect, it } from "vitest";
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
});
