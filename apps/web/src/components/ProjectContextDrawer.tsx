import { useEffect, useMemo, useState } from "react";
import type { CompanyAgent, CompanyProject, CompanyProjectTask } from "../types/platform";
import { ProjectRepositoryBrowser } from "../pages/projects/repository";
import { Pagination, usePagination } from "./Pagination";
import { Icon } from "./ui";

export type ProjectContextTab = "repository" | "tasks";

export function ProjectContextDrawer(props: {
  companyId: string;
  project: CompanyProject;
  agents: CompanyAgent[];
  token: string;
  activeTab: ProjectContextTab;
  selectedTaskId?: string | null;
  onTabChange: (tab: ProjectContextTab) => void;
  onTaskSelect?: (taskId: string | null) => void;
  onClose: () => void;
  onError: (error: unknown) => void;
}) {
  return (
    <aside className={`project-context-drawer ${props.activeTab}`} aria-label="项目上下文">
      <header className="project-context-drawer-head">
        <div>
          <span className="drawer-eyebrow">PROJECT CONTEXT</span>
          <strong>{props.project.project.name}</strong>
          <small>{props.project.git?.default_branch ?? "仓库初始化中"} · {props.project.tasks.length} 个任务</small>
        </div>
        <button type="button" className="icon-button" aria-label="关闭项目面板" onClick={props.onClose}><Icon name="close" /></button>
      </header>

      <nav className="project-context-tabs" role="tablist" aria-label="项目内容">
        <button className={props.activeTab === "repository" ? "active" : ""} type="button" role="tab" aria-selected={props.activeTab === "repository"} onClick={() => props.onTabChange("repository")}>
          <Icon name="folder" /> 目录
        </button>
        <button className={props.activeTab === "tasks" ? "active" : ""} type="button" role="tab" aria-selected={props.activeTab === "tasks"} onClick={() => props.onTabChange("tasks")}>
          <Icon name="tasks" /> 任务 <span>{props.project.tasks.length}</span>
        </button>
      </nav>

      <div className="project-context-drawer-body">
        {props.activeTab === "repository" ? (
          <ProjectRepositoryBrowser
            companyId={props.companyId}
            project={props.project}
            token={props.token}
            onError={props.onError}
          />
        ) : (
          <ProjectTaskPanel project={props.project} agents={props.agents} selectedTaskId={props.selectedTaskId} onTaskSelect={props.onTaskSelect} />
        )}
      </div>
    </aside>
  );
}

function ProjectTaskPanel(props: { project: CompanyProject; agents: CompanyAgent[]; selectedTaskId?: string | null; onTaskSelect?: (taskId: string | null) => void }) {
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState("");
  const agentNames = useMemo(
    () => new Map(props.agents.map((agent) => [agent.agent_profile.id, agent.agent_profile.display_name])),
    [props.agents],
  );
  const tasks = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase();
    const statusOrder: Record<CompanyProjectTask["status"], number> = { in_progress: 0, blocked: 1, failed: 2, todo: 3, done: 4, cancelled: 5 };
    const priorityOrder: Record<CompanyProjectTask["priority"], number> = { urgent: 0, high: 1, normal: 2, low: 3 };
    return props.project.tasks
      .filter((task) => !status || task.status === status)
      .filter((task) => !normalizedQuery
        || task.title.toLocaleLowerCase().includes(normalizedQuery)
        || task.description.toLocaleLowerCase().includes(normalizedQuery)
        || (task.assignee_agent_id ? agentNames.get(task.assignee_agent_id)?.toLocaleLowerCase().includes(normalizedQuery) : false))
      .sort((left, right) => statusOrder[left.status] - statusOrder[right.status]
        || priorityOrder[left.priority] - priorityOrder[right.priority]
        || right.updated_at.localeCompare(left.updated_at));
  }, [agentNames, props.project.tasks, query, status]);
  const pagination = usePagination(tasks, 8, `${query}:${status}:${props.project.project.id}`);

  useEffect(() => {
    if (!props.selectedTaskId) return;
    const index = tasks.findIndex((task) => task.id === props.selectedTaskId);
    if (index >= 0) pagination.setPage(Math.floor(index / pagination.pageSize) + 1);
  }, [props.selectedTaskId, tasks, pagination.pageSize]);

  return (
    <section className="project-context-tasks" aria-label="项目任务列表">
      <div className="project-context-task-filters">
        <label><Icon name="search" /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索任务或负责人" /></label>
        <select aria-label="筛选任务状态" value={status} onChange={(event) => setStatus(event.target.value)}>
          <option value="">全部状态</option>
          <option value="todo">待处理</option>
          <option value="in_progress">进行中</option>
          <option value="blocked">阻塞</option>
          <option value="failed">失败</option>
          <option value="done">已完成</option>
          <option value="cancelled">已取消</option>
        </select>
      </div>

      <div className="project-context-task-summary">
        <span><i className="in_progress" />进行中 <strong>{props.project.tasks.filter((task) => task.status === "in_progress").length}</strong></span>
        <span><i className="todo" />待处理 <strong>{props.project.tasks.filter((task) => task.status === "todo").length}</strong></span>
        <span><i className="blocked" />异常 <strong>{props.project.tasks.filter((task) => ["blocked", "failed"].includes(task.status)).length}</strong></span>
      </div>

      <div className="project-context-task-list">
        {pagination.pageItems.map((task) => {
          const dependencies = props.project.task_dependencies
            .filter((dependency) => dependency.task_id === task.id)
            .map((dependency) => ({ dependency, task: props.project.tasks.find((candidate) => candidate.id === dependency.depends_on_task_id) }))
            .filter((item): item is { dependency: CompanyProject["task_dependencies"][number]; task: CompanyProjectTask } => Boolean(item.task));
          const unresolved = dependencies.filter(({ dependency, task }) => {
            if (dependency.dependency_condition === "completion") return !["done", "failed", "cancelled"].includes(task.status);
            if (dependency.dependency_condition === "failure") return task.status !== "failed";
            return !["done", "cancelled"].includes(task.status);
          });
          const assignee = task.assignee_agent_id ? agentNames.get(task.assignee_agent_id) ?? "未知 Agent" : "未分配";
          return (
            <article
              className={`project-context-task ${task.status} ${props.selectedTaskId === task.id ? "selected" : ""}`}
              key={task.id}
              role="button"
              tabIndex={0}
              aria-pressed={props.selectedTaskId === task.id}
              onClick={() => props.onTaskSelect?.(props.selectedTaskId === task.id ? null : task.id)}
              onKeyDown={(event) => {
                if (event.key !== "Enter" && event.key !== " ") return;
                event.preventDefault();
                props.onTaskSelect?.(props.selectedTaskId === task.id ? null : task.id);
              }}
            >
              <div className="project-context-task-head">
                <span className={`task-state ${unresolved.length ? "blocked" : task.status}`}>{unresolved.length ? "等待前置" : taskStatusLabel(task.status)}</span>
                <span className={`project-context-task-priority ${task.priority}`}>{taskPriorityLabel(task.priority)}</span>
              </div>
              <strong>{task.title}</strong>
              {task.description ? <p>{task.description}</p> : null}
              {unresolved.length ? <div className="project-context-task-dependency"><Icon name="network" /> 等待：{unresolved.map((item) => item.task.title).join("、")}</div> : null}
              <footer>
                <span><span className="agent-avatar tiny">{assignee.slice(0, 1)}</span>{assignee}</span>
                <time>{task.due_at ? `截止 ${formatTaskDate(task.due_at)}` : `更新 ${formatTaskDate(task.updated_at)}`}</time>
              </footer>
            </article>
          );
        })}
        {!tasks.length ? <div className="project-context-task-empty"><Icon name="tasks" />{props.project.tasks.length ? "没有符合筛选条件的任务" : "当前项目还没有任务"}</div> : null}
      </div>
      <Pagination {...pagination} onPageChange={pagination.setPage} compact />
    </section>
  );
}

function taskStatusLabel(status: CompanyProjectTask["status"]) {
  return ({ todo: "待处理", in_progress: "进行中", blocked: "阻塞", done: "已完成", failed: "失败", cancelled: "已取消" } as const)[status];
}

function taskPriorityLabel(priority: CompanyProjectTask["priority"]) {
  return ({ low: "低", normal: "普通", high: "高", urgent: "紧急" } as const)[priority];
}

function formatTaskDate(value: string) {
  return new Date(value).toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" });
}
