import { useEffect, useState } from "react";
import { api } from "../../api/client";
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
  onError: (error: unknown) => void;
}) {
  const [rows, setRows] = useState<ProjectSessionRow[]>([]);
  const [loading, setLoading] = useState(true);

  async function loadSessions(reportError = true) {
    setLoading(true);
    try {
      const results = await Promise.all(props.project.members.map(async (member) => {
        const agentId = member.agent_profile.id;
        const [sessionResponse, triggerResponse] = await Promise.all([
          api<{ sessions: CodexSession[] }>(
            `/api/v1/companies/${props.companyId}/agents/${agentId}/codex-sessions?project_id=${props.project.project.id}`,
            {},
            props.token,
          ),
          api<{ trigger: CodexTriggerView | null }>(
            `/api/v1/companies/${props.companyId}/agents/${agentId}/codex-trigger`,
            {},
            props.token,
          ),
        ]);
        const runningRun = runningProjectRun(triggerResponse.trigger, props.project.project.id);
        const currentTaskTitles = props.project.tasks
          .filter((task) => task.assignee_agent_id === agentId && task.status === "in_progress")
          .map((task) => task.title);
        const sessions: ProjectSessionRow[] = sessionResponse.sessions
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
      }));
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
      setLoading(false);
    }
  }

  useEffect(() => {
    void loadSessions();
    const refreshTimer = window.setInterval(() => void loadSessions(false), 10_000);
    return () => window.clearInterval(refreshTimer);
  }, [props.companyId, props.project.project.id, props.token]);

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
