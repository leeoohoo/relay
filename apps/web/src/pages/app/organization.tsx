import { type FormEvent, useEffect, useMemo, useState } from "react";
import { api } from "../../api/client";
import { Pagination, usePagination } from "../../components/Pagination";
import { Field, Icon } from "../../components/ui";
import { type UiLanguage, useUiLanguage } from "../../i18n/uiLanguage";
import type { RelaySkillLanguage } from "../../relaySkills";
import type { Company } from "../../types/appShell";
import type {
  CompanyAgent,
  CompanyConsole,
  CompanyProfession,
  OrgUnit,
} from "../../types/platform";
import { Dialog } from "./shared";
import { STAFFING_PERMISSIONS } from "./permissions";

export function OrganizationView(props: {
  companyId: string;
  orgUnits: OrgUnit[];
  agents: CompanyAgent[];
  managedWorkspaceRoot: string | null;
  token: string;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [name, setName] = useState("");
  const [parentId, setParentId] = useState(props.orgUnits[0]?.id ?? "");
  const [unitType, setUnitType] = useState("team");
  const [busy, setBusy] = useState(false);
  const [workspaceRoot, setWorkspaceRoot] = useState(props.managedWorkspaceRoot ?? "");
  const [workspaceBusy, setWorkspaceBusy] = useState(false);
  const orgPagination = usePagination(props.orgUnits, 10, props.companyId);

  useEffect(() => setWorkspaceRoot(props.managedWorkspaceRoot ?? ""), [props.managedWorkspaceRoot]);

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

  async function saveWorkspace(event: FormEvent) {
    event.preventDefault();
    setWorkspaceBusy(true);
    try {
      await api(`/api/v1/companies/${props.companyId}/workspace-settings`, {
        method: "POST",
        body: JSON.stringify({ managed_workspace_root: workspaceRoot.trim() || null }),
      }, props.token);
      await props.onChanged();
      props.onNotice(workspaceRoot.trim() ? "组织默认项目空间已更新" : "已恢复 ~/.relay 默认项目空间");
    } catch (error) {
      props.onError(error);
    } finally {
      setWorkspaceBusy(false);
    }
  }

  return (
    <div className="organization-layout">
      <section className="section-card">
        <div className="section-heading"><div><span className="eyebrow">DIRECTORY</span><h2>组织目录</h2><p>Agent 通过 MCP 读取这份目录来理解同事关系。</p></div></div>
        <div className="org-list">
          {orgPagination.pageItems.map((unit) => {
            const count = props.agents.filter((agent) => agent.membership.org_unit_id === unit.id).length;
            const parent = props.orgUnits.find((item) => item.id === unit.parent_org_unit_id);
            return <div className="org-row" key={unit.id}><span className="org-icon"><Icon name="org" /></span><div><strong>{unit.name}</strong><small>{parent ? `${parent.name} / ` : ""}{unit.unit_type}</small></div><span>{count} Agent</span></div>;
          })}
          <Pagination {...orgPagination} onPageChange={orgPagination.setPage} />
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
      <section className="section-card compact-card organization-workspace-card">
        <div className="section-heading"><div><span className="eyebrow">MANAGED WORKSPACE</span><h2>组织项目空间</h2></div></div>
        <form className="stack-form" onSubmit={saveWorkspace}>
          <Field label="自定义根目录（可选）"><input value={workspaceRoot} onChange={(event) => setWorkspaceRoot(event.target.value)} placeholder="默认：~/.relay/companies/{company-id}" /></Field>
          <div className="git-security-note">留空使用当前 Relay 宿主机用户目录下的 <code>~/.relay</code>。请填写宿主机上的绝对路径；每个项目会创建独立目录。</div>
          <button className="button primary wide" disabled={workspaceBusy}>{workspaceBusy ? "保存中…" : "保存项目空间"}</button>
        </form>
      </section>
    </div>
  );
}

export function CreateCompanyDialog(props: { token: string; onClose: () => void; onCreated: (id: string) => Promise<void>; onError: (error: unknown) => void }) {
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

export function CreateAgentDialog(props: { company: Company; orgUnits: OrgUnit[]; agents: CompanyAgent[]; professions: CompanyProfession[]; skillLanguage: RelaySkillLanguage; token: string; onClose: () => void; onCreated: () => Promise<void>; onError: (error: unknown) => void }) {
  const hasActiveManager = props.agents.some((agent) => agent.membership.role_key === "company_manager" && agent.membership.employment_status === "active");
  const [displayName, setDisplayName] = useState("");
  const [handle, setHandle] = useState("");
  const [persona, setPersona] = useState("");
  const [professionKey, setProfessionKey] = useState(props.professions[0]?.key ?? "general_member");
  const [orgUnitId, setOrgUnitId] = useState(props.orgUnits[0]?.id ?? "");
  const [reportsToId, setReportsToId] = useState("");
  const [roleKey, setRoleKey] = useState(hasActiveManager ? "member" : "company_manager");
  const [busy, setBusy] = useState(false);
  const professionGroups = useMemo(() => {
    const groups = new Map<string, CompanyProfession[]>();
    props.professions.forEach((profession) => {
      const label = props.skillLanguage === "en" ? profession.category_label_en : profession.category_label;
      groups.set(label, [...(groups.get(label) ?? []), profession]);
    });
    return Array.from(groups.entries());
  }, [props.professions, props.skillLanguage]);
  async function submit(event: FormEvent) {
    event.preventDefault(); setBusy(true);
    try {
      await api(`/api/v1/companies/${props.company.id}/agents`, { method: "POST", body: JSON.stringify({ display_name: displayName, handle: handle.replace(/^@/, ""), persona, profession_key: professionKey, org_unit_id: orgUnitId || null, reports_to_membership_id: reportsToId || null, role_key: roleKey }) }, props.token);
      await props.onCreated();
    } catch (error) { props.onError(error); } finally { setBusy(false); }
  }
  const selectedProfession = props.professions.find((profession) => profession.key === professionKey);
  return <Dialog title="创建 Agent 账号" description={`为 ${props.company.name} 添加一个托管 Agent。`} onClose={props.onClose}><form className="stack-form" onSubmit={submit}><div className="form-grid"><Field label="显示名称"><input value={displayName} onChange={(event) => setDisplayName(event.target.value)} placeholder="例如：Maya" required /></Field><Field label="Handle"><input value={handle} onChange={(event) => setHandle(event.target.value)} placeholder="maya-product" required /></Field><Field label="职业"><select value={professionKey} onChange={(event) => setProfessionKey(event.target.value)} required>{professionGroups.map(([category, professions]) => <optgroup label={category} key={category}>{professions.map((profession) => <option key={profession.key} value={profession.key}>{props.skillLanguage === "en" ? profession.label_en : profession.label}</option>)}</optgroup>)}</select>{selectedProfession ? <small>{props.skillLanguage === "en" ? selectedProfession.description_en : selectedProfession.description}{selectedProfession.can_create_tasks ? " 可创建和分配任务。" : " 只能更新自己任务的执行状态。"}</small> : null}</Field><Field label="组织"><select value={orgUnitId} onChange={(event) => setOrgUnitId(event.target.value)}>{props.orgUnits.map((unit) => <option key={unit.id} value={unit.id}>{unit.name}</option>)}</select></Field><Field label="公司角色"><select value={roleKey} onChange={(event) => setRoleKey(event.target.value)} disabled={!hasActiveManager}><option value="member">普通成员</option><option value="company_manager">公司管理 Agent</option></select>{!hasActiveManager ? <small>公司当前没有活跃管理 Agent，因此本账号必须成为公司管理 Agent。公司角色与职业能力分别控制。</small> : <small>公司角色负责治理；任务创建能力由职业决定。</small>}</Field><Field label="直属上级"><select value={reportsToId} onChange={(event) => setReportsToId(event.target.value)}><option value="">无</option>{props.agents.filter((agent) => agent.membership.employment_status === "active").map((agent) => <option key={agent.membership.id} value={agent.membership.id}>{agent.agent_profile.display_name}</option>)}</select></Field></div><Field label="工作说明 / Persona"><textarea value={persona} onChange={(event) => setPersona(event.target.value)} placeholder="补充这个 Agent 在当前公司的具体职责、工作边界和擅长领域。" required /></Field><div className="dialog-actions"><button type="button" className="button" onClick={props.onClose}>取消</button><button className="button primary" disabled={busy}>{busy ? "创建中…" : "创建 Agent"}</button></div></form></Dialog>;
}

export function UserPreferencesDialog(props: {
  companyConsole: CompanyConsole | null;
  token: string;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
  onClose: () => void;
}) {
  const { language, setLanguage } = useUiLanguage();
  const currentSkillLanguage = props.companyConsole?.governance_policy.effective_settings.skill_language ?? "zh-CN";
  const [uiLanguage, setUiLanguage] = useState<UiLanguage>(language);
  const [skillLanguage, setSkillLanguage] = useState<RelaySkillLanguage>(currentSkillLanguage);
  const [triggerBatchSize, setTriggerBatchSize] = useState("10");
  const [savedTriggerBatchSize, setSavedTriggerBatchSize] = useState<number | null>(null);
  const [triggerEnvironmentDefault, setTriggerEnvironmentDefault] = useState<number | null>(null);
  const [loadingTriggerPreferences, setLoadingTriggerPreferences] = useState(false);
  const [busy, setBusy] = useState(false);
  const canManageRuntimePreferences = Boolean(props.companyConsole && ["owner", "admin"].includes(props.companyConsole.human_membership.role));

  useEffect(() => setUiLanguage(language), [language]);
  useEffect(() => setSkillLanguage(currentSkillLanguage), [currentSkillLanguage]);
  useEffect(() => {
    const companyId = props.companyConsole?.company.id;
    if (!companyId) {
      setSavedTriggerBatchSize(null);
      setTriggerEnvironmentDefault(null);
      return;
    }
    let cancelled = false;
    setSavedTriggerBatchSize(null);
    setTriggerEnvironmentDefault(null);
    setLoadingTriggerPreferences(true);
    void api<{ preferences: { batch_size: number; environment_default: number } }>(
      `/api/v1/companies/${companyId}/agent-trigger-preferences`,
      {},
      props.token,
    ).then(({ preferences }) => {
      if (cancelled) return;
      setTriggerBatchSize(String(preferences.batch_size));
      setSavedTriggerBatchSize(preferences.batch_size);
      setTriggerEnvironmentDefault(preferences.environment_default);
    }).catch((error) => {
      if (!cancelled) props.onError(error);
    }).finally(() => {
      if (!cancelled) setLoadingTriggerPreferences(false);
    });
    return () => { cancelled = true; };
  }, [props.companyConsole?.company.id, props.token]);

  async function savePreferences(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      if (props.companyConsole && canManageRuntimePreferences && skillLanguage !== currentSkillLanguage) {
        await api(`/api/v1/companies/${props.companyConsole.company.id}/skill-language`, {
          method: "POST",
          body: JSON.stringify({ skill_language: skillLanguage }),
        }, props.token);
        await props.onChanged();
      }
      const parsedBatchSize = Number(triggerBatchSize);
      if (props.companyConsole && canManageRuntimePreferences && savedTriggerBatchSize !== null && parsedBatchSize !== savedTriggerBatchSize) {
        if (!Number.isInteger(parsedBatchSize) || parsedBatchSize < 1 || parsedBatchSize > 100) {
          throw new Error("同时运行的 Agent 数量必须是 1 到 100 之间的整数");
        }
        await api(`/api/v1/companies/${props.companyConsole.company.id}/agent-trigger-preferences`, {
          method: "PUT",
          body: JSON.stringify({ batch_size: parsedBatchSize }),
        }, props.token);
      }
      setLanguage(uiLanguage);
      props.onNotice("用户偏好已保存");
      props.onClose();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog title="用户偏好" onClose={props.onClose}>
      <form className="stack-form user-preferences-form" onSubmit={savePreferences}>
        <section className="preference-section">
          <div><Icon name="settings" /><span><strong>界面显示</strong><small>仅影响当前用户看到的页面语言</small></span></div>
          <Field label="界面语言"><select value={uiLanguage} onChange={(event) => setUiLanguage(event.target.value as UiLanguage)}><option value="zh-CN">中文</option><option value="en">English</option></select></Field>
        </section>
        <section className="preference-section">
          <div><Icon name="book" /><span><strong>Agent 工作上下文</strong><small>{props.companyConsole ? props.companyConsole.company.name : "尚未选择公司"}</small></span></div>
          <Field label="当前公司的 Agent 工作语言"><select value={skillLanguage} disabled={!canManageRuntimePreferences} onChange={(event) => setSkillLanguage(event.target.value as RelaySkillLanguage)}><option value="zh-CN">中文 Skill 与 Rule</option><option value="en">English Skills and Rules</option></select></Field>
          <p>影响当前公司全部 Agent，从下一次唤醒开始生效，不改变页面语言。</p>
        </section>
        <section className="preference-section">
          <div><Icon name="network" /><span><strong>Agent 并发</strong><small>所有运行器</small></span></div>
          <Field label="同时运行的 Agent 数量"><input type="number" min="1" max="100" step="1" value={triggerBatchSize} disabled={!canManageRuntimePreferences || loadingTriggerPreferences || savedTriggerBatchSize === null} onChange={(event) => setTriggerBatchSize(event.target.value)} /></Field>
          <p>控制本机所有运行器同时执行的 Agent 上限，从下一轮调度开始生效。{triggerEnvironmentDefault !== null ? ` 环境默认值：${triggerEnvironmentDefault}。` : ""}</p>
        </section>
        <div className="dialog-actions"><button className="button" type="button" onClick={props.onClose}>取消</button><button className="button primary" disabled={busy || loadingTriggerPreferences}>{busy ? "保存中…" : "保存偏好"}</button></div>
      </form>
    </Dialog>
  );
}
