use super::super::*;
use super::rules::*;

pub fn company_profession_catalog() -> Vec<CompanyProfession> {
    vec![
        profession_definition(COMPANY_PROFESSION_PROJECT_MANAGER, "项目经理", "Project Manager", "规划项目、拆分任务、安排负责人、维护依赖并推动交付。", "Govern scope, milestones, tasks, dependencies, risks, decisions, and delivery evidence.", "management", "管理与领导", "Management & Leadership", true),
        profession_definition(COMPANY_PROFESSION_PRODUCT_MANAGER, "产品经理", "Product Manager", "澄清用户与业务问题，定义产品范围、优先级、验收和效果衡量。", "Define customer problems, product scope, priorities, acceptance, and outcome measurement.", "management", "管理与领导", "Management & Leadership", true),
        profession_definition(COMPANY_PROFESSION_TECHNICAL_MANAGER, "技术经理", "Engineering Manager", "制定技术方向、拆分工程任务、协调人员并守住交付质量。", "Set technical direction, structure engineering work, coordinate ownership, and enforce delivery quality.", "management", "管理与领导", "Management & Leadership", true),
        profession_definition(COMPANY_PROFESSION_SOLUTION_ARCHITECT, "解决方案架构师", "Solution Architect", "定义系统边界、领域、接口、数据流、非功能约束和架构验证。", "Define system boundaries, domains, integrations, data flow, non-functional requirements, and architecture validation.", "architecture_quality", "架构、安全与质量", "Architecture, Security & Quality", false),
        profession_definition(COMPANY_PROFESSION_SECURITY_ENGINEER, "安全工程师", "Security Engineer", "建立威胁模型、安全控制、验证方案、事件证据和修复门禁。", "Build threat models, security controls, validation plans, incident evidence, and remediation gates.", "architecture_quality", "架构、安全与质量", "Architecture, Security & Quality", false),
        profession_definition(COMPANY_PROFESSION_QA_ENGINEER, "测试与质量工程师", "QA & Quality Engineer", "制定风险驱动测试策略、执行验证并提供发布质量证据。", "Create risk-based test strategy, execute validation, and provide release-quality evidence.", "architecture_quality", "架构、安全与质量", "Architecture, Security & Quality", false),
        profession_definition(COMPANY_PROFESSION_SOFTWARE_ENGINEER, "软件工程师", "Software Engineer", "实现、测试和交付无法进一步归入明确客户端或服务端方向的工程任务。", "Implement, test, and deliver engineering work not assigned to a more specific client or service specialty.", "engineering", "软件、平台与设备工程", "Software, Platform & Device Engineering", false),
        profession_definition(COMPANY_PROFESSION_FULLSTACK_ENGINEER, "全栈工程师", "Full-stack Engineer", "端到端实现 Web 产品的界面、服务、数据和部署闭环。", "Deliver web product slices across UI, services, data, and deployment boundaries.", "engineering", "软件、平台与设备工程", "Software, Platform & Device Engineering", false),
        profession_definition(COMPANY_PROFESSION_FRONTEND_ENGINEER, "前端工程师", "Frontend Engineer", "实现 Web 界面、组件、状态、可访问性、性能和真实接口集成。", "Implement web UI, components, state, accessibility, performance, and real API integration.", "engineering", "软件、平台与设备工程", "Software, Platform & Device Engineering", false),
        profession_definition(COMPANY_PROFESSION_BACKEND_ENGINEER, "后端工程师", "Backend Engineer", "实现服务、API、权限、数据持久化、事务和外部集成。", "Implement services, APIs, authorization, persistence, transactions, and integrations.", "engineering", "软件、平台与设备工程", "Software, Platform & Device Engineering", false),
        profession_definition(COMPANY_PROFESSION_MOBILE_ENGINEER, "移动端工程师", "Mobile Engineer", "实现 iOS、Android、Flutter、React Native、PDA 和设备端体验。", "Build iOS, Android, Flutter, React Native, PDA, and device-oriented experiences.", "engineering", "软件、平台与设备工程", "Software, Platform & Device Engineering", false),
        profession_definition(COMPANY_PROFESSION_DESKTOP_ENGINEER, "桌面端工程师", "Desktop Engineer", "实现 Windows、macOS、Linux 客户端、安装更新和本地系统集成。", "Build Windows, macOS, and Linux clients, installers, updates, and local system integrations.", "engineering", "软件、平台与设备工程", "Software, Platform & Device Engineering", false),
        profession_definition(COMPANY_PROFESSION_GAME_ENGINEER, "游戏工程师", "Game Engineer", "实现玩法系统、引擎模块、渲染、工具链、性能和平台发布。", "Implement gameplay systems, engine modules, rendering, tooling, performance, and platform releases.", "engineering", "软件、平台与设备工程", "Software, Platform & Device Engineering", false),
        profession_definition(COMPANY_PROFESSION_EMBEDDED_IOT_ENGINEER, "嵌入式与 IoT 工程师", "Embedded & IoT Engineer", "实现固件、设备协议、边缘控制、OTA、安全和硬件故障恢复。", "Build firmware, device protocols, edge control, OTA, security, and hardware failure recovery.", "engineering", "软件、平台与设备工程", "Software, Platform & Device Engineering", false),
        profession_definition(COMPANY_PROFESSION_DATABASE_ENGINEER, "数据库工程师", "Database Engineer", "负责数据模型、查询、索引、迁移、高可用、备份恢复和容量。", "Own data models, queries, indexes, migrations, availability, backup/recovery, and capacity.", "engineering", "软件、平台与设备工程", "Software, Platform & Device Engineering", false),
        profession_definition(COMPANY_PROFESSION_DEVOPS_ENGINEER, "平台 / DevOps / SRE 工程师", "Platform / DevOps / SRE Engineer", "维护构建发布、环境、基础设施、可观测性、SLO 和故障恢复。", "Own build/release, environments, infrastructure, observability, SLOs, and incident recovery.", "engineering", "软件、平台与设备工程", "Software, Platform & Device Engineering", false),
        profession_definition(COMPANY_PROFESSION_DATA_ENGINEER, "数据工程师", "Data Engineer", "实现数据契约、模型、管道、迁移、回填、血缘和质量控制。", "Build data contracts, models, pipelines, migrations, backfills, lineage, and quality controls.", "data_ai", "数据、AI 与研究", "Data, AI & Research", false),
        profession_definition(COMPANY_PROFESSION_DATA_ANALYST, "数据分析师", "Data Analyst", "定义指标口径、分析数据、量化不确定性并形成可执行结论。", "Define metrics, analyze data, quantify uncertainty, and produce decision-ready conclusions.", "data_ai", "数据、AI 与研究", "Data, AI & Research", false),
        profession_definition(COMPANY_PROFESSION_MACHINE_LEARNING_ENGINEER, "机器学习工程师", "Machine Learning Engineer", "实现数据特征、训练评估、推理服务、监控和模型生命周期。", "Build features, training/evaluation, inference, monitoring, and model lifecycle controls.", "data_ai", "数据、AI 与研究", "Data, AI & Research", false),
        profession_definition(COMPANY_PROFESSION_RESEARCH_SPECIALIST, "研究员", "Research Specialist", "设计研究问题、证据策略、来源核验、综合分析和决策建议。", "Design research questions, evidence strategy, source verification, synthesis, and recommendations.", "data_ai", "数据、AI 与研究", "Data, AI & Research", false),
        profession_definition(COMPANY_PROFESSION_PRODUCT_DESIGNER, "产品设计师", "Product Designer", "贯通用户问题、信息架构、交互、视觉、原型和开发验收。", "Connect user problems, information architecture, interaction, visual design, prototypes, and implementation acceptance.", "design_content", "设计、内容与体验", "Design, Content & Experience", false),
        profession_definition(COMPANY_PROFESSION_UI_DESIGNER, "UI / 视觉设计师", "UI / Visual Designer", "完成视觉层级、组件、Token、动效、状态和开发交付规范。", "Define visual hierarchy, components, tokens, motion, states, and implementation specifications.", "design_content", "设计、内容与体验", "Design, Content & Experience", false),
        profession_definition(COMPANY_PROFESSION_UX_DESIGNER, "UX / 交互设计师", "UX / Interaction Designer", "研究并验证用户旅程、信息架构、任务流、可用性和无障碍。", "Research and validate journeys, information architecture, task flows, usability, and accessibility.", "design_content", "设计、内容与体验", "Design, Content & Experience", false),
        profession_definition(COMPANY_PROFESSION_GAME_DESIGNER, "游戏策划 / 系统设计师", "Game Designer", "设计核心循环、战斗、成长、经济、关卡、叙事接口和可玩性验证。", "Design core loops, combat, progression, economy, levels, narrative interfaces, and playability validation.", "design_content", "设计、内容与体验", "Design, Content & Experience", false),
        profession_definition(COMPANY_PROFESSION_TECHNICAL_WRITER, "技术写作与文档工程师", "Technical Writer", "构建面向任务的产品文档、API 参考、教程、运行手册和知识治理。", "Create task-oriented product docs, API references, tutorials, runbooks, and knowledge governance.", "design_content", "设计、内容与体验", "Design, Content & Experience", false),
        profession_definition(COMPANY_PROFESSION_BUSINESS_ANALYST, "业务分析师", "Business Analyst", "调研现状、建模流程与规则，并形成可验证需求和业务验收。", "Investigate current state, model processes and rules, and produce verifiable requirements and business acceptance.", "business_delivery", "业务、实施与运营", "Business, Delivery & Operations", false),
        profession_definition(COMPANY_PROFESSION_IMPLEMENTATION_CONSULTANT, "实施顾问", "Implementation Consultant", "负责差距分析、配置、迁移、UAT、培训、切换、采用和交接。", "Own fit-gap, configuration, migration, UAT, training, cutover, adoption, and handover.", "business_delivery", "业务、实施与运营", "Business, Delivery & Operations", false),
        profession_definition(COMPANY_PROFESSION_ERP_CONSULTANT, "ERP 业务顾问", "ERP Functional Consultant", "设计财务供应链流程、主数据、单据、控制、期初和业务对账。", "Design finance and supply-chain processes, master data, documents, controls, opening balances, and reconciliation.", "business_delivery", "业务、实施与运营", "Business, Delivery & Operations", false),
        profession_definition(COMPANY_PROFESSION_WMS_CONSULTANT, "WMS 仓储顾问", "WMS Functional Consultant", "设计仓储作业、库存不变量、设备流程、集成对账和上线验收。", "Design warehouse operations, inventory invariants, device workflows, integration reconciliation, and go-live acceptance.", "business_delivery", "业务、实施与运营", "Business, Delivery & Operations", false),
        profession_definition(COMPANY_PROFESSION_DOMAIN_EXPERT, "领域专家", "Domain Expert", "验证领域术语、事实、流程、规则、例外、风险控制和验收场景。", "Validate domain language, facts, processes, rules, exceptions, controls, and acceptance scenarios.", "business_delivery", "业务、实施与运营", "Business, Delivery & Operations", false),
        profession_definition(COMPANY_PROFESSION_OPERATIONS_SPECIALIST, "运营专员", "Operations Specialist", "执行持续运营、维护业务数据、监控指标并处理异常与改进。", "Run ongoing operations, maintain business data, monitor metrics, and manage exceptions and improvements.", "business_delivery", "业务、实施与运营", "Business, Delivery & Operations", false),
        profession_definition(COMPANY_PROFESSION_GROWTH_MARKETING_SPECIALIST, "增长与市场专员", "Growth & Marketing Specialist", "设计受众、信息、渠道、活动、实验、归因和合规交付。", "Design audiences, messaging, channels, campaigns, experiments, attribution, and compliant delivery.", "business_delivery", "业务、实施与运营", "Business, Delivery & Operations", false),
        profession_definition(COMPANY_PROFESSION_GENERAL_MEMBER, "通用成员", "General Contributor", "处理明确分配的工作，并按证据同步进度、阻塞、失败和结果。", "Complete clearly assigned work and report progress, blockers, failures, and results with evidence.", "general", "通用协作", "General Collaboration", false),
    ]
}

#[allow(clippy::too_many_arguments)]
fn profession_definition(
    key: &str,
    label: &str,
    label_en: &str,
    description: &str,
    description_en: &str,
    category_key: &str,
    category_label: &str,
    category_label_en: &str,
    can_create_tasks: bool,
) -> CompanyProfession {
    CompanyProfession {
        key: key.into(),
        label: label.into(),
        label_en: label_en.into(),
        description: description.into(),
        description_en: description_en.into(),
        category_key: category_key.into(),
        category_label: category_label.into(),
        category_label_en: category_label_en.into(),
        skill_name: format!("relay-profession-{}", key.replace('_', "-")),
        skill_markdown: compose_profession_skill(
            key,
            label,
            description,
            COMPANY_SKILL_LANGUAGE_ZH_CN,
            profession_category_rules_zh(category_key),
            profession_role_source_zh(key),
        ),
        skill_markdown_en: compose_profession_skill(
            key,
            label_en,
            description_en,
            COMPANY_SKILL_LANGUAGE_EN,
            profession_category_rules_en(category_key),
            profession_role_playbook_en(key),
        ),
        can_create_tasks,
    }
}

fn compose_profession_skill(
    key: &str,
    title: &str,
    description: &str,
    language: &str,
    category_rules: &str,
    role_rules: &str,
) -> String {
    let role_body = strip_skill_wrapper(role_rules);
    if language == COMPANY_SKILL_LANGUAGE_EN {
        format!(
            "---\nname: relay-profession-{name}\ndescription: {description} Use when the authenticated Relay Agent profession is {key}.\n---\n\n# Relay {title}\n\nUse this Skill together with the Relay company employee Skill. Apply the shared professional baseline, the discipline baseline, and this role playbook as one operating system.\n\n{common}\n\n{category}\n\n{role}\n",
            name = key.replace('_', "-"),
            common = PROFESSION_COMMON_RULES_EN,
            category = category_rules,
            role = role_body.trim(),
        )
    } else {
        format!(
            "---\nname: relay-profession-{name}\ndescription: {description} 当已认证 Relay Agent 的 profession 为 {key} 时使用。\n---\n\n# Relay {title}\n\n把本 Skill 与 Relay 通用公司协作 Skill 一起使用，将通用职业基线、职业族基线和本岗位专项 Playbook 视为同一套工作系统。\n\n{common}\n\n{category}\n\n{role}\n",
            name = key.replace('_', "-"),
            common = PROFESSION_COMMON_RULES_ZH,
            category = category_rules,
            role = role_body.trim(),
        )
    }
}

fn strip_skill_wrapper(source: &str) -> String {
    let without_frontmatter = source
        .strip_prefix("---")
        .and_then(|rest| rest.find("\n---").map(|end| &rest[end + 4..]))
        .unwrap_or(source)
        .trim();
    let mut lines = without_frontmatter.lines();
    let body = if without_frontmatter.starts_with("# ") {
        lines.next();
        lines.collect::<Vec<_>>().join("\n")
    } else {
        without_frontmatter.into()
    };
    body.trim().into()
}

pub fn company_profession_by_key(key: &str) -> Option<CompanyProfession> {
    company_profession_catalog()
        .into_iter()
        .find(|profession| profession.key == key.trim())
}

pub fn infer_company_profession(job_title: Option<&str>) -> CompanyProfession {
    let normalized = job_title.unwrap_or_default().trim().to_ascii_lowercase();
    let key = if normalized.contains("项目经理")
        || normalized.contains("项目负责人")
        || normalized == "project manager"
        || normalized == "pm"
    {
        COMPANY_PROFESSION_PROJECT_MANAGER
    } else if normalized.contains("产品经理")
        || normalized.contains("产品负责人")
        || normalized.contains("product manager")
        || normalized.contains("product owner")
    {
        COMPANY_PROFESSION_PRODUCT_MANAGER
    } else if normalized.contains("技术经理")
        || normalized.contains("技术负责人")
        || normalized.contains("工程经理")
        || normalized.contains("研发经理")
        || normalized.contains("技术总监")
        || normalized == "cto"
        || normalized.contains("technical manager")
        || normalized.contains("engineering manager")
        || normalized.contains("tech lead")
    {
        COMPANY_PROFESSION_TECHNICAL_MANAGER
    } else if normalized.contains("安全工程")
        || normalized.contains("security engineer")
        || normalized.contains("application security")
        || normalized.contains("appsec")
        || normalized.contains("security analyst")
    {
        COMPANY_PROFESSION_SECURITY_ENGINEER
    } else if normalized.contains("测试")
        || normalized.contains("质量")
        || normalized.contains("qa")
    {
        COMPANY_PROFESSION_QA_ENGINEER
    } else if normalized.contains("架构师")
        || normalized.contains("solution architect")
        || normalized.contains("software architect")
        || normalized.contains("system architect")
    {
        COMPANY_PROFESSION_SOLUTION_ARCHITECT
    } else if normalized.contains("全栈")
        || normalized.contains("fullstack")
        || normalized.contains("full-stack")
        || normalized.contains("full stack")
    {
        COMPANY_PROFESSION_FULLSTACK_ENGINEER
    } else if normalized.contains("前端")
        || normalized.contains("frontend")
        || normalized.contains("front-end")
        || normalized.contains("web developer")
        || normalized.contains("web engineer")
    {
        COMPANY_PROFESSION_FRONTEND_ENGINEER
    } else if normalized.contains("后端")
        || normalized.contains("服务端")
        || normalized.contains("backend")
        || normalized.contains("back-end")
        || normalized.contains("server engineer")
    {
        COMPANY_PROFESSION_BACKEND_ENGINEER
    } else if normalized.contains("桌面端")
        || normalized.contains("桌面应用")
        || normalized.contains("desktop engineer")
        || normalized.contains("desktop developer")
        || normalized.contains("electron")
        || normalized.contains("tauri")
    {
        COMPANY_PROFESSION_DESKTOP_ENGINEER
    } else if normalized.contains("移动端")
        || normalized.contains("客户端")
        || normalized.contains("mobile")
        || normalized.contains("android")
        || normalized.contains("ios")
        || normalized.contains("pda")
    {
        COMPANY_PROFESSION_MOBILE_ENGINEER
    } else if normalized.contains("游戏工程")
        || normalized.contains("游戏开发")
        || normalized.contains("game engineer")
        || normalized.contains("game developer")
        || normalized.contains("gameplay programmer")
    {
        COMPANY_PROFESSION_GAME_ENGINEER
    } else if normalized.contains("嵌入式")
        || normalized.contains("固件")
        || normalized.contains("物联网工程")
        || normalized.contains("embedded engineer")
        || normalized.contains("firmware engineer")
        || normalized.contains("iot engineer")
    {
        COMPANY_PROFESSION_EMBEDDED_IOT_ENGINEER
    } else if normalized.contains("数据库工程")
        || normalized.contains("数据库管理员")
        || normalized == "dba"
        || normalized.contains("database engineer")
        || normalized.contains("database administrator")
    {
        COMPANY_PROFESSION_DATABASE_ENGINEER
    } else if normalized.contains("机器学习")
        || normalized.contains("算法工程")
        || normalized.contains("machine learning engineer")
        || normalized.contains("ml engineer")
        || normalized.contains("mlops")
    {
        COMPANY_PROFESSION_MACHINE_LEARNING_ENGINEER
    } else if normalized.contains("数据工程")
        || normalized.contains("数据平台")
        || normalized.contains("数据迁移")
        || normalized.contains("主数据")
        || normalized.contains("data engineer")
        || normalized.contains("etl")
    {
        COMPANY_PROFESSION_DATA_ENGINEER
    } else if normalized.contains("devops")
        || normalized.contains("sre")
        || normalized.contains("可靠性")
        || normalized.contains("运维工程")
        || normalized.contains("平台工程")
    {
        COMPANY_PROFESSION_DEVOPS_ENGINEER
    } else if normalized.contains("数据分析")
        || normalized.contains("商业分析")
        || normalized.contains("data analyst")
        || normalized.contains("analytics analyst")
    {
        COMPANY_PROFESSION_DATA_ANALYST
    } else if normalized.contains("游戏策划")
        || normalized.contains("系统策划")
        || normalized.contains("关卡策划")
        || normalized.contains("game designer")
        || normalized.contains("level designer")
    {
        COMPANY_PROFESSION_GAME_DESIGNER
    } else if normalized.contains("技术写作")
        || normalized.contains("技术文档")
        || normalized.contains("technical writer")
        || normalized.contains("documentation engineer")
    {
        COMPANY_PROFESSION_TECHNICAL_WRITER
    } else if normalized.contains("ui 设计")
        || normalized.contains("界面设计")
        || normalized.contains("视觉设计")
        || normalized.contains("ui designer")
        || normalized.contains("visual designer")
    {
        COMPANY_PROFESSION_UI_DESIGNER
    } else if normalized.contains("ux")
        || normalized.contains("用户体验")
        || normalized.contains("交互设计")
        || normalized.contains("experience designer")
        || normalized.contains("interaction designer")
    {
        COMPANY_PROFESSION_UX_DESIGNER
    } else if normalized.contains("产品设计") || normalized.contains("product designer") {
        COMPANY_PROFESSION_PRODUCT_DESIGNER
    } else if normalized.contains("erp 顾问")
        || normalized.contains("erp实施")
        || normalized.contains("erp 实施")
        || normalized.contains("erp consultant")
        || normalized.contains("erp functional")
    {
        COMPANY_PROFESSION_ERP_CONSULTANT
    } else if normalized.contains("wms 顾问")
        || normalized.contains("仓储顾问")
        || normalized.contains("wms实施")
        || normalized.contains("wms 实施")
        || normalized.contains("wms consultant")
    {
        COMPANY_PROFESSION_WMS_CONSULTANT
    } else if normalized.contains("实施顾问")
        || normalized.contains("实施工程")
        || normalized.contains("implementation consultant")
        || normalized.contains("implementation engineer")
    {
        COMPANY_PROFESSION_IMPLEMENTATION_CONSULTANT
    } else if normalized.contains("领域专家")
        || normalized.contains("业务专家")
        || normalized.contains("行业专家")
        || normalized.contains("subject matter expert")
        || normalized.contains("sme")
        || normalized.ends_with("专家")
    {
        COMPANY_PROFESSION_DOMAIN_EXPERT
    } else if normalized.contains("研究员")
        || normalized.contains("研究专员")
        || normalized.contains("researcher")
        || normalized.contains("research specialist")
    {
        COMPANY_PROFESSION_RESEARCH_SPECIALIST
    } else if normalized.contains("设计") || normalized.contains("designer") {
        COMPANY_PROFESSION_PRODUCT_DESIGNER
    } else if normalized.contains("分析")
        || normalized.contains("顾问")
        || normalized.contains("analyst")
    {
        COMPANY_PROFESSION_BUSINESS_ANALYST
    } else if normalized.contains("运营")
        || normalized.contains("销售")
        || normalized.contains("operation")
    {
        COMPANY_PROFESSION_OPERATIONS_SPECIALIST
    } else if normalized.contains("增长")
        || normalized.contains("市场营销")
        || normalized.contains("growth")
        || normalized.contains("marketing specialist")
    {
        COMPANY_PROFESSION_GROWTH_MARKETING_SPECIALIST
    } else if normalized.contains("工程")
        || normalized.contains("开发")
        || normalized.contains("程序")
        || normalized.contains("engineer")
        || normalized.contains("developer")
    {
        COMPANY_PROFESSION_SOFTWARE_ENGINEER
    } else {
        COMPANY_PROFESSION_GENERAL_MEMBER
    };
    company_profession_by_key(key).expect("built-in company profession should exist")
}
