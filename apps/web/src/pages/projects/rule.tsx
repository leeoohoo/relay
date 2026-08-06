import { FormEvent, useEffect, useMemo, useState } from "react";
import { api } from "../../api/client";
import { Field } from "../../components/ui";
import type { CompanyAgent, CompanyProject, CompanyProjectType } from "../../types/platform";
import { formatTime } from "../app/shared";
import { activeProjectAgents, agentHasPermission, grantAgentProjectPermission, preferredProjectRuleAgentId } from "./permissions";

export function ProjectRuleCard(props: {
  companyId: string;
  project: CompanyProject;
  projectTypes: CompanyProjectType[];
  agents: CompanyAgent[];
  token: string;
  canManage: boolean;
  onChanged: () => Promise<void>;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const projectAgents = useMemo(() => activeProjectAgents(props.project, props.agents), [props.project.members, props.agents]);
  const [content, setContent] = useState(props.project.rule?.content ?? "");
  const preferredAgentId = preferredProjectRuleAgentId(props.project, projectAgents);
  const [agentId, setAgentId] = useState(preferredAgentId);
  const [instructions, setInstructions] = useState("");
  const [busy, setBusy] = useState(false);
  const selectedAgent = projectAgents.find((agent) => agent.agent_profile.id === agentId);
  const selectedAgentNeedsPermission = Boolean(selectedAgent && !agentHasPermission(selectedAgent, "project.rules.manage"));
  const systemType = props.projectTypes.find((type) => type.key === props.project.project.project_type);

  useEffect(() => setContent(props.project.rule?.content ?? ""), [props.project.project.id, props.project.rule?.updated_at]);
  useEffect(() => {
    if (!projectAgents.some((agent) => agent.agent_profile.id === agentId)) {
      setAgentId(preferredAgentId);
    }
  }, [agentId, preferredAgentId, projectAgents]);

  async function saveRule(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      await api(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/rule`,
        { method: "PUT", body: JSON.stringify({ content }) },
        props.token,
      );
      props.onNotice("项目 Rule 已保存");
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  async function requestGeneration(event: FormEvent) {
    event.preventDefault();
    if (!agentId) return;
    setBusy(true);
    try {
      if (selectedAgentNeedsPermission && selectedAgent) {
        await grantAgentProjectPermission(props.companyId, selectedAgent, "project.rules.manage", props.token);
      }
      await api(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/rule/generate`,
        { method: "POST", body: JSON.stringify({ agent_id: agentId, instructions: instructions || null }) },
        props.token,
      );
      setInstructions("");
      props.onNotice(selectedAgentNeedsPermission ? "已授权并即时唤醒 Agent 生成或更新 Rule" : "已交给授权 Agent；消息将即时唤醒它生成或更新 Rule");
      await props.onChanged();
    } catch (error) {
      props.onError(error);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="project-rule-layout">
      <section className="project-system-rule-card">
        <div className="project-tab-heading">
          <div><span className="eyebrow">SYSTEM PROJECT SKILL</span><h3>{systemType?.label ?? "通用项目"}固定规则</h3><p>这是 Relay 按项目类型自动加载的强制基线。Human Rule 和 Agent 生成内容只能补充，不能删除或弱化。</p></div>
          <span className="pill neutral">自动加载</span>
        </div>
        <pre>{systemType?.rule_markdown ?? "项目类型规则暂不可用"}</pre>
      </section>
      <form className="project-rule-editor" onSubmit={saveRule}>
        <div className="project-tab-heading">
          <div><span className="eyebrow">HUMAN PROJECT RULE</span><h3>项目补充注意事项</h3><p>这里记录项目特有约束、风险和协作规则，并与上方系统固定规则一起进入 Agent Skill。</p></div>
          {props.project.rule ? <small>更新于 {formatTime(props.project.rule.updated_at)}</small> : <small>尚未创建</small>}
        </div>
        <textarea
          className="project-rule-textarea"
          value={content}
          onChange={(event) => setContent(event.target.value)}
          placeholder={"# 项目规则\n\n- 修改前先运行测试\n- 不要提交密钥或本地配置\n- 默认分支禁止直接推送"}
          disabled={!props.canManage || busy}
        />
        <div className="project-tab-actions">
          <small>最多 50,000 个字符；Human 始终可以直接维护。</small>
          {props.canManage ? <button className="button primary" disabled={busy}>{busy ? "保存中…" : "保存 Rule"}</button> : null}
        </div>
      </form>
      <form className="project-delegation-card" onSubmit={requestGeneration}>
        <span className="eyebrow">AGENT DELEGATION</span>
        <h4>授权 Agent 生成</h4>
        <p>这里列出本项目的活跃 Agent；选择尚未授权的 Agent 后，提交时会一并授予 <code>project.rules.manage</code>。</p>
        <Field label="执行 Agent">
          <select value={agentId} onChange={(event) => setAgentId(event.target.value)} disabled={!props.canManage || busy}>
            <option value="">请选择 Agent</option>
            {projectAgents.map((agent) => <option key={agent.agent_profile.id} value={agent.agent_profile.id}>{agent.agent_profile.display_name}{agentHasPermission(agent, "project.rules.manage") ? "" : "（提交时授权）"}</option>)}
          </select>
        </Field>
        <Field label="补充要求（可选）">
          <textarea value={instructions} onChange={(event) => setInstructions(event.target.value)} placeholder="例如：结合仓库现状补充测试、发布和安全约束" disabled={!props.canManage || busy} />
        </Field>
        {!projectAgents.length ? <div className="git-security-note">这个项目还没有活跃 Agent，请先在项目中添加成员。</div> : selectedAgentNeedsPermission ? <div className="git-security-note">提交后将授予所选 Agent 的 Rule 维护权限。</div> : null}
        {props.canManage ? <button className="button primary wide" disabled={!agentId || busy}>{busy ? "提交中…" : selectedAgentNeedsPermission ? "授权并交给 Agent" : "交给 Agent 生成"}</button> : null}
      </form>
    </div>
  );
}

