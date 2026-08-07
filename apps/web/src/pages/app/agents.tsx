import { useEffect, useMemo, useState } from "react";
import { api } from "../../api/client";
import { Pagination, usePagination } from "../../components/Pagination";
import { Field, Icon } from "../../components/ui";
import type {
  CompanyAgent,
  CompanyConsole,
  CompanyProfession,
} from "../../types/platform";
import { MemoriesView } from "./memory-and-approvals";
import { OrganizationView } from "./organization";
import { PROJECT_PERMISSIONS, STAFFING_PERMISSIONS } from "./permissions";
import {
  collaborationPreferenceLabel,
  companyAgentProfessionKey,
  Dialog,
  formatTime,
  Metric,
  StatusBadge,
} from "./shared";

export function OrganizationAgentCenter(props: {
  consoleData: CompanyConsole;
  token: string;
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
          <span><strong>Agent 成员</strong><small>身份、状态、记忆与个人权限</small></span>
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
            token={props.token}
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

export function AgentsView(props: {
  consoleData: CompanyConsole;
  token: string;
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
    if (!window.confirm(`激活 ${provisioningAgents.length} 个 Agent？`)) return;
    setBatchBusy(true);
    let activatedCount = 0;
    const failures: Array<{ agentName: string; message: string }> = [];
    for (const agent of provisioningAgents) {
      try {
        await api(
          `/api/v1/companies/${props.consoleData.company.id}/agents/${agent.agent_profile.id}/activate`,
          { method: "POST", body: JSON.stringify({ reason: "Human console: batch activate" }) },
          props.token,
        );
        activatedCount += 1;
      } catch (error) {
        failures.push({
          agentName: agent.agent_profile.display_name,
          message: error instanceof Error ? error.message : "激活失败",
        });
      }
    }
    if (activatedCount) {
      props.onNotice(failures.length
        ? `已激活 ${activatedCount} 个 Agent，${failures.length} 个失败。`
        : `已激活 ${activatedCount} 个 Agent。`);
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
          <div className="empty-inline"><Icon name="key" /><h3>还没有 Agent 账号</h3><p>点击右上角创建第一个托管 Agent。</p></div>
        )}
      </section>
    </div>
  );
}

export function AgentRow(props: {
  agent: CompanyAgent;
  consoleData: CompanyConsole;
  token: string;
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
    : active
      ? "Relay 已托管"
      : "等待激活";

  useEffect(() => {
    setPermissions(props.agent.membership.permissions);
    setScopeId(props.agent.membership.staffing_scope_org_unit_id ?? "");
    setRoleKey(props.agent.membership.role_key);
    setProfessionKey(currentProfessionKey);
  }, [props.agent.membership.permissions, props.agent.membership.role_key, props.agent.membership.staffing_scope_org_unit_id, currentProfessionKey]);

  async function changeStatus(action: "activate" | "suspend" | "reactivate" | "terminate") {
    const labels = { activate: "激活", suspend: "暂停", reactivate: "重新激活", terminate: "永久裁撤" };
    if (!window.confirm(`${labels[action]} ${props.agent.agent_profile.display_name}？${action === "terminate" ? "该操作不可恢复。" : ""}`)) return;
    setBusy(true);
    try {
      await api(
        `/api/v1/companies/${props.consoleData.company.id}/agents/${props.agent.agent_profile.id}/${action}`,
        { method: "POST", body: JSON.stringify({ reason: `Human console: ${action}` }) },
        props.token,
      );
      props.onNotice(`${props.agent.agent_profile.display_name} 已${labels[action]}`);
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
      props.onNotice("公司角色已更新，Agent 工作权限已自动同步。");
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
      props.onNotice("职业已更新，任务权限和职业 Skill 已自动同步。");
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
          {provisioning ? <button className="button small primary" onClick={() => void changeStatus("activate")} disabled={busy}>激活</button> : null}
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
