import { useEffect, useState } from "react";
import { api } from "../../api/client";
import type { CompanyRealtimeEvent } from "../../api/types";
import { Pagination, usePagination } from "../../components/Pagination";
import { Icon } from "../../components/ui";
import type { CodexCompanyCliSettings, CodexEnvironmentView, CodexRunnerProfileView, CompanyConsole } from "../../types/platform";
import { Metric, StatusBadge } from "../app/shared";
import { CodexRunnerProfilesPanel } from "./profiles";
import { CodexTriggerPanel } from "./trigger";

export function CodexRunnersView(props: {
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
  const [cliSettings, setCliSettings] = useState<CodexCompanyCliSettings | null>(null);
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

  async function loadCliSettings() {
    try {
      const response = await api<{ settings: CodexCompanyCliSettings }>(
        `/api/v1/companies/${props.consoleData.company.id}/codex-cli-settings`,
        {},
        props.token,
      );
      setCliSettings(response.settings);
    } catch (error) {
      props.onError(error);
    }
  }

  async function refreshProfiles() {
    await Promise.all([loadProfiles(), loadCliSettings()]);
  }

  useEffect(() => {
    setProfilesLoading(true);
    void refreshProfiles();
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
        cliSettings={cliSettings}
        onChanged={refreshProfiles}
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
                  projects={props.consoleData.projects.map((project) => ({ id: project.project.id, name: project.project.name }))}
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
