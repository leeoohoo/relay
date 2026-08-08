import { type ReactNode, useEffect, useMemo, useState } from "react";
import { api } from "../../api/client";
import { Pagination, usePagination } from "../../components/Pagination";
import { Icon } from "../../components/ui";
import { useUiLanguage } from "../../i18n/uiLanguage";
import {
  analyzeRelaySkill,
  getRelaySkillDocuments,
  relayProfessionSkillDocument,
  RELAY_EMPLOYEE_SKILL,
  RELAY_EMPLOYEE_SKILL_EN,
  RELAY_PROFESSION_SKILLS,
  RELAY_STAFFING_MANAGER_SKILL,
  RELAY_STAFFING_MANAGER_SKILL_EN,
  type RelaySkillDocument,
  type RelaySkillLanguage,
  type RelaySkillSection,
} from "../../relaySkills";
import type { CompanyConsole, CompanyProfession, CompanyProjectType } from "../../types/platform";
import { companyAgentProfessionKey, copyText, Metric, relayAgentConnectionNames, SkillCopyBlock } from "./shared";

export function SkillsView(props: {
  consoleData: CompanyConsole | null;
  systemProjectTypes: CompanyProjectType[];
  token: string;
}) {
  const { consoleData, systemProjectTypes } = props;
  const { language: uiLanguage } = useUiLanguage();
  const skillLanguage: RelaySkillLanguage = uiLanguage;
  const [catalog, setCatalog] = useState<{
    professions: CompanyProfession[];
    project_types: CompanyProjectType[];
  } | null>(null);
  const [tab, setTab] = useState<"agent_skills" | "project_rules">("agent_skills");
  useEffect(() => {
    if (!consoleData) {
      setCatalog(null);
      return;
    }
    let active = true;
    api<{ skill_catalog: { professions: CompanyProfession[]; project_types: CompanyProjectType[] } }>(
      `/api/v1/companies/${consoleData.company.id}/skill-catalog`,
      {},
      props.token,
    )
      .then((response) => { if (active) setCatalog(response.skill_catalog); })
      .catch(() => { if (active) setCatalog(null); });
    return () => { active = false; };
  }, [consoleData?.company.id, props.token]);
  const professions = useMemo<CompanyProfession[]>(() => catalog?.professions ?? Object.entries(RELAY_PROFESSION_SKILLS).map(([key, document]) => ({
    key,
    label: document.title.replace("职业 Skill", ""),
    label_en: document.title.replace("职业 Skill", ""),
    description: `${document.title.replace("职业 Skill", "")}的岗位工作方法、交付标准和权限边界。`,
    description_en: `Professional workflow, deliverables, quality gates, and authority boundaries for ${document.title}.`,
    category_key: "general",
    category_label: "通用协作",
    category_label_en: "General Collaboration",
    skill_name: document.name,
    skill_markdown: document.content,
    skill_markdown_en: document.content,
    can_create_tasks: key === "project_manager" || key === "product_manager" || key === "technical_manager",
  })), [catalog?.professions]);
  const agents = useMemo(() => consoleData?.agents ?? [], [consoleData?.agents]);
  const [selectedAgentId, setSelectedAgentId] = useState(agents[0]?.agent_profile.id ?? "");
  const library = useMemo(() => [
    {
      category: skillLanguage === "en" ? "Shared Layer" : "通用层",
      description: skillLanguage === "en" ? "Company identity, messaging, task, memory, and silence protocol used by every Agent." : "所有 Agent 都会使用的公司身份、消息、任务和静默协作协议。",
      document: skillLanguage === "en" ? RELAY_EMPLOYEE_SKILL_EN : RELAY_EMPLOYEE_SKILL,
    },
    ...professions.map((profession) => ({
      category: skillLanguage === "en" ? profession.category_label_en : profession.category_label,
      description: skillLanguage === "en" ? profession.description_en : profession.description,
      document: relayProfessionSkillDocument(profession, skillLanguage),
    })),
    {
      category: skillLanguage === "en" ? "Authorization Layer" : "授权层",
      description: skillLanguage === "en" ? "Added only when Human grants staffing permissions; governs hiring, suspension, and termination." : "仅在 Human 授予人员管理权限时追加，约束招聘、暂停和裁撤动作。",
      document: skillLanguage === "en" ? RELAY_STAFFING_MANAGER_SKILL_EN : RELAY_STAFFING_MANAGER_SKILL,
    },
  ], [professions, skillLanguage]);
  const [selectedSkillName, setSelectedSkillName] = useState(RELAY_PROFESSION_SKILLS.project_manager.name);
  const [skillPreviewMode, setSkillPreviewMode] = useState<"guide" | "source">("guide");
  const skillCategories = useMemo(() => [...new Set(library.map((item) => item.category))], [library]);
  const [skillCategory, setSkillCategory] = useState("all");
  const visibleLibrary = useMemo(() => skillCategory === "all" ? library : library.filter((item) => item.category === skillCategory), [library, skillCategory]);
  const skillPagination = usePagination(visibleLibrary, 8, `${skillLanguage}:${skillCategory}:${visibleLibrary.length}`);
  const projectTypes = useMemo(
    () => catalog?.project_types ?? systemProjectTypes,
    [catalog?.project_types, systemProjectTypes],
  );
  const projectTypeCategories = useMemo(() => Array.from(
    new Map(projectTypes.map((type) => [type.category_key, skillLanguage === "en" ? type.category_label_en : type.category_label])).entries(),
  ).map(([key, label]) => ({ key, label })), [projectTypes, skillLanguage]);
  const [projectTypeCategory, setProjectTypeCategory] = useState("all");
  const visibleProjectTypes = useMemo(() => projectTypeCategory === "all"
    ? projectTypes
    : projectTypes.filter((type) => type.category_key === projectTypeCategory), [projectTypeCategory, projectTypes]);
  const [selectedProjectTypeKey, setSelectedProjectTypeKey] = useState(projectTypes[0]?.key ?? "");
  const [projectRulePreviewMode, setProjectRulePreviewMode] = useState<"guide" | "source">("guide");
  const projectTypePagination = usePagination(visibleProjectTypes, 6, `${projectTypeCategory}:${visibleProjectTypes.length}`);

  useEffect(() => {
    if (!agents.some((agent) => agent.agent_profile.id === selectedAgentId)) {
      setSelectedAgentId(agents[0]?.agent_profile.id ?? "");
    }
  }, [agents, selectedAgentId]);

  useEffect(() => {
    if (!visibleLibrary.some((item) => item.document.name === selectedSkillName)) {
      setSelectedSkillName(visibleLibrary[0]?.document.name ?? RELAY_EMPLOYEE_SKILL.name);
    }
  }, [visibleLibrary, selectedSkillName]);

  useEffect(() => {
    if (!visibleProjectTypes.some((type) => type.key === selectedProjectTypeKey)) {
      setSelectedProjectTypeKey(visibleProjectTypes[0]?.key ?? "");
    }
  }, [visibleProjectTypes, selectedProjectTypeKey]);

  const selectedAgent = agents.find((agent) => agent.agent_profile.id === selectedAgentId);
  const selectedProfession = selectedAgent
    ? professions.find((profession) => profession.key === companyAgentProfessionKey(selectedAgent, professions))
    : null;
  const composedDocuments = selectedAgent ? getRelaySkillDocuments(
    selectedAgent.membership.permissions,
    {
      agentId: selectedAgent.agent_profile.id,
      handle: selectedAgent.agent_profile.handle,
      mcpServerName: relayAgentConnectionNames(selectedAgent.agent_profile).mcpServer,
    },
    companyAgentProfessionKey(selectedAgent, professions),
    skillLanguage,
    selectedProfession ?? undefined,
  ) : [];
  const selectedLibraryItem = library.find((item) => item.document.name === selectedSkillName) ?? library[0];
  const selectedSkillAnalysis = useMemo(
    () => analyzeRelaySkill(selectedLibraryItem.document),
    [selectedLibraryItem.document],
  );
  const selectedProjectType = visibleProjectTypes.find((type) => type.key === selectedProjectTypeKey) ?? visibleProjectTypes[0];
  const selectedProjectRuleDocument = useMemo<RelaySkillDocument>(() => ({
    name: selectedProjectType ? `relay-project-${selectedProjectType.key}` : "relay-project-unavailable",
    title: selectedProjectType ? skillLanguage === "en" ? `${selectedProjectType.label_en} Rules` : `${selectedProjectType.label}固定规则` : skillLanguage === "en" ? "Project type Rules unavailable" : "项目类型规则暂不可用",
    content: selectedProjectType ? skillLanguage === "en" ? selectedProjectType.rule_markdown_en : selectedProjectType.rule_markdown : "",
  }), [selectedProjectType, skillLanguage]);
  const selectedProjectRuleAnalysis = useMemo(
    () => analyzeRelaySkill(selectedProjectRuleDocument),
    [selectedProjectRuleDocument],
  );

  return (
    <div className="content-stack">
      <nav className="control-center-tabs two-tabs skill-center-tabs" role="tablist" aria-label="Skill 中心">
        <button
          className={tab === "agent_skills" ? "active" : ""}
          type="button"
          role="tab"
          aria-selected={tab === "agent_skills"}
          onClick={() => setTab("agent_skills")}
        >
          <span className="control-center-tab-icon"><Icon name="book" /></span>
          <span><strong>Agent Skill</strong><small>职业 Skill、组合与授权</small></span>
        </button>
        <button
          className={tab === "project_rules" ? "active" : ""}
          type="button"
          role="tab"
          aria-selected={tab === "project_rules"}
          onClick={() => setTab("project_rules")}
        >
          <span className="control-center-tab-icon"><Icon name="git" /></span>
          <span><strong>项目类型 Rule</strong><small>固定项目基线与执行流程</small></span>
        </button>
      </nav>

      <div className="skill-center-panel" role="tabpanel">

      {tab === "agent_skills" ? <section className="metric-row">
        <Metric label="Skill 模板" value={String(library.length)} detail="通用、职业和授权模板" />
        <Metric label="系统职业" value={String(professions.length)} detail="职业决定任务权限和工作方法" />
        <Metric label="项目类型" value={String(projectTypes.length)} detail="每类项目都有不可弱化的固定规则" />
        <Metric label="Agent 组合" value={String(agents.length)} detail="每个 Agent 独立生成绑定版本" />
      </section> : null}

      {tab === "project_rules" ? <section className="section-card skill-library-card project-rule-library-card">
        <div className="section-heading">
          <div>
            <span className="eyebrow">PROJECT TYPE RULES</span>
            <h2>项目类型规则库</h2>
          </div>
          <span className="count-badge">{projectTypes.length}</span>
        </div>
        <div className="project-rule-category-filter" role="tablist" aria-label="项目规则分类">
          <button className={projectTypeCategory === "all" ? "active" : ""} type="button" onClick={() => setProjectTypeCategory("all")}>全部 <span>{projectTypes.length}</span></button>
          {projectTypeCategories.map((category) => {
            const count = projectTypes.filter((type) => type.category_key === category.key).length;
            return <button className={projectTypeCategory === category.key ? "active" : ""} type="button" key={category.key} onClick={() => setProjectTypeCategory(category.key)}>{category.label} <span>{count}</span></button>;
          })}
        </div>
        {selectedProjectType ? (
          <div className="skill-library-layout">
            <div className="skill-library-list project-rule-type-list">
              {projectTypePagination.pageItems.map((type) => {
                const typeLabel = skillLanguage === "en" ? type.label_en : type.label;
                const typeDescription = skillLanguage === "en" ? type.description_en : type.description;
                const typeCategory = skillLanguage === "en" ? type.category_label_en : type.category_label;
                const document: RelaySkillDocument = { name: `relay-project-${type.key}`, title: skillLanguage === "en" ? `${typeLabel} Rules` : `${typeLabel}固定规则`, content: skillLanguage === "en" ? type.rule_markdown_en : type.rule_markdown };
                const analysis = analyzeRelaySkill(document);
                return (
                  <button className={type.key === selectedProjectType.key ? "active" : ""} key={type.key} onClick={() => setSelectedProjectTypeKey(type.key)}>
                    <span>{typeCategory}</span>
                    <strong>{typeLabel}</strong>
                    <small>{typeDescription}</small>
                    <div className="skill-list-stats"><b>{analysis.sections.length} 个模块</b><b>{analysis.instructionCount} 条规则</b></div>
                    <code>{type.key}</code>
                  </button>
                );
              })}
              <Pagination {...projectTypePagination} onPageChange={projectTypePagination.setPage} compact />
            </div>
            <div className="skill-template-preview project-rule-preview">
              <div className="skill-template-preview-head">
                <div>
                  <span>SYSTEM PROJECT SKILL</span>
                  <strong>{selectedProjectRuleDocument.title}</strong>
                  <code>{selectedProjectRuleDocument.name}/SKILL.md</code>
                </div>
                <div className="skill-preview-actions">
                  <div className="skill-preview-toggle">
                    <button className={projectRulePreviewMode === "guide" ? "active" : ""} onClick={() => setProjectRulePreviewMode("guide")}>规则手册</button>
                    <button className={projectRulePreviewMode === "source" ? "active" : ""} onClick={() => setProjectRulePreviewMode("source")}>Rule 原文</button>
                  </div>
                  <button className="button small" onClick={() => void copyText(selectedProjectRuleDocument.content)}><Icon name="copy" /> 复制规则</button>
                </div>
              </div>
              {projectRulePreviewMode === "guide" ? (
                <div className="skill-guide-preview">
                  <section className="skill-guide-overview project-rule-overview">
                    <div>
                      <span className="eyebrow">MANDATORY PROJECT BASELINE</span>
                      <h3>{skillLanguage === "en" ? selectedProjectType.label_en : selectedProjectType.label}</h3>
                      <p>{skillLanguage === "en"
                        ? `${selectedProjectType.description_en} Rules combine the shared governance baseline, the ${selectedProjectType.category_label_en} discipline baseline, and the ${selectedProjectType.label_en} playbook. Agents and Humans may add stricter constraints only.`
                        : `${selectedProjectType.description} 规则按“共同治理基线 + ${selectedProjectType.category_label}领域基线 + ${selectedProjectType.label}专项规则”组合，Agent 和 Human 只能追加更严格的项目约束。`}</p>
                    </div>
                    <div className="skill-guide-metrics">
                      <span><small>规则模块</small><strong>{selectedProjectRuleAnalysis.sections.length}</strong></span>
                      <span><small>执行要求</small><strong>{selectedProjectRuleAnalysis.instructionCount}</strong></span>
                      <span><small>自动加载</small><strong>YES</strong></span>
                      <span><small>允许弱化</small><strong>NO</strong></span>
                    </div>
                  </section>
                  <nav className="skill-section-index" aria-label="项目类型规则目录">
                    {selectedProjectRuleAnalysis.sections.map((section, index) => <span key={`${section.title}-${index}`}><b>{String(index + 1).padStart(2, "0")}</b>{section.title}</span>)}
                  </nav>
                  <div className="skill-section-grid project-rule-section-grid">
                    {selectedProjectRuleAnalysis.sections.map((section, index) => <SkillGuideSection key={`${section.title}-${index}`} section={section} index={index} />)}
                  </div>
                </div>
              ) : <pre>{selectedProjectRuleDocument.content}</pre>}
            </div>
          </div>
        ) : (
          <div className="empty-inline"><Icon name="book" /><h3>项目类型规则尚未加载</h3><p>选择一个公司后，Relay 会从系统目录加载全部项目类型和固定 Rule。</p></div>
        )}
      </section> : null}

      {tab === "agent_skills" ? <>
      <section className="section-card">
        <div className="section-heading">
          <div>
            <span className="eyebrow">AGENT COMPOSITION</span>
            <h2>Agent 实际 Skill 组合</h2>
          </div>
          {agents.length ? (
            <select className="skill-agent-select" value={selectedAgentId} onChange={(event) => setSelectedAgentId(event.target.value)}>
              {agents.map((agent) => <option key={agent.agent_profile.id} value={agent.agent_profile.id}>{agent.agent_profile.display_name} · {agent.profession?.label ?? agent.membership.job_title}</option>)}
            </select>
          ) : null}
        </div>
        {selectedAgent ? (
          <div className="skill-agent-composition">
            <div className="skill-agent-summary">
              <span className="agent-avatar">{selectedAgent.agent_profile.display_name.slice(0, 1).toUpperCase()}</span>
              <div>
                <strong>{selectedAgent.agent_profile.display_name}</strong>
                <small>@{selectedAgent.agent_profile.handle.replace(/^@/, "")} · {selectedProfession?.label ?? selectedAgent.membership.job_title}</small>
              </div>
              <div className="skill-composition-tags">
                {composedDocuments.map((document, index) => <span key={document.name}>{index === 0 ? "通用" : index === 1 ? "职业" : "授权"} · {document.title.split(" · ")[0]}</span>)}
              </div>
            </div>
            <SkillCopyBlock documents={composedDocuments} step="完整组合" />
          </div>
        ) : (
          <div className="empty-inline"><Icon name="book" /><h3>还没有 Agent</h3><p>创建 Agent 后，这里会展示它最终使用的通用、职业和授权 Skill 组合。</p></div>
        )}
      </section>

      <section className="section-card skill-library-card">
        <div className="section-heading">
          <div>
            <span className="eyebrow">SKILL LIBRARY</span>
            <h2>Skill 模板库</h2>
          </div>
          <span className="count-badge">{library.length}</span>
        </div>
        <div className="project-rule-category-filter profession-skill-category-filter" role="tablist" aria-label="Profession Skill categories">
          <button className={skillCategory === "all" ? "active" : ""} type="button" onClick={() => setSkillCategory("all")}>{skillLanguage === "en" ? "All" : "全部"} <span>{library.length}</span></button>
          {skillCategories.map((category) => <button className={skillCategory === category ? "active" : ""} type="button" key={category} onClick={() => setSkillCategory(category)}>{category} <span>{library.filter((item) => item.category === category).length}</span></button>)}
        </div>
        <div className="skill-library-layout">
          <div className="skill-library-list">
            {skillPagination.pageItems.map((item) => {
              const analysis = analyzeRelaySkill(item.document);
              return (
                <button className={item.document.name === selectedLibraryItem.document.name ? "active" : ""} key={item.document.name} onClick={() => setSelectedSkillName(item.document.name)}>
                  <span>{item.category}</span>
                  <strong>{item.document.title}</strong>
                  <small>{item.description}</small>
                  <div className="skill-list-stats"><b>{analysis.sections.length} 个模块</b><b>{analysis.instructionCount} 条规则</b></div>
                  <code>{item.document.name}/SKILL.md</code>
                </button>
              );
            })}
            <Pagination {...skillPagination} onPageChange={skillPagination.setPage} compact />
          </div>
          <div className="skill-template-preview">
            <div className="skill-template-preview-head">
              <div>
                <span>{selectedLibraryItem.category}</span>
                <strong>{selectedLibraryItem.document.title}</strong>
                <code>{selectedLibraryItem.document.name}/SKILL.md</code>
              </div>
              <div className="skill-preview-actions">
                <div className="skill-preview-toggle">
                  <button className={skillPreviewMode === "guide" ? "active" : ""} onClick={() => setSkillPreviewMode("guide")}>岗位手册</button>
                  <button className={skillPreviewMode === "source" ? "active" : ""} onClick={() => setSkillPreviewMode("source")}>Skill 原文</button>
                </div>
                <button className="button small" onClick={() => void copyText(selectedLibraryItem.document.content)}><Icon name="copy" /> 复制原文</button>
              </div>
            </div>
            {skillPreviewMode === "guide" ? (
              <div className="skill-guide-preview">
                <section className="skill-guide-overview">
                  <div>
                    <span className="eyebrow">ROLE OPERATING SYSTEM</span>
                    <h3>{selectedLibraryItem.document.title}</h3>
                    <p>{selectedSkillAnalysis.overview || selectedLibraryItem.description}</p>
                  </div>
                  <div className="skill-guide-metrics">
                    <span><small>工作模块</small><strong>{selectedSkillAnalysis.sections.length}</strong></span>
                    <span><small>行动规则</small><strong>{selectedSkillAnalysis.instructionCount}</strong></span>
                    <span><small>质量门禁</small><strong>{selectedSkillAnalysis.qualityGateCount}</strong></span>
                    <span><small>状态模型</small><strong>{selectedSkillAnalysis.states.length || 4}</strong></span>
                  </div>
                  {selectedSkillAnalysis.tools.length ? <div className="skill-tool-strip"><small>涉及工具 / 状态</small>{selectedSkillAnalysis.tools.map((tool) => <code key={tool}>{tool}</code>)}{selectedSkillAnalysis.states.map((state) => <code key={state}>{state}</code>)}</div> : null}
                </section>
                <nav className="skill-section-index" aria-label="Skill 内容目录">
                  {selectedSkillAnalysis.sections.map((section, index) => <span key={`${section.title}-${index}`}><b>{String(index + 1).padStart(2, "0")}</b>{section.title}</span>)}
                </nav>
                <div className="skill-section-grid">
                  {selectedSkillAnalysis.sections.map((section, index) => <SkillGuideSection key={`${section.title}-${index}`} section={section} index={index} />)}
                </div>
              </div>
            ) : <pre>{selectedLibraryItem.document.content}</pre>}
          </div>
        </div>
      </section>
      </> : null}
      </div>
    </div>
  );
}

function renderSkillInline(value: string): ReactNode[] {
  return value.split(/(`[^`]+`)/g).filter(Boolean).map((part, index) => (
    part.startsWith("`") && part.endsWith("`")
      ? <code key={`${part}-${index}`}>{part.slice(1, -1)}</code>
      : <span key={`${part}-${index}`}>{part}</span>
  ));
}

export function SkillGuideSection({ section, index }: { section: RelaySkillSection; index: number }) {
  return (
    <article className={`skill-guide-section ${section.kind}`}>
      <header><span>{String(index + 1).padStart(2, "0")}</span><div><small>{section.kind.toUpperCase()}</small><h4>{section.title}</h4></div></header>
      {section.paragraphs.map((paragraph, paragraphIndex) => <p key={`${paragraph}-${paragraphIndex}`}>{renderSkillInline(paragraph)}</p>)}
      {section.orderedItems.length ? <ol>{section.orderedItems.map((item, itemIndex) => <li key={`${item}-${itemIndex}`}>{renderSkillInline(item)}</li>)}</ol> : null}
      {section.bulletItems.length ? <ul>{section.bulletItems.map((item, itemIndex) => <li key={`${item}-${itemIndex}`}>{renderSkillInline(item)}</li>)}</ul> : null}
      {section.codeBlocks.map((block, blockIndex) => <pre className="skill-guide-code" key={`${blockIndex}-${block.slice(0, 20)}`}>{block}</pre>)}
      {section.tableRows.length ? (
        <div className="skill-guide-table-wrap"><table><thead><tr>{section.tableRows[0].map((cell, cellIndex) => <th key={`${cell}-${cellIndex}`}>{renderSkillInline(cell)}</th>)}</tr></thead><tbody>{section.tableRows.slice(1).map((row, rowIndex) => <tr key={`${rowIndex}-${row.join("-")}`}>{row.map((cell, cellIndex) => <td key={`${cell}-${cellIndex}`}>{renderSkillInline(cell)}</td>)}</tr>)}</tbody></table></div>
      ) : null}
    </article>
  );
}
