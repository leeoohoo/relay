import { useEffect, useMemo, useState } from "react";
import { api } from "../../api/client";
import { Pagination, usePagination } from "../../components/Pagination";
import { Field, Icon } from "../../components/ui";
import type { CompanyAgent, CompanyProject } from "../../types/platform";
import { formatTime, projectAssetTypeLabel } from "../app/shared";
import { activeProjectAgents, agentHasPermission, grantAgentProjectPermission } from "./permissions";

function ProjectAssetStatusBadge({ status }: { status: CompanyProject["assets"][number]["status"] }) {
  const label = { active: "正常", missing: "缺失", deprecated: "已废弃", unknown: "待确认" }[status];
  return <span className={`project-asset-status ${status}`}><span className="status-dot" />{label}</span>;
}


export function ProjectAssetsCard(props: {
  companyId: string;
  project: CompanyProject;
  agents: CompanyAgent[];
  token: string;
  canManage: boolean;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const projectAgents = useMemo(() => activeProjectAgents(props.project, props.agents), [props.project.members, props.agents]);
  const [agentId, setAgentId] = useState(props.project.asset_refresh?.maintainer_agent_id ?? projectAgents[0]?.agent_profile.id ?? "");
  const [intervalMinutes, setIntervalMinutes] = useState(props.project.asset_refresh?.interval_minutes ?? 1440);
  const [enabled, setEnabled] = useState(props.project.asset_refresh?.enabled ?? true);
  const [busy, setBusy] = useState(false);
  const [reloading, setReloading] = useState(false);
  const [search, setSearch] = useState("");
  const [typeFilter, setTypeFilter] = useState("");
  const [statusFilter, setStatusFilter] = useState("");
  const maintainer = props.agents.find((agent) => agent.agent_profile.id === props.project.asset_refresh?.maintainer_agent_id);
  const selectedAgent = projectAgents.find((agent) => agent.agent_profile.id === agentId);
  const selectedAgentNeedsPermission = Boolean(selectedAgent && !agentHasPermission(selectedAgent, "project.assets.manage"));
  const assetTypes = Array.from(new Set(props.project.assets.map((asset) => asset.asset_type))).sort();
  const filteredAssets = props.project.assets.filter((asset) => {
    const normalizedSearch = search.trim().toLocaleLowerCase();
    return (!normalizedSearch
      || asset.name.toLocaleLowerCase().includes(normalizedSearch)
      || asset.description.toLocaleLowerCase().includes(normalizedSearch)
      || asset.locator.toLocaleLowerCase().includes(normalizedSearch))
      && (!typeFilter || asset.asset_type === typeFilter)
      && (!statusFilter || asset.status === statusFilter);
  });
  const assetPagination = usePagination(filteredAssets, 9, `${props.project.project.id}:${search}:${typeFilter}:${statusFilter}`);
  const activeAssetCount = props.project.assets.filter((asset) => asset.status === "active").length;
  const lastAssetUpdate = props.project.assets.reduce<string | null>((latest, asset) => !latest || new Date(asset.updated_at) > new Date(latest) ? asset.updated_at : latest, null);

  useEffect(() => {
    setAgentId(props.project.asset_refresh?.maintainer_agent_id ?? projectAgents[0]?.agent_profile.id ?? "");
    setIntervalMinutes(props.project.asset_refresh?.interval_minutes ?? 1440);
    setEnabled(props.project.asset_refresh?.enabled ?? true);
  }, [props.project.project.id, props.project.asset_refresh?.updated_at]);

  async function saveRefresh(runNow: boolean) {
    if (!agentId) return;
    setBusy(true);
    try {
      if (selectedAgentNeedsPermission && selectedAgent) {
        await grantAgentProjectPermission(props.companyId, selectedAgent, "project.assets.manage", props.token);
      }
      await api(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/asset-refresh`,
        {
          method: "PUT",
          body: JSON.stringify({
            maintainer_agent_id: agentId,
            interval_minutes: intervalMinutes,
            enabled,
            run_now: runNow,
          }),
        },
        props.token,
      );
      props.onNotice(
        selectedAgentNeedsPermission
          ? runNow ? "已授权 Agent，资产维护任务已安排到下一次唤醒" : "已授权 Agent 并保存项目资产更新计划"
          : runNow ? "资产维护任务已安排到下一次 Agent 唤醒" : "项目资产更新计划已保存",
      );
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function reloadAssets() {
    setReloading(true);
    try {
      await props.onChanged();
      props.onNotice("项目资产显示已刷新");
    } catch (error) {
      props.onError(error);
    } finally {
      setReloading(false);
    }
  }

  return (
    <div className="project-assets-layout">
      <section className="project-assets-main">
        <div className="project-tab-heading">
          <div><span className="eyebrow">PROJECT ASSETS</span><h3>项目资产清单</h3><p>由授权 Agent 扫描真实工作区后维护。这里展示代码模块、文档、接口、配置和数据文件的真实位置。</p></div>
          <div className="project-assets-heading-actions">
            <span className="count-badge">{props.project.assets.length}</span>
            <button className="button small" type="button" onClick={() => void reloadAssets()} disabled={reloading}><Icon name="refresh" /> {reloading ? "刷新中…" : "刷新显示"}</button>
          </div>
        </div>
        <div className="project-asset-summary">
          <span><small>全部资产</small><strong>{props.project.assets.length}</strong></span>
          <span><small>正常可用</small><strong>{activeAssetCount}</strong></span>
          <span><small>资产类型</small><strong>{assetTypes.length}</strong></span>
          <span><small>最近更新</small><strong>{lastAssetUpdate ? formatTime(lastAssetUpdate) : "尚未生成"}</strong></span>
        </div>
        {props.project.assets.length ? (
          <>
            <div className="project-asset-filters">
              <label className="task-search"><Icon name="search" /><input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="搜索资产名称、说明或路径" /></label>
              <select value={typeFilter} onChange={(event) => setTypeFilter(event.target.value)}><option value="">全部类型</option>{assetTypes.map((assetType) => <option key={assetType} value={assetType}>{projectAssetTypeLabel(assetType)}</option>)}</select>
              <select value={statusFilter} onChange={(event) => setStatusFilter(event.target.value)}><option value="">全部状态</option><option value="active">正常</option><option value="missing">缺失</option><option value="deprecated">已废弃</option><option value="unknown">待确认</option></select>
            </div>
            {filteredAssets.length ? <div className="project-asset-grid">
              {assetPagination.pageItems.map((asset) => {
                const updater = props.agents.find((agent) => agent.agent_profile.id === asset.updated_by_agent_id);
                return <article className="project-asset-card" key={asset.id}>
                  <div className="project-asset-card-head">
                    <span className="project-asset-icon"><Icon name="folder" /></span>
                    <span><strong>{asset.name}</strong><small>{projectAssetTypeLabel(asset.asset_type)}</small></span>
                    <ProjectAssetStatusBadge status={asset.status} />
                  </div>
                  <p>{asset.description || "这个资产暂时没有补充说明。"}</p>
                  <div className="project-asset-locator"><small>项目内位置</small><code>{asset.locator}</code></div>
                  <footer><span>{updater ? `${updater.agent_profile.display_name} 更新` : "资产清单"}</span><time>{formatTime(asset.updated_at)}</time></footer>
                </article>;
              })}
              <Pagination {...assetPagination} onPageChange={assetPagination.setPage} />
            </div> : <div className="project-assets-empty compact"><Icon name="search" /><strong>没有符合条件的资产</strong><p>调整搜索词、类型或状态筛选后再试。</p></div>}
          </>
        ) : (
          <div className="project-assets-empty">
            <Icon name="folder" />
            <strong>资产清单尚未生成</strong>
            <p>右侧选择项目 Agent，点击“授权并立即更新”。Agent 会扫描项目真实工作区并通过 MCP 回写资产。</p>
            <div className="project-assets-empty-steps"><span><b>1</b>选择维护 Agent</span><span><b>2</b>立即唤醒扫描</span><span><b>3</b>回到这里查看结果</span></div>
          </div>
        )}
      </section>
      <aside className="project-delegation-card asset-refresh-card">
        <span className="eyebrow">PERIODIC REFRESH</span>
        <h4>定期维护设置</h4>
        <p>Trigger 到期后只负责唤醒指定 Agent；这里可直接选择项目成员，保存时会为尚未授权的 Agent 授予资产维护权限。</p>
        <Field label="维护 Agent">
          <select value={agentId} onChange={(event) => setAgentId(event.target.value)} disabled={!props.canManage || busy}>
            <option value="">请选择 Agent</option>
            {projectAgents.map((agent) => <option key={agent.agent_profile.id} value={agent.agent_profile.id}>{agent.agent_profile.display_name}{agentHasPermission(agent, "project.assets.manage") ? "" : "（保存时授权）"}</option>)}
          </select>
        </Field>
        <Field label="更新周期">
          <select value={intervalMinutes} onChange={(event) => setIntervalMinutes(Number(event.target.value))} disabled={!props.canManage || busy}>
            <option value={60}>每小时</option>
            <option value={360}>每 6 小时</option>
            <option value={720}>每 12 小时</option>
            <option value={1440}>每天</option>
            <option value={4320}>每 3 天</option>
            <option value={10080}>每周</option>
          </select>
        </Field>
        <label className="check-row"><input type="checkbox" checked={enabled} onChange={(event) => setEnabled(event.target.checked)} disabled={!props.canManage || busy} /><span>启用定期更新</span></label>
        {props.project.asset_refresh ? (
          <div className="asset-refresh-status">
            <strong>{props.project.asset_refresh.last_completed_at ? "资产维护已运行" : props.project.asset_refresh.last_requested_at ? "已唤醒，等待 Agent 回写" : "维护计划已保存"}</strong>
            <span>当前维护人：{maintainer?.agent_profile.display_name ?? "未知 Agent"}</span>
            <span>下次检查：{formatTime(props.project.asset_refresh.next_refresh_at)}</span>
            <span>最近完成：{props.project.asset_refresh.last_completed_at ? formatTime(props.project.asset_refresh.last_completed_at) : "尚未完成"}</span>
          </div>
        ) : null}
        {!projectAgents.length ? <div className="git-security-note">这个项目还没有活跃 Agent，请先在项目中添加成员。</div> : selectedAgentNeedsPermission ? <div className="git-security-note">保存后将授予所选 Agent 的项目资产维护权限。</div> : null}
        {props.canManage ? (
          <div className="project-asset-actions">
            <button className="button" type="button" onClick={() => void saveRefresh(false)} disabled={!agentId || busy}>{selectedAgentNeedsPermission ? "授权并保存计划" : "保存计划"}</button>
            <button className="button primary" type="button" onClick={() => void saveRefresh(true)} disabled={!agentId || !enabled || busy}>{busy ? "提交中…" : selectedAgentNeedsPermission ? "授权并立即更新" : "保存并立即更新"}</button>
          </div>
        ) : null}
      </aside>
    </div>
  );
}

