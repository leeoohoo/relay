import { useEffect, useState } from "react";
import { api } from "../../api/client";
import { Pagination, usePagination } from "../../components/Pagination";
import { Icon } from "../../components/ui";
import type { CompanyConsole, CompanyProject } from "../../types/platform";
import { MemoriesView } from "../app/memory-and-approvals";
import { projectStatusLabel, projectTypeLabel } from "../app/shared";
import { ProjectAssetsCard } from "./assets";
import { CreateProjectDialog } from "./create-project-dialog";
import { ProjectGitCard } from "./git";
import { ProjectOwnerDialog } from "./owner-dialog";
import { ProjectRepositoryBrowser } from "./repository";
import { ProjectRuleCard } from "./rule";
import { ProjectSessionsCard } from "./sessions";
import { TasksView } from "./tasks";

export type ProjectDetailTab = "git" | "repository" | "rule" | "assets" | "tasks" | "memories" | "sessions";

export function ProjectsView(props: {
  consoleData: CompanyConsole;
  token: string;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const canManage = ["owner", "admin"].includes(props.consoleData.human_membership.role);
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [showCreateProject, setShowCreateProject] = useState(false);
  const [showOwnerDialog, setShowOwnerDialog] = useState(false);
  const [activeTab, setActiveTab] = useState<ProjectDetailTab>("git");
  const [projectActionBusy, setProjectActionBusy] = useState(false);
  const selectedProject = props.consoleData.projects.find((project) => project.project.id === selectedProjectId) ?? null;
  const projectStats = {
    git: props.consoleData.projects.filter((project) => project.git).length,
    assets: props.consoleData.projects.reduce((total, project) => total + project.assets.length, 0),
    tasks: props.consoleData.projects.reduce((total, project) => total + project.tasks.length, 0),
  };
  const projectPagination = usePagination(props.consoleData.projects, 8, props.consoleData.company.id);

  function openProject(projectId: string, tab: ProjectDetailTab) {
    setSelectedProjectId(projectId);
    setActiveTab(tab);
  }

  async function setProjectPaused(project: CompanyProject, paused: boolean) {
    setProjectActionBusy(true);
    try {
      await api(`/api/v1/companies/${props.consoleData.company.id}/projects/${project.project.id}/${paused ? "pause" : "resume"}`, { method: "POST" }, props.token);
      await props.onChanged();
      props.onNotice(paused
        ? "项目已暂停：已停止新的项目消息、任务唤醒和资产维护；运行中的项目会话正在取消。"
        : "项目已恢复：项目群和 Agent 工作流重新启用。");
    } catch (error) {
      props.onError(error);
    } finally {
      setProjectActionBusy(false);
    }
  }

  useEffect(() => {
    if (selectedProjectId && !selectedProject) setSelectedProjectId(null);
  }, [selectedProject, selectedProjectId]);

  if (selectedProject) {
    const projectPaused = selectedProject.project.status === "paused";
    const ownerAgent = props.consoleData.agents.find((agent) => agent.agent_profile.id === selectedProject.project.owner_agent_id);
    const ownerLabel = ownerAgent
      ? `${ownerAgent.agent_profile.display_name} · ${ownerAgent.profession?.label ?? ownerAgent.membership.job_title}`
      : "Owner 信息不可用";
    return (
      <div className="content-stack">
        <section className="section-card project-git-detail-card">
          <button className="project-back-button" type="button" onClick={() => { setSelectedProjectId(null); setActiveTab("git"); }}>
            <Icon name="arrow-left" /> 返回项目列表
          </button>
          <div className="project-git-detail-heading">
            <div className="project-repository-icon"><Icon name="git" /></div>
            <div>
              <span className="eyebrow">PROJECT REPOSITORY</span>
              <h2>{selectedProject.project.name}</h2>
              <p>{selectedProject.project.description || "暂无项目说明"}</p>
              <div className="project-detail-facts">
                <span className="project-owner-fact">Owner · {ownerLabel}</span>
                <span>{projectStatusLabel(selectedProject.project.status)}</span>
                <span>{projectTypeLabel(selectedProject.project.project_type, props.consoleData.project_types, props.consoleData.governance_policy.effective_settings.skill_language)} · 识别置信度 {selectedProject.project.project_type_confidence}%</span>
                <span>{selectedProject.git ? `${selectedProject.git.git_host} · ${selectedProject.git.default_branch}` : "等待 Human 配置 Git"}</span>
              </div>
            </div>
            <div className="project-state-actions">
              {canManage ? <button className="button small" type="button" disabled={projectPaused} onClick={() => setShowOwnerDialog(true)}>更换 Owner</button> : null}
              <span className={`git-config-state ${selectedProject.git ? "configured" : ""}`}>
                {selectedProject.git ? "已配置 Git" : "未配置 Git"}
              </span>
              {canManage ? <button className={`button small ${projectPaused ? "primary" : "danger-outline"}`} type="button" disabled={projectActionBusy} onClick={() => void setProjectPaused(selectedProject, !projectPaused)}><Icon name={projectPaused ? "play" : "pause"} /> {projectActionBusy ? "处理中…" : projectPaused ? "恢复项目" : "暂停项目"}</button> : null}
            </div>
          </div>
          {projectPaused ? <div className="project-paused-banner"><Icon name="pause" /><span><strong>项目已暂停</strong><small>项目群不可发送消息，Agent 不会因本项目任务、消息或资产维护启动；正在运行的项目 Codex 会被取消。</small></span></div> : null}
          <div className="project-detail-tabs" role="tablist" aria-label="项目详情">
            <button className={activeTab === "git" ? "active" : ""} type="button" role="tab" aria-selected={activeTab === "git"} onClick={() => setActiveTab("git")}>Git 仓库</button>
            <button className={activeTab === "repository" ? "active" : ""} type="button" role="tab" aria-selected={activeTab === "repository"} onClick={() => setActiveTab("repository")}>项目目录</button>
            <button className={activeTab === "rule" ? "active" : ""} type="button" role="tab" aria-selected={activeTab === "rule"} onClick={() => setActiveTab("rule")}>Rule</button>
            <button className={activeTab === "assets" ? "active" : ""} type="button" role="tab" aria-selected={activeTab === "assets"} onClick={() => setActiveTab("assets")}>项目资产 <span>{selectedProject.assets.length}</span></button>
            <button className={activeTab === "tasks" ? "active" : ""} type="button" role="tab" aria-selected={activeTab === "tasks"} onClick={() => setActiveTab("tasks")}>项目任务 <span>{selectedProject.tasks.length}</span></button>
            <button className={activeTab === "memories" ? "active" : ""} type="button" role="tab" aria-selected={activeTab === "memories"} onClick={() => setActiveTab("memories")}>项目记忆</button>
            <button className={activeTab === "sessions" ? "active" : ""} type="button" role="tab" aria-selected={activeTab === "sessions"} onClick={() => setActiveTab("sessions")}>Agent 会话</button>
          </div>
          {activeTab === "git" ? (
            <ProjectGitCard
              companyId={props.consoleData.company.id}
              project={selectedProject}
              token={props.token}
              onError={props.onError}
            />
          ) : null}
          {activeTab === "repository" ? (
            <ProjectRepositoryBrowser
              companyId={props.consoleData.company.id}
              project={selectedProject}
              token={props.token}
              onError={props.onError}
            />
          ) : null}
          {activeTab === "rule" ? (
            <ProjectRuleCard
              companyId={props.consoleData.company.id}
              project={selectedProject}
              projectTypes={props.consoleData.project_types}
              agents={props.consoleData.agents}
              token={props.token}
              canManage={canManage && !projectPaused}
              onChanged={props.onChanged}
              onError={props.onError}
              onNotice={props.onNotice}
            />
          ) : null}
          {activeTab === "assets" ? (
            <ProjectAssetsCard
              companyId={props.consoleData.company.id}
              project={selectedProject}
              agents={props.consoleData.agents}
              token={props.token}
              canManage={canManage && !projectPaused}
              onChanged={props.onChanged}
              onError={props.onError}
              onNotice={props.onNotice}
            />
          ) : null}
          {activeTab === "tasks" ? (
            <TasksView
              consoleData={props.consoleData}
              token={props.token}
              fixedProjectId={selectedProject.project.id}
              onChanged={props.onChanged}
              onError={props.onError}
              onNotice={props.onNotice}
            />
          ) : null}
          {activeTab === "memories" ? (
            <MemoriesView
              consoleData={props.consoleData}
              token={props.token}
              fixedProjectId={selectedProject.project.id}
              embedded
              onError={props.onError}
              onNotice={props.onNotice}
            />
          ) : null}
          {activeTab === "sessions" ? (
            <ProjectSessionsCard
              companyId={props.consoleData.company.id}
              project={selectedProject}
              token={props.token}
              onError={props.onError}
            />
          ) : null}
          {showOwnerDialog ? (
            <ProjectOwnerDialog
              companyId={props.consoleData.company.id}
              project={selectedProject}
              consoleData={props.consoleData}
              token={props.token}
              onClose={() => setShowOwnerDialog(false)}
              onChanged={props.onChanged}
              onError={props.onError}
              onNotice={props.onNotice}
            />
          ) : null}
        </section>
      </div>
    );
  }

  return (
    <div className="content-stack">
      <section className="section-card">
        <div className="section-heading">
          <div>
            <span className="eyebrow">PROJECT CENTER</span>
            <h2>项目列表</h2>
          </div>
          <div className="section-heading-actions">
            <span className="count-badge">{props.consoleData.projects.length}</span>
            {canManage ? <button className="button primary" type="button" onClick={() => setShowCreateProject(true)}><Icon name="plus" /> 新建项目</button> : null}
          </div>
        </div>
        {props.consoleData.projects.length ? (
          <div className="project-overview-metrics">
            <span><small>项目</small><strong>{props.consoleData.projects.length}</strong></span>
            <span><small>已配置 Git</small><strong>{projectStats.git}</strong></span>
            <span><small>可见资产</small><strong>{projectStats.assets}</strong></span>
            <span><small>项目任务</small><strong>{projectStats.tasks}</strong></span>
          </div>
        ) : null}
        {props.consoleData.projects.length ? (
          <div className="project-repository-list">
            {projectPagination.pageItems.map((project) => (
              <div
                className={`project-repository-row ${project.project.status === "paused" ? "paused" : ""}`}
                key={project.project.id}
              >
                <button className="project-repository-main" type="button" onClick={() => openProject(project.project.id, "git")}>
                  <span className="project-repository-icon"><Icon name="git" /></span>
                  <span className="project-repository-copy">
                    <span className="project-repository-title">
                      <strong>{project.project.name}</strong>
                      <small>{projectTypeLabel(project.project.project_type, props.consoleData.project_types, props.consoleData.governance_policy.effective_settings.skill_language)} · {projectStatusLabel(project.project.status)}</small>
                    </span>
                    <span className="project-repository-facts">
                      Owner · {props.consoleData.agents.find((agent) => agent.agent_profile.id === project.project.owner_agent_id)?.agent_profile.display_name ?? "未知 Agent"} · {project.git ? `${project.git.git_host} · 默认分支 ${project.git.default_branch}${project.git.push_enabled ? " · Agent 可推送" : ""}` : "尚未关联仓库"}
                    </span>
                  </span>
                  <Icon name="chevron-right" />
                </button>
                <div className="project-repository-shortcuts">
                  <button type="button" onClick={() => openProject(project.project.id, "git")}><Icon name="git" /> Git</button>
                  <button type="button" onClick={() => openProject(project.project.id, "rule")}><Icon name="shield" /> Rule</button>
                  <button className={project.assets.length ? "has-assets" : ""} type="button" onClick={() => openProject(project.project.id, "assets")}><Icon name="folder" /> 资产 <strong>{project.assets.length}</strong></button>
                  <span className={`project-refresh-indicator ${project.asset_refresh?.enabled ? "active" : ""}`}>{project.asset_refresh?.enabled ? "定期维护中" : "未设置维护"}</span>
                  {canManage ? <button className={`project-list-pause ${project.project.status === "paused" ? "resume" : ""}`} type="button" disabled={projectActionBusy} onClick={() => void setProjectPaused(project, project.project.status !== "paused")}><Icon name={project.project.status === "paused" ? "play" : "pause"} /> {project.project.status === "paused" ? "恢复" : "暂停"}</button> : null}
                </div>
              </div>
            ))}
            <Pagination {...projectPagination} onPageChange={projectPagination.setPage} />
          </div>
        ) : (
          <div className="empty-inline">
            <Icon name="git" />
            <h3>还没有正式项目</h3>
            <p>从本地文件夹或 Git 地址创建项目。Relay 会自动识别项目类型并加载固定执行规则。</p>
            {canManage ? <button className="button primary" type="button" onClick={() => setShowCreateProject(true)}><Icon name="plus" /> 创建第一个项目</button> : null}
          </div>
        )}
      </section>
      {showCreateProject ? (
        <CreateProjectDialog
          consoleData={props.consoleData}
          token={props.token}
          onClose={() => setShowCreateProject(false)}
          onCreated={async () => {
            setShowCreateProject(false);
            await props.onChanged();
            props.onNotice("项目已创建，固定项目 Skill 会在 Agent 下一次进入项目时自动加载");
          }}
          onError={props.onError}
        />
      ) : null}
    </div>
  );
}
