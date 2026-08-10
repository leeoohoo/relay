import { describe, expect, it } from "vitest";
import type { CodexSession, CodexTriggerView } from "../../types/platform";
import { projectSessionIsRunning, runningProjectRun } from "./sessions";

const session = {
  id: "session-1",
  project_id: "project-1",
  codex_thread_id: "thread-1",
} as CodexSession;

function trigger(threadId: string | null): CodexTriggerView {
  return {
    recent_runs: [{
      id: "run-1",
      status: "running",
      project_id: "project-1",
      codex_thread_id: threadId,
    }],
  } as CodexTriggerView;
}

describe("project session runtime state", () => {
  it("finds a running project before Codex has returned its thread id", () => {
    const run = runningProjectRun(trigger(null), "project-1");
    expect(run?.id).toBe("run-1");
    expect(projectSessionIsRunning(session, run)).toBe(true);
  });

  it("shows the project as running while Trigger switches from control to worker thread", () => {
    const run = runningProjectRun(trigger("thread-2"), "project-1");
    expect(projectSessionIsRunning(session, run)).toBe(true);
  });
});
