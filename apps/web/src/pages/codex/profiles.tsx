import { FormEvent, useEffect, useState } from "react";
import { api } from "../../api/client";
import { Pagination, usePagination } from "../../components/Pagination";
import { Field, Icon } from "../../components/ui";
import { useUiLanguage, type UiLanguage } from "../../i18n/uiLanguage";
import type { CodexApprovalPolicy, CodexAuthProfile, CodexCompanyCliSettings, CodexPersonality, CodexReasoningEffort, CodexReasoningSummary, CodexRunnerProfileView, CodexSandboxMode, CodexVerbosity, CodexWebSearch } from "../../types/platform";
import { codexReasoningEffortLabel, formatAgentCount, formatInterval, formatRunSeconds } from "../app/shared";
import { codexAuthStatusLabel } from "./auth";

type LocalCodexModel = {
  id: string;
  display_name: string;
  default_reasoning_effort: CodexReasoningEffort | null;
  reasoning_efforts: Array<{
    effort: CodexReasoningEffort;
    description: string;
  }>;
};


export function CodexRunnerProfilesPanel(props: {
  companyId: string;
  profiles: CodexRunnerProfileView[];
  loading: boolean;
  authProfiles: CodexAuthProfile[];
  cliSettings: CodexCompanyCliSettings | null;
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
          cliSettings={props.cliSettings}
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
              cliSettings={props.cliSettings}
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
  cliSettings: CodexCompanyCliSettings | null;
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
  const [intervalSeconds, setIntervalSeconds] = useState(profile?.interval_seconds ?? 3600);
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
    const reasoningLabel = profile.reasoning_effort
      ? codexReasoningEffortLabel(profile.reasoning_effort, language)
      : cliSettingsValue(reasoningEffortValue(props.cliSettings, language), language);
    const sandboxLabel = profile.sandbox_mode === "inherit"
      ? cliSettingsValue(sandboxValue(props.cliSettings?.sandbox_mode, language), language)
      : sandboxValue(profile.sandbox_mode, language);
    const approvalLabel = profile.approval_policy === "inherit"
      ? cliSettingsValue(approvalValue(props.cliSettings?.approval_policy, language), language)
      : approvalValue(profile.approval_policy, language);
    return (
      <article className={`runner-profile-card ${profile.is_default ? "default" : ""}`}>
        <div className="runner-profile-main"><span className="runner-profile-icon"><Icon name="terminal" /></span><div><strong>{profile.name}</strong><small>{profile.model || "Codex 默认模型"} · Profile: {profile.codex_profile}</small></div></div>
        <div className="runner-profile-facts">
          <span><small>兜底检查</small><strong>{formatInterval(profile.interval_seconds)}</strong></span>
          <span><small>思考等级</small><strong>{reasoningLabel}</strong></span>
          <span><small>Sandbox</small><strong>{sandboxLabel}</strong></span>
          <span><small>审批</small><strong>{approvalLabel}</strong></span>
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
  const companyModelValue = props.cliSettings?.model ?? (language === "en" ? "Codex / project config" : "Codex / 项目配置");
  const companyReasoningValue = reasoningEffortValue(props.cliSettings, language, defaultReasoningLabel);
  const companyVerbosityValue = props.cliSettings?.verbosity ?? (language === "en" ? "Codex default" : "Codex 默认");
  const companyPersonalityValue = props.cliSettings?.personality ?? (language === "en" ? "Codex default" : "Codex 默认");
  const companyServiceTierValue = props.cliSettings?.service_tier ?? (language === "en" ? "standard / Codex config" : "标准 / Codex 配置");
  const advancedOverrideCount = [
    intervalSeconds !== 3600, Boolean(reasoningEffort), Boolean(reasoningSummary), Boolean(verbosity),
    Boolean(personality), Boolean(serviceTier), sandboxMode !== "inherit", approvalPolicy !== "inherit",
    networkAccess !== "inherit", Boolean(webSearch), featureMultiAgent !== "inherit",
    featureRemotePlugin !== "inherit", featureHooks !== "inherit", featureGoals !== "inherit",
    featureShellTool !== "inherit", maxRunSeconds !== 3600,
  ].filter(Boolean).length;
  return (
    <form className="runner-profile-form" onSubmit={save}>
      <div className="runner-profile-form-head"><div><span className="eyebrow">{profile ? "EDIT PROFILE" : "NEW PROFILE"}</span><h3>{profile ? `编辑 ${profile.name}` : "新建运行配置"}</h3></div><label className="check-row"><input type="checkbox" checked={isDefault} onChange={(event) => setIsDefault(event.target.checked)} disabled={profile?.is_default} />新 Agent 默认使用</label></div>
      <div className="runner-profile-fields runner-profile-basic-fields">
        <Field label="配置名称"><input value={name} onChange={(event) => setName(event.target.value)} placeholder="例如：开发模式" required /></Field>
        <Field label="认证环境"><select value={codexProfile} onChange={(event) => { setCodexProfile(event.target.value); setModel(""); setReasoningEffort(""); }} required><option value="default">默认登录</option>{codexProfile.startsWith("relay_") && !props.authProfiles.some((item) => item.selector === codexProfile) ? <option value={codexProfile}>{codexProfile}（当前不可用）</option> : null}{props.authProfiles.map((item) => <option key={item.id} value={item.selector} disabled={item.status !== "active"}>{item.name}{item.status === "active" ? "" : `（${codexAuthStatusLabel(item.status)}）`}</option>)}</select></Field>
        <Field label="模型"><select value={model} onChange={(event) => { const nextModel = event.target.value; setModel(nextModel); const supported = models.find((item) => item.id === nextModel)?.reasoning_efforts ?? []; if (reasoningEffort && supported.length && !supported.some((item) => item.effort === reasoningEffort)) setReasoningEffort(""); }} disabled={modelsLoading}><option value="">{cliSettingsOption(companyModelValue, language)}</option>{currentModelMissing ? <option value={model}>{model}（当前配置）</option> : null}{models.map((item) => <option key={item.id} value={item.id}>{item.display_name === item.id ? item.id : `${item.display_name} · ${item.id}`}</option>)}</select>{modelsError ? <small className="codex-runtime-error">{modelsError}</small> : null}</Field>
      </div>
      <details className="runner-profile-advanced">
        <summary><span>高级设置</span><small>{advancedOverrideCount ? `已自定义 ${advancedOverrideCount} 项` : "使用系统默认，通常无需修改"}</small></summary>
        <div className="runner-profile-fields">
        <Field label="兜底检查周期（秒）"><input type="number" min={10} max={604800} value={intervalSeconds} onChange={(event) => setIntervalSeconds(Number(event.target.value))} /></Field>
        <Field label="思考等级"><select value={reasoningEffort} onChange={(event) => setReasoningEffort(event.target.value as CodexReasoningEffort | "")} disabled={modelsLoading}><option value="">{cliSettingsOption(companyReasoningValue, language)}</option>{currentReasoningMissing ? <option value={reasoningEffort}>{reasoningEffort}（当前配置）</option> : null}{reasoningOptions.map((item) => <option key={item.effort} value={item.effort}>{codexReasoningEffortLabel(item.effort, language)} · {item.effort}</option>)}</select></Field>
        <Field label="推理摘要"><select value={reasoningSummary} onChange={(event) => setReasoningSummary(event.target.value as CodexReasoningSummary | "")}><option value="">{cliSettingsOption(props.cliSettings?.reasoning_summary ?? "auto", language)}</option><option value="auto">auto</option><option value="concise">concise</option><option value="detailed">detailed</option><option value="none">none</option></select></Field>
        <Field label="输出详细度"><select value={verbosity} onChange={(event) => setVerbosity(event.target.value as CodexVerbosity | "")}><option value="">{cliSettingsOption(companyVerbosityValue, language)}</option><option value="low">low</option><option value="medium">medium</option><option value="high">high</option></select></Field>
        <Field label="Personality"><select value={personality} onChange={(event) => setPersonality(event.target.value as CodexPersonality | "")}><option value="">{cliSettingsOption(companyPersonalityValue, language)}</option><option value="none">none</option><option value="friendly">friendly</option><option value="pragmatic">pragmatic</option></select></Field>
        <Field label="Fast 模式"><select value={serviceTier} onChange={(event) => setServiceTier(event.target.value as "fast" | "")}><option value="">{cliSettingsOption(companyServiceTierValue, language)}</option><option value="fast">开启 fast</option></select></Field>
        <Field label="Sandbox"><select value={sandboxMode} onChange={(event) => setSandboxMode(event.target.value as CodexSandboxMode)}><option value="inherit">{cliSettingsOption(sandboxValue(props.cliSettings?.sandbox_mode, language), language)}</option><option value="workspace_write">workspace-write</option><option value="read_only">read-only</option></select></Field>
        <Field label="审批策略"><select value={approvalPolicy} onChange={(event) => setApprovalPolicy(event.target.value as CodexApprovalPolicy)}><option value="inherit">{cliSettingsOption(approvalValue(props.cliSettings?.approval_policy, language), language)}</option><option value="never">never</option><option value="on-request">on-request</option></select></Field>
        <Field label="工作区网络"><CodexBooleanOverrideSelect value={networkAccess} companyValue={props.cliSettings?.network_access} language={language} onChange={setNetworkAccess} /></Field>
        <Field label="Web Search"><select value={webSearch} onChange={(event) => setWebSearch(event.target.value as CodexWebSearch | "")}><option value="">{cliSettingsOption(webSearchValue(props.cliSettings?.web_search, language), language)}</option><option value="disabled">关闭</option><option value="cached">缓存</option><option value="indexed">索引</option><option value="live">实时</option></select></Field>
        <Field label="多 Agent"><CodexBooleanOverrideSelect value={featureMultiAgent} companyValue={props.cliSettings?.feature_multi_agent} language={language} onChange={setFeatureMultiAgent} /></Field>
        <Field label="插件"><CodexBooleanOverrideSelect value={featureRemotePlugin} companyValue={props.cliSettings?.feature_remote_plugin} language={language} onChange={setFeatureRemotePlugin} /></Field>
        <Field label="Hooks"><CodexBooleanOverrideSelect value={featureHooks} companyValue={props.cliSettings?.feature_hooks} language={language} onChange={setFeatureHooks} /></Field>
        <Field label="Goals"><CodexBooleanOverrideSelect value={featureGoals} companyValue={props.cliSettings?.feature_goals} language={language} onChange={setFeatureGoals} /></Field>
        <Field label="Shell"><CodexBooleanOverrideSelect value={featureShellTool} companyValue={props.cliSettings?.feature_shell_tool} language={language} onChange={setFeatureShellTool} /></Field>
        <Field label="单次最长运行（秒）"><input type="number" min={60} max={7200} value={maxRunSeconds} onChange={(event) => setMaxRunSeconds(Number(event.target.value))} /></Field>
        </div>
      </details>
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
  companyValue: boolean | undefined;
  language: UiLanguage;
  onChange: (value: CodexBooleanOverride) => void;
}) {
  return <select value={props.value} onChange={(event) => props.onChange(event.target.value as CodexBooleanOverride)}><option value="inherit">{cliSettingsOption(booleanValue(props.companyValue, props.language), props.language)}</option><option value="true">开启</option><option value="false">关闭</option></select>;
}

function cliSettingsOption(value: string, language: UiLanguage) {
  return language === "en" ? `Default · ${value}` : `默认 · ${value}`;
}

function cliSettingsValue(value: string, language: UiLanguage) {
  return language === "en" ? `Default · ${value}` : `默认 · ${value}`;
}

function reasoningEffortValue(settings: CodexCompanyCliSettings | null, language: UiLanguage, modelDefault?: string) {
  if (!settings?.reasoning_effort) return modelDefault ?? codexReasoningEffortLabel(null, language);
  return `${codexReasoningEffortLabel(settings.reasoning_effort, language)} · ${settings.reasoning_effort}`;
}

function sandboxValue(value: "read_only" | "workspace_write" | undefined, language: UiLanguage) {
  if (language === "en") return value === "read_only" ? "read-only" : value === "workspace_write" ? "workspace-write" : "company value";
  return value === "read_only" ? "只读" : value === "workspace_write" ? "可写工作区" : "公司级基础值";
}

function approvalValue(value: "never" | "on-request" | undefined, language: UiLanguage) {
  if (language === "en") return value === "never" ? "never" : value === "on-request" ? "on-request" : "company value";
  return value === "never" ? "无需审批" : value === "on-request" ? "Human 审批" : "公司级基础值";
}

function webSearchValue(value: CodexWebSearch | undefined, language: UiLanguage) {
  if (!value) return language === "en" ? "company value" : "公司级基础值";
  if (language === "en") return value;
  return ({ disabled: "关闭", cached: "缓存", indexed: "索引", live: "实时" } as const)[value];
}

function booleanValue(value: boolean | undefined, language: UiLanguage) {
  if (value === undefined) return language === "en" ? "company value" : "公司级基础值";
  return language === "en" ? (value ? "On" : "Off") : value ? "开启" : "关闭";
}
