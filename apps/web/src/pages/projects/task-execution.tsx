import { useEffect, useMemo, useState } from "react";
import { api } from "../../api/client";
import { Icon } from "../../components/ui";
import type { CompanyProject, CompanyProjectTask, ProjectTaskExecution } from "../../types/platform";
import { formatTime } from "../app/shared";

export function TaskExecutionPanel(props: { companyId: string; project: CompanyProject; task: CompanyProjectTask; token: string; onChanged: () => Promise<void>; onError: (error: unknown) => void; onNotice: (notice: string) => void }) {
  const [execution, setExecution] = useState<ProjectTaskExecution | null>(null);
  const [busy, setBusy] = useState(false);
  const [showBlocker, setShowBlocker] = useState(false);
  const [showEvidence, setShowEvidence] = useState(false);
  const [blockerSummary, setBlockerSummary] = useState("");
  const [resolutionCondition, setResolutionCondition] = useState("");
  const [evidenceTitle, setEvidenceTitle] = useState("");
  const [evidenceSummary, setEvidenceSummary] = useState("");
  const taskNames = useMemo(() => new Map(props.project.tasks.map((task) => [task.id, task.title])), [props.project.tasks]);

  async function load() {
    try {
      const response = await api<{ execution: ProjectTaskExecution }>(`/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/tasks/${props.task.id}/execution`, {}, props.token);
      setExecution(response.execution);
    } catch (error) { props.onError(error); }
  }
  useEffect(() => { void load(); }, [props.task.id]);

  async function openBlocker() {
    setBusy(true);
    try {
      await api(`/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/tasks/${props.task.id}/blockers`, { method: "POST", body: JSON.stringify({ blocker_type: "external", summary: blockerSummary, resolution_condition: resolutionCondition, owner_agent_id: props.task.assignee_agent_id, attempt_id: null }) }, props.token);
      setShowBlocker(false); setBlockerSummary(""); setResolutionCondition(""); props.onNotice("阻塞项已创建，任务在解决前保持等待"); await load();
    } catch (error) { props.onError(error); } finally { setBusy(false); }
  }

  async function resolveBlocker(blockerId: string) {
    setBusy(true);
    try {
      await api(`/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/tasks/${props.task.id}/blockers/${blockerId}`, { method: "PUT", body: JSON.stringify({ status: "resolved", resolution_summary: "Human 已确认阻塞条件解决" }) }, props.token);
      props.onNotice("阻塞项已解决，任务就绪状态已重新计算"); await load();
    } catch (error) { props.onError(error); } finally { setBusy(false); }
  }

  async function createEvidence() {
    setBusy(true);
    try {
      await api(`/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/tasks/${props.task.id}/evidence`, { method: "POST", body: JSON.stringify({ evidence_type: "report", title: evidenceTitle, summary: evidenceSummary, result: "informational", artifact_refs: [], metrics: {}, attempt_id: null, gate_id: null, environment_id: null, dedupe_key: null }) }, props.token);
      setShowEvidence(false); setEvidenceTitle(""); setEvidenceSummary(""); props.onNotice("证据已保存，可被 Attempt、Gate 和任务引用"); await load();
    } catch (error) { props.onError(error); } finally { setBusy(false); }
  }

  async function openDiscussion(scopeType: "task" | "blocker", subjectId: string) {
    try {
      const response = await api<{ conversation: { preview: { title: string } } }>(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/discussion-threads`,
        { method: "POST", body: JSON.stringify({ scope_type: scopeType, subject_id: subjectId }) },
        props.token,
      );
      await props.onChanged();
      props.onNotice(`${response.conversation.preview.title} 已建立，可在聊天列表中继续讨论`);
    } catch (error) { props.onError(error); }
  }

  if (!execution) return <div className="task-execution-loading"><span className="loading-dot" />正在读取执行记录…</div>;
  return <div className="task-execution-panel">
    <div className="task-execution-heading"><strong>执行记录</strong><span>{execution.attempts.length} 次执行 · {execution.blockers.filter((item) => item.status === "open").length} 个开放阻塞 · {execution.evidence.length} 条证据</span><div><button className="button small" type="button" onClick={() => void openDiscussion("task", props.task.id)}><Icon name="message" /> 任务讨论</button><button className="button small" type="button" onClick={() => setShowBlocker((value) => !value)}><Icon name="plus" /> 阻塞项</button><button className="button small" type="button" onClick={() => setShowEvidence((value) => !value)}><Icon name="plus" /> 证据</button></div></div>
    <div className={`task-readiness-summary ${execution.readiness.can_start ? "ready" : "waiting"}`}>
      <span className={`task-execution-status ${execution.readiness.can_start ? "passed" : "open"}`}>{execution.readiness.can_start ? "可执行" : "等待条件"}</span>
      <div><strong>{execution.readiness.can_start ? "门禁、环境、依赖与阻塞均已满足" : `还有 ${execution.readiness.waiting_reasons.length} 项条件未满足`}</strong><small>{execution.readiness.can_start ? "可开始或继续 Attempt" : "需先处理以下条件"}</small></div>
    </div>
    {showBlocker ? <div className="task-execution-inline-form"><input value={blockerSummary} onChange={(event) => setBlockerSummary(event.target.value)} placeholder="阻塞原因" required /><input value={resolutionCondition} onChange={(event) => setResolutionCondition(event.target.value)} placeholder="解除条件" required /><button className="button small primary" type="button" disabled={busy || !blockerSummary.trim() || !resolutionCondition.trim()} onClick={() => void openBlocker()}>创建</button></div> : null}
    {showEvidence ? <div className="task-execution-inline-form"><input value={evidenceTitle} onChange={(event) => setEvidenceTitle(event.target.value)} placeholder="证据标题" required /><input value={evidenceSummary} onChange={(event) => setEvidenceSummary(event.target.value)} placeholder="结论摘要" required /><button className="button small primary" type="button" disabled={busy || !evidenceTitle.trim() || !evidenceSummary.trim()} onClick={() => void createEvidence()}>保存</button></div> : null}
    <div className="task-execution-sections">
      <section><strong>任务就绪条件</strong>{execution.readiness.waiting_reasons.length ? execution.readiness.waiting_reasons.map((reason) => <article key={`${reason.kind}:${reason.related_id ?? reason.code}`}><span className="task-execution-status open">{readinessKindLabel(reason.kind)}</span><div><b>{reason.summary}</b><p>{execution.readiness.suggested_actions.find((action) => action.includes(readinessActionKeyword(reason.kind))) ?? "由对应责任人满足条件后，Relay 会自动重新判断任务。"}</p></div></article>) : <p className="task-execution-empty">当前没有未满足的门禁、环境、依赖或阻塞。</p>}{execution.readiness.gate_requirements.map((item) => <article key={`gate:${item.requirement.gate_id}`}><span className={`task-execution-status ${item.satisfied ? "passed" : "open"}`}>{item.satisfied ? "已满足" : "待通过"}</span><div><b>门禁 · {item.gate?.title ?? "记录缺失"}</b><p>当前 {item.gate?.status ?? "missing"} · 要求 {item.requirement.required_status}</p></div></article>)}{execution.readiness.environment_requirements.map((item) => <article key={`environment:${item.requirement.environment_id}`}><span className={`task-execution-status ${item.satisfied ? "passed" : "open"}`}>{item.satisfied ? "已满足" : "待就绪"}</span><div><b>环境 · {item.environment?.display_name ?? "记录缺失"}</b><p>当前 {item.environment?.status ?? "missing"} · Revision {item.environment?.observed_revision ?? "未观测"}{item.requirement.required_services.length ? ` · 服务 ${item.requirement.required_services.join("、")}` : ""}</p></div></article>)}</section>
      <section><strong>Attempt 时间线</strong>{execution.attempts.length ? execution.attempts.map((attempt) => <article key={attempt.id}><span className={`task-execution-status ${attempt.status}`}>{attempt.status}</span><div><b>#{attempt.attempt_number} · {attempt.attempt_type}</b><p>{attempt.objective}</p>{attempt.result_summary ? <small>{attempt.result_summary}</small> : null}</div><time>{formatTime(attempt.created_at)}</time></article>) : <p className="task-execution-empty">Agent 尚未开始结构化执行。</p>}</section>
      <section><strong>阻塞与关系</strong>{execution.blockers.map((blocker) => <article key={blocker.id}><span className={`task-execution-status ${blocker.status}`}>{blocker.status}</span><div><b>{blocker.blocker_type}</b><p>{blocker.summary}</p><small>解除条件：{blocker.resolution_condition}</small></div><button className="button small" type="button" onClick={() => void openDiscussion("blocker", blocker.id)}>讨论</button>{blocker.status === "open" ? <button className="button small" type="button" disabled={busy} onClick={() => void resolveBlocker(blocker.id)}>解决</button> : <time>{formatTime(blocker.resolved_at ?? blocker.created_at)}</time>}</article>)}{execution.relations.map((relation) => <article key={relation.id}><Icon name="tasks" /><div><b>{relation.relation_type}</b><p>{taskNames.get(relation.source_task_id)} → {taskNames.get(relation.target_task_id)}</p></div><time>{formatTime(relation.created_at)}</time></article>)}{!execution.blockers.length && !execution.relations.length ? <p className="task-execution-empty">没有开放阻塞或任务关系。</p> : null}</section>
      <section><strong>Evidence</strong>{execution.evidence.length ? execution.evidence.map((item) => <article key={item.id}><span className={`task-execution-status ${item.result}`}>{item.result}</span><div><b>{item.title}</b><p>{item.summary}</p><small>{item.evidence_type}</small></div><time>{formatTime(item.created_at)}</time></article>) : <p className="task-execution-empty">还没有结构化证据。</p>}</section>
    </div>
  </div>;
}

function readinessKindLabel(kind: ProjectTaskExecution["readiness"]["waiting_reasons"][number]["kind"]) {
  return { dependency: "前置", gate: "门禁", environment: "环境", blocker: "阻塞" }[kind];
}

function readinessActionKeyword(kind: ProjectTaskExecution["readiness"]["waiting_reasons"][number]["kind"]) {
  return { dependency: "前置任务", gate: "company.gate", environment: "company.environment", blocker: "blocker_resolve" }[kind];
}
