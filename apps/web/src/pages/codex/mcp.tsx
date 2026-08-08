import { FormEvent, useEffect, useState } from "react";
import { api } from "../../api/client";
import { Pagination, usePagination } from "../../components/Pagination";
import { Field, Icon } from "../../components/ui";
import type { CodexEnvironmentView, CodexMcpEnvironmentSnapshot, CodexMcpServer } from "../../types/platform";
import { formatTime, LoadingState } from "../app/shared";

export function CodexMcpView(props: {
  companyId: string;
  token: string;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [environment, setEnvironment] = useState<CodexEnvironmentView | null>(null);
  const [targetSelector, setTargetSelector] = useState("default");
  const [creating, setCreating] = useState(false);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState("");
  const [name, setName] = useState("");
  const [transport, setTransport] = useState<"streamable_http" | "stdio">("streamable_http");
  const [url, setUrl] = useState("");
  const [command, setCommand] = useState("");
  const [argsText, setArgsText] = useState("");
  const [bearerTokenEnvVar, setBearerTokenEnvVar] = useState("");

  async function loadEnvironment(silent = false) {
    if (!silent) setLoading(true);
    try {
      const response = await api<CodexEnvironmentView>(
        `/api/v1/companies/${props.companyId}/codex-environments`,
        {},
        props.token,
      );
      setEnvironment(response);
      const selectors = ["default", ...response.profiles.filter((profile) => profile.status === "active").map((profile) => profile.selector)];
      setTargetSelector((current) => selectors.includes(current) ? current : "default");
    } catch (error) {
      props.onError(error);
    } finally {
      if (!silent) setLoading(false);
    }
  }

  useEffect(() => { void loadEnvironment(); }, [props.companyId, props.token]);
  const hasPendingOperation = environment?.mcp_environments.some((snapshot) => !["idle", "failed"].includes(snapshot.operation_status));
  useEffect(() => {
    const timer = window.setTimeout(() => void loadEnvironment(true), hasPendingOperation ? 2_000 : 30_000);
    return () => window.clearTimeout(timer);
  }, [hasPendingOperation, environment?.runtime.updated_at, props.companyId, props.token]);

  const snapshot = environment?.mcp_environments.find((item) => item.selector === targetSelector) ?? null;
  const profile = environment?.profiles.find((item) => item.selector === targetSelector) ?? null;
  const servers = snapshot?.servers ?? [];
  const serverPagination = usePagination(servers, 9, targetSelector);
  const operationBusy = Boolean(snapshot && !["idle", "failed"].includes(snapshot.operation_status));

  function resetForm() {
    setName("");
    setUrl("");
    setCommand("");
    setArgsText("");
    setBearerTokenEnvVar("");
  }

  async function refresh() {
    setBusy("refresh");
    try {
      await api(
        `/api/v1/companies/${props.companyId}/codex-mcp-servers/refresh`,
        { method: "POST", body: JSON.stringify({ target_selector: targetSelector }) },
        props.token,
      );
      props.onNotice("MCP 刷新请求已交给宿主机 Trigger");
      await loadEnvironment(true);
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy("");
    }
  }

  async function addServer(event: FormEvent) {
    event.preventDefault();
    setBusy("add");
    try {
      await api(
        `/api/v1/companies/${props.companyId}/codex-mcp-servers`,
        {
          method: "POST",
          body: JSON.stringify({
            target_selector: targetSelector,
            name,
            transport,
            url: transport === "streamable_http" ? url : null,
            command: transport === "stdio" ? command : null,
            args: transport === "stdio" ? argsText.split("\n").map((item) => item.trim()).filter(Boolean) : [],
            bearer_token_env_var: transport === "streamable_http" && bearerTokenEnvVar.trim() ? bearerTokenEnvVar.trim() : null,
          }),
        },
        props.token,
      );
      resetForm();
      setCreating(false);
      props.onNotice("MCP 配置已排队，Trigger 将写入目标 Codex CLI 环境");
      await loadEnvironment(true);
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy("");
    }
  }

  async function removeServer(server: CodexMcpServer) {
    setBusy(`remove:${server.name}`);
    try {
      await api(
        `/api/v1/companies/${props.companyId}/codex-mcp-servers/${encodeURIComponent(targetSelector)}/${encodeURIComponent(server.name)}`,
        { method: "DELETE" },
        props.token,
      );
      props.onNotice(`已提交删除 MCP「${server.name}」的请求`);
      await loadEnvironment(true);
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy("");
    }
  }

  return (
    <div className="content-stack codex-mcp-page">
      <section className="section-card">
        <div className="section-heading mcp-catalog-heading">
          <div><span className="eyebrow">MCP ENVIRONMENT</span><h2>已配置的 MCP</h2><p>{snapshot?.last_checked_at ? `由 Trigger 调用 codex mcp list 发现 · ${formatTime(snapshot.last_checked_at)}` : "等待宿主机 Trigger 上报 MCP 列表。"}</p></div>
          <div className="section-heading-actions">
            <select value={targetSelector} onChange={(event) => setTargetSelector(event.target.value)}>
              <option value="default">宿主机默认 Codex</option>
              {environment?.profiles.filter((item) => item.status === "active").map((item) => <option key={item.id} value={item.selector}>{item.name} · 独立 CODEX_HOME</option>)}
            </select>
            <button className="button small" onClick={() => void refresh()} disabled={Boolean(busy) || operationBusy || !environment?.runtime.installed}><Icon name="refresh" /> {busy === "refresh" ? "提交中…" : "刷新"}</button>
            <button className="button primary small" onClick={() => setCreating((value) => !value)} disabled={operationBusy || !environment?.runtime.installed}><Icon name="plus" /> 添加 MCP</button>
          </div>
        </div>

        <div className="mcp-environment-summary">
          <span><small>目标环境</small><strong>{targetSelector === "default" ? "宿主机默认" : profile?.name ?? targetSelector}</strong></span>
          <span><small>已发现</small><strong>{servers.length}</strong></span>
          <span><small>用户配置</small><strong>{servers.filter((server) => server.configured_by_user).length}</strong></span>
          <span><small>Relay 托管</small><strong>{servers.filter((server) => server.managed_by_relay).length}</strong></span>
          <span><small>当前状态</small><strong>{codexMcpOperationLabel(snapshot?.operation_status ?? "idle")}</strong></span>
        </div>

        {snapshot?.last_error ? <div className="mcp-error-banner"><Icon name="alert" /><span><strong>MCP 操作失败</strong><small>{snapshot.last_error}</small></span></div> : null}

        {creating ? (
          <form className="mcp-create-form" onSubmit={addServer}>
            <div className="mcp-create-form-head"><div><strong>添加到 {targetSelector === "default" ? "宿主机默认 Codex" : profile?.name ?? targetSelector}</strong><small>配置由 Trigger 通过官方 codex mcp add 命令写入。</small></div><button type="button" className="icon-button" onClick={() => { setCreating(false); resetForm(); }}><Icon name="close" /></button></div>
            <div className="mcp-create-grid">
              <Field label="MCP 名称"><input value={name} onChange={(event) => setName(event.target.value)} placeholder="例如 openaiDeveloperDocs" pattern="[A-Za-z0-9_-]+" required maxLength={80} /></Field>
              <Field label="连接方式"><select value={transport} onChange={(event) => setTransport(event.target.value as "streamable_http" | "stdio")}><option value="streamable_http">Streamable HTTP</option><option value="stdio">本地 stdio</option></select></Field>
              {transport === "streamable_http" ? <><Field label="MCP URL"><input type="url" value={url} onChange={(event) => setUrl(event.target.value)} placeholder="https://example.com/mcp" required /></Field><Field label="Bearer Token 环境变量（可选）"><input value={bearerTokenEnvVar} onChange={(event) => setBearerTokenEnvVar(event.target.value)} placeholder="例如 FIGMA_OAUTH_TOKEN" pattern="[A-Za-z_][A-Za-z0-9_]*" /></Field></> : <><Field label="启动命令"><input value={command} onChange={(event) => setCommand(event.target.value)} placeholder="例如 npx" required /></Field><Field label="参数（每行一个）"><textarea value={argsText} onChange={(event) => setArgsText(event.target.value)} placeholder={"-y\n@upstash/context7-mcp"} rows={4} /></Field></>}
            </div>
            <p className="mcp-security-note"><Icon name="shield" /> 不要把 Token 直接写进 URL、命令或参数。HTTP Token 只填写环境变量名称，并在 Trigger 环境及 `AGENT_TRIGGER_CODEX_ENV_ALLOWLIST` 中提供。</p>
            <div className="mcp-create-actions"><button type="button" className="button small" onClick={() => { setCreating(false); resetForm(); }} disabled={Boolean(busy)}>取消</button><button className="button primary small" disabled={Boolean(busy) || servers.some((server) => server.name === name)}>{busy === "add" ? "正在提交…" : servers.some((server) => server.name === name) ? "名称已存在" : "保存到 Codex CLI"}</button></div>
          </form>
        ) : null}

        {loading ? <LoadingState /> : !snapshot ? <div className="empty-inline compact-empty"><Icon name="network" /><h3>等待 MCP 发现</h3><p>Trigger 启动后会自动读取现有 Codex MCP 配置。</p></div> : servers.length ? (
          <div className="mcp-server-grid">
            {serverPagination.pageItems.map((server) => <article className={`mcp-server-card ${server.enabled ? "enabled" : "disabled"}`} key={server.name}><div className="mcp-server-card-head"><span className="mcp-server-icon"><Icon name="network" /></span><div><strong>{server.name}</strong><small>{server.managed_by_relay ? "Chrome DevTools MCP · 项目隔离" : server.transport === "streamable_http" ? "Streamable HTTP" : server.transport === "stdio" ? "本地 stdio" : server.transport}</small></div><span className={server.enabled ? "plugin-enabled" : "plugin-disabled"}>{server.enabled ? "已启用" : "已停用"}</span></div><div className="mcp-server-endpoint">{server.address ? <code>{server.address}</code> : <code>{server.command ?? "隐藏命令"}{server.argument_count ? ` · ${server.argument_count} 个参数` : ""}</code>}</div><div className="mcp-server-meta"><span>{server.managed_by_relay ? "Relay 托管" : server.configured_by_user ? "用户配置" : "插件提供"}</span><span>{server.managed_by_relay ? "网站访问需审批" : codexMcpAuthLabel(server.auth_status)}</span>{server.bearer_token_env_var ? <span>Token: {server.bearer_token_env_var}</span> : null}</div>{server.disabled_reason ? <p>{server.disabled_reason}</p> : null}<div className="mcp-server-actions">{server.configured_by_user && !server.managed_by_relay ? <button className="button small danger" onClick={() => void removeServer(server)} disabled={Boolean(busy) || operationBusy}>{busy === `remove:${server.name}` ? "提交中…" : "删除"}</button> : <small>{server.managed_by_relay ? "自动注入所有项目 Agent" : "随插件安装，不能在此删除"}</small>}</div></article>)}
            <Pagination {...serverPagination} onPageChange={serverPagination.setPage} />
          </div>
        ) : <div className="empty-inline compact-empty"><Icon name="network" /><h3>当前环境还没有 MCP</h3><p>点击“添加 MCP”，配置 HTTP 服务或本地 stdio 服务。</p></div>}
      </section>
    </div>
  );
}

function codexMcpOperationLabel(status: CodexMcpEnvironmentSnapshot["operation_status"]) {
  if (["refresh_pending", "refreshing"].includes(status)) return "正在刷新";
  if (["add_pending", "adding"].includes(status)) return "正在添加";
  if (["remove_pending", "removing"].includes(status)) return "正在删除";
  if (status === "failed") return "需要处理";
  return "已同步";
}

function codexMcpAuthLabel(status: string | null) {
  if (status === "authenticated" || status === "logged_in") return "已认证";
  if (status === "not_logged_in") return "等待 OAuth 登录";
  if (status === "unsupported") return "无需 OAuth";
  return status || "认证状态未知";
}

