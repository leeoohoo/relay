import { useEffect, useRef, useState } from "react";
import { api } from "../api/client";
import {
  companyConsoleRegionsForEvent,
  fetchCompanyConsole,
  fetchCompanyConsoleRegionPage,
  fetchCompanyConsoleRegions,
  type CompanyConsoleRegion,
} from "../api/companyConsole";
import type { CompanyRealtimeEvent } from "../api/types";
import { AuthScreen } from "../components/AuthScreen";
import { Sidebar } from "../components/Sidebar";
import { Icon } from "../components/ui";
import { useCompanyEvents } from "../hooks/useCompanyEvents";
import type { Company, HumanUser, RuntimeConfig, Session, View } from "../types/appShell";
import type {
  AgentToolApproval,
  CompanyConsole,
} from "../types/platform";
import {
  EmptyCompany,
  LoadingState,
  persistSession,
  readSession,
  Toast,
} from "./app/shared";
import { CreateAgentDialog, CreateCompanyDialog, UserPreferencesDialog } from "./app/organization";
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
  const [loadingRegion, setLoadingRegion] = useState<"agents" | "conversations" | "projects" | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [showCompanyForm, setShowCompanyForm] = useState(false);
  const [showAgentForm, setShowAgentForm] = useState(false);
  const [showUserPreferences, setShowUserPreferences] = useState(false);
  const [approvals, setApprovals] = useState<AgentToolApproval[]>([]);
  const [dismissedApprovalIds, setDismissedApprovalIds] = useState<Set<string>>(() => new Set());
  const [realtimeEvent, setRealtimeEvent] = useState<CompanyRealtimeEvent | null>(null);
  const realtimeRefreshTimerRef = useRef<number | null>(null);
  const pendingRealtimeRegionsRef = useRef<Set<CompanyConsoleRegion>>(new Set());

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
      for (const region of companyConsoleRegionsForEvent(event.event_type)) {
        pendingRealtimeRegionsRef.current.add(region);
      }
      if (pendingRealtimeRegionsRef.current.size === 0) return;
      if (realtimeRefreshTimerRef.current !== null) {
        window.clearTimeout(realtimeRefreshTimerRef.current);
      }
      realtimeRefreshTimerRef.current = window.setTimeout(() => {
        realtimeRefreshTimerRef.current = null;
        if (selectedCompanyId && session?.token) {
          const regions = new Set(pendingRealtimeRegionsRef.current);
          pendingRealtimeRegionsRef.current.clear();
          void refreshCompanyRegions(selectedCompanyId, session.token, regions);
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
      const nextConsole = await fetchCompanyConsole(companyId, token);
      setCompanyConsole(nextConsole);
    } catch (requestError) {
      showError(requestError);
    }
  }

  async function refreshCompanyRegions(
    companyId: string,
    token: string,
    regions: Iterable<CompanyConsoleRegion>,
  ) {
    try {
      const patch = await fetchCompanyConsoleRegions(companyId, token, regions);
      setCompanyConsole((current) => {
        if (!current || current.company.id !== companyId) return current;
        return {
          ...current,
          ...patch.data,
          pagination: { ...current.pagination, ...patch.pagination },
        };
      });
    } catch (requestError) {
      showError(requestError);
    }
  }

  async function loadMoreCompanyRegion(region: "agents" | "conversations" | "projects") {
    if (!companyConsole || !session || loadingRegion) return;
    const page = companyConsole.pagination[region];
    if (!page.has_more || !page.next_cursor) return;
    setLoadingRegion(region);
    try {
      if (region === "agents") {
        const next = await fetchCompanyConsoleRegionPage(
          companyConsole.company.id,
          session.token,
          "agents",
          page.next_cursor,
        );
        setCompanyConsole((current) => {
          if (!current || current.company.id !== companyConsole.company.id) return current;
          const known = new Set(current.agents.map((item) => item.agent_profile.id));
          return {
            ...current,
            agents: [...current.agents, ...next.items.filter((item) => !known.has(item.agent_profile.id))],
            pagination: { ...current.pagination, agents: next.page },
          };
        });
      } else if (region === "conversations") {
        const next = await fetchCompanyConsoleRegionPage(
          companyConsole.company.id,
          session.token,
          "conversations",
          page.next_cursor,
        );
        setCompanyConsole((current) => {
          if (!current || current.company.id !== companyConsole.company.id) return current;
          const known = new Set(current.conversations.map((item) => item.preview.id));
          return {
            ...current,
            conversations: [...current.conversations, ...next.items.filter((item) => !known.has(item.preview.id))],
            pagination: { ...current.pagination, conversations: next.page },
          };
        });
      } else {
        const next = await fetchCompanyConsoleRegionPage(
          companyConsole.company.id,
          session.token,
          "projects",
          page.next_cursor,
        );
        setCompanyConsole((current) => {
          if (!current || current.company.id !== companyConsole.company.id) return current;
          const known = new Set(current.projects.map((item) => item.project.id));
          return {
            ...current,
            projects: [...current.projects, ...next.items.filter((item) => !known.has(item.project.id))],
            pagination: { ...current.pagination, projects: next.page },
          };
        });
      }
    } catch (requestError) {
      showError(requestError);
    } finally {
      setLoadingRegion(null);
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

  function changeView(nextView: View) {
    setView(nextView);
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

  async function reviewApproval(approvalId: string, decision: "approve" | "always_allow" | "reject", reviewNote: string) {
    if (!session || !selectedCompanyId) return;
    const endpointDecision = decision === "always_allow" ? "approve" : decision;
    await api(
      `/api/v1/companies/${selectedCompanyId}/approvals/${approvalId}/${endpointDecision}`,
      { method: "POST", body: JSON.stringify({ review_note: reviewNote || null, approval_mode: decision === "always_allow" ? "always" : "once" }) },
      session.token,
    );
    setDismissedApprovalIds((current) => {
      const next = new Set(current);
      next.add(approvalId);
      return next;
    });
    await refreshApprovals();
    setNotice(decision === "always_allow" ? "已始终允许当前 Agent 在此项目访问该网站" : decision === "approve" ? "审批已通过，等待中的 Codex 会继续执行" : "审批已拒绝，Codex 会收到拒绝结果并继续处理");
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
        onViewChange={changeView}
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
              token={session.token}
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
              <div className="page-header-actions">
                {view === "agents" ? (
                  <button className="button primary" onClick={() => setShowAgentForm(true)}>
                    <Icon name="plus" /> 创建 Agent 账号
                  </button>
                ) : null}
                {view === "agents" && companyConsole.pagination.agents.has_more ? (
                  <button className="button secondary" disabled={loadingRegion !== null} onClick={() => void loadMoreCompanyRegion("agents")}>
                    {loadingRegion === "agents" ? "加载中…" : "加载更多 Agent"}
                  </button>
                ) : null}
                {view === "projects" && companyConsole.pagination.projects.has_more ? (
                  <button className="button secondary" disabled={loadingRegion !== null} onClick={() => void loadMoreCompanyRegion("projects")}>
                    {loadingRegion === "projects" ? "加载中…" : "加载更多项目"}
                  </button>
                ) : null}
                {view === "messages" && companyConsole.pagination.conversations.has_more ? (
                  <button className="button secondary" disabled={loadingRegion !== null} onClick={() => void loadMoreCompanyRegion("conversations")}>
                    {loadingRegion === "conversations" ? "加载中…" : "加载更多会话"}
                  </button>
                ) : null}
              </div>
            </header>

            {view === "agents" ? (
              <OrganizationAgentCenter
                consoleData={companyConsole}
                token={session.token}
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
          onCreated={async () => {
            setShowAgentForm(false);
            await refreshCompany();
            setNotice("Agent 已创建并完成托管配置。");
          }}
          onError={showError}
        />
      ) : null}
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
