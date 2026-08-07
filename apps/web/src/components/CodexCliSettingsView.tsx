import { FormEvent, useEffect, useState } from "react";
import { api } from "../api/client";
import type { CodexCompanyCliSettings, CodexPersonality, CodexReasoningEffort, CodexReasoningSummary, CodexVerbosity, CodexWebSearch } from "../types/platform";
import { Field, Icon } from "./ui";

type Settings = CodexCompanyCliSettings;

type RunnerProfile = {
  id: string;
  name: string;
  is_default: boolean;
  model: string | null;
  reasoning_effort: CodexReasoningEffort | null;
  reasoning_summary: CodexReasoningSummary | null;
  verbosity: CodexVerbosity | null;
  personality: CodexPersonality | null;
  service_tier: "fast" | null;
  approval_policy: "inherit" | "never" | "on-request";
  sandbox_mode: "inherit" | "read_only" | "workspace_write";
  network_access: boolean | null;
  web_search: CodexWebSearch | null;
  feature_multi_agent: boolean | null;
  feature_remote_plugin: boolean | null;
  feature_hooks: boolean | null;
  feature_goals: boolean | null;
  feature_shell_tool: boolean | null;
};

type Environment = {
  runtime: {
    installed: boolean;
    installed_version: string | null;
    host_os: string;
    host_arch: string;
    executable_path: string | null;
    source: string;
    default_auth: { config: { codex_home: string | null; config_path: string | null } };
  };
};

const reasoningEfforts: CodexReasoningEffort[] = ["minimal", "low", "medium", "high", "xhigh", "max", "ultra"];

export function CodexCliSettingsView(props: {
  companyId: string;
  token: string;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [draft, setDraft] = useState<Settings | null>(null);
  const [profiles, setProfiles] = useState<RunnerProfile[]>([]);
  const [environment, setEnvironment] = useState<Environment | null>(null);
  const [selectedProfileId, setSelectedProfileId] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  async function load() {
    setLoading(true);
    try {
      const [settingsResponse, profilesResponse, environmentResponse] = await Promise.all([
        api<{ settings: Settings }>(`/api/v1/companies/${props.companyId}/codex-cli-settings`, {}, props.token),
        api<{ profiles: Array<{ profile: RunnerProfile }> }>(`/api/v1/companies/${props.companyId}/codex-runner-profiles`, {}, props.token),
        api<Environment>(`/api/v1/companies/${props.companyId}/codex-environments`, {}, props.token),
      ]);
      const nextProfiles = profilesResponse.profiles.map((item) => item.profile);
      setSettings(settingsResponse.settings);
      setDraft(settingsResponse.settings);
      setProfiles(nextProfiles);
      setEnvironment(environmentResponse);
      setSelectedProfileId((current) => current || nextProfiles.find((profile) => profile.is_default)?.id || nextProfiles[0]?.id || "");
    } catch (error) {
      props.onError(error);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { void load(); }, [props.companyId, props.token]);

  function update<K extends keyof Settings>(key: K, value: Settings[K]) {
    setDraft((current) => current ? { ...current, [key]: value } : current);
  }

  async function save(event: FormEvent) {
    event.preventDefault();
    if (!draft) return;
    setSaving(true);
    try {
      const response = await api<{ settings: Settings }>(
        `/api/v1/companies/${props.companyId}/codex-cli-settings`,
        { method: "PUT", body: JSON.stringify(draft) },
        props.token,
      );
      setSettings(response.settings);
      setDraft(response.settings);
      props.onNotice("CLI 公司默认已保存，将在 Agent 下次启动时生效");
    } catch (error) {
      props.onError(error);
    } finally {
      setSaving(false);
    }
  }

  if (loading || !draft || !settings) return <section className="section-card"><small>正在读取 CLI 设置…</small></section>;
  const runtime = environment?.runtime;
  const selectedProfile = profiles.find((profile) => profile.id === selectedProfileId) ?? null;
  const rows = effectiveRows(settings, selectedProfile);

  return (
    <div className="content-stack codex-cli-settings-page">
      <section className="cli-settings-statusbar">
        <div><span className={`status-dot ${runtime?.installed ? "online" : ""}`} /><strong>{runtime?.installed ? `Codex ${runtime.installed_version ?? "已安装"}` : "未检测到 Codex CLI"}</strong><small>{runtime ? `${platformName(runtime.host_os)} · ${runtime.host_arch}` : "等待 Trigger 上报"}</small></div>
        <div><small>托管方式</small><strong>启动参数注入</strong><code>--config</code></div>
        <div><small>CODEX_HOME</small><strong>{runtime?.default_auth.config.codex_home ?? "未发现"}</strong><code>{runtime?.default_auth.config.config_path ?? "~/.codex/config.toml"}</code></div>
        <button className="button small" type="button" onClick={() => void load()}><Icon name="refresh" />刷新</button>
      </section>

      <form className="section-card cli-settings-form" onSubmit={save}>
        <div className="section-heading"><div><span className="eyebrow">COMPANY DEFAULT</span><h2>公司默认</h2></div><button className="button primary small" disabled={saving}>{saving ? "保存中…" : "保存设置"}</button></div>
        <div className="cli-settings-groups">
          <fieldset><legend>模型</legend><div className="cli-settings-grid">
            <Field label="默认模型"><input value={draft.model ?? ""} onChange={(event) => update("model", event.target.value || null)} placeholder="Codex / 项目配置" /></Field>
            <Field label="思考等级"><select value={draft.reasoning_effort ?? ""} onChange={(event) => update("reasoning_effort", (event.target.value || null) as CodexReasoningEffort | null)}><option value="">模型默认</option>{reasoningEfforts.map((value) => <option key={value}>{value}</option>)}</select></Field>
            <Field label="推理摘要"><select value={draft.reasoning_summary} onChange={(event) => update("reasoning_summary", event.target.value as CodexReasoningSummary)}><option value="auto">auto</option><option value="concise">concise</option><option value="detailed">detailed</option><option value="none">none</option></select></Field>
            <Field label="输出详细度"><select value={draft.verbosity ?? ""} onChange={(event) => update("verbosity", (event.target.value || null) as CodexVerbosity | null)}><option value="">Codex 默认</option><option value="low">low</option><option value="medium">medium</option><option value="high">high</option></select></Field>
            <Field label="Personality"><select value={draft.personality ?? ""} onChange={(event) => update("personality", (event.target.value || null) as CodexPersonality | null)}><option value="">Codex 默认</option><option value="none">none</option><option value="friendly">friendly</option><option value="pragmatic">pragmatic</option></select></Field>
            <Switch label="Fast 模式" detail="service_tier = fast" checked={draft.service_tier === "fast"} onChange={(checked) => update("service_tier", checked ? "fast" : null)} />
          </div></fieldset>
          <fieldset><legend>权限与网络</legend><div className="cli-settings-grid">
            <Field label="审批策略"><select value={draft.approval_policy} onChange={(event) => update("approval_policy", event.target.value as Settings["approval_policy"])}><option value="never">never</option><option value="on-request">on-request</option></select></Field>
            <Field label="沙盒等级"><select value={draft.sandbox_mode} onChange={(event) => update("sandbox_mode", event.target.value as Settings["sandbox_mode"])}><option value="workspace_write">workspace-write</option><option value="read_only">read-only</option></select></Field>
            <Field label="Web Search"><select value={draft.web_search} onChange={(event) => update("web_search", event.target.value as CodexWebSearch)}><option value="disabled">关闭</option><option value="cached">缓存</option><option value="indexed">索引</option><option value="live">实时</option></select></Field>
            <Switch label="工作区网络访问" detail="sandbox network_access" checked={draft.network_access} onChange={(checked) => update("network_access", checked)} />
          </div></fieldset>
          <fieldset><legend>能力开关</legend><div className="cli-feature-switches">
            <Switch label="多 Agent" detail="features.multi_agent" checked={draft.feature_multi_agent} onChange={(checked) => update("feature_multi_agent", checked)} compact />
            <Switch label="插件" detail="features.remote_plugin" checked={draft.feature_remote_plugin} onChange={(checked) => update("feature_remote_plugin", checked)} compact />
            <Switch label="Hooks" detail="features.hooks" checked={draft.feature_hooks} onChange={(checked) => update("feature_hooks", checked)} compact />
            <Switch label="Goals" detail="features.goals" checked={draft.feature_goals} onChange={(checked) => update("feature_goals", checked)} compact />
            <Switch label="Shell" detail="features.shell_tool" checked={draft.feature_shell_tool} onChange={(checked) => update("feature_shell_tool", checked)} compact />
          </div></fieldset>
        </div>
      </form>

      <section className="section-card cli-effective-card">
        <div className="section-heading"><div><span className="eyebrow">EFFECTIVE CONFIG</span><h2>生效状态</h2></div><Field label="运行配置"><select value={selectedProfileId} onChange={(event) => setSelectedProfileId(event.target.value)}><option value="">仅公司默认</option>{profiles.map((profile) => <option value={profile.id} key={profile.id}>{profile.name}{profile.is_default ? "（默认）" : ""}</option>)}</select></Field></div>
        <div className="cli-config-layers"><span className="readonly"><strong>全局 / 项目 config.toml</strong><small>只读 · Relay 不改写</small></span><i>→</i><span><strong>公司默认</strong><small>Relay 托管</small></span><i>→</i><span><strong>运行配置</strong><small>{selectedProfile ? "显式字段覆盖" : "未选择"}</small></span><i>→</i><span><strong>CLI 参数</strong><small>实际生效</small></span></div>
        <div className="cli-effective-table-wrap"><table className="cli-effective-table"><thead><tr><th>设置</th><th>公司值</th><th>运行配置</th><th>实际生效值</th><th>来源</th></tr></thead><tbody>{rows.map((row) => <tr key={row.key}><th>{row.label}<code>{row.key}</code></th><td>{row.company}</td><td>{row.override}</td><td><strong>{row.effective}</strong></td><td><span className={`config-source ${row.source === "运行配置" ? "override" : "company"}`}>{row.source}</span></td></tr>)}</tbody></table></div>
      </section>
    </div>
  );
}

function Switch(props: { label: string; detail: string; checked: boolean; onChange: (checked: boolean) => void; compact?: boolean }) {
  return <label className={`cli-setting-switch ${props.compact ? "compact" : ""}`}><span><strong>{props.label}</strong><small>{props.detail}</small></span><input type="checkbox" checked={props.checked} onChange={(event) => props.onChange(event.target.checked)} /></label>;
}

function effectiveRows(settings: Settings, profile: RunnerProfile | null) {
  const show = (value: unknown, fallback = "未设置") => value === null || value === undefined || value === "" ? fallback : typeof value === "boolean" ? (value ? "开启" : "关闭") : String(value);
  const row = (profileKey: keyof RunnerProfile, companyKey: keyof Settings, label: string, key: string) => {
    const override = profile?.[profileKey];
    const overridden = override !== null && override !== undefined && override !== "inherit";
    const companyConfigured = settings[companyKey] !== null && settings[companyKey] !== undefined && settings[companyKey] !== "";
    return { key, label, company: show(settings[companyKey], "未设置"), override: profile ? show(overridden ? override : null, "继承") : "—", effective: show(overridden ? override : settings[companyKey], "Codex / 项目配置"), source: overridden ? "运行配置" : companyConfigured ? "公司默认" : "Codex 配置" };
  };
  const globalRow = (companyKey: keyof Settings, label: string, key: string) => {
    const configured = settings[companyKey] !== null && settings[companyKey] !== undefined && settings[companyKey] !== "";
    return { key, label, company: show(settings[companyKey], "未设置"), override: profile ? "全局统一" : "—", effective: show(settings[companyKey], "Codex / 项目配置"), source: configured ? "公司默认" : "Codex 配置" };
  };
  return [
    row("model", "model", "模型", "model"), row("reasoning_effort", "reasoning_effort", "思考等级", "model_reasoning_effort"), row("reasoning_summary", "reasoning_summary", "推理摘要", "model_reasoning_summary"), row("verbosity", "verbosity", "输出详细度", "model_verbosity"), row("personality", "personality", "Personality", "personality"), globalRow("service_tier", "Fast 模式", "service_tier"), row("approval_policy", "approval_policy", "审批策略", "approval_policy"), row("sandbox_mode", "sandbox_mode", "沙盒等级", "sandbox_mode"), globalRow("network_access", "网络访问", "sandbox_workspace_write.network_access"), globalRow("web_search", "Web Search", "web_search"), globalRow("feature_multi_agent", "多 Agent", "features.multi_agent"), globalRow("feature_remote_plugin", "插件", "features.remote_plugin"), globalRow("feature_hooks", "Hooks", "features.hooks"), globalRow("feature_goals", "Goals", "features.goals"), globalRow("feature_shell_tool", "Shell", "features.shell_tool"),
  ];
}

function platformName(value: string) {
  if (value === "macos") return "macOS";
  if (value === "windows") return "Windows";
  if (value === "linux") return "Linux";
  return value || "未知系统";
}
