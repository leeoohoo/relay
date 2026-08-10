import { useState, type ReactNode } from "react";
import { Icon } from "../../components/ui";
import type { UiLanguage } from "../../i18n/uiLanguage";
import type { RelaySkillDocument, RelaySkillLanguage } from "../../relaySkills";
import type { Session } from "../../types/appShell";
import type {
  AgentMemory,
  AgentProfile,
  AgentToolApproval,
  CodexPluginOperation,
  CodexReasoningEffort,
  CodexSession,
  CodexTriggerRun,
  CodexTriggerView,
  CompanyAgent,
  CompanyProfessionSummary,
  CompanyProjectTask,
  CompanyProjectTypeSummary,
} from "../../types/platform";

const SESSION_KEY = "agent_company_session";

export function Dialog(props: { title: string; description?: string; onClose: () => void; children: ReactNode; wide?: boolean; extraWide?: boolean }) {
  return <div className="dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) props.onClose(); }}><div className={`dialog ${props.extraWide ? "extra-wide" : props.wide ? "wide" : ""}`} role="dialog" aria-modal="true"><div className="dialog-head"><div><h2>{props.title}</h2>{props.description ? <p>{props.description}</p> : null}</div><button className="icon-button" onClick={props.onClose}><Icon name="close" /></button></div>{props.children}</div></div>;
}

export function CodeBlock(props: { label: string; value: string; secret?: boolean }) {
  const [visible, setVisible] = useState(!props.secret);
  return <div className="code-block"><div><span>{props.label}</span><div>{props.secret ? <button onClick={() => setVisible(!visible)}><Icon name="eye" /> {visible ? "隐藏" : "显示"}</button> : null}<button onClick={() => void copyText(props.value)}><Icon name="copy" /> 复制</button></div></div><pre>{visible ? props.value : "••••••••••••••••••••••••••••••••"}</pre></div>;
}

export function SkillCopyBlock({ documents, step = "3" }: { documents: RelaySkillDocument[]; step?: string }) {
  const [expanded, setExpanded] = useState(false);
  const value = formatSkillBundle(documents);
  const compositionLabel = documents.length <= 1
    ? "按当前权限生成通用协作 Skill"
    : documents.length === 2
      ? "通用协作 Skill + 职业专属 Skill"
      : "通用协作 Skill + 职业专属 Skill + 特殊授权 Skill";
  return <div className="skill-copy-block"><div className="skill-copy-head"><div><span>{step}. 对应的完整 Agent Skill</span><small>{compositionLabel}</small></div><div className="skill-copy-actions"><button onClick={() => setExpanded(!expanded)}><Icon name={expanded ? "chevron-up" : "chevron-down"} /> {expanded ? "收起" : "查看"}</button><button onClick={() => void copyText(value)}><Icon name="copy" /> 复制完整 Skill</button></div></div><div className="skill-file-list">{documents.map((document) => <span key={document.name}><strong>{document.title}</strong><code>{document.name}/SKILL.md</code></span>)}</div>{expanded ? <pre>{value}</pre> : null}</div>;
}

export function EmptyCompany(props: { onCreate: () => void }) { return <div className="center-state"><span className="brand-mark"><Icon name="network" /></span><span className="eyebrow">START HERE</span><h1>先创建一家公司</h1><p>公司会成为外部 Agent 的身份与通信边界。创建后再添加组织和 Agent 账号。</p><button className="button primary" onClick={props.onCreate}><Icon name="plus" /> 创建公司</button></div>; }
export function LoadingState() { return <div className="center-state"><span className="loader" /><h2>正在读取公司目录</h2></div>; }
export function Metric(props: { label: string; value: string; detail: string; onClick?: () => void; active?: boolean }) {
  const content = <><span>{props.label}</span><strong>{props.value}</strong><small>{props.detail}</small></>;
  return props.onClick
    ? <button type="button" className={`metric interactive ${props.active ? "active" : ""}`} aria-pressed={props.active} onClick={props.onClick}>{content}</button>
    : <div className="metric">{content}</div>;
}
export function StatusBadge({ value }: { value: string }) { const label = { active: "可用", connected: "已连接", not_connected: "待连接", awaiting_activation: "待激活", provisioning: "待激活", pending: "待处理", deleting: "删除中", idle: "就绪", install_pending: "等待安装", installing: "安装中", update_pending: "等待更新", updating: "更新中", suspended: "已暂停", terminated: "已裁撤", key_revoked: "Key 已撤销", key_expired: "Key 已过期", no_key: "无 Key", running: "运行中", succeeded: "成功", failed: "失败", timed_out: "超时", cancelled: "已取消", lease_lost: "租约丢失", approved: "已批准", rejected: "已拒绝" }[value] ?? value; return <span className={`status-badge ${value}`}><span className="status-dot" />{label}</span>; }
export function codexSessionTurnLabel(session: CodexSession) {
  const status = typeof session.checkpoint_json.last_turn_status === "string"
    ? session.checkpoint_json.last_turn_status
    : null;
  if (status === "timed_out" && session.checkpoint_json.continuation_expected === true) return "上轮达到时限，已保留会话等待续跑";
  if (status === "succeeded") return "上轮已完成，会话可继续复用";
  if (status === "failed") return "上轮失败，会话仍保留用于诊断或恢复";
  if (status === "cancelled") return "上轮因暂停而停止，恢复后可继续";
  return session.status === "active" ? "会话可复用，当前未必正在执行" : "历史会话";
}
export function Toast(props: { children: ReactNode; tone?: "error"; onClose: () => void }) { return <div className={`toast ${props.tone ?? ""}`}><span>{props.children}</span><button onClick={props.onClose}><Icon name="close" /></button></div>; }

export function readSession(): Session | null {
  try { const value = localStorage.getItem(SESSION_KEY); return value ? JSON.parse(value) as Session : null; } catch { return null; }
}
export function persistSession(session: Session) { localStorage.setItem(SESSION_KEY, JSON.stringify(session)); }
export async function copyText(value: string) { await navigator.clipboard.writeText(value); }
export function formatSkillBundle(documents: RelaySkillDocument[]) {
  return documents
    .map((document) => `===== ${document.name}/SKILL.md =====\n${document.content.trim()}`)
    .join("\n\n");
}
export function projectAssetTypeLabel(value: string) {
  return {
    code: "代码模块",
    document: "文档",
    api: "接口",
    config: "配置",
    data: "数据",
    database: "数据库",
    script: "脚本",
    service: "服务",
    test: "测试",
  }[value] ?? value;
}
export function relayAgentConnectionNames(agent: AgentProfile) {
  const token = agent.handle
    .replace(/^@/, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "") || agent.id.replace(/-/g, "").slice(0, 8).toLowerCase();
  return {
    environmentVariable: `RELAY_AGENT_KEY_${token.toUpperCase()}`,
    mcpServer: `relay_${token}`,
  };
}
export function formatTime(value: string) { return new Date(value).toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" }); }
export function formatElapsed(value: string) {
  const seconds = Math.max(0, Math.floor((Date.now() - new Date(value).getTime()) / 1000));
  if (seconds < 60) return `${seconds} 秒`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} 分 ${seconds % 60} 秒`;
  return `${Math.floor(minutes / 60)} 小时 ${minutes % 60} 分`;
}
export function formatInterval(seconds: number) {
  if (seconds % 86400 === 0) return `${seconds / 86400} 天`;
  if (seconds % 3600 === 0) return `${seconds / 3600} 小时`;
  if (seconds % 60 === 0) return `${seconds / 60} 分钟`;
  return `${seconds} 秒`;
}

export function formatRunSeconds(seconds: number, language: UiLanguage) {
  return language === "en" ? `${seconds} seconds` : `${seconds} 秒`;
}

export function formatAgentCount(count: number, language: UiLanguage) {
  return language === "en" ? `${count} ${count === 1 ? "Agent" : "Agents"}` : `${count} Agent`;
}

export function codexReasoningEffortLabel(value: CodexReasoningEffort | null, language: UiLanguage = "zh-CN") {
  if (language === "en") {
    if (!value) return "Model Default";
    return ({ none: "None", minimal: "Minimal", low: "Low", medium: "Medium", high: "High", xhigh: "Extra High", max: "Maximum", ultra: "Ultra" } as Record<CodexReasoningEffort, string>)[value];
  }
  if (!value) return "跟随模型默认";
  return ({ none: "关闭", minimal: "最低", low: "低", medium: "中", high: "高", xhigh: "超高", max: "最大", ultra: "极致" } as Record<CodexReasoningEffort, string>)[value];
}
export function companyAgentProfessionKey(agent: CompanyAgent, professions: CompanyProfessionSummary[]) {
  if (agent.profession?.key) return agent.profession.key;
  const title = (agent.membership.job_title || "").trim().toLowerCase();
  const exact = professions.find((profession) => profession.label.toLowerCase() === title);
  if (exact) return exact.key;
  if (title.includes("项目经理") || title.includes("项目负责人") || title === "project manager" || title === "pm") return "project_manager";
  if (title.includes("产品经理") || title.includes("产品负责人") || title.includes("product manager") || title.includes("product owner")) return "product_manager";
  if (title.includes("技术经理") || title.includes("技术负责人") || title.includes("工程经理") || title.includes("研发经理") || title.includes("技术总监") || title === "cto" || title.includes("technical manager") || title.includes("engineering manager") || title.includes("tech lead")) return "technical_manager";
  if (title.includes("测试") || title.includes("质量") || title.includes("qa")) return "qa_engineer";
  if (title.includes("架构师") || title.includes("solution architect") || title.includes("software architect") || title.includes("system architect")) return "solution_architect";
  if (title.includes("前端") || title.includes("frontend") || title.includes("front-end") || title.includes("web developer") || title.includes("web engineer")) return "frontend_engineer";
  if (title.includes("后端") || title.includes("服务端") || title.includes("backend") || title.includes("back-end") || title.includes("server engineer")) return "backend_engineer";
  if (title.includes("移动端") || title.includes("客户端") || title.includes("mobile") || title.includes("android") || title.includes("ios") || title.includes("pda")) return "mobile_engineer";
  if (title.includes("数据工程") || title.includes("数据平台") || title.includes("数据迁移") || title.includes("主数据") || title.includes("data engineer") || title.includes("etl")) return "data_engineer";
  if (title.includes("devops") || title.includes("sre") || title.includes("可靠性") || title.includes("运维工程") || title.includes("平台工程")) return "devops_engineer";
  if (title.includes("ui 设计") || title.includes("界面设计") || title.includes("视觉设计") || title.includes("ui designer") || title.includes("visual designer")) return "ui_designer";
  if (title.includes("ux") || title.includes("用户体验") || title.includes("交互设计") || title.includes("experience designer") || title.includes("interaction designer")) return "ux_designer";
  if (title.includes("产品设计") || title.includes("product designer")) return "product_designer";
  if (title.includes("实施顾问") || title.includes("实施工程") || title.includes("implementation consultant") || title.includes("implementation engineer")) return "implementation_consultant";
  if (title.includes("领域专家") || title.includes("业务专家") || title.includes("行业专家") || title.includes("subject matter expert") || title.includes("sme") || title.endsWith("专家")) return "domain_expert";
  if (title.includes("设计") || title.includes("designer")) return "product_designer";
  if (title.includes("分析") || title.includes("顾问") || title.includes("analyst")) return "business_analyst";
  if (title.includes("运营") || title.includes("销售") || title.includes("operation")) return "operations_specialist";
  if (title.includes("工程") || title.includes("开发") || title.includes("程序") || title.includes("engineer") || title.includes("developer")) return "software_engineer";
  return "general_member";
}
export function projectStatusLabel(value: string) { return ({ planned: "计划中", active: "进行中", paused: "已暂停", blocked: "已阻塞", completed: "已完成", cancelled: "已取消" } as Record<string, string>)[value] ?? value; }
export function projectTypeLabel(value: string, types: CompanyProjectTypeSummary[], language: RelaySkillLanguage = "zh-CN") {
  const projectType = types.find((type) => type.key === value);
  return projectType ? language === "en" ? projectType.label_en : projectType.label : value;
}
export function memoryTypeLabel(value: AgentMemory["memory_type"]) { return ({ fact: "事实", decision: "决策", lesson: "教训", preference: "偏好", procedure: "操作规则", relationship: "协作关系", handoff: "交接" } as Record<AgentMemory["memory_type"], string>)[value]; }
export function memoryStatusLabel(value: AgentMemory["status"]) { return ({ draft: "待验证", active: "有效", archived: "已归档", superseded: "已替代" } as Record<AgentMemory["status"], string>)[value]; }
export function memoryTierLabel(value: AgentMemory["memory_tier"]) { return value === "long_term" ? "长期" : "短期"; }
export function memoryScopeLabel(value: AgentMemory["scope"]) { return ({ agent: "Agent 全局", control: "控制会话", project: "项目工作", session: "指定会话" } as Record<AgentMemory["scope"], string>)[value]; }
export function memoryInjectionLabel(value: AgentMemory["injection_mode"]) { return value === "always" ? "自动注入" : "MCP 按需检索"; }
export function taskStatusLabel(value: CompanyProjectTask["status"]) { return { todo: "待处理", in_progress: "进行中", blocked: "阻塞", done: "已完成", failed: "失败", cancelled: "已取消" }[value]; }
export function taskPriorityLabel(value: CompanyProjectTask["priority"]) { return { low: "低", normal: "普通", high: "高", urgent: "紧急" }[value]; }
export function formatTaskDue(value: string) { return new Date(value).toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" }); }
export function taskDueClass(task: CompanyProjectTask) { return task.due_at && !["done", "failed", "cancelled"].includes(task.status) && new Date(task.due_at).getTime() < Date.now() ? "task-due overdue" : "task-due"; }
export function toDateTimeLocalValue(value: string | null) {
  if (!value) return "";
  const date = new Date(value);
  const local = new Date(date.getTime() - date.getTimezoneOffset() * 60_000);
  return local.toISOString().slice(0, 16);
}
export function collaborationPreferenceLabel(value: AgentProfile["collaboration_preference"]) { return { available: "可协作", low_cost_only: "仅接受低成本请求", unavailable: "暂不接受请求" }[value] ?? value; }
export function codexTriggerStatusLabel(value: CodexTriggerView["config"]["status"]) { return { active: "已启用", paused: "已暂停", error: "错误" }[value]; }
export function codexOperationalStatusLabel(value: string) { return { running: "执行中", queued: "排队中", idle: "等待检查", paused: "已暂停", error: "错误" }[value] ?? value; }
export function codexActivityPhaseLabel(value: string) { return ({ preparing: "准备工作区", starting: "启动 Codex", session: "连接会话", thinking: "分析", planning: "规划", tool: "调用工具", command: "执行命令", files: "修改文件", searching: "搜索", reporting: "进度说明", continuing: "保存进度", finishing: "收尾", waiting_approval: "等待审批", approval_delivery_failed: "审批投递失败", approval_rejected: "审批未通过", running: "执行中", completed: "已完成", failed: "失败", timed_out: "超时", cancelled: "已取消", lease_lost: "进程中断" } as Record<string, string>)[value] ?? value; }
export function codexTriggerTypeLabel(value: string) { return { scheduled: "定时", manual: "手动", run_now: "手动", message: "消息", task: "任务", asset_refresh: "资产维护" }[value] ?? value; }
export function codexPluginOperationStatusLabel(value: CodexPluginOperation["status"]) { return { queued: "排队中", running: "执行中", succeeded: "已完成", failed: "失败" }[value]; }
export function codexRunDisplayMessage(run: CodexTriggerRun) {
  if (run.final_message_summary) return run.final_message_summary;
  if (run.error_message) return run.error_message;
  return {
    running: "本轮仍在执行，尚未完成",
    succeeded: "Codex 已完成本轮",
    timed_out: "本轮运行超时",
    cancelled: "本轮已取消",
    lease_lost: "本轮租约已失效",
    failed: "本轮运行失败",
  }[run.status] ?? "等待运行结果";
}
export function approvalToolLabel(value: string) { return ({ "codex.command_execution": "执行命令", "codex.file_change": "修改受保护文件", "codex.permissions": "申请额外权限", "codex.website_access": "访问网站", "agent.staff.hire": "扩招 Agent", "agent.staff.suspend": "暂停 Agent", "agent.staff.terminate": "裁撤 Agent", "company.project.task.reassign": "重新分配任务" } as Record<string, string>)[value] ?? value; }
export function approvalStatusLabel(value: AgentToolApproval["status"]) { return { pending: "待审批", approved: "已批准", executing: "执行中", executed: "已执行", rejected: "已拒绝", expired: "已过期", failed: "失败" }[value]; }
export function approvalRiskLabel(value: AgentToolApproval["risk_level"]) { return { low: "低", medium: "中", high: "高" }[value]; }
export function approvalRequestDetail(approval: AgentToolApproval) {
  const params = typeof approval.arguments.params === "object" && approval.arguments.params !== null ? approval.arguments.params as Record<string, unknown> : approval.arguments;
  if (approval.tool_name === "codex.command_execution") {
    const command = typeof params.command === "string" ? params.command : "";
    const cwd = typeof params.cwd === "string" ? params.cwd : "";
    return [command, cwd ? `cwd: ${cwd}` : ""].filter(Boolean).join("\n");
  }
  if (approval.tool_name === "codex.file_change") {
    const itemId = typeof params.itemId === "string" ? params.itemId : "";
    const grantRoot = typeof params.grantRoot === "string" ? params.grantRoot : "";
    return [itemId ? `文件变更项：${itemId}` : "", grantRoot ? `请求写入：${grantRoot}` : ""].filter(Boolean).join("\n");
  }
  if (approval.tool_name === "codex.permissions") {
    return JSON.stringify(params.permissions ?? params, null, 2);
  }
  if (approval.tool_name === "codex.website_access") {
    const url = typeof approval.arguments.url === "string" ? approval.arguments.url : "";
    const tool = typeof approval.arguments.tool === "string" ? approval.arguments.tool : "";
    return [url, tool ? `浏览器操作：${tool}` : ""].filter(Boolean).join("\n");
  }
  return JSON.stringify(params, null, 2);
}
