import { useEffect, useRef, useState } from "react";
import { fetchCodexRuntimeOverview } from "../../api/codexRuntime";
import type { CompanyRealtimeEvent } from "../../api/types";
import type {
  CodexSession,
  CodexTriggerRun,
  CodexTriggerView,
  CompanyProject,
} from "../../types/platform";
import {
  codexActivityPhaseLabel,
  codexSessionTurnLabel,
  formatElapsed,
  formatTime,
  StatusBadge,
} from "../app/shared";

type ProjectSessionRow = {
  agentId: string;
  agentName: string;
  session: CodexSession | null;
  runningRun: CodexTriggerRun | null;
  currentTaskTitles: string[];
};

export function runningProjectRun(
  trigger: CodexTriggerView | null,
  projectId: string,
) {
  return trigger?.recent_runs.find((run) => (
    run.status === "running" && run.project_id === projectId
  )) ?? null;
}

export function projectSessionIsRunning(
  session: CodexSession,
  run: CodexTriggerRun | null,
) {
  return Boolean(run && run.project_id === session.project_id);
}

export function ProjectSessionsCard(props: {
  companyId: string;
  project: CompanyProject;
  token: string;
  realtimeEvent?: CompanyRealtimeEvent | null;
  onError: (error: unknown) => void;
}) {
  const [rows, setRows] = useState<ProjectSessionRow[]>([]);
  const [loading, setLoading] = useState(true);
  const rowsRef = useRef(rows);
  const refreshTimerRef = useRef<number | null>(null);
  const refreshInFlightRef = useRef(false);
  rowsRef.current = rows;

  async function loadSessions(reportError = true) {
    if (reportError) setLoading(true);
    try {
      const response = await fetchCodexRuntimeOverview(
        props.companyId,
        props.project.members.map((member) => member.agent_profile.id),
        props.token,
        props.project.project.id,
      );
      const runtimeByAgent = new Map(response.agents.map((item) => [item.agent_id, item]));
      const results = props.project.members.map((member) => {
        const agentId = member.agent_profile.id;
        const runtime = runtimeByAgent.get(agentId);
        const runningRun = runningProjectRun(runtime?.trigger ?? null, props.project.project.id);
        const currentTaskTitles = props.project.tasks
          .filter((task) => task.assignee_agent_id === agentId && task.status === "in_progress")
          .map((task) => task.title);
        const sessions: ProjectSessionRow[] = (runtime?.sessions ?? [])
          .filter((session) => session.status === "active" && !session.archived_at)
          .map((session) => ({
            agentId,
            agentName: member.agent_profile.display_name,
            session,
            runningRun: projectSessionIsRunning(session, runningRun) ? runningRun : null,
            currentTaskTitles,
          }));
        if (!sessions.length && runningRun) {
          sessions.push({
            agentId,
            agentName: member.agent_profile.display_name,
            session: null,
            runningRun,
            currentTaskTitles,
          });
        }
        return sessions;
      });
      setRows(results.flat().sort((left, right) => {
        if (left.runningRun && !right.runningRun) return -1;
        if (!left.runningRun && right.runningRun) return 1;
        const leftTime = left.runningRun?.started_at ?? left.session?.last_used_at ?? "";
        const rightTime = right.runningRun?.started_at ?? right.session?.last_used_at ?? "";
        return rightTime.localeCompare(leftTime);
      }));
    } catch (error) {
      if (reportError) props.onError(error);
    } finally {
      if (reportError) setLoading(false);
    }
  }

  function scheduleSessionsRefresh() {
    if (refreshTimerRef.current !== null || refreshInFlightRef.current) return;
    refreshTimerRef.current = window.setTimeout(() => {
      refreshTimerRef.current = null;
      refreshInFlightRef.current = true;
      void loadSessions(false).finally(() => { refreshInFlightRef.current = false; });
    }, 1_000);
  }

  useEffect(() => {
    void loadSessions();
  }, [props.companyId, props.project.project.id, props.token]);

  useEffect(() => {
    const event = props.realtimeEvent;
    if (event?.event_type !== "codex.run.updated") return;
    const agentId = typeof event.payload.agent_profile_id === "string"
      ? event.payload.agent_profile_id
      : null;
    if (!agentId || !props.project.members.some((member) => member.agent_profile.id === agentId)) return;
    const runId = typeof event.payload.run_id === "string" ? event.payload.run_id : null;
    const projectId = typeof event.payload.project_id === "string" ? event.payload.project_id : null;
    if (projectId && projectId !== props.project.project.id) return;
    const rowIndex = runId
      ? rowsRef.current.findIndex((row) => row.agentId === agentId && row.runningRun?.id === runId)
      : -1;
    const status = typeof event.payload.status === "string" ? event.payload.status : null;
    if (rowIndex < 0 || status !== "running") {
      // A new run needs its session association, and a terminal run needs its
      // final checkpoint. Coalesce those boundary reads into one request.
      scheduleSessionsRefresh();
      return;
    }
    setRows((current) => current.map((row, index) => index === rowIndex && row.runningRun
      ? {
          ...row,
          runningRun: {
            ...row.runningRun,
            activity_phase: stringPayload(event.payload.activity_phase, row.runningRun.activity_phase),
            activity_summary: nullableStringPayload(event.payload.activity_summary, row.runningRun.activity_summary),
            last_activity_at: nullableStringPayload(event.payload.last_activity_at, row.runningRun.last_activity_at),
            heartbeat_at: nullableStringPayload(event.payload.heartbeat_at, row.runningRun.heartbeat_at ?? null),
            state_reason: nullableStringPayload(event.payload.state_reason, row.runningRun.state_reason ?? null),
            current_intent_id: nullableStringPayload(event.payload.current_intent_id, row.runningRun.current_intent_id ?? null),
            current_task_id: nullableStringPayload(event.payload.current_task_id, row.runningRun.current_task_id ?? null),
          },
        }
      : row));
  }, [props.realtimeEvent?.sequence_id]);

  useEffect(() => () => {
    if (refreshTimerRef.current !== null) window.clearTimeout(refreshTimerRef.current);
  }, []);

  return (
    <div className="project-session-card">
      <div className="project-tab-heading">
        <div><h3>项目工作会话</h3><small>状态来自 Trigger 实时运行记录；总结展示上一轮 checkpoint</small></div>
        <button className="button small" type="button" onClick={() => void loadSessions()} disabled={loading}>{loading ? "读取中…" : "刷新"}</button>
      </div>
      {rows.length ? (
        <div className="codex-work-session-list">
          {rows.map(({ agentId, session, agentName, runningRun, currentTaskTitles }) => (
            <div className="codex-work-session-row" key={session?.id ?? `pending-${agentId}`}>
              <StatusBadge value={runningRun ? "running" : session?.status ?? "queued"} />
              <div>
                <strong>{agentName}</strong>
                <small>{runningRun
                  ? session
                    ? `执行中 · 第 ${session.generation} 代 · 已运行 ${formatElapsed(runningRun.started_at)}`
                    : `正在创建项目工作会话 · 已等待 ${formatElapsed(runningRun.started_at)}`
                  : session
                    ? `第 ${session.generation} 代 · ${codexSessionTurnLabel(session)}`
                    : "正在排队"}</small>
              </div>
              <p>{runningRun
                ? `${currentTaskTitles.length ? `当前任务：${currentTaskTitles.join("、")} · ` : ""}${codexActivityPhaseLabel(runningRun.activity_phase)}：${runningRun.activity_summary ?? "正在连接项目工作会话"}`
                : session?.summary_short
                  ? `上轮总结：${session.summary_short}`
                  : "尚未生成最近工作总结"}</p>
              <time>{runningRun?.last_activity_at || runningRun?.started_at || session?.last_used_at
                ? formatTime(runningRun?.last_activity_at ?? runningRun?.started_at ?? session?.last_used_at ?? "")
                : "—"}</time>
            </div>
          ))}
        </div>
      ) : loading ? null : <div className="empty-inline"><h3>还没有工作会话</h3><p>Agent 第一次接收本项目的 Execution Intent 后会自动创建。</p></div>}
    </div>
  );
}

function stringPayload(value: unknown, fallback: string): string {
  return typeof value === "string" ? value : fallback;
}

function nullableStringPayload(value: unknown, fallback: string | null): string | null {
  return value === null || typeof value === "string" ? value : fallback;
}
