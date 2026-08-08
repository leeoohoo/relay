import { useEffect, useState } from "react";
import { api } from "../../api/client";
import type { CompanyRealtimeEvent } from "../../api/types";
import { Field } from "../../components/ui";
import { useUiLanguage } from "../../i18n/uiLanguage";
import type { CodexRunnerProfileView, CodexSession, CodexTriggerView } from "../../types/platform";
import { codexActivityPhaseLabel, codexOperationalStatusLabel, codexReasoningEffortLabel, codexRunDisplayMessage, codexSessionTurnLabel, codexTriggerStatusLabel, codexTriggerTypeLabel, formatElapsed, formatInterval, formatRunSeconds, formatTime, StatusBadge } from "../app/shared";

export function CodexTriggerPanel(props: {
  companyId: string;
  agentId: string;
  agentName: string;
  projects: Array<{ id: string; name: string }>;
  active: boolean;
  profiles: CodexRunnerProfileView[];
  token: string;
  realtimeEvent: CompanyRealtimeEvent | null;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
  onAssigned: () => Promise<void>;
}) {
  const { language } = useUiLanguage();
  const [trigger, setTrigger] = useState<CodexTriggerView | null>(null);
  const [sessions, setSessions] = useState<CodexSession[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const selectedProfile = props.profiles.find((item) => item.profile.id === selectedProfileId)?.profile;
  const runningRun = trigger?.recent_runs.find((run) => run.status === "running") ?? null;
  const operationalStatus = trigger
    ? trigger.config.status !== "active"
      ? trigger.config.status
      : runningRun
        ? "running"
        : trigger.config.lease_owner
          ? "queued"
          : "idle"
    : null;

  const endpoint = `/api/v1/companies/${props.companyId}/agents/${props.agentId}/codex-trigger`;
  const sessionsEndpoint = `/api/v1/companies/${props.companyId}/agents/${props.agentId}/codex-sessions`;

  function applyTrigger(next: CodexTriggerView | null) {
    setTrigger(next);
    setSelectedProfileId(next?.runner_profile_id ?? "");
  }

  useEffect(() => {
    let active = true;
    setLoading(true);
    Promise.all([
      api<{ trigger: CodexTriggerView | null }>(endpoint, {}, props.token),
      api<{ sessions: CodexSession[] }>(sessionsEndpoint, {}, props.token),
    ])
      .then(([{ trigger }, { sessions }]) => { if (active) { applyTrigger(trigger); setSessions(sessions); } })
      .catch(props.onError)
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [endpoint, sessionsEndpoint, props.token]);

  useEffect(() => {
    const event = props.realtimeEvent;
    if (!event || !event.event_type.startsWith("codex.")) return;
    if (event.payload.agent_profile_id !== props.agentId) return;
    let active = true;
    Promise.all([
      api<{ trigger: CodexTriggerView | null }>(endpoint, {}, props.token),
      api<{ sessions: CodexSession[] }>(sessionsEndpoint, {}, props.token),
    ])
      .then(([{ trigger }, { sessions }]) => { if (active) { setTrigger(trigger); setSessions(sessions); } })
      .catch(() => undefined);
    return () => { active = false; };
  }, [endpoint, sessionsEndpoint, props.agentId, props.realtimeEvent, props.token]);

  useEffect(() => {
    if (!loading && !selectedProfileId && props.profiles.length) {
      const fallback = props.profiles.find((item) => item.profile.is_default) ?? props.profiles[0];
      setSelectedProfileId(fallback.profile.id);
    }
  }, [loading, props.profiles, selectedProfileId]);

  async function saveTrigger() {
    if (!selectedProfileId) {
      props.onError(new Error("请先创建并选择一个运行配置"));
      return;
    }
    setBusy(true);
    try {
      const response = await api<{ trigger: CodexTriggerView }>(
        endpoint,
        {
          method: "PUT",
          body: JSON.stringify({ runner_profile_id: selectedProfileId }),
        },
        props.token,
      );
      applyTrigger(response.trigger);
      props.onNotice(`${props.agentName} 已选择「${selectedProfile?.name ?? "运行配置"}」`);
      await props.onAssigned();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function triggerAction(action: "pause" | "resume" | "run-now") {
    const wasRunningOrQueued = Boolean(runningRun || trigger?.config.lease_owner);
    setBusy(true);
    try {
      const response = await api<{ trigger: CodexTriggerView }>(
        `${endpoint}/${action}`,
        { method: "POST" },
        props.token,
      );
      applyTrigger(response.trigger);
      props.onNotice(action === "run-now"
        ? wasRunningOrQueued ? "当前轮次结束后会再次唤醒，不会并发重复启动" : "已请求立即唤醒"
        : action === "pause" ? "定时触发已暂停" : "定时触发已恢复");
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="codex-trigger-card">
      <div className="codex-trigger-heading">
        <div>
          <span className="eyebrow">LOCAL CODEX TRIGGER</span>
          <h4>Agent 运行配置</h4>
          <p>Human 私聊会即时唤醒当前 Agent，群消息会即时唤醒群内 Agent；定时周期只作为兜底。</p>
        </div>
        {trigger && operationalStatus ? <span className={`status-badge ${operationalStatus}`}><span className="status-dot" />{codexOperationalStatusLabel(operationalStatus)}</span> : <span className="git-config-state">未配置</span>}
      </div>
      {trigger ? (
        <div className="codex-session-strip">
          <span><small>控制会话</small><strong>{sessions.some((session) => session.session_kind === "control" && session.status === "active") ? "已建立" : "首次有效唤醒时创建"}</strong></span>
          <span><small>项目工作会话</small><strong>{sessions.filter((session) => session.session_kind === "project" && session.status === "active").length} 个</strong></span>
          <span><small>当前状态</small><strong>{runningRun ? `已运行 ${formatElapsed(runningRun.started_at)}` : trigger.config.lease_owner ? "已领取，等待本地 Codex 启动" : trigger.config.manual_run_requested_at ? "手动唤醒已排队" : trigger.config.wake_requested_at ? "消息唤醒已排队" : codexTriggerStatusLabel(trigger.config.status)}</strong></span>
          <span><small>下次兜底检查</small><strong>{formatTime(trigger.config.next_run_at)}</strong></span>
          <span><small>最近成功</small><strong>{trigger.config.last_success_at ? formatTime(trigger.config.last_success_at) : "尚未成功运行"}</strong></span>
        </div>
      ) : null}
      {sessions.length ? (
        <div className="codex-work-session-list">
          <div className="codex-run-list-heading"><strong>工作会话目录</strong><small>控制会话负责判断；项目会话负责执行</small></div>
          {sessions.slice(0, 12).map((session) => {
            const projectName = session.project_id
              ? props.projects.find((project) => project.id === session.project_id)?.name ?? `项目 ${session.project_id.slice(0, 8)}`
              : "Relay 控制会话";
            const sessionRunning = trigger?.recent_runs.some((run) => run.status === "running" && run.project_id === session.project_id) ?? false;
            return (
              <div className="codex-work-session-row" key={session.id}>
                <StatusBadge value={sessionRunning ? "running" : session.status} />
                <div><strong>{projectName}</strong><small>{sessionRunning ? "当前 Trigger 正在使用这个会话" : codexSessionTurnLabel(session)}</small></div>
                <p>{session.summary_short || "尚未生成最近工作总结"}</p>
                <time>{formatTime(session.last_used_at)}</time>
              </div>
            );
          })}
        </div>
      ) : null}
      {runningRun ? (
        <div className="codex-live-activity">
          <div className="codex-live-activity-head">
            <div><span className="live-pulse" /><strong>实时执行动态</strong></div>
            <small>{runningRun.last_activity_at ? `最近活动 ${formatTime(runningRun.last_activity_at)}` : "正在等待 Codex 返回活动"}</small>
          </div>
          <div className="codex-current-activity">
            <span>{codexActivityPhaseLabel(runningRun.activity_phase)}</span>
            <strong>{runningRun.activity_summary ?? "正在启动本地 Codex 并连接固定会话"}</strong>
          </div>
          {runningRun.activity_log.length ? (
            <div className="codex-activity-log">
              {runningRun.activity_log.slice(-8).reverse().map((activity, index) => (
                <div key={`${activity.at}-${index}`}><time>{formatTime(activity.at)}</time><span>{codexActivityPhaseLabel(activity.phase)}</span><p>{activity.summary}</p></div>
              ))}
            </div>
          ) : null}
        </div>
      ) : null}
      {loading ? <small>正在读取 Trigger 配置…</small> : (
        <>
          <div className="agent-profile-picker">
            <Field label="选择运行配置">
              <select value={selectedProfileId} onChange={(event) => setSelectedProfileId(event.target.value)} disabled={!props.profiles.length}>
                {!props.profiles.length ? <option value="">请先创建运行配置</option> : null}
                {props.profiles.map((item) => <option key={item.profile.id} value={item.profile.id}>{item.profile.name}{item.profile.is_default ? "（默认）" : ""}</option>)}
              </select>
            </Field>
            {selectedProfile ? (
              <div className="selected-profile-summary">
                <span><small>模型</small><strong>{selectedProfile.model || "Codex 默认模型"}</strong></span>
                <span><small>思考等级</small><strong>{codexReasoningEffortLabel(selectedProfile.reasoning_effort, language)}</strong></span>
                <span><small>兜底检查</small><strong>{formatInterval(selectedProfile.interval_seconds)}</strong></span>
                <span><small>Sandbox</small><strong>{selectedProfile.sandbox_mode === "inherit" ? "继承公司" : selectedProfile.sandbox_mode === "workspace_write" ? "可写工作区" : "只读"}</strong></span>
                <span><small>审批</small><strong>{selectedProfile.approval_policy === "inherit" ? "继承公司" : selectedProfile.approval_policy === "on-request" ? "Human 审批" : "无需审批"}</strong></span>
                <span><small>运行上限</small><strong>{formatRunSeconds(selectedProfile.max_run_seconds, language)}</strong></span>
              </div>
            ) : null}
          </div>
          <div className="codex-trigger-actions">
            {trigger?.config.status === "active" ? <button className="button small" onClick={() => void triggerAction("pause")} disabled={busy}>暂停</button> : null}
            {trigger && trigger.config.status !== "active" ? <button className="button small" onClick={() => void triggerAction("resume")} disabled={busy || !props.active}>恢复</button> : null}
            {trigger ? <button className="button small" onClick={() => void triggerAction("run-now")} disabled={busy || trigger.config.status !== "active" || !props.active}>{runningRun || trigger.config.lease_owner ? "本轮后再唤醒" : trigger.config.manual_run_requested_at ? "已排队，再次请求" : "立即唤醒"}</button> : null}
            <button className="button primary small" onClick={() => void saveTrigger()} disabled={busy || !props.active || !selectedProfileId}>{busy ? "处理中…" : trigger ? "保存选择" : "启用这个配置"}</button>
          </div>
          {trigger?.config.last_error && !runningRun && !trigger.config.lease_owner ? <div className="inline-error">最近错误：{trigger.config.last_error}</div> : null}
          {trigger?.recent_runs.length ? (
            <div className="codex-run-list">
              <div className="codex-run-list-heading"><strong>最近执行记录</strong><small>以下是历史记录；当前状态以上方状态栏为准</small></div>
              {trigger.recent_runs.slice(0, 5).map((run) => (
                <div key={run.id} className="codex-run-row">
                  <StatusBadge value={run.status} />
                  <span>{codexTriggerTypeLabel(run.trigger_type)}</span>
                  <small>{formatTime(run.started_at)}</small>
                  <code title={run.codex_thread_id ?? undefined}>{run.codex_thread_id ? run.codex_thread_id.slice(0, 18) : "no-thread"}</code>
                  <p>{codexRunDisplayMessage(run)}</p>
                </div>
              ))}
            </div>
          ) : null}
        </>
      )}
    </div>
  );
}
