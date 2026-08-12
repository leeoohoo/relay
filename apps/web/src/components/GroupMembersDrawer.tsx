import { useEffect, useMemo, useState } from "react";
import { api } from "../api/client";
import type { CompanyRealtimeEvent } from "../api/types";
import type { CodexTriggerRun, CodexTriggerView, CompanyAgent, CompanyProject, CompanyProjectTask } from "../types/platform";
import { Pagination, usePagination } from "./Pagination";
import { Icon } from "./ui";

type RuntimeState = {
  loading: boolean;
  trigger: CodexTriggerView | null;
  error: string | null;
};

export function GroupMembersDrawer(props: {
  companyId: string;
  conversationTitle: string;
  humanName: string;
  agents: CompanyAgent[];
  project: CompanyProject | null;
  mode: "group" | "direct";
  token: string;
  realtimeEvent: CompanyRealtimeEvent | null;
  onTaskOpen?: (taskId: string) => void;
  onClose: () => void;
}) {
  const agentIdsKey = props.agents.map((agent) => agent.agent_profile.id).join(",");
  const [runtimeByAgent, setRuntimeByAgent] = useState<Record<string, RuntimeState>>({});
  const pagination = usePagination(props.agents, 8, agentIdsKey);

  useEffect(() => {
    let active = true;
    setRuntimeByAgent(Object.fromEntries(props.agents.map((agent) => [agent.agent_profile.id, emptyRuntime(true)])));
    Promise.all(props.agents.map(async (agent) => {
      const agentId = agent.agent_profile.id;
      if (agent.membership.employment_status !== "active") return [agentId, emptyRuntime(false)] as const;
      try {
        const response = await api<{ trigger: CodexTriggerView | null }>(
          `/api/v1/companies/${props.companyId}/agents/${agentId}/codex-trigger`,
          {},
          props.token,
        );
        return [agentId, { loading: false, trigger: response.trigger, error: null }] as const;
      } catch (error) {
        return [agentId, { loading: false, trigger: null, error: errorMessage(error) }] as const;
      }
    })).then((entries) => {
      if (active) setRuntimeByAgent(Object.fromEntries(entries));
    });
    return () => { active = false; };
  }, [agentIdsKey, props.companyId, props.token]);

  useEffect(() => {
    const event = props.realtimeEvent;
    if (!event?.event_type.startsWith("codex.")) return;
    const agentId = typeof event.payload.agent_profile_id === "string" ? event.payload.agent_profile_id : null;
    if (!agentId || !props.agents.some((agent) => agent.agent_profile.id === agentId)) return;
    let active = true;
    api<{ trigger: CodexTriggerView | null }>(
      `/api/v1/companies/${props.companyId}/agents/${agentId}/codex-trigger`,
      {},
      props.token,
    ).then(({ trigger }) => {
      if (!active) return;
      setRuntimeByAgent((current) => ({ ...current, [agentId]: { loading: false, trigger, error: null } }));
    }).catch((error) => {
      if (!active) return;
      setRuntimeByAgent((current) => ({ ...current, [agentId]: { loading: false, trigger: null, error: errorMessage(error) } }));
    });
    return () => { active = false; };
  }, [agentIdsKey, props.companyId, props.realtimeEvent, props.token]);

  return (
    <aside className="group-members-drawer" aria-label={props.mode === "group" ? "群成员与运行情况" : "私聊成员与运行情况"}>
      <header className="group-members-drawer-head">
        <div>
          <span className="drawer-eyebrow">{props.mode === "group" ? "GROUP RUNTIME" : "DIRECT RUNTIME"}</span>
          <strong>{props.mode === "group" ? "群成员" : "对话成员"}</strong>
          <small>{props.conversationTitle} · {props.agents.length} 个 Agent</small>
        </div>
        <button type="button" className="icon-button" aria-label={props.mode === "group" ? "关闭群成员" : "关闭运行详情"} onClick={props.onClose}><Icon name="close" /></button>
      </header>

      <div className="group-members-drawer-body">
        <div className="group-human-member">
          <span className="agent-avatar small">{props.humanName.slice(0, 1)}</span>
          <div><strong>{props.humanName}</strong><small>Human · 当前登录用户</small></div>
          <span className="member-kind human">在线</span>
        </div>

        <div className="group-runtime-list">
          {pagination.pageItems.map((agent) => (
            <AgentRuntimeDetails
              key={agent.agent_profile.id}
              agent={agent}
              runtime={runtimeByAgent[agent.agent_profile.id] ?? emptyRuntime(true)}
              tasks={assignedTasks(props.project, agent.agent_profile.id)}
              onTaskOpen={props.onTaskOpen}
            />
          ))}
          {!props.agents.length ? <div className="conversation-member-empty">当前会话没有可展示的 Agent 成员。</div> : null}
        </div>
        <Pagination {...pagination} onPageChange={pagination.setPage} compact />
      </div>
    </aside>
  );
}

function AgentRuntimeDetails(props: { agent: CompanyAgent; runtime: RuntimeState; tasks: CompanyProjectTask[]; onTaskOpen?: (taskId: string) => void }) {
  const operationalStatus = runtimeStatus(props.agent, props.runtime, props.tasks);
  const runningRun = props.runtime.trigger?.recent_runs.find((run) => run.status === "running") ?? null;
  const latestRun = runningRun ?? props.runtime.trigger?.recent_runs[0] ?? null;
  const currentTask = props.tasks.find((task) => task.status === "in_progress") ?? props.tasks[0] ?? null;
  const currentIntent = props.runtime.trigger?.active_intents.find((intent) => intent.status === "running")
    ?? props.runtime.trigger?.active_intents[0]
    ?? null;
  const projectSession = props.runtime.trigger?.recent_sessions.find((session) => session.session_kind === "project") ?? null;
  const summary = props.agent.membership.employment_status === "active"
    ? runtimeSummary(props.runtime, latestRun, currentTask, operationalStatus)
    : "该 Agent 已暂停工作";

  return (
    <details className={`group-runtime-member ${operationalStatus}`}>
      <summary>
        <span className="agent-avatar small">{props.agent.agent_profile.display_name.slice(0, 1)}</span>
        <span className="group-member-identity">
          <strong>{props.agent.agent_profile.display_name}</strong>
          <small>{props.agent.membership.job_title || "Agent"}</small>
          <em>{summary}</em>
        </span>
        <span className={`runtime-status ${operationalStatus}`}><i />{runtimeStatusLabel(operationalStatus)}</span>
        <Icon name="chevron-down" />
      </summary>
      <div className="group-runtime-detail">
        <div className="runtime-facts">
          <span><small>连接</small><strong>{connectionLabel(props.agent.connection.status)}</strong></span>
          <span><small>Trigger</small><strong>{triggerStatusLabel(props.runtime.trigger)}</strong></span>
          {latestRun ? <span><small>{latestRun.status === "running" ? "已运行" : "最近执行"}</small><strong>{latestRun.status === "running" ? formatElapsed(latestRun.started_at) : formatTime(latestRun.started_at)}</strong></span> : null}
        </div>

        {props.tasks.length ? (
          <section className="runtime-task-section">
            <h4>项目任务</h4>
            {props.tasks.slice(0, 3).map((task) => <button type="button" className="runtime-task" key={task.id} onClick={() => props.onTaskOpen?.(task.id)}><span className={`task-state ${task.status}`}>{taskStatusLabel(task.status)}</span><strong>{task.title}</strong><Icon name="chevron-right" /></button>)}
          </section>
        ) : null}

        {currentIntent ? <IntentProgress intent={currentIntent} tasks={props.tasks} session={projectSession} /> : null}

        {props.runtime.loading ? <div className="runtime-empty"><span className="loader" /> 正在读取运行情况…</div> : null}
        {props.runtime.error ? <div className="runtime-error"><Icon name="alert" /> <span><strong>运行信息读取失败</strong><small>{props.runtime.error}</small></span></div> : null}
        {!props.runtime.loading && !props.runtime.error && !props.runtime.trigger ? <div className="runtime-empty">这个 Agent 尚未启用 Codex Trigger。</div> : null}
        {latestRun ? <RunProcess run={latestRun} /> : null}

        {props.runtime.trigger && props.runtime.trigger.recent_runs.length > (latestRun ? 1 : 0) ? (
          <section className="runtime-history-section">
            <h4>最近执行</h4>
            {props.runtime.trigger.recent_runs.filter((run) => run.id !== latestRun?.id).slice(0, 3).map((run) => (
              <div className="runtime-history-row" key={run.id}>
                <span className={`runtime-history-state ${run.status}`}>{runStatusLabel(run.status)}</span>
                <div><strong>{runDisplayMessage(run)}</strong><small>{formatTime(run.started_at)} · {triggerTypeLabel(run.trigger_type)}</small></div>
              </div>
            ))}
          </section>
        ) : null}
      </div>
    </details>
  );
}

function IntentProgress(props: {
  intent: CodexTriggerView["active_intents"][number];
  tasks: CompanyProjectTask[];
  session: CodexTriggerView["recent_sessions"][number] | null;
}) {
  const taskNames = props.intent.task_ids
    .map((taskId) => props.tasks.find((task) => task.id === taskId)?.title)
    .filter((title): title is string => Boolean(title));
  return (
    <section className="runtime-intent-section">
      <div className="runtime-process-head">
        <h4>当前项目工作</h4>
        <span className={`runtime-history-state ${props.intent.status}`}>{props.intent.status === "running" ? "工作中" : "待接续"}</span>
      </div>
      <strong>{props.intent.objective}</strong>
      {taskNames.length ? <small>关联任务 · {taskNames.join("、")}</small> : null}
      {props.session?.summary_short ? <p>{summaryPreview(props.session.summary_short)}</p> : <p>项目工作会话已建立，等待产生第一条成果总结。</p>}
    </section>
  );
}

function RunProcess({ run }: { run: CodexTriggerRun }) {
  const activities = run.activity_log.slice(-12);
  return (
    <section className="runtime-process-section">
      <div className="runtime-process-head">
        <h4>{run.status === "running" ? "当前执行过程" : "最近一次执行过程"}</h4>
        <span className={`runtime-history-state ${run.status}`}>{runStatusLabel(run.status)}</span>
      </div>
      <div className="runtime-current-step">
        <span>{activityPhaseLabel(run.activity_phase)}</span>
        <strong>{run.activity_summary ?? runDisplayMessage(run)}</strong>
      </div>
      {activities.length ? <div className="runtime-activity-timeline">{activities.map((activity, index) => (
        <div key={`${activity.at}-${index}`}>
          <i />
          <time>{formatTime(activity.at)}</time>
          <span>{activityPhaseLabel(activity.phase)}</span>
          <p>{activity.summary}</p>
        </div>
      ))}</div> : <div className="runtime-empty compact">还没有可展示的过程记录。</div>}
      {run.codex_thread_id ? <code className="runtime-thread-id" title={run.codex_thread_id}>Thread · {run.codex_thread_id}</code> : null}
      {run.error_message ? <div className="runtime-run-error">{run.error_message}</div> : null}
    </section>
  );
}

function assignedTasks(project: CompanyProject | null, agentId: string) {
  if (!project) return [];
  const priority = { in_progress: 0, blocked: 1, todo: 2, failed: 3, done: 4, cancelled: 5 } as Record<CompanyProjectTask["status"], number>;
  return project.tasks
    .filter((task) => task.assignee_agent_id === agentId)
    .sort((left, right) => priority[left.status] - priority[right.status] || right.updated_at.localeCompare(left.updated_at));
}

function emptyRuntime(loading: boolean): RuntimeState {
  return { loading, trigger: null, error: null };
}

function runtimeStatus(agent: CompanyAgent, runtime: RuntimeState, tasks: CompanyProjectTask[]) {
  if (agent.membership.employment_status !== "active") return "paused";
  if (runtime.loading) return "loading";
  if (runtime.error) return "error";
  const trigger = runtime.trigger;
  if (!trigger) return agent.connection.status === "connected" ? "idle" : "offline";
  if (trigger.config.status !== "active") return trigger.config.status;
  if (trigger.active_intents?.some((intent) => intent.status === "running")) return "running";
  if (trigger.recent_runs.some((run) => run.status === "running")) return "running";
  if (trigger.active_intents?.length) return "continuing";
  if (trigger.config.lease_owner || trigger.config.manual_run_requested_at || trigger.config.wake_requested_at) return "queued";
  if (tasks.some((task) => task.status === "in_progress")) return "continuing";
  return "idle";
}

function runtimeSummary(runtime: RuntimeState, run: CodexTriggerRun | null, task: CompanyProjectTask | null, operationalStatus: string) {
  if (runtime.loading) return "正在同步运行数据";
  if (runtime.error) return "无法读取运行详情";
  if (operationalStatus === "queued") return "任务已进入执行队列";
  if (operationalStatus === "continuing" && task) return `任务尚未完成，等待接续或外部条件 · ${task.title}`;
  if (operationalStatus === "paused") return "Trigger 已暂停";
  if (operationalStatus === "error") return runtime.trigger?.config.last_error ?? "Trigger 运行异常";
  if (run?.status === "running") return run.activity_summary ?? "Codex 正在执行任务";
  if (task) return `${taskStatusLabel(task.status)} · ${task.title}`;
  if (run) return runDisplayMessage(run);
  return runtime.trigger ? "当前空闲，等待新任务" : "尚未启用运行配置";
}

function runtimeStatusLabel(value: string) {
  return ({ loading: "同步中", running: "运行中", queued: "排队中", continuing: "待继续", idle: "空闲", paused: "已暂停", error: "异常", offline: "未连接" } as Record<string, string>)[value] ?? value;
}

function runStatusLabel(value: string) {
  return ({ running: "运行中", succeeded: "已完成", failed: "失败", timed_out: "超时", cancelled: "已取消", lease_lost: "异常中断", restarted: "已接续" } as Record<string, string>)[value] ?? value;
}

function connectionLabel(value: string) {
  return ({ connected: "已连接", not_connected: "待连接", awaiting_activation: "待激活", suspended: "已暂停", terminated: "已裁撤", key_revoked: "Key 已撤销", key_expired: "Key 已过期", no_key: "无 Key" } as Record<string, string>)[value] ?? value;
}

function triggerStatusLabel(trigger: CodexTriggerView | null) {
  if (!trigger) return "未启用";
  return ({ active: "已启用", paused: "已暂停", error: "异常" } as Record<string, string>)[trigger.config.status] ?? trigger.config.status;
}

function taskStatusLabel(value: CompanyProjectTask["status"]) {
  return ({ todo: "待处理", in_progress: "进行中", blocked: "阻塞", done: "已完成", failed: "失败", cancelled: "已取消" } as Record<CompanyProjectTask["status"], string>)[value];
}

function triggerTypeLabel(value: string) {
  return ({ scheduled: "定时", manual: "手动", run_now: "手动", message: "消息", task: "任务", asset_refresh: "资产维护" } as Record<string, string>)[value] ?? value;
}

function activityPhaseLabel(value: string) {
  return ({ preparing: "准备工作区", starting: "启动 Codex", session: "连接会话", thinking: "分析", planning: "规划", tool: "调用工具", command: "执行命令", files: "修改文件", searching: "搜索", reporting: "进度说明", dispatching: "进入项目工作", retrying: "准备重试", continuing: "接续工作", finishing: "收尾", waiting_approval: "等待审批", approval_delivery_failed: "审批投递失败", approval_rejected: "审批未通过", running: "执行中", completed: "已完成", failed: "失败", timed_out: "超时", cancelled: "已取消", lease_lost: "进程中断" } as Record<string, string>)[value] ?? value;
}

function runDisplayMessage(run: CodexTriggerRun) {
  if (run.final_message_summary) return run.final_message_summary;
  if (run.error_message) return run.error_message;
  return ({ running: "本轮仍在执行", succeeded: "Codex 已完成本轮", timed_out: "本轮运行超时", cancelled: "本轮已取消", lease_lost: "Trigger 异常退出，工作等待恢复", restarted: "Trigger 服务重启，工作已由后续运行接续", failed: "本轮运行失败" } as Record<string, string>)[run.status] ?? "等待运行结果";
}

function summaryPreview(value: string) {
  const plain = value.replace(/\[[^\]]+\]\([^\)]+\)/g, "").replace(/[#*_`>-]/g, " ").replace(/\s+/g, " ").trim();
  return plain.length > 220 ? `${plain.slice(0, 220)}…` : plain;
}

function formatTime(value: string) {
  return new Date(value).toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" });
}

function formatElapsed(value: string) {
  const seconds = Math.max(0, Math.floor((Date.now() - new Date(value).getTime()) / 1_000));
  if (seconds < 60) return `${seconds} 秒`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} 分钟`;
  return `${Math.floor(minutes / 60)} 小时 ${minutes % 60} 分`;
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : "未知错误";
}
