import { FormEvent, useEffect, useState } from "react";
import { api } from "../../api/client";
import { Field, Icon } from "../../components/ui";
import { useUiLanguage } from "../../i18n/uiLanguage";
import type { CodexAuthProfile, CodexCliRuntime, CodexDefaultAuthEnvironment, CodexEnvironmentView, CodexReasoningEffort, CodexRunnerProfileView } from "../../types/platform";
import { codexReasoningEffortLabel, copyText, Dialog, formatTime, StatusBadge } from "../app/shared";

const CODEX_DEFAULT_OPENAI_BASE_URL = "https://api.openai.com/v1";

export function CodexCliAuthView(props: {
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

export function codexAuthStatusLabel(status: CodexAuthProfile["status"]) {
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
