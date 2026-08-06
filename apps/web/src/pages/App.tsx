import { useEffect, useRef, useState } from "react";
import { api } from "../api/client";
import type { CompanyRealtimeEvent } from "../api/types";
import { AuthScreen } from "../components/AuthScreen";
import { Sidebar } from "../components/Sidebar";
import { Icon } from "../components/ui";
import { useCompanyEvents } from "../hooks/useCompanyEvents";
import type { Company, HumanUser, RuntimeConfig, Session, View } from "../types/appShell";
import type {
  AgentToolApproval,
  BatchCredentialResult,
  CompanyConsole,
  Credential,
} from "../types/platform";
import {
  EmptyCompany,
  LoadingState,
  persistSession,
  readSession,
  Toast,
} from "./app/shared";
import {
  BatchCredentialDialog,
  CreateAgentDialog,
  CreateCompanyDialog,
  CredentialDialog,
  UserPreferencesDialog,
} from "./app/organization";
import { ApprovalDialog } from "./app/memory-and-approvals";
import { OrganizationAgentCenter } from "./app/agents";
import { ChatCenter } from "./app/chat";
import { SkillsView } from "./app/skills";
import { CodexControlCenter } from "./codex/control-center";
import { ProjectsView } from "./projects";

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
