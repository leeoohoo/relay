import { useEffect, useMemo, useState } from "react";
import { api } from "../../api/client";
import { Field, Icon } from "../../components/ui";
import type { CompanyProject } from "../../types/platform";

type ProjectEnvironment = {
  id: string;
  environment_key: string;
  display_name: string;
  status: "unknown" | "provisioning" | "ready" | "degraded" | "offline";
  desired_revision: string | null;
  observed_revision: string | null;
  health_summary: Record<string, unknown>;
  last_observed_at: string | null;
};

type EnvironmentService = {
  id: string;
  environment_id: string;
  service_key: string;
  observed_revision: string | null;
  image_digest: string | null;
  health_status: "unknown" | "healthy" | "unhealthy";
};

type EnvironmentRequirement = {
  task_id: string;
  environment_id: string;
  required_revision: string | null;
  required_services: string[];
  require_healthy: boolean;
};

const statusLabels: Record<ProjectEnvironment["status"], string> = {
  unknown: "未观测",
  provisioning: "准备中",
  ready: "就绪",
  degraded: "异常",
  offline: "离线",
};

export function ProjectEnvironmentsCard(props: {
  companyId: string;
  project: CompanyProject;
  token: string;
  canManage: boolean;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [environments, setEnvironments] = useState<ProjectEnvironment[]>([]);
  const [services, setServices] = useState<EnvironmentService[]>([]);
  const [requirements, setRequirements] = useState<EnvironmentRequirement[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [showCreate, setShowCreate] = useState(false);
  const [key, setKey] = useState("staging");
  const [name, setName] = useState("Staging");
  const [desiredRevision, setDesiredRevision] = useState("");
  const [taskId, setTaskId] = useState("");
  const [selectedEnvironmentId, setSelectedEnvironmentId] = useState("");
  const [requiredRevision, setRequiredRevision] = useState("");
  const [observations, setObservations] = useState<Record<string, { status: ProjectEnvironment["status"]; revision: string; services: string }>>({});

  const tasksById = useMemo(() => new Map(props.project.tasks.map((task) => [task.id, task])), [props.project.tasks]);

  async function load() {
    setLoading(true);
    try {
      const response = await api<{ environments: ProjectEnvironment[]; services: EnvironmentService[]; requirements: EnvironmentRequirement[] }>(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/environments`,
        {},
        props.token,
      );
      setEnvironments(response.environments);
      setServices(response.services);
      setRequirements(response.requirements);
      setSelectedEnvironmentId((current) => current || response.environments[0]?.id || "");
    } catch (error) {
      props.onError(error);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { void load(); }, [props.project.project.id]);

  async function createEnvironment(event: React.FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      await api(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/environments`,
        { method: "POST", body: JSON.stringify({ environment_key: key, display_name: name, desired_revision: desiredRevision.trim() || null }) },
        props.token,
      );
      setShowCreate(false);
      props.onNotice("项目环境已创建，等待首次观测");
      await load();
    } catch (error) { props.onError(error); } finally { setBusy(false); }
  }

  async function observe(environment: ProjectEnvironment) {
    const draft = observations[environment.id] ?? { status: environment.status, revision: environment.observed_revision ?? "", services: "" };
    const serviceKeys = draft.services.split(",").map((value) => value.trim()).filter(Boolean);
    setBusy(true);
    try {
      await api(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/environments/${environment.id}`,
        {
          method: "PUT",
          body: JSON.stringify({
            status: draft.status,
            desired_revision: environment.desired_revision,
            observed_revision: draft.revision.trim() || null,
            configuration_fingerprint: null,
            health_summary: { source: "human_console" },
            services: serviceKeys.map((service_key) => ({ service_key, health_status: draft.status === "ready" ? "healthy" : "unhealthy", health_details: {} })),
          }),
        },
        props.token,
      );
      props.onNotice(draft.status === "ready" ? "环境已就绪，满足条件的任务已自动 Ready" : "环境观测已更新");
      await load();
    } catch (error) { props.onError(error); } finally { setBusy(false); }
  }

  async function bindRequirement(event: React.FormEvent) {
    event.preventDefault();
    if (!taskId || !selectedEnvironmentId) return;
    setBusy(true);
    try {
      await api(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/tasks/${taskId}/environments/${selectedEnvironmentId}`,
        { method: "PUT", body: JSON.stringify({ required_revision: requiredRevision.trim() || null, required_services: [], require_healthy: true }) },
        props.token,
      );
      props.onNotice("任务环境要求已保存，环境未满足前任务保持等待");
      await load();
    } catch (error) { props.onError(error); } finally { setBusy(false); }
  }

  return <div className="project-environments-layout">
    <div className="project-tab-heading"><div><span className="eyebrow">PROJECT ENVIRONMENTS</span><h3>项目环境</h3></div><div className="project-tab-actions"><span className="count-badge">{environments.length}</span>{props.canManage ? <button className="button small primary" onClick={() => setShowCreate((value) => !value)}><Icon name="plus" /> 新建环境</button> : null}</div></div>
    {showCreate ? <form className="project-gate-create" onSubmit={createEnvironment}><div className="form-grid"><Field label="环境标识"><input value={key} onChange={(event) => setKey(event.target.value)} required /></Field><Field label="显示名称"><input value={name} onChange={(event) => setName(event.target.value)} required /></Field><Field label="期望 Revision"><input value={desiredRevision} onChange={(event) => setDesiredRevision(event.target.value)} placeholder="commit / image tag" /></Field></div><div className="dialog-actions"><button className="button" type="button" onClick={() => setShowCreate(false)}>取消</button><button className="button primary" disabled={busy}>创建环境</button></div></form> : null}
    {props.canManage && environments.length && props.project.tasks.length ? <form className="project-environment-requirement" onSubmit={bindRequirement}><strong>任务环境要求</strong><select value={taskId} onChange={(event) => setTaskId(event.target.value)} required><option value="">选择任务</option>{props.project.tasks.map((task) => <option key={task.id} value={task.id}>{task.title}</option>)}</select><select value={selectedEnvironmentId} onChange={(event) => setSelectedEnvironmentId(event.target.value)} required>{environments.map((environment) => <option key={environment.id} value={environment.id}>{environment.display_name}</option>)}</select><input value={requiredRevision} onChange={(event) => setRequiredRevision(event.target.value)} placeholder="所需 Revision（可选）" /><button className="button small primary" disabled={busy}>保存要求</button></form> : null}
    {loading ? <div className="empty-inline"><span className="loading-dot" />正在读取项目环境…</div> : null}
    {!loading && !environments.length ? <div className="empty-inline compact-empty"><Icon name="server" /><h3>还没有项目环境</h3></div> : null}
    <div className="project-environment-list">{environments.map((environment) => {
      const draft = observations[environment.id] ?? { status: environment.status, revision: environment.observed_revision ?? "", services: services.filter((service) => service.environment_id === environment.id).map((service) => service.service_key).join(", ") };
      const bound = requirements.filter((requirement) => requirement.environment_id === environment.id);
      return <article className={`project-environment-card ${environment.status}`} key={environment.id}><header><span className={`status-badge ${environment.status}`}><span className="status-dot" />{statusLabels[environment.status]}</span><strong>{environment.display_name}</strong><small>{environment.environment_key}</small></header><div className="environment-revision-grid"><span><small>期望</small><code>{environment.desired_revision ?? "未指定"}</code></span><span><small>实际</small><code>{environment.observed_revision ?? "未观测"}</code></span></div>{bound.length ? <div className="project-gate-tasks">{bound.map((requirement) => <span key={requirement.task_id}><Icon name="tasks" />{tasksById.get(requirement.task_id)?.title ?? requirement.task_id}{requirement.required_revision ? ` · ${requirement.required_revision}` : ""}</span>)}</div> : null}{props.canManage ? <div className="project-environment-actions"><select value={draft.status} onChange={(event) => setObservations((current) => ({ ...current, [environment.id]: { ...draft, status: event.target.value as ProjectEnvironment["status"] } }))}><option value="unknown">未观测</option><option value="provisioning">准备中</option><option value="ready">就绪</option><option value="degraded">异常</option><option value="offline">离线</option></select><input value={draft.revision} onChange={(event) => setObservations((current) => ({ ...current, [environment.id]: { ...draft, revision: event.target.value } }))} placeholder="实际 Revision" /><input value={draft.services} onChange={(event) => setObservations((current) => ({ ...current, [environment.id]: { ...draft, services: event.target.value } }))} placeholder="服务，逗号分隔" /><button className="button small primary" disabled={busy} onClick={() => void observe(environment)} type="button">更新观测</button></div> : null}</article>;
    })}</div>
  </div>;
}
