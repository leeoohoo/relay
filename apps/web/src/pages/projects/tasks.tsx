import { FormEvent, useMemo, useState } from "react";
import { api } from "../../api/client";
import { Pagination, usePagination } from "../../components/Pagination";
import { Field, Icon } from "../../components/ui";
import type { CompanyConsole, CompanyProject, CompanyProjectTask } from "../../types/platform";
import { Dialog, formatTaskDue, formatTime, taskDueClass, taskPriorityLabel, taskStatusLabel, toDateTimeLocalValue } from "../app/shared";
import { TaskExecutionPanel } from "./task-execution";

export function TasksView(props: {
  consoleData: CompanyConsole;
  token: string;
  fixedProjectId?: string;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const canManage = ["owner", "admin"].includes(props.consoleData.human_membership.role);
  const [projectFilter, setProjectFilter] = useState("");
  const [statusFilter, setStatusFilter] = useState("");
  const [assigneeFilter, setAssigneeFilter] = useState("");
  const [search, setSearch] = useState("");
  const [showCreate, setShowCreate] = useState(false);
  const [editingTaskId, setEditingTaskId] = useState<string | null>(null);
  const agentNames = useMemo(
    () => new Map(props.consoleData.agents.map((agent) => [agent.agent_profile.id, agent.agent_profile.display_name])),
    [props.consoleData.agents],
  );
  const scopedProjects = useMemo(
    () => props.fixedProjectId
      ? props.consoleData.projects.filter((project) => project.project.id === props.fixedProjectId)
      : props.consoleData.projects,
    [props.consoleData.projects, props.fixedProjectId],
  );
  const scopedProject = scopedProjects[0] ?? null;
  const taskEntries = useMemo(
    () => scopedProjects.flatMap((project) => project.tasks.map((task) => ({ project, task }))),
    [scopedProjects],
  );
  const activeProjects = scopedProjects.filter((project) => project.project.status !== "paused");
  const filteredTasks = useMemo(() => {
    const normalizedSearch = search.trim().toLowerCase();
    return taskEntries
      .filter(({ project }) => props.fixedProjectId || !projectFilter || project.project.id === projectFilter)
      .filter(({ task }) => !statusFilter || task.status === statusFilter)
      .filter(({ task }) => !assigneeFilter
        || (assigneeFilter === "unassigned" ? !task.assignee_agent_id : task.assignee_agent_id === assigneeFilter))
      .filter(({ project, task }) => !normalizedSearch
        || task.title.toLowerCase().includes(normalizedSearch)
        || task.description.toLowerCase().includes(normalizedSearch)
        || project.project.name.toLowerCase().includes(normalizedSearch))
      .sort((left, right) => {
        const priorityOrder = { urgent: 0, high: 1, normal: 2, low: 3 };
        const statusOrder = { blocked: 0, failed: 1, in_progress: 2, todo: 3, done: 4, cancelled: 5 };
        return statusOrder[left.task.status] - statusOrder[right.task.status]
          || priorityOrder[left.task.priority] - priorityOrder[right.task.priority]
          || new Date(right.task.updated_at).getTime() - new Date(left.task.updated_at).getTime();
      });
  }, [assigneeFilter, projectFilter, props.fixedProjectId, search, statusFilter, taskEntries]);
  const taskPagination = usePagination(filteredTasks, 12, `${projectFilter}:${statusFilter}:${assigneeFilter}:${search}`);
  const editingEntry = taskEntries.find(({ task }) => task.id === editingTaskId) ?? null;
  const stats = {
    todo: taskEntries.filter(({ task }) => task.status === "todo").length,
    inProgress: taskEntries.filter(({ task }) => task.status === "in_progress").length,
    blocked: taskEntries.filter(({ task }) => task.status === "blocked").length,
    failed: taskEntries.filter(({ task }) => task.status === "failed").length,
    done: taskEntries.filter(({ task }) => task.status === "done").length,
  };

  return (
    <div className="content-stack task-center">
      <section className="task-metrics">
        <TaskMetric label="待处理" value={stats.todo} tone="todo" />
        <TaskMetric label="进行中" value={stats.inProgress} tone="in-progress" />
        <TaskMetric label="阻塞" value={stats.blocked} tone="blocked" />
        <TaskMetric label="失败" value={stats.failed} tone="blocked" />
        <TaskMetric label="已完成" value={stats.done} tone="done" />
      </section>

      <section className="section-card task-board-card">
        <div className="section-heading">
          <div>
            <span className="eyebrow">PROJECT TASKS</span>
            <h2>{scopedProject ? `${scopedProject.project.name} · 项目任务` : "项目任务"}</h2>
            <p>任务被分配后会写入 Agent Inbox，并在下次定时 Trigger 中唤醒它的固定 Codex 会话。</p>
          </div>
          <div className="section-heading-actions">
            <span className="count-badge">{filteredTasks.length}</span>
            {canManage ? (
              <button className="button primary small" type="button" onClick={() => setShowCreate(true)} disabled={!activeProjects.length}>
                <Icon name="plus" /> 新建任务
              </button>
            ) : null}
          </div>
        </div>

        <div className={`task-filters ${props.fixedProjectId ? "fixed-project" : ""}`}>
          <label className="task-search">
            <Icon name="search" />
            <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="搜索任务或项目" />
          </label>
          {!props.fixedProjectId ? <select value={projectFilter} onChange={(event) => setProjectFilter(event.target.value)}>
            <option value="">全部项目</option>
            {props.consoleData.projects.map((project) => <option key={project.project.id} value={project.project.id}>{project.project.name}</option>)}
          </select> : null}
          <select value={statusFilter} onChange={(event) => setStatusFilter(event.target.value)}>
            <option value="">全部状态</option>
            <option value="todo">待处理</option>
            <option value="in_progress">进行中</option>
            <option value="blocked">阻塞</option>
            <option value="failed">失败</option>
            <option value="done">已完成</option>
            <option value="cancelled">已取消</option>
          </select>
          <select value={assigneeFilter} onChange={(event) => setAssigneeFilter(event.target.value)}>
            <option value="">全部负责人</option>
            <option value="unassigned">未分配</option>
            {props.consoleData.agents.map((agent) => <option key={agent.agent_profile.id} value={agent.agent_profile.id}>{agent.agent_profile.display_name}</option>)}
          </select>
        </div>

        {filteredTasks.length ? (
          <div className="task-list">
            <div className="task-list-head">
              <span>任务</span><span>状态</span><span>优先级</span><span>负责人</span><span>截止时间</span>
            </div>
            {taskPagination.pageItems.map(({ project, task }) => {
              const dependencies = project.task_dependencies
                .filter((dependency) => dependency.task_id === task.id)
                .map((dependency) => ({ dependency, task: project.tasks.find((candidate) => candidate.id === dependency.depends_on_task_id) }))
                .filter((item): item is { dependency: CompanyProject["task_dependencies"][number]; task: CompanyProjectTask } => Boolean(item.task));
              const unresolvedDependencies = dependencies.filter((item) => !taskDependencyResolved(item.dependency.dependency_condition, item.task.status));
              return (
                <div
                  className={`task-row ${canManage && project.project.status !== "paused" ? "editable" : ""} ${project.project.status === "paused" ? "project-paused" : ""}`}
                  key={task.id}
                  role={canManage && project.project.status !== "paused" ? "button" : undefined}
                  tabIndex={canManage && project.project.status !== "paused" ? 0 : undefined}
                  onClick={() => { if (canManage && project.project.status !== "paused") setEditingTaskId(task.id); }}
                  onKeyDown={(event) => { if (canManage && project.project.status !== "paused" && (event.key === "Enter" || event.key === " ")) setEditingTaskId(task.id); }}
                >
                  <span className="task-title-cell">
                    <strong>{task.title}</strong>
                    <small>{project.project.name}{project.project.status === "paused" ? " · 项目已暂停" : ""}{dependencies.length ? ` · ${dependencies.length} 个前置任务` : ""}</small>
                    {unresolvedDependencies.length ? <em className="task-waiting-dependencies">等待：{unresolvedDependencies.map((item) => item.task.title).join("、")}</em> : null}
                  </span>
                  <span>{unresolvedDependencies.length ? <span className="task-status waiting"><span className="status-dot" />等待前置</span> : <TaskStatusBadge status={task.status} />}</span>
                  <span><TaskPriorityBadge priority={task.priority} /></span>
                  <span className="task-assignee">
                    {task.assignee_agent_id ? <span className="agent-avatar tiny">{(agentNames.get(task.assignee_agent_id) ?? "A").slice(0, 1)}</span> : null}
                    <span>{task.assignee_agent_id ? agentNames.get(task.assignee_agent_id) ?? "未知 Agent" : "未分配"}</span>
                  </span>
                  <span className={taskDueClass(task)}>{task.due_at ? formatTaskDue(task.due_at) : "未设置"}</span>
                </div>
              );
            })}
            <Pagination {...taskPagination} onPageChange={taskPagination.setPage} />
          </div>
        ) : (
          <div className="empty-inline">
            <Icon name="tasks" />
            <h3>{taskEntries.length ? "没有符合筛选条件的任务" : "还没有任务"}</h3>
            <p>{taskEntries.length ? "调整项目、状态或负责人筛选后再试。" : canManage ? "创建第一条任务并分配给项目 Agent。" : "具备管理权限的 Human 或 Agent 可以创建任务。"}</p>
          </div>
        )}
      </section>

      {showCreate ? (
        <TaskDialog
          companyId={props.consoleData.company.id}
          projects={activeProjects}
          token={props.token}
          onClose={() => setShowCreate(false)}
          onSaved={async (task) => {
            setShowCreate(false);
            props.onNotice(task.assignee_agent_id ? "任务已创建，负责人会在下次 Trigger 时收到它。" : "任务已创建。" );
            await props.onChanged();
          }}
          onError={props.onError}
          onNotice={props.onNotice}
        />
      ) : null}
      {editingEntry ? (
        <TaskDialog
          companyId={props.consoleData.company.id}
          projects={scopedProjects}
          task={editingEntry.task}
          token={props.token}
          onClose={() => setEditingTaskId(null)}
          onSaved={async (task) => {
            setEditingTaskId(null);
            props.onNotice(task.assignee_agent_id !== editingEntry.task.assignee_agent_id ? "任务已更新，新负责人会收到 Inbox 事件。" : "任务已更新。" );
            await props.onChanged();
          }}
          onError={props.onError}
          onNotice={props.onNotice}
        />
      ) : null}
    </div>
  );
}

function taskDependencyResolved(
  condition: CompanyProject["task_dependencies"][number]["dependency_condition"],
  status: CompanyProjectTask["status"],
) {
  if (condition === "completion") return ["done", "failed", "cancelled"].includes(status);
  if (condition === "failure") return status === "failed";
  return ["done", "cancelled"].includes(status);
}

function TaskMetric({ label, value, tone }: { label: string; value: number; tone: string }) {
  return <div className={`task-metric ${tone}`}><span className="task-metric-icon"><Icon name="tasks" /></span><span><small>{label}</small><strong>{value}</strong></span></div>;
}

function TaskStatusBadge({ status }: { status: CompanyProjectTask["status"] }) {
  return <span className={`task-status ${status}`}><span className="status-dot" />{taskStatusLabel(status)}</span>;
}

function TaskPriorityBadge({ priority }: { priority: CompanyProjectTask["priority"] }) {
  return <span className={`task-priority ${priority}`}>{taskPriorityLabel(priority)}</span>;
}


function TaskDialog(props: {
  companyId: string;
  projects: CompanyProject[];
  task?: CompanyProjectTask;
  token: string;
  onClose: () => void;
  onSaved: (task: CompanyProjectTask) => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [projectId, setProjectId] = useState(props.task?.project_id ?? props.projects[0]?.project.id ?? "");
  const [title, setTitle] = useState(props.task?.title ?? "");
  const [description, setDescription] = useState(props.task?.description ?? "");
  const [status, setStatus] = useState<CompanyProjectTask["status"]>(props.task?.status ?? "todo");
  const [priority, setPriority] = useState<CompanyProjectTask["priority"]>(props.task?.priority ?? "normal");
  const [assigneeId, setAssigneeId] = useState(props.task?.assignee_agent_id ?? "");
  const [dueAt, setDueAt] = useState(toDateTimeLocalValue(props.task?.due_at ?? null));
  const [dependencyIds, setDependencyIds] = useState<string[]>(() => {
    if (!props.task) return [];
    const taskProject = props.projects.find((item) => item.project.id === props.task?.project_id);
    return taskProject?.task_dependencies
      .filter((dependency) => dependency.task_id === props.task?.id)
      .map((dependency) => dependency.depends_on_task_id) ?? [];
  });
  const [busy, setBusy] = useState(false);
  const project = props.projects.find((item) => item.project.id === projectId) ?? null;
  const activeMembers = project?.members.filter((member) => !member.member.left_at) ?? [];
  const dependencyCandidates = project?.tasks.filter((candidate) => candidate.id !== props.task?.id) ?? [];
  const selectedDependencies = dependencyIds
    .map((dependencyId) => dependencyCandidates.find((candidate) => candidate.id === dependencyId))
    .filter((dependency): dependency is CompanyProjectTask => Boolean(dependency));
  const unresolvedDependencies = selectedDependencies.filter((dependency) => {
    const condition = project?.task_dependencies.find((item) => (
      item.task_id === props.task?.id && item.depends_on_task_id === dependency.id
    ))?.dependency_condition ?? "success";
    return !taskDependencyResolved(condition, dependency.status);
  });
  const dependenciesLocked = Boolean(props.task && ["done", "failed", "cancelled"].includes(props.task.status));
  const statusBlockedByDependencies = Boolean(props.task && unresolvedDependencies.length && ["in_progress", "done"].includes(status));
  const statusHistory = props.task
    ? (project?.task_status_history ?? []).filter((entry) => entry.task_id === props.task?.id)
    : [];
  const dependencyPagination = usePagination(dependencyCandidates, 8, projectId);
  const historyPagination = usePagination(statusHistory, 6, props.task?.id ?? "new");

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!projectId) return;
    setBusy(true);
    try {
      const endpoint = props.task
        ? `/api/v1/companies/${props.companyId}/projects/${projectId}/tasks/${props.task.id}`
        : `/api/v1/companies/${props.companyId}/projects/${projectId}/tasks`;
      const body = props.task
        ? {
            title,
            description,
            status,
            priority,
            assignee_agent_id: assigneeId || null,
            clear_assignee: !assigneeId,
            due_at: dueAt ? new Date(dueAt).toISOString() : null,
            clear_due_at: !dueAt,
            depends_on_task_ids: dependencyIds,
          }
        : {
            title,
            description: description || null,
            priority,
            assignee_agent_id: assigneeId || null,
            due_at: dueAt ? new Date(dueAt).toISOString() : null,
            depends_on_task_ids: dependencyIds,
          };
      const response = await api<{ task: CompanyProjectTask }>(endpoint, {
        method: props.task ? "PUT" : "POST",
        body: JSON.stringify(body),
      }, props.token);
      await props.onSaved(response.task);
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog
      title={props.task ? "编辑任务" : "新建任务"}
      description={props.task ? "Human 的修改会保留审计信息；重新分配会唤醒新的负责人。" : "分配给 Agent 后，Relay 只产生任务事件并由本地 Codex 在定时启动时处理。"}
      onClose={props.onClose}
      wide
    >
      <form className="stack-form task-form" onSubmit={submit}>
        <Field label="所属项目">
          <select value={projectId} onChange={(event) => { setProjectId(event.target.value); setAssigneeId(""); setDependencyIds([]); }} disabled={Boolean(props.task)} required>
            {props.projects.map((item) => <option key={item.project.id} value={item.project.id}>{item.project.name}</option>)}
          </select>
        </Field>
        <Field label="任务标题">
          <input value={title} onChange={(event) => setTitle(event.target.value)} maxLength={160} placeholder="写清楚可验证的交付物" required />
        </Field>
        <Field label="任务说明与验收标准">
          <textarea value={description} onChange={(event) => setDescription(event.target.value)} maxLength={4000} placeholder="背景、范围、验收标准、相关文件或限制条件" />
        </Field>
        <Field label="前置任务">
          <div className="task-dependency-picker">
            {dependencyCandidates.length ? dependencyPagination.pageItems.map((candidate) => (
              <label className="task-dependency-option" key={candidate.id}>
                <input
                  type="checkbox"
                  checked={dependencyIds.includes(candidate.id)}
                  disabled={dependenciesLocked}
                  onChange={(event) => setDependencyIds((current) => event.target.checked ? [...current, candidate.id] : current.filter((id) => id !== candidate.id))}
                />
                <span><strong>{candidate.title}</strong><small>{taskStatusLabel(candidate.status)}{candidate.assignee_agent_id ? ` · ${activeMembers.find((member) => member.agent_profile.id === candidate.assignee_agent_id)?.agent_profile.display_name ?? "未知 Agent"}` : " · 未分配"}</small></span>
              </label>
            )) : <div className="task-dependency-empty">当前项目还没有其他任务。先创建基础任务后，就可以把它选作前置。</div>}
            <Pagination {...dependencyPagination} onPageChange={dependencyPagination.setPage} compact />
          </div>
          {dependenciesLocked ? <small>已完成、失败或已取消的任务不能再修改前置关系。</small> : unresolvedDependencies.length ? <small className="task-dependency-warning">当前需等待：{unresolvedDependencies.map((dependency) => dependency.title).join("、")}</small> : dependencyIds.length ? <small>所选前置任务均已完成，可以开始执行。</small> : <small>未选择前置任务时，这条任务可以直接开始。</small>}
        </Field>
        <div className="task-form-grid">
          {props.task ? (
            <Field label="状态">
              <select value={status} onChange={(event) => setStatus(event.target.value as CompanyProjectTask["status"])}>
                <option value="todo">待处理</option><option value="in_progress" disabled={Boolean(unresolvedDependencies.length)}>进行中</option><option value="blocked">阻塞</option><option value="failed">失败</option><option value="done" disabled={Boolean(unresolvedDependencies.length)}>已完成</option><option value="cancelled">已取消</option>
              </select>
            </Field>
          ) : null}
          <Field label="优先级">
            <select value={priority} onChange={(event) => setPriority(event.target.value as CompanyProjectTask["priority"])}>
              <option value="low">低</option><option value="normal">普通</option><option value="high">高</option><option value="urgent">紧急</option>
            </select>
          </Field>
          <Field label="负责人">
            <select value={assigneeId} onChange={(event) => setAssigneeId(event.target.value)}>
              <option value="">暂不分配</option>
              {activeMembers.map((member) => <option key={member.agent_profile.id} value={member.agent_profile.id}>{member.agent_profile.display_name} · {member.member.role}</option>)}
            </select>
          </Field>
          <Field label="截止时间">
            <input type="datetime-local" value={dueAt} onChange={(event) => setDueAt(event.target.value)} />
          </Field>
        </div>
        {props.task && statusHistory.length ? (
          <div className="task-status-history">
            <strong>状态历史</strong>
            {historyPagination.pageItems.map((entry) => {
              const actor = entry.changed_by_agent_id
                ? activeMembers.find((member) => member.agent_profile.id === entry.changed_by_agent_id)?.agent_profile.display_name ?? "Agent"
                : entry.changed_by_human_user_id ? "Human" : "系统";
              return <span key={entry.id}><small>{formatTime(entry.created_at)}</small><em>{entry.from_status ? `${taskStatusLabel(entry.from_status)} → ` : "初始状态："}{taskStatusLabel(entry.to_status)}</em><i>{actor}</i></span>;
            })}
            <Pagination {...historyPagination} onPageChange={historyPagination.setPage} compact />
          </div>
        ) : null}
        {props.task && project ? <TaskExecutionPanel companyId={props.companyId} project={project} task={props.task} token={props.token} onError={props.onError} onNotice={props.onNotice} /> : null}
        {!activeMembers.length ? <div className="git-security-note">这个项目还没有活跃成员。你可以先保存为未分配任务，或让有权限的 Agent 添加项目成员。</div> : null}
        <div className="dialog-actions">
          <button className="button" type="button" onClick={props.onClose} disabled={busy}>取消</button>
          <button className="button primary" disabled={busy || !projectId || statusBlockedByDependencies}>{busy ? "正在保存…" : props.task ? "保存修改" : "创建任务"}</button>
        </div>
      </form>
    </Dialog>
  );
}
