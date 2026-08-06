import { FormEvent, ReactNode, useEffect, useMemo, useRef, useState } from "react";
import { API_BASE_URL, api } from "../api/client";
import type { CompanyRealtimeEvent } from "../api/types";
import { AuthScreen } from "../components/AuthScreen";
import { MessagesView } from "../components/MessagesView";
import { Pagination, usePagination } from "../components/Pagination";
import { Sidebar } from "../components/Sidebar";
import { Field, Icon } from "../components/ui";
import { useCompanyEvents } from "../hooks/useCompanyEvents";
import { UiLanguage, useUiLanguage } from "../i18n/uiLanguage";
import {
  analyzeRelaySkill,
  getRelaySkillDocuments,
  relayProfessionSkillDocument,
  RELAY_EMPLOYEE_SKILL,
  RELAY_EMPLOYEE_SKILL_EN,
  RELAY_PROFESSION_SKILLS,
  RELAY_STAFFING_MANAGER_SKILL,
  RELAY_STAFFING_MANAGER_SKILL_EN,
  RelaySkillDocument,
  RelaySkillLanguage,
  RelaySkillSection,
} from "../relaySkills";
import type { Company, HumanUser, RuntimeConfig, Session, View } from "../types/appShell";
import type { Conversation, Message } from "../types/chat";
import type {
  AgentMemory,
  AgentMembership,
  AgentProfile,
  AgentToolApproval,
  BatchCredentialResult,
  CodexApprovalPolicy,
  CodexAuthProfile,
  CodexCliRuntime,
  CodexDefaultAuthEnvironment,
  CodexEnvironmentView,
  CodexPersonality,
  CodexReasoningEffort,
  CodexReasoningSummary,
  CodexRunnerProfileView,
  CodexSandboxMode,
  CodexTriggerRun,
  CodexTriggerView,
  CodexVerbosity,
  CodexWebSearch,
  CompanyAgent,
  CompanyConsole,
  CompanyProfession,
  CompanyProject,
  CompanyProjectTask,
  CompanyProjectType,
  Credential,
  OrgUnit,
  ProjectGitAdminView,
} from "../types/platform";
import {
  approvalRequestDetail,
  approvalRiskLabel,
  approvalStatusLabel,
  approvalToolLabel,
  CodeBlock,
  codexActivityPhaseLabel,
  codexOperationalStatusLabel,
  codexReasoningEffortLabel,
  codexRunDisplayMessage,
  codexTriggerStatusLabel,
  codexTriggerTypeLabel,
  collaborationPreferenceLabel,
  companyAgentProfessionKey,
  copyText,
  Dialog,
  EmptyCompany,
  formatAgentCount,
  formatElapsed,
  formatInterval,
  formatRunSeconds,
  formatSkillBundle,
  formatTaskDue,
  formatTime,
  LoadingState,
  memoryStatusLabel,
  memoryTierLabel,
  memoryTypeLabel,
  Metric,
  persistSession,
  projectAssetTypeLabel,
  projectStatusLabel,
  projectTypeLabel,
  readSession,
  relayAgentConnectionNames,
  SkillCopyBlock,
  StatusBadge,
  taskDueClass,
  taskPriorityLabel,
  taskStatusLabel,
  Toast,
  toDateTimeLocalValue,
} from "./app/shared";
import {
  BatchCredentialDialog,
  CreateAgentDialog,
  CreateCompanyDialog,
  CredentialDialog,
  UserPreferencesDialog,
} from "./app/organization";
import { PROJECT_PERMISSIONS, STAFFING_PERMISSIONS } from "./app/permissions";
import { ApprovalDialog, ApprovalsView, MemoriesView } from "./app/memory-and-approvals";
import { OrganizationAgentCenter } from "./app/agents";
import { ChatCenter } from "./app/chat";
import { SkillsView } from "./app/skills";
import { CodexControlCenter } from "./codex/control-center";

const SESSION_KEY = "agent_company_session";
export function App() {
  const [runtimeConfig, setRuntimeConfig] = useState<RuntimeConfig | null>(null);
  const [session, setSession] = useState<Session | null>(() => readSession());
  const [companies, setCompanies] = useState<Company[]>([]);
  const [selectedCompanyId, setSelectedCompanyId] = useState<string | null>(null);
  const [companyConsole, setCompanyConsole] = useState<CompanyConsole | null>(null);
  const [view, setView] = useState<View>("agents");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [showCompanyForm, setShowCompanyForm] = useState(false);
  const [showAgentForm, setShowAgentForm] = useState(false);
  const [showUserPreferences, setShowUserPreferences] = useState(false);
  const [credential, setCredential] = useState<Credential | null>(null);
  const [batchCredentials, setBatchCredentials] = useState<BatchCredentialResult | null>(null);
  const [approvals, setApprovals] = useState<AgentToolApproval[]>([]);
  const [dismissedApprovalIds, setDismissedApprovalIds] = useState<Set<string>>(() => new Set());
  const [realtimeEvent, setRealtimeEvent] = useState<CompanyRealtimeEvent | null>(null);
  const realtimeRefreshTimerRef = useRef<number | null>(null);

  useEffect(() => {
    api<RuntimeConfig>("/api/v1/runtime-config")
      .then(setRuntimeConfig)
      .catch(() => setRuntimeConfig({ dev_endpoints_enabled: false, email_verification_required: false, project_types: [] }));
  }, []);

  useEffect(() => {
    if (!session) return;
    let active = true;
    const heartbeat = async () => {
      try {
        const { user } = await api<{ user: HumanUser }>("/api/v1/auth/me", {}, session.token);
        if (!active) return;
        const next = { ...session, user };
        setSession(next);
        persistSession(next);
      } catch {
        if (active) signOut(false);
      }
    };
    void heartbeat();
    return () => {
      active = false;
    };
  }, [session?.token]);

  useEffect(() => {
    if (!session) {
      setCompanies([]);
      setSelectedCompanyId(null);
      setCompanyConsole(null);
      return;
    }
    void loadCompanies(session.token);
  }, [session?.token]);

  useEffect(() => {
    if (!session || !selectedCompanyId) {
      setCompanyConsole(null);
      return;
    }
    void loadCompanyConsole(selectedCompanyId, session.token);
  }, [selectedCompanyId, session?.token]);

  useEffect(() => {
    setApprovals([]);
    setDismissedApprovalIds(new Set());
    if (!session || !selectedCompanyId || !companyConsole || !["owner", "admin"].includes(companyConsole.human_membership.role)) return;
    let active = true;
    const refresh = async () => {
      try {
        const response = await api<{ approvals: AgentToolApproval[] }>(
          `/api/v1/companies/${selectedCompanyId}/approvals?limit=100`,
          {},
          session.token,
        );
        if (active) setApprovals(response.approvals);
      } catch {
        // Approval polling should not interrupt the rest of the console.
      }
    };
    void refresh();
    return () => {
      active = false;
    };
  }, [session?.token, selectedCompanyId, companyConsole?.human_membership.role]);

  useCompanyEvents({
    companyId: selectedCompanyId,
    token: session?.token ?? null,
    onEvent: (event) => {
      setRealtimeEvent(event);
      if (event.event_type.startsWith("agent.runtime.approval_")) {
        void refreshApprovals().catch(() => undefined);
      }
      if (realtimeRefreshTimerRef.current !== null) {
        window.clearTimeout(realtimeRefreshTimerRef.current);
      }
      realtimeRefreshTimerRef.current = window.setTimeout(() => {
        realtimeRefreshTimerRef.current = null;
        if (selectedCompanyId && session?.token) {
          void loadCompanyConsole(selectedCompanyId, session.token);
        }
      }, 100);
    },
  });

  useEffect(() => () => {
    if (realtimeRefreshTimerRef.current !== null) {
      window.clearTimeout(realtimeRefreshTimerRef.current);
    }
  }, []);

  async function loadCompanies(token: string, preferredCompanyId?: string) {
    try {
      const response = await api<{ companies: Company[] }>("/api/v1/companies", {}, token);
      setCompanies(response.companies);
      const currentIsValid = response.companies.some((company) => company.id === selectedCompanyId);
      const nextId = preferredCompanyId ?? (currentIsValid ? selectedCompanyId : response.companies[0]?.id) ?? null;
      setSelectedCompanyId(nextId);
    } catch (requestError) {
      showError(requestError);
    }
  }

  async function loadCompanyConsole(companyId: string, token = session?.token) {
    if (!token) return;
    try {
      const response = await api<{ company_console: CompanyConsole }>(
        `/api/v1/companies/${companyId}/console`,
        {},
        token,
      );
      setCompanyConsole(response.company_console);
    } catch (requestError) {
      showError(requestError);
    }
  }

  function completeAuth(next: Session) {
    setSession(next);
    persistSession(next);
    setError("");
  }

  async function signOut(callApi = true) {
    if (callApi && session) {
      try {
        await api("/api/v1/auth/logout", { method: "POST" }, session.token);
      } catch {
        // Local sign-out must still succeed if the server session already expired.
      }
    }
    localStorage.removeItem(SESSION_KEY);
    setSession(null);
  }

  function showError(value: unknown) {
    setError(value instanceof Error ? value.message : "请求失败，请稍后重试");
  }

  async function refreshCompany() {
    if (selectedCompanyId) await loadCompanyConsole(selectedCompanyId);
  }

  async function refreshApprovals() {
    if (!session || !selectedCompanyId) return;
    const response = await api<{ approvals: AgentToolApproval[] }>(
      `/api/v1/companies/${selectedCompanyId}/approvals?limit=100`,
      {},
      session.token,
    );
    setApprovals(response.approvals);
  }

  async function reviewApproval(approvalId: string, decision: "approve" | "reject", reviewNote: string) {
    if (!session || !selectedCompanyId) return;
    await api(
      `/api/v1/companies/${selectedCompanyId}/approvals/${approvalId}/${decision}`,
      { method: "POST", body: JSON.stringify({ review_note: reviewNote || null }) },
      session.token,
    );
    setDismissedApprovalIds((current) => {
      const next = new Set(current);
      next.add(approvalId);
      return next;
    });
    await refreshApprovals();
    setNotice(decision === "approve" ? "审批已通过，等待中的 Codex 会继续执行" : "审批已拒绝，Codex 会收到拒绝结果并继续处理");
  }

  const pendingApprovals = approvals.filter((approval) => approval.status === "pending");
  const popupApproval = pendingApprovals.find((approval) => !dismissedApprovalIds.has(approval.id)) ?? null;

  if (!session) {
    return (
      <AuthScreen
        runtimeConfig={runtimeConfig}
        busy={busy}
        error={error}
        setBusy={setBusy}
        setError={setError}
        onAuthenticated={completeAuth}
      />
    );
  }

  return (
    <div className="app-shell">
      <Sidebar
        user={session.user}
        companies={companies}
        selectedCompanyId={selectedCompanyId}
        view={view}
        onCompanyChange={setSelectedCompanyId}
        onViewChange={setView}
        pendingApprovalCount={pendingApprovals.length}
        onCreateCompany={() => setShowCompanyForm(true)}
        onOpenPreferences={() => setShowUserPreferences(true)}
        onSignOut={() => void signOut()}
      />

      <main className="main-content">
        {error ? <Toast tone="error" onClose={() => setError("")}>{error}</Toast> : null}
        {notice ? <Toast onClose={() => setNotice("")}>{notice}</Toast> : null}

        {view === "skills" ? (
          <>
            <header className="page-header">
              <div>
                <span className="eyebrow">{companyConsole?.company.slug ?? "system-skills"}</span>
                <h1>Skill 中心</h1>
                <p>Agent 工作协议与职业能力目录</p>
              </div>
            </header>
            <SkillsView
              consoleData={companyConsole}
              systemProjectTypes={runtimeConfig?.project_types ?? []}
            />
          </>
        ) : !companies.length ? (
          <EmptyCompany onCreate={() => setShowCompanyForm(true)} />
        ) : !companyConsole ? (
          <LoadingState />
        ) : (
          <>
            <header className="page-header">
              <div>
                <span className="eyebrow">{companyConsole.company.slug}</span>
                <h1>{view === "agents" ? "组织与 Agent" : view === "projects" ? "项目中心" : view === "messages" ? "聊天" : view === "codex" ? "Codex 控制台" : companyConsole.company.name}</h1>
                <p>{view === "codex"
                  ? "运行配置、CLI 能力与宿主机环境"
                  : view === "messages"
                      ? "Human、群组与 Agent 实时通信和审批"
                      : view === "projects"
                        ? "仓库、规则、资产、任务与项目记忆"
                        : "身份、凭证、组织架构与权限范围"}</p>
              </div>
              {view === "agents" ? (
                <button className="button primary" onClick={() => setShowAgentForm(true)}>
                  <Icon name="plus" /> 创建 Agent 账号
                </button>
              ) : null}
            </header>

            {view === "agents" ? (
              <OrganizationAgentCenter
                consoleData={companyConsole}
                humanUserId={session.user.id}
                token={session.token}
                onCredential={setCredential}
                onBatchCredentials={setBatchCredentials}
                onChanged={refreshCompany}
                onError={showError}
                onNotice={setNotice}
              />
            ) : null}
            {view === "projects" ? (
              <ProjectsView
                consoleData={companyConsole}
                token={session.token}
                onChanged={refreshCompany}
                onError={showError}
                onNotice={setNotice}
              />
            ) : null}
            {view === "codex" ? (
              <CodexControlCenter
                consoleData={companyConsole}
                token={session.token}
                realtimeEvent={realtimeEvent}
                onError={showError}
                onNotice={setNotice}
              />
            ) : null}
            {view === "messages" ? (
              <ChatCenter
                consoleData={companyConsole}
                humanUser={session.user}
                token={session.token}
                realtimeEvent={realtimeEvent}
                approvals={approvals}
                onReview={reviewApproval}
                onChanged={refreshCompany}
                onError={showError}
                onNotice={setNotice}
              />
            ) : null}
          </>
        )}
      </main>

      {showCompanyForm ? (
        <CreateCompanyDialog
          token={session.token}
          onClose={() => setShowCompanyForm(false)}
          onCreated={async (companyId) => {
            setShowCompanyForm(false);
            await loadCompanies(session.token, companyId);
          }}
          onError={showError}
        />
      ) : null}
      {showUserPreferences ? (
        <UserPreferencesDialog
          companyConsole={companyConsole}
          token={session.token}
          onChanged={refreshCompany}
          onError={showError}
          onNotice={setNotice}
          onClose={() => setShowUserPreferences(false)}
        />
      ) : null}
      {showAgentForm && companyConsole ? (
        <CreateAgentDialog
          company={companyConsole.company}
          orgUnits={companyConsole.org_units}
          agents={companyConsole.agents}
          professions={companyConsole.professions}
          skillLanguage={companyConsole.governance_policy.effective_settings.skill_language}
          token={session.token}
          onClose={() => setShowAgentForm(false)}
          onCreated={async (nextCredential) => {
            setShowAgentForm(false);
            setCredential(nextCredential);
            await refreshCompany();
          }}
          onError={showError}
        />
      ) : null}
      {credential ? <CredentialDialog credential={credential} onClose={() => setCredential(null)} /> : null}
      {batchCredentials ? <BatchCredentialDialog result={batchCredentials} onClose={() => setBatchCredentials(null)} /> : null}
      {popupApproval && companyConsole ? (
        <ApprovalDialog
          approval={popupApproval}
          agents={companyConsole.agents}
          onReview={reviewApproval}
          onError={showError}
          onClose={() => setDismissedApprovalIds((current) => new Set(current).add(popupApproval.id))}
        />
      ) : null}
    </div>
  );
}

function ProjectsView(props: {
  consoleData: CompanyConsole;
  token: string;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const canManage = ["owner", "admin"].includes(props.consoleData.human_membership.role);
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [showCreateProject, setShowCreateProject] = useState(false);
  const [activeTab, setActiveTab] = useState<"git" | "rule" | "assets" | "tasks" | "memories">("git");
  const [projectActionBusy, setProjectActionBusy] = useState(false);
  const selectedProject = props.consoleData.projects.find((project) => project.project.id === selectedProjectId) ?? null;
  const projectStats = {
    git: props.consoleData.projects.filter((project) => project.git).length,
    assets: props.consoleData.projects.reduce((total, project) => total + project.assets.length, 0),
    tasks: props.consoleData.projects.reduce((total, project) => total + project.tasks.length, 0),
  };
  const projectPagination = usePagination(props.consoleData.projects, 8, props.consoleData.company.id);

  function openProject(projectId: string, tab: "git" | "rule" | "assets" | "tasks" | "memories") {
    setSelectedProjectId(projectId);
    setActiveTab(tab);
  }

  async function setProjectPaused(project: CompanyProject, paused: boolean) {
    setProjectActionBusy(true);
    try {
      await api(`/api/v1/companies/${props.consoleData.company.id}/projects/${project.project.id}/${paused ? "pause" : "resume"}`, { method: "POST" }, props.token);
      await props.onChanged();
      props.onNotice(paused
        ? "项目已暂停：项目群、任务唤醒、资产维护和正在运行的项目 Agent 已停止。"
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
                <span>{projectStatusLabel(selectedProject.project.status)}</span>
                <span>{projectTypeLabel(selectedProject.project.project_type, props.consoleData.project_types, props.consoleData.governance_policy.effective_settings.skill_language)} · 识别置信度 {selectedProject.project.project_type_confidence}%</span>
                <span>{selectedProject.git ? `${selectedProject.git.git_host} · ${selectedProject.git.default_branch}` : "等待 Human 配置 Git"}</span>
              </div>
            </div>
            <div className="project-state-actions">
              <span className={`git-config-state ${selectedProject.git ? "configured" : ""}`}>
                {selectedProject.git ? "已配置 Git" : "未配置 Git"}
              </span>
              {canManage ? <button className={`button small ${projectPaused ? "primary" : "danger-outline"}`} type="button" disabled={projectActionBusy} onClick={() => void setProjectPaused(selectedProject, !projectPaused)}><Icon name={projectPaused ? "play" : "pause"} /> {projectActionBusy ? "处理中…" : projectPaused ? "恢复项目" : "暂停项目"}</button> : null}
            </div>
          </div>
          {projectPaused ? <div className="project-paused-banner"><Icon name="pause" /><span><strong>项目已暂停</strong><small>项目群不可发送消息，Agent 不会因本项目任务、消息或资产维护启动；正在运行的项目 Codex 会被取消。</small></span></div> : null}
          <div className="project-detail-tabs" role="tablist" aria-label="项目详情">
            <button className={activeTab === "git" ? "active" : ""} type="button" role="tab" aria-selected={activeTab === "git"} onClick={() => setActiveTab("git")}>Git 仓库</button>
            <button className={activeTab === "rule" ? "active" : ""} type="button" role="tab" aria-selected={activeTab === "rule"} onClick={() => setActiveTab("rule")}>Rule</button>
            <button className={activeTab === "assets" ? "active" : ""} type="button" role="tab" aria-selected={activeTab === "assets"} onClick={() => setActiveTab("assets")}>项目资产 <span>{selectedProject.assets.length}</span></button>
            <button className={activeTab === "tasks" ? "active" : ""} type="button" role="tab" aria-selected={activeTab === "tasks"} onClick={() => setActiveTab("tasks")}>项目任务 <span>{selectedProject.tasks.length}</span></button>
            <button className={activeTab === "memories" ? "active" : ""} type="button" role="tab" aria-selected={activeTab === "memories"} onClick={() => setActiveTab("memories")}>项目记忆</button>
          </div>
          {activeTab === "git" ? (
            <ProjectGitCard
              companyId={props.consoleData.company.id}
              project={selectedProject}
              token={props.token}
              canManage={canManage && !projectPaused}
              onChanged={props.onChanged}
              onError={props.onError}
              onNotice={props.onNotice}
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
                      {project.git ? `${project.git.git_host} · 默认分支 ${project.git.default_branch}${project.git.push_enabled ? " · Agent 可推送" : ""}` : "尚未关联仓库和宿主机目录"}
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

function CreateProjectDialog(props: {
  consoleData: CompanyConsole;
  token: string;
  onClose: () => void;
  onCreated: () => Promise<void>;
  onError: (error: unknown) => void;
}) {
  const activeAgents = props.consoleData.agents.filter((agent) => agent.membership.employment_status === "active");
  const preferredOwner = activeAgents.find((agent) => ["project_manager", "product_manager", "technical_manager"].includes(agent.profession?.key ?? companyAgentProfessionKey(agent, props.consoleData.professions))) ?? activeAgents[0];
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [ownerAgentId, setOwnerAgentId] = useState(preferredOwner?.agent_profile.id ?? "");
  const [memberAgentIds, setMemberAgentIds] = useState<string[]>(preferredOwner ? [preferredOwner.agent_profile.id] : []);
  const [projectType, setProjectType] = useState("");
  const [sourceKind, setSourceKind] = useState<"local_folder" | "git">("local_folder");
  const [selectedFolderName, setSelectedFolderName] = useState("");
  const [selectedFolderFiles, setSelectedFolderFiles] = useState<File[]>([]);
  const [gitRemoteUrl, setGitRemoteUrl] = useState("");
  const [defaultBranch, setDefaultBranch] = useState("main");
  const [busy, setBusy] = useState(false);
  const skillLanguage = props.consoleData.governance_policy.effective_settings.skill_language;
  const folderInputRef = useRef<HTMLInputElement | null>(null);
  const selectedType = props.consoleData.project_types.find((item) => item.key === projectType);
  const projectTypeGroups = useMemo(() => {
    const groups = new Map<string, CompanyProjectType[]>();
    props.consoleData.project_types.forEach((type) => {
      const categoryLabel = skillLanguage === "en" ? type.category_label_en : type.category_label;
      const current = groups.get(categoryLabel) ?? [];
      current.push(type);
      groups.set(categoryLabel, current);
    });
    return Array.from(groups.entries());
  }, [props.consoleData.project_types, skillLanguage]);

  function selectFolder(files: File[]) {
    const firstFile = files[0];
    if (!firstFile) return;
    const relativePath = firstFile.webkitRelativePath.replace(/\\/gu, "/");
    const rootName = relativePath.split("/")[0] || firstFile.name;
    setSelectedFolderName(rootName);
    setSelectedFolderFiles(files);
    if (!name.trim()) setName(rootName);
  }

  function toggleMember(agentId: string, checked: boolean) {
    setMemberAgentIds((current) => checked
      ? current.includes(agentId) ? current : [...current, agentId]
      : current.filter((id) => id !== agentId));
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (sourceKind === "local_folder" && !selectedFolderName) {
      props.onError(new Error("请先选择要导入的本地文件夹。"));
      return;
    }
    setBusy(true);
    try {
      const commonInput = {
        name,
        description: description || null,
        owner_agent_id: ownerAgentId,
        member_agent_ids: Array.from(new Set([ownerAgentId, ...memberAgentIds])),
        project_type: projectType || null,
      };
      if (sourceKind === "local_folder") {
        const excluded = new Set([".git", ".relay", ".relay-agent-trigger", "node_modules", "target"]);
        const uploadEntries = selectedFolderFiles.flatMap((file) => {
          const parts = file.webkitRelativePath.replace(/\\/gu, "/").split("/").filter(Boolean);
          const relativePath = (parts.length > 1 ? parts.slice(1) : [file.name]).join("/");
          return relativePath.split("/").some((part) => excluded.has(part)) ? [] : [{ file, relativePath }];
        });
        const form = new FormData();
        form.append("metadata", JSON.stringify({
          ...commonInput,
          file_paths: uploadEntries.map((entry) => entry.relativePath),
        }));
        uploadEntries.forEach((entry, index) => form.append(`file_${index}`, entry.file, entry.file.name));
        await api(`/api/v1/companies/${props.consoleData.company.id}/projects/import-folder`, {
          method: "POST",
          body: form,
        }, props.token);
      } else {
        await api(`/api/v1/companies/${props.consoleData.company.id}/projects`, {
          method: "POST",
          body: JSON.stringify({
            ...commonInput,
            source_kind: "git",
            source_local_path: null,
            git_remote_url: gitRemoteUrl,
            default_branch: defaultBranch,
          }),
        }, props.token);
      }
      await props.onCreated();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog title="新建项目" description="选择本地文件夹或 Git 仓库。Relay 会创建托管副本、识别项目类型，并生成不可被弱化的固定项目 Skill。" onClose={props.onClose} extraWide>
      <form className="stack-form create-project-form" onSubmit={submit}>
        <div className="project-source-switch" role="tablist" aria-label="项目来源">
          <button className={sourceKind === "local_folder" ? "active" : ""} type="button" onClick={() => setSourceKind("local_folder")}><Icon name="folder" /><span><strong>导入本地文件夹</strong><small>复制到组织托管空间，不修改原目录</small></span></button>
          <button className={sourceKind === "git" ? "active" : ""} type="button" onClick={() => setSourceKind("git")}><Icon name="git" /><span><strong>从 Git 创建</strong><small>填写仓库地址，由 Trigger 创建工作区</small></span></button>
        </div>
        {sourceKind === "local_folder" ? (
          <>
            <div className={`project-folder-picker ${selectedFolderName ? "selected" : ""}`}>
              <Icon name="folder" />
              <div><strong>{selectedFolderName || "选择要导入的项目文件夹"}</strong><small>{selectedFolderName ? `已选择 ${selectedFolderFiles.length} 个文件；创建时会流式复制到组织托管空间。` : "Relay 会忽略 .git、.relay、node_modules 和 target，并复制到组织默认空间。"}</small></div>
              <button className="button" type="button" onClick={() => folderInputRef.current?.click()}>选择文件夹</button>
              <input ref={(element) => { folderInputRef.current = element; element?.setAttribute("webkitdirectory", ""); }} className="hidden-file-input" type="file" multiple onChange={(event) => { selectFolder(Array.from(event.target.files ?? [])); event.target.value = ""; }} />
            </div>
          </>
        ) : (
          <div className="form-grid">
            <Field label="Git 地址"><input value={gitRemoteUrl} onChange={(event) => setGitRemoteUrl(event.target.value)} placeholder="https://github.com/org/repository.git" required /></Field>
            <Field label="默认分支"><input value={defaultBranch} onChange={(event) => setDefaultBranch(event.target.value)} placeholder="main" required /></Field>
          </div>
        )}
        <div className="form-grid">
          <Field label="项目名称"><input value={name} onChange={(event) => setName(event.target.value)} placeholder="项目名称" required /></Field>
          <Field label="项目负责人"><select value={ownerAgentId} onChange={(event) => { setOwnerAgentId(event.target.value); toggleMember(event.target.value, true); }} required><option value="">请选择 Agent</option>{activeAgents.map((agent) => <option key={agent.agent_profile.id} value={agent.agent_profile.id}>{agent.agent_profile.display_name} · {agent.profession?.label ?? agent.membership.job_title}</option>)}</select></Field>
        </div>
        <Field label="项目说明"><textarea value={description} onChange={(event) => setDescription(event.target.value)} placeholder="描述目标、用户、范围和期望交付；Relay 会用它辅助识别项目类型。" /></Field>
        <Field label="项目类型"><select value={projectType} onChange={(event) => setProjectType(event.target.value)}><option value="">自动识别（推荐）</option>{projectTypeGroups.map(([category, types]) => <optgroup label={category} key={category}>{types.map((type) => <option key={type.key} value={type.key}>{skillLanguage === "en" ? type.label_en : type.label} · {skillLanguage === "en" ? type.description_en : type.description}</option>)}</optgroup>)}</select></Field>
        <div className="project-type-preview">
          <span className="eyebrow">FIXED PROJECT SKILL</span>
          <strong>{selectedType ? skillLanguage === "en" ? selectedType.label_en : selectedType.label : "创建后自动识别"}</strong>
          <p>{selectedType ? `${skillLanguage === "en" ? selectedType.category_label_en : selectedType.category_label} · ${skillLanguage === "en" ? selectedType.description_en : selectedType.description}` : "Relay 会综合项目说明与文件结构选择类型；Human 仍可在创建前明确指定。"}</p>
          {selectedType ? <pre>{skillLanguage === "en" ? selectedType.rule_markdown_en : selectedType.rule_markdown}</pre> : null}
        </div>
        <div className="project-member-selector">
          <strong>项目成员</strong><small>负责人会自动加入；其他成员可在这里一并加入项目群。</small>
          <div className="permission-grid">{activeAgents.map((agent) => <label className="check-row" key={agent.agent_profile.id}><input type="checkbox" checked={agent.agent_profile.id === ownerAgentId || memberAgentIds.includes(agent.agent_profile.id)} disabled={agent.agent_profile.id === ownerAgentId} onChange={(event) => toggleMember(agent.agent_profile.id, event.target.checked)} /><span>{agent.agent_profile.display_name} · {agent.profession?.label ?? agent.membership.job_title}</span></label>)}</div>
        </div>
        {!activeAgents.length ? <div className="inline-error">请先创建并激活至少一个 Agent，项目需要一个 Agent 负责人。</div> : null}
        <div className="dialog-actions"><button className="button" type="button" onClick={props.onClose} disabled={busy}>取消</button><button className="button primary" disabled={busy || !ownerAgentId || !name.trim() || (sourceKind === "git" ? !gitRemoteUrl.trim() : !selectedFolderName)}>{busy ? "正在创建托管项目…" : "创建项目"}</button></div>
      </form>
    </Dialog>
  );
}

function TasksView(props: {
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
                .map((dependency) => project.tasks.find((candidate) => candidate.id === dependency.depends_on_task_id))
                .filter((dependency): dependency is CompanyProjectTask => Boolean(dependency));
              const unresolvedDependencies = dependencies.filter((dependency) => !["done", "cancelled"].includes(dependency.status));
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
                    {unresolvedDependencies.length ? <em className="task-waiting-dependencies">等待：{unresolvedDependencies.map((dependency) => dependency.title).join("、")}</em> : null}
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
        />
      ) : null}
    </div>
  );
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

function ProjectAssetStatusBadge({ status }: { status: CompanyProject["assets"][number]["status"] }) {
  const label = { active: "正常", missing: "缺失", deprecated: "已废弃", unknown: "待确认" }[status];
  return <span className={`project-asset-status ${status}`}><span className="status-dot" />{label}</span>;
}

function TaskDialog(props: {
  companyId: string;
  projects: CompanyProject[];
  task?: CompanyProjectTask;
  token: string;
  onClose: () => void;
  onSaved: (task: CompanyProjectTask) => Promise<void>;
  onError: (error: unknown) => void;
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
  const unresolvedDependencies = selectedDependencies.filter((dependency) => !["done", "cancelled"].includes(dependency.status));
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
        {!activeMembers.length ? <div className="git-security-note">这个项目还没有活跃成员。你可以先保存为未分配任务，或让有权限的 Agent 添加项目成员。</div> : null}
        <div className="dialog-actions">
          <button className="button" type="button" onClick={props.onClose} disabled={busy}>取消</button>
          <button className="button primary" disabled={busy || !projectId || statusBlockedByDependencies}>{busy ? "正在保存…" : props.task ? "保存修改" : "创建任务"}</button>
        </div>
      </form>
    </Dialog>
  );
}

function ProjectRuleCard(props: {
  companyId: string;
  project: CompanyProject;
  projectTypes: CompanyProjectType[];
  agents: CompanyAgent[];
  token: string;
  canManage: boolean;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const projectAgents = useMemo(() => activeProjectAgents(props.project, props.agents), [props.project.members, props.agents]);
  const [content, setContent] = useState(props.project.rule?.content ?? "");
  const preferredAgentId = preferredProjectRuleAgentId(props.project, projectAgents);
  const [agentId, setAgentId] = useState(preferredAgentId);
  const [instructions, setInstructions] = useState("");
  const [busy, setBusy] = useState(false);
  const selectedAgent = projectAgents.find((agent) => agent.agent_profile.id === agentId);
  const selectedAgentNeedsPermission = Boolean(selectedAgent && !agentHasPermission(selectedAgent, "project.rules.manage"));
  const systemType = props.projectTypes.find((type) => type.key === props.project.project.project_type);

  useEffect(() => setContent(props.project.rule?.content ?? ""), [props.project.project.id, props.project.rule?.updated_at]);
  useEffect(() => {
    if (!projectAgents.some((agent) => agent.agent_profile.id === agentId)) {
      setAgentId(preferredAgentId);
    }
  }, [agentId, preferredAgentId, projectAgents]);

  async function saveRule(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      await api(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/rule`,
        { method: "PUT", body: JSON.stringify({ content }) },
        props.token,
      );
      props.onNotice("项目 Rule 已保存");
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function requestGeneration(event: FormEvent) {
    event.preventDefault();
    if (!agentId) return;
    setBusy(true);
    try {
      if (selectedAgentNeedsPermission && selectedAgent) {
        await grantAgentProjectPermission(props.companyId, selectedAgent, "project.rules.manage", props.token);
      }
      await api(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/rule/generate`,
        { method: "POST", body: JSON.stringify({ agent_id: agentId, instructions: instructions || null }) },
        props.token,
      );
      setInstructions("");
      props.onNotice(selectedAgentNeedsPermission ? "已授权并即时唤醒 Agent 生成或更新 Rule" : "已交给授权 Agent；消息将即时唤醒它生成或更新 Rule");
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="project-rule-layout">
      <section className="project-system-rule-card">
        <div className="project-tab-heading">
          <div><span className="eyebrow">SYSTEM PROJECT SKILL</span><h3>{systemType?.label ?? "通用项目"}固定规则</h3><p>这是 Relay 按项目类型自动加载的强制基线。Human Rule 和 Agent 生成内容只能补充，不能删除或弱化。</p></div>
          <span className="pill neutral">自动加载</span>
        </div>
        <pre>{systemType?.rule_markdown ?? "项目类型规则暂不可用"}</pre>
      </section>
      <form className="project-rule-editor" onSubmit={saveRule}>
        <div className="project-tab-heading">
          <div><span className="eyebrow">HUMAN PROJECT RULE</span><h3>项目补充注意事项</h3><p>这里记录项目特有约束、风险和协作规则，并与上方系统固定规则一起进入 Agent Skill。</p></div>
          {props.project.rule ? <small>更新于 {formatTime(props.project.rule.updated_at)}</small> : <small>尚未创建</small>}
        </div>
        <textarea
          className="project-rule-textarea"
          value={content}
          onChange={(event) => setContent(event.target.value)}
          placeholder={"# 项目规则\n\n- 修改前先运行测试\n- 不要提交密钥或本地配置\n- 默认分支禁止直接推送"}
          disabled={!props.canManage || busy}
        />
        <div className="project-tab-actions">
          <small>最多 50,000 个字符；Human 始终可以直接维护。</small>
          {props.canManage ? <button className="button primary" disabled={busy}>{busy ? "保存中…" : "保存 Rule"}</button> : null}
        </div>
      </form>
      <form className="project-delegation-card" onSubmit={requestGeneration}>
        <span className="eyebrow">AGENT DELEGATION</span>
        <h4>授权 Agent 生成</h4>
        <p>这里列出本项目的活跃 Agent；选择尚未授权的 Agent 后，提交时会一并授予 <code>project.rules.manage</code>。</p>
        <Field label="执行 Agent">
          <select value={agentId} onChange={(event) => setAgentId(event.target.value)} disabled={!props.canManage || busy}>
            <option value="">请选择 Agent</option>
            {projectAgents.map((agent) => <option key={agent.agent_profile.id} value={agent.agent_profile.id}>{agent.agent_profile.display_name}{agentHasPermission(agent, "project.rules.manage") ? "" : "（提交时授权）"}</option>)}
          </select>
        </Field>
        <Field label="补充要求（可选）">
          <textarea value={instructions} onChange={(event) => setInstructions(event.target.value)} placeholder="例如：结合仓库现状补充测试、发布和安全约束" disabled={!props.canManage || busy} />
        </Field>
        {!projectAgents.length ? <div className="git-security-note">这个项目还没有活跃 Agent，请先在项目中添加成员。</div> : selectedAgentNeedsPermission ? <div className="git-security-note">提交后将授予所选 Agent 的 Rule 维护权限。</div> : null}
        {props.canManage ? <button className="button primary wide" disabled={!agentId || busy}>{busy ? "提交中…" : selectedAgentNeedsPermission ? "授权并交给 Agent" : "交给 Agent 生成"}</button> : null}
      </form>
    </div>
  );
}

function ProjectAssetsCard(props: {
  companyId: string;
  project: CompanyProject;
  agents: CompanyAgent[];
  token: string;
  canManage: boolean;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const projectAgents = useMemo(() => activeProjectAgents(props.project, props.agents), [props.project.members, props.agents]);
  const [agentId, setAgentId] = useState(props.project.asset_refresh?.maintainer_agent_id ?? projectAgents[0]?.agent_profile.id ?? "");
  const [intervalMinutes, setIntervalMinutes] = useState(props.project.asset_refresh?.interval_minutes ?? 1440);
  const [enabled, setEnabled] = useState(props.project.asset_refresh?.enabled ?? true);
  const [busy, setBusy] = useState(false);
  const [reloading, setReloading] = useState(false);
  const [search, setSearch] = useState("");
  const [typeFilter, setTypeFilter] = useState("");
  const [statusFilter, setStatusFilter] = useState("");
  const maintainer = props.agents.find((agent) => agent.agent_profile.id === props.project.asset_refresh?.maintainer_agent_id);
  const selectedAgent = projectAgents.find((agent) => agent.agent_profile.id === agentId);
  const selectedAgentNeedsPermission = Boolean(selectedAgent && !agentHasPermission(selectedAgent, "project.assets.manage"));
  const assetTypes = Array.from(new Set(props.project.assets.map((asset) => asset.asset_type))).sort();
  const filteredAssets = props.project.assets.filter((asset) => {
    const normalizedSearch = search.trim().toLocaleLowerCase();
    return (!normalizedSearch
      || asset.name.toLocaleLowerCase().includes(normalizedSearch)
      || asset.description.toLocaleLowerCase().includes(normalizedSearch)
      || asset.locator.toLocaleLowerCase().includes(normalizedSearch))
      && (!typeFilter || asset.asset_type === typeFilter)
      && (!statusFilter || asset.status === statusFilter);
  });
  const assetPagination = usePagination(filteredAssets, 9, `${props.project.project.id}:${search}:${typeFilter}:${statusFilter}`);
  const activeAssetCount = props.project.assets.filter((asset) => asset.status === "active").length;
  const lastAssetUpdate = props.project.assets.reduce<string | null>((latest, asset) => !latest || new Date(asset.updated_at) > new Date(latest) ? asset.updated_at : latest, null);

  useEffect(() => {
    setAgentId(props.project.asset_refresh?.maintainer_agent_id ?? projectAgents[0]?.agent_profile.id ?? "");
    setIntervalMinutes(props.project.asset_refresh?.interval_minutes ?? 1440);
    setEnabled(props.project.asset_refresh?.enabled ?? true);
  }, [props.project.project.id, props.project.asset_refresh?.updated_at]);

  async function saveRefresh(runNow: boolean) {
    if (!agentId) return;
    setBusy(true);
    try {
      if (selectedAgentNeedsPermission && selectedAgent) {
        await grantAgentProjectPermission(props.companyId, selectedAgent, "project.assets.manage", props.token);
      }
      await api(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/asset-refresh`,
        {
          method: "PUT",
          body: JSON.stringify({
            maintainer_agent_id: agentId,
            interval_minutes: intervalMinutes,
            enabled,
            run_now: runNow,
          }),
        },
        props.token,
      );
      props.onNotice(
        selectedAgentNeedsPermission
          ? runNow ? "已授权 Agent，资产维护任务已安排到下一次唤醒" : "已授权 Agent 并保存项目资产更新计划"
          : runNow ? "资产维护任务已安排到下一次 Agent 唤醒" : "项目资产更新计划已保存",
      );
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function reloadAssets() {
    setReloading(true);
    try {
      await props.onChanged();
      props.onNotice("项目资产显示已刷新");
    } catch (error) {
      props.onError(error);
    } finally {
      setReloading(false);
    }
  }

  return (
    <div className="project-assets-layout">
      <section className="project-assets-main">
        <div className="project-tab-heading">
          <div><span className="eyebrow">PROJECT ASSETS</span><h3>项目资产清单</h3><p>由授权 Agent 扫描真实工作区后维护。这里展示代码模块、文档、接口、配置和数据文件的真实位置。</p></div>
          <div className="project-assets-heading-actions">
            <span className="count-badge">{props.project.assets.length}</span>
            <button className="button small" type="button" onClick={() => void reloadAssets()} disabled={reloading}><Icon name="refresh" /> {reloading ? "刷新中…" : "刷新显示"}</button>
          </div>
        </div>
        <div className="project-asset-summary">
          <span><small>全部资产</small><strong>{props.project.assets.length}</strong></span>
          <span><small>正常可用</small><strong>{activeAssetCount}</strong></span>
          <span><small>资产类型</small><strong>{assetTypes.length}</strong></span>
          <span><small>最近更新</small><strong>{lastAssetUpdate ? formatTime(lastAssetUpdate) : "尚未生成"}</strong></span>
        </div>
        {props.project.assets.length ? (
          <>
            <div className="project-asset-filters">
              <label className="task-search"><Icon name="search" /><input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="搜索资产名称、说明或路径" /></label>
              <select value={typeFilter} onChange={(event) => setTypeFilter(event.target.value)}><option value="">全部类型</option>{assetTypes.map((assetType) => <option key={assetType} value={assetType}>{projectAssetTypeLabel(assetType)}</option>)}</select>
              <select value={statusFilter} onChange={(event) => setStatusFilter(event.target.value)}><option value="">全部状态</option><option value="active">正常</option><option value="missing">缺失</option><option value="deprecated">已废弃</option><option value="unknown">待确认</option></select>
            </div>
            {filteredAssets.length ? <div className="project-asset-grid">
              {assetPagination.pageItems.map((asset) => {
                const updater = props.agents.find((agent) => agent.agent_profile.id === asset.updated_by_agent_id);
                return <article className="project-asset-card" key={asset.id}>
                  <div className="project-asset-card-head">
                    <span className="project-asset-icon"><Icon name="folder" /></span>
                    <span><strong>{asset.name}</strong><small>{projectAssetTypeLabel(asset.asset_type)}</small></span>
                    <ProjectAssetStatusBadge status={asset.status} />
                  </div>
                  <p>{asset.description || "这个资产暂时没有补充说明。"}</p>
                  <div className="project-asset-locator"><small>项目内位置</small><code>{asset.locator}</code></div>
                  <footer><span>{updater ? `${updater.agent_profile.display_name} 更新` : "资产清单"}</span><time>{formatTime(asset.updated_at)}</time></footer>
                </article>;
              })}
              <Pagination {...assetPagination} onPageChange={assetPagination.setPage} />
            </div> : <div className="project-assets-empty compact"><Icon name="search" /><strong>没有符合条件的资产</strong><p>调整搜索词、类型或状态筛选后再试。</p></div>}
          </>
        ) : (
          <div className="project-assets-empty">
            <Icon name="folder" />
            <strong>资产清单尚未生成</strong>
            <p>右侧选择项目 Agent，点击“授权并立即更新”。Agent 会扫描项目真实工作区并通过 MCP 回写资产。</p>
            <div className="project-assets-empty-steps"><span><b>1</b>选择维护 Agent</span><span><b>2</b>立即唤醒扫描</span><span><b>3</b>回到这里查看结果</span></div>
          </div>
        )}
      </section>
      <aside className="project-delegation-card asset-refresh-card">
        <span className="eyebrow">PERIODIC REFRESH</span>
        <h4>定期维护设置</h4>
        <p>Trigger 到期后只负责唤醒指定 Agent；这里可直接选择项目成员，保存时会为尚未授权的 Agent 授予资产维护权限。</p>
        <Field label="维护 Agent">
          <select value={agentId} onChange={(event) => setAgentId(event.target.value)} disabled={!props.canManage || busy}>
            <option value="">请选择 Agent</option>
            {projectAgents.map((agent) => <option key={agent.agent_profile.id} value={agent.agent_profile.id}>{agent.agent_profile.display_name}{agentHasPermission(agent, "project.assets.manage") ? "" : "（保存时授权）"}</option>)}
          </select>
        </Field>
        <Field label="更新周期">
          <select value={intervalMinutes} onChange={(event) => setIntervalMinutes(Number(event.target.value))} disabled={!props.canManage || busy}>
            <option value={60}>每小时</option>
            <option value={360}>每 6 小时</option>
            <option value={720}>每 12 小时</option>
            <option value={1440}>每天</option>
            <option value={4320}>每 3 天</option>
            <option value={10080}>每周</option>
          </select>
        </Field>
        <label className="check-row"><input type="checkbox" checked={enabled} onChange={(event) => setEnabled(event.target.checked)} disabled={!props.canManage || busy} /><span>启用定期更新</span></label>
        {props.project.asset_refresh ? (
          <div className="asset-refresh-status">
            <strong>{props.project.asset_refresh.last_completed_at ? "资产维护已运行" : props.project.asset_refresh.last_requested_at ? "已唤醒，等待 Agent 回写" : "维护计划已保存"}</strong>
            <span>当前维护人：{maintainer?.agent_profile.display_name ?? "未知 Agent"}</span>
            <span>下次检查：{formatTime(props.project.asset_refresh.next_refresh_at)}</span>
            <span>最近完成：{props.project.asset_refresh.last_completed_at ? formatTime(props.project.asset_refresh.last_completed_at) : "尚未完成"}</span>
          </div>
        ) : null}
        {!projectAgents.length ? <div className="git-security-note">这个项目还没有活跃 Agent，请先在项目中添加成员。</div> : selectedAgentNeedsPermission ? <div className="git-security-note">保存后将授予所选 Agent 的项目资产维护权限。</div> : null}
        {props.canManage ? (
          <div className="project-asset-actions">
            <button className="button" type="button" onClick={() => void saveRefresh(false)} disabled={!agentId || busy}>{selectedAgentNeedsPermission ? "授权并保存计划" : "保存计划"}</button>
            <button className="button primary" type="button" onClick={() => void saveRefresh(true)} disabled={!agentId || !enabled || busy}>{busy ? "提交中…" : selectedAgentNeedsPermission ? "授权并立即更新" : "保存并立即更新"}</button>
          </div>
        ) : null}
      </aside>
    </div>
  );
}

function activeProjectAgents(project: CompanyProject, agents: CompanyAgent[]) {
  const projectMemberIds = new Set(project.members.map((member) => member.agent_profile.id));
  return agents.filter((agent) =>
    projectMemberIds.has(agent.agent_profile.id)
    && agent.membership.employment_status === "active");
}

function preferredProjectRuleAgentId(project: CompanyProject, projectAgents: CompanyAgent[]) {
  const lastRuleAgentId = project.rule?.updated_by_agent_id;
  if (lastRuleAgentId && projectAgents.some((agent) => agent.agent_profile.id === lastRuleAgentId)) {
    return lastRuleAgentId;
  }
  return projectAgents.find((agent) => agentHasPermission(agent, "project.rules.manage"))?.agent_profile.id
    ?? projectAgents[0]?.agent_profile.id
    ?? "";
}

function agentHasPermission(agent: CompanyAgent, permission: string) {
  return agent.membership.permissions.includes(permission);
}

async function grantAgentProjectPermission(companyId: string, agent: CompanyAgent, permission: string, token: string) {
  const staffingPermissions = agent.membership.permissions.filter((current) => STAFFING_PERMISSIONS.some((item) => item.key === current));
  const projectPermissions = agent.membership.permissions.filter((current) => PROJECT_PERMISSIONS.some((item) => item.key === current));
  if (!projectPermissions.includes(permission)) projectPermissions.push(permission);
  await api(
    `/api/v1/companies/${companyId}/agents/${agent.agent_profile.id}/permissions`,
    {
      method: "POST",
      body: JSON.stringify({
        staffing_permissions: staffingPermissions,
        project_permissions: projectPermissions,
        staffing_scope_org_unit_id: staffingPermissions.length ? agent.membership.staffing_scope_org_unit_id : null,
        reason: `Human console project delegation: grant ${permission}`,
      }),
    },
    token,
  );
}

function ProjectGitCard(props: {
  companyId: string;
  project: CompanyProject;
  token: string;
  canManage: boolean;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [remoteUrl, setRemoteUrl] = useState(props.project.git?.remote_url ?? "");
  const [hostLocalPath, setHostLocalPath] = useState("");
  const [defaultBranch, setDefaultBranch] = useState(props.project.git?.default_branch ?? "main");
  const [githubToken, setGithubToken] = useState("");
  const [githubTokenConfigured, setGithubTokenConfigured] = useState(false);
  const [managedTokenConfigured, setManagedTokenConfigured] = useState(false);
  const [branchPrefix, setBranchPrefix] = useState(props.project.git?.branch_prefix ?? "relay/");
  const [allowAgentPush, setAllowAgentPush] = useState(props.project.git?.push_enabled ?? false);
  const [configured, setConfigured] = useState(Boolean(props.project.git));
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!props.canManage) return;
    let active = true;
    api<{ git: ProjectGitAdminView | null; github_token_configured: boolean; managed_token_configured: boolean }>(
      `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/git`,
      {},
      props.token,
    )
      .then(({ git, github_token_configured, managed_token_configured }) => {
        if (!active) return;
        setRemoteUrl(git?.remote_url ?? "");
        setHostLocalPath(git?.host_local_path ?? "");
        setDefaultBranch(git?.default_branch ?? "main");
        setGithubToken("");
        setGithubTokenConfigured(github_token_configured);
        setManagedTokenConfigured(managed_token_configured);
        setBranchPrefix(git?.branch_prefix ?? "relay/");
        setAllowAgentPush(git?.allow_agent_push ?? false);
        setConfigured(Boolean(git));
      })
      .catch(props.onError);
    return () => { active = false; };
  }, [props.canManage, props.companyId, props.project.project.id, props.token]);

  async function saveGit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      const response = await api<{ git: ProjectGitAdminView; github_token_configured: boolean; managed_token_configured: boolean }>(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/git`,
        {
          method: "PUT",
          body: JSON.stringify({
            remote_url: remoteUrl,
            host_local_path: hostLocalPath || null,
            default_branch: defaultBranch || null,
            github_token: githubToken || null,
            clear_github_token: false,
            allow_agent_push: allowAgentPush,
            branch_prefix: branchPrefix || null,
          }),
        },
        props.token,
      );
      setRemoteUrl(response.git.remote_url);
      setHostLocalPath(response.git.host_local_path);
      setDefaultBranch(response.git.default_branch);
      setGithubToken("");
      setGithubTokenConfigured(response.github_token_configured);
      setManagedTokenConfigured(response.managed_token_configured);
      setBranchPrefix(response.git.branch_prefix);
      setAllowAgentPush(response.git.allow_agent_push);
      setConfigured(true);
      props.onNotice(`${props.project.project.name} 的 Git 配置已保存`);
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function clearGithubToken() {
    if (!window.confirm("清除这个项目已保存的 GitHub Token？公开仓库仍可拉取，但私有仓库和 Push 将不可用。")) return;
    setBusy(true);
    try {
      const response = await api<{ git: ProjectGitAdminView; github_token_configured: boolean; managed_token_configured: boolean }>(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/git`,
        {
          method: "PUT",
          body: JSON.stringify({
            remote_url: remoteUrl,
            host_local_path: hostLocalPath || null,
            default_branch: defaultBranch || null,
            github_token: null,
            clear_github_token: true,
            allow_agent_push: allowAgentPush,
            branch_prefix: branchPrefix || null,
          }),
        },
        props.token,
      );
      setGithubToken("");
      setGithubTokenConfigured(response.github_token_configured);
      setManagedTokenConfigured(response.managed_token_configured);
      props.onNotice(`${props.project.project.name} 的 GitHub Token 已清除`);
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function clearGit() {
    if (!window.confirm(`清除 ${props.project.project.name} 的 Git 配置？Agent 将无法再从 Relay 获取仓库地址。`)) return;
    setBusy(true);
    try {
      await api(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/git`,
        { method: "DELETE" },
        props.token,
      );
      setRemoteUrl("");
      setHostLocalPath("");
      setDefaultBranch("main");
      setGithubToken("");
      setGithubTokenConfigured(false);
      setManagedTokenConfigured(false);
      setBranchPrefix("relay/");
      setAllowAgentPush(false);
      setConfigured(false);
      props.onNotice(`${props.project.project.name} 的 Git 配置已清除`);
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <article className="project-git-editor">
      {props.canManage ? (
        <form className="project-git-form" onSubmit={saveGit}>
          <Field label="Git Remote URL">
            <input
              value={remoteUrl}
              onChange={(event) => setRemoteUrl(event.target.value)}
              placeholder="https://github.com/owner/repository.git"
              required
            />
          </Field>
          <div className="managed-workspace-note">
            <div>
              <strong>Agent 工作区由 Relay 自动管理</strong>
              <span>默认会按项目生成独立目录，你只需要填写 Git 地址。</span>
            </div>
            {configured && hostLocalPath ? <code>{hostLocalPath}</code> : null}
          </div>
          <details className="project-git-advanced">
            <summary>高级设置：自定义宿主机目录</summary>
            <Field label="宿主机项目根目录">
              <input
                value={hostLocalPath}
                onChange={(event) => setHostLocalPath(event.target.value)}
                placeholder="留空时由 Relay 自动生成"
              />
            </Field>
            <small>仅当 Codex 必须使用指定的现有目录时才需要设置。修改已有目录不会自动搬迁旧工作区。</small>
          </details>
          <div className="form-grid">
            <Field label="默认分支">
              <input value={defaultBranch} onChange={(event) => setDefaultBranch(event.target.value)} placeholder="main" />
            </Field>
            <Field label="Agent 分支前缀">
              <input value={branchPrefix} onChange={(event) => setBranchPrefix(event.target.value)} placeholder="relay/" />
            </Field>
          </div>
          <Field label={managedTokenConfigured ? "Git Token（Relay 已自动配置）" : "GitHub Token（私有仓库或需要 Push 时填写）"}>
            <input
              type="password"
              autoComplete="new-password"
              value={githubToken}
              onChange={(event) => setGithubToken(event.target.value)}
              placeholder={managedTokenConfigured ? "托管凭证已就绪；填写新 Token 可手动覆盖" : githubTokenConfigured ? "Token 已保存，留空表示不修改" : "github_pat_..."}
            />
          </Field>
          {managedTokenConfigured ? (
            <div className="project-git-actions">
              <span className="git-security-note">项目专用 Token 已由 Relay 自动创建并保存在宿主机</span>
            </div>
          ) : githubTokenConfigured ? (
            <div className="project-git-actions">
              <span className="git-security-note">Token 已安全保存在宿主机</span>
              <button className="button small" type="button" onClick={() => void clearGithubToken()} disabled={busy}>清除 Token</button>
            </div>
          ) : null}
          <label className="check-row">
            <input type="checkbox" checked={allowAgentPush} onChange={(event) => setAllowAgentPush(event.target.checked)} />
            允许 Codex 在完成验证后 push 自己的工作分支
          </label>
          <div className="git-security-note">
            Git 地址请使用 HTTPS。Token 只保存在宿主机本地凭证目录，不写入项目数据库，也不会通过 MCP 返回；配置托管代码平台后，Agent 可以自行创建仓库与项目专用 Token。
          </div>
          <div className="project-git-actions">
            {configured ? <button className="button small" type="button" onClick={() => void clearGit()} disabled={busy}>清除配置</button> : null}
            <button className="button primary small" disabled={busy}>{busy ? "正在保存…" : "保存 Git 配置"}</button>
          </div>
        </form>
      ) : (
        <div className="project-git-readonly">
          {props.project.git ? (
            <>
              <code>{props.project.git.remote_url}</code>
              <span>默认分支 {props.project.git.default_branch}</span>
            </>
          ) : <span>请联系公司 Owner/Admin 配置仓库。</span>}
        </div>
      )}
    </article>
  );
}
