import { type FormEvent, useEffect, useMemo, useState } from "react";
import { api } from "../../api/client";
import { Pagination, usePagination } from "../../components/Pagination";
import { Field, Icon } from "../../components/ui";
import type { AgentMemory, AgentToolApproval, CompanyAgent, CompanyConsole } from "../../types/platform";
import {
  approvalRequestDetail,
  approvalRiskLabel,
  approvalStatusLabel,
  approvalToolLabel,
  Dialog,
  formatTime,
  memoryStatusLabel,
  memoryInjectionLabel,
  memoryScopeLabel,
  memoryTierLabel,
  memoryTypeLabel,
  Metric,
} from "./shared";

export type ApprovalReviewDecision = "approve" | "always_allow" | "always_allow_localhost" | "reject";

export function MemoriesView(props: {
  consoleData: CompanyConsole;
  token: string;
  fixedAgentId?: string;
  fixedProjectId?: string;
  embedded?: boolean;
  onError: (error: unknown) => void;
  onNotice: (message: string) => void;
}) {
  const [memories, setMemories] = useState<AgentMemory[]>([]);
  const [loading, setLoading] = useState(true);
  const [query, setQuery] = useState("");
  const [agentFilter, setAgentFilter] = useState("");
  const [projectFilter, setProjectFilter] = useState("");
  const [scope, setScope] = useState("");
  const [memoryTier, setMemoryTier] = useState("");
  const [status, setStatus] = useState("");
  const [editing, setEditing] = useState<AgentMemory | null>(null);
  const companyId = props.consoleData.company.id;
  const agentId = props.fixedAgentId ?? agentFilter;
  const projectId = props.fixedProjectId ?? projectFilter;
  const scopedAgent = props.consoleData.agents.find((agent) => agent.agent_profile.id === props.fixedAgentId);
  const scopedProject = props.consoleData.projects.find((project) => project.project.id === props.fixedProjectId);

  async function loadMemories() {
    const params = new URLSearchParams({ limit: "500" });
    if (query.trim()) params.set("query", query.trim());
    if (agentId) params.set("owner_agent_id", agentId);
    if (projectId) params.set("project_id", projectId);
    if (scope) params.set("scope", scope);
    if (memoryTier) params.set("memory_tier", memoryTier);
    if (status) params.set("status", status);
    const response = await api<{ memories: AgentMemory[] }>(
      `/api/v1/companies/${companyId}/memories?${params.toString()}`,
      {},
      props.token,
    );
    setMemories(response.memories);
  }

  useEffect(() => {
    let active = true;
    const timer = window.setTimeout(() => {
      setLoading(true);
      void loadMemories()
        .catch((error) => { if (active) props.onError(error); })
        .finally(() => { if (active) setLoading(false); });
    }, query ? 250 : 0);
    return () => { active = false; window.clearTimeout(timer); };
  }, [companyId, props.token, query, agentId, projectId, scope, memoryTier, status]);

  async function updateMemory(memory: AgentMemory, values: Record<string, unknown>, notice: string) {
    try {
      await api(
        `/api/v1/companies/${companyId}/memories/${memory.id}`,
        { method: "PUT", body: JSON.stringify(values) },
        props.token,
      );
      await loadMemories();
      props.onNotice(notice);
    } catch (error) {
      props.onError(error);
      throw error;
    }
  }

  async function deleteMemory(memory: AgentMemory) {
    if (!window.confirm(`确定永久删除记忆“${memory.title}”吗？如果只是暂时不用，建议归档。`)) return;
    try {
      await api(`/api/v1/companies/${companyId}/memories/${memory.id}`, { method: "DELETE" }, props.token);
      await loadMemories();
      props.onNotice("记忆已永久删除");
    } catch (error) {
      props.onError(error);
    }
  }

  const activeCount = memories.filter((memory) => memory.status === "active").length;
  const longTermCount = memories.filter((memory) => memory.status === "active" && memory.memory_tier === "long_term").length;
  const shortTermCount = memories.filter((memory) => memory.status === "active" && memory.memory_tier === "short_term").length;
  const activeInjectionChars = memories
    .filter((memory) => memory.status === "active" && memory.memory_tier === "long_term")
    .reduce((total, memory) => total + memory.injection_cost_chars, 0);
  const agentNames = new Map(props.consoleData.agents.map((agent) => [agent.agent_profile.id, agent.agent_profile.display_name]));
  const projectNames = new Map(props.consoleData.projects.map((project) => [project.project.id, project.project.name]));
  const memoryPagination = usePagination(memories, 9, `${query}:${agentId}:${projectId}:${scope}:${memoryTier}:${status}`);

  return (
    <div className={`content-stack memory-center ${props.embedded ? "embedded-memory-center" : ""}`}>
      <section className="memory-metrics">
        <Metric label="有效记忆" value={String(activeCount)} detail="全部为所属 Agent 私有" />
        <Metric label="长期记忆" value={String(longTermCount)} detail="按作用域注入对应会话" />
        <Metric label="短期记忆" value={String(shortTermCount)} detail="仅通过 MCP 按需检索" />
        <Metric label="长期注入量" value={activeInjectionChars.toLocaleString()} detail={activeInjectionChars > 24_000 ? "偏高：建议整理低价值记忆" : "软治理，不做固定小额截断"} />
      </section>

      <section className="section-card memory-library-card">
        <div className="section-heading memory-library-heading">
          <div>
            <span className="eyebrow">{scopedProject ? "PROJECT MEMORY" : "PRIVATE MEMORY"}</span>
            <h2>{scopedProject ? `${scopedProject.project.name} · 项目记忆` : scopedAgent ? `${scopedAgent.agent_profile.display_name} · 独立记忆` : "Agent 私有记忆库"}</h2>
            <p>{scopedProject ? "仅展示与当前项目关联的 Agent 精华记忆，用于保留项目决策、经验、流程和交接结论。" : scopedAgent ? "这里汇总该 Agent 的全部长期与短期记忆；其他 Agent 无权读取或复用。" : "Human 可以审阅和维护，但 Agent 只能读取自己的记忆，不能搜索或复用其他 Agent 的内容。"}</p>
          </div>
          <span className="count-badge">{memories.length}</span>
        </div>
        <div className="memory-filters">
          <label className="task-search"><Icon name="search" /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索主题、结论、标签或使用场景" /></label>
          {!props.fixedAgentId ? <select value={agentFilter} onChange={(event) => setAgentFilter(event.target.value)}><option value="">全部 Agent</option>{props.consoleData.agents.map((agent) => <option key={agent.agent_profile.id} value={agent.agent_profile.id}>{agent.agent_profile.display_name}</option>)}</select> : null}
          {!props.fixedProjectId ? <select value={projectFilter} onChange={(event) => setProjectFilter(event.target.value)}><option value="">全部项目</option>{props.consoleData.projects.map((project) => <option key={project.project.id} value={project.project.id}>{project.project.name}</option>)}</select> : null}
          <select value={scope} onChange={(event) => setScope(event.target.value)}><option value="">全部作用域</option><option value="agent">Agent 全局</option><option value="control">控制会话</option><option value="project">项目工作</option><option value="session">指定会话</option></select>
          <select value={memoryTier} onChange={(event) => setMemoryTier(event.target.value)}><option value="">全部类型</option><option value="long_term">长期记忆 · 按作用域注入</option><option value="short_term">短期记忆 · MCP 按需查询</option></select>
          <select value={status} onChange={(event) => setStatus(event.target.value)}><option value="">全部状态</option><option value="active">有效</option><option value="archived">已归档</option><option value="superseded">已替代</option></select>
        </div>

        {loading ? <div className="memory-loading"><span className="loader" />正在读取精华记忆…</div> : memories.length ? (
          <div className="memory-grid">
            {memoryPagination.pageItems.map((memory) => (
              <article className={`memory-card ${memory.status} ${memory.pinned ? "pinned" : ""}`} key={memory.id}>
                <header>
                  <div className="memory-card-kinds"><span className={`memory-tier ${memory.memory_tier}`}>{memoryTierLabel(memory.memory_tier)}</span><span className="memory-type">{memoryScopeLabel(memory.scope)}</span><span className={`memory-type ${memory.memory_type}`}>{memoryTypeLabel(memory.memory_type)}</span></div>
                  <div className="memory-card-state">{memory.pinned ? <span title="已置顶">置顶</span> : null}<span className={`memory-status ${memory.status}`}>{memoryStatusLabel(memory.status)}</span></div>
                </header>
                <div className="memory-title"><h3>{memory.title}</h3><code>{memory.topic_key}</code></div>
                <p className="memory-summary">{memory.summary}</p>
                {memory.when_to_use ? <div className="memory-usage"><strong>何时使用</strong><span>{memory.when_to_use}</span></div> : null}
                {memory.tags.length ? <div className="memory-tags">{memory.tags.map((tag) => <span key={tag}>{tag}</span>)}</div> : null}
                <div className="memory-facts">
                  <span><strong>{memory.importance}/5</strong>重要度</span>
                  <span><strong>{memory.confidence}%</strong>置信度</span>
                  <span><strong>{memoryInjectionLabel(memory.injection_mode)}</strong>{memory.scope === "project" && memory.project_id ? `${projectNames.get(memory.project_id) ?? "相关项目"}工作会话` : memory.scope === "control" ? "仅控制会话" : memory.scope === "session" ? "仅指定会话" : "控制与项目工作会话"}</span>
                </div>
                <div className="memory-governance-note"><strong>{memory.injection_cost_chars.toLocaleString()} 字符</strong><span>{memory.classification_reason}</span>{memory.estimated_ttl_days ? <small>建议保留 {memory.estimated_ttl_days} 天{memory.expires_at ? ` · 到期 ${formatTime(memory.expires_at)}` : ""}</small> : null}</div>
                <footer>
                  <div><span className="agent-avatar tiny">{(agentNames.get(memory.owner_agent_id) ?? "A").slice(0, 1)}</span><span><strong>{agentNames.get(memory.owner_agent_id) ?? "未知 Agent"}</strong><small>更新于 {formatTime(memory.updated_at)}{memory.source_refs.length ? ` · ${memory.source_refs.length} 个来源引用` : ""}</small></span></div>
                  <div className="memory-actions">
                    <button className="icon-button" title="编辑精华" onClick={() => setEditing(memory)}><Icon name="book" /></button>
                    {memory.status === "active" ? <button className="button small" onClick={() => void updateMemory(memory, { pinned: !memory.pinned }, memory.pinned ? "已取消置顶" : "记忆已置顶")}>{memory.pinned ? "取消置顶" : "置顶"}</button> : null}
                    {!['archived', 'superseded'].includes(memory.status) ? <button className="button small" onClick={() => void updateMemory(memory, { status: "archived" }, "记忆已归档")}>归档</button> : null}
                    <button className="icon-button danger" title="永久删除" onClick={() => void deleteMemory(memory)}><Icon name="trash" /></button>
                  </div>
                </footer>
              </article>
            ))}
            <Pagination {...memoryPagination} onPageChange={memoryPagination.setPage} />
          </div>
        ) : <div className="empty-inline memory-empty"><Icon name="memory" /><h3>还没有符合条件的精华记忆</h3><p>Agent 会在真实工作中提炼阶段性短期结论或稳定长期规则，再通过 <code>agent.memory</code> 写入。系统不会自动复制聊天记录。</p></div>}
      </section>

      {editing ? <MemoryEditDialog memory={editing} onClose={() => setEditing(null)} onSave={async (values) => { await updateMemory(editing, values, "记忆精华已更新"); setEditing(null); }} /> : null}
    </div>
  );
}

export function MemoryEditDialog(props: { memory: AgentMemory; onClose: () => void; onSave: (values: Record<string, unknown>) => Promise<void> }) {
  const [memoryTier, setMemoryTier] = useState<AgentMemory["memory_tier"]>(props.memory.memory_tier);
  const [title, setTitle] = useState(props.memory.title);
  const [summary, setSummary] = useState(props.memory.summary);
  const [whenToUse, setWhenToUse] = useState(props.memory.when_to_use);
  const [tags, setTags] = useState(props.memory.tags.join(", "));
  const [importance, setImportance] = useState(props.memory.importance);
  const [confidence, setConfidence] = useState(props.memory.confidence);
  const [busy, setBusy] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      await props.onSave({
        memory_tier: memoryTier,
        title,
        summary,
        when_to_use: whenToUse,
        tags: tags.split(/[,，]/).map((tag) => tag.trim()).filter(Boolean),
        importance,
        confidence,
      });
    } finally {
      setBusy(false);
    }
  }

  return <Dialog title="编辑精华记忆" description={`主题键 ${props.memory.topic_key} 保持稳定，用于 Agent 去重和更新同一主题。`} onClose={props.onClose} wide><form className="stack-form memory-edit-form" onSubmit={(event) => void submit(event)}><Field label="生效范围"><input value={`${memoryScopeLabel(props.memory.scope)} · ${memoryInjectionLabel(props.memory.injection_mode)}`} disabled /><small>作用域由 Agent 在形成记忆时确定，项目记忆只会进入对应项目工作会话。</small></Field><Field label="记忆层级"><select value={memoryTier} onChange={(event) => setMemoryTier(event.target.value as AgentMemory["memory_tier"])}><option value="long_term">长期记忆 · 按当前作用域自动注入</option><option value="short_term">短期记忆 · Agent 需要时通过 MCP 查询</option></select><small>只有稳定、长期指导工作的规则才应升级为长期记忆。</small></Field><Field label="标题"><input value={title} maxLength={200} onChange={(event) => setTitle(event.target.value)} required /></Field><Field label="精华结论（不是原始记录）"><textarea value={summary} minLength={10} maxLength={2000} onChange={(event) => setSummary(event.target.value)} required /></Field><Field label="何时使用"><textarea value={whenToUse} maxLength={1000} onChange={(event) => setWhenToUse(event.target.value)} placeholder="说明适用的任务、模块、条件或决策场景" /></Field><div className="form-grid"><Field label="标签（逗号分隔）"><input value={tags} onChange={(event) => setTags(event.target.value)} /></Field><Field label="重要度（1–5）"><input type="number" min={1} max={5} value={importance} onChange={(event) => setImportance(Number(event.target.value))} /></Field><Field label="置信度（0–100）"><input type="number" min={0} max={100} value={confidence} onChange={(event) => setConfidence(Number(event.target.value))} /></Field></div>{props.memory.source_refs.length ? <div className="memory-source-list"><strong>来源引用</strong>{props.memory.source_refs.map((source) => <span key={`${source.source_type}-${source.source_id}`}><b>{source.source_type}</b><code>{source.source_id}</code>{source.label ? <small>{source.label}</small> : null}</span>)}</div> : null}<div className="dialog-actions"><button className="button" type="button" onClick={props.onClose} disabled={busy}>取消</button><button className="button primary" disabled={busy}>{busy ? "保存中…" : "保存精华"}</button></div></form></Dialog>;
}

export function ApprovalsView(props: {
  approvals: AgentToolApproval[];
  agents: CompanyAgent[];
  onReview: (approvalId: string, decision: ApprovalReviewDecision, reviewNote: string) => Promise<void>;
  onError: (error: unknown) => void;
}) {
  const [filter, setFilter] = useState<"pending" | "approved" | "rejected" | "all">("pending");
  const visible = props.approvals.filter((approval) => {
    if (filter === "all") return true;
    if (filter === "approved") return ["approved", "executed"].includes(approval.status);
    return approval.status === filter;
  });
  const approvalPagination = usePagination(visible, 8, filter);
  const pendingCount = props.approvals.filter((approval) => approval.status === "pending").length;
  const completedCount = props.approvals.filter((approval) => ["approved", "executed"].includes(approval.status)).length;
  const rejectedCount = props.approvals.filter((approval) => approval.status === "rejected").length;

  return (
    <div className="content-stack">
      <section className="metric-row approval-metrics">
        <Metric label="待处理" value={String(pendingCount)} detail="Codex 会保持当前 turn 等待" active={filter === "pending"} onClick={() => setFilter("pending")} />
        <Metric label="已通过" value={String(completedCount)} detail="批准后继续原进程" active={filter === "approved"} onClick={() => setFilter("approved")} />
        <Metric label="已拒绝" value={String(rejectedCount)} detail="Codex 收到 decline 后继续判断" active={filter === "rejected"} onClick={() => setFilter("rejected")} />
        <Metric label="审批来源" value="Codex + Agent" detail="查看全部审批记录" active={filter === "all"} onClick={() => setFilter("all")} />
      </section>
      <section className="section-card approval-center-card">
        <div className="section-heading">
          <div><span className="eyebrow">APPROVAL CENTER</span><h2>审批请求</h2><p>这里的 Codex 审批直接连接等待中的 app-server 请求，不会重新启动会话。</p></div>
          <div className="segmented approval-filter"><button className={filter === "pending" ? "active" : ""} onClick={() => setFilter("pending")}>待审批 {pendingCount}</button><button className={filter === "all" ? "active" : ""} onClick={() => setFilter("all")}>全部 {props.approvals.length}</button></div>
        </div>
        {visible.length ? <div className="approval-list">{approvalPagination.pageItems.map((approval) => <ApprovalCard key={approval.id} approval={approval} agents={props.agents} onReview={props.onReview} onError={props.onError} />)}<Pagination {...approvalPagination} onPageChange={approvalPagination.setPage} /></div> : <div className="empty-inline compact-empty"><Icon name="shield" /><h3>{filter === "pending" ? "没有待审批请求" : "还没有审批记录"}</h3><p>选择 on-request 的运行配置后，Codex 需要越权时会自动出现在这里。</p></div>}
      </section>
    </div>
  );
}

export function ApprovalCard(props: {
  approval: AgentToolApproval;
  agents: CompanyAgent[];
  onReview: (approvalId: string, decision: ApprovalReviewDecision, reviewNote: string) => Promise<void>;
  onError: (error: unknown) => void;
}) {
  const [expanded, setExpanded] = useState(props.approval.status === "pending");
  const agentName = props.agents.find((agent) => agent.agent_profile.id === props.approval.requested_by_agent_id)?.agent_profile.display_name ?? "Unknown Agent";
  const detail = approvalRequestDetail(props.approval);
  const alwaysAllowTarget = props.approval.execution_result.approval_mode === "always"
    && typeof props.approval.execution_result.approval_target === "string"
    ? props.approval.execution_result.approval_target
    : null;
  return (
    <article className={`approval-card ${props.approval.status} ${expanded ? "expanded" : "collapsed"}`}>
      <button type="button" className="approval-card-head" aria-expanded={expanded} onClick={() => setExpanded((current) => !current)}>
        <span className="approval-icon"><Icon name={props.approval.approval_source === "codex" ? "terminal" : "shield"} /></span>
        <div><strong>{approvalToolLabel(props.approval.tool_name)}</strong><small>{agentName} · {props.approval.approval_source === "codex" ? "Codex 运行审批" : "Agent 高影响动作"} · {formatTime(props.approval.created_at)}</small></div>
        <span className={`approval-status ${props.approval.status}`}>{approvalStatusLabel(props.approval.status)}</span>
        <Icon name={expanded ? "chevron-up" : "chevron-down"} />
      </button>
      {expanded ? <div className="approval-card-body">
        {props.approval.reason ? <p>{props.approval.reason}</p> : null}
        {detail ? <pre>{detail}</pre> : null}
        <div className="approval-meta"><span>风险：{approvalRiskLabel(props.approval.risk_level)}</span><span>有效期至 {formatTime(props.approval.expires_at)}</span>{alwaysAllowTarget ? <span><b>始终允许</b> · {alwaysAllowTarget}</span> : null}{props.approval.review_note ? <span>备注：{props.approval.review_note}</span> : null}</div>
      </div> : null}
      {expanded && props.approval.status === "pending" ? <ApprovalReviewActions approval={props.approval} onReview={props.onReview} onError={props.onError} /> : null}
    </article>
  );
}

export function ApprovalReviewActions(props: {
  approval: AgentToolApproval;
  onReview: (approvalId: string, decision: ApprovalReviewDecision, reviewNote: string) => Promise<void>;
  onError: (error: unknown) => void;
}) {
  const [reviewNote, setReviewNote] = useState("");
  const [busy, setBusy] = useState<ApprovalReviewDecision | null>(null);
  const canAlwaysAllow = props.approval.tool_name === "codex.website_access"
    && typeof props.approval.arguments.relay_approval_scope === "string"
    && typeof props.approval.arguments.relay_approval_target === "string";
  const canAllowLocalhostPorts = canAlwaysAllow
    && typeof props.approval.arguments.relay_approval_local_target === "string";
  async function review(decision: ApprovalReviewDecision) {
    setBusy(decision);
    try {
      await props.onReview(props.approval.id, decision, reviewNote);
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(null);
    }
  }
  return <div className="approval-actions"><input value={reviewNote} onChange={(event) => setReviewNote(event.target.value)} placeholder="审批备注（可选）" disabled={Boolean(busy)} /><button className="button small danger-outline" onClick={() => void review("reject")} disabled={Boolean(busy)}>{busy === "reject" ? "处理中…" : "拒绝"}</button><button className="button small" title={canAlwaysAllow ? "放行当前 Agent 工作会话内该网站 origin 的后续操作" : undefined} onClick={() => void review("approve")} disabled={Boolean(busy)}>{busy === "approve" ? "处理中…" : canAlwaysAllow ? "本次会话允许" : "允许一次"}</button>{canAlwaysAllow ? <button className="button primary small" title="仅对当前 Agent、当前项目和当前网站 origin 生效" onClick={() => void review("always_allow")} disabled={Boolean(busy)}>{busy === "always_allow" ? "处理中…" : "始终允许此网站"}</button> : null}{canAllowLocalhostPorts ? <button className="button primary small" title="仅对当前 Agent、当前项目的非特权 localhost 预览端口生效" onClick={() => void review("always_allow_localhost")} disabled={Boolean(busy)}>{busy === "always_allow_localhost" ? "处理中…" : "允许本项目本地预览端口"}</button> : null}</div>;
}

export function ApprovalDialog(props: {
  approval: AgentToolApproval;
  agents: CompanyAgent[];
  onReview: (approvalId: string, decision: ApprovalReviewDecision, reviewNote: string) => Promise<void>;
  onError: (error: unknown) => void;
  onClose: () => void;
}) {
  const agentName = props.agents.find((agent) => agent.agent_profile.id === props.approval.requested_by_agent_id)?.agent_profile.display_name ?? "Agent";
  return <Dialog title="Codex 正在等待审批" description={`${agentName} 的当前会话已暂停。批准或拒绝后，同一个 Codex turn 会继续执行。`} onClose={props.onClose} wide><div className="approval-dialog-content"><ApprovalCard approval={props.approval} agents={props.agents} onReview={async (...args) => { await props.onReview(...args); props.onClose(); }} onError={props.onError} /><button className="button wide" onClick={props.onClose}>稍后到审批中心处理</button></div></Dialog>;
}
