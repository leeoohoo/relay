use super::*;

#[test]
fn technical_manager_is_a_system_profession_with_task_assignment_permission() {
    let profession = company_profession_by_key(COMPANY_PROFESSION_TECHNICAL_MANAGER)
        .expect("technical manager profession should exist");
    assert_eq!(profession.label, "技术经理");
    assert_eq!(profession.skill_name, "relay-profession-technical-manager");
    assert!(profession.can_create_tasks);

    let permissions = default_company_agent_permissions_for_profession(
        COMPANY_AGENT_ROLE_MEMBER,
        COMPANY_PROFESSION_TECHNICAL_MANAGER,
    );
    assert!(permissions
        .iter()
        .any(|permission| permission == COMPANY_PERMISSION_TASK_ASSIGN));
    assert!(!permissions
        .iter()
        .any(|permission| permission == COMPANY_PERMISSION_PROJECT_MANAGE));
}

#[test]
fn historical_technical_lead_titles_map_to_technical_manager() {
    for title in [
        "技术经理",
        "WMS 技术负责人",
        "研发经理",
        "Engineering Manager",
        "Tech Lead",
    ] {
        assert_eq!(
            infer_company_profession(Some(title)).key,
            COMPANY_PROFESSION_TECHNICAL_MANAGER
        );
    }
}

#[test]
fn specialist_titles_map_to_specific_professions() {
    for (title, expected) in [
        ("WMS 解决方案架构师", COMPANY_PROFESSION_SOLUTION_ARCHITECT),
        (
            "WMS 前端与 PDA 工程师",
            COMPANY_PROFESSION_FRONTEND_ENGINEER,
        ),
        ("WMS 后端工程师", COMPANY_PROFESSION_BACKEND_ENGINEER),
        ("Android 客户端工程师", COMPANY_PROFESSION_MOBILE_ENGINEER),
        ("WMS 主数据与迁移工程师", COMPANY_PROFESSION_DATA_ENGINEER),
        ("WMS 集成与可靠性工程师", COMPANY_PROFESSION_DEVOPS_ENGINEER),
        ("视觉 UI 设计师", COMPANY_PROFESSION_UI_DESIGNER),
        ("用户体验 UX 设计师", COMPANY_PROFESSION_UX_DESIGNER),
        ("WMS 实施顾问", COMPANY_PROFESSION_WMS_CONSULTANT),
        ("仓储运营专家", COMPANY_PROFESSION_DOMAIN_EXPERT),
    ] {
        assert_eq!(
            infer_company_profession(Some(title)).key,
            expected,
            "{title}"
        );
    }
}

#[test]
fn specialist_professions_keep_execution_only_task_permissions() {
    for profession_key in [
        COMPANY_PROFESSION_SOLUTION_ARCHITECT,
        COMPANY_PROFESSION_FRONTEND_ENGINEER,
        COMPANY_PROFESSION_BACKEND_ENGINEER,
        COMPANY_PROFESSION_MOBILE_ENGINEER,
        COMPANY_PROFESSION_DATA_ENGINEER,
        COMPANY_PROFESSION_DEVOPS_ENGINEER,
        COMPANY_PROFESSION_UI_DESIGNER,
        COMPANY_PROFESSION_UX_DESIGNER,
        COMPANY_PROFESSION_IMPLEMENTATION_CONSULTANT,
        COMPANY_PROFESSION_DOMAIN_EXPERT,
    ] {
        let profession =
            company_profession_by_key(profession_key).expect("specialist profession should exist");
        assert!(!profession.can_create_tasks, "{profession_key}");
        let permissions = default_company_agent_permissions_for_profession(
            COMPANY_AGENT_ROLE_MEMBER,
            profession_key,
        );
        assert!(!permissions
            .iter()
            .any(|permission| permission == COMPANY_PERMISSION_TASK_ASSIGN));
    }
}

#[test]
fn profession_catalog_is_detailed_grouped_and_bilingual() {
    let catalog = company_profession_catalog();
    assert_eq!(catalog.len(), 33);
    let unique_keys = catalog
        .iter()
        .map(|profession| profession.key.as_str())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(unique_keys.len(), catalog.len());

    for profession in &catalog {
        assert!(!profession.category_key.is_empty(), "{}", profession.key);
        assert!(!profession.category_label.is_empty(), "{}", profession.key);
        assert!(
            !profession.category_label_en.is_empty(),
            "{}",
            profession.key
        );
        assert!(!profession.label_en.is_empty(), "{}", profession.key);
        assert!(!profession.description_en.is_empty(), "{}", profession.key);
        assert!(
            profession.skill_markdown.contains("通用职业工作基线"),
            "{} is missing the shared Chinese baseline",
            profession.key
        );
        assert!(
            profession
                .skill_markdown_en
                .contains("Shared Professional Operating Baseline"),
            "{} is missing the shared English baseline",
            profession.key
        );
        assert!(
            profession.skill_markdown.chars().count() > 1_500,
            "{} has an underspecified Chinese Skill",
            profession.key
        );
        assert!(
            profession.skill_markdown_en.chars().count() > 1_500,
            "{} has an underspecified English Skill",
            profession.key
        );
    }

    for key in [
        COMPANY_PROFESSION_SECURITY_ENGINEER,
        COMPANY_PROFESSION_DATABASE_ENGINEER,
        COMPANY_PROFESSION_MACHINE_LEARNING_ENGINEER,
        COMPANY_PROFESSION_GAME_DESIGNER,
        COMPANY_PROFESSION_TECHNICAL_WRITER,
        COMPANY_PROFESSION_ERP_CONSULTANT,
        COMPANY_PROFESSION_WMS_CONSULTANT,
    ] {
        assert!(
            catalog.iter().any(|profession| profession.key == key),
            "missing profession {key}"
        );
    }

    let project_manager = catalog
        .iter()
        .find(|profession| profession.key == COMPANY_PROFESSION_PROJECT_MANAGER)
        .expect("project manager profession");
    assert!(project_manager.skill_markdown.contains("强制阶段门禁编排"));
    assert!(project_manager.skill_markdown.contains("固定集成分支职责"));
    assert!(project_manager
        .skill_markdown
        .contains("问题接收、任务化与闭环"));
    assert!(project_manager
        .skill_markdown_en
        .contains("Mandatory Phase-Gate Orchestration"));
    assert!(project_manager
        .skill_markdown_en
        .contains("Stable Integration Branch Responsibility"));
    assert!(project_manager
        .skill_markdown_en
        .contains("Issue Intake and Task Closure"));

    let product_manager = catalog
        .iter()
        .find(|profession| profession.key == COMPANY_PROFESSION_PRODUCT_MANAGER)
        .expect("product manager profession");
    assert!(product_manager.skill_markdown.contains("需求阶段门禁"));
    assert!(product_manager
        .skill_markdown_en
        .contains("Requirements Phase Gate"));

    let technical_manager = catalog
        .iter()
        .find(|profession| profession.key == COMPANY_PROFESSION_TECHNICAL_MANAGER)
        .expect("technical manager profession");
    assert!(technical_manager.skill_markdown.contains("技术阶段门禁"));
    assert!(technical_manager
        .skill_markdown_en
        .contains("Engineering Phase Gates"));
    assert!(technical_manager
        .skill_markdown_en
        .contains("Technical Issue Triage and Tasking"));

    let qa_engineer = catalog
        .iter()
        .find(|profession| profession.key == COMPANY_PROFESSION_QA_ENGINEER)
        .expect("QA engineer profession");
    assert!(qa_engineer
        .skill_markdown
        .contains("可直接建任务的缺陷建议"));
    assert!(qa_engineer.skill_markdown_en.contains("task-ready defect"));
}

#[test]
fn newly_specialized_titles_infer_the_expected_profession() {
    for (title, expected) in [
        ("WMS 仓储顾问", COMPANY_PROFESSION_WMS_CONSULTANT),
        ("游戏策划", COMPANY_PROFESSION_GAME_DESIGNER),
        ("数据库管理员 DBA", COMPANY_PROFESSION_DATABASE_ENGINEER),
        (
            "机器学习工程师",
            COMPANY_PROFESSION_MACHINE_LEARNING_ENGINEER,
        ),
        ("全栈开发", COMPANY_PROFESSION_FULLSTACK_ENGINEER),
    ] {
        assert_eq!(
            infer_company_profession(Some(title)).key,
            expected,
            "{title}"
        );
    }
}

#[test]
fn project_type_catalog_contains_fixed_rules_for_core_project_kinds() {
    let catalog = company_project_type_catalog();
    assert_eq!(catalog.len(), 27);
    let unique_keys = catalog
        .iter()
        .map(|definition| definition.key.as_str())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(unique_keys.len(), catalog.len());
    for definition in &catalog {
        assert!(!definition.category_key.is_empty(), "{}", definition.key);
        assert!(!definition.category_label.is_empty(), "{}", definition.key);
        assert!(
            definition.rule_markdown.contains("项目治理与完成定义"),
            "{}",
            definition.key
        );
        assert!(
            definition
                .rule_markdown
                .contains("所有项目通用的可视化与设计资产门禁"),
            "{} is missing the universal visual-design gate",
            definition.key
        );
        assert!(
            definition.rule_markdown.chars().count() > 800,
            "project type {} has an underspecified rule",
            definition.key
        );
        assert!(
            definition
                .rule_markdown_en
                .contains("Project Governance and Definition of Done"),
            "{} is missing the English governance baseline",
            definition.key
        );
        assert!(
            definition
                .rule_markdown_en
                .contains("Universal Visual and Design Asset Gate"),
            "{} is missing the English universal visual-design gate",
            definition.key
        );
        assert!(
            definition.rule_markdown_en.chars().count() > 1_200,
            "project type {} has an underspecified English rule",
            definition.key
        );
    }
    for key in [
        PROJECT_TYPE_SOFTWARE_DEVELOPMENT,
        PROJECT_TYPE_WEB_APPLICATION,
        PROJECT_TYPE_MOBILE_APPLICATION,
        PROJECT_TYPE_GAME_DEVELOPMENT,
        PROJECT_TYPE_ENTERPRISE_ERP,
        PROJECT_TYPE_WAREHOUSE_MANAGEMENT_SYSTEM,
        PROJECT_TYPE_CUSTOMER_RELATIONSHIP_MANAGEMENT,
        PROJECT_TYPE_MANUFACTURING_EXECUTION_SYSTEM,
        PROJECT_TYPE_ECOMMERCE_PLATFORM,
        PROJECT_TYPE_DATA_ENGINEERING_PLATFORM,
        PROJECT_TYPE_NOVEL_WRITING,
    ] {
        let definition = catalog
            .iter()
            .find(|definition| definition.key == key)
            .expect("core project type should exist");
        assert!(definition.rule_markdown.chars().count() > 1_200);
    }
    assert!(
        company_project_type_by_key(PROJECT_TYPE_SOFTWARE_DEVELOPMENT)
            .expect("software project type")
            .rule_markdown
            .contains("SVG")
    );
    for key in [
        PROJECT_TYPE_SOFTWARE_DEVELOPMENT,
        PROJECT_TYPE_WEB_APPLICATION,
        PROJECT_TYPE_MOBILE_APPLICATION,
        PROJECT_TYPE_GAME_DEVELOPMENT,
    ] {
        let definition = company_project_type_by_key(key).expect("software product type");
        assert!(
            definition.rule_markdown.contains("软件项目强制阶段流程"),
            "{key} is missing the mandatory Chinese software workflow"
        );
        assert!(
            definition
                .rule_markdown_en
                .contains("Mandatory Phase-Gated Software Delivery Workflow"),
            "{key} is missing the mandatory English software workflow"
        );
    }
    let web = company_project_type_by_key(PROJECT_TYPE_WEB_APPLICATION)
        .expect("web application project type");
    assert!(web.rule_markdown.contains("Web 项目不可跳过的执行顺序"));
    let mut previous = 0;
    for phase in [
        "写需求",
        "画 SVG 设计图",
        "完成技术选型",
        "搭建工程框架",
        "开发基础模块",
        "开发核心逻辑",
        "执行系统测试",
        "完成 Docker 部署",
        "发布与验收",
    ] {
        let position = web
            .rule_markdown
            .find(phase)
            .unwrap_or_else(|| panic!("missing Web phase: {phase}"));
        assert!(position >= previous, "Web phase is out of order: {phase}");
        previous = position;
    }
    assert!(web
        .rule_markdown_en
        .contains("Web Project Non-Skippable Execution Sequence"));
    let mut previous_en = 0;
    for phase in [
        "Write requirements",
        "Create SVG designs",
        "Complete technology selection",
        "Build the engineering scaffold",
        "Build foundation modules",
        "Build core logic",
        "Run system verification",
        "Complete Docker deployment",
        "Release and accept",
    ] {
        let position = web
            .rule_markdown_en
            .find(phase)
            .unwrap_or_else(|| panic!("missing English Web phase: {phase}"));
        assert!(
            position >= previous_en,
            "English Web phase is out of order: {phase}"
        );
        previous_en = position;
    }
    assert!(company_project_type_by_key(PROJECT_TYPE_NOVEL_WRITING)
        .expect("novel project type")
        .rule_markdown
        .contains("每个章节必须使用独立文件"));
    let wms_rules = company_project_type_by_key(PROJECT_TYPE_WAREHOUSE_MANAGEMENT_SYSTEM)
        .expect("WMS project type")
        .rule_markdown;
    for required in ["负库存", "波次", "PDA", "日终对账"] {
        assert!(wms_rules.contains(required), "missing WMS rule: {required}");
    }
    let wms_rules_en = company_project_type_by_key(PROJECT_TYPE_WAREHOUSE_MANAGEMENT_SYSTEM)
        .expect("WMS project type")
        .rule_markdown_en;
    for required in ["negative inventory", "PDA", "end-of-day reconciliation"] {
        assert!(
            wms_rules_en.contains(required),
            "missing English WMS rule: {required}"
        );
    }
    let erp_rules = company_project_type_by_key(PROJECT_TYPE_ENTERPRISE_ERP)
        .expect("ERP project type")
        .rule_markdown;
    for required in ["借贷平衡", "关账", "期初"] {
        assert!(erp_rules.contains(required), "missing ERP rule: {required}");
    }
}

#[test]
fn project_type_inference_prefers_folder_manifest_evidence() {
    let (project_type, confidence, evidence) = infer_company_project_type(
        "Northstar",
        "构建一个新的产品",
        &[
            "Cargo.toml".into(),
            "src/main.rs".into(),
            "package.json".into(),
        ],
    );
    assert_eq!(project_type, PROJECT_TYPE_SOFTWARE_DEVELOPMENT);
    assert!(confidence >= 70);
    assert!(!evidence.is_empty());

    let (project_type, _, _) = infer_company_project_type(
        "雾港纪事",
        "长篇小说，需要人物设定和章节大纲",
        &["chapters/01.md".into()],
    );
    assert_eq!(project_type, PROJECT_TYPE_NOVEL_WRITING);

    for (name, description, files, expected) in [
        (
            "智能仓",
            "建设仓储 WMS，覆盖库位、波次和 PDA 拣选",
            vec!["package.json".into()],
            PROJECT_TYPE_WAREHOUSE_MANAGEMENT_SYSTEM,
        ),
        (
            "企业经营平台",
            "基于 ERPNext 打通财务、库存和关账",
            vec!["package.json".into()],
            PROJECT_TYPE_ENTERPRISE_ERP,
        ),
        (
            "数字工厂",
            "MES 生产工单、工艺路线、在制品和 OEE",
            vec!["Cargo.toml".into()],
            PROJECT_TYPE_MANUFACTURING_EXECUTION_SYSTEM,
        ),
        (
            "Field App",
            "现场移动应用",
            vec!["pubspec.yaml".into()],
            PROJECT_TYPE_MOBILE_APPLICATION,
        ),
        (
            "Analytics Core",
            "建设可复现的数据工程平台",
            vec!["dbt_project.yml".into()],
            PROJECT_TYPE_DATA_ENGINEERING_PLATFORM,
        ),
    ] {
        let (project_type, confidence, evidence) =
            infer_company_project_type(name, description, &files);
        assert_eq!(project_type, expected, "{name}: {evidence:?}");
        assert!(confidence >= 80, "{name}: {confidence}");
    }
}
