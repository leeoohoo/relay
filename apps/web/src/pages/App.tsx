import { FormEvent, ReactNode, useEffect, useMemo, useRef, useState } from "react";
import { API_BASE_URL, api } from "../api/client";
import type { CompanyRealtimeEvent } from "../api/types";
import { AuthScreen } from "../components/AuthScreen";
import { CodexCliSettingsView } from "../components/CodexCliSettingsView";
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
  CodexMcpEnvironmentSnapshot,
  CodexMcpServer,
  CodexPersonality,
  CodexPluginCatalog,
  CodexPluginOperation,
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
  codexPluginOperationStatusLabel,
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
  OrganizationView,
  UserPreferencesDialog,
} from "./app/organization";
import { PROJECT_PERMISSIONS, STAFFING_PERMISSIONS } from "./app/permissions";

const SESSION_KEY = "agent_company_session";
const CODEX_DEFAULT_OPENAI_BASE_URL = "https://api.openai.com/v1";

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

function OrganizationAgentCenter(props: {
  consoleData: CompanyConsole;
  humanUserId: string;
  token: string;
  onCredential: (credential: Credential) => void;
  onBatchCredentials: (result: BatchCredentialResult) => void;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [tab, setTab] = useState<"agents" | "organization">("agents");

  return (
    <div className="control-center organization-agent-center">
      <nav className="control-center-tabs two-tabs" role="tablist" aria-label="组织与 Agent">
        <button
          className={tab === "agents" ? "active" : ""}
          type="button"
          role="tab"
          aria-selected={tab === "agents"}
          onClick={() => setTab("agents")}
        >
          <span className="control-center-tab-icon"><Icon name="key" /></span>
          <span><strong>Agent 成员</strong><small>身份、凭证、记忆与个人权限</small></span>
        </button>
        <button
          className={tab === "organization" ? "active" : ""}
          type="button"
          role="tab"
          aria-selected={tab === "organization"}
          onClick={() => setTab("organization")}
        >
          <span className="control-center-tab-icon"><Icon name="org" /></span>
          <span><strong>组织架构</strong><small>组织节点、归属与权限范围</small></span>
        </button>
      </nav>

      <div className="control-center-panel" role="tabpanel">
        {tab === "agents" ? (
          <AgentsView
            consoleData={props.consoleData}
            humanUserId={props.humanUserId}
            token={props.token}
            onCredential={props.onCredential}
            onBatchCredentials={props.onBatchCredentials}
            onChanged={props.onChanged}
            onError={props.onError}
            onNotice={props.onNotice}
          />
        ) : null}
        {tab === "organization" ? (
          <OrganizationView
            companyId={props.consoleData.company.id}
            orgUnits={props.consoleData.org_units}
            agents={props.consoleData.agents}
            managedWorkspaceRoot={props.consoleData.governance_policy.effective_settings.managed_workspace_root}
            token={props.token}
            onChanged={props.onChanged}
            onError={props.onError}
            onNotice={props.onNotice}
          />
        ) : null}
      </div>
    </div>
  );
}

function AgentsView(props: {
  consoleData: CompanyConsole;
  humanUserId: string;
  token: string;
  onCredential: (credential: Credential) => void;
  onBatchCredentials: (result: BatchCredentialResult) => void;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const activeAgents = props.consoleData.agents.filter((agent) => agent.membership.employment_status === "active");
  const connectedAgents = activeAgents.filter((agent) => agent.connection.status === "connected");
  const provisioningAgents = props.consoleData.agents.filter((agent) => agent.membership.employment_status === "provisioning");
  const [batchBusy, setBatchBusy] = useState(false);
  const agentPagination = usePagination(props.consoleData.agents, 8, props.consoleData.company.id);

  async function activateAllProvisioningAgents() {
    if (!window.confirm(`依次激活 ${provisioningAgents.length} 个 Agent 并签发一次性 Key？`)) return;
    setBatchBusy(true);
    const credentials: Credential[] = [];
    const failures: BatchCredentialResult["failures"] = [];
    for (const agent of provisioningAgents) {
      try {
        const response = await api<{ result: { membership: AgentMembership; agent_key_plaintext: string; agent_key_prefix: string } }>(
          `/api/v1/companies/${props.consoleData.company.id}/agents/${agent.agent_profile.id}/activate`,
          { method: "POST", body: JSON.stringify({ reason: "Human console: batch activate" }) },
          props.token,
        );
        credentials.push({
          agent: agent.agent_profile,
          key: response.result.agent_key_plaintext,
          keyPrefix: response.result.agent_key_prefix,
          permissions: response.result.membership.permissions,
          professionKey: companyAgentProfessionKey(agent, props.consoleData.professions),
          profession: props.consoleData.professions.find((profession) => profession.key === companyAgentProfessionKey(agent, props.consoleData.professions)),
          skillLanguage: props.consoleData.governance_policy.effective_settings.skill_language,
        });
      } catch (error) {
        failures.push({
          agentName: agent.agent_profile.display_name,
          message: error instanceof Error ? error.message : "激活失败",
        });
      }
    }
    if (credentials.length) {
      props.onBatchCredentials({ credentials, failures });
    } else {
      props.onError(new Error(failures.map((failure) => `${failure.agentName}: ${failure.message}`).join("；") || "批量激活失败"));
    }
    try {
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBatchBusy(false);
    }
  }
  return (
    <div className="content-stack">
      <section className="metric-row">
        <Metric label="Agent 账号" value={String(props.consoleData.agents.length)} detail={`${connectedAgents.length} 个已连接，${Math.max(0, activeAgents.length - connectedAgents.length)} 个待连接`} />
        <Metric label="组织节点" value={String(props.consoleData.org_units.length)} detail="用于身份与授权范围" />
        <Metric label="公司会话" value={String(props.consoleData.conversations.length)} detail="私聊、群聊与项目群" />
        <Metric label="正式项目" value={String(props.consoleData.projects.length)} detail="Human 创建，Agent 协作维护" />
      </section>

      <section className="section-card">
        <div className="section-heading">
          <div><span className="eyebrow">IDENTITIES</span><h2>Agent 账号</h2></div>
          <div className="section-heading-actions">
            {provisioningAgents.length ? <button className="button small primary" onClick={() => void activateAllProvisioningAgents()} disabled={batchBusy}>{batchBusy ? "正在依次激活…" : `批量激活 ${provisioningAgents.length} 个`}</button> : null}
            <span className="count-badge">{props.consoleData.agents.length}</span>
          </div>
        </div>
        {props.consoleData.agents.length ? (
          <div className="agent-list">
            {agentPagination.pageItems.map((agent) => (
              <AgentRow key={agent.agent_profile.id} agent={agent} {...props} />
            ))}
            <Pagination {...agentPagination} onPageChange={agentPagination.setPage} />
          </div>
        ) : (
          <div className="empty-inline"><Icon name="key" /><h3>还没有 Agent 账号</h3><p>点击右上角创建第一个账号，系统会签发一次性 Key。</p></div>
        )}
      </section>
    </div>
  );
}

function AgentRow(props: {
  agent: CompanyAgent;
  consoleData: CompanyConsole;
  humanUserId: string;
  token: string;
  onCredential: (credential: Credential) => void;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const currentProfessionKey = companyAgentProfessionKey(props.agent, props.consoleData.professions);
  const skillLanguage = props.consoleData.governance_policy.effective_settings.skill_language;
  const professionGroups = useMemo(() => {
    const groups = new Map<string, CompanyProfession[]>();
    props.consoleData.professions.forEach((profession) => {
      const category = skillLanguage === "en" ? profession.category_label_en : profession.category_label;
      groups.set(category, [...(groups.get(category) ?? []), profession]);
    });
    return Array.from(groups.entries());
  }, [props.consoleData.professions, skillLanguage]);
  const [expanded, setExpanded] = useState(false);
  const [showMemories, setShowMemories] = useState(false);
  const [permissions, setPermissions] = useState(props.agent.membership.permissions);
  const [scopeId, setScopeId] = useState(props.agent.membership.staffing_scope_org_unit_id ?? "");
  const [roleKey, setRoleKey] = useState(props.agent.membership.role_key);
  const [professionKey, setProfessionKey] = useState(currentProfessionKey);
  const [busy, setBusy] = useState(false);
  const unit = props.consoleData.org_units.find((item) => item.id === props.agent.membership.org_unit_id);
  const active = props.agent.membership.employment_status === "active";
  const provisioning = props.agent.membership.employment_status === "provisioning";
  const displayedStatus = active ? props.agent.connection.status : props.agent.membership.employment_status;
  const connectionDetail = props.agent.connection.last_used_at
    ? `最近连接 ${formatTime(props.agent.connection.last_used_at)}`
    : props.agent.connection.key_expires_at
      ? `Key 有效期至 ${formatTime(props.agent.connection.key_expires_at)}`
      : "尚未签发可用 Key";

  useEffect(() => {
    setPermissions(props.agent.membership.permissions);
    setScopeId(props.agent.membership.staffing_scope_org_unit_id ?? "");
    setRoleKey(props.agent.membership.role_key);
    setProfessionKey(currentProfessionKey);
  }, [props.agent.membership.permissions, props.agent.membership.role_key, props.agent.membership.staffing_scope_org_unit_id, currentProfessionKey]);

  async function rotateKey() {
    if (!window.confirm(`轮换 ${props.agent.agent_profile.display_name} 的 Key？旧 Key 会立即失效。`)) return;
    setBusy(true);
    try {
      const response = await api<{ result: { agent_key_plaintext: string; agent_key_prefix: string } }>(
        `/api/v1/humans/${props.humanUserId}/agents/${props.agent.agent_profile.id}/rotate-key`,
        { method: "POST" },
        props.token,
      );
      props.onCredential({
        agent: props.agent.agent_profile,
        key: response.result.agent_key_plaintext,
        keyPrefix: response.result.agent_key_prefix,
        permissions: props.agent.membership.permissions,
        professionKey: currentProfessionKey,
        profession: props.consoleData.professions.find((profession) => profession.key === currentProfessionKey),
        skillLanguage: props.consoleData.governance_policy.effective_settings.skill_language,
      });
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  function openConnectionGuide() {
    props.onCredential({
      agent: props.agent.agent_profile,
      key: null,
      keyPrefix: props.agent.connection.key_prefix ?? "",
      permissions: props.agent.membership.permissions,
      professionKey: currentProfessionKey,
      profession: props.consoleData.professions.find((profession) => profession.key === currentProfessionKey),
      skillLanguage: props.consoleData.governance_policy.effective_settings.skill_language,
    });
  }

  async function changeStatus(action: "activate" | "suspend" | "reactivate" | "terminate") {
    const labels = { activate: "激活", suspend: "暂停", reactivate: "重新激活", terminate: "永久裁撤" };
    if (!window.confirm(`${labels[action]} ${props.agent.agent_profile.display_name}？${action === "terminate" ? "该操作不可恢复。" : ""}`)) return;
    setBusy(true);
    try {
      const response = await api<{ result: { agent_key_plaintext?: string; agent_key_prefix?: string } }>(
        `/api/v1/companies/${props.consoleData.company.id}/agents/${props.agent.agent_profile.id}/${action}`,
        { method: "POST", body: JSON.stringify({ reason: `Human console: ${action}` }) },
        props.token,
      );
      if (response.result.agent_key_plaintext) {
        props.onCredential({
          agent: props.agent.agent_profile,
          key: response.result.agent_key_plaintext,
          keyPrefix: response.result.agent_key_prefix ?? "",
          permissions: props.agent.membership.permissions,
          professionKey: currentProfessionKey,
          profession: props.consoleData.professions.find((profession) => profession.key === currentProfessionKey),
          skillLanguage: props.consoleData.governance_policy.effective_settings.skill_language,
        });
      } else {
        props.onNotice(`${props.agent.agent_profile.display_name} 已${labels[action]}`);
      }
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function savePermissions() {
    setBusy(true);
    try {
      await api(
        `/api/v1/companies/${props.consoleData.company.id}/agents/${props.agent.agent_profile.id}/permissions`,
        {
          method: "POST",
          body: JSON.stringify({
            staffing_permissions: permissions.filter((permission) => STAFFING_PERMISSIONS.some((item) => item.key === permission)),
            project_permissions: permissions.filter((permission) => PROJECT_PERMISSIONS.some((item) => item.key === permission)),
            staffing_scope_org_unit_id: permissions.some((permission) => permission.startsWith("agent.staff.")) ? scopeId || null : null,
            reason: "Human console permission update",
          }),
        },
        props.token,
      );
      props.onNotice("Agent 特殊权限已更新");
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function saveRole() {
    setBusy(true);
    try {
      await api(
        `/api/v1/companies/${props.consoleData.company.id}/agents/${props.agent.agent_profile.id}/role`,
        {
          method: "POST",
          body: JSON.stringify({ role_key: roleKey, reason: "Human console role update" }),
        },
        props.token,
      );
      props.onNotice("公司角色已更新，请重新复制该 Agent 的接入资料以获取最新 Skill");
      await props.onChanged();
    } catch (error) {
      setRoleKey(props.agent.membership.role_key);
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function saveProfession() {
    setBusy(true);
    try {
      await api(
        `/api/v1/companies/${props.consoleData.company.id}/agents/${props.agent.agent_profile.id}/profession`,
        {
          method: "POST",
          body: JSON.stringify({ profession_key: professionKey, reason: "Human console profession update" }),
        },
        props.token,
      );
      props.onNotice("职业已更新，任务权限和职业 Skill 已同步重算；请重新复制该 Agent 的接入资料");
      await props.onChanged();
    } catch (error) {
      setProfessionKey(currentProfessionKey);
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <article className={`agent-row ${expanded ? "expanded" : ""}`}>
      <div className="agent-summary">
        <span className="agent-avatar">{props.agent.agent_profile.display_name.slice(0, 1).toUpperCase()}</span>
        <div className="agent-identity"><strong>{props.agent.agent_profile.display_name}</strong><span>@{props.agent.agent_profile.handle.replace(/^@/, "")}</span></div>
        <div className="agent-meta"><span>{props.agent.membership.job_title || "Agent"}</span><small>{unit?.name ?? "未分配组织"}</small><small className="connection-detail">{connectionDetail}</small></div>
        <StatusBadge value={displayedStatus} />
        <div className="agent-actions">
          <button className="button small" type="button" onClick={() => setShowMemories(true)}><Icon name="memory" /> 记忆</button>
          {props.agent.connection.key_prefix ? <button className="button small" onClick={openConnectionGuide}><Icon name="key" /> 接入资料</button> : null}
          {active ? <button className="button small" onClick={() => void rotateKey()} disabled={busy}><Icon name="refresh" /> 轮换 Key</button> : null}
          {provisioning ? <button className="button small primary" onClick={() => void changeStatus("activate")} disabled={busy}>激活并签发 Key</button> : null}
          {active ? <button className="icon-button" title="暂停" onClick={() => void changeStatus("suspend")} disabled={busy}><Icon name="pause" /></button> : null}
          {props.agent.membership.employment_status === "suspended" ? <button className="icon-button" title="重新激活" onClick={() => void changeStatus("reactivate")} disabled={busy}><Icon name="play" /></button> : null}
          {props.agent.membership.employment_status !== "terminated" ? <button className="icon-button danger" title="裁撤" onClick={() => void changeStatus("terminate")} disabled={busy}><Icon name="trash" /></button> : null}
          <button className="icon-button" title="展开画像与权限" onClick={() => setExpanded(!expanded)}><Icon name={expanded ? "chevron-up" : "chevron-down"} /></button>
        </div>
      </div>
      {expanded ? (
        <div className="agent-permissions">
          <div className="work-profile-card">
            <div className="work-profile-summary">
              <span className="eyebrow">WORK PROFILE</span>
              <h4>工作画像</h4>
              <p>{props.agent.agent_profile.persona || "尚未填写工作说明"}</p>
            </div>
            <div className="work-profile-field">
              <strong>职责</strong>
              <div className="profile-tags">
                {props.agent.membership.responsibilities.length
                  ? props.agent.membership.responsibilities.map((item) => <span key={item}>{item}</span>)
                  : <small>尚未维护</small>}
              </div>
            </div>
            <div className="work-profile-field">
              <strong>技能</strong>
              <div className="profile-tags">
                {props.agent.membership.skills.length
                  ? props.agent.membership.skills.map((item) => <span key={item}>{item}</span>)
                  : <small>尚未维护</small>}
              </div>
            </div>
            <div className="work-profile-field">
              <strong>当前重点</strong>
              <p>{props.agent.membership.current_focus || "尚未声明当前重点"}</p>
              <small>协作状态：{collaborationPreferenceLabel(props.agent.agent_profile.collaboration_preference)}</small>
            </div>
          </div>
          <div className="role-access-card">
            <div><h4>职业与公司角色</h4><p>职业决定工作方法、职业 Skill 和任务权限；公司角色只负责组织治理与项目成员管理。</p></div>
            <Field label="系统职业">
              <select value={professionKey} onChange={(event) => setProfessionKey(event.target.value)} disabled={props.agent.membership.employment_status === "terminated"}>
                {professionGroups.map(([category, professions]) => (
                  <optgroup label={category} key={category}>
                    {professions.map((profession) => <option key={profession.key} value={profession.key}>{skillLanguage === "en" ? profession.label_en : profession.label}</option>)}
                  </optgroup>
                ))}
              </select>
              {props.consoleData.professions.find((profession) => profession.key === professionKey) ? (
                <small>{skillLanguage === "en" ? props.consoleData.professions.find((profession) => profession.key === professionKey)?.description_en : props.consoleData.professions.find((profession) => profession.key === professionKey)?.description} {props.consoleData.professions.find((profession) => profession.key === professionKey)?.can_create_tasks ? "可创建、拆分和分配任务。" : "只能查看任务并更新自己任务的执行状态。"}</small>
              ) : null}
            </Field>
            <button className="button primary small" onClick={() => void saveProfession()} disabled={busy || professionKey === currentProfessionKey}>保存职业</button>
            <Field label="当前角色">
              <select value={roleKey} onChange={(event) => setRoleKey(event.target.value)} disabled={props.agent.membership.employment_status === "terminated"}>
                <option value="member">普通成员</option>
                <option value="company_manager">公司管理 Agent</option>
              </select>
            </Field>
            <button className="button primary small" onClick={() => void saveRole()} disabled={busy || roleKey === props.agent.membership.role_key}>保存角色</button>
          </div>
          <div className="staffing-access-card">
            <div><h4>特殊人员授权</h4><p>这是独立于公司角色的高风险授权。被授权的 Agent 可以通过 MCP 操作授权组织范围内的人员。</p></div>
            <div className="permission-grid">
              {STAFFING_PERMISSIONS.map((permission) => (
                <label className="check-row" key={permission.key}>
                  <input
                    type="checkbox"
                    checked={permissions.includes(permission.key)}
                    onChange={(event) => setPermissions(event.target.checked ? [...permissions, permission.key] : permissions.filter((item) => item !== permission.key))}
                  />
                  <span>{permission.label}</span>
                </label>
              ))}
            </div>
            <Field label="授权组织范围">
              <select value={scopeId} onChange={(event) => setScopeId(event.target.value)}>
                <option value="">全公司</option>
                {props.consoleData.org_units.map((orgUnit) => <option key={orgUnit.id} value={orgUnit.id}>{orgUnit.name}</option>)}
              </select>
            </Field>
            <button className="button primary small" onClick={() => void savePermissions()} disabled={busy}>保存授权</button>
          </div>
          <div className="project-access-card">
            <div><h4>项目内容授权</h4><p>授权后，项目成员可以通过 MCP 生成 Rule 或维护资产清单；权限只在其参与的项目内生效。</p></div>
            <div className="permission-grid">
              {PROJECT_PERMISSIONS.map((permission) => (
                <label className="check-row" key={permission.key}>
                  <input
                    type="checkbox"
                    checked={permissions.includes(permission.key)}
                    onChange={(event) => setPermissions(event.target.checked ? [...permissions, permission.key] : permissions.filter((item) => item !== permission.key))}
                  />
                  <span>{permission.label}</span>
                </label>
              ))}
            </div>
            <button className="button primary small" onClick={() => void savePermissions()} disabled={busy}>保存授权</button>
          </div>
        </div>
      ) : null}
      {showMemories ? (
        <Dialog
          title={`${props.agent.agent_profile.display_name} · Agent 记忆`}
          description="这里仅展示该 Agent 独立拥有的长期与短期精华记忆。"
          onClose={() => setShowMemories(false)}
          extraWide
        >
          <MemoriesView
            consoleData={props.consoleData}
            token={props.token}
            fixedAgentId={props.agent.agent_profile.id}
            embedded
            onError={props.onError}
            onNotice={props.onNotice}
          />
        </Dialog>
      ) : null}
    </article>
  );
}

function SkillsView(props: {
  consoleData: CompanyConsole | null;
  systemProjectTypes: CompanyProjectType[];
}) {
  const { consoleData, systemProjectTypes } = props;
  const { language: uiLanguage } = useUiLanguage();
  const skillLanguage: RelaySkillLanguage = uiLanguage;
  const [tab, setTab] = useState<"agent_skills" | "project_rules">("agent_skills");
  const professions = useMemo<CompanyProfession[]>(() => consoleData?.professions ?? Object.entries(RELAY_PROFESSION_SKILLS).map(([key, document]) => ({
    key,
    label: document.title.replace("职业 Skill", ""),
    label_en: document.title.replace("职业 Skill", ""),
    description: `${document.title.replace("职业 Skill", "")}的岗位工作方法、交付标准和权限边界。`,
    description_en: `Professional workflow, deliverables, quality gates, and authority boundaries for ${document.title}.`,
    category_key: "general",
    category_label: "通用协作",
    category_label_en: "General Collaboration",
    skill_name: document.name,
    skill_markdown: document.content,
    skill_markdown_en: document.content,
    can_create_tasks: key === "project_manager" || key === "product_manager" || key === "technical_manager",
  })), [consoleData?.professions]);
  const agents = useMemo(() => consoleData?.agents ?? [], [consoleData?.agents]);
  const [selectedAgentId, setSelectedAgentId] = useState(agents[0]?.agent_profile.id ?? "");
  const library = useMemo(() => [
    {
      category: skillLanguage === "en" ? "Shared Layer" : "通用层",
      description: skillLanguage === "en" ? "Company identity, messaging, task, memory, and silence protocol used by every Agent." : "所有 Agent 都会使用的公司身份、消息、任务和静默协作协议。",
      document: skillLanguage === "en" ? RELAY_EMPLOYEE_SKILL_EN : RELAY_EMPLOYEE_SKILL,
    },
    ...professions.map((profession) => ({
      category: skillLanguage === "en" ? profession.category_label_en : profession.category_label,
      description: skillLanguage === "en" ? profession.description_en : profession.description,
      document: relayProfessionSkillDocument(profession, skillLanguage),
    })),
    {
      category: skillLanguage === "en" ? "Authorization Layer" : "授权层",
      description: skillLanguage === "en" ? "Added only when Human grants staffing permissions; governs hiring, suspension, and termination." : "仅在 Human 授予人员管理权限时追加，约束招聘、暂停和裁撤动作。",
      document: skillLanguage === "en" ? RELAY_STAFFING_MANAGER_SKILL_EN : RELAY_STAFFING_MANAGER_SKILL,
    },
  ], [professions, skillLanguage]);
  const [selectedSkillName, setSelectedSkillName] = useState(RELAY_PROFESSION_SKILLS.project_manager.name);
  const [skillPreviewMode, setSkillPreviewMode] = useState<"guide" | "source">("guide");
  const skillCategories = useMemo(() => [...new Set(library.map((item) => item.category))], [library]);
  const [skillCategory, setSkillCategory] = useState("all");
  const visibleLibrary = useMemo(() => skillCategory === "all" ? library : library.filter((item) => item.category === skillCategory), [library, skillCategory]);
  const skillPagination = usePagination(visibleLibrary, 8, `${skillLanguage}:${skillCategory}:${visibleLibrary.length}`);
  const projectTypes = useMemo(() => consoleData?.project_types ?? systemProjectTypes, [consoleData?.project_types, systemProjectTypes]);
  const projectTypeCategories = useMemo(() => Array.from(
    new Map(projectTypes.map((type) => [type.category_key, skillLanguage === "en" ? type.category_label_en : type.category_label])).entries(),
  ).map(([key, label]) => ({ key, label })), [projectTypes, skillLanguage]);
  const [projectTypeCategory, setProjectTypeCategory] = useState("all");
  const visibleProjectTypes = useMemo(() => projectTypeCategory === "all"
    ? projectTypes
    : projectTypes.filter((type) => type.category_key === projectTypeCategory), [projectTypeCategory, projectTypes]);
  const [selectedProjectTypeKey, setSelectedProjectTypeKey] = useState(projectTypes[0]?.key ?? "");
  const [projectRulePreviewMode, setProjectRulePreviewMode] = useState<"guide" | "source">("guide");
  const projectTypePagination = usePagination(visibleProjectTypes, 6, `${projectTypeCategory}:${visibleProjectTypes.length}`);

  useEffect(() => {
    if (!agents.some((agent) => agent.agent_profile.id === selectedAgentId)) {
      setSelectedAgentId(agents[0]?.agent_profile.id ?? "");
    }
  }, [agents, selectedAgentId]);

  useEffect(() => {
    if (!visibleLibrary.some((item) => item.document.name === selectedSkillName)) {
      setSelectedSkillName(visibleLibrary[0]?.document.name ?? RELAY_EMPLOYEE_SKILL.name);
    }
  }, [visibleLibrary, selectedSkillName]);

  useEffect(() => {
    if (!visibleProjectTypes.some((type) => type.key === selectedProjectTypeKey)) {
      setSelectedProjectTypeKey(visibleProjectTypes[0]?.key ?? "");
    }
  }, [visibleProjectTypes, selectedProjectTypeKey]);

  const selectedAgent = agents.find((agent) => agent.agent_profile.id === selectedAgentId);
  const selectedProfession = selectedAgent
    ? professions.find((profession) => profession.key === companyAgentProfessionKey(selectedAgent, professions))
    : null;
  const composedDocuments = selectedAgent ? getRelaySkillDocuments(
    selectedAgent.membership.permissions,
    {
      agentId: selectedAgent.agent_profile.id,
      handle: selectedAgent.agent_profile.handle,
      mcpServerName: relayAgentConnectionNames(selectedAgent.agent_profile).mcpServer,
    },
    companyAgentProfessionKey(selectedAgent, professions),
    skillLanguage,
    selectedProfession ?? undefined,
  ) : [];
  const selectedLibraryItem = library.find((item) => item.document.name === selectedSkillName) ?? library[0];
  const selectedSkillAnalysis = useMemo(
    () => analyzeRelaySkill(selectedLibraryItem.document),
    [selectedLibraryItem.document],
  );
  const selectedProjectType = visibleProjectTypes.find((type) => type.key === selectedProjectTypeKey) ?? visibleProjectTypes[0];
  const selectedProjectRuleDocument = useMemo<RelaySkillDocument>(() => ({
    name: selectedProjectType ? `relay-project-${selectedProjectType.key}` : "relay-project-unavailable",
    title: selectedProjectType ? skillLanguage === "en" ? `${selectedProjectType.label_en} Rules` : `${selectedProjectType.label}固定规则` : skillLanguage === "en" ? "Project type Rules unavailable" : "项目类型规则暂不可用",
    content: selectedProjectType ? skillLanguage === "en" ? selectedProjectType.rule_markdown_en : selectedProjectType.rule_markdown : "",
  }), [selectedProjectType, skillLanguage]);
  const selectedProjectRuleAnalysis = useMemo(
    () => analyzeRelaySkill(selectedProjectRuleDocument),
    [selectedProjectRuleDocument],
  );

  return (
    <div className="content-stack">
      <nav className="control-center-tabs two-tabs skill-center-tabs" role="tablist" aria-label="Skill 中心">
        <button
          className={tab === "agent_skills" ? "active" : ""}
          type="button"
          role="tab"
          aria-selected={tab === "agent_skills"}
          onClick={() => setTab("agent_skills")}
        >
          <span className="control-center-tab-icon"><Icon name="book" /></span>
          <span><strong>Agent Skill</strong><small>职业 Skill、组合与授权</small></span>
        </button>
        <button
          className={tab === "project_rules" ? "active" : ""}
          type="button"
          role="tab"
          aria-selected={tab === "project_rules"}
          onClick={() => setTab("project_rules")}
        >
          <span className="control-center-tab-icon"><Icon name="git" /></span>
          <span><strong>项目类型 Rule</strong><small>固定项目基线与执行流程</small></span>
        </button>
      </nav>

      <div className="skill-center-panel" role="tabpanel">

      {tab === "agent_skills" ? <section className="metric-row">
        <Metric label="Skill 模板" value={String(library.length)} detail="通用、职业和授权模板" />
        <Metric label="系统职业" value={String(professions.length)} detail="职业决定任务权限和工作方法" />
        <Metric label="项目类型" value={String(projectTypes.length)} detail="每类项目都有不可弱化的固定规则" />
        <Metric label="Agent 组合" value={String(agents.length)} detail="每个 Agent 独立生成绑定版本" />
      </section> : null}

      {tab === "project_rules" ? <section className="section-card skill-library-card project-rule-library-card">
        <div className="section-heading">
          <div>
            <span className="eyebrow">PROJECT TYPE RULES</span>
            <h2>项目类型规则库</h2>
          </div>
          <span className="count-badge">{projectTypes.length}</span>
        </div>
        <div className="project-rule-category-filter" role="tablist" aria-label="项目规则分类">
          <button className={projectTypeCategory === "all" ? "active" : ""} type="button" onClick={() => setProjectTypeCategory("all")}>全部 <span>{projectTypes.length}</span></button>
          {projectTypeCategories.map((category) => {
            const count = projectTypes.filter((type) => type.category_key === category.key).length;
            return <button className={projectTypeCategory === category.key ? "active" : ""} type="button" key={category.key} onClick={() => setProjectTypeCategory(category.key)}>{category.label} <span>{count}</span></button>;
          })}
        </div>
        {selectedProjectType ? (
          <div className="skill-library-layout">
            <div className="skill-library-list project-rule-type-list">
              {projectTypePagination.pageItems.map((type) => {
                const typeLabel = skillLanguage === "en" ? type.label_en : type.label;
                const typeDescription = skillLanguage === "en" ? type.description_en : type.description;
                const typeCategory = skillLanguage === "en" ? type.category_label_en : type.category_label;
                const document: RelaySkillDocument = { name: `relay-project-${type.key}`, title: skillLanguage === "en" ? `${typeLabel} Rules` : `${typeLabel}固定规则`, content: skillLanguage === "en" ? type.rule_markdown_en : type.rule_markdown };
                const analysis = analyzeRelaySkill(document);
                return (
                  <button className={type.key === selectedProjectType.key ? "active" : ""} key={type.key} onClick={() => setSelectedProjectTypeKey(type.key)}>
                    <span>{typeCategory}</span>
                    <strong>{typeLabel}</strong>
                    <small>{typeDescription}</small>
                    <div className="skill-list-stats"><b>{analysis.sections.length} 个模块</b><b>{analysis.instructionCount} 条规则</b></div>
                    <code>{type.key}</code>
                  </button>
                );
              })}
              <Pagination {...projectTypePagination} onPageChange={projectTypePagination.setPage} compact />
            </div>
            <div className="skill-template-preview project-rule-preview">
              <div className="skill-template-preview-head">
                <div>
                  <span>SYSTEM PROJECT SKILL</span>
                  <strong>{selectedProjectRuleDocument.title}</strong>
                  <code>{selectedProjectRuleDocument.name}/SKILL.md</code>
                </div>
                <div className="skill-preview-actions">
                  <div className="skill-preview-toggle">
                    <button className={projectRulePreviewMode === "guide" ? "active" : ""} onClick={() => setProjectRulePreviewMode("guide")}>规则手册</button>
                    <button className={projectRulePreviewMode === "source" ? "active" : ""} onClick={() => setProjectRulePreviewMode("source")}>Rule 原文</button>
                  </div>
                  <button className="button small" onClick={() => void copyText(selectedProjectRuleDocument.content)}><Icon name="copy" /> 复制规则</button>
                </div>
              </div>
              {projectRulePreviewMode === "guide" ? (
                <div className="skill-guide-preview">
                  <section className="skill-guide-overview project-rule-overview">
                    <div>
                      <span className="eyebrow">MANDATORY PROJECT BASELINE</span>
                      <h3>{skillLanguage === "en" ? selectedProjectType.label_en : selectedProjectType.label}</h3>
                      <p>{skillLanguage === "en"
                        ? `${selectedProjectType.description_en} Rules combine the shared governance baseline, the ${selectedProjectType.category_label_en} discipline baseline, and the ${selectedProjectType.label_en} playbook. Agents and Humans may add stricter constraints only.`
                        : `${selectedProjectType.description} 规则按“共同治理基线 + ${selectedProjectType.category_label}领域基线 + ${selectedProjectType.label}专项规则”组合，Agent 和 Human 只能追加更严格的项目约束。`}</p>
                    </div>
                    <div className="skill-guide-metrics">
                      <span><small>规则模块</small><strong>{selectedProjectRuleAnalysis.sections.length}</strong></span>
                      <span><small>执行要求</small><strong>{selectedProjectRuleAnalysis.instructionCount}</strong></span>
                      <span><small>自动加载</small><strong>YES</strong></span>
                      <span><small>允许弱化</small><strong>NO</strong></span>
                    </div>
                  </section>
                  <nav className="skill-section-index" aria-label="项目类型规则目录">
                    {selectedProjectRuleAnalysis.sections.map((section, index) => <span key={`${section.title}-${index}`}><b>{String(index + 1).padStart(2, "0")}</b>{section.title}</span>)}
                  </nav>
                  <div className="skill-section-grid project-rule-section-grid">
                    {selectedProjectRuleAnalysis.sections.map((section, index) => <SkillGuideSection key={`${section.title}-${index}`} section={section} index={index} />)}
                  </div>
                </div>
              ) : <pre>{selectedProjectRuleDocument.content}</pre>}
            </div>
          </div>
        ) : (
          <div className="empty-inline"><Icon name="book" /><h3>项目类型规则尚未加载</h3><p>选择一个公司后，Relay 会从系统目录加载全部项目类型和固定 Rule。</p></div>
        )}
      </section> : null}

      {tab === "agent_skills" ? <>
      <section className="section-card">
        <div className="section-heading">
          <div>
            <span className="eyebrow">AGENT COMPOSITION</span>
            <h2>Agent 实际 Skill 组合</h2>
          </div>
          {agents.length ? (
            <select className="skill-agent-select" value={selectedAgentId} onChange={(event) => setSelectedAgentId(event.target.value)}>
              {agents.map((agent) => <option key={agent.agent_profile.id} value={agent.agent_profile.id}>{agent.agent_profile.display_name} · {agent.profession?.label ?? agent.membership.job_title}</option>)}
            </select>
          ) : null}
        </div>
        {selectedAgent ? (
          <div className="skill-agent-composition">
            <div className="skill-agent-summary">
              <span className="agent-avatar">{selectedAgent.agent_profile.display_name.slice(0, 1).toUpperCase()}</span>
              <div>
                <strong>{selectedAgent.agent_profile.display_name}</strong>
                <small>@{selectedAgent.agent_profile.handle.replace(/^@/, "")} · {selectedProfession?.label ?? selectedAgent.membership.job_title}</small>
              </div>
              <div className="skill-composition-tags">
                {composedDocuments.map((document, index) => <span key={document.name}>{index === 0 ? "通用" : index === 1 ? "职业" : "授权"} · {document.title.split(" · ")[0]}</span>)}
              </div>
            </div>
            <SkillCopyBlock documents={composedDocuments} step="完整组合" />
          </div>
        ) : (
          <div className="empty-inline"><Icon name="book" /><h3>还没有 Agent</h3><p>创建 Agent 后，这里会展示它最终使用的通用、职业和授权 Skill 组合。</p></div>
        )}
      </section>

      <section className="section-card skill-library-card">
        <div className="section-heading">
          <div>
            <span className="eyebrow">SKILL LIBRARY</span>
            <h2>Skill 模板库</h2>
          </div>
          <span className="count-badge">{library.length}</span>
        </div>
        <div className="project-rule-category-filter profession-skill-category-filter" role="tablist" aria-label="Profession Skill categories">
          <button className={skillCategory === "all" ? "active" : ""} type="button" onClick={() => setSkillCategory("all")}>{skillLanguage === "en" ? "All" : "全部"} <span>{library.length}</span></button>
          {skillCategories.map((category) => <button className={skillCategory === category ? "active" : ""} type="button" key={category} onClick={() => setSkillCategory(category)}>{category} <span>{library.filter((item) => item.category === category).length}</span></button>)}
        </div>
        <div className="skill-library-layout">
          <div className="skill-library-list">
            {skillPagination.pageItems.map((item) => {
              const analysis = analyzeRelaySkill(item.document);
              return (
                <button className={item.document.name === selectedLibraryItem.document.name ? "active" : ""} key={item.document.name} onClick={() => setSelectedSkillName(item.document.name)}>
                  <span>{item.category}</span>
                  <strong>{item.document.title}</strong>
                  <small>{item.description}</small>
                  <div className="skill-list-stats"><b>{analysis.sections.length} 个模块</b><b>{analysis.instructionCount} 条规则</b></div>
                  <code>{item.document.name}/SKILL.md</code>
                </button>
              );
            })}
            <Pagination {...skillPagination} onPageChange={skillPagination.setPage} compact />
          </div>
          <div className="skill-template-preview">
            <div className="skill-template-preview-head">
              <div>
                <span>{selectedLibraryItem.category}</span>
                <strong>{selectedLibraryItem.document.title}</strong>
                <code>{selectedLibraryItem.document.name}/SKILL.md</code>
              </div>
              <div className="skill-preview-actions">
                <div className="skill-preview-toggle">
                  <button className={skillPreviewMode === "guide" ? "active" : ""} onClick={() => setSkillPreviewMode("guide")}>岗位手册</button>
                  <button className={skillPreviewMode === "source" ? "active" : ""} onClick={() => setSkillPreviewMode("source")}>Skill 原文</button>
                </div>
                <button className="button small" onClick={() => void copyText(selectedLibraryItem.document.content)}><Icon name="copy" /> 复制原文</button>
              </div>
            </div>
            {skillPreviewMode === "guide" ? (
              <div className="skill-guide-preview">
                <section className="skill-guide-overview">
                  <div>
                    <span className="eyebrow">ROLE OPERATING SYSTEM</span>
                    <h3>{selectedLibraryItem.document.title}</h3>
                    <p>{selectedSkillAnalysis.overview || selectedLibraryItem.description}</p>
                  </div>
                  <div className="skill-guide-metrics">
                    <span><small>工作模块</small><strong>{selectedSkillAnalysis.sections.length}</strong></span>
                    <span><small>行动规则</small><strong>{selectedSkillAnalysis.instructionCount}</strong></span>
                    <span><small>质量门禁</small><strong>{selectedSkillAnalysis.qualityGateCount}</strong></span>
                    <span><small>状态模型</small><strong>{selectedSkillAnalysis.states.length || 4}</strong></span>
                  </div>
                  {selectedSkillAnalysis.tools.length ? <div className="skill-tool-strip"><small>涉及工具 / 状态</small>{selectedSkillAnalysis.tools.map((tool) => <code key={tool}>{tool}</code>)}{selectedSkillAnalysis.states.map((state) => <code key={state}>{state}</code>)}</div> : null}
                </section>
                <nav className="skill-section-index" aria-label="Skill 内容目录">
                  {selectedSkillAnalysis.sections.map((section, index) => <span key={`${section.title}-${index}`}><b>{String(index + 1).padStart(2, "0")}</b>{section.title}</span>)}
                </nav>
                <div className="skill-section-grid">
                  {selectedSkillAnalysis.sections.map((section, index) => <SkillGuideSection key={`${section.title}-${index}`} section={section} index={index} />)}
                </div>
              </div>
            ) : <pre>{selectedLibraryItem.document.content}</pre>}
          </div>
        </div>
      </section>
      </> : null}
      </div>
    </div>
  );
}

function renderSkillInline(value: string): ReactNode[] {
  return value.split(/(`[^`]+`)/g).filter(Boolean).map((part, index) => (
    part.startsWith("`") && part.endsWith("`")
      ? <code key={`${part}-${index}`}>{part.slice(1, -1)}</code>
      : <span key={`${part}-${index}`}>{part}</span>
  ));
}

function SkillGuideSection({ section, index }: { section: RelaySkillSection; index: number }) {
  return (
    <article className={`skill-guide-section ${section.kind}`}>
      <header><span>{String(index + 1).padStart(2, "0")}</span><div><small>{section.kind.toUpperCase()}</small><h4>{section.title}</h4></div></header>
      {section.paragraphs.map((paragraph, paragraphIndex) => <p key={`${paragraph}-${paragraphIndex}`}>{renderSkillInline(paragraph)}</p>)}
      {section.orderedItems.length ? <ol>{section.orderedItems.map((item, itemIndex) => <li key={`${item}-${itemIndex}`}>{renderSkillInline(item)}</li>)}</ol> : null}
      {section.bulletItems.length ? <ul>{section.bulletItems.map((item, itemIndex) => <li key={`${item}-${itemIndex}`}>{renderSkillInline(item)}</li>)}</ul> : null}
      {section.codeBlocks.map((block, blockIndex) => <pre className="skill-guide-code" key={`${blockIndex}-${block.slice(0, 20)}`}>{block}</pre>)}
      {section.tableRows.length ? (
        <div className="skill-guide-table-wrap"><table><thead><tr>{section.tableRows[0].map((cell, cellIndex) => <th key={`${cell}-${cellIndex}`}>{renderSkillInline(cell)}</th>)}</tr></thead><tbody>{section.tableRows.slice(1).map((row, rowIndex) => <tr key={`${rowIndex}-${row.join("-")}`}>{row.map((cell, cellIndex) => <td key={`${cell}-${cellIndex}`}>{renderSkillInline(cell)}</td>)}</tr>)}</tbody></table></div>
      ) : null}
    </article>
  );
}

function ChatCenter(props: {
  consoleData: CompanyConsole;
  humanUser: HumanUser;
  token: string;
  realtimeEvent: CompanyRealtimeEvent | null;
  approvals: AgentToolApproval[];
  onReview: (approvalId: string, decision: "approve" | "reject", reviewNote: string) => Promise<void>;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [tab, setTab] = useState<"messages" | "approvals">("messages");
  const pendingApprovalCount = props.approvals.filter((approval) => approval.status === "pending").length;

  return (
    <div className="control-center chat-center">
      <nav className="control-center-tabs two-tabs" role="tablist" aria-label="聊天">
        <button className={tab === "messages" ? "active" : ""} type="button" role="tab" aria-selected={tab === "messages"} onClick={() => setTab("messages")}>
          <span className="control-center-tab-icon"><Icon name="message" /></span>
          <span><strong>聊天</strong><small>Human 与 Agent</small></span>
        </button>
        <button className={tab === "approvals" ? "active" : ""} type="button" role="tab" aria-selected={tab === "approvals"} onClick={() => setTab("approvals")}>
          <span className="control-center-tab-icon"><Icon name="shield" /></span>
          <span><strong>审批</strong><small>权限升级请求</small></span>
          {pendingApprovalCount ? <span className="control-center-tab-badge">{pendingApprovalCount}</span> : null}
        </button>
      </nav>
      <div className="control-center-panel" role="tabpanel">
        {tab === "messages" ? (
          <MessagesView
            consoleData={props.consoleData}
            humanUser={props.humanUser}
            token={props.token}
            realtimeEvent={props.realtimeEvent}
            onChanged={props.onChanged}
            onError={props.onError}
            onNotice={props.onNotice}
          />
        ) : null}
        {tab === "approvals" ? (
          <ApprovalsView
            approvals={props.approvals}
            agents={props.consoleData.agents}
            onReview={props.onReview}
            onError={props.onError}
          />
        ) : null}
      </div>
    </div>
  );
}

function CodexControlCenter(props: {
  consoleData: CompanyConsole;
  token: string;
  realtimeEvent: CompanyRealtimeEvent | null;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [tab, setTab] = useState<"runners" | "settings" | "mcp" | "plugins" | "environment">("runners");

  return (
    <div className="control-center codex-control-center">
      <nav className="control-center-tabs" role="tablist" aria-label="Codex 控制台">
        <button
          className={tab === "runners" ? "active" : ""}
          type="button"
          role="tab"
          aria-selected={tab === "runners"}
          onClick={() => setTab("runners")}
        >
          <span className="control-center-tab-icon"><Icon name="terminal" /></span>
          <span><strong>运行器</strong><small>配置与会话</small></span>
        </button>
        <button
          className={tab === "settings" ? "active" : ""}
          type="button"
          role="tab"
          aria-selected={tab === "settings"}
          onClick={() => setTab("settings")}
        >
          <span className="control-center-tab-icon"><Icon name="settings" /></span>
          <span><strong>CLI 设置</strong><small>默认值与覆盖</small></span>
        </button>
        <button
          className={tab === "mcp" ? "active" : ""}
          type="button"
          role="tab"
          aria-selected={tab === "mcp"}
          onClick={() => setTab("mcp")}
        >
          <span className="control-center-tab-icon"><Icon name="network" /></span>
          <span><strong>MCP</strong><small>服务与连接</small></span>
        </button>
        <button
          className={tab === "plugins" ? "active" : ""}
          type="button"
          role="tab"
          aria-selected={tab === "plugins"}
          onClick={() => setTab("plugins")}
        >
          <span className="control-center-tab-icon"><Icon name="plugin" /></span>
          <span><strong>插件</strong><small>CLI 能力目录</small></span>
        </button>
        <button
          className={tab === "environment" ? "active" : ""}
          type="button"
          role="tab"
          aria-selected={tab === "environment"}
          onClick={() => setTab("environment")}
        >
          <span className="control-center-tab-icon"><Icon name="key" /></span>
          <span><strong>CLI 与认证</strong><small>安装与账号环境</small></span>
        </button>
      </nav>

      <div className="control-center-panel" role="tabpanel">
        {tab === "runners" ? (
          <CodexRunnersView
            consoleData={props.consoleData}
            token={props.token}
            realtimeEvent={props.realtimeEvent}
            onError={props.onError}
            onNotice={props.onNotice}
          />
        ) : null}
        {tab === "plugins" ? (
          <CodexPluginsView
            companyId={props.consoleData.company.id}
            token={props.token}
            realtimeEvent={props.realtimeEvent}
            onError={props.onError}
            onNotice={props.onNotice}
          />
        ) : null}
        {tab === "settings" ? (
          <CodexCliSettingsView
            companyId={props.consoleData.company.id}
            token={props.token}
            onError={props.onError}
            onNotice={props.onNotice}
          />
        ) : null}
        {tab === "mcp" ? (
          <CodexMcpView
            companyId={props.consoleData.company.id}
            token={props.token}
            onError={props.onError}
            onNotice={props.onNotice}
          />
        ) : null}
        {tab === "environment" ? (
          <CodexCliAuthView
            companyId={props.consoleData.company.id}
            token={props.token}
            onError={props.onError}
            onNotice={props.onNotice}
          />
        ) : null}
      </div>
    </div>
  );
}

function CodexMcpView(props: {
  companyId: string;
  token: string;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [environment, setEnvironment] = useState<CodexEnvironmentView | null>(null);
  const [targetSelector, setTargetSelector] = useState("default");
  const [creating, setCreating] = useState(false);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState("");
  const [name, setName] = useState("");
  const [transport, setTransport] = useState<"streamable_http" | "stdio">("streamable_http");
  const [url, setUrl] = useState("");
  const [command, setCommand] = useState("");
  const [argsText, setArgsText] = useState("");
  const [bearerTokenEnvVar, setBearerTokenEnvVar] = useState("");

  async function loadEnvironment(silent = false) {
    if (!silent) setLoading(true);
    try {
      const response = await api<CodexEnvironmentView>(
        `/api/v1/companies/${props.companyId}/codex-environments`,
        {},
        props.token,
      );
      setEnvironment(response);
      const selectors = ["default", ...response.profiles.filter((profile) => profile.status === "active").map((profile) => profile.selector)];
      setTargetSelector((current) => selectors.includes(current) ? current : "default");
    } catch (error) {
      props.onError(error);
    } finally {
      if (!silent) setLoading(false);
    }
  }

  useEffect(() => { void loadEnvironment(); }, [props.companyId, props.token]);
  const hasPendingOperation = environment?.mcp_environments.some((snapshot) => !["idle", "failed"].includes(snapshot.operation_status));
  useEffect(() => {
    const timer = window.setTimeout(() => void loadEnvironment(true), hasPendingOperation ? 2_000 : 30_000);
    return () => window.clearTimeout(timer);
  }, [hasPendingOperation, environment?.runtime.updated_at, props.companyId, props.token]);

  const snapshot = environment?.mcp_environments.find((item) => item.selector === targetSelector) ?? null;
  const profile = environment?.profiles.find((item) => item.selector === targetSelector) ?? null;
  const servers = snapshot?.servers ?? [];
  const serverPagination = usePagination(servers, 9, targetSelector);
  const operationBusy = Boolean(snapshot && !["idle", "failed"].includes(snapshot.operation_status));

  function resetForm() {
    setName("");
    setUrl("");
    setCommand("");
    setArgsText("");
    setBearerTokenEnvVar("");
  }

  async function refresh() {
    setBusy("refresh");
    try {
      await api(
        `/api/v1/companies/${props.companyId}/codex-mcp-servers/refresh`,
        { method: "POST", body: JSON.stringify({ target_selector: targetSelector }) },
        props.token,
      );
      props.onNotice("MCP 刷新请求已交给宿主机 Trigger");
      await loadEnvironment(true);
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy("");
    }
  }

  async function addServer(event: FormEvent) {
    event.preventDefault();
    setBusy("add");
    try {
      await api(
        `/api/v1/companies/${props.companyId}/codex-mcp-servers`,
        {
          method: "POST",
          body: JSON.stringify({
            target_selector: targetSelector,
            name,
            transport,
            url: transport === "streamable_http" ? url : null,
            command: transport === "stdio" ? command : null,
            args: transport === "stdio" ? argsText.split("\n").map((item) => item.trim()).filter(Boolean) : [],
            bearer_token_env_var: transport === "streamable_http" && bearerTokenEnvVar.trim() ? bearerTokenEnvVar.trim() : null,
          }),
        },
        props.token,
      );
      resetForm();
      setCreating(false);
      props.onNotice("MCP 配置已排队，Trigger 将写入目标 Codex CLI 环境");
      await loadEnvironment(true);
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy("");
    }
  }

  async function removeServer(server: CodexMcpServer) {
    setBusy(`remove:${server.name}`);
    try {
      await api(
        `/api/v1/companies/${props.companyId}/codex-mcp-servers/${encodeURIComponent(targetSelector)}/${encodeURIComponent(server.name)}`,
        { method: "DELETE" },
        props.token,
      );
      props.onNotice(`已提交删除 MCP「${server.name}」的请求`);
      await loadEnvironment(true);
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy("");
    }
  }

  return (
    <div className="content-stack codex-mcp-page">
      <section className="section-card">
        <div className="section-heading mcp-catalog-heading">
          <div><span className="eyebrow">MCP ENVIRONMENT</span><h2>已配置的 MCP</h2><p>{snapshot?.last_checked_at ? `由 Trigger 调用 codex mcp list 发现 · ${formatTime(snapshot.last_checked_at)}` : "等待宿主机 Trigger 上报 MCP 列表。"}</p></div>
          <div className="section-heading-actions">
            <select value={targetSelector} onChange={(event) => setTargetSelector(event.target.value)}>
              <option value="default">宿主机默认 Codex</option>
              {environment?.profiles.filter((item) => item.status === "active").map((item) => <option key={item.id} value={item.selector}>{item.name} · 独立 CODEX_HOME</option>)}
            </select>
            <button className="button small" onClick={() => void refresh()} disabled={Boolean(busy) || operationBusy || !environment?.runtime.installed}><Icon name="refresh" /> {busy === "refresh" ? "提交中…" : "刷新"}</button>
            <button className="button primary small" onClick={() => setCreating((value) => !value)} disabled={operationBusy || !environment?.runtime.installed}><Icon name="plus" /> 添加 MCP</button>
          </div>
        </div>

        <div className="mcp-environment-summary">
          <span><small>目标环境</small><strong>{targetSelector === "default" ? "宿主机默认" : profile?.name ?? targetSelector}</strong></span>
          <span><small>已发现</small><strong>{servers.length}</strong></span>
          <span><small>用户配置</small><strong>{servers.filter((server) => server.configured_by_user).length}</strong></span>
          <span><small>插件提供</small><strong>{servers.filter((server) => !server.configured_by_user).length}</strong></span>
          <span><small>当前状态</small><strong>{codexMcpOperationLabel(snapshot?.operation_status ?? "idle")}</strong></span>
        </div>

        {snapshot?.last_error ? <div className="mcp-error-banner"><Icon name="alert" /><span><strong>MCP 操作失败</strong><small>{snapshot.last_error}</small></span></div> : null}

        {creating ? (
          <form className="mcp-create-form" onSubmit={addServer}>
            <div className="mcp-create-form-head"><div><strong>添加到 {targetSelector === "default" ? "宿主机默认 Codex" : profile?.name ?? targetSelector}</strong><small>配置由 Trigger 通过官方 codex mcp add 命令写入。</small></div><button type="button" className="icon-button" onClick={() => { setCreating(false); resetForm(); }}><Icon name="close" /></button></div>
            <div className="mcp-create-grid">
              <Field label="MCP 名称"><input value={name} onChange={(event) => setName(event.target.value)} placeholder="例如 openaiDeveloperDocs" pattern="[A-Za-z0-9_-]+" required maxLength={80} /></Field>
              <Field label="连接方式"><select value={transport} onChange={(event) => setTransport(event.target.value as "streamable_http" | "stdio")}><option value="streamable_http">Streamable HTTP</option><option value="stdio">本地 stdio</option></select></Field>
              {transport === "streamable_http" ? <><Field label="MCP URL"><input type="url" value={url} onChange={(event) => setUrl(event.target.value)} placeholder="https://example.com/mcp" required /></Field><Field label="Bearer Token 环境变量（可选）"><input value={bearerTokenEnvVar} onChange={(event) => setBearerTokenEnvVar(event.target.value)} placeholder="例如 FIGMA_OAUTH_TOKEN" pattern="[A-Za-z_][A-Za-z0-9_]*" /></Field></> : <><Field label="启动命令"><input value={command} onChange={(event) => setCommand(event.target.value)} placeholder="例如 npx" required /></Field><Field label="参数（每行一个）"><textarea value={argsText} onChange={(event) => setArgsText(event.target.value)} placeholder={"-y\n@upstash/context7-mcp"} rows={4} /></Field></>}
            </div>
            <p className="mcp-security-note"><Icon name="shield" /> 不要把 Token 直接写进 URL、命令或参数。HTTP Token 只填写环境变量名称，并在 Trigger 环境及 `AGENT_TRIGGER_CODEX_ENV_ALLOWLIST` 中提供。</p>
            <div className="mcp-create-actions"><button type="button" className="button small" onClick={() => { setCreating(false); resetForm(); }} disabled={Boolean(busy)}>取消</button><button className="button primary small" disabled={Boolean(busy) || servers.some((server) => server.name === name)}>{busy === "add" ? "正在提交…" : servers.some((server) => server.name === name) ? "名称已存在" : "保存到 Codex CLI"}</button></div>
          </form>
        ) : null}

        {loading ? <LoadingState /> : !snapshot ? <div className="empty-inline compact-empty"><Icon name="network" /><h3>等待 MCP 发现</h3><p>Trigger 启动后会自动读取现有 Codex MCP 配置。</p></div> : servers.length ? (
          <div className="mcp-server-grid">
            {serverPagination.pageItems.map((server) => <article className={`mcp-server-card ${server.enabled ? "enabled" : "disabled"}`} key={server.name}><div className="mcp-server-card-head"><span className="mcp-server-icon"><Icon name="network" /></span><div><strong>{server.name}</strong><small>{server.transport === "streamable_http" ? "Streamable HTTP" : server.transport === "stdio" ? "本地 stdio" : server.transport}</small></div><span className={server.enabled ? "plugin-enabled" : "plugin-disabled"}>{server.enabled ? "已启用" : "已停用"}</span></div><div className="mcp-server-endpoint">{server.address ? <code>{server.address}</code> : <code>{server.command ?? "隐藏命令"}{server.argument_count ? ` · ${server.argument_count} 个参数` : ""}</code>}</div><div className="mcp-server-meta"><span>{server.configured_by_user ? "用户配置" : "插件提供"}</span><span>{codexMcpAuthLabel(server.auth_status)}</span>{server.bearer_token_env_var ? <span>Token: {server.bearer_token_env_var}</span> : null}</div>{server.disabled_reason ? <p>{server.disabled_reason}</p> : null}<div className="mcp-server-actions">{server.configured_by_user ? <button className="button small danger" onClick={() => void removeServer(server)} disabled={Boolean(busy) || operationBusy}>{busy === `remove:${server.name}` ? "提交中…" : "删除"}</button> : <small>随插件安装，不能在此删除</small>}</div></article>)}
            <Pagination {...serverPagination} onPageChange={serverPagination.setPage} />
          </div>
        ) : <div className="empty-inline compact-empty"><Icon name="network" /><h3>当前环境还没有 MCP</h3><p>点击“添加 MCP”，配置 HTTP 服务或本地 stdio 服务。</p></div>}
      </section>
    </div>
  );
}

function codexMcpOperationLabel(status: CodexMcpEnvironmentSnapshot["operation_status"]) {
  if (["refresh_pending", "refreshing"].includes(status)) return "正在刷新";
  if (["add_pending", "adding"].includes(status)) return "正在添加";
  if (["remove_pending", "removing"].includes(status)) return "正在删除";
  if (status === "failed") return "需要处理";
  return "已同步";
}

function codexMcpAuthLabel(status: string | null) {
  if (status === "authenticated" || status === "logged_in") return "已认证";
  if (status === "not_logged_in") return "等待 OAuth 登录";
  if (status === "unsupported") return "无需 OAuth";
  return status || "认证状态未知";
}

function CodexPluginsView(props: {
  companyId: string;
  token: string;
  realtimeEvent: CompanyRealtimeEvent | null;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [catalogs, setCatalogs] = useState<CodexPluginCatalog[]>([]);
  const [operations, setOperations] = useState<CodexPluginOperation[]>([]);
  const [runnerId, setRunnerId] = useState("");
  const [tab, setTab] = useState<"installed" | "available">("installed");
  const [query, setQuery] = useState("");
  const [marketplace, setMarketplace] = useState("");
  const [loading, setLoading] = useState(true);
  const [busyKey, setBusyKey] = useState("");

  async function loadPlugins(silent = false) {
    if (!silent) setLoading(true);
    try {
      const response = await api<{ catalogs: CodexPluginCatalog[]; operations: CodexPluginOperation[] }>(
        `/api/v1/companies/${props.companyId}/codex-plugins?operation_limit=80`,
        {},
        props.token,
      );
      setCatalogs(response.catalogs);
      setOperations(response.operations);
      setRunnerId((current) => response.catalogs.some((catalog) => catalog.runner_id === current)
        ? current
        : response.catalogs[0]?.runner_id ?? "");
    } catch (error) {
      props.onError(error);
    } finally {
      if (!silent) setLoading(false);
    }
  }

  useEffect(() => { void loadPlugins(); }, [props.companyId, props.token]);
  useEffect(() => {
    if (props.realtimeEvent?.event_type.startsWith("codex.plugin.")) {
      void loadPlugins(true);
    }
  }, [props.realtimeEvent?.sequence_id]);

  async function requestOperation(operation: CodexPluginOperation["operation"], pluginId: string | null) {
    if (!runnerId) return;
    const key = `${operation}:${pluginId ?? "catalog"}`;
    setBusyKey(key);
    try {
      await api(
        `/api/v1/companies/${props.companyId}/codex-plugins/operations`,
        { method: "POST", body: JSON.stringify({ target_runner_id: runnerId, operation, plugin_id: pluginId }) },
        props.token,
      );
      props.onNotice(operation === "install" ? "安装请求已交给宿主机 Trigger" : operation === "remove" ? "卸载请求已交给宿主机 Trigger" : "插件目录刷新请求已提交");
      await loadPlugins(true);
    } catch (error) {
      props.onError(error);
    } finally {
      setBusyKey("");
    }
  }

  const catalog = catalogs.find((item) => item.runner_id === runnerId) ?? null;
  const plugins = (tab === "installed" ? catalog?.installed : catalog?.available) ?? [];
  const marketplaces = Array.from(new Set(plugins.map((plugin) => plugin.marketplaceName))).sort();
  const visible = plugins.filter((plugin) => {
    const matchesQuery = !query.trim() || `${plugin.name} ${plugin.pluginId} ${plugin.marketplaceName}`.toLowerCase().includes(query.trim().toLowerCase());
    return matchesQuery && (!marketplace || plugin.marketplaceName === marketplace);
  });
  const pluginPagination = usePagination(visible, 12, `${runnerId}:${tab}:${query}:${marketplace}`);
  const visibleOperations = operations.filter((operation) => operation.target_runner_id === runnerId);
  const operationPagination = usePagination(visibleOperations, 8, runnerId);
  const activeOperationKeys = new Set(operations
    .filter((operation) => operation.target_runner_id === runnerId && ["queued", "running"].includes(operation.status))
    .map((operation) => `${operation.operation}:${operation.plugin_id ?? "catalog"}`));

  return (
    <div className="content-stack codex-plugin-page">
      <section className="section-card">
        <div className="section-heading plugin-catalog-heading">
          <div><span className="eyebrow">PLUGIN CATALOG</span><h2>插件目录</h2><p>{catalog ? `${catalog.hostname} · ${catalog.codex_version ?? "Codex CLI"} · ${formatTime(catalog.discovered_at)}` : "等待宿主机 Trigger 上报 Codex CLI 插件目录。"}</p></div>
          <div className="section-heading-actions">
            {catalogs.length > 1 ? <select value={runnerId} onChange={(event) => setRunnerId(event.target.value)}>{catalogs.map((item) => <option key={item.runner_id} value={item.runner_id}>{item.hostname} · {item.runner_id}</option>)}</select> : null}
            <button className="button small" onClick={() => void requestOperation("refresh", null)} disabled={!runnerId || Boolean(busyKey) || activeOperationKeys.has("refresh:catalog")}><Icon name="refresh" /> 刷新目录</button>
          </div>
        </div>
        {loading ? <LoadingState /> : !catalog ? (
          <div className="empty-inline compact-empty"><Icon name="plugin" /><h3>还没有发现宿主机插件</h3><p>请确认本地 ai-chat-agent-trigger 正在运行，并且与终端使用同一个 Codex CLI 用户环境。</p></div>
        ) : (
          <>
            <div className="plugin-summary">
              <span><small>已安装</small><strong>{catalog.installed.length}</strong></span>
              <span><small>可安装</small><strong>{catalog.available.length}</strong></span>
              <span><small>Marketplace</small><strong>{catalog.marketplaces.length}</strong></span>
              <span><small>能力指纹</small><code>{catalog.fingerprint.slice(0, 12)}</code></span>
            </div>
            <div className="plugin-toolbar">
              <div className="project-detail-tabs"><button className={tab === "installed" ? "active" : ""} onClick={() => setTab("installed")}>已安装 {catalog.installed.length}</button><button className={tab === "available" ? "active" : ""} onClick={() => setTab("available")}>可安装 {catalog.available.length}</button></div>
              <div className="plugin-filters"><label><Icon name="search" /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索插件" /></label><select value={marketplace} onChange={(event) => setMarketplace(event.target.value)}><option value="">全部来源</option>{marketplaces.map((name) => <option key={name} value={name}>{name}</option>)}</select></div>
            </div>
            {visible.length ? <div className="plugin-grid">{pluginPagination.pageItems.map((plugin) => {
              const operation = tab === "installed" ? "remove" : "install";
              const operationKey = `${operation}:${plugin.pluginId}`;
              const pending = activeOperationKeys.has(operationKey);
              return <article className="plugin-card" key={plugin.pluginId}><div className="plugin-card-icon"><Icon name="plugin" /></div><div className="plugin-card-main"><div><strong>{plugin.name}</strong><span className={plugin.enabled ? "plugin-enabled" : "plugin-disabled"}>{plugin.enabled ? "已启用" : "未启用"}</span></div><code>{plugin.pluginId}</code><p><span>{plugin.marketplaceName}</span><span>v{plugin.version}</span><span>{plugin.authPolicy === "ON_INSTALL" ? "安装时认证" : plugin.authPolicy === "ON_USE" ? "使用时认证" : "无额外认证"}</span></p></div><button className={`button small ${tab === "available" ? "primary" : ""}`} onClick={() => void requestOperation(operation, plugin.pluginId)} disabled={Boolean(busyKey) || pending}>{pending || busyKey === operationKey ? "处理中…" : tab === "installed" ? "卸载" : "安装"}</button></article>;
            })}<Pagination {...pluginPagination} onPageChange={pluginPagination.setPage} /></div> : <div className="empty-inline compact-empty"><Icon name="search" /><h3>没有匹配的插件</h3><p>换一个关键词或来源筛选。</p></div>}
          </>
        )}
      </section>

      <section className="section-card">
        <div className="section-heading"><div><span className="eyebrow">OPERATIONS</span><h2>安装任务</h2><p>失败会自动重试最多 3 次；最终失败会保留宿主机 Codex CLI 返回的错误。</p></div><span className="count-badge">{operations.length}</span></div>
        {visibleOperations.length ? <div className="plugin-operation-list">{operationPagination.pageItems.map((operation) => <div key={operation.id}><span className={`status-badge ${operation.status}`}><span className="status-dot" />{codexPluginOperationStatusLabel(operation.status)}</span><strong>{operation.operation === "install" ? "安装" : operation.operation === "remove" ? "卸载" : "刷新目录"}</strong><code>{operation.plugin_id ?? operation.target_runner_id}</code><small>尝试 {operation.attempt_count}/3 · {formatTime(operation.requested_at)}</small><p>{operation.error_message ?? (operation.status === "succeeded" ? "操作完成，下一次 Agent 会话将加载最新能力。" : "等待宿主机 Trigger 处理。")}</p></div>)}<Pagination {...operationPagination} onPageChange={operationPagination.setPage} /></div> : <div className="empty-inline compact-empty"><Icon name="plugin" /><h3>还没有安装任务</h3><p>从可安装列表选择插件即可。</p></div>}
      </section>
    </div>
  );
}

function CodexRunnersView(props: {
  consoleData: CompanyConsole;
  token: string;
  realtimeEvent: CompanyRealtimeEvent | null;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const canManage = ["owner", "admin"].includes(props.consoleData.human_membership.role);
  const runnableAgents = props.consoleData.agents.filter((agent) => agent.membership.employment_status === "active");
  const configuredProjects = props.consoleData.projects.filter((project) => project.git);
  const [profiles, setProfiles] = useState<CodexRunnerProfileView[]>([]);
  const [profilesLoading, setProfilesLoading] = useState(true);
  const [environment, setEnvironment] = useState<CodexEnvironmentView | null>(null);
  const runnerPagination = usePagination(runnableAgents, 6, props.consoleData.company.id);

  async function loadProfiles() {
    try {
      const response = await api<{ profiles: CodexRunnerProfileView[] }>(
        `/api/v1/companies/${props.consoleData.company.id}/codex-runner-profiles`,
        {},
        props.token,
      );
      setProfiles(response.profiles);
    } catch (error) {
      props.onError(error);
    } finally {
      setProfilesLoading(false);
    }
  }

  async function loadEnvironment() {
    try {
      const response = await api<CodexEnvironmentView>(
        `/api/v1/companies/${props.consoleData.company.id}/codex-environments`,
        {},
        props.token,
      );
      setEnvironment(response);
    } catch (error) {
      props.onError(error);
    }
  }

  useEffect(() => {
    setProfilesLoading(true);
    void loadProfiles();
    void loadEnvironment();
  }, [props.consoleData.company.id, props.token]);

  useEffect(() => {
    const hasPendingWork = environment?.runtime.operation_status !== "idle"
      || environment?.profiles.some((profile) => ["pending", "deleting"].includes(profile.status));
    const timer = window.setTimeout(() => void loadEnvironment(), hasPendingWork ? 2_000 : 60_000);
    return () => window.clearTimeout(timer);
  }, [environment, props.consoleData.company.id, props.token]);

  if (!canManage) {
    return (
      <div className="content-stack">
        <section className="section-card">
          <div className="empty-inline">
            <Icon name="shield" />
            <h3>需要 Owner 或 Admin 权限</h3>
            <p>Codex 运行器会操作宿主机工作区，因此只有 Human Owner/Admin 可以配置和唤醒。</p>
          </div>
        </section>
      </div>
    );
  }

  return (
    <div className="content-stack">
      <section className="metric-row runner-metrics">
        <Metric label="可运行 Agent" value={String(runnableAgents.length)} detail="每个 Agent 独立会话" />
        <Metric label="运行配置" value={String(profiles.length)} detail={profiles.some((item) => item.profile.is_default) ? "已设置默认配置" : "请创建默认配置"} />
        <Metric label="已配置 Git" value={String(configuredProjects.length)} detail="具备宿主机本地目录" />
        <Metric label="执行方式" value="Codex" detail="Relay 不直接调用模型 API" />
      </section>

      <CodexRunnerProfilesPanel
        companyId={props.consoleData.company.id}
        profiles={profiles}
        loading={profilesLoading}
        token={props.token}
        authProfiles={environment?.profiles ?? []}
        onChanged={loadProfiles}
        onError={props.onError}
        onNotice={props.onNotice}
      />

      <section className="section-card">
        <div className="section-heading">
          <div>
            <span className="eyebrow">AGENT RUNNERS</span>
            <h2>Agent 运行器</h2>
          </div>
          <span className="count-badge">{runnableAgents.length}</span>
        </div>
        {runnableAgents.length ? (
          <div className="codex-runner-list">
            {runnerPagination.pageItems.map((agent) => (
              <article className="codex-runner-agent" key={agent.agent_profile.id}>
                <div className="codex-runner-agent-head">
                  <span className="agent-avatar">{agent.agent_profile.display_name.slice(0, 1).toUpperCase()}</span>
                  <div>
                    <strong>{agent.agent_profile.display_name}</strong>
                    <small>@{agent.agent_profile.handle.replace(/^@/, "")} · {agent.membership.job_title || "Agent"}</small>
                  </div>
                  <StatusBadge value={agent.connection.status} />
                </div>
                <CodexTriggerPanel
                  companyId={props.consoleData.company.id}
                  agentId={agent.agent_profile.id}
                  agentName={agent.agent_profile.display_name}
                  active
                  profiles={profiles}
                  token={props.token}
                  realtimeEvent={props.realtimeEvent}
                  onError={props.onError}
                  onNotice={props.onNotice}
                  onAssigned={loadProfiles}
                />
              </article>
            ))}
            <Pagination {...runnerPagination} onPageChange={runnerPagination.setPage} />
          </div>
        ) : (
          <div className="empty-inline">
            <Icon name="terminal" />
            <h3>还没有可运行的 Agent</h3>
            <p>请先创建并激活 Agent，再回来配置它的 Codex 定时运行器。</p>
          </div>
        )}
      </section>
    </div>
  );
}

type LocalCodexModel = {
  id: string;
  display_name: string;
  default_reasoning_effort: CodexReasoningEffort | null;
  reasoning_efforts: Array<{
    effort: CodexReasoningEffort;
    description: string;
  }>;
};

function CodexCliAuthView(props: {
  companyId: string;
  token: string;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [environment, setEnvironment] = useState<CodexEnvironmentView | null>(null);
  const [runnerProfiles, setRunnerProfiles] = useState<CodexRunnerProfileView[]>([]);
  const [loading, setLoading] = useState(true);

  async function loadEnvironment(silent = false) {
    if (!silent) setLoading(true);
    try {
      const [environmentResponse, runnerProfileResponse] = await Promise.all([
        api<CodexEnvironmentView>(`/api/v1/companies/${props.companyId}/codex-environments`, {}, props.token),
        api<{ profiles: CodexRunnerProfileView[] }>(`/api/v1/companies/${props.companyId}/codex-runner-profiles`, {}, props.token),
      ]);
      setEnvironment(environmentResponse);
      setRunnerProfiles(runnerProfileResponse.profiles);
    } catch (error) {
      props.onError(error);
    } finally {
      if (!silent) setLoading(false);
    }
  }

  useEffect(() => { void loadEnvironment(); }, [props.companyId, props.token]);

  const hasPendingWork = environment?.runtime.operation_status !== "idle"
    || environment?.profiles.some((profile) => ["pending", "deleting"].includes(profile.status));
  useEffect(() => {
    const timer = window.setTimeout(() => void loadEnvironment(true), hasPendingWork ? 2_000 : 60_000);
    return () => window.clearTimeout(timer);
  }, [hasPendingWork, environment?.runtime.updated_at, props.companyId, props.token]);

  return (
    <div className="content-stack codex-cli-auth-page">
      <CodexEnvironmentPanel
        companyId={props.companyId}
        environment={environment}
        runnerProfiles={runnerProfiles}
        loading={loading}
        token={props.token}
        onChanged={() => loadEnvironment(true)}
        onError={props.onError}
        onNotice={props.onNotice}
      />
    </div>
  );
}

function CodexEnvironmentPanel(props: {
  companyId: string;
  environment: CodexEnvironmentView | null;
  runnerProfiles: CodexRunnerProfileView[];
  loading: boolean;
  token: string;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState(CODEX_DEFAULT_OPENAI_BASE_URL);
  const [busy, setBusy] = useState("");
  const [showDefaultDetails, setShowDefaultDetails] = useState(false);
  const runtime = props.environment?.runtime;
  const configuredAuthSelectors = Array.from(new Set(props.runnerProfiles.map((item) => item.profile.codex_profile)));
  const currentAuthSelector = configuredAuthSelectors.length === 1 ? configuredAuthSelectors[0] : null;
  const hasMixedAuthSelectors = configuredAuthSelectors.length > 1;

  function authUsage(selector: string) {
    const profiles = props.runnerProfiles.filter((item) => item.profile.codex_profile === selector);
    return {
      runnerProfileCount: profiles.length,
      assignedAgentCount: profiles.reduce((total, item) => total + item.assigned_agent_count, 0),
      isCurrentDefault: currentAuthSelector === selector,
    };
  }

  async function selectDefaultAuth(selector: string) {
    if (currentAuthSelector === selector) return;
    setBusy(`select:${selector}`);
    try {
      if (props.runnerProfiles.length) {
        await Promise.all(props.runnerProfiles.map(({ profile }) => api(
          `/api/v1/companies/${props.companyId}/codex-runner-profiles/${profile.id}`,
          {
            method: "PUT",
            body: JSON.stringify({
              name: profile.name,
              interval_seconds: profile.interval_seconds,
              codex_profile: selector,
              model: profile.model,
              reasoning_effort: profile.reasoning_effort,
              reasoning_summary: profile.reasoning_summary,
              verbosity: profile.verbosity,
              personality: profile.personality,
              service_tier: profile.service_tier,
              sandbox_mode: profile.sandbox_mode,
              approval_policy: profile.approval_policy,
              network_access: profile.network_access,
              web_search: profile.web_search,
              feature_multi_agent: profile.feature_multi_agent,
              feature_remote_plugin: profile.feature_remote_plugin,
              feature_hooks: profile.feature_hooks,
              feature_goals: profile.feature_goals,
              feature_shell_tool: profile.feature_shell_tool,
              max_run_seconds: profile.max_run_seconds,
              is_default: profile.is_default,
            }),
          },
          props.token,
        )));
      } else {
        await api(`/api/v1/companies/${props.companyId}/codex-runner-profiles`, {
          method: "POST",
          body: JSON.stringify({
            name: "默认运行配置",
            interval_seconds: 3600,
            codex_profile: selector,
            model: null,
            reasoning_effort: null,
            reasoning_summary: null,
            verbosity: null,
            personality: null,
            service_tier: null,
            sandbox_mode: "inherit",
            approval_policy: "inherit",
            network_access: null,
            web_search: null,
            feature_multi_agent: null,
            feature_remote_plugin: null,
            feature_hooks: null,
            feature_goals: null,
            feature_shell_tool: null,
            max_run_seconds: 3600,
            is_default: true,
          }),
        }, props.token);
      }
      props.onNotice("默认认证已更新，所有运行器将在 Agent 下次唤醒时生效");
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy("");
    }
  }

  async function requestCliAction(action: "install" | "update") {
    if (action === "update" && !window.confirm("确认更新宿主机上的 Codex CLI？Relay 会等待当前 Agent 运行结束后再执行。")) return;
    setBusy(action);
    try {
      await api(`/api/v1/companies/${props.companyId}/codex-cli/${action}`, { method: "POST" }, props.token);
      props.onNotice(action === "install" ? "已提交 Codex CLI 安装请求" : "已提交 Codex CLI 更新请求");
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy("");
    }
  }

  async function createProfile(event: FormEvent) {
    event.preventDefault();
    setBusy("create");
    try {
      await api(
        `/api/v1/companies/${props.companyId}/codex-auth-profiles`,
        { method: "POST", body: JSON.stringify({ name, api_key: apiKey, base_url: baseUrl }) },
        props.token,
      );
      setName("");
      setApiKey("");
      setBaseUrl(CODEX_DEFAULT_OPENAI_BASE_URL);
      setCreating(false);
      props.onNotice("认证配置已提交，Trigger 正在写入独立 Codex 登录环境");
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy("");
    }
  }

  return (
    <section className="section-card codex-environment-panel">
      <div className="section-heading">
        <div>
          <span className="eyebrow">CODEX CLI & AUTH</span>
          <h2>Codex CLI 与认证环境</h2>
          <p>宿主机已有的 Codex 默认登录会直接显示并可复用；额外认证配置使用独立 CODEX_HOME，可分别设置 API Key 与 OpenAI 兼容 Base URL。</p>
        </div>
        <button className="button primary small" onClick={() => setCreating(true)} disabled={creating || !runtime?.installed}><Icon name="plus" /> 新建认证配置</button>
      </div>

      {props.loading && !runtime ? <small>正在读取宿主机 Codex 状态…</small> : runtime ? (
        <>
          <div className={`codex-cli-runtime ${runtime.update_available ? "has-update" : ""}`}>
            <div className="codex-cli-runtime-icon"><Icon name="terminal" /></div>
            <div className="codex-cli-runtime-main">
              <div><strong>{runtime.installed ? runtime.installed_version || "Codex CLI 已安装" : "尚未安装 Codex CLI"}</strong><StatusBadge value={runtime.operation_status} /></div>
              <small>{runtime.installed ? `${codexRuntimeSourceLabel(runtime.source)} · ${runtime.executable_path || "宿主机命令"}` : runtime.installation_supported ? `Trigger 将使用 ${codexInstallerLabel(runtime.installer_kind)} 安装到 Relay 托管目录。` : "当前 Trigger 宿主机不支持自动安装。"}</small>
              {runtime.last_error ? <span className="codex-runtime-error">{runtime.last_error}</span> : null}
              {runtime.update_check_error ? <small>当前无法联网检查更新，现有版本仍可继续使用。</small> : null}
            </div>
            <div className="codex-cli-version-facts">
              <span><small>当前版本</small><strong>{runtime.installed_version?.split(" ").slice(-1)[0] || "—"}</strong></span>
              <span><small>最新版本</small><strong>{runtime.latest_version || (runtime.update_check_error ? "离线" : "检查中")}</strong></span>
            </div>
            <div className="codex-cli-runtime-actions">
              {!runtime.installed ? <button className="button primary small" onClick={() => void requestCliAction("install")} disabled={!runtime.installation_supported || Boolean(busy) || runtime.operation_status !== "idle" && runtime.operation_status !== "failed"}><Icon name="download" /> {busy === "install" ? "提交中…" : "安装 Codex CLI"}</button> : null}
              {runtime.update_available ? <button className="button primary small" onClick={() => void requestCliAction("update")} disabled={Boolean(busy) || runtime.operation_status !== "idle"}><Icon name="refresh" /> {busy === "update" ? "提交中…" : `更新到 ${runtime.latest_version}`}</button> : null}
              {runtime.installed && !runtime.update_available ? <span className="codex-up-to-date">已是可检测到的最新版本</span> : null}
            </div>
          </div>
          <div className="codex-platform-support" aria-label="Codex CLI 支持平台">
            <div><small>当前 Trigger</small><strong>{codexHostPlatformLabel(runtime.host_os)} · {runtime.host_arch || "未知架构"}</strong><span>{codexInstallerLabel(runtime.installer_kind)}</span></div>
            <div className={runtime.host_os === "macos" ? "current" : ""}><Icon name="terminal" /><span><strong>macOS</strong><small>官方 install.sh</small></span></div>
            <div className={runtime.host_os === "linux" ? "current" : ""}><Icon name="terminal" /><span><strong>Linux</strong><small>官方 install.sh</small></span></div>
            <div className={runtime.host_os === "windows" ? "current" : ""}><Icon name="terminal" /><span><strong>Windows</strong><small>官方 install.ps1</small></span></div>
          </div>
        </>
      ) : null}

      {creating ? (
        <form className="codex-auth-form" onSubmit={createProfile}>
          <Field label="配置名称"><input value={name} onChange={(event) => setName(event.target.value)} placeholder="例如：团队生产账号" required maxLength={80} /></Field>
          <Field label="Base URL"><input type="url" value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} placeholder={CODEX_DEFAULT_OPENAI_BASE_URL} required /></Field>
          <Field label="OpenAI API Key"><input type="password" autoComplete="new-password" value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder="只临时交给本机 Trigger" required /></Field>
          <div className="codex-auth-form-actions"><button type="button" className="button small" onClick={() => { setCreating(false); setApiKey(""); setBaseUrl(CODEX_DEFAULT_OPENAI_BASE_URL); }} disabled={Boolean(busy)}>取消</button><button className="button primary small" disabled={Boolean(busy)}>{busy === "create" ? "正在提交…" : "创建并登录"}</button></div>
        </form>
      ) : null}

      <div className="codex-auth-current-summary">
        <span className="runner-profile-icon"><Icon name="key" /></span>
        <div><small>当前默认认证</small><strong>{currentAuthSelector ? codexAuthSelectorName(currentAuthSelector, runtime, props.environment?.profiles ?? []) : hasMixedAuthSelectors ? "多个认证环境" : "尚未设置"}</strong><code>{currentAuthSelector ?? (hasMixedAuthSelectors ? "不同运行器正在使用不同认证" : "设置后将应用到所有运行器")}</code></div>
        {currentAuthSelector ? <span className="default-badge">{props.runnerProfiles.length} 个运行器</span> : <span className="status-badge pending"><span className="status-dot" />{hasMixedAuthSelectors ? "未统一" : "未配置"}</span>}
      </div>

      <div className="codex-auth-profile-list">
        {runtime?.default_auth ? (
          <article className={`codex-auth-profile host-default ${runtime.default_auth.status === "active" ? "active" : runtime.default_auth.status === "logged_out" ? "failed" : "pending"} ${currentAuthSelector === "default" ? "current" : ""}`}>
            <div className="codex-auth-profile-main"><span className="runner-profile-icon"><Icon name="terminal" /></span><div><strong>{runtime.default_auth.name}</strong><small>default · Trigger 宿主机现有 Codex 配置</small></div></div>
            <CodexAuthUsageState usage={authUsage("default")} detail={runtime.default_auth.last_error ?? codexDefaultAuthStatusLabel(runtime.default_auth)} />
            <div className="runner-profile-actions"><button className="button small" onClick={() => setShowDefaultDetails(true)}>查看配置</button><button className={`button small ${currentAuthSelector === "default" ? "selected-action" : ""}`} onClick={() => void selectDefaultAuth("default")} disabled={currentAuthSelector === "default" || runtime.default_auth.status !== "active" || Boolean(busy)}>{currentAuthSelector === "default" ? "当前默认" : busy === "select:default" ? "切换中…" : "设为默认"}</button></div>
          </article>
        ) : null}
        {props.environment?.profiles.map((profile) => (
          <CodexAuthProfileCard key={profile.id} companyId={props.companyId} profile={profile} usage={authUsage(profile.selector)} selecting={busy === `select:${profile.selector}`} token={props.token} onSelectDefault={() => selectDefaultAuth(profile.selector)} onChanged={props.onChanged} onError={props.onError} onNotice={props.onNotice} />
        ))}
        {!props.environment?.profiles.length ? <div className="codex-auth-profile-note"><Icon name="shield" /><span><strong>当前只有宿主机默认环境</strong><small>它已经可以供运行配置选择；需要隔离多个账号时再新建认证配置。</small></span></div> : null}
      </div>
      {showDefaultDetails && runtime?.default_auth ? <CodexDefaultAuthDialog auth={runtime.default_auth} runtime={runtime} onClose={() => setShowDefaultDetails(false)} /> : null}
    </section>
  );
}

function CodexDefaultAuthDialog(props: { auth: CodexDefaultAuthEnvironment; runtime: CodexCliRuntime; onClose: () => void }) {
  const { language } = useUiLanguage();
  const config = props.auth.config;
  const safeSummary = [
    `认证环境: ${props.auth.name} (${props.auth.selector})`,
    `状态: ${codexDefaultAuthStatusLabel(props.auth)}`,
    `登录方式: ${codexDefaultAuthMethodLabel(props.auth.method)}`,
    `凭证标识: ${config.credential_hint ?? "未提供"}`,
    `OpenAI Base URL: ${config.openai_base_url ?? CODEX_DEFAULT_OPENAI_BASE_URL}`,
    `Codex Home: ${config.codex_home ?? "未发现"}`,
    `配置文件: ${config.config_path ?? "未发现"}`,
    `认证文件: ${config.auth_path ?? "未发现"}`,
    `模型 Provider: ${config.model_provider ?? "Codex 默认"}`,
    `默认模型: ${config.model ?? "Codex 默认"}`,
    `思考等级: ${config.reasoning_effort ?? "模型默认"}`,
    `MCP: ${config.mcp_servers.join(", ") || "无"}`,
    `命名 Profile: ${config.named_profiles.join(", ") || "无"}`,
  ].join("\n");
  return (
    <Dialog title="宿主机默认 Codex 配置" description="由本机 Trigger 读取安全摘要。认证信息只展示脱敏标识，不会把明文 API Key 返回给浏览器。" onClose={props.onClose} wide>
      <div className="codex-default-config-dialog">
        <div className="codex-default-config-status">
          <span className="runner-profile-icon"><Icon name="terminal" /></span>
          <div><span className="eyebrow">HOST DEFAULT ENVIRONMENT</span><strong>{props.runtime.installed_version ?? "Codex CLI"}</strong><small>{props.runtime.executable_path ?? "宿主机 Codex 命令"}</small></div>
          <StatusBadge value={props.auth.status === "active" ? "active" : props.auth.status === "logged_out" ? "failed" : "pending"} />
        </div>
        <div className="codex-default-config-grid">
          <span><small>登录方式</small><strong>{codexDefaultAuthMethodLabel(props.auth.method)}</strong></span>
          <span><small>脱敏凭证</small><code>{config.credential_hint ?? "不可用"}</code></span>
          <span><small>默认模型</small><strong>{config.model ?? "Codex 默认"}</strong></span>
          <span><small>思考等级</small><strong>{config.reasoning_effort ? codexReasoningEffortLabel(config.reasoning_effort as CodexReasoningEffort, language) : "模型默认"}</strong></span>
          <span><small>模型 Provider</small><strong>{config.model_provider ?? "默认"}</strong></span>
          <span><small>Base URL</small><code>{config.openai_base_url ?? CODEX_DEFAULT_OPENAI_BASE_URL}</code></span>
          <span><small>Sandbox</small><strong>{config.sandbox_mode ?? "运行配置决定"}</strong></span>
          <span><small>审批策略</small><strong>{config.approval_policy ?? "运行配置决定"}</strong></span>
          <span><small>最近检查</small><strong>{props.auth.last_checked_at ? formatTime(props.auth.last_checked_at) : "等待 Trigger"}</strong></span>
        </div>
        <section className="codex-default-config-section"><div><span className="eyebrow">LOCAL FILES</span><h3>本地配置位置</h3></div><dl><dt>CODEX_HOME</dt><dd><code>{config.codex_home ?? "未发现"}</code></dd><dt>config.toml</dt><dd><code>{config.config_path ?? "未发现"}</code><b>{config.config_exists ? "存在" : "不存在"}</b></dd><dt>auth.json</dt><dd><code>{config.auth_path ?? "未发现"}</code><b>{config.auth_exists ? "存在" : "不存在"}</b></dd></dl></section>
        <section className="codex-default-config-section"><div><span className="eyebrow">CAPABILITIES</span><h3>已发现配置</h3></div><div className="codex-default-config-counts"><span><small>MCP Server</small><strong>{config.mcp_servers.length}</strong></span><span><small>命名 Profile</small><strong>{config.named_profiles.length}</strong></span><span><small>可信项目</small><strong>{config.trusted_project_count}</strong></span><span><small>插件配置</small><strong>{config.plugin_count}</strong></span></div>{config.mcp_servers.length ? <div className="codex-default-config-tags">{config.mcp_servers.map((name) => <code key={name}>{name}</code>)}</div> : null}{config.named_profiles.length ? <div className="codex-default-config-tags"><small>Profiles</small>{config.named_profiles.map((name) => <code key={name}>{name}</code>)}</div> : null}</section>
        <div className="credential-warning codex-default-config-warning"><Icon name="shield" /><p><strong>明文凭证不会通过 Relay 页面展示</strong><span>Relay 只保存脱敏提示和非敏感配置摘要；实际认证仍由宿主机 Codex 自己维护。</span></p></div>
        <div className="dialog-actions"><button className="button" onClick={() => void copyText(safeSummary)}><Icon name="copy" /> 复制安全摘要</button><button className="button primary" onClick={props.onClose}>关闭</button></div>
      </div>
    </Dialog>
  );
}

type CodexAuthUsage = {
  runnerProfileCount: number;
  assignedAgentCount: number;
  isCurrentDefault: boolean;
};

function CodexAuthUsageState(props: { usage: CodexAuthUsage; detail: string }) {
  const usageLabel = props.usage.isCurrentDefault
    ? "默认"
    : props.usage.runnerProfileCount
      ? "使用中"
      : "未使用";
  return (
    <div className="codex-auth-profile-state">
      <span className={`auth-usage-badge ${props.usage.isCurrentDefault ? "current" : props.usage.runnerProfileCount ? "used" : "unused"}`}><span className="status-dot" />{usageLabel}</span>
      <span className="auth-usage-copy"><small>{props.detail}</small><b>{props.usage.runnerProfileCount ? `${props.usage.runnerProfileCount} 个运行配置 · ${props.usage.assignedAgentCount} 个 Agent` : "没有运行配置选择它"}</b></span>
    </div>
  );
}

function CodexAuthProfileCard(props: {
  companyId: string;
  profile: CodexAuthProfile;
  usage: CodexAuthUsage;
  selecting: boolean;
  token: string;
  onSelectDefault: () => Promise<void>;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(props.profile.name);
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState(props.profile.base_url ?? CODEX_DEFAULT_OPENAI_BASE_URL);
  const [busy, setBusy] = useState(false);
  useEffect(() => setName(props.profile.name), [props.profile.name]);
  useEffect(() => setBaseUrl(props.profile.base_url ?? CODEX_DEFAULT_OPENAI_BASE_URL), [props.profile.base_url]);

  async function save(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      await api(
        `/api/v1/companies/${props.companyId}/codex-auth-profiles/${props.profile.id}`,
        { method: "PUT", body: JSON.stringify({ name, api_key: apiKey || null, base_url: baseUrl }) },
        props.token,
      );
      setApiKey("");
      setEditing(false);
      props.onNotice(apiKey ? "API Key 与 Base URL 更新已提交，Trigger 将重新验证登录" : "认证配置已更新，Trigger 将同步 Base URL");
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (props.usage.runnerProfileCount) return;
    if (!window.confirm(`删除认证配置「${props.profile.name}」及其独立 Codex 登录环境？`)) return;
    setBusy(true);
    try {
      await api(`/api/v1/companies/${props.companyId}/codex-auth-profiles/${props.profile.id}`, { method: "DELETE" }, props.token);
      props.onNotice("认证配置删除已提交");
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <article className={`codex-auth-profile ${props.profile.status} ${props.usage.isCurrentDefault ? "current" : props.usage.runnerProfileCount ? "in-use" : ""}`}>
      <div className="codex-auth-profile-main"><span className="runner-profile-icon"><Icon name="key" /></span><div><strong>{props.profile.name}</strong><small>{props.profile.selector}</small><code>{props.profile.base_url ?? CODEX_DEFAULT_OPENAI_BASE_URL}</code></div></div>
      <CodexAuthUsageState usage={props.usage} detail={props.profile.last_error ?? codexAuthStatusLabel(props.profile.status)} />
      <div className="runner-profile-actions"><button className={`button small ${props.usage.isCurrentDefault ? "selected-action" : ""}`} onClick={() => void props.onSelectDefault()} disabled={props.usage.isCurrentDefault || props.profile.status !== "active" || busy || props.selecting}>{props.usage.isCurrentDefault ? "当前默认" : props.selecting ? "切换中…" : "设为默认"}</button><button className="button small" onClick={() => setEditing((value) => !value)} disabled={busy || props.profile.status === "deleting"}>编辑</button><button className="icon-button danger" onClick={() => void remove()} disabled={busy || props.profile.status === "deleting" || Boolean(props.usage.runnerProfileCount)} title={props.usage.runnerProfileCount ? "请先让运行配置改用其他认证环境" : "删除认证配置"}><Icon name="trash" /></button></div>
      {editing ? <form className="codex-auth-profile-edit" onSubmit={save}><Field label="配置名称"><input value={name} onChange={(event) => setName(event.target.value)} required maxLength={80} /></Field><Field label="Base URL"><input type="url" value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} required /></Field><Field label="新 API Key（可留空）"><input type="password" autoComplete="new-password" value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder="填写后重新登录" /></Field><div className="codex-auth-form-actions"><button type="button" className="button small" onClick={() => { setEditing(false); setApiKey(""); setBaseUrl(props.profile.base_url ?? CODEX_DEFAULT_OPENAI_BASE_URL); }}>取消</button><button className="button primary small" disabled={busy}>保存</button></div></form> : null}
    </article>
  );
}

function codexRuntimeSourceLabel(source: string) {
  if (source === "managed") return "Relay 托管安装";
  if (source === "system") return "宿主机 PATH";
  if (source === "explicit") return "自定义可执行文件";
  return "未检测到";
}

function codexHostPlatformLabel(hostOs: string) {
  if (hostOs === "macos") return "macOS";
  if (hostOs === "linux") return "Linux";
  if (hostOs === "windows") return "Windows";
  return hostOs || "未知系统";
}

function codexInstallerLabel(installer: CodexCliRuntime["installer_kind"]) {
  if (installer === "powershell") return "PowerShell 安装器";
  if (installer === "posix_shell") return "Shell 安装器";
  return "无可用安装器";
}

function codexAuthStatusLabel(status: CodexAuthProfile["status"]) {
  if (status === "active") return "登录有效，可供运行配置选择";
  if (status === "pending") return "等待本机 Trigger 验证";
  if (status === "deleting") return "等待本机 Trigger 清理";
  return "验证失败，可编辑并重新提交 API Key";
}

function codexAuthSelectorName(selector: string, runtime: CodexCliRuntime | undefined, profiles: CodexAuthProfile[]) {
  if (selector === "default") return runtime?.default_auth.name ?? "宿主机默认登录";
  return profiles.find((profile) => profile.selector === selector)?.name ?? selector;
}

function codexDefaultAuthMethodLabel(method: CodexDefaultAuthEnvironment["method"]) {
  if (method === "api_key") return "API Key 登录";
  if (method === "chatgpt") return "ChatGPT 登录";
  return "已配置登录";
}

function codexDefaultAuthStatusLabel(auth: CodexDefaultAuthEnvironment) {
  if (auth.status === "active") return `登录有效，可供运行配置选择${auth.last_checked_at ? ` · ${formatTime(auth.last_checked_at)}` : ""}`;
  if (auth.status === "logged_out") return "未检测到登录，请在 Trigger 宿主机运行 codex login";
  return "等待本机 Trigger 检查现有 Codex 登录";
}

function CodexRunnerProfilesPanel(props: {
  companyId: string;
  profiles: CodexRunnerProfileView[];
  loading: boolean;
  authProfiles: CodexAuthProfile[];
  token: string;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [creating, setCreating] = useState(false);
  const profilePagination = usePagination(props.profiles, 6, props.companyId);
  const defaultAuthSelector = props.profiles.find((item) => item.profile.is_default)?.profile.codex_profile
    ?? props.profiles[0]?.profile.codex_profile
    ?? "default";

  return (
    <section className="section-card">
      <div className="section-heading">
        <div>
          <span className="eyebrow">RUNNER PROFILES</span>
          <h2>运行配置</h2>
        </div>
        <div className="section-heading-actions">
          <button className="button primary small" onClick={() => setCreating(true)} disabled={creating}><Icon name="plus" /> 新建配置</button>
        </div>
      </div>
      {creating ? (
        <CodexRunnerProfileEditor
          companyId={props.companyId}
          profileView={null}
          authProfiles={props.authProfiles}
          initialCodexProfile={defaultAuthSelector}
          token={props.token}
          onSaved={async () => { setCreating(false); await props.onChanged(); }}
          onCancel={() => setCreating(false)}
          onError={props.onError}
          onNotice={props.onNotice}
        />
      ) : null}
      {props.loading ? <small>正在读取运行配置…</small> : props.profiles.length ? (
        <div className="runner-profile-list">
          {profilePagination.pageItems.map((profile) => (
            <CodexRunnerProfileEditor
              key={profile.profile.id}
              companyId={props.companyId}
              profileView={profile}
              authProfiles={props.authProfiles}
              initialCodexProfile={defaultAuthSelector}
              token={props.token}
              onSaved={props.onChanged}
              onCancel={() => undefined}
              onError={props.onError}
              onNotice={props.onNotice}
            />
          ))}
          <Pagination {...profilePagination} onPageChange={profilePagination.setPage} />
        </div>
      ) : !creating ? (
        <div className="empty-inline compact-empty"><Icon name="terminal" /><h3>还没有运行配置</h3><p>创建第一个配置后，它会自动成为默认配置。</p></div>
      ) : null}
    </section>
  );
}

function CodexRunnerProfileEditor(props: {
  companyId: string;
  profileView: CodexRunnerProfileView | null;
  authProfiles: CodexAuthProfile[];
  initialCodexProfile: string;
  token: string;
  onSaved: () => Promise<void>;
  onCancel: () => void;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const { language } = useUiLanguage();
  const profile = props.profileView?.profile;
  const [editing, setEditing] = useState(!profile);
  const [name, setName] = useState(profile?.name ?? "");
  const [intervalSeconds, setIntervalSeconds] = useState(profile?.interval_seconds ?? 30);
  const [codexProfile, setCodexProfile] = useState(profile?.codex_profile ?? props.initialCodexProfile);
  const [model, setModel] = useState(profile?.model ?? "");
  const [reasoningEffort, setReasoningEffort] = useState<CodexReasoningEffort | "">(profile?.reasoning_effort ?? "");
  const [reasoningSummary, setReasoningSummary] = useState<CodexReasoningSummary | "">(profile?.reasoning_summary ?? "");
  const [verbosity, setVerbosity] = useState<CodexVerbosity | "">(profile?.verbosity ?? "");
  const [personality, setPersonality] = useState<CodexPersonality | "">(profile?.personality ?? "");
  const [serviceTier, setServiceTier] = useState<"fast" | "">(profile?.service_tier ?? "");
  const [sandboxMode, setSandboxMode] = useState<CodexSandboxMode>(profile?.sandbox_mode ?? "inherit");
  const [approvalPolicy, setApprovalPolicy] = useState<CodexApprovalPolicy>(profile?.approval_policy ?? "inherit");
  const [networkAccess, setNetworkAccess] = useState<"inherit" | "true" | "false">(profile?.network_access == null ? "inherit" : String(profile.network_access) as "true" | "false");
  const [webSearch, setWebSearch] = useState<CodexWebSearch | "">(profile?.web_search ?? "");
  const [featureMultiAgent, setFeatureMultiAgent] = useState<"inherit" | "true" | "false">(codexBooleanOverride(profile?.feature_multi_agent));
  const [featureRemotePlugin, setFeatureRemotePlugin] = useState<"inherit" | "true" | "false">(codexBooleanOverride(profile?.feature_remote_plugin));
  const [featureHooks, setFeatureHooks] = useState<"inherit" | "true" | "false">(codexBooleanOverride(profile?.feature_hooks));
  const [featureGoals, setFeatureGoals] = useState<"inherit" | "true" | "false">(codexBooleanOverride(profile?.feature_goals));
  const [featureShellTool, setFeatureShellTool] = useState<"inherit" | "true" | "false">(codexBooleanOverride(profile?.feature_shell_tool));
  const [maxRunSeconds, setMaxRunSeconds] = useState(profile?.max_run_seconds ?? 3600);
  const [isDefault, setIsDefault] = useState(profile?.is_default ?? false);
  const [busy, setBusy] = useState(false);
  const [models, setModels] = useState<LocalCodexModel[]>([]);
  const [modelsLoading, setModelsLoading] = useState(false);
  const [modelsError, setModelsError] = useState("");

  async function loadModels(profileSelector = codexProfile) {
    setModelsLoading(true);
    setModelsError("");
    try {
      const response = await api<{ models: LocalCodexModel[] }>(
        `/api/v1/local-codex/models?codex_profile=${encodeURIComponent(profileSelector || "default")}`,
        {},
        props.token,
      );
      setModels(response.models);
    } catch (error) {
      setModels([]);
      setModelsError(error instanceof Error ? error.message : "无法读取这个 Codex 环境的模型列表");
    } finally {
      setModelsLoading(false);
    }
  }

  useEffect(() => {
    if (editing) void loadModels(codexProfile);
  }, [editing, codexProfile, props.token]);

  async function save(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      await api(
        profile ? `/api/v1/companies/${props.companyId}/codex-runner-profiles/${profile.id}` : `/api/v1/companies/${props.companyId}/codex-runner-profiles`,
        {
          method: profile ? "PUT" : "POST",
          body: JSON.stringify({
            name, interval_seconds: intervalSeconds, codex_profile: codexProfile || "default",
            model: model || null, reasoning_effort: reasoningEffort || null,
            reasoning_summary: reasoningSummary || null, verbosity: verbosity || null,
            personality: personality || null, service_tier: serviceTier || null,
            sandbox_mode: sandboxMode, approval_policy: approvalPolicy,
            network_access: codexBooleanOverrideValue(networkAccess), web_search: webSearch || null,
            feature_multi_agent: codexBooleanOverrideValue(featureMultiAgent),
            feature_remote_plugin: codexBooleanOverrideValue(featureRemotePlugin),
            feature_hooks: codexBooleanOverrideValue(featureHooks),
            feature_goals: codexBooleanOverrideValue(featureGoals),
            feature_shell_tool: codexBooleanOverrideValue(featureShellTool),
            max_run_seconds: maxRunSeconds, is_default: isDefault,
          }),
        },
        props.token,
      );
      setEditing(false);
      props.onNotice(profile ? `${name} 已更新，绑定的 Agent 将同步使用` : `${name} 运行配置已创建`);
      await props.onSaved();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!profile || !window.confirm(`删除运行配置「${profile.name}」？`)) return;
    setBusy(true);
    try {
      await api(`/api/v1/companies/${props.companyId}/codex-runner-profiles/${profile.id}`, { method: "DELETE" }, props.token);
      props.onNotice(`${profile.name} 已删除`);
      await props.onSaved();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  if (!editing && profile) {
    return (
      <article className={`runner-profile-card ${profile.is_default ? "default" : ""}`}>
        <div className="runner-profile-main"><span className="runner-profile-icon"><Icon name="terminal" /></span><div><strong>{profile.name}</strong><small>{profile.model || "Codex 默认模型"} · Profile: {profile.codex_profile}</small></div></div>
        <div className="runner-profile-facts">
          <span><small>兜底检查</small><strong>{formatInterval(profile.interval_seconds)}</strong></span>
          <span><small>思考等级</small><strong>{codexReasoningEffortLabel(profile.reasoning_effort, language)}</strong></span>
          <span><small>Sandbox</small><strong>{profile.sandbox_mode === "inherit" ? "继承公司" : profile.sandbox_mode === "workspace_write" ? "可写工作区" : "只读"}</strong></span>
          <span><small>审批</small><strong>{profile.approval_policy === "inherit" ? "继承公司" : profile.approval_policy === "on-request" ? "Human 审批" : "无需审批"}</strong></span>
          <span><small>运行上限</small><strong>{formatRunSeconds(profile.max_run_seconds, language)}</strong></span>
          <span><small>已绑定</small><strong>{formatAgentCount(props.profileView?.assigned_agent_count ?? 0, language)}</strong></span>
        </div>
        <div className="runner-profile-actions">{profile.is_default ? <span className="default-badge">默认</span> : null}<button className="button small" onClick={() => setEditing(true)}>编辑</button><button className="icon-button danger" title={props.profileView?.assigned_agent_count ? "请先让 Agent 改选其他配置" : "删除配置"} onClick={() => void remove()} disabled={busy || Boolean(props.profileView?.assigned_agent_count)}><Icon name="trash" /></button></div>
      </article>
    );
  }

  const currentModelMissing = Boolean(model && !models.some((item) => item.id === model));
  const selectedModel = models.find((item) => item.id === model) ?? null;
  const reasoningOptions = (selectedModel?.reasoning_efforts ?? models.flatMap((item) => item.reasoning_efforts))
    .filter((item, index, items) => items.findIndex((candidate) => candidate.effort === item.effort) === index);
  const currentReasoningMissing = Boolean(reasoningEffort && !reasoningOptions.some((item) => item.effort === reasoningEffort));
  const defaultReasoningLabel = selectedModel?.default_reasoning_effort
    ? codexReasoningEffortLabel(selectedModel.default_reasoning_effort, language)
    : "Codex 默认";
  return (
    <form className="runner-profile-form" onSubmit={save}>
      <div className="runner-profile-form-head"><div><span className="eyebrow">{profile ? "EDIT PROFILE" : "NEW PROFILE"}</span><h3>{profile ? `编辑 ${profile.name}` : "新建运行配置"}</h3></div><label className="check-row"><input type="checkbox" checked={isDefault} onChange={(event) => setIsDefault(event.target.checked)} disabled={profile?.is_default} />设为默认</label></div>
      <div className="runner-profile-fields">
        <Field label="配置名称"><input value={name} onChange={(event) => setName(event.target.value)} placeholder="例如：开发模式" required /></Field>
        <Field label="兜底检查周期（秒）"><input type="number" min={10} max={604800} value={intervalSeconds} onChange={(event) => setIntervalSeconds(Number(event.target.value))} /><small>Human 消息会即时唤醒；这里最多可设置为 7 天，只作为无消息时的兜底。</small></Field>
        <Field label="Codex 认证环境"><select value={codexProfile} onChange={(event) => { setCodexProfile(event.target.value); setModel(""); setReasoningEffort(""); }} required><option value="default">宿主机默认登录</option>{codexProfile.startsWith("relay_") && !props.authProfiles.some((item) => item.selector === codexProfile) ? <option value={codexProfile}>{codexProfile}（当前不可用）</option> : null}{props.authProfiles.map((item) => <option key={item.id} value={item.selector} disabled={item.status !== "active"}>{item.name}{item.status === "active" ? "" : `（${codexAuthStatusLabel(item.status)}）`}</option>)}</select><small>每个托管认证配置都有独立登录态、会话与本地配置。</small></Field>
        <Field label="模型"><select value={model} onChange={(event) => { const nextModel = event.target.value; setModel(nextModel); const supported = models.find((item) => item.id === nextModel)?.reasoning_efforts ?? []; if (reasoningEffort && supported.length && !supported.some((item) => item.effort === reasoningEffort)) setReasoningEffort(""); }} disabled={modelsLoading}><option value="">使用 Codex 默认模型</option>{currentModelMissing ? <option value={model}>{model}（当前配置）</option> : null}{models.map((item) => <option key={item.id} value={item.id}>{item.display_name === item.id ? item.id : `${item.display_name} · ${item.id}`}</option>)}</select>{modelsError ? <small className="codex-runtime-error">{modelsError}</small> : <small>{modelsLoading ? "正在从对应 Codex 环境读取模型…" : "模型列表由本机 Trigger 发现。"}</small>}</Field>
        <Field label="思考等级"><select value={reasoningEffort} onChange={(event) => setReasoningEffort(event.target.value as CodexReasoningEffort | "")} disabled={modelsLoading}><option value="">继承公司 / 模型默认（{defaultReasoningLabel}）</option>{currentReasoningMissing ? <option value={reasoningEffort}>{reasoningEffort}（当前配置）</option> : null}{reasoningOptions.map((item) => <option key={item.effort} value={item.effort}>{codexReasoningEffortLabel(item.effort, language)} · {item.effort}</option>)}</select></Field>
        <Field label="推理摘要"><select value={reasoningSummary} onChange={(event) => setReasoningSummary(event.target.value as CodexReasoningSummary | "")}><option value="">继承公司默认</option><option value="auto">auto</option><option value="concise">concise</option><option value="detailed">detailed</option><option value="none">none</option></select></Field>
        <Field label="输出详细度"><select value={verbosity} onChange={(event) => setVerbosity(event.target.value as CodexVerbosity | "")}><option value="">继承公司默认</option><option value="low">low</option><option value="medium">medium</option><option value="high">high</option></select></Field>
        <Field label="Personality"><select value={personality} onChange={(event) => setPersonality(event.target.value as CodexPersonality | "")}><option value="">继承公司默认</option><option value="none">none</option><option value="friendly">friendly</option><option value="pragmatic">pragmatic</option></select></Field>
        <Field label="Fast 模式"><select value={serviceTier} onChange={(event) => setServiceTier(event.target.value as "fast" | "")}><option value="">继承公司默认</option><option value="fast">开启 fast</option></select></Field>
        <Field label="Sandbox"><select value={sandboxMode} onChange={(event) => setSandboxMode(event.target.value as CodexSandboxMode)}><option value="inherit">继承公司默认</option><option value="workspace_write">workspace-write</option><option value="read_only">read-only</option></select></Field>
        <Field label="审批策略"><select value={approvalPolicy} onChange={(event) => setApprovalPolicy(event.target.value as CodexApprovalPolicy)}><option value="inherit">继承公司默认</option><option value="never">never</option><option value="on-request">on-request</option></select></Field>
        <Field label="工作区网络"><CodexBooleanOverrideSelect value={networkAccess} onChange={setNetworkAccess} /></Field>
        <Field label="Web Search"><select value={webSearch} onChange={(event) => setWebSearch(event.target.value as CodexWebSearch | "")}><option value="">继承公司默认</option><option value="disabled">关闭</option><option value="cached">缓存</option><option value="indexed">索引</option><option value="live">实时</option></select></Field>
        <Field label="多 Agent"><CodexBooleanOverrideSelect value={featureMultiAgent} onChange={setFeatureMultiAgent} /></Field>
        <Field label="插件"><CodexBooleanOverrideSelect value={featureRemotePlugin} onChange={setFeatureRemotePlugin} /></Field>
        <Field label="Hooks"><CodexBooleanOverrideSelect value={featureHooks} onChange={setFeatureHooks} /></Field>
        <Field label="Goals"><CodexBooleanOverrideSelect value={featureGoals} onChange={setFeatureGoals} /></Field>
        <Field label="Shell"><CodexBooleanOverrideSelect value={featureShellTool} onChange={setFeatureShellTool} /></Field>
        <Field label="单次最长运行（秒）"><input type="number" min={60} max={7200} value={maxRunSeconds} onChange={(event) => setMaxRunSeconds(Number(event.target.value))} /></Field>
      </div>
      <div className="runner-profile-form-actions"><button className="button small" type="button" onClick={() => { setEditing(false); props.onCancel(); }} disabled={busy}>取消</button><button className="button primary small" disabled={busy}>{busy ? "正在保存…" : "保存运行配置"}</button></div>
    </form>
  );
}

type CodexBooleanOverride = "inherit" | "true" | "false";

function codexBooleanOverride(value: boolean | null | undefined): CodexBooleanOverride {
  return value == null ? "inherit" : value ? "true" : "false";
}

function codexBooleanOverrideValue(value: CodexBooleanOverride): boolean | null {
  return value === "inherit" ? null : value === "true";
}

function CodexBooleanOverrideSelect(props: {
  value: CodexBooleanOverride;
  onChange: (value: CodexBooleanOverride) => void;
}) {
  return <select value={props.value} onChange={(event) => props.onChange(event.target.value as CodexBooleanOverride)}><option value="inherit">继承公司默认</option><option value="true">开启</option><option value="false">关闭</option></select>;
}

function MemoriesView(props: {
  consoleData: CompanyConsole;
  token: string;
  fixedAgentId?: string;
  fixedProjectId?: string;
  embedded?: boolean;
  onError: (error: unknown) => void;
  onNotice: (message: string) => void;
}) {
  const [memories, setMemories] = useState<AgentMemory[]>([]);
  const [loading, setLoading] = useState(true);
  const [query, setQuery] = useState("");
  const [agentFilter, setAgentFilter] = useState("");
  const [projectFilter, setProjectFilter] = useState("");
  const [memoryTier, setMemoryTier] = useState("");
  const [status, setStatus] = useState("");
  const [editing, setEditing] = useState<AgentMemory | null>(null);
  const companyId = props.consoleData.company.id;
  const agentId = props.fixedAgentId ?? agentFilter;
  const projectId = props.fixedProjectId ?? projectFilter;
  const scopedAgent = props.consoleData.agents.find((agent) => agent.agent_profile.id === props.fixedAgentId);
  const scopedProject = props.consoleData.projects.find((project) => project.project.id === props.fixedProjectId);

  async function loadMemories() {
    const params = new URLSearchParams({ limit: "500" });
    if (query.trim()) params.set("query", query.trim());
    if (agentId) params.set("owner_agent_id", agentId);
    if (projectId) params.set("project_id", projectId);
    if (memoryTier) params.set("memory_tier", memoryTier);
    if (status) params.set("status", status);
    const response = await api<{ memories: AgentMemory[] }>(
      `/api/v1/companies/${companyId}/memories?${params.toString()}`,
      {},
      props.token,
    );
    setMemories(response.memories);
  }

  useEffect(() => {
    let active = true;
    const timer = window.setTimeout(() => {
      setLoading(true);
      void loadMemories()
        .catch((error) => { if (active) props.onError(error); })
        .finally(() => { if (active) setLoading(false); });
    }, query ? 250 : 0);
    return () => { active = false; window.clearTimeout(timer); };
  }, [companyId, props.token, query, agentId, projectId, memoryTier, status]);

  async function updateMemory(memory: AgentMemory, values: Record<string, unknown>, notice: string) {
    try {
      await api(
        `/api/v1/companies/${companyId}/memories/${memory.id}`,
        { method: "PUT", body: JSON.stringify(values) },
        props.token,
      );
      await loadMemories();
      props.onNotice(notice);
    } catch (error) {
      props.onError(error);
      throw error;
    }
  }

  async function deleteMemory(memory: AgentMemory) {
    if (!window.confirm(`确定永久删除记忆“${memory.title}”吗？如果只是暂时不用，建议归档。`)) return;
    try {
      await api(`/api/v1/companies/${companyId}/memories/${memory.id}`, { method: "DELETE" }, props.token);
      await loadMemories();
      props.onNotice("记忆已永久删除");
    } catch (error) {
      props.onError(error);
    }
  }

  const activeCount = memories.filter((memory) => memory.status === "active").length;
  const longTermCount = memories.filter((memory) => memory.status === "active" && memory.memory_tier === "long_term").length;
  const shortTermCount = memories.filter((memory) => memory.status === "active" && memory.memory_tier === "short_term").length;
  const pinnedCount = memories.filter((memory) => memory.pinned).length;
  const agentNames = new Map(props.consoleData.agents.map((agent) => [agent.agent_profile.id, agent.agent_profile.display_name]));
  const projectNames = new Map(props.consoleData.projects.map((project) => [project.project.id, project.project.name]));
  const memoryPagination = usePagination(memories, 9, `${query}:${agentId}:${projectId}:${memoryTier}:${status}`);

  return (
    <div className={`content-stack memory-center ${props.embedded ? "embedded-memory-center" : ""}`}>
      <section className="memory-metrics">
        <Metric label="有效记忆" value={String(activeCount)} detail="全部为所属 Agent 私有" />
        <Metric label="长期记忆" value={String(longTermCount)} detail="每次唤醒自动进入 Skill" />
        <Metric label="短期记忆" value={String(shortTermCount)} detail="仅通过 MCP 按需检索" />
        <Metric label="已置顶" value={String(pinnedCount)} detail="在同类记忆中优先展示" />
      </section>

      <section className="section-card memory-library-card">
        <div className="section-heading memory-library-heading">
          <div>
            <span className="eyebrow">{scopedProject ? "PROJECT MEMORY" : "PRIVATE MEMORY"}</span>
            <h2>{scopedProject ? `${scopedProject.project.name} · 项目记忆` : scopedAgent ? `${scopedAgent.agent_profile.display_name} · 独立记忆` : "Agent 私有记忆库"}</h2>
            <p>{scopedProject ? "仅展示与当前项目关联的 Agent 精华记忆，用于保留项目决策、经验、流程和交接结论。" : scopedAgent ? "这里汇总该 Agent 的全部长期与短期记忆；其他 Agent 无权读取或复用。" : "Human 可以审阅和维护，但 Agent 只能读取自己的记忆，不能搜索或复用其他 Agent 的内容。"}</p>
          </div>
          <span className="count-badge">{memories.length}</span>
        </div>
        <div className="memory-filters">
          <label className="task-search"><Icon name="search" /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索主题、结论、标签或使用场景" /></label>
          {!props.fixedAgentId ? <select value={agentFilter} onChange={(event) => setAgentFilter(event.target.value)}><option value="">全部 Agent</option>{props.consoleData.agents.map((agent) => <option key={agent.agent_profile.id} value={agent.agent_profile.id}>{agent.agent_profile.display_name}</option>)}</select> : null}
          {!props.fixedProjectId ? <select value={projectFilter} onChange={(event) => setProjectFilter(event.target.value)}><option value="">全部项目</option>{props.consoleData.projects.map((project) => <option key={project.project.id} value={project.project.id}>{project.project.name}</option>)}</select> : null}
          <select value={memoryTier} onChange={(event) => setMemoryTier(event.target.value)}><option value="">全部类型</option><option value="long_term">长期记忆 · 自动进入 Skill</option><option value="short_term">短期记忆 · MCP 按需查询</option></select>
          <select value={status} onChange={(event) => setStatus(event.target.value)}><option value="">全部状态</option><option value="active">有效</option><option value="archived">已归档</option><option value="superseded">已替代</option></select>
        </div>

        {loading ? <div className="memory-loading"><span className="loader" />正在读取精华记忆…</div> : memories.length ? (
          <div className="memory-grid">
            {memoryPagination.pageItems.map((memory) => (
              <article className={`memory-card ${memory.status} ${memory.pinned ? "pinned" : ""}`} key={memory.id}>
                <header>
                  <div className="memory-card-kinds"><span className={`memory-tier ${memory.memory_tier}`}>{memoryTierLabel(memory.memory_tier)}</span><span className={`memory-type ${memory.memory_type}`}>{memoryTypeLabel(memory.memory_type)}</span></div>
                  <div className="memory-card-state">{memory.pinned ? <span title="已置顶">置顶</span> : null}<span className={`memory-status ${memory.status}`}>{memoryStatusLabel(memory.status)}</span></div>
                </header>
                <div className="memory-title"><h3>{memory.title}</h3><code>{memory.topic_key}</code></div>
                <p className="memory-summary">{memory.summary}</p>
                {memory.when_to_use ? <div className="memory-usage"><strong>何时使用</strong><span>{memory.when_to_use}</span></div> : null}
                {memory.tags.length ? <div className="memory-tags">{memory.tags.map((tag) => <span key={tag}>{tag}</span>)}</div> : null}
                <div className="memory-facts">
                  <span><strong>{memory.importance}/5</strong>重要度</span>
                  <span><strong>{memory.confidence}%</strong>置信度</span>
                  <span><strong>{memory.memory_tier === "long_term" ? "自动注入 Skill" : "MCP 按需检索"}</strong>{memory.project_id ? projectNames.get(memory.project_id) ?? "相关项目" : "Agent 私有"}</span>
                </div>
                <footer>
                  <div><span className="agent-avatar tiny">{(agentNames.get(memory.owner_agent_id) ?? "A").slice(0, 1)}</span><span><strong>{agentNames.get(memory.owner_agent_id) ?? "未知 Agent"}</strong><small>更新于 {formatTime(memory.updated_at)}{memory.source_refs.length ? ` · ${memory.source_refs.length} 个来源引用` : ""}</small></span></div>
                  <div className="memory-actions">
                    <button className="icon-button" title="编辑精华" onClick={() => setEditing(memory)}><Icon name="book" /></button>
                    {memory.status === "active" ? <button className="button small" onClick={() => void updateMemory(memory, { pinned: !memory.pinned }, memory.pinned ? "已取消置顶" : "记忆已置顶")}>{memory.pinned ? "取消置顶" : "置顶"}</button> : null}
                    {!['archived', 'superseded'].includes(memory.status) ? <button className="button small" onClick={() => void updateMemory(memory, { status: "archived" }, "记忆已归档")}>归档</button> : null}
                    <button className="icon-button danger" title="永久删除" onClick={() => void deleteMemory(memory)}><Icon name="trash" /></button>
                  </div>
                </footer>
              </article>
            ))}
            <Pagination {...memoryPagination} onPageChange={memoryPagination.setPage} />
          </div>
        ) : <div className="empty-inline memory-empty"><Icon name="memory" /><h3>还没有符合条件的精华记忆</h3><p>Agent 会在真实工作中提炼阶段性短期结论或稳定长期规则，再通过 <code>agent.memory</code> 写入。系统不会自动复制聊天记录。</p></div>}
      </section>

      {editing ? <MemoryEditDialog memory={editing} onClose={() => setEditing(null)} onSave={async (values) => { await updateMemory(editing, values, "记忆精华已更新"); setEditing(null); }} /> : null}
    </div>
  );
}

function MemoryEditDialog(props: { memory: AgentMemory; onClose: () => void; onSave: (values: Record<string, unknown>) => Promise<void> }) {
  const [memoryTier, setMemoryTier] = useState<AgentMemory["memory_tier"]>(props.memory.memory_tier);
  const [title, setTitle] = useState(props.memory.title);
  const [summary, setSummary] = useState(props.memory.summary);
  const [whenToUse, setWhenToUse] = useState(props.memory.when_to_use);
  const [tags, setTags] = useState(props.memory.tags.join(", "));
  const [importance, setImportance] = useState(props.memory.importance);
  const [confidence, setConfidence] = useState(props.memory.confidence);
  const [busy, setBusy] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      await props.onSave({
        memory_tier: memoryTier,
        title,
        summary,
        when_to_use: whenToUse,
        tags: tags.split(/[,，]/).map((tag) => tag.trim()).filter(Boolean),
        importance,
        confidence,
      });
    } finally {
      setBusy(false);
    }
  }

  return <Dialog title="编辑精华记忆" description={`主题键 ${props.memory.topic_key} 保持稳定，用于 Agent 去重和更新同一主题。`} onClose={props.onClose} wide><form className="stack-form memory-edit-form" onSubmit={(event) => void submit(event)}><Field label="记忆层级"><select value={memoryTier} onChange={(event) => setMemoryTier(event.target.value as AgentMemory["memory_tier"])}><option value="long_term">长期记忆 · 每次唤醒自动进入 Agent Skill</option><option value="short_term">短期记忆 · Agent 需要时通过 MCP 查询</option></select><small>只有稳定、长期指导工作的规则才应升级为长期记忆。</small></Field><Field label="标题"><input value={title} maxLength={200} onChange={(event) => setTitle(event.target.value)} required /></Field><Field label="精华结论（不是原始记录）"><textarea value={summary} minLength={10} maxLength={2000} onChange={(event) => setSummary(event.target.value)} required /></Field><Field label="何时使用"><textarea value={whenToUse} maxLength={1000} onChange={(event) => setWhenToUse(event.target.value)} placeholder="说明适用的任务、模块、条件或决策场景" /></Field><div className="form-grid"><Field label="标签（逗号分隔）"><input value={tags} onChange={(event) => setTags(event.target.value)} /></Field><Field label="重要度（1–5）"><input type="number" min={1} max={5} value={importance} onChange={(event) => setImportance(Number(event.target.value))} /></Field><Field label="置信度（0–100）"><input type="number" min={0} max={100} value={confidence} onChange={(event) => setConfidence(Number(event.target.value))} /></Field></div>{props.memory.source_refs.length ? <div className="memory-source-list"><strong>来源引用</strong>{props.memory.source_refs.map((source) => <span key={`${source.source_type}-${source.source_id}`}><b>{source.source_type}</b><code>{source.source_id}</code>{source.label ? <small>{source.label}</small> : null}</span>)}</div> : null}<div className="dialog-actions"><button className="button" type="button" onClick={props.onClose} disabled={busy}>取消</button><button className="button primary" disabled={busy}>{busy ? "保存中…" : "保存精华"}</button></div></form></Dialog>;
}

function ApprovalsView(props: {
  approvals: AgentToolApproval[];
  agents: CompanyAgent[];
  onReview: (approvalId: string, decision: "approve" | "reject", reviewNote: string) => Promise<void>;
  onError: (error: unknown) => void;
}) {
  const [filter, setFilter] = useState<"pending" | "all">("pending");
  const visible = filter === "pending" ? props.approvals.filter((approval) => approval.status === "pending") : props.approvals;
  const approvalPagination = usePagination(visible, 8, filter);
  const pendingCount = props.approvals.filter((approval) => approval.status === "pending").length;
  const completedCount = props.approvals.filter((approval) => ["approved", "executed"].includes(approval.status)).length;
  const rejectedCount = props.approvals.filter((approval) => approval.status === "rejected").length;

  return (
    <div className="content-stack">
      <section className="metric-row approval-metrics">
        <Metric label="待处理" value={String(pendingCount)} detail="Codex 会保持当前 turn 等待" />
        <Metric label="已通过" value={String(completedCount)} detail="批准后继续原进程" />
        <Metric label="已拒绝" value={String(rejectedCount)} detail="Codex 收到 decline 后继续判断" />
        <Metric label="审批来源" value="Codex + Agent" detail="统一公司级审批入口" />
      </section>
      <section className="section-card approval-center-card">
        <div className="section-heading">
          <div><span className="eyebrow">APPROVAL CENTER</span><h2>审批请求</h2><p>这里的 Codex 审批直接连接等待中的 app-server 请求，不会重新启动会话。</p></div>
          <div className="segmented approval-filter"><button className={filter === "pending" ? "active" : ""} onClick={() => setFilter("pending")}>待审批 {pendingCount}</button><button className={filter === "all" ? "active" : ""} onClick={() => setFilter("all")}>全部 {props.approvals.length}</button></div>
        </div>
        {visible.length ? <div className="approval-list">{approvalPagination.pageItems.map((approval) => <ApprovalCard key={approval.id} approval={approval} agents={props.agents} onReview={props.onReview} onError={props.onError} />)}<Pagination {...approvalPagination} onPageChange={approvalPagination.setPage} /></div> : <div className="empty-inline compact-empty"><Icon name="shield" /><h3>{filter === "pending" ? "没有待审批请求" : "还没有审批记录"}</h3><p>选择 on-request 的运行配置后，Codex 需要越权时会自动出现在这里。</p></div>}
      </section>
    </div>
  );
}

function ApprovalCard(props: {
  approval: AgentToolApproval;
  agents: CompanyAgent[];
  onReview: (approvalId: string, decision: "approve" | "reject", reviewNote: string) => Promise<void>;
  onError: (error: unknown) => void;
}) {
  const agentName = props.agents.find((agent) => agent.agent_profile.id === props.approval.requested_by_agent_id)?.agent_profile.display_name ?? "Unknown Agent";
  const detail = approvalRequestDetail(props.approval);
  return (
    <article className={`approval-card ${props.approval.status}`}>
      <div className="approval-card-head">
        <span className="approval-icon"><Icon name={props.approval.approval_source === "codex" ? "terminal" : "shield"} /></span>
        <div><strong>{approvalToolLabel(props.approval.tool_name)}</strong><small>{agentName} · {props.approval.approval_source === "codex" ? "Codex 运行审批" : "Agent 高影响动作"} · {formatTime(props.approval.created_at)}</small></div>
        <span className={`approval-status ${props.approval.status}`}>{approvalStatusLabel(props.approval.status)}</span>
      </div>
      <div className="approval-card-body">
        {props.approval.reason ? <p>{props.approval.reason}</p> : null}
        {detail ? <pre>{detail}</pre> : null}
        <div className="approval-meta"><span>风险：{approvalRiskLabel(props.approval.risk_level)}</span><span>有效期至 {formatTime(props.approval.expires_at)}</span>{props.approval.review_note ? <span>备注：{props.approval.review_note}</span> : null}</div>
      </div>
      {props.approval.status === "pending" ? <ApprovalReviewActions approvalId={props.approval.id} onReview={props.onReview} onError={props.onError} /> : null}
    </article>
  );
}

function ApprovalReviewActions(props: {
  approvalId: string;
  onReview: (approvalId: string, decision: "approve" | "reject", reviewNote: string) => Promise<void>;
  onError: (error: unknown) => void;
}) {
  const [reviewNote, setReviewNote] = useState("");
  const [busy, setBusy] = useState(false);
  async function review(decision: "approve" | "reject") {
    setBusy(true);
    try {
      await props.onReview(props.approvalId, decision, reviewNote);
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }
  return <div className="approval-actions"><input value={reviewNote} onChange={(event) => setReviewNote(event.target.value)} placeholder="审批备注（可选）" disabled={busy} /><button className="button small danger-outline" onClick={() => void review("reject")} disabled={busy}>拒绝</button><button className="button primary small" onClick={() => void review("approve")} disabled={busy}>{busy ? "处理中…" : "批准并继续"}</button></div>;
}

function ApprovalDialog(props: {
  approval: AgentToolApproval;
  agents: CompanyAgent[];
  onReview: (approvalId: string, decision: "approve" | "reject", reviewNote: string) => Promise<void>;
  onError: (error: unknown) => void;
  onClose: () => void;
}) {
  const agentName = props.agents.find((agent) => agent.agent_profile.id === props.approval.requested_by_agent_id)?.agent_profile.display_name ?? "Agent";
  return <Dialog title="Codex 正在等待审批" description={`${agentName} 的当前会话已暂停。批准或拒绝后，同一个 Codex turn 会继续执行。`} onClose={props.onClose} wide><div className="approval-dialog-content"><ApprovalCard approval={props.approval} agents={props.agents} onReview={async (...args) => { await props.onReview(...args); props.onClose(); }} onError={props.onError} /><button className="button wide" onClick={props.onClose}>稍后到审批中心处理</button></div></Dialog>;
}

function CodexTriggerPanel(props: {
  companyId: string;
  agentId: string;
  agentName: string;
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
  const [selectedProfileId, setSelectedProfileId] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const persistentThreadId = trigger?.recent_runs.find((run) => run.codex_thread_id)?.codex_thread_id ?? null;
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

  function applyTrigger(next: CodexTriggerView | null) {
    setTrigger(next);
    setSelectedProfileId(next?.runner_profile_id ?? "");
  }

  useEffect(() => {
    let active = true;
    setLoading(true);
    api<{ trigger: CodexTriggerView | null }>(endpoint, {}, props.token)
      .then(({ trigger }) => { if (active) applyTrigger(trigger); })
      .catch(props.onError)
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [endpoint, props.token]);

  useEffect(() => {
    const event = props.realtimeEvent;
    if (!event || !event.event_type.startsWith("codex.")) return;
    if (event.payload.agent_profile_id !== props.agentId) return;
    let active = true;
    api<{ trigger: CodexTriggerView | null }>(endpoint, {}, props.token)
      .then(({ trigger }) => { if (active) setTrigger(trigger); })
      .catch(() => undefined);
    return () => { active = false; };
  }, [endpoint, props.agentId, props.realtimeEvent, props.token]);

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
          <span><small>固定 Codex 会话</small><code title={persistentThreadId ?? undefined}>{persistentThreadId ?? "首次运行时创建"}</code></span>
          <span><small>当前状态</small><strong>{runningRun ? `已运行 ${formatElapsed(runningRun.started_at)}` : trigger.config.lease_owner ? "已领取，等待本地 Codex 启动" : trigger.config.manual_run_requested_at ? "手动唤醒已排队" : trigger.config.wake_requested_at ? "消息唤醒已排队" : codexTriggerStatusLabel(trigger.config.status)}</strong></span>
          <span><small>下次兜底检查</small><strong>{formatTime(trigger.config.next_run_at)}</strong></span>
          <span><small>最近成功</small><strong>{trigger.config.last_success_at ? formatTime(trigger.config.last_success_at) : "尚未成功运行"}</strong></span>
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
