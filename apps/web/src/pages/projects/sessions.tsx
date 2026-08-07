import { useEffect, useState } from "react";
import { api } from "../../api/client";
import type { CodexSession, CompanyProject } from "../../types/platform";
import { formatTime, StatusBadge } from "../app/shared";

export function ProjectSessionsCard(props: {
  companyId: string;
  project: CompanyProject;
  token: string;
  onError: (error: unknown) => void;
}) {
  const [sessions, setSessions] = useState<Array<{ session: CodexSession; agentName: string }>>([]);
  const [loading, setLoading] = useState(true);

  async function loadSessions() {
    setLoading(true);
    try {
      const results = await Promise.all(props.project.members.map(async (member) => {
        const response = await api<{ sessions: CodexSession[] }>(
          `/api/v1/companies/${props.companyId}/agents/${member.agent_profile.id}/codex-sessions?project_id=${props.project.project.id}`,
          {},
          props.token,
        );
        return response.sessions.map((session) => ({
          session,
          agentName: member.agent_profile.display_name,
        }));
      }));
      setSessions(results.flat().sort((left, right) => right.session.last_used_at.localeCompare(left.session.last_used_at)));
    } catch (error) {
      props.onError(error);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void loadSessions();
  }, [props.companyId, props.project.project.id, props.token]);

  return (
    <div className="project-session-card">
      <div className="project-tab-heading">
        <div><h3>项目工作会话</h3><small>每个 Agent 在本项目拥有独立 Codex 会话、Skill 和记忆范围</small></div>
        <button className="button small" type="button" onClick={() => void loadSessions()} disabled={loading}>{loading ? "读取中…" : "刷新"}</button>
      </div>
      {sessions.length ? (
        <div className="codex-work-session-list">
          {sessions.map(({ session, agentName }) => (
            <div className="codex-work-session-row" key={session.id}>
              <StatusBadge value={session.status} />
              <div><strong>{agentName}</strong><small>第 {session.generation} 代 · {session.scope_key}</small></div>
              <p>{session.summary_short || "尚未生成最近工作总结"}</p>
              <time>{formatTime(session.last_used_at)}</time>
            </div>
          ))}
        </div>
      ) : loading ? null : <div className="empty-inline"><h3>还没有工作会话</h3><p>Agent 第一次接收本项目的 Execution Intent 后会自动创建。</p></div>}
    </div>
  );
}
