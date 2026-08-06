use super::super::*;
use super::catalog::company_project_type_catalog;

pub fn company_project_type_by_key(key: &str) -> Option<CompanyProjectTypeDefinition> {
    company_project_type_catalog()
        .into_iter()
        .find(|definition| definition.key == key)
}

pub fn infer_company_project_type(
    name: &str,
    description: &str,
    file_evidence: &[String],
) -> (String, i32, Vec<String>) {
    let subject = format!("{} {}", name, description).to_lowercase();
    let file_manifest = file_evidence
        .iter()
        .map(|entry| entry.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");

    // More specific business and delivery types intentionally appear before generic software.
    // Each keyword carries a specificity weight. Human descriptions are stronger evidence than
    // a generic manifest such as package.json, so an ERP or WMS does not collapse into "software".
    let candidates: &[(&str, &[(&str, usize)])] = &[
        (
            PROJECT_TYPE_WAREHOUSE_MANAGEMENT_SYSTEM,
            &[
                ("wms", 5),
                ("仓储管理", 5),
                ("仓库管理", 5),
                ("warehouse management", 5),
                ("波次", 4),
                ("拣选", 4),
                ("上架", 3),
                ("库位", 3),
                ("盘点", 3),
                ("补货", 3),
                ("pda", 2),
                ("warehouse", 1),
                ("仓储", 2),
            ],
        ),
        (
            PROJECT_TYPE_ENTERPRISE_ERP,
            &[
                ("erpnext", 5),
                ("odoo", 5),
                ("企业资源计划", 5),
                ("erp", 5),
                ("财务供应链", 4),
                ("总账", 3),
                ("应收", 2),
                ("应付", 2),
                ("采购销售库存", 3),
                ("关账", 2),
            ],
        ),
        (
            PROJECT_TYPE_CUSTOMER_RELATIONSHIP_MANAGEMENT,
            &[
                ("crm", 5),
                ("客户关系管理", 5),
                ("销售漏斗", 4),
                ("商机", 3),
                ("销售线索", 3),
                ("客户成功", 3),
                ("线索", 2),
                ("联系人", 1),
            ],
        ),
        (
            PROJECT_TYPE_MANUFACTURING_EXECUTION_SYSTEM,
            &[
                ("mes", 5),
                ("制造执行", 5),
                ("生产执行", 5),
                ("在制品", 4),
                ("工艺路线", 4),
                ("oee", 4),
                ("生产工单", 3),
                ("设备采集", 3),
                ("生产追溯", 3),
            ],
        ),
        (
            PROJECT_TYPE_ECOMMERCE_PLATFORM,
            &[
                ("ecommerce", 5),
                ("e-commerce", 5),
                ("电商平台", 5),
                ("商城", 4),
                ("购物车", 3),
                ("checkout", 3),
                ("订单履约", 3),
                ("促销", 2),
                ("payment", 2),
                ("支付", 1),
                ("售后", 1),
            ],
        ),
        (
            PROJECT_TYPE_GAME_DEVELOPMENT,
            &[
                ("project.godot", 5),
                ("godot", 5),
                ("unity", 5),
                ("unreal", 5),
                ("游戏开发", 5),
                ("game", 3),
                ("游戏", 3),
                ("关卡", 3),
                ("gameloop", 3),
                ("gameplay", 3),
            ],
        ),
        (
            PROJECT_TYPE_MOBILE_APPLICATION,
            &[
                ("pubspec.yaml", 5),
                ("react-native", 5),
                ("react native", 5),
                ("flutter", 5),
                ("移动端应用", 5),
                ("android/", 3),
                ("ios/", 3),
                ("android", 2),
                ("ios", 2),
                ("mobile app", 4),
                ("移动端", 3),
            ],
        ),
        (
            PROJECT_TYPE_DESKTOP_APPLICATION,
            &[
                ("tauri.conf", 5),
                ("electron", 5),
                ("tauri", 5),
                ("桌面应用", 5),
                ("desktop app", 4),
                ("windows 客户端", 4),
                ("macos", 2),
                ("桌面端", 3),
            ],
        ),
        (
            PROJECT_TYPE_IOT_EMBEDDED_SYSTEM,
            &[
                ("firmware", 5),
                ("embedded", 5),
                ("嵌入式", 5),
                ("esp32", 5),
                ("stm32", 5),
                ("设备固件", 5),
                ("mqtt", 3),
                ("modbus", 3),
                ("plc", 3),
                ("iot", 4),
                ("边缘设备", 3),
            ],
        ),
        (
            PROJECT_TYPE_DATA_ENGINEERING_PLATFORM,
            &[
                ("dbt_project.yml", 5),
                ("airflow", 5),
                ("lakehouse", 5),
                ("数据工程", 5),
                ("数据平台", 4),
                ("data warehouse", 4),
                ("数据仓库", 4),
                ("spark", 4),
                ("etl", 4),
                ("elt", 4),
                ("数据管道", 3),
                ("flink", 4),
                ("dagster", 4),
            ],
        ),
        (
            PROJECT_TYPE_MACHINE_LEARNING_SYSTEM,
            &[
                ("pytorch", 5),
                ("tensorflow", 5),
                ("mlflow", 5),
                ("机器学习", 5),
                ("machine learning", 5),
                ("模型训练", 4),
                ("模型推理", 4),
                ("sklearn", 4),
                ("推荐系统", 3),
                ("特征工程", 3),
            ],
        ),
        (
            PROJECT_TYPE_NOVEL_WRITING,
            &[
                ("小说", 5),
                ("novel", 5),
                ("人物设定", 4),
                ("世界观", 4),
                ("章节", 3),
                ("chapter", 3),
                ("大纲", 3),
                ("outline", 3),
                ("chapters/", 3),
            ],
        ),
        (
            PROJECT_TYPE_DATA_ANALYSIS,
            &[
                ("数据分析", 5),
                ("data analysis", 5),
                (".ipynb", 4),
                ("notebook", 3),
                ("pandas", 3),
                ("数据集", 2),
                ("dataset", 2),
                ("指标体系", 3),
                ("报表", 2),
            ],
        ),
        (
            PROJECT_TYPE_RESEARCH,
            &[
                ("研究", 4),
                ("调研", 4),
                ("research", 4),
                ("literature", 3),
                ("文献", 3),
                ("竞品分析", 3),
                ("可行性研究", 4),
                ("白皮书", 2),
            ],
        ),
        (
            PROJECT_TYPE_DESIGN_SYSTEM_BRAND,
            &[
                ("design token", 5),
                ("design system", 5),
                ("设计系统", 5),
                ("storybook", 4),
                ("组件库", 4),
                ("品牌规范", 4),
                ("视觉识别", 3),
                ("brand identity", 4),
            ],
        ),
        (
            PROJECT_TYPE_PRODUCT_DESIGN,
            &[
                ("产品设计", 5),
                ("交互设计", 5),
                ("ui design", 4),
                ("ux", 3),
                ("figma", 3),
                ("wireframe", 3),
                ("用户旅程", 3),
                ("原型", 3),
            ],
        ),
        (
            PROJECT_TYPE_MARKETING_CONTENT,
            &[
                ("营销", 4),
                ("市场活动", 4),
                ("campaign", 4),
                ("marketing", 4),
                ("社媒", 3),
                ("增长实验", 3),
                ("品牌传播", 3),
            ],
        ),
        (
            PROJECT_TYPE_DOCUMENTATION,
            &[
                ("documentation", 4),
                ("mkdocs", 5),
                ("docusaurus", 5),
                ("api reference", 4),
                ("用户手册", 4),
                ("知识库", 3),
                ("文档", 2),
                ("docs/", 2),
            ],
        ),
        (
            PROJECT_TYPE_AUTOMATION,
            &[
                ("自动化", 4),
                ("automation", 4),
                ("workflow", 3),
                ("mcp", 3),
                ("agent", 2),
                ("机器人", 3),
                ("rpa", 4),
                ("爬虫", 2),
            ],
        ),
        (
            PROJECT_TYPE_IMPLEMENTATION_MIGRATION,
            &[
                ("implementation", 4),
                ("系统实施", 5),
                ("数据迁移", 5),
                ("cutover", 5),
                ("上线切换", 5),
                ("uat", 4),
                ("试运行", 3),
                ("迁移演练", 4),
                ("上线交接", 4),
            ],
        ),
        (
            PROJECT_TYPE_OPERATIONS,
            &[
                ("运营", 4),
                ("运维", 4),
                ("operations", 4),
                ("runbook", 4),
                ("持续服务", 3),
                ("值班", 3),
                ("事件响应", 3),
            ],
        ),
        (
            PROJECT_TYPE_WEB_APPLICATION,
            &[
                ("next.config", 5),
                ("vite.config", 4),
                ("web app", 5),
                ("web 应用", 5),
                ("管理后台", 4),
                ("frontend", 3),
                ("前端", 3),
                ("website", 3),
                ("网页", 2),
            ],
        ),
        (
            PROJECT_TYPE_BACKEND_SERVICE,
            &[
                ("openapi", 5),
                ("swagger", 5),
                ("microservice", 5),
                ("微服务", 5),
                ("后端服务", 5),
                ("migrations/", 3),
                ("backend", 3),
                ("api", 2),
                ("gateway", 3),
            ],
        ),
        (
            PROJECT_TYPE_LIBRARY_SDK,
            &[
                ("sdk", 5),
                ("component library", 5),
                ("npm package", 4),
                ("开发工具", 4),
                ("公共库", 4),
                ("library", 3),
                ("crate", 3),
                ("cli", 2),
                ("插件", 2),
            ],
        ),
        (
            PROJECT_TYPE_SOFTWARE_DEVELOPMENT,
            &[
                ("软件开发", 3),
                ("软件", 2),
                ("开发", 1),
                ("cargo.toml", 1),
                ("package.json", 1),
                ("go.mod", 1),
                ("pom.xml", 1),
                ("pyproject.toml", 1),
                ("src/", 1),
            ],
        ),
        (
            PROJECT_TYPE_GENERAL_WRITING,
            &[
                ("写作", 4),
                ("文章", 3),
                ("文案", 3),
                ("演讲稿", 4),
                ("essay", 3),
                ("article", 3),
                ("script", 2),
                ("稿件", 3),
            ],
        ),
    ];
    let mut best = (PROJECT_TYPE_GENERAL, 0usize, Vec::<String>::new());
    for (project_type, keywords) in candidates {
        let mut score = 0usize;
        let mut matched = Vec::new();
        for (keyword, weight) in *keywords {
            let in_subject = subject.contains(keyword);
            let in_files = file_manifest.contains(keyword);
            if in_subject || in_files {
                score += weight * if in_subject { 3 } else { 2 };
                matched.push((*keyword).to_string());
            }
        }
        if score > best.1 {
            best = (project_type, score, matched);
        }
    }
    if best.1 == 0 {
        return (PROJECT_TYPE_GENERAL.into(), 30, Vec::new());
    }
    let confidence = (52 + best.1 * 3).min(96) as i32;
    (best.0.into(), confidence, best.2)
}
