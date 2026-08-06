import { API_BASE_URL } from "../api/client";
import type { Company, HumanUser, View } from "../types/appShell";
import { useUiLanguage } from "../i18n/uiLanguage";
import { Icon, NavItem } from "./ui";

export function Sidebar(props: {
  user: HumanUser;
  companies: Company[];
  selectedCompanyId: string | null;
  view: View;
  onCompanyChange: (id: string) => void;
  onViewChange: (view: View) => void;
  pendingApprovalCount: number;
  onCreateCompany: () => void;
  onOpenPreferences: () => void;
  onSignOut: () => void;
}) {
  const { language, setLanguage } = useUiLanguage();
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
        <NavItem icon="org" label="组织与 Agent" active={props.view === "agents"} onClick={() => props.onViewChange("agents")} />
        <NavItem icon="book" label="Skill 中心" active={props.view === "skills"} onClick={() => props.onViewChange("skills")} />
        <NavItem icon="git" label="项目中心" active={props.view === "projects"} onClick={() => props.onViewChange("projects")} />
        <NavItem icon="terminal" label="Codex 控制台" active={props.view === "codex"} onClick={() => props.onViewChange("codex")} />
        <NavItem icon="message" label="聊天" badge={props.pendingApprovalCount} active={props.view === "messages"} onClick={() => props.onViewChange("messages")} />
      </nav>
      <div className="sidebar-spacer" />
      <div className="ui-language-switch" role="group" aria-label="Interface language">
        <span>UI</span>
        <button className={language === "zh-CN" ? "active" : ""} type="button" onClick={() => setLanguage("zh-CN")}>中</button>
        <button className={language === "en" ? "active" : ""} type="button" onClick={() => setLanguage("en")}>EN</button>
      </div>
      <div className="mcp-status"><span className="status-dot online" /><div><strong>MCP 服务</strong><small>{API_BASE_URL.replace(/^https?:\/\//, "")}/mcp</small></div></div>
      <div className="user-menu">
        <span className="avatar">{props.user.display_name.slice(0, 1).toUpperCase()}</span>
        <div><strong>{props.user.display_name}</strong><small>{props.user.email}</small></div>
        <button title="用户偏好" onClick={props.onOpenPreferences}><Icon name="settings" /></button>
        <button title="退出登录" onClick={props.onSignOut}><Icon name="logout" /></button>
      </div>
    </aside>
  );
}
