import { FormEvent, ReactNode, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  analyzeRelaySkill,
  getRelaySkillDocuments,
  RELAY_EMPLOYEE_SKILL,
  RELAY_PROFESSION_SKILLS,
  RELAY_STAFFING_MANAGER_SKILL,
  RelaySkillDocument,
  RelaySkillSection,
} from "../relaySkills";

type HumanUser = {
  id: string;
  email: string;
  display_name: string;
};

type Company = {
  id: string;
  name: string;
  slug: string;
  description: string;
};

type CompanyProfession = {
  key: string;
  label: string;
  description: string;
  skill_name: string;
  can_create_tasks: boolean;
};

type OrgUnit = {
  id: string;
  parent_org_unit_id: string | null;
  name: string;
  unit_type: string;
};

type AgentProfile = {
  id: string;
  display_name: string;
  handle: string;
  persona: string;
  collaboration_preference: "available" | "low_cost_only" | "unavailable";
  status: string;
  created_at: string;
};

type AgentMembership = {
  id: string;
  agent_profile_id: string;
  org_unit_id: string;
  job_title: string;
  role_key: string;
  permissions: string[];
  responsibilities: string[];
  skills: string[];
  current_focus: string;
  staffing_scope_org_unit_id: string | null;
  employment_status: string;
  reports_to_membership_id: string | null;
};

type AgentConnection = {
  status: "connected" | "not_connected" | "awaiting_activation" | "suspended" | "terminated" | "key_revoked" | "key_expired" | "no_key";
  key_prefix: string | null;
  key_created_at: string | null;
  key_expires_at: string | null;
  last_used_at: string | null;
};

type CompanyAgent = {
  agent_profile: AgentProfile;
  membership: AgentMembership;
  profession?: CompanyProfession;
  connection: AgentConnection;
};

type Conversation = {
  preview: {
    id: string;
    title: string;
    conversation_type: "direct" | "group";
    last_message_preview: string | null;
    updated_at: string;
  };
  context: {
    context_type: string;
    project_id: string | null;
  };
  member_agent_ids: string[];
};

type Message = {
  id: string;
  sender_agent_id: string | null;
  sender_human_user_id: string | null;
  content: string;
  created_at: string;
};

type ProjectGitView = {
  remote_url: string;
  default_branch: string;
  git_host: string;
  push_enabled: boolean;
  branch_prefix: string;
  auth_configured: boolean;
};

type ProjectGitAdminView = {
  remote_url: string;
  default_branch: string;
  git_host: string;
  host_local_path: string;
  allow_agent_push: boolean;
  branch_prefix: string;
  created_at: string;
  updated_at: string;
};

type CompanyProjectTask = {
  id: string;
  project_id: string;
  title: string;
  description: string;
  status: "todo" | "in_progress" | "blocked" | "done" | "failed" | "cancelled";
  priority: "low" | "normal" | "high" | "urgent";
  assignee_agent_id: string | null;
  created_by_agent_id: string | null;
  created_by_human_user_id: string | null;
  updated_by_agent_id: string | null;
  updated_by_human_user_id: string | null;
  due_at: string | null;
  completed_at: string | null;
  created_at: string;
  updated_at: string;
};

type CompanyProject = {
  project: {
    id: string;
    name: string;
    description: string;
    status: string;
  };
  git: ProjectGitView | null;
  rule: {
    project_id: string;
    content: string;
    updated_by_agent_id: string | null;
    updated_by_human_user_id: string | null;
    created_at: string;
    updated_at: string;
  } | null;
  assets: Array<{
    id: string;
    project_id: string;
    name: string;
    asset_type: string;
    locator: string;
    description: string;
    status: "active" | "missing" | "deprecated" | "unknown";
    metadata: Record<string, unknown>;
    updated_by_agent_id: string | null;
    updated_by_human_user_id: string | null;
    created_at: string;
    updated_at: string;
  }>;
  asset_refresh: {
    project_id: string;
    maintainer_agent_id: string;
    interval_minutes: number;
    enabled: boolean;
    next_refresh_at: string;
    last_requested_at: string | null;
    last_completed_at: string | null;
    created_at: string;
    updated_at: string;
  } | null;
  members: Array<{
    member: {
      id: string;
      project_id: string;
      agent_profile_id: string;
      role: string;
      left_at: string | null;
    };
    agent_profile: AgentProfile;
  }>;
  tasks: CompanyProjectTask[];
  task_dependencies: Array<{
    id: string;
    project_id: string;
    task_id: string;
    depends_on_task_id: string;
  }>;
  task_status_history: Array<{
    id: string;
    project_id: string;
    task_id: string;
    from_status: CompanyProjectTask["status"] | null;
    to_status: CompanyProjectTask["status"];
    changed_by_agent_id: string | null;
    changed_by_human_user_id: string | null;
    change_source: "agent" | "human" | "system";
    metadata: Record<string, unknown>;
    created_at: string;
  }>;
};

type CodexTriggerRun = {
  id: string;
  trigger_type: string;
  status: string;
  project_id: string | null;
  codex_thread_id: string | null;
  started_at: string;
  finished_at: string | null;
  final_message_summary: string | null;
  error_message: string | null;
  activity_phase: string;
  activity_summary: string | null;
  last_activity_at: string | null;
  activity_log: Array<{
    at: string;
    phase: string;
    summary: string;
  }>;
};

type CodexTriggerView = {
  runner_profile_id: string | null;
  config: {
    id: string;
    status: "active" | "paused" | "error";
    interval_seconds: number;
    codex_profile: string;
    model: string | null;
    reasoning_effort: CodexReasoningEffort | null;
    sandbox_mode: "read_only" | "workspace_write";
    approval_policy: "never" | "on-request";
    max_run_seconds: number;
    next_run_at: string;
    lease_owner: string | null;
    lease_expires_at: string | null;
    manual_run_requested_at: string | null;
    wake_requested_at: string | null;
    wake_reason: string | null;
    last_run_at: string | null;
    last_success_at: string | null;
    last_error: string | null;
    consecutive_failure_count: number;
  };
  recent_runs: CodexTriggerRun[];
};

type CodexRunnerProfileView = {
  profile: {
    id: string;
    company_id: string;
    name: string;
    interval_seconds: number;
    codex_profile: string;
    model: string | null;
    reasoning_effort: CodexReasoningEffort | null;
    sandbox_mode: "read_only" | "workspace_write";
    approval_policy: "never" | "on-request";
    max_run_seconds: number;
    is_default: boolean;
    created_at: string;
    updated_at: string;
  };
  assigned_agent_count: number;
};

type AgentToolApproval = {
  id: string;
  company_id: string;
  approval_source: "runtime_model" | "codex";
  runtime_config_id: string | null;
  runtime_run_id: string | null;
  codex_trigger_run_id: string | null;
  requested_by_agent_id: string;
  tool_name: string;
  risk_level: "low" | "medium" | "high";
  reason: string;
  arguments: Record<string, unknown>;
  status: "pending" | "approved" | "executing" | "executed" | "rejected" | "expired" | "failed";
  expires_at: string;
  reviewed_by_human_user_id: string | null;
  review_note: string;
  reviewed_at: string | null;
  execution_result: Record<string, unknown>;
  error_message: string | null;
  created_at: string;
  updated_at: string;
};

type CompanyConsole = {
  company: Company;
  human_membership: { role: "owner" | "admin" | "viewer"; status: string };
  org_units: OrgUnit[];
  agents: CompanyAgent[];
  conversations: Conversation[];
  projects: CompanyProject[];
  professions: CompanyProfession[];
};

type RuntimeConfig = {
  dev_endpoints_enabled: boolean;
  email_verification_required: boolean;
};

type Session = {
  token: string;
  user: HumanUser;
};

type Credential = {
  agent: AgentProfile;
  key: string | null;
  keyPrefix: string;
  permissions: string[];
  professionKey: string;
};

type BatchCredentialResult = {
  credentials: Credential[];
  failures: Array<{ agentName: string; message: string }>;
};

type View = "agents" | "organization" | "skills" | "projects" | "tasks" | "codex" | "approvals" | "messages";

const API_BASE_URL =
  (import.meta as ImportMeta & { env?: Record<string, string> }).env?.VITE_API_BASE_URL ??
  window.location.origin;
const SESSION_KEY = "agent_company_session";
const STAFFING_PERMISSIONS = [
  { key: "agent.staff.hire", label: "扩招 Agent" },
  { key: "agent.staff.suspend", label: "暂停 Agent" },
  { key: "agent.staff.terminate", label: "裁撤 Agent" },
];
const PROJECT_PERMISSIONS = [
  { key: "project.rules.manage", label: "生成 / 更新项目 Rule" },
  { key: "project.assets.manage", label: "维护项目资产清单" },
];

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
  const [credential, setCredential] = useState<Credential | null>(null);
  const [batchCredentials, setBatchCredentials] = useState<BatchCredentialResult | null>(null);
  const [approvals, setApprovals] = useState<AgentToolApproval[]>([]);
  const [dismissedApprovalIds, setDismissedApprovalIds] = useState<Set<string>>(() => new Set());

  useEffect(() => {
    api<RuntimeConfig>("/api/v1/runtime-config")
      .then(setRuntimeConfig)
      .catch(() => setRuntimeConfig({ dev_endpoints_enabled: false, email_verification_required: false }));
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
    const timer = window.setInterval(() => void heartbeat(), 15_000);
    return () => {
      active = false;
      window.clearInterval(timer);
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
    const timer = window.setInterval(() => void refresh(), 2_000);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [session?.token, selectedCompanyId, companyConsole?.human_membership.role]);

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
                <p>查看通用协作、职业方法和特殊授权 Skill；有 Agent 时，还可以查看每个 Agent 实际使用的组合。</p>
              </div>
            </header>
            <SkillsView consoleData={companyConsole} />
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
                <h1>{view === "codex" ? "Codex 运行器" : view === "tasks" ? "任务中心" : view === "approvals" ? "审批中心" : companyConsole.company.name}</h1>
                <p>{view === "codex"
                  ? "Human 消息即时唤醒 Agent；定时检查只作为兜底，并继续宿主机上的固定 Codex 会话。"
                  : view === "tasks"
                    ? "Human 在这里创建和分配任务；Agent 通过 MCP 读取自己的任务并同步进度。"
                    : view === "approvals"
                      ? "处理 Codex 越权操作和 Agent 高影响动作；审批后原运行会继续。"
                    : companyConsole.company.description || "为外部 Agent 提供组织身份与 MCP 通信。"}</p>
              </div>
              {view === "agents" ? (
                <button className="button primary" onClick={() => setShowAgentForm(true)}>
                  <Icon name="plus" /> 创建 Agent 账号
                </button>
              ) : null}
            </header>

            {view === "agents" ? (
              <AgentsView
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
            {view === "organization" ? (
              <OrganizationView
                companyId={companyConsole.company.id}
                orgUnits={companyConsole.org_units}
                agents={companyConsole.agents}
                token={session.token}
                onChanged={refreshCompany}
                onError={showError}
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
            {view === "tasks" ? (
              <TasksView
                consoleData={companyConsole}
                token={session.token}
                onChanged={refreshCompany}
                onError={showError}
                onNotice={setNotice}
              />
            ) : null}
            {view === "codex" ? (
              <CodexRunnersView
                consoleData={companyConsole}
                token={session.token}
                onError={showError}
                onNotice={setNotice}
              />
            ) : null}
            {view === "approvals" ? (
              <ApprovalsView
                approvals={approvals}
                agents={companyConsole.agents}
                onReview={reviewApproval}
                onError={showError}
              />
            ) : null}
            {view === "messages" ? (
              <MessagesView
                consoleData={companyConsole}
                humanUser={session.user}
                token={session.token}
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
      {showAgentForm && companyConsole ? (
        <CreateAgentDialog
          company={companyConsole.company}
          orgUnits={companyConsole.org_units}
          agents={companyConsole.agents}
          professions={companyConsole.professions}
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

function AuthScreen(props: {
  runtimeConfig: RuntimeConfig | null;
  busy: boolean;
  error: string;
  setBusy: (value: boolean) => void;
  setError: (value: string) => void;
  onAuthenticated: (session: Session) => void;
}) {
  const [mode, setMode] = useState<"login" | "register">("login");
  const [email, setEmail] = useState("owner@example.com");
  const [displayName, setDisplayName] = useState("Owner");
  const [password, setPassword] = useState("password123");

  async function submit(event: FormEvent) {
    event.preventDefault();
    props.setBusy(true);
    props.setError("");
    try {
      const response = await api<{ user: HumanUser; session_token: string }>(
        mode === "login" ? "/api/v1/auth/login" : "/api/v1/auth/register",
        {
          method: "POST",
          body: JSON.stringify(
            mode === "login" ? { email, password } : { email, display_name: displayName, password },
          ),
        },
      );
      props.onAuthenticated({ token: response.session_token, user: response.user });
    } catch (requestError) {
      props.setError(requestError instanceof Error ? requestError.message : "登录失败");
    } finally {
      props.setBusy(false);
    }
  }

  async function devLogin() {
    props.setBusy(true);
    props.setError("");
    try {
      const response = await api<{ user: HumanUser; session_token: string }>("/api/v1/dev/login", {
        method: "POST",
        body: JSON.stringify({ email, display_name: displayName }),
      });
      props.onAuthenticated({ token: response.session_token, user: response.user });
    } catch (requestError) {
      props.setError(requestError instanceof Error ? requestError.message : "开发登录失败");
    } finally {
      props.setBusy(false);
    }
  }

  return (
    <div className="auth-shell">
      <section className="auth-story">
        <div className="brand-mark"><Icon name="network" /></div>
        <span className="eyebrow">Agent Company Network</span>
        <h1>让你的本地 Agent<br />真正拥有同事。</h1>
        <p>给 Codex、Claude Code 或任意外部 Agent 分配公司身份、同事目录和收件箱。它们通过 MCP 自主收发消息，而不是被平台托管运行。</p>
        <div className="auth-flow">
          <FlowStep index="1" title="创建身份" detail="公司、组织与独立 Agent Key" />
          <FlowStep index="2" title="连接 MCP" detail="粘贴一段配置到本地 Agent" />
          <FlowStep index="3" title="开始协作" detail="找同事、发消息、维护项目进度" />
        </div>
      </section>
      <section className="auth-panel">
        <form className="auth-card" onSubmit={submit}>
          <div className="segmented">
            <button type="button" className={mode === "login" ? "active" : ""} onClick={() => setMode("login")}>登录</button>
            <button type="button" className={mode === "register" ? "active" : ""} onClick={() => setMode("register")}>注册</button>
          </div>
          <div className="auth-heading">
            <h2>{mode === "login" ? "欢迎回来" : "创建管理账号"}</h2>
            <p>人类账号只负责组织与凭证治理，不替 Agent 工作。</p>
          </div>
          {mode === "register" ? (
            <Field label="你的名字">
              <input value={displayName} onChange={(event) => setDisplayName(event.target.value)} required />
            </Field>
          ) : null}
          <Field label="邮箱">
            <input type="email" value={email} onChange={(event) => setEmail(event.target.value)} required />
          </Field>
          <Field label="密码">
            <input type="password" value={password} onChange={(event) => setPassword(event.target.value)} minLength={8} required />
          </Field>
          {props.error ? <div className="inline-error">{props.error}</div> : null}
          <button className="button primary wide" disabled={props.busy}>
            {props.busy ? "请稍候…" : mode === "login" ? "进入控制台" : "创建账号"}
          </button>
          {props.runtimeConfig?.dev_endpoints_enabled ? (
            <button className="button ghost wide" type="button" onClick={() => void devLogin()} disabled={props.busy}>
              开发环境快速进入
            </button>
          ) : null}
        </form>
      </section>
    </div>
  );
}

function Sidebar(props: {
  user: HumanUser;
  companies: Company[];
  selectedCompanyId: string | null;
  view: View;
  onCompanyChange: (id: string) => void;
  onViewChange: (view: View) => void;
  pendingApprovalCount: number;
  onCreateCompany: () => void;
  onSignOut: () => void;
}) {
  return (
    <aside className="sidebar">
      <div className="brand"><span className="brand-mark small"><Icon name="network" /></span><strong>Relay</strong></div>
      <div className="company-switcher">
        <span>当前公司</span>
        <select value={props.selectedCompanyId ?? ""} onChange={(event) => props.onCompanyChange(event.target.value)} disabled={!props.companies.length}>
          {!props.companies.length ? <option value="">尚未创建公司</option> : null}
          {props.companies.map((company) => <option key={company.id} value={company.id}>{company.name}</option>)}
        </select>
        <button onClick={props.onCreateCompany}><Icon name="plus" /> 新建公司</button>
      </div>
      <nav className="side-nav">
        <NavItem icon="key" label="Agent 与凭证" active={props.view === "agents"} onClick={() => props.onViewChange("agents")} />
        <NavItem icon="org" label="组织与权限" active={props.view === "organization"} onClick={() => props.onViewChange("organization")} />
        <NavItem icon="book" label="Skill 中心" active={props.view === "skills"} onClick={() => props.onViewChange("skills")} />
        <NavItem icon="git" label="项目中心" active={props.view === "projects"} onClick={() => props.onViewChange("projects")} />
        <NavItem icon="tasks" label="任务中心" active={props.view === "tasks"} onClick={() => props.onViewChange("tasks")} />
        <NavItem icon="terminal" label="Codex 运行器" active={props.view === "codex"} onClick={() => props.onViewChange("codex")} />
        <NavItem icon="shield" label="审批中心" badge={props.pendingApprovalCount} active={props.view === "approvals"} onClick={() => props.onViewChange("approvals")} />
        <NavItem icon="message" label="通信观察" active={props.view === "messages"} onClick={() => props.onViewChange("messages")} />
      </nav>
      <div className="sidebar-spacer" />
      <div className="mcp-status"><span className="status-dot online" /><div><strong>MCP 服务</strong><small>{API_BASE_URL.replace(/^https?:\/\//, "")}/mcp</small></div></div>
      <div className="user-menu">
        <span className="avatar">{props.user.display_name.slice(0, 1).toUpperCase()}</span>
        <div><strong>{props.user.display_name}</strong><small>{props.user.email}</small></div>
        <button title="退出登录" onClick={props.onSignOut}><Icon name="logout" /></button>
      </div>
    </aside>
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
  const issuedAgents = props.consoleData.agents.filter((agent) => agent.connection.key_prefix);
  const provisioningAgents = props.consoleData.agents.filter((agent) => agent.membership.employment_status === "provisioning");
  const [batchBusy, setBatchBusy] = useState(false);

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
      <section className="connect-card">
        <div className="connect-copy">
          <span className="pill"><span className="status-dot online" /> 远程 MCP 已就绪</span>
          <h2>这里不运行 Agent，只给它们一间办公室。</h2>
          <p>每个外部 Agent 使用自己的 Key 连接 MCP。连接后，它会知道自己在哪家公司、同事是谁，并通过 Inbox 收发消息。</p>
        </div>
        <div className="connect-steps">
          <MiniStep number="01" text="创建 Agent 账号" done={props.consoleData.agents.length > 0} />
          <MiniStep number="02" text="复制 Key 与 MCP 配置" done={issuedAgents.length > 0} />
          <MiniStep number="03" text="让 Agent 完成首次连接" done={connectedAgents.length > 0} />
        </div>
      </section>

      <section className="metric-row">
        <Metric label="Agent 账号" value={String(props.consoleData.agents.length)} detail={`${connectedAgents.length} 个已连接，${Math.max(0, activeAgents.length - connectedAgents.length)} 个待连接`} />
        <Metric label="组织节点" value={String(props.consoleData.org_units.length)} detail="用于身份与授权范围" />
        <Metric label="公司会话" value={String(props.consoleData.conversations.length)} detail="私聊、群聊与项目群" />
        <Metric label="正式项目" value={String(props.consoleData.projects.length)} detail="由 Agent 通过 MCP 维护" />
      </section>

      <section className="section-card">
        <div className="section-heading">
          <div><span className="eyebrow">IDENTITIES</span><h2>Agent 账号</h2><p>凭证属于外部 Agent，不属于浏览器或平台 Runtime。</p></div>
          <div className="section-heading-actions">
            {provisioningAgents.length ? <button className="button small primary" onClick={() => void activateAllProvisioningAgents()} disabled={batchBusy}>{batchBusy ? "正在依次激活…" : `批量激活 ${provisioningAgents.length} 个`}</button> : null}
            <span className="count-badge">{props.consoleData.agents.length}</span>
          </div>
        </div>
        {props.consoleData.agents.length ? (
          <div className="agent-list">
            {props.consoleData.agents.map((agent) => (
              <AgentRow key={agent.agent_profile.id} agent={agent} {...props} />
            ))}
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
  const [expanded, setExpanded] = useState(false);
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
                {props.consoleData.professions.map((profession) => <option key={profession.key} value={profession.key}>{profession.label}</option>)}
              </select>
              {props.consoleData.professions.find((profession) => profession.key === professionKey) ? (
                <small>{props.consoleData.professions.find((profession) => profession.key === professionKey)?.description} {props.consoleData.professions.find((profession) => profession.key === professionKey)?.can_create_tasks ? "可创建、拆分和分配任务。" : "只能查看任务并更新自己任务的执行状态。"}</small>
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
    </article>
  );
}

function SkillsView({ consoleData }: { consoleData: CompanyConsole | null }) {
  const professions = useMemo<CompanyProfession[]>(() => consoleData?.professions ?? Object.entries(RELAY_PROFESSION_SKILLS).map(([key, document]) => ({
    key,
    label: document.title.replace("职业 Skill", ""),
    description: `${document.title.replace("职业 Skill", "")}的岗位工作方法、交付标准和权限边界。`,
    skill_name: document.name,
    can_create_tasks: key === "project_manager" || key === "product_manager" || key === "technical_manager",
  })), [consoleData?.professions]);
  const agents = useMemo(() => consoleData?.agents ?? [], [consoleData?.agents]);
  const [selectedAgentId, setSelectedAgentId] = useState(agents[0]?.agent_profile.id ?? "");
  const library = useMemo(() => [
    {
      category: "通用层",
      description: "所有 Agent 都会使用的公司身份、消息、任务和静默协作协议。",
      document: RELAY_EMPLOYEE_SKILL,
    },
    ...professions.map((profession) => ({
      category: "职业层",
      description: profession.description,
      document: RELAY_PROFESSION_SKILLS[profession.key] ?? RELAY_PROFESSION_SKILLS.general_member,
    })),
    {
      category: "授权层",
      description: "仅在 Human 授予人员管理权限时追加，约束招聘、暂停和裁撤动作。",
      document: RELAY_STAFFING_MANAGER_SKILL,
    },
  ], [professions]);
  const [selectedSkillName, setSelectedSkillName] = useState(RELAY_PROFESSION_SKILLS.project_manager.name);
  const [skillPreviewMode, setSkillPreviewMode] = useState<"guide" | "source">("guide");

  useEffect(() => {
    if (!agents.some((agent) => agent.agent_profile.id === selectedAgentId)) {
      setSelectedAgentId(agents[0]?.agent_profile.id ?? "");
    }
  }, [agents, selectedAgentId]);

  useEffect(() => {
    if (!library.some((item) => item.document.name === selectedSkillName)) {
      setSelectedSkillName(RELAY_EMPLOYEE_SKILL.name);
    }
  }, [library, selectedSkillName]);

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
  ) : [];
  const selectedLibraryItem = library.find((item) => item.document.name === selectedSkillName) ?? library[0];
  const selectedSkillAnalysis = useMemo(
    () => analyzeRelaySkill(selectedLibraryItem.document),
    [selectedLibraryItem.document],
  );

  return (
    <div className="content-stack">
      <section className="skill-center-hero">
        <div>
          <span className="pill"><Icon name="book" /> RELAY SKILLS</span>
          <h2>每个 Agent 都由多层 Skill 共同约束。</h2>
          <p>通用层定义公司协作协议，职业层定义岗位工作方法，授权层只在 Human 明确授权后追加。MCP 当前开放的工具与权限始终是最终边界。</p>
        </div>
        <div className="skill-layer-flow">
          <span><strong>01</strong>通用协作 Skill</span>
          <span><strong>02</strong>职业专属 Skill</span>
          <span><strong>03</strong>可选授权 Skill</span>
        </div>
      </section>

      <section className="metric-row">
        <Metric label="Skill 模板" value={String(library.length)} detail="通用、职业和授权模板" />
        <Metric label="系统职业" value={String(professions.length)} detail="职业决定任务权限和工作方法" />
        <Metric label="Agent 组合" value={String(agents.length)} detail="每个 Agent 独立生成绑定版本" />
        <Metric label="组合顺序" value="通用 → 职业" detail="有特殊授权时再追加授权 Skill" />
      </section>

      <section className="section-card">
        <div className="section-heading">
          <div>
            <span className="eyebrow">AGENT COMPOSITION</span>
            <h2>Agent 实际 Skill 组合</h2>
            <p>这里展示复制接入资料时，该 Agent 真正会拿到的绑定、权限裁剪后版本。</p>
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
            <p>选择模板查看完整原文；Agent 实际版本还会加入账号绑定并按权限裁剪。</p>
          </div>
          <span className="count-badge">{library.length}</span>
        </div>
        <div className="skill-library-layout">
          <div className="skill-library-list">
            {library.map((item) => {
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

function CodexRunnersView(props: {
  consoleData: CompanyConsole;
  token: string;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const canManage = ["owner", "admin"].includes(props.consoleData.human_membership.role);
  const runnableAgents = props.consoleData.agents.filter((agent) => agent.membership.employment_status === "active");
  const configuredProjects = props.consoleData.projects.filter((project) => project.git);
  const [profiles, setProfiles] = useState<CodexRunnerProfileView[]>([]);
  const [profilesLoading, setProfilesLoading] = useState(true);

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

  useEffect(() => {
    setProfilesLoading(true);
    void loadProfiles();
  }, [props.consoleData.company.id, props.token]);

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
      <section className="runner-hero">
        <div>
          <span className="pill"><span className="status-dot online" /> LOCAL CODEX</span>
          <h2>Relay 只做触发，工作交给 Codex。</h2>
          <p>私聊只唤醒目标 Agent，群聊唤醒群内 Agent；无消息时才按兜底周期检查。Codex 始终启动或恢复同一个固定会话，审批后原 turn 原地继续。</p>
        </div>
        <div className="runner-flow" aria-label="Codex 运行流程">
          <span><strong>01</strong>检查消息</span>
          <span><strong>02</strong>准备 Agent worktree</span>
          <span><strong>03</strong><code>thread/resume</code></span>
        </div>
      </section>

      <section className="metric-row runner-metrics">
        <Metric label="可运行 Agent" value={String(runnableAgents.length)} detail="每个 Agent 独立会话" />
        <Metric label="运行配置" value={String(profiles.length)} detail={profiles.some((item) => item.profile.is_default) ? "已设置通用默认" : "请创建默认配置"} />
        <Metric label="已配置 Git" value={String(configuredProjects.length)} detail="具备宿主机本地目录" />
        <Metric label="执行方式" value="Codex" detail="Relay 不直接调用模型 API" />
      </section>

      <CodexRunnerProfilesPanel
        companyId={props.consoleData.company.id}
        profiles={profiles}
        loading={profilesLoading}
        token={props.token}
        onChanged={loadProfiles}
        onError={props.onError}
        onNotice={props.onNotice}
      />

      <section className="section-card">
        <div className="section-heading">
          <div>
            <span className="eyebrow">AGENT RUNNERS</span>
            <h2>Agent 运行器</h2>
            <p>每个 Agent 只选择一个公司级运行配置；固定 Codex 会话和运行历史仍然彼此独立。</p>
          </div>
          <span className="count-badge">{runnableAgents.length}</span>
        </div>
        {runnableAgents.length ? (
          <div className="codex-runner-list">
            {runnableAgents.map((agent) => (
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
                  onError={props.onError}
                  onNotice={props.onNotice}
                  onAssigned={loadProfiles}
                />
              </article>
            ))}
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

type CodexReasoningEffort = "minimal" | "low" | "medium" | "high" | "xhigh" | "max" | "ultra";

type LocalCodexModel = {
  id: string;
  display_name: string;
  default_reasoning_effort: CodexReasoningEffort | null;
  reasoning_efforts: Array<{
    effort: CodexReasoningEffort;
    description: string;
  }>;
};

function CodexRunnerProfilesPanel(props: {
  companyId: string;
  profiles: CodexRunnerProfileView[];
  loading: boolean;
  token: string;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [creating, setCreating] = useState(false);
  const [models, setModels] = useState<LocalCodexModel[]>([]);
  const [modelsLoading, setModelsLoading] = useState(true);
  const [modelsError, setModelsError] = useState("");

  async function loadModels() {
    setModelsLoading(true);
    setModelsError("");
    try {
      const response = await api<{ models: LocalCodexModel[] }>(
        "/api/v1/local-codex/models?codex_profile=default",
        {},
        props.token,
      );
      setModels(response.models);
    } catch (error) {
      setModels([]);
      setModelsError(error instanceof Error ? error.message : "无法读取本地 Codex 模型列表");
    } finally {
      setModelsLoading(false);
    }
  }

  useEffect(() => { void loadModels(); }, [props.token]);

  return (
    <section className="section-card">
      <div className="section-heading">
        <div>
          <span className="eyebrow">RUNNER PROFILES</span>
          <h2>运行配置</h2>
          <p>先建立可复用配置，再由 Agent 选择。首个配置会自动成为通用默认，新 Agent 激活时自动使用。</p>
        </div>
        <div className="section-heading-actions">
          <button className="button small" onClick={() => void loadModels()} disabled={modelsLoading}><Icon name="refresh" /> {modelsLoading ? "读取模型中…" : "刷新本地模型"}</button>
          <button className="button primary small" onClick={() => setCreating(true)} disabled={creating}><Icon name="plus" /> 新建配置</button>
        </div>
      </div>
      {modelsError ? <div className="local-model-warning"><Icon name="alert" /><span><strong>本地 Codex 模型目录不可用</strong><small>{modelsError}</small></span></div> : null}
      {creating ? (
        <CodexRunnerProfileEditor
          companyId={props.companyId}
          profileView={null}
          models={models}
          modelsLoading={modelsLoading}
          token={props.token}
          onSaved={async () => { setCreating(false); await props.onChanged(); }}
          onCancel={() => setCreating(false)}
          onError={props.onError}
          onNotice={props.onNotice}
        />
      ) : null}
      {props.loading ? <small>正在读取运行配置…</small> : props.profiles.length ? (
        <div className="runner-profile-list">
          {props.profiles.map((profile) => (
            <CodexRunnerProfileEditor
              key={profile.profile.id}
              companyId={props.companyId}
              profileView={profile}
              models={models}
              modelsLoading={modelsLoading}
              token={props.token}
              onSaved={props.onChanged}
              onCancel={() => undefined}
              onError={props.onError}
              onNotice={props.onNotice}
            />
          ))}
        </div>
      ) : !creating ? (
        <div className="empty-inline compact-empty"><Icon name="terminal" /><h3>还没有运行配置</h3><p>创建第一个配置后，它会自动成为公司通用默认配置。</p></div>
      ) : null}
    </section>
  );
}

function CodexRunnerProfileEditor(props: {
  companyId: string;
  profileView: CodexRunnerProfileView | null;
  models: LocalCodexModel[];
  modelsLoading: boolean;
  token: string;
  onSaved: () => Promise<void>;
  onCancel: () => void;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const profile = props.profileView?.profile;
  const [editing, setEditing] = useState(!profile);
  const [name, setName] = useState(profile?.name ?? "");
  const [intervalSeconds, setIntervalSeconds] = useState(profile?.interval_seconds ?? 30);
  const [codexProfile, setCodexProfile] = useState(profile?.codex_profile ?? "default");
  const [model, setModel] = useState(profile?.model ?? "");
  const [reasoningEffort, setReasoningEffort] = useState<CodexReasoningEffort | "">(profile?.reasoning_effort ?? "");
  const [sandboxMode, setSandboxMode] = useState<"read_only" | "workspace_write">(profile?.sandbox_mode ?? "workspace_write");
  const [approvalPolicy, setApprovalPolicy] = useState<"never" | "on-request">(profile?.approval_policy ?? "never");
  const [maxRunSeconds, setMaxRunSeconds] = useState(profile?.max_run_seconds ?? 1800);
  const [isDefault, setIsDefault] = useState(profile?.is_default ?? false);
  const [busy, setBusy] = useState(false);

  async function save(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      await api(
        profile ? `/api/v1/companies/${props.companyId}/codex-runner-profiles/${profile.id}` : `/api/v1/companies/${props.companyId}/codex-runner-profiles`,
        {
          method: profile ? "PUT" : "POST",
          body: JSON.stringify({ name, interval_seconds: intervalSeconds, codex_profile: codexProfile || "default", model: model || null, reasoning_effort: reasoningEffort || null, sandbox_mode: sandboxMode, approval_policy: approvalPolicy, max_run_seconds: maxRunSeconds, is_default: isDefault }),
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
          <span><small>思考等级</small><strong>{codexReasoningEffortLabel(profile.reasoning_effort)}</strong></span>
          <span><small>Sandbox</small><strong>{profile.sandbox_mode === "workspace_write" ? "可写工作区" : "只读"}</strong></span>
          <span><small>审批</small><strong>{profile.approval_policy === "on-request" ? "Human 审批" : "无需审批"}</strong></span>
          <span><small>运行上限</small><strong>{profile.max_run_seconds} 秒</strong></span>
          <span><small>已绑定</small><strong>{props.profileView?.assigned_agent_count ?? 0} Agent</strong></span>
        </div>
        <div className="runner-profile-actions">{profile.is_default ? <span className="default-badge">通用默认</span> : null}<button className="button small" onClick={() => setEditing(true)}>编辑</button><button className="icon-button danger" title={props.profileView?.assigned_agent_count ? "请先让 Agent 改选其他配置" : "删除配置"} onClick={() => void remove()} disabled={busy || Boolean(props.profileView?.assigned_agent_count)}><Icon name="trash" /></button></div>
      </article>
    );
  }

  const currentModelMissing = Boolean(model && !props.models.some((item) => item.id === model));
  const selectedModel = props.models.find((item) => item.id === model) ?? null;
  const reasoningOptions = (selectedModel?.reasoning_efforts ?? props.models.flatMap((item) => item.reasoning_efforts))
    .filter((item, index, items) => items.findIndex((candidate) => candidate.effort === item.effort) === index);
  const currentReasoningMissing = Boolean(reasoningEffort && !reasoningOptions.some((item) => item.effort === reasoningEffort));
  const defaultReasoningLabel = selectedModel?.default_reasoning_effort
    ? codexReasoningEffortLabel(selectedModel.default_reasoning_effort)
    : "Codex 默认";
  return (
    <form className="runner-profile-form" onSubmit={save}>
      <div className="runner-profile-form-head"><div><span className="eyebrow">{profile ? "EDIT PROFILE" : "NEW PROFILE"}</span><h3>{profile ? `编辑 ${profile.name}` : "新建运行配置"}</h3></div><label className="check-row"><input type="checkbox" checked={isDefault} onChange={(event) => setIsDefault(event.target.checked)} disabled={profile?.is_default} />设为通用默认</label></div>
      <div className="runner-profile-fields">
        <Field label="配置名称"><input value={name} onChange={(event) => setName(event.target.value)} placeholder="例如：开发模式" required /></Field>
        <Field label="兜底检查周期（秒）"><input type="number" min={10} max={604800} value={intervalSeconds} onChange={(event) => setIntervalSeconds(Number(event.target.value))} /><small>Human 消息会即时唤醒；这里最多可设置为 7 天，只作为无消息时的兜底。</small></Field>
        <Field label="本地 Codex 配置 Profile"><input value={codexProfile} onChange={(event) => setCodexProfile(event.target.value)} placeholder="default" required /></Field>
        <Field label="模型"><select value={model} onChange={(event) => { const nextModel = event.target.value; setModel(nextModel); const supported = props.models.find((item) => item.id === nextModel)?.reasoning_efforts ?? []; if (reasoningEffort && supported.length && !supported.some((item) => item.effort === reasoningEffort)) setReasoningEffort(""); }} disabled={props.modelsLoading}><option value="">使用 Codex 默认模型</option>{currentModelMissing ? <option value={model}>{model}（当前配置）</option> : null}{props.models.map((item) => <option key={item.id} value={item.id}>{item.display_name === item.id ? item.id : `${item.display_name} · ${item.id}`}</option>)}</select></Field>
        <Field label="思考等级"><select value={reasoningEffort} onChange={(event) => setReasoningEffort(event.target.value as CodexReasoningEffort | "")} disabled={props.modelsLoading}><option value="">跟随模型默认（{defaultReasoningLabel}）</option>{currentReasoningMissing ? <option value={reasoningEffort}>{reasoningEffort}（当前配置）</option> : null}{reasoningOptions.map((item) => <option key={item.effort} value={item.effort}>{codexReasoningEffortLabel(item.effort)} · {item.effort}</option>)}</select><small>{reasoningEffort ? reasoningOptions.find((item) => item.effort === reasoningEffort)?.description || "固定使用这个思考等级。" : "使用本地 Codex 为当前模型推荐的默认等级。"}</small></Field>
        <Field label="Sandbox"><select value={sandboxMode} onChange={(event) => setSandboxMode(event.target.value as "read_only" | "workspace_write")}><option value="workspace_write">workspace-write（允许改代码）</option><option value="read_only">read-only（只读）</option></select></Field>
        <Field label="审批策略"><select value={approvalPolicy} onChange={(event) => setApprovalPolicy(event.target.value as "never" | "on-request")}><option value="never">never（无人值守）</option><option value="on-request">on-request（Human 审批）</option></select><small>{approvalPolicy === "on-request" ? "Codex 越权时暂停，并在审批中心等待处理。" : "Codex 不弹审批；被 Sandbox 拒绝的操作会直接失败。"}</small></Field>
        <Field label="单次最长运行（秒）"><input type="number" min={60} max={7200} value={maxRunSeconds} onChange={(event) => setMaxRunSeconds(Number(event.target.value))} /></Field>
      </div>
      <div className="runner-profile-form-actions"><button className="button small" type="button" onClick={() => { setEditing(false); props.onCancel(); }} disabled={busy}>取消</button><button className="button primary small" disabled={busy}>{busy ? "正在保存…" : "保存运行配置"}</button></div>
    </form>
  );
}

function ApprovalsView(props: {
  approvals: AgentToolApproval[];
  agents: CompanyAgent[];
  onReview: (approvalId: string, decision: "approve" | "reject", reviewNote: string) => Promise<void>;
  onError: (error: unknown) => void;
}) {
  const [filter, setFilter] = useState<"pending" | "all">("pending");
  const visible = filter === "pending" ? props.approvals.filter((approval) => approval.status === "pending") : props.approvals;
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
        {visible.length ? <div className="approval-list">{visible.map((approval) => <ApprovalCard key={approval.id} approval={approval} agents={props.agents} onReview={props.onReview} onError={props.onError} />)}</div> : <div className="empty-inline compact-empty"><Icon name="shield" /><h3>{filter === "pending" ? "没有待审批请求" : "还没有审批记录"}</h3><p>选择 on-request 的运行配置后，Codex 需要越权时会自动出现在这里。</p></div>}
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
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
  onAssigned: () => Promise<void>;
}) {
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
    let active = true;
    const timer = window.setInterval(() => {
      api<{ trigger: CodexTriggerView | null }>(endpoint, {}, props.token)
        .then(({ trigger }) => { if (active) setTrigger(trigger); })
        .catch(() => undefined);
    }, 3000);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [endpoint, props.token]);

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
                {props.profiles.map((item) => <option key={item.profile.id} value={item.profile.id}>{item.profile.name}{item.profile.is_default ? "（通用默认）" : ""}</option>)}
              </select>
            </Field>
            {selectedProfile ? (
              <div className="selected-profile-summary">
                <span><small>模型</small><strong>{selectedProfile.model || "Codex 默认模型"}</strong></span>
                <span><small>思考等级</small><strong>{codexReasoningEffortLabel(selectedProfile.reasoning_effort)}</strong></span>
                <span><small>兜底检查</small><strong>{formatInterval(selectedProfile.interval_seconds)}</strong></span>
                <span><small>Sandbox</small><strong>{selectedProfile.sandbox_mode === "workspace_write" ? "可写工作区" : "只读"}</strong></span>
                <span><small>审批</small><strong>{selectedProfile.approval_policy === "on-request" ? "Human 审批" : "无需审批"}</strong></span>
                <span><small>运行上限</small><strong>{selectedProfile.max_run_seconds} 秒</strong></span>
              </div>
            ) : null}
          </div>
          <div className="codex-trigger-actions">
            {trigger?.config.status === "active" ? <button className="button small" onClick={() => void triggerAction("pause")} disabled={busy}>暂停</button> : null}
            {trigger && trigger.config.status !== "active" ? <button className="button small" onClick={() => void triggerAction("resume")} disabled={busy || !props.active}>恢复</button> : null}
            {trigger ? <button className="button small" onClick={() => void triggerAction("run-now")} disabled={busy || trigger.config.status !== "active" || !props.active}>{runningRun || trigger.config.lease_owner ? "本轮后再唤醒" : trigger.config.manual_run_requested_at ? "已排队，再次请求" : "立即唤醒"}</button> : null}
            <button className="button primary small" onClick={() => void saveTrigger()} disabled={busy || !props.active || !selectedProfileId}>{busy ? "处理中…" : trigger ? "保存选择" : "启用这个配置"}</button>
          </div>
          {trigger?.config.last_error ? <div className="inline-error">最近错误：{trigger.config.last_error}</div> : null}
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
  const [activeTab, setActiveTab] = useState<"git" | "rule" | "assets">("git");
  const selectedProject = props.consoleData.projects.find((project) => project.project.id === selectedProjectId) ?? null;
  const projectStats = {
    git: props.consoleData.projects.filter((project) => project.git).length,
    assets: props.consoleData.projects.reduce((total, project) => total + project.assets.length, 0),
    refreshing: props.consoleData.projects.filter((project) => project.asset_refresh?.enabled).length,
  };

  function openProject(projectId: string, tab: "git" | "rule" | "assets") {
    setSelectedProjectId(projectId);
    setActiveTab(tab);
  }

  useEffect(() => {
    if (selectedProjectId && !selectedProject) setSelectedProjectId(null);
  }, [selectedProject, selectedProjectId]);

  if (selectedProject) {
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
                <span>{selectedProject.git ? `${selectedProject.git.git_host} · ${selectedProject.git.default_branch}` : "等待 Human 配置 Git"}</span>
              </div>
            </div>
            <span className={`git-config-state ${selectedProject.git ? "configured" : ""}`}>
              {selectedProject.git ? "已配置 Git" : "未配置 Git"}
            </span>
          </div>
          <div className="project-detail-tabs" role="tablist" aria-label="项目详情">
            <button className={activeTab === "git" ? "active" : ""} type="button" onClick={() => setActiveTab("git")}>Git 仓库</button>
            <button className={activeTab === "rule" ? "active" : ""} type="button" onClick={() => setActiveTab("rule")}>Rule</button>
            <button className={activeTab === "assets" ? "active" : ""} type="button" onClick={() => setActiveTab("assets")}>项目资产 <span>{selectedProject.assets.length}</span></button>
          </div>
          {activeTab === "git" ? (
            <ProjectGitCard
              companyId={props.consoleData.company.id}
              project={selectedProject}
              token={props.token}
              canManage={canManage}
              onChanged={props.onChanged}
              onError={props.onError}
              onNotice={props.onNotice}
            />
          ) : null}
          {activeTab === "rule" ? (
            <ProjectRuleCard
              companyId={props.consoleData.company.id}
              project={selectedProject}
              agents={props.consoleData.agents}
              token={props.token}
              canManage={canManage}
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
              canManage={canManage}
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
            <h2>项目中心</h2>
            <p>在一个入口查看项目 Git、Rule 和资产清单；可以直接进入对应模块。</p>
          </div>
          <span className="count-badge">{props.consoleData.projects.length}</span>
        </div>
        {props.consoleData.projects.length ? (
          <div className="project-overview-metrics">
            <span><small>项目</small><strong>{props.consoleData.projects.length}</strong></span>
            <span><small>已配置 Git</small><strong>{projectStats.git}</strong></span>
            <span><small>可见资产</small><strong>{projectStats.assets}</strong></span>
            <span><small>定期维护</small><strong>{projectStats.refreshing}</strong></span>
          </div>
        ) : null}
        {props.consoleData.projects.length ? (
          <div className="project-repository-list">
            {props.consoleData.projects.map((project) => (
              <div
                className="project-repository-row"
                key={project.project.id}
              >
                <button className="project-repository-main" type="button" onClick={() => openProject(project.project.id, "git")}>
                  <span className="project-repository-icon"><Icon name="git" /></span>
                  <span className="project-repository-copy">
                    <span className="project-repository-title">
                      <strong>{project.project.name}</strong>
                      <small>{projectStatusLabel(project.project.status)}</small>
                    </span>
                    <span className="project-repository-description">{project.project.description || "暂无项目说明"}</span>
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
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="empty-inline">
            <Icon name="git" />
            <h3>还没有正式项目</h3>
            <p>项目仍由具备权限的 Agent 通过 MCP 创建；项目出现后，Human 可在这里填写 Git 地址。</p>
          </div>
        )}
      </section>
    </div>
  );
}

function TasksView(props: {
  consoleData: CompanyConsole;
  token: string;
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
  const taskEntries = useMemo(
    () => props.consoleData.projects.flatMap((project) => project.tasks.map((task) => ({ project, task }))),
    [props.consoleData.projects],
  );
  const filteredTasks = useMemo(() => {
    const normalizedSearch = search.trim().toLowerCase();
    return taskEntries
      .filter(({ project, task }) => !projectFilter || project.project.id === projectFilter)
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
  }, [assigneeFilter, projectFilter, search, statusFilter, taskEntries]);
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
            <span className="eyebrow">COMPANY TASKS</span>
            <h2>项目任务</h2>
            <p>任务被分配后会写入 Agent Inbox，并在下次定时 Trigger 中唤醒它的固定 Codex 会话。</p>
          </div>
          <div className="section-heading-actions">
            <span className="count-badge">{filteredTasks.length}</span>
            {canManage ? (
              <button className="button primary small" type="button" onClick={() => setShowCreate(true)} disabled={!props.consoleData.projects.length}>
                <Icon name="plus" /> 新建任务
              </button>
            ) : null}
          </div>
        </div>

        <div className="task-filters">
          <label className="task-search">
            <Icon name="search" />
            <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="搜索任务或项目" />
          </label>
          <select value={projectFilter} onChange={(event) => setProjectFilter(event.target.value)}>
            <option value="">全部项目</option>
            {props.consoleData.projects.map((project) => <option key={project.project.id} value={project.project.id}>{project.project.name}</option>)}
          </select>
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
            {filteredTasks.map(({ project, task }) => {
              const dependencies = project.task_dependencies
                .filter((dependency) => dependency.task_id === task.id)
                .map((dependency) => project.tasks.find((candidate) => candidate.id === dependency.depends_on_task_id))
                .filter((dependency): dependency is CompanyProjectTask => Boolean(dependency));
              const unresolvedDependencies = dependencies.filter((dependency) => !["done", "cancelled"].includes(dependency.status));
              return (
                <div
                  className={`task-row ${canManage ? "editable" : ""}`}
                  key={task.id}
                  role={canManage ? "button" : undefined}
                  tabIndex={canManage ? 0 : undefined}
                  onClick={() => { if (canManage) setEditingTaskId(task.id); }}
                  onKeyDown={(event) => { if (canManage && (event.key === "Enter" || event.key === " ")) setEditingTaskId(task.id); }}
                >
                  <span className="task-title-cell">
                    <strong>{task.title}</strong>
                    <small>{project.project.name}{dependencies.length ? ` · ${dependencies.length} 个前置任务` : ""}</small>
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
          projects={props.consoleData.projects}
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
          projects={props.consoleData.projects}
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
            {dependencyCandidates.length ? dependencyCandidates.map((candidate) => (
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
            {statusHistory.map((entry) => {
              const actor = entry.changed_by_agent_id
                ? activeMembers.find((member) => member.agent_profile.id === entry.changed_by_agent_id)?.agent_profile.display_name ?? "Agent"
                : entry.changed_by_human_user_id ? "Human" : "系统";
              return <span key={entry.id}><small>{formatTime(entry.created_at)}</small><em>{entry.from_status ? `${taskStatusLabel(entry.from_status)} → ` : "初始状态："}{taskStatusLabel(entry.to_status)}</em><i>{actor}</i></span>;
            })}
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
      <form className="project-rule-editor" onSubmit={saveRule}>
        <div className="project-tab-heading">
          <div><span className="eyebrow">PROJECT RULE</span><h3>项目注意事项</h3><p>这里的 Markdown 会作为项目上下文提供给项目 Agent，适合记录约束、规范、风险和协作规则。</p></div>
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
              {filteredAssets.map((asset) => {
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
  const [branchPrefix, setBranchPrefix] = useState(props.project.git?.branch_prefix ?? "relay/");
  const [allowAgentPush, setAllowAgentPush] = useState(props.project.git?.push_enabled ?? false);
  const [configured, setConfigured] = useState(Boolean(props.project.git));
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!props.canManage) return;
    let active = true;
    api<{ git: ProjectGitAdminView | null; github_token_configured: boolean }>(
      `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/git`,
      {},
      props.token,
    )
      .then(({ git, github_token_configured }) => {
        if (!active) return;
        setRemoteUrl(git?.remote_url ?? "");
        setHostLocalPath(git?.host_local_path ?? "");
        setDefaultBranch(git?.default_branch ?? "main");
        setGithubToken("");
        setGithubTokenConfigured(github_token_configured);
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
      const response = await api<{ git: ProjectGitAdminView; github_token_configured: boolean }>(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/git`,
        {
          method: "PUT",
          body: JSON.stringify({
            remote_url: remoteUrl,
            host_local_path: hostLocalPath,
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
      const response = await api<{ git: ProjectGitAdminView; github_token_configured: boolean }>(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/git`,
        {
          method: "PUT",
          body: JSON.stringify({
            remote_url: remoteUrl,
            host_local_path: hostLocalPath,
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
          <Field label="宿主机项目根目录">
            <input
              value={hostLocalPath}
              onChange={(event) => setHostLocalPath(event.target.value)}
              placeholder="/Users/runner/relay-projects/my-project"
              required
            />
          </Field>
          <div className="form-grid">
            <Field label="默认分支">
              <input value={defaultBranch} onChange={(event) => setDefaultBranch(event.target.value)} placeholder="main" />
            </Field>
            <Field label="Agent 分支前缀">
              <input value={branchPrefix} onChange={(event) => setBranchPrefix(event.target.value)} placeholder="relay/" />
            </Field>
          </div>
          <Field label="GitHub Token（私有仓库或需要 Push 时填写）">
            <input
              type="password"
              autoComplete="new-password"
              value={githubToken}
              onChange={(event) => setGithubToken(event.target.value)}
              placeholder={githubTokenConfigured ? "Token 已保存，留空表示不修改" : "github_pat_..."}
            />
          </Field>
          {githubTokenConfigured ? (
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
            GitHub 地址请使用 HTTPS，例如 <code>https://github.com/owner/repository.git</code>。Token 只保存在宿主机本地凭证目录，不写入项目数据库，也不会通过 MCP 返回。公开仓库可以不填 Token。
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

function OrganizationView(props: {
  companyId: string;
  orgUnits: OrgUnit[];
  agents: CompanyAgent[];
  token: string;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
}) {
  const [name, setName] = useState("");
  const [parentId, setParentId] = useState(props.orgUnits[0]?.id ?? "");
  const [unitType, setUnitType] = useState("team");
  const [busy, setBusy] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      await api(`/api/v1/companies/${props.companyId}/org-units`, {
        method: "POST",
        body: JSON.stringify({ name, parent_org_unit_id: parentId || null, unit_type: unitType }),
      }, props.token);
      setName("");
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="organization-layout">
      <section className="section-card">
        <div className="section-heading"><div><span className="eyebrow">DIRECTORY</span><h2>组织目录</h2><p>Agent 通过 MCP 读取这份目录来理解同事关系。</p></div></div>
        <div className="org-list">
          {props.orgUnits.map((unit) => {
            const count = props.agents.filter((agent) => agent.membership.org_unit_id === unit.id).length;
            const parent = props.orgUnits.find((item) => item.id === unit.parent_org_unit_id);
            return <div className="org-row" key={unit.id}><span className="org-icon"><Icon name="org" /></span><div><strong>{unit.name}</strong><small>{parent ? `${parent.name} / ` : ""}{unit.unit_type}</small></div><span>{count} Agent</span></div>;
          })}
        </div>
      </section>
      <section className="section-card compact-card">
        <div className="section-heading"><div><span className="eyebrow">NEW UNIT</span><h2>添加组织节点</h2></div></div>
        <form className="stack-form" onSubmit={submit}>
          <Field label="名称"><input value={name} onChange={(event) => setName(event.target.value)} placeholder="例如：产品组" required /></Field>
          <Field label="上级节点"><select value={parentId} onChange={(event) => setParentId(event.target.value)}>{props.orgUnits.map((unit) => <option key={unit.id} value={unit.id}>{unit.name}</option>)}</select></Field>
          <Field label="类型"><select value={unitType} onChange={(event) => setUnitType(event.target.value)}><option value="division">事业部</option><option value="department">部门</option><option value="team">团队</option></select></Field>
          <button className="button primary wide" disabled={busy}>{busy ? "创建中…" : "创建节点"}</button>
        </form>
      </section>
    </div>
  );
}

function MessagesView(props: {
  consoleData: CompanyConsole;
  humanUser: HumanUser;
  token: string;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (message: string) => void;
}) {
  const [selectedId, setSelectedId] = useState(props.consoleData.conversations[0]?.preview.id ?? "");
  const [messages, setMessages] = useState<Message[]>([]);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [showNewDirect, setShowNewDirect] = useState(false);
  const [showGroupMembers, setShowGroupMembers] = useState(false);
  const [targetAgentId, setTargetAgentId] = useState("");
  const [mentionedAgentIds, setMentionedAgentIds] = useState<string[]>([]);
  const [mentionAll, setMentionAll] = useState(false);
  const [mentionQuery, setMentionQuery] = useState<string | null>(null);
  const messageStreamRef = useRef<HTMLDivElement | null>(null);
  const shouldScrollToLatestRef = useRef(true);
  const isNearLatestRef = useRef(true);
  const selected = props.consoleData.conversations.find((item) => item.preview.id === selectedId);
  const canSend = props.consoleData.human_membership.status === "active"
    && ["owner", "admin"].includes(props.consoleData.human_membership.role);
  const activeAgents = props.consoleData.agents.filter((agent) => agent.membership.employment_status === "active");
  const agentNames = useMemo(() => new Map(props.consoleData.agents.map((agent) => [agent.agent_profile.id, agent.agent_profile.display_name])), [props.consoleData.agents]);
  const agentDirectory = useMemo(() => new Map(props.consoleData.agents.map((agent) => [agent.agent_profile.id, agent])), [props.consoleData.agents]);
  const selectedGroupAgents = (selected?.member_agent_ids ?? []).flatMap((agentId) => {
    const agent = agentDirectory.get(agentId);
    return agent ? [agent] : [];
  });
  const selectedIsGroup = selected?.preview.conversation_type === "group";
  const mentionCandidates = selectedIsGroup && mentionQuery !== null
    ? selectedGroupAgents.filter((agent) => {
      const query = mentionQuery.trim().toLocaleLowerCase();
      return !query
        || agent.agent_profile.display_name.toLocaleLowerCase().includes(query)
        || agent.agent_profile.handle.replace(/^@/, "").toLocaleLowerCase().includes(query);
    })
    : [];
  const showMentionAllCandidate = mentionQuery !== null
    && (!mentionQuery.trim() || "所有人".includes(mentionQuery.trim()));

  async function fetchMessages(conversationId: string) {
    const response = await api<{ messages: Message[] }>(`/api/v1/conversations/${conversationId}/messages?limit=100`, {}, props.token);
    return response.messages;
  }

  function replaceMessages(nextMessages: Message[]) {
    setMessages((current) => current.length === nextMessages.length
      && current.every((message, index) => message.id === nextMessages[index]?.id)
      ? current
      : nextMessages);
  }

  useEffect(() => {
    setSelectedId((current) => current && props.consoleData.conversations.some((item) => item.preview.id === current)
      ? current
      : props.consoleData.conversations[0]?.preview.id ?? "");
  }, [props.consoleData.conversations]);

  useEffect(() => {
    setShowGroupMembers(false);
    setMentionedAgentIds([]);
    setMentionAll(false);
    setMentionQuery(null);
    shouldScrollToLatestRef.current = true;
    isNearLatestRef.current = true;
    if (!selectedId) {
      setMessages([]);
      return;
    }
    let active = true;
    const refreshMessages = async (reportError: boolean) => {
      try {
        const nextMessages = await fetchMessages(selectedId);
        if (active) replaceMessages(nextMessages);
      } catch (error) {
        if (active && reportError) props.onError(error);
      }
    };
    void refreshMessages(true);
    const timer = window.setInterval(() => void refreshMessages(false), 3_000);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [selectedId, props.token]);

  const latestMessageId = messages[messages.length - 1]?.id ?? "";
  useLayoutEffect(() => {
    const stream = messageStreamRef.current;
    if (!stream || (!shouldScrollToLatestRef.current && !isNearLatestRef.current)) return;
    stream.scrollTop = stream.scrollHeight;
    shouldScrollToLatestRef.current = false;
    isNearLatestRef.current = true;
  }, [selectedId, messages.length, latestMessageId]);

  function trackMessageScroll() {
    const stream = messageStreamRef.current;
    if (!stream) return;
    isNearLatestRef.current = stream.scrollHeight - stream.scrollTop - stream.clientHeight <= 80;
  }

  async function openDirect(event: FormEvent) {
    event.preventDefault();
    if (!targetAgentId) return;
    setBusy(true);
    try {
      const response = await api<{ conversation: Conversation }>(`/api/v1/companies/${props.consoleData.company.id}/conversations/direct`, {
        method: "POST",
        body: JSON.stringify({ target_agent_id: targetAgentId }),
      }, props.token);
      setSelectedId(response.conversation.preview.id);
      setShowNewDirect(false);
      setTargetAgentId("");
      await props.onChanged();
      props.onNotice("Human 私聊已建立，消息会在 Agent 下次 Trigger 时被发现。");
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function sendMessage(event: FormEvent) {
    event.preventDefault();
    if (!selectedId || !draft.trim()) return;
    setBusy(true);
    try {
      const effectiveMentionAll = selectedIsGroup && mentionAll && draft.includes("@所有人");
      const effectiveMentionedAgentIds = effectiveMentionAll ? [] : mentionedAgentIds.filter((agentId) => {
        const agent = agentDirectory.get(agentId);
        if (!agent) return false;
        return draft.includes(`@${agent.agent_profile.display_name}`)
          || draft.includes(`@${agent.agent_profile.handle.replace(/^@/, "")}`);
      });
      await api(`/api/v1/companies/${props.consoleData.company.id}/conversations/${selectedId}/messages`, {
        method: "POST",
        body: JSON.stringify({
          content: draft,
          mentioned_agent_ids: effectiveMentionedAgentIds,
          mention_all: effectiveMentionAll,
        }),
      }, props.token);
      setDraft("");
      setMentionedAgentIds([]);
      setMentionAll(false);
      setMentionQuery(null);
      shouldScrollToLatestRef.current = true;
      const [nextMessages] = await Promise.all([fetchMessages(selectedId), props.onChanged()]);
      replaceMessages(nextMessages);
      props.onNotice(selected?.preview.conversation_type === "direct"
        ? "消息已发送，目标 Agent 正在被即时唤醒。"
        : effectiveMentionAll || !effectiveMentionedAgentIds.length
          ? "群消息已发送，群内 Agent 正在被即时唤醒。"
          : `群消息已发送，仅即时唤醒 ${effectiveMentionedAgentIds.length} 个被 @ 的 Agent。`);
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  function updateDraft(value: string) {
    setDraft(value);
    if (!selectedIsGroup) {
      setMentionQuery(null);
      return;
    }
    const match = value.match(/(?:^|\s)@([^\s@]*)$/u);
    setMentionQuery(match ? match[1] : null);
    if (!value.includes("@所有人")) setMentionAll(false);
    setMentionedAgentIds((current) => current.filter((agentId) => {
      const agent = agentDirectory.get(agentId);
      return Boolean(agent && (
        value.includes(`@${agent.agent_profile.display_name}`)
        || value.includes(`@${agent.agent_profile.handle.replace(/^@/, "")}`)
      ));
    }));
  }

  function insertMention(agent?: CompanyAgent) {
    const mentionText = agent ? agent.agent_profile.display_name : "所有人";
    const match = draft.match(/(?:^|\s)@([^\s@]*)$/u);
    const mentionStart = match ? (match.index ?? 0) + match[0].lastIndexOf("@") : -1;
    const nextDraft = mentionStart >= 0
      ? `${draft.slice(0, mentionStart)}@${mentionText} `
      : `${draft}${draft && !draft.endsWith(" ") ? " " : ""}@${mentionText} `;
    setDraft(nextDraft);
    setMentionQuery(null);
    if (agent) {
      setMentionAll(false);
      setMentionedAgentIds((current) => current.includes(agent.agent_profile.id)
        ? current
        : [...current, agent.agent_profile.id]);
    } else {
      setMentionAll(true);
      setMentionedAgentIds([]);
    }
  }

  function removeMention(agentId?: string) {
    if (!agentId) {
      setMentionAll(false);
      setDraft((current) => current.replace(/@所有人\s*/gu, ""));
      return;
    }
    const agent = agentDirectory.get(agentId);
    setMentionedAgentIds((current) => current.filter((id) => id !== agentId));
    if (!agent) return;
    const tokens = [agent.agent_profile.display_name, agent.agent_profile.handle.replace(/^@/, "")]
      .map(escapeRegExp)
      .join("|");
    setDraft((current) => current.replace(new RegExp(`@(?:${tokens})\\s*`, "gu"), ""));
  }

  return (
    <>
      <section className="message-console">
        <div className="conversation-list">
          <div className="conversation-list-head">
            <div><span className="eyebrow">CONVERSATIONS</span><strong>公司通信</strong></div>
            {canSend ? <button className="new-direct-button" type="button" onClick={() => setShowNewDirect(true)}><Icon name="plus" /> 新建私聊</button> : null}
          </div>
          {props.consoleData.conversations.map((conversation) => (
            <button key={conversation.preview.id} className={selectedId === conversation.preview.id ? "active" : ""} onClick={() => setSelectedId(conversation.preview.id)}>
              <span className="conversation-icon"><Icon name={conversation.preview.conversation_type === "group" ? "group" : "message"} /></span>
              <span><strong>{conversationDisplayTitle(conversation, agentNames)}</strong><small>{conversation.preview.last_message_preview ?? "暂无消息"}</small></span>
            </button>
          ))}
          {!props.consoleData.conversations.length ? <div className="conversation-list-empty">还没有会话，可以先与一个 Agent 建立私聊。</div> : null}
        </div>
        <div className="message-panel">
          <div className="message-head">
            <div><strong>{conversationDisplayTitle(selected, agentNames) || "选择一个会话"}</strong><small>{selected ? `${selected.member_agent_ids.length} 个 Agent · Human 可发送消息` : "从左侧选择或新建会话"}</small></div>
            <div className="message-head-actions">
              {selectedIsGroup ? <button className="group-members-button" type="button" onClick={() => setShowGroupMembers(true)}><Icon name="group" /> 群成员 <span>{selected.member_agent_ids.length}</span></button> : null}
              {selected ? <span className="pill neutral">{formatConversationContext(selected.context.context_type)}</span> : null}
            </div>
          </div>
          <div className="message-stream" ref={messageStreamRef} onScroll={trackMessageScroll}>
            {messages.length ? messages.map((message) => {
              const isHuman = message.sender_human_user_id !== null;
              const senderName = isHuman ? props.humanUser.display_name : agentNames.get(message.sender_agent_id ?? "") ?? "Unknown Agent";
              return <div className={`message-item ${isHuman ? "human" : ""}`} key={message.id}><span className="agent-avatar small">{senderName.slice(0, 1)}</span><div><div><strong>{senderName}{isHuman ? " · Human" : ""}</strong><time>{formatTime(message.created_at)}</time></div><p>{renderMessageContent(message.content, selectedGroupAgents)}</p></div></div>;
            }) : <div className="empty-messages">{selected ? "这段会话还没有消息。" : "请选择一个会话。"}</div>}
          </div>
          {canSend ? (
            <form className="message-composer" onSubmit={sendMessage}>
              <div className="message-input-shell">
                {mentionQuery !== null ? <div className="mention-menu">
                  {showMentionAllCandidate ? <button type="button" onMouseDown={(event) => { event.preventDefault(); insertMention(); }}><span className="mention-avatar all">@</span><span><strong>所有人</strong><small>唤醒群内全部 Agent</small></span></button> : null}
                  {mentionCandidates.map((agent) => <button key={agent.agent_profile.id} type="button" onMouseDown={(event) => { event.preventDefault(); insertMention(agent); }}><span className="mention-avatar">{agent.agent_profile.display_name.slice(0, 1)}</span><span><strong>{agent.agent_profile.display_name}</strong><small>@{agent.agent_profile.handle.replace(/^@/, "")} · {agent.membership.job_title || "Agent"}</small></span></button>)}
                  {!mentionCandidates.length && !showMentionAllCandidate ? <div className="mention-empty">没有匹配的群成员</div> : null}
                </div> : null}
                <textarea value={draft} onChange={(event) => updateDraft(event.target.value)} onKeyDown={(event) => {
                  if (mentionQuery === null) return;
                  if (event.key === "Escape") { event.preventDefault(); setMentionQuery(null); }
                  if ((event.key === "Enter" || event.key === "Tab") && !event.shiftKey) {
                    const candidate = mentionCandidates[0];
                    if (candidate || showMentionAllCandidate) {
                      event.preventDefault();
                      insertMention(candidate);
                    }
                  }
                }} placeholder={selectedIsGroup ? "输入 @ 选择要立即唤醒的 Agent…" : "给 Agent 分配任务、补充信息…"} disabled={!selected || busy} required />
                {selectedIsGroup && (mentionAll || mentionedAgentIds.length) ? <div className="mention-chips">
                  {mentionAll ? <button type="button" onClick={() => removeMention()}><span>@所有人</span><Icon name="close" /></button> : mentionedAgentIds.map((agentId) => {
                    const agent = agentDirectory.get(agentId);
                    return agent ? <button type="button" key={agentId} onClick={() => removeMention(agentId)}><span>@{agent.agent_profile.display_name}</span><Icon name="close" /></button> : null;
                  })}
                </div> : null}
              </div>
              <button className="button primary" disabled={!selected || !draft.trim() || busy}>{busy ? "发送中…" : "发送消息"}</button>
              <small>{selectedIsGroup
                ? mentionAll
                  ? "已 @所有人；发送后会即时唤醒群内全部 Agent。"
                  : mentionedAgentIds.length
                    ? `已 @ ${mentionedAgentIds.length} 个 Agent；只有被提及成员会立即唤醒。`
                    : "未指定 @；发送后会唤醒群内全部 Agent。"
                : "私聊会即时唤醒目标 Agent。"} 若 Agent 正在运行，会在本轮结束后继续处理，不会重复并发启动。</small>
            </form>
          ) : <div className="observer-note"><Icon name="eye" /> Viewer 保持只读；Owner 或 Admin 可以发送消息。</div>}
        </div>
      </section>
      {showNewDirect ? (
        <Dialog title="新建 Human 私聊" description="选择一个活跃 Agent。重复选择同一个 Agent 会打开原有私聊。" onClose={() => setShowNewDirect(false)}>
          <form className="stack-form" onSubmit={openDirect}>
            <Field label="目标 Agent"><select value={targetAgentId} onChange={(event) => setTargetAgentId(event.target.value)} required><option value="">请选择 Agent</option>{activeAgents.map((agent) => <option key={agent.agent_profile.id} value={agent.agent_profile.id}>{agent.agent_profile.display_name} · {agent.membership.job_title}</option>)}</select></Field>
            {!activeAgents.length ? <div className="git-security-note">当前没有可接收消息的活跃 Agent。</div> : null}
            <button className="button primary wide" disabled={!targetAgentId || busy}>{busy ? "正在建立…" : "建立私聊"}</button>
          </form>
        </Dialog>
      ) : null}
      {showGroupMembers && selectedIsGroup && selected ? (
        <Dialog title="群成员" description={`${conversationDisplayTitle(selected, agentNames)} · ${selected.member_agent_ids.length} 个 Agent，当前 Human 可参与通信。`} onClose={() => setShowGroupMembers(false)}>
          <div className="conversation-member-list">
            <div className="conversation-member-row human-member">
              <span className="agent-avatar small">{props.humanUser.display_name.slice(0, 1)}</span>
              <div><strong>{props.humanUser.display_name}</strong><small>Human · 当前登录用户</small></div>
              <span className="member-kind human">可发送消息</span>
            </div>
            {selectedGroupAgents.map((agent) => {
              const displayedStatus = agent.membership.employment_status === "active" ? agent.connection.status : agent.membership.employment_status;
              return <div className="conversation-member-row" key={agent.agent_profile.id}><span className="agent-avatar small">{agent.agent_profile.display_name.slice(0, 1)}</span><div><strong>{agent.agent_profile.display_name}</strong><small>@{agent.agent_profile.handle.replace(/^@/, "")} · {agent.membership.job_title || "Agent"}</small></div><StatusBadge value={displayedStatus} /></div>;
            })}
            {!selectedGroupAgents.length ? <div className="conversation-member-empty">这个群目前还没有 Agent 成员。</div> : null}
          </div>
        </Dialog>
      ) : null}
    </>
  );
}

function CreateCompanyDialog(props: { token: string; onClose: () => void; onCreated: (id: string) => Promise<void>; onError: (error: unknown) => void }) {
  const [name, setName] = useState("");
  const [slug, setSlug] = useState("");
  const [description, setDescription] = useState("");
  const [busy, setBusy] = useState(false);
  async function submit(event: FormEvent) {
    event.preventDefault(); setBusy(true);
    try {
      const response = await api<{ company_console: CompanyConsole }>("/api/v1/companies", { method: "POST", body: JSON.stringify({ name, slug: slug || undefined, description }) }, props.token);
      await props.onCreated(response.company_console.company.id);
    } catch (error) { props.onError(error); } finally { setBusy(false); }
  }
  return <Dialog title="创建公司" description="公司是 Agent 身份、组织和通信的租户边界。" onClose={props.onClose}><form className="stack-form" onSubmit={submit}><Field label="公司名称"><input value={name} onChange={(event) => setName(event.target.value)} placeholder="例如：Northstar Studio" required /></Field><Field label="唯一标识（可选）"><input value={slug} onChange={(event) => setSlug(event.target.value)} placeholder="northstar" /></Field><Field label="简介"><textarea value={description} onChange={(event) => setDescription(event.target.value)} placeholder="这家公司负责什么？" /></Field><div className="dialog-actions"><button type="button" className="button" onClick={props.onClose}>取消</button><button className="button primary" disabled={busy}>{busy ? "创建中…" : "创建公司"}</button></div></form></Dialog>;
}

function CreateAgentDialog(props: { company: Company; orgUnits: OrgUnit[]; agents: CompanyAgent[]; professions: CompanyProfession[]; token: string; onClose: () => void; onCreated: (credential: Credential) => Promise<void>; onError: (error: unknown) => void }) {
  const hasActiveManager = props.agents.some((agent) => agent.membership.role_key === "company_manager" && agent.membership.employment_status === "active");
  const [displayName, setDisplayName] = useState("");
  const [handle, setHandle] = useState("");
  const [persona, setPersona] = useState("");
  const [professionKey, setProfessionKey] = useState(props.professions[0]?.key ?? "general_member");
  const [orgUnitId, setOrgUnitId] = useState(props.orgUnits[0]?.id ?? "");
  const [reportsToId, setReportsToId] = useState("");
  const [roleKey, setRoleKey] = useState(hasActiveManager ? "member" : "company_manager");
  const [busy, setBusy] = useState(false);
  async function submit(event: FormEvent) {
    event.preventDefault(); setBusy(true);
    try {
      const response = await api<{ result: { agent_profile: AgentProfile; membership: AgentMembership; agent_key_plaintext: string; agent_key_prefix: string } }>(`/api/v1/companies/${props.company.id}/agents`, { method: "POST", body: JSON.stringify({ display_name: displayName, handle: handle.replace(/^@/, ""), persona, profession_key: professionKey, org_unit_id: orgUnitId || null, reports_to_membership_id: reportsToId || null, role_key: roleKey }) }, props.token);
      await props.onCreated({
        agent: response.result.agent_profile,
        key: response.result.agent_key_plaintext,
        keyPrefix: response.result.agent_key_prefix,
        permissions: response.result.membership.permissions,
        professionKey,
      });
    } catch (error) { props.onError(error); } finally { setBusy(false); }
  }
  const selectedProfession = props.professions.find((profession) => profession.key === professionKey);
  return <Dialog title="创建 Agent 账号" description={`为 ${props.company.name} 中的一个外部 Agent 签发身份。`} onClose={props.onClose}><form className="stack-form" onSubmit={submit}><div className="form-grid"><Field label="显示名称"><input value={displayName} onChange={(event) => setDisplayName(event.target.value)} placeholder="例如：Maya" required /></Field><Field label="Handle"><input value={handle} onChange={(event) => setHandle(event.target.value)} placeholder="maya-product" required /></Field><Field label="职业"><select value={professionKey} onChange={(event) => setProfessionKey(event.target.value)} required>{props.professions.map((profession) => <option key={profession.key} value={profession.key}>{profession.label}</option>)}</select>{selectedProfession ? <small>{selectedProfession.description}{selectedProfession.can_create_tasks ? " 可创建和分配任务。" : " 只能更新自己任务的执行状态。"}</small> : null}</Field><Field label="组织"><select value={orgUnitId} onChange={(event) => setOrgUnitId(event.target.value)}>{props.orgUnits.map((unit) => <option key={unit.id} value={unit.id}>{unit.name}</option>)}</select></Field><Field label="公司角色"><select value={roleKey} onChange={(event) => setRoleKey(event.target.value)} disabled={!hasActiveManager}><option value="member">普通成员</option><option value="company_manager">公司管理 Agent</option></select>{!hasActiveManager ? <small>公司当前没有活跃管理 Agent，因此本账号必须成为公司管理 Agent。公司角色与职业能力分别控制。</small> : <small>公司角色负责治理；任务创建能力由职业决定。</small>}</Field><Field label="直属上级"><select value={reportsToId} onChange={(event) => setReportsToId(event.target.value)}><option value="">无</option>{props.agents.filter((agent) => agent.membership.employment_status === "active").map((agent) => <option key={agent.membership.id} value={agent.membership.id}>{agent.agent_profile.display_name}</option>)}</select></Field></div><Field label="工作说明 / Persona"><textarea value={persona} onChange={(event) => setPersona(event.target.value)} placeholder="补充这个 Agent 在当前公司的具体职责、工作边界和擅长领域。" required /></Field><div className="security-note"><Icon name="shield" /><span><strong>Key 只会显示一次</strong><small>系统只保存哈希。关闭下一步窗口后无法找回，只能轮换。</small></span></div><div className="dialog-actions"><button type="button" className="button" onClick={props.onClose}>取消</button><button className="button primary" disabled={busy}>{busy ? "签发中…" : "创建并签发 Key"}</button></div></form></Dialog>;
}

function CredentialDialog(props: { credential: Credential; onClose: () => void }) {
  const endpoint = `${API_BASE_URL.replace(/\/$/, "")}/mcp`;
  const hasPlaintextKey = props.credential.key !== null;
  const connectionNames = relayAgentConnectionNames(props.credential.agent);
  const envCommand = `export ${connectionNames.environmentVariable}="${props.credential.key ?? "<粘贴该 Agent 的完整 Key>"}"`;
  const config = `[mcp_servers.${connectionNames.mcpServer}]\nurl = "${endpoint}"\nenv_http_headers = { "x-agent-key" = "${connectionNames.environmentVariable}" }`;
  const hasStaffingPermission = props.credential.permissions.some((permission) => permission.startsWith("agent.staff."));
  const skillDocuments = getRelaySkillDocuments(props.credential.permissions, {
    agentId: props.credential.agent.id,
    handle: props.credential.agent.handle,
    mcpServerName: connectionNames.mcpServer,
  }, props.credential.professionKey);
  const skillBundle = formatSkillBundle(skillDocuments);
  const fullBundle = [
    `# Relay Agent: ${props.credential.agent.display_name}`,
    hasPlaintextKey
      ? `## Agent Key\n${props.credential.key}`
      : `## Agent Key\n系统只保存 Key 哈希，无法再次读取完整 Key。当前 Key 前缀：${props.credential.keyPrefix || "未知"}。如果完整 Key 已遗失，请在管理台轮换 Key。`,
    `## 环境变量\n${envCommand}`,
    `## Codex config.toml\n${config}`,
    `## Agent Skill\n${skillBundle}`,
  ].join("\n\n");
  const title = hasPlaintextKey ? "Agent 已可连接" : "Agent 接入资料";
  const description = hasPlaintextKey
    ? `${props.credential.agent.display_name} 的 Key 和对应 Skill 已准备好。请现在复制，关闭后 Key 不会再次显示。`
    : `重新查看 ${props.credential.agent.display_name} 的 MCP 配置和完整 Skill。`;
  return <Dialog title={title} description={description} onClose={props.onClose} wide><div className="credential-stack"><div className="credential-warning"><Icon name="alert" /><p><strong>{hasPlaintextKey ? "这是唯一一次明文展示" : "完整 Key 无法再次读取"}</strong><span>Key 前缀：{props.credential.keyPrefix || "未知"} · MCP：{connectionNames.mcpServer} · {hasStaffingPermission ? "通用 + 职业 + 人员管理 Skill" : "通用 + 职业 Skill"}</span>{!hasPlaintextKey ? <small>系统只保存 Key 哈希。如果完整 Key 已遗失，请关闭本窗口后点击“轮换 Key”。</small> : null}</p></div>{hasPlaintextKey ? <CodeBlock label="1. 完整 Agent Key" value={props.credential.key ?? ""} secret /> : null}<CodeBlock label={`${hasPlaintextKey ? "2" : "1"}. Codex MCP 配置（可与其他 Agent 并存）`} value={`${envCommand}\n\n${config}`} /><SkillCopyBlock documents={skillDocuments} step={hasPlaintextKey ? "3" : "2"} /><div className="bootstrap-call"><span className="step-number">{hasPlaintextKey ? "4" : "3"}</span><div><strong>从 {connectionNames.mcpServer} 调用 agent.bootstrap</strong><p>确认返回的 handle 是 @{props.credential.agent.handle.replace(/^@/, "")}，再使用该身份处理公司消息和工作。</p></div></div><button className="button primary wide" onClick={() => void copyText(fullBundle)}><Icon name="copy" /> {hasPlaintextKey ? "复制 Key + 配置 + 完整 Skill" : "复制 MCP 配置 + 完整 Skill"}</button><button className="button ghost wide" onClick={props.onClose}>{hasPlaintextKey ? "我已安全保存" : "关闭"}</button></div></Dialog>;
}

function BatchCredentialDialog(props: { result: BatchCredentialResult; onClose: () => void }) {
  const endpoint = `${API_BASE_URL.replace(/\/$/, "")}/mcp`;
  const bundles = props.result.credentials.map((credential) => {
    const names = relayAgentConnectionNames(credential.agent);
    const envCommand = `export ${names.environmentVariable}="${credential.key}"`;
    const config = `[mcp_servers.${names.mcpServer}]\nurl = "${endpoint}"\nenv_http_headers = { "x-agent-key" = "${names.environmentVariable}" }`;
    const documents = getRelaySkillDocuments(credential.permissions, {
      agentId: credential.agent.id,
      handle: credential.agent.handle,
      mcpServerName: names.mcpServer,
    }, credential.professionKey);
    return {
      credential,
      names,
      envCommand,
      config,
      documents,
      full: [`# Relay Agent: ${credential.agent.display_name}`, `## Agent Key\n${credential.key}`, `## 环境变量\n${envCommand}`, `## Codex config.toml\n${config}`, `## Agent Skill\n${formatSkillBundle(documents)}`].join("\n\n"),
    };
  });
  const fullBundle = bundles.map((bundle) => bundle.full).join("\n\n\n----------------------------------------\n\n");
  return <Dialog title={`已激活 ${bundles.length} 个 Agent`} description="所有明文 Key 只展示这一次。请先复制全部接入资料，再关闭窗口。" onClose={props.onClose} wide><div className="credential-stack"><div className="credential-warning"><Icon name="alert" /><p><strong>请立即保存全部 Key</strong><span>{bundles.map((bundle) => `@${bundle.credential.agent.handle.replace(/^@/, "")} · ${bundle.names.mcpServer}`).join("  /  ")}</span></p></div><button className="button primary wide" onClick={() => void copyText(fullBundle)}><Icon name="copy" /> 复制全部 Agent 的 Key + 配置 + 完整 Skill</button>{props.result.failures.length ? <div className="batch-failures"><strong>{props.result.failures.length} 个 Agent 激活失败</strong>{props.result.failures.map((failure) => <span key={failure.agentName}>{failure.agentName}：{failure.message}</span>)}</div> : null}<div className="batch-credential-list">{bundles.map((bundle) => <details className="batch-credential-card" key={bundle.credential.agent.id}><summary><span><strong>{bundle.credential.agent.display_name}</strong><small>@{bundle.credential.agent.handle.replace(/^@/, "")} · {bundle.names.mcpServer}</small></span><Icon name="chevron-down" /></summary><div><CodeBlock label="完整 Agent Key" value={bundle.credential.key ?? ""} secret /><CodeBlock label="Codex MCP 配置" value={`${bundle.envCommand}\n\n${bundle.config}`} /><SkillCopyBlock documents={bundle.documents} step="3" /><button className="button wide" onClick={() => void copyText(bundle.full)}><Icon name="copy" /> 复制这个 Agent 的全部接入资料</button></div></details>)}</div><button className="button ghost wide" onClick={props.onClose}>我已安全保存</button></div></Dialog>;
}

function Dialog(props: { title: string; description?: string; onClose: () => void; children: ReactNode; wide?: boolean }) {
  return <div className="dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) props.onClose(); }}><div className={`dialog ${props.wide ? "wide" : ""}`} role="dialog" aria-modal="true"><div className="dialog-head"><div><h2>{props.title}</h2>{props.description ? <p>{props.description}</p> : null}</div><button className="icon-button" onClick={props.onClose}><Icon name="close" /></button></div>{props.children}</div></div>;
}

function CodeBlock(props: { label: string; value: string; secret?: boolean }) {
  const [visible, setVisible] = useState(!props.secret);
  return <div className="code-block"><div><span>{props.label}</span><div>{props.secret ? <button onClick={() => setVisible(!visible)}><Icon name="eye" /> {visible ? "隐藏" : "显示"}</button> : null}<button onClick={() => void copyText(props.value)}><Icon name="copy" /> 复制</button></div></div><pre>{visible ? props.value : "••••••••••••••••••••••••••••••••"}</pre></div>;
}

function SkillCopyBlock({ documents, step = "3" }: { documents: RelaySkillDocument[]; step?: string }) {
  const [expanded, setExpanded] = useState(false);
  const value = formatSkillBundle(documents);
  const compositionLabel = documents.length <= 1
    ? "按当前权限生成通用协作 Skill"
    : documents.length === 2
      ? "通用协作 Skill + 职业专属 Skill"
      : "通用协作 Skill + 职业专属 Skill + 特殊授权 Skill";
  return <div className="skill-copy-block"><div className="skill-copy-head"><div><span>{step}. 对应的完整 Agent Skill</span><small>{compositionLabel}</small></div><div className="skill-copy-actions"><button onClick={() => setExpanded(!expanded)}><Icon name={expanded ? "chevron-up" : "chevron-down"} /> {expanded ? "收起" : "查看"}</button><button onClick={() => void copyText(value)}><Icon name="copy" /> 复制完整 Skill</button></div></div><div className="skill-file-list">{documents.map((document) => <span key={document.name}><strong>{document.title}</strong><code>{document.name}/SKILL.md</code></span>)}</div>{expanded ? <pre>{value}</pre> : null}</div>;
}

function EmptyCompany(props: { onCreate: () => void }) { return <div className="center-state"><span className="brand-mark"><Icon name="network" /></span><span className="eyebrow">START HERE</span><h1>先创建一家公司</h1><p>公司会成为外部 Agent 的身份与通信边界。创建后再添加组织和 Agent 账号。</p><button className="button primary" onClick={props.onCreate}><Icon name="plus" /> 创建公司</button></div>; }
function LoadingState() { return <div className="center-state"><span className="loader" /><h2>正在读取公司目录</h2></div>; }
function FlowStep(props: { index: string; title: string; detail: string }) { return <div><span>{props.index}</span><p><strong>{props.title}</strong><small>{props.detail}</small></p></div>; }
function MiniStep(props: { number: string; text: string; done: boolean }) { return <div className={props.done ? "done" : ""}><span>{props.done ? <Icon name="check" /> : props.number}</span><strong>{props.text}</strong></div>; }
function Metric(props: { label: string; value: string; detail: string }) { return <div className="metric"><span>{props.label}</span><strong>{props.value}</strong><small>{props.detail}</small></div>; }
function Field(props: { label: string; children: ReactNode }) { return <label className="field"><span>{props.label}</span>{props.children}</label>; }
function NavItem(props: { icon: string; label: string; badge?: number; active: boolean; onClick: () => void }) { return <button className={props.active ? "active" : ""} onClick={props.onClick}><Icon name={props.icon} />{props.label}{props.badge ? <span className="nav-badge">{props.badge > 99 ? "99+" : props.badge}</span> : null}</button>; }
function StatusBadge({ value }: { value: string }) { const label = { active: "可连接", connected: "已连接", not_connected: "待连接", awaiting_activation: "待激活", provisioning: "待激活", suspended: "已暂停", terminated: "已裁撤", key_revoked: "Key 已撤销", key_expired: "Key 已过期", no_key: "无 Key", running: "运行中", succeeded: "成功", failed: "失败", timed_out: "超时", cancelled: "已取消", lease_lost: "租约丢失", approved: "已批准", rejected: "已拒绝" }[value] ?? value; return <span className={`status-badge ${value}`}><span className="status-dot" />{label}</span>; }
function Toast(props: { children: ReactNode; tone?: "error"; onClose: () => void }) { return <div className={`toast ${props.tone ?? ""}`}><span>{props.children}</span><button onClick={props.onClose}><Icon name="close" /></button></div>; }

function Icon({ name }: { name: string }) {
  const paths: Record<string, ReactNode> = {
    network: <><circle cx="6" cy="6" r="2"/><circle cx="18" cy="6" r="2"/><circle cx="12" cy="18" r="2"/><path d="m7.7 7 3.2 8M16.3 7l-3.2 8M8 6h8"/></>,
    plus: <path d="M12 5v14M5 12h14"/>, key: <><circle cx="8" cy="12" r="3"/><path d="M11 12h9M17 12v3M20 12v2"/></>,
    git: <><circle cx="6" cy="4" r="2"/><circle cx="18" cy="7" r="2"/><circle cx="6" cy="20" r="2"/><path d="M6 6v12M8 7c5 0 3 0 8 0M13 7v4c0 4-2 5-5 5"/></>,
    tasks: <><rect x="4" y="3" width="16" height="18" rx="2"/><path d="m8 8 1.5 1.5L12 7M14 9h3M8 14l1.5 1.5L12 13M14 15h3"/></>,
    search: <><circle cx="11" cy="11" r="7"/><path d="m20 20-4-4"/></>,
    terminal: <><rect x="3" y="4" width="18" height="16" rx="2"/><path d="m7 9 3 3-3 3M13 15h4"/></>,
    book: <><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/><path d="M8 7h8M8 11h6"/></>,
    folder: <><path d="M3 6a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v9a3 3 0 0 1-3 3H6a3 3 0 0 1-3-3z"/><path d="M3 9h18"/></>,
    org: <><rect x="9" y="3" width="6" height="5" rx="1"/><rect x="3" y="16" width="6" height="5" rx="1"/><rect x="15" y="16" width="6" height="5" rx="1"/><path d="M12 8v4M6 16v-4h12v4"/></>,
    message: <path d="M21 15a4 4 0 0 1-4 4H8l-5 3V7a4 4 0 0 1 4-4h10a4 4 0 0 1 4 4z"/>, group: <><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75"/></>,
    logout: <><path d="M10 17l5-5-5-5M15 12H3"/><path d="M14 3h5a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-5"/></>, refresh: <><path d="M20 11a8.1 8.1 0 0 0-15.5-2M4 4v5h5"/><path d="M4 13a8.1 8.1 0 0 0 15.5 2M20 20v-5h-5"/></>,
    pause: <><path d="M9 5v14M15 5v14"/></>, play: <path d="m8 5 11 7-11 7z"/>, trash: <><path d="M3 6h18M8 6V4h8v2M19 6l-1 15H6L5 6M10 11v6M14 11v6"/></>,
    "chevron-down": <path d="m6 9 6 6 6-6"/>, "chevron-up": <path d="m18 15-6-6-6 6"/>, "chevron-right": <path d="m9 18 6-6-6-6"/>, "arrow-left": <path d="m19 12H5m6 6-6-6 6-6"/>, check: <path d="m5 12 4 4L19 6"/>, shield: <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>,
    close: <path d="M18 6 6 18M6 6l12 12"/>, alert: <><path d="M10.3 2.9 1.8 17a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 2.9a2 2 0 0 0-3.4 0z"/><path d="M12 9v4M12 17h.01"/></>,
    eye: <><path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/></>, copy: <><rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></>,
  };
  return <svg viewBox="0 0 24 24" aria-hidden="true">{paths[name] ?? paths.network}</svg>;
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function renderMessageContent(content: string, agents: CompanyAgent[]): ReactNode {
  const mentionTokens = Array.from(new Set([
    "所有人",
    ...agents.flatMap((agent) => [
      agent.agent_profile.display_name,
      agent.agent_profile.handle.replace(/^@/, ""),
    ]),
  ])).filter(Boolean).sort((left, right) => right.length - left.length);
  if (!mentionTokens.length) return content;
  const mentionPattern = new RegExp(`(@(?:${mentionTokens.map(escapeRegExp).join("|")}))`, "gu");
  return content.split(mentionPattern).map((part, index) => part.startsWith("@")
    ? <mark className="message-mention" key={`${part}-${index}`}>{part}</mark>
    : part);
}

async function api<T = unknown>(path: string, init: RequestInit = {}, token?: string): Promise<T> {
  const headers = new Headers(init.headers);
  if (init.body && !headers.has("content-type")) headers.set("content-type", "application/json");
  if (token) headers.set("authorization", `Bearer ${token}`);
  const response = await fetch(`${API_BASE_URL.replace(/\/$/, "")}${path}`, { ...init, headers });
  const body = await response.json().catch(() => null);
  if (!response.ok) throw new Error(body?.message ?? `请求失败 (${response.status})`);
  return body as T;
}

function readSession(): Session | null {
  try { const value = localStorage.getItem(SESSION_KEY); return value ? JSON.parse(value) as Session : null; } catch { return null; }
}
function persistSession(session: Session) { localStorage.setItem(SESSION_KEY, JSON.stringify(session)); }
async function copyText(value: string) { await navigator.clipboard.writeText(value); }
function formatSkillBundle(documents: RelaySkillDocument[]) {
  return documents
    .map((document) => `===== ${document.name}/SKILL.md =====\n${document.content.trim()}`)
    .join("\n\n");
}
function projectAssetTypeLabel(value: string) {
  return {
    code: "代码模块",
    document: "文档",
    api: "接口",
    config: "配置",
    data: "数据",
    database: "数据库",
    script: "脚本",
    service: "服务",
    test: "测试",
  }[value] ?? value;
}
function relayAgentConnectionNames(agent: AgentProfile) {
  const token = agent.handle
    .replace(/^@/, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "") || agent.id.replace(/-/g, "").slice(0, 8).toLowerCase();
  return {
    environmentVariable: `RELAY_AGENT_KEY_${token.toUpperCase()}`,
    mcpServer: `relay_${token}`,
  };
}
function formatTime(value: string) { return new Date(value).toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" }); }
function formatElapsed(value: string) {
  const seconds = Math.max(0, Math.floor((Date.now() - new Date(value).getTime()) / 1000));
  if (seconds < 60) return `${seconds} 秒`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} 分 ${seconds % 60} 秒`;
  return `${Math.floor(minutes / 60)} 小时 ${minutes % 60} 分`;
}
function formatInterval(seconds: number) {
  if (seconds % 86400 === 0) return `${seconds / 86400} 天`;
  if (seconds % 3600 === 0) return `${seconds / 3600} 小时`;
  if (seconds % 60 === 0) return `${seconds / 60} 分钟`;
  return `${seconds} 秒`;
}

function codexReasoningEffortLabel(value: CodexReasoningEffort | null) {
  if (!value) return "跟随模型默认";
  return ({ minimal: "最低", low: "低", medium: "中", high: "高", xhigh: "超高", max: "最大", ultra: "极致" } as Record<CodexReasoningEffort, string>)[value];
}
function companyAgentProfessionKey(agent: CompanyAgent, professions: CompanyProfession[]) {
  if (agent.profession?.key) return agent.profession.key;
  const title = (agent.membership.job_title || "").trim().toLowerCase();
  const exact = professions.find((profession) => profession.label.toLowerCase() === title);
  if (exact) return exact.key;
  if (title.includes("项目经理") || title.includes("项目负责人") || title === "project manager" || title === "pm") return "project_manager";
  if (title.includes("产品经理") || title.includes("产品负责人") || title.includes("product manager") || title.includes("product owner")) return "product_manager";
  if (title.includes("技术经理") || title.includes("技术负责人") || title.includes("工程经理") || title.includes("研发经理") || title.includes("技术总监") || title === "cto" || title.includes("technical manager") || title.includes("engineering manager") || title.includes("tech lead")) return "technical_manager";
  if (title.includes("测试") || title.includes("质量") || title.includes("qa")) return "qa_engineer";
  if (title.includes("架构师") || title.includes("solution architect") || title.includes("software architect") || title.includes("system architect")) return "solution_architect";
  if (title.includes("前端") || title.includes("frontend") || title.includes("front-end") || title.includes("web developer") || title.includes("web engineer")) return "frontend_engineer";
  if (title.includes("后端") || title.includes("服务端") || title.includes("backend") || title.includes("back-end") || title.includes("server engineer")) return "backend_engineer";
  if (title.includes("移动端") || title.includes("客户端") || title.includes("mobile") || title.includes("android") || title.includes("ios") || title.includes("pda")) return "mobile_engineer";
  if (title.includes("数据工程") || title.includes("数据平台") || title.includes("数据迁移") || title.includes("主数据") || title.includes("data engineer") || title.includes("etl")) return "data_engineer";
  if (title.includes("devops") || title.includes("sre") || title.includes("可靠性") || title.includes("运维工程") || title.includes("平台工程")) return "devops_engineer";
  if (title.includes("ui 设计") || title.includes("界面设计") || title.includes("视觉设计") || title.includes("ui designer") || title.includes("visual designer")) return "ui_designer";
  if (title.includes("ux") || title.includes("用户体验") || title.includes("交互设计") || title.includes("experience designer") || title.includes("interaction designer")) return "ux_designer";
  if (title.includes("产品设计") || title.includes("product designer")) return "product_designer";
  if (title.includes("实施顾问") || title.includes("实施工程") || title.includes("implementation consultant") || title.includes("implementation engineer")) return "implementation_consultant";
  if (title.includes("领域专家") || title.includes("业务专家") || title.includes("行业专家") || title.includes("subject matter expert") || title.includes("sme") || title.endsWith("专家")) return "domain_expert";
  if (title.includes("设计") || title.includes("designer")) return "product_designer";
  if (title.includes("分析") || title.includes("顾问") || title.includes("analyst")) return "business_analyst";
  if (title.includes("运营") || title.includes("销售") || title.includes("operation")) return "operations_specialist";
  if (title.includes("工程") || title.includes("开发") || title.includes("程序") || title.includes("engineer") || title.includes("developer")) return "software_engineer";
  return "general_member";
}
function projectStatusLabel(value: string) { return ({ planned: "计划中", active: "进行中", blocked: "已阻塞", completed: "已完成", cancelled: "已取消" } as Record<string, string>)[value] ?? value; }
function taskStatusLabel(value: CompanyProjectTask["status"]) { return { todo: "待处理", in_progress: "进行中", blocked: "阻塞", done: "已完成", failed: "失败", cancelled: "已取消" }[value]; }
function taskPriorityLabel(value: CompanyProjectTask["priority"]) { return { low: "低", normal: "普通", high: "高", urgent: "紧急" }[value]; }
function formatTaskDue(value: string) { return new Date(value).toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" }); }
function taskDueClass(task: CompanyProjectTask) { return task.due_at && !["done", "failed", "cancelled"].includes(task.status) && new Date(task.due_at).getTime() < Date.now() ? "task-due overdue" : "task-due"; }
function toDateTimeLocalValue(value: string | null) {
  if (!value) return "";
  const date = new Date(value);
  const local = new Date(date.getTime() - date.getTimezoneOffset() * 60_000);
  return local.toISOString().slice(0, 16);
}
function conversationDisplayTitle(conversation: Conversation | undefined, agentNames: Map<string, string>) {
  if (!conversation) return "";
  const isDirect = conversation.preview.conversation_type === "direct" || conversation.context.context_type === "company_direct";
  if (!isDirect) return conversation.preview.title;
  const memberNames = conversation.member_agent_ids
    .map((agentId) => agentNames.get(agentId))
    .filter((name): name is string => Boolean(name));
  return memberNames.length >= 2 ? memberNames.join(" ↔ ") : conversation.preview.title;
}
function formatConversationContext(value?: string) { return { company_all: "公司全员群", company_group: "公司群", project_group: "项目群", company_direct: "公司私聊" }[value ?? ""] ?? value ?? "公司会话"; }
function collaborationPreferenceLabel(value: AgentProfile["collaboration_preference"]) { return { available: "可协作", low_cost_only: "仅接受低成本请求", unavailable: "暂不接受请求" }[value] ?? value; }
function codexTriggerStatusLabel(value: CodexTriggerView["config"]["status"]) { return { active: "已启用", paused: "已暂停", error: "错误" }[value]; }
function codexOperationalStatusLabel(value: string) { return { running: "执行中", queued: "排队中", idle: "等待检查", paused: "已暂停", error: "错误" }[value] ?? value; }
function codexActivityPhaseLabel(value: string) { return ({ preparing: "准备工作区", starting: "启动 Codex", session: "连接会话", thinking: "分析", planning: "规划", tool: "调用工具", command: "执行命令", files: "修改文件", searching: "搜索", reporting: "整理结果", finishing: "收尾", waiting_approval: "等待审批", approval_rejected: "审批未通过", running: "执行中", completed: "已完成", failed: "失败", timed_out: "超时", cancelled: "已取消" } as Record<string, string>)[value] ?? value; }
function codexTriggerTypeLabel(value: string) { return { scheduled: "定时", manual: "手动", run_now: "手动", message: "消息", task: "任务", asset_refresh: "资产维护" }[value] ?? value; }
function codexRunDisplayMessage(run: CodexTriggerRun) {
  if (run.final_message_summary) return run.final_message_summary;
  if (run.error_message) return run.error_message;
  return {
    running: "本轮仍在执行，尚未完成",
    succeeded: "Codex 已完成本轮",
    timed_out: "本轮运行超时",
    cancelled: "本轮已取消",
    lease_lost: "本轮租约已失效",
    failed: "本轮运行失败",
  }[run.status] ?? "等待运行结果";
}
function approvalToolLabel(value: string) { return ({ "codex.command_execution": "执行命令", "codex.file_change": "修改受保护文件", "codex.permissions": "申请额外权限", "agent.staff.hire": "扩招 Agent", "agent.staff.suspend": "暂停 Agent", "agent.staff.terminate": "裁撤 Agent", "company.project.task.reassign": "重新分配任务" } as Record<string, string>)[value] ?? value; }
function approvalStatusLabel(value: AgentToolApproval["status"]) { return { pending: "待审批", approved: "已批准", executing: "执行中", executed: "已执行", rejected: "已拒绝", expired: "已过期", failed: "失败" }[value]; }
function approvalRiskLabel(value: AgentToolApproval["risk_level"]) { return { low: "低", medium: "中", high: "高" }[value]; }
function approvalRequestDetail(approval: AgentToolApproval) {
  const params = typeof approval.arguments.params === "object" && approval.arguments.params !== null ? approval.arguments.params as Record<string, unknown> : approval.arguments;
  if (approval.tool_name === "codex.command_execution") {
    const command = typeof params.command === "string" ? params.command : "";
    const cwd = typeof params.cwd === "string" ? params.cwd : "";
    return [command, cwd ? `cwd: ${cwd}` : ""].filter(Boolean).join("\n");
  }
  if (approval.tool_name === "codex.file_change") {
    const itemId = typeof params.itemId === "string" ? params.itemId : "";
    const grantRoot = typeof params.grantRoot === "string" ? params.grantRoot : "";
    return [itemId ? `文件变更项：${itemId}` : "", grantRoot ? `请求写入：${grantRoot}` : ""].filter(Boolean).join("\n");
  }
  if (approval.tool_name === "codex.permissions") {
    return JSON.stringify(params.permissions ?? params, null, 2);
  }
  return JSON.stringify(params, null, 2);
}
