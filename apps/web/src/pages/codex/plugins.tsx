import { useEffect, useState } from "react";
import { api } from "../../api/client";
import type { CompanyRealtimeEvent } from "../../api/types";
import { Pagination, usePagination } from "../../components/Pagination";
import { Icon } from "../../components/ui";
import type { CodexEnvironmentView, CodexPluginCatalog, CodexPluginOperation } from "../../types/platform";
import { codexPluginOperationStatusLabel, formatTime, LoadingState } from "../app/shared";

export function CodexPluginsView(props: {
  companyId: string;
  token: string;
  realtimeEvent: CompanyRealtimeEvent | null;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [catalogs, setCatalogs] = useState<CodexPluginCatalog[]>([]);
  const [operations, setOperations] = useState<CodexPluginOperation[]>([]);
  const [environment, setEnvironment] = useState<CodexEnvironmentView | null>(null);
  const [catalogKey, setCatalogKey] = useState("");
  const [tab, setTab] = useState<"installed" | "available">("installed");
  const [query, setQuery] = useState("");
  const [marketplace, setMarketplace] = useState("");
  const [loading, setLoading] = useState(true);
  const [busyKey, setBusyKey] = useState("");

  async function loadPlugins(silent = false) {
    if (!silent) setLoading(true);
    try {
      const [response, environmentResponse] = await Promise.all([
        api<{ catalogs: CodexPluginCatalog[]; operations: CodexPluginOperation[] }>(
          `/api/v1/companies/${props.companyId}/codex-plugins?operation_limit=80`, {}, props.token,
        ),
        api<CodexEnvironmentView>(
          `/api/v1/companies/${props.companyId}/codex-environments`, {}, props.token,
        ),
      ]);
      setCatalogs(response.catalogs);
      setOperations(response.operations);
      setEnvironment(environmentResponse);
      setCatalogKey((current) => response.catalogs.some((catalog) => pluginCatalogKey(catalog) === current)
        ? current
        : pluginCatalogKey(response.catalogs.find((catalog) => catalog.discovery_status === "ready") ?? response.catalogs[0]));
    } catch (error) {
      props.onError(error);
    } finally {
      if (!silent) setLoading(false);
    }
  }

  useEffect(() => { void loadPlugins(); }, [props.companyId, props.token]);
  useEffect(() => {
    if (props.realtimeEvent?.event_type.startsWith("codex.plugin.")) {
      void loadPlugins(true);
    }
  }, [props.realtimeEvent?.sequence_id]);

  async function requestOperation(operation: CodexPluginOperation["operation"], pluginId: string | null) {
    const catalog = catalogs.find((item) => pluginCatalogKey(item) === catalogKey);
    if (!catalog) return;
    const key = `${operation}:${pluginId ?? "catalog"}`;
    setBusyKey(key);
    try {
      await api(
        `/api/v1/companies/${props.companyId}/codex-plugins/operations`,
        { method: "POST", body: JSON.stringify({ target_runner_id: catalog.runner_id, target_selector: catalog.target_selector, operation, plugin_id: pluginId }) },
        props.token,
      );
      props.onNotice(operation === "install" ? "安装请求已交给宿主机 Trigger" : operation === "remove" ? "卸载请求已交给宿主机 Trigger" : "插件目录刷新请求已提交");
      await loadPlugins(true);
    } catch (error) {
      props.onError(error);
    } finally {
      setBusyKey("");
    }
  }

  const catalog = catalogs.find((item) => pluginCatalogKey(item) === catalogKey) ?? null;
  const runnerId = catalog?.runner_id ?? "";
  const targetSelector = catalog?.target_selector ?? "";
  const environmentName = targetSelector === "default"
    ? "宿主机默认登录"
    : environment?.profiles.find((profile) => profile.selector === targetSelector)?.name ?? targetSelector;
  const plugins = (tab === "installed" ? catalog?.installed : catalog?.available) ?? [];
  const marketplaces = Array.from(new Set(plugins.map((plugin) => plugin.marketplaceName))).sort();
  const visible = plugins.filter((plugin) => {
    const matchesQuery = !query.trim() || `${plugin.name} ${plugin.pluginId} ${plugin.marketplaceName}`.toLowerCase().includes(query.trim().toLowerCase());
    return matchesQuery && (!marketplace || plugin.marketplaceName === marketplace);
  });
  const pluginPagination = usePagination(visible, 12, `${runnerId}:${targetSelector}:${tab}:${query}:${marketplace}`);
  const visibleOperations = operations.filter((operation) => operation.target_runner_id === runnerId && operation.target_selector === targetSelector);
  const operationPagination = usePagination(visibleOperations, 8, `${runnerId}:${targetSelector}`);
  const activeOperationKeys = new Set(operations
    .filter((operation) => operation.target_runner_id === runnerId && operation.target_selector === targetSelector && ["queued", "running"].includes(operation.status))
    .map((operation) => `${operation.operation}:${operation.plugin_id ?? "catalog"}`));

  return (
    <div className="content-stack codex-plugin-page">
      <section className="section-card">
        <div className="section-heading plugin-catalog-heading">
          <div><span className="eyebrow">PLUGIN CATALOG</span><h2>插件目录</h2><p>{catalog ? `${catalog.hostname} · ${environmentName} · ${catalog.codex_version ?? "Codex CLI"} · ${formatTime(catalog.discovered_at)}` : "等待宿主机 Trigger 上报 Codex CLI 插件目录。"}</p></div>
          <div className="section-heading-actions">
            {catalogs.length > 1 ? <select value={catalogKey} onChange={(event) => setCatalogKey(event.target.value)}>{catalogs.map((item) => <option key={pluginCatalogKey(item)} value={pluginCatalogKey(item)}>{item.hostname} · {item.target_selector === "default" ? "宿主机默认登录" : environment?.profiles.find((profile) => profile.selector === item.target_selector)?.name ?? item.target_selector}</option>)}</select> : null}
            <button className="button small" onClick={() => void requestOperation("refresh", null)} disabled={!runnerId || Boolean(busyKey) || activeOperationKeys.has("refresh:catalog")}><Icon name="refresh" /> 刷新目录</button>
          </div>
        </div>
        {loading ? <LoadingState /> : !catalog ? (
          <div className="empty-inline compact-empty"><Icon name="plugin" /><h3>还没有发现宿主机插件</h3><p>请确认本地 ai-chat-agent-trigger 正在运行，并且与终端使用同一个 Codex CLI 用户环境。</p></div>
        ) : (
          <>
            <div className="plugin-summary">
              <span><small>已安装</small><strong>{catalog.installed.length}</strong></span>
              <span><small>可安装</small><strong>{catalog.available.length}</strong></span>
              <span><small>Marketplace</small><strong>{catalog.marketplaces.length}</strong></span>
              <span><small>能力指纹</small><code>{catalog.fingerprint.slice(0, 12)}</code></span>
            </div>
            {catalog.discovery_status === "empty" ? <div className="credential-warning"><Icon name="alert" /><p><strong>当前认证环境没有插件 Marketplace</strong><small>这里读取的是「{environmentName}」的独立 Codex 配置。请先完成该环境登录或配置 Marketplace，再点击刷新目录；空目录不会再被当作成功。</small></p></div> : null}
            <div className="plugin-toolbar">
              <div className="project-detail-tabs"><button className={tab === "installed" ? "active" : ""} onClick={() => setTab("installed")}>已安装 {catalog.installed.length}</button><button className={tab === "available" ? "active" : ""} onClick={() => setTab("available")}>可安装 {catalog.available.length}</button></div>
              <div className="plugin-filters"><label><Icon name="search" /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索插件" /></label><select value={marketplace} onChange={(event) => setMarketplace(event.target.value)}><option value="">全部来源</option>{marketplaces.map((name) => <option key={name} value={name}>{name}</option>)}</select></div>
            </div>
            {visible.length ? <div className="plugin-grid">{pluginPagination.pageItems.map((plugin) => {
              const operation = tab === "installed" ? "remove" : "install";
              const operationKey = `${operation}:${plugin.pluginId}`;
              const pending = activeOperationKeys.has(operationKey);
              return <article className="plugin-card" key={plugin.pluginId}><div className="plugin-card-icon"><Icon name="plugin" /></div><div className="plugin-card-main"><div><strong>{plugin.name}</strong><span className={plugin.enabled ? "plugin-enabled" : "plugin-disabled"}>{plugin.enabled ? "已启用" : "未启用"}</span></div><code>{plugin.pluginId}</code><p><span>{plugin.marketplaceName}</span><span>v{plugin.version}</span><span>{plugin.authPolicy === "ON_INSTALL" ? "安装时认证" : plugin.authPolicy === "ON_USE" ? "使用时认证" : "无额外认证"}</span></p></div><button className={`button small ${tab === "available" ? "primary" : ""}`} onClick={() => void requestOperation(operation, plugin.pluginId)} disabled={Boolean(busyKey) || pending}>{pending || busyKey === operationKey ? "处理中…" : tab === "installed" ? "卸载" : "安装"}</button></article>;
            })}<Pagination {...pluginPagination} onPageChange={pluginPagination.setPage} /></div> : <div className="empty-inline compact-empty"><Icon name="search" /><h3>没有匹配的插件</h3><p>换一个关键词或来源筛选。</p></div>}
          </>
        )}
      </section>

      <section className="section-card">
        <div className="section-heading"><div><span className="eyebrow">OPERATIONS</span><h2>安装任务</h2><p>失败会自动重试最多 3 次；最终失败会保留宿主机 Codex CLI 返回的错误。</p></div><span className="count-badge">{operations.length}</span></div>
        {visibleOperations.length ? <div className="plugin-operation-list">{operationPagination.pageItems.map((operation) => <div key={operation.id}><span className={`status-badge ${operation.status}`}><span className="status-dot" />{codexPluginOperationStatusLabel(operation.status)}</span><strong>{operation.operation === "install" ? "安装" : operation.operation === "remove" ? "卸载" : "刷新目录"}</strong><code>{operation.plugin_id ?? operation.target_runner_id}</code><small>尝试 {operation.attempt_count}/3 · {formatTime(operation.requested_at)}</small><p>{operation.error_message ?? (operation.status === "succeeded" ? "操作完成，下一次 Agent 会话将加载最新能力。" : "等待宿主机 Trigger 处理。")}</p></div>)}<Pagination {...operationPagination} onPageChange={operationPagination.setPage} /></div> : <div className="empty-inline compact-empty"><Icon name="plugin" /><h3>还没有安装任务</h3><p>从可安装列表选择插件即可。</p></div>}
      </section>
    </div>
  );
}

function pluginCatalogKey(catalog: CodexPluginCatalog | undefined) {
  return catalog ? `${catalog.runner_id}\n${catalog.target_selector}` : "";
}
