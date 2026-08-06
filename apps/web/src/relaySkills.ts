import employeeSkill from "../../../skills/relay-company-employee/SKILL.md?raw";
import employeeSkillEn from "../../../skills/relay-company-employee/references/en.md?raw";
import staffingManagerSkill from "../../../skills/relay-company-staffing-manager/SKILL.md?raw";
import staffingManagerSkillEn from "../../../skills/relay-company-staffing-manager/references/en.md?raw";
import projectManagerSkill from "../../../skills/relay-profession-project-manager/SKILL.md?raw";
import productManagerSkill from "../../../skills/relay-profession-product-manager/SKILL.md?raw";
import technicalManagerSkill from "../../../skills/relay-profession-technical-manager/SKILL.md?raw";
import solutionArchitectSkill from "../../../skills/relay-profession-solution-architect/SKILL.md?raw";
import softwareEngineerSkill from "../../../skills/relay-profession-software-engineer/SKILL.md?raw";
import frontendEngineerSkill from "../../../skills/relay-profession-frontend-engineer/SKILL.md?raw";
import backendEngineerSkill from "../../../skills/relay-profession-backend-engineer/SKILL.md?raw";
import mobileEngineerSkill from "../../../skills/relay-profession-mobile-engineer/SKILL.md?raw";
import dataEngineerSkill from "../../../skills/relay-profession-data-engineer/SKILL.md?raw";
import devopsEngineerSkill from "../../../skills/relay-profession-devops-engineer/SKILL.md?raw";
import qaEngineerSkill from "../../../skills/relay-profession-qa-engineer/SKILL.md?raw";
import productDesignerSkill from "../../../skills/relay-profession-product-designer/SKILL.md?raw";
import uiDesignerSkill from "../../../skills/relay-profession-ui-designer/SKILL.md?raw";
import uxDesignerSkill from "../../../skills/relay-profession-ux-designer/SKILL.md?raw";
import businessAnalystSkill from "../../../skills/relay-profession-business-analyst/SKILL.md?raw";
import implementationConsultantSkill from "../../../skills/relay-profession-implementation-consultant/SKILL.md?raw";
import domainExpertSkill from "../../../skills/relay-profession-domain-expert/SKILL.md?raw";
import operationsSpecialistSkill from "../../../skills/relay-profession-operations-specialist/SKILL.md?raw";
import generalMemberSkill from "../../../skills/relay-profession-general-member/SKILL.md?raw";

export type RelaySkillDocument = {
  name: string;
  title: string;
  content: string;
};

export type RelaySkillSection = {
  title: string;
  paragraphs: string[];
  orderedItems: string[];
  bulletItems: string[];
  codeBlocks: string[];
  tableRows: string[][];
  kind: "workflow" | "quality" | "collaboration" | "boundary" | "reference";
};

export type RelaySkillAnalysis = {
  overview: string;
  sections: RelaySkillSection[];
  instructionCount: number;
  qualityGateCount: number;
  tools: string[];
  states: string[];
};

export type RelaySkillIdentity = {
  agentId: string;
  handle: string;
  mcpServerName: string;
};

export type RelaySkillLanguage = "zh-CN" | "en";

export type RelayProfessionSkillDefinition = {
  key: string;
  label: string;
  label_en: string;
  description: string;
  description_en: string;
  skill_name: string;
  skill_markdown: string;
  skill_markdown_en: string;
};

export const RELAY_EMPLOYEE_SKILL: RelaySkillDocument = {
  name: "relay-company-employee",
  title: "公司协作 Agent Skill",
  content: employeeSkill,
};

export const RELAY_STAFFING_MANAGER_SKILL: RelaySkillDocument = {
  name: "relay-company-staffing-manager",
  title: "人员管理 Agent Skill",
  content: staffingManagerSkill,
};

export const RELAY_EMPLOYEE_SKILL_EN: RelaySkillDocument = {
  name: "relay-company-employee",
  title: "Company Collaboration Agent Skill",
  content: employeeSkillEn,
};

export const RELAY_STAFFING_MANAGER_SKILL_EN: RelaySkillDocument = {
  name: "relay-company-staffing-manager",
  title: "Staffing Governance Agent Skill",
  content: staffingManagerSkillEn,
};

export const RELAY_PROFESSION_SKILLS: Record<string, RelaySkillDocument> = {
  project_manager: { name: "relay-profession-project-manager", title: "项目经理职业 Skill", content: projectManagerSkill },
  product_manager: { name: "relay-profession-product-manager", title: "产品经理职业 Skill", content: productManagerSkill },
  technical_manager: { name: "relay-profession-technical-manager", title: "技术经理职业 Skill", content: technicalManagerSkill },
  solution_architect: { name: "relay-profession-solution-architect", title: "解决方案架构师职业 Skill", content: solutionArchitectSkill },
  software_engineer: { name: "relay-profession-software-engineer", title: "软件工程师职业 Skill", content: softwareEngineerSkill },
  frontend_engineer: { name: "relay-profession-frontend-engineer", title: "前端工程师职业 Skill", content: frontendEngineerSkill },
  backend_engineer: { name: "relay-profession-backend-engineer", title: "后端工程师职业 Skill", content: backendEngineerSkill },
  mobile_engineer: { name: "relay-profession-mobile-engineer", title: "移动端工程师职业 Skill", content: mobileEngineerSkill },
  data_engineer: { name: "relay-profession-data-engineer", title: "数据工程师职业 Skill", content: dataEngineerSkill },
  devops_engineer: { name: "relay-profession-devops-engineer", title: "DevOps / SRE 工程师职业 Skill", content: devopsEngineerSkill },
  qa_engineer: { name: "relay-profession-qa-engineer", title: "测试工程师职业 Skill", content: qaEngineerSkill },
  product_designer: { name: "relay-profession-product-designer", title: "产品设计师职业 Skill", content: productDesignerSkill },
  ui_designer: { name: "relay-profession-ui-designer", title: "UI 设计师职业 Skill", content: uiDesignerSkill },
  ux_designer: { name: "relay-profession-ux-designer", title: "UX 设计师职业 Skill", content: uxDesignerSkill },
  business_analyst: { name: "relay-profession-business-analyst", title: "业务分析师职业 Skill", content: businessAnalystSkill },
  implementation_consultant: { name: "relay-profession-implementation-consultant", title: "实施顾问职业 Skill", content: implementationConsultantSkill },
  domain_expert: { name: "relay-profession-domain-expert", title: "领域专家职业 Skill", content: domainExpertSkill },
  operations_specialist: { name: "relay-profession-operations-specialist", title: "运营专员职业 Skill", content: operationsSpecialistSkill },
  general_member: { name: "relay-profession-general-member", title: "通用成员职业 Skill", content: generalMemberSkill },
};

export function relayProfessionSkillDocument(
  profession: RelayProfessionSkillDefinition,
  language: RelaySkillLanguage,
): RelaySkillDocument {
  return {
    name: profession.skill_name,
    title: language === "en" ? `${profession.label_en} Profession Skill` : `${profession.label}职业 Skill`,
    content: language === "en" ? profession.skill_markdown_en : profession.skill_markdown,
  };
}

function skillSectionKind(title: string): RelaySkillSection["kind"] {
  if (/权限|边界|禁止|停止/.test(title)) return "boundary";
  if (/协作|交接|沟通|接口/.test(title)) return "collaboration";
  if (/质量|门禁|完成|验证|状态|验收|交付/.test(title)) return "quality";
  if (/启动|开始|流程|计划|实施|设计|执行|研究|分析|实现|响应|控制|准备/.test(title)) return "workflow";
  return "reference";
}

export function analyzeRelaySkill(document: RelaySkillDocument): RelaySkillAnalysis {
  const content = document.content.replace(/^---[\s\S]*?---\s*/m, "").trim();
  const lines = content.split("\n");
  const overviewParts: string[] = [];
  const sections: RelaySkillSection[] = [];
  let current: RelaySkillSection | null = null;
  let codeLines: string[] | null = null;

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (line.startsWith("```")) {
      if (codeLines) {
        current?.codeBlocks.push(codeLines.join("\n"));
        codeLines = null;
      } else {
        codeLines = [];
      }
      continue;
    }
    if (codeLines) {
      codeLines.push(rawLine);
      continue;
    }
    if (!line || line.startsWith("# ")) continue;
    if (line.startsWith("## ")) {
      const title = line.slice(3).trim();
      current = {
        title,
        paragraphs: [],
        orderedItems: [],
        bulletItems: [],
        codeBlocks: [],
        tableRows: [],
        kind: skillSectionKind(title),
      };
      sections.push(current);
      continue;
    }
    if (!current) {
      overviewParts.push(line);
      continue;
    }
    if (line.startsWith("|") && line.endsWith("|")) {
      const cells = line.slice(1, -1).split("|").map((cell) => cell.trim());
      if (!cells.every((cell) => /^:?-{3,}:?$/.test(cell))) current.tableRows.push(cells);
      continue;
    }
    const ordered = line.match(/^\d+\.\s+(.+)$/);
    if (ordered) {
      current.orderedItems.push(ordered[1]);
      continue;
    }
    const bullet = line.match(/^[-*]\s+(.+)$/);
    if (bullet) {
      current.bulletItems.push(bullet[1]);
      continue;
    }
    current.paragraphs.push(line);
  }

  const allItems = sections.flatMap((section) => [...section.orderedItems, ...section.bulletItems]);
  const codeTokens = [...document.content.matchAll(/`([^`]+)`/g)].map((match) => match[1]);
  const tools = [...new Set(codeTokens.filter((token) => /^[a-z]+(?:\.[a-z]+)+(?:\s+[a-z]+)?$/i.test(token)))].slice(0, 8);
  const states = [...new Set(codeTokens.filter((token) => ["todo", "in_progress", "blocked", "failed", "done", "ready"].includes(token)))];
  const qualitySections = sections.filter((section) => section.kind === "quality");

  return {
    overview: overviewParts.join(" "),
    sections,
    instructionCount: allItems.length,
    qualityGateCount: qualitySections.reduce((total, section) => total + section.orderedItems.length + section.bulletItems.length, 0),
    tools,
    states,
  };
}

const PERMISSION_BLOCK_PATTERN =
  /<!-- relay-permission:([a-z0-9._-]+):start -->([\s\S]*?)<!-- relay-permission:\1:end -->/g;

function tailorSkillToPermissions(content: string, permissions: string[]) {
  return `${content
    .replace(
      PERMISSION_BLOCK_PATTERN,
      (_match: string, permission: string, block: string) =>
        permissions.includes(permission) ? block.trim() : "",
    )
    .replace(/\n{3,}/g, "\n\n")
    .trim()}\n`;
}

function skillIdentityToken(identity: RelaySkillIdentity) {
  const handle = identity.handle
    .replace(/^@/, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 36) || "agent";
  return `${handle}-${identity.agentId.replace(/-/g, "").slice(0, 8).toLowerCase()}`;
}

function bindSkillToAgent(
  document: RelaySkillDocument,
  identity: RelaySkillIdentity,
  suffix: string,
  language: RelaySkillLanguage,
) {
  const handle = identity.handle.replace(/^@/, "");
  const name = `relay-${skillIdentityToken(identity)}-${suffix}`;
  const identityGuide = (language === "en" ? [
    "## Relay Account Binding",
    "",
    `- This Skill represents only Relay Agent \`@${handle}\`.`,
    `- Always use Relay tools provided by Codex MCP Server \`${identity.mcpServerName}\`.`,
    `- After the first \`agent.bootstrap\`, verify the returned handle is \`${handle}\`; stop immediately on mismatch.`,
    "- When one Codex setup has several Relay Agents, never mix IDs, sessions, projects, or messages returned by different MCP Servers.",
  ] : [
    "## Relay 账号绑定",
    "",
    `- 本 Skill 只代表 Relay Agent \`@${handle}\`。`,
    `- 始终使用 Codex MCP Server \`${identity.mcpServerName}\` 提供的 Relay 工具。`,
    `- 首次调用 \`agent.bootstrap\` 后确认返回的 handle 为 \`${handle}\`；不一致时立即停止，避免串用其他 Agent 身份。`,
    "- 同一 Codex 配置多个 Relay Agent 时，不混用不同 MCP Server 返回的 ID、会话、项目或消息。",
  ]).join("\n");
  const content = document.content
    .replace(/^name: .+$/m, `name: ${name}`)
    .replace(/`relay-company-employee`/g, `\`${name.replace(/-staffing$/, "-employee")}\``)
    .replace(/^(# .+)$/m, `$1\n\n${identityGuide}`);
  return {
    ...document,
    name,
    title: `${document.title} · @${handle}`,
    content,
  };
}

export function getRelaySkillDocuments(
  permissions: string[],
  identity: RelaySkillIdentity,
  professionKey: string,
  language: RelaySkillLanguage = "zh-CN",
  professionDefinition?: RelayProfessionSkillDefinition,
): RelaySkillDocument[] {
  const employeeSkillDocument = language === "en" ? RELAY_EMPLOYEE_SKILL_EN : RELAY_EMPLOYEE_SKILL;
  const staffingSkillDocument = language === "en" ? RELAY_STAFFING_MANAGER_SKILL_EN : RELAY_STAFFING_MANAGER_SKILL;
  const employeeDocument = {
    ...employeeSkillDocument,
    content: tailorSkillToPermissions(employeeSkillDocument.content, permissions),
  };
  const documents = [bindSkillToAgent(employeeDocument, identity, "employee", language)];
  const professionDocument = professionDefinition
    ? relayProfessionSkillDocument(professionDefinition, language)
    : RELAY_PROFESSION_SKILLS[professionKey] ?? RELAY_PROFESSION_SKILLS.general_member;
  documents.push(bindSkillToAgent(
    professionDocument,
    identity,
    `profession-${professionKey.replace(/_/g, "-")}`,
    language,
  ));
  if (permissions.some((permission) => permission.startsWith("agent.staff."))) {
    const staffingDocument = {
      ...staffingSkillDocument,
      content: tailorSkillToPermissions(staffingSkillDocument.content, permissions),
    };
    documents.push(bindSkillToAgent(staffingDocument, identity, "staffing", language));
  }
  return documents;
}
