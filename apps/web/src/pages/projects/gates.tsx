import { useEffect, useMemo, useState } from "react";
import { api } from "../../api/client";
import { Field, Icon } from "../../components/ui";
import type { CompanyProject } from "../../types/platform";

type ProjectGate = {
  id: string;
  project_id: string;
  gate_key: string;
  gate_type: string;
  title: string;
  status: "pending" | "evaluating" | "passed" | "failed" | "waived" | "cancelled";
  related_task_id: string | null;
  required_evidence: string[];
  decision_summary: string;
  updated_at: string;
};

type GateRequirement = {
  task_id: string;
  gate_id: string;
  required_status: "passed" | "waived";
};

const gateTypes = [
  ["design", "设计"],
  ["technical", "技术方案"],
  ["qa", "质量验收"],
  ["pm", "项目决策"],
  ["environment", "运行环境"],
  ["approval", "人工审批"],
  ["release", "发布"],
  ["custom", "其他"],
] as const;

const statusLabels: Record<ProjectGate["status"], string> = {
  pending: "待评估",
  evaluating: "评估中",
  passed: "已通过",
  failed: "未通过",
  waived: "已豁免",
  cancelled: "已取消",
};

export function ProjectGatesCard(props: {
  companyId: string;
  project: CompanyProject;
  token: string;
  canManage: boolean;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [gates, setGates] = useState<ProjectGate[]>([]);
  const [requirements, setRequirements] = useState<GateRequirement[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [showCreate, setShowCreate] = useState(false);
  const [title, setTitle] = useState("");
  const [gateType, setGateType] = useState("design");
  const [taskId, setTaskId] = useState("");
  const [evidence, setEvidence] = useState("");
  const [decisionDrafts, setDecisionDrafts] = useState<Record<string, string>>({});

  const tasksById = useMemo(
    () => new Map(props.project.tasks.map((task) => [task.id, task])),
    [props.project.tasks],
  );

  async function load() {
    setLoading(true);
    try {
      const response = await api<{ gates: ProjectGate[]; requirements: GateRequirement[] }>(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/gates`,
        {},
        props.token,
      );
      setGates(response.gates);
      setRequirements(response.requirements);
    } catch (error) {
      props.onError(error);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load();
  }, [props.project.project.id]);

  async function createGate(event: React.FormEvent) {
    event.preventDefault();
    if (!title.trim()) return;
    setBusy(true);
    try {
      const created = await api<{ gate: ProjectGate }>(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/gates`,
        {
          method: "POST",
          body: JSON.stringify({
            gate_key: `gate-${Date.now()}`,
            gate_type: gateType,
            title: title.trim(),
            related_task_id: taskId || null,
            required_evidence: evidence.split("\n").map((item) => item.trim()).filter(Boolean),
          }),
        },
        props.token,
      );
      if (taskId) {
        await api(
          `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/tasks/${taskId}/gates/${created.gate.id}`,
          { method: "PUT", body: JSON.stringify({ required_status: "passed" }) },
          props.token,
        );
      }
      setTitle("");
      setTaskId("");
      setEvidence("");
      setShowCreate(false);
      props.onNotice("项目 Gate 已创建，相关任务将等待 Gate 通过后进入 Ready");
      await load();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function decide(gate: ProjectGate, status: "passed" | "failed" | "waived") {
    const summary = decisionDrafts[gate.id]?.trim() ?? "";
    if (!summary) {
      props.onError(new Error("请先填写本次 Gate 决策依据"));
      return;
    }
    setBusy(true);
    try {
      await api(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/gates/${gate.id}`,
        { method: "PUT", body: JSON.stringify({ status, decision_summary: summary }) },
        props.token,
      );
      setDecisionDrafts((current) => ({ ...current, [gate.id]: "" }));
      props.onNotice(status === "passed" ? "Gate 已通过，满足全部条件的任务已自动 Ready" : status === "waived" ? "Gate 已豁免" : "Gate 未通过，关联任务继续等待");
      await load();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function openDiscussion(gate: ProjectGate) {
    try {
      const response = await api<{ conversation: { preview: { title: string } } }>(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/discussion-threads`,
        { method: "POST", body: JSON.stringify({ scope_type: "gate", subject_id: gate.id }) },
        props.token,
      );
      await props.onChanged();
      props.onNotice(`${response.conversation.preview.title} 已建立，可在聊天列表中继续讨论`);
    } catch (error) { props.onError(error); }
  }

  return (
    <div className="project-gates-layout">
      <div className="project-tab-heading">
        <div><span className="eyebrow">PROJECT GATES</span><h3>项目门禁</h3></div>
        <div className="project-tab-actions">
          <span className="count-badge">{gates.length}</span>
          {props.canManage ? <button className="button small primary" type="button" onClick={() => setShowCreate((value) => !value)}><Icon name="plus" /> 新建 Gate</button> : null}
        </div>
      </div>
      {showCreate ? (
        <form className="project-gate-create" onSubmit={createGate}>
          <div className="form-grid">
            <Field label="Gate 名称"><input value={title} onChange={(event) => setTitle(event.target.value)} placeholder="例如：交互设计评审" required /></Field>
            <Field label="类型"><select value={gateType} onChange={(event) => setGateType(event.target.value)}>{gateTypes.map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></Field>
            <Field label="约束任务"><select value={taskId} onChange={(event) => setTaskId(event.target.value)}><option value="">暂不绑定任务</option>{props.project.tasks.map((task) => <option key={task.id} value={task.id}>{task.title}</option>)}</select></Field>
          </div>
          <Field label="所需证据"><textarea value={evidence} onChange={(event) => setEvidence(event.target.value)} placeholder={"每行一项，例如：\n可编辑设计源文件\nSVG 审阅稿"} /></Field>
          <div className="dialog-actions"><button className="button" type="button" onClick={() => setShowCreate(false)}>取消</button><button className="button primary" disabled={busy}>{busy ? "创建中…" : "创建 Gate"}</button></div>
        </form>
      ) : null}
      {loading ? <div className="empty-inline"><span className="loading-dot" />正在读取项目 Gate…</div> : null}
      {!loading && !gates.length ? <div className="empty-inline compact-empty"><Icon name="shield" /><h3>还没有项目 Gate</h3></div> : null}
      <div className="project-gate-list">
        {gates.map((gate) => {
          const boundRequirements = requirements.filter((requirement) => requirement.gate_id === gate.id);
          const relatedTasks = Array.from(new Set([
            ...(gate.related_task_id ? [gate.related_task_id] : []),
            ...boundRequirements.map((requirement) => requirement.task_id),
          ])).map((id) => tasksById.get(id)).filter(Boolean);
          return (
            <article className={`project-gate-card ${gate.status}`} key={gate.id}>
              <header><span className={`status-badge ${gate.status}`}><span className="status-dot" />{statusLabels[gate.status]}</span><strong>{gate.title}</strong><small>{gateTypes.find(([type]) => type === gate.gate_type)?.[1] ?? gate.gate_type}</small></header>
              {relatedTasks.length ? <div className="project-gate-tasks">{relatedTasks.map((task) => <span key={task!.id}><Icon name="tasks" />{task!.title}</span>)}</div> : null}
              {gate.required_evidence.length ? <ul>{gate.required_evidence.map((item) => <li key={item}>{item}</li>)}</ul> : null}
              {gate.decision_summary ? <p className="project-gate-decision">{gate.decision_summary}</p> : null}
              <div className="project-gate-actions"><button className="button small" type="button" onClick={() => void openDiscussion(gate)}><Icon name="message" /> Gate 讨论</button>{props.canManage && !["cancelled"].includes(gate.status) ? <><input value={decisionDrafts[gate.id] ?? ""} onChange={(event) => setDecisionDrafts((current) => ({ ...current, [gate.id]: event.target.value }))} placeholder="填写评审结果或决策依据" /><button className="button small primary" disabled={busy} type="button" onClick={() => void decide(gate, "passed")}>通过</button><button className="button small" disabled={busy} type="button" onClick={() => void decide(gate, "waived")}>豁免</button><button className="button small danger-outline" disabled={busy} type="button" onClick={() => void decide(gate, "failed")}>不通过</button></> : null}</div>
            </article>
          );
        })}
      </div>
    </div>
  );
}
