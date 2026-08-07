import { useEffect, useState } from "react";
import { api } from "../../api/client";
import { Pagination, usePagination } from "../../components/Pagination";
import { Icon } from "../../components/ui";
import type { CodexSession, CompanyAgent, CompanyConsole } from "../../types/platform";
import { Dialog, formatTime, StatusBadge } from "./shared";

export function AgentSessionsDialog(props: {
  agent: CompanyAgent;
  consoleData: CompanyConsole;
  token: string;
  onClose: () => void;
  onError: (error: unknown) => void;
}) {
  const [sessions, setSessions] = useState<CodexSession[]>([]);
  const [loading, setLoading] = useState(true);
  const companyId = props.consoleData.company.id;
  const agentId = props.agent.agent_profile.id;
  const pagination = usePagination(sessions, 8, agentId);

  async function loadSessions() {
    setLoading(true);
    try {
      const response = await api<{ sessions: CodexSession[] }>(
        `/api/v1/companies/${companyId}/agents/${agentId}/codex-sessions?limit=100`,
        {},
        props.token,
      );
      setSessions(response.sessions);
    } catch (error) {
      props.onError(error);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void loadSessions();
  }, [companyId, agentId, props.token]);

  const activeSessions = sessions.filter((session) => session.status === "active");
  const projectSessionCount = activeSessions.filter((session) => session.session_kind === "project").length;

  return (
    <Dialog
      title={`${props.agent.agent_profile.display_name} · Codex 会话`}
      description="控制会话负责消息与派工，项目工作会话按项目独立保存。"
      onClose={props.onClose}
      extraWide
    >
      <div className="agent-session-dialog">
        <div className="agent-session-toolbar">
          <div className="agent-session-counts">
            <span><small>控制会话</small><strong>{activeSessions.some((session) => session.session_kind === "control") ? "1" : "0"}</strong></span>
            <span><small>项目工作会话</small><strong>{projectSessionCount}</strong></span>
            <span><small>全部记录</small><strong>{sessions.length}</strong></span>
          </div>
          <button className="button small" type="button" onClick={() => void loadSessions()} disabled={loading}><Icon name="refresh" /> {loading ? "读取中…" : "刷新"}</button>
        </div>

        {loading && !sessions.length ? (
          <div className="memory-loading"><span className="loader" />正在读取会话…</div>
        ) : sessions.length ? (
          <div className="codex-work-session-list agent-session-list">
            {pagination.pageItems.map((session) => {
              const project = session.project_id
                ? props.consoleData.projects.find((item) => item.project.id === session.project_id)
                : null;
              const title = session.session_kind === "control"
                ? "Relay 控制会话"
                : project?.project.name ?? `项目 ${session.project_id?.slice(0, 8) ?? "未知"}`;
              return (
                <div className="codex-work-session-row" key={session.id}>
                  <StatusBadge value={session.status} />
                  <div><strong>{title}</strong><small>{session.session_kind === "control" ? "消息、协调与派工" : `项目工作 · 第 ${session.generation} 代`}</small></div>
                  <p>{session.summary_short || "尚未生成最近工作总结"}</p>
                  <time>{formatTime(session.last_used_at)}</time>
                </div>
              );
            })}
            <Pagination {...pagination} onPageChange={pagination.setPage} />
          </div>
        ) : (
          <div className="empty-inline"><Icon name="terminal" /><h3>还没有 Codex 会话</h3><p>首次有效唤醒会创建控制会话，实际派工后创建项目工作会话。</p></div>
        )}
      </div>
    </Dialog>
  );
}
