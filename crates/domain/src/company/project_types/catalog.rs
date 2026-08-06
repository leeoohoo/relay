use super::super::*;
use super::rules::*;

pub fn company_project_type_catalog() -> Vec<CompanyProjectTypeDefinition> {
    vec![
        project_type_definition(
            PROJECT_TYPE_SOFTWARE_DEVELOPMENT,
            "通用软件开发",
            "无法进一步归入明确形态的软件产品、服务或工程项目。",
            "software_product",
            "软件与数字产品",
            SOFTWARE_PRODUCT_RULES,
            SOFTWARE_DEVELOPMENT_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_WEB_APPLICATION,
            "Web 应用与网站",
            "面向浏览器的业务应用、门户、管理后台、官网和全栈 Web 产品。",
            "software_product",
            "软件与数字产品",
            SOFTWARE_PRODUCT_RULES,
            WEB_APPLICATION_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_MOBILE_APPLICATION,
            "移动端应用",
            "iOS、Android、Flutter、React Native、PDA 和移动设备应用。",
            "software_product",
            "软件与数字产品",
            SOFTWARE_PRODUCT_RULES,
            MOBILE_APPLICATION_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_DESKTOP_APPLICATION,
            "桌面应用",
            "Windows、macOS、Linux 桌面软件、客户端和跨平台桌面产品。",
            "software_product",
            "软件与数字产品",
            SOFTWARE_PRODUCT_RULES,
            DESKTOP_APPLICATION_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_BACKEND_SERVICE,
            "后端服务与 API",
            "微服务、单体后端、开放 API、网关和集成服务。",
            "software_product",
            "软件与数字产品",
            SOFTWARE_PRODUCT_RULES,
            BACKEND_SERVICE_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_LIBRARY_SDK,
            "库、SDK 与开发工具",
            "公共库、SDK、CLI、编译工具、插件和开发者基础设施。",
            "software_product",
            "软件与数字产品",
            SOFTWARE_PRODUCT_RULES,
            LIBRARY_SDK_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_GAME_DEVELOPMENT,
            "游戏开发",
            "2D/3D 游戏、互动体验、关卡、游戏服务和内容工具链。",
            "software_product",
            "软件与数字产品",
            SOFTWARE_PRODUCT_RULES,
            GAME_DEVELOPMENT_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_IOT_EMBEDDED_SYSTEM,
            "IoT 与嵌入式系统",
            "设备固件、边缘计算、工业协议、硬件控制和设备云。",
            "software_product",
            "软件与数字产品",
            SOFTWARE_PRODUCT_RULES,
            IOT_EMBEDDED_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_ENTERPRISE_ERP,
            "ERP 企业资源计划",
            "覆盖财务、采购、销售、库存、制造、人力或项目核算的企业系统。",
            "enterprise_system",
            "企业业务系统",
            ENTERPRISE_SYSTEM_RULES,
            ERP_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_WAREHOUSE_MANAGEMENT_SYSTEM,
            "WMS 仓储管理系统",
            "覆盖入库、上架、库存、补货、波次、拣选、复核、出库和盘点的仓储系统。",
            "enterprise_system",
            "企业业务系统",
            ENTERPRISE_SYSTEM_RULES,
            WMS_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_CUSTOMER_RELATIONSHIP_MANAGEMENT,
            "CRM 客户关系管理",
            "覆盖线索、客户、联系人、商机、报价、活动和客户成功的业务系统。",
            "enterprise_system",
            "企业业务系统",
            ENTERPRISE_SYSTEM_RULES,
            CRM_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_MANUFACTURING_EXECUTION_SYSTEM,
            "MES 制造执行系统",
            "覆盖工单、BOM、工艺、在制品、质量、追溯、设备和生产绩效的制造系统。",
            "enterprise_system",
            "企业业务系统",
            ENTERPRISE_SYSTEM_RULES,
            MES_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_ECOMMERCE_PLATFORM,
            "电商与交易平台",
            "商城、交易中台、订单、支付、促销、履约、售后和多渠道零售系统。",
            "enterprise_system",
            "企业业务系统",
            ENTERPRISE_SYSTEM_RULES,
            ECOMMERCE_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_DATA_ANALYSIS,
            "数据分析",
            "数据清洗、探索分析、指标体系、报表、可视化和决策结论。",
            "data_research",
            "数据、AI 与研究",
            DATA_RESEARCH_RULES,
            DATA_ANALYSIS_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_DATA_ENGINEERING_PLATFORM,
            "数据工程与平台",
            "数据仓库、湖仓、ETL/ELT、流处理、调度、数据质量和数据服务。",
            "data_research",
            "数据、AI 与研究",
            DATA_RESEARCH_RULES,
            DATA_ENGINEERING_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_MACHINE_LEARNING_SYSTEM,
            "机器学习与模型系统",
            "训练、评估、推理、推荐、搜索、预测和 MLOps 系统。",
            "data_research",
            "数据、AI 与研究",
            DATA_RESEARCH_RULES,
            MACHINE_LEARNING_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_RESEARCH,
            "研究调研",
            "资料调研、竞品分析、学术研究、行业研究和可行性论证。",
            "data_research",
            "数据、AI 与研究",
            DATA_RESEARCH_RULES,
            RESEARCH_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_PRODUCT_DESIGN,
            "产品与体验设计",
            "产品方案、用户旅程、信息架构、交互、视觉和服务设计。",
            "design_content",
            "设计、内容与知识",
            DESIGN_CONTENT_RULES,
            PRODUCT_DESIGN_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_DESIGN_SYSTEM_BRAND,
            "设计系统与品牌",
            "设计 Token、组件库、品牌识别、视觉规范和多端一致性建设。",
            "design_content",
            "设计、内容与知识",
            DESIGN_CONTENT_RULES,
            DESIGN_SYSTEM_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_NOVEL_WRITING,
            "小说创作",
            "长篇、短篇、连载小说和其他叙事作品。",
            "design_content",
            "设计、内容与知识",
            DESIGN_CONTENT_RULES,
            NOVEL_WRITING_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_GENERAL_WRITING,
            "专业写作",
            "文章、报告、白皮书、演讲稿、脚本和非虚构内容。",
            "design_content",
            "设计、内容与知识",
            DESIGN_CONTENT_RULES,
            GENERAL_WRITING_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_DOCUMENTATION,
            "文档与知识库",
            "产品文档、API 文档、操作手册、教程、规范和知识库。",
            "design_content",
            "设计、内容与知识",
            DESIGN_CONTENT_RULES,
            DOCUMENTATION_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_MARKETING_CONTENT,
            "市场、品牌与增长内容",
            "营销活动、品牌传播、社媒、增长实验和销售内容。",
            "design_content",
            "设计、内容与知识",
            DESIGN_CONTENT_RULES,
            MARKETING_CONTENT_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_AUTOMATION,
            "自动化、Agent 与集成",
            "工作流、脚本、Agent、MCP、机器人和重复任务自动化。",
            "operations_delivery",
            "自动化、运营与交付",
            OPERATIONS_DELIVERY_RULES,
            AUTOMATION_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_OPERATIONS,
            "运营与持续服务",
            "产品运营、平台运营、内容运营、客户运营和持续服务。",
            "operations_delivery",
            "自动化、运营与交付",
            OPERATIONS_DELIVERY_RULES,
            OPERATIONS_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_IMPLEMENTATION_MIGRATION,
            "实施、迁移与上线",
            "企业软件实施、数据迁移、系统切换、培训、UAT 和上线交接。",
            "operations_delivery",
            "自动化、运营与交付",
            OPERATIONS_DELIVERY_RULES,
            IMPLEMENTATION_MIGRATION_RULES,
        ),
        project_type_definition(
            PROJECT_TYPE_GENERAL,
            "通用项目",
            "尚不属于其他明确类别的协作项目。",
            "general",
            "通用项目",
            "",
            GENERAL_PROJECT_RULES,
        ),
    ]
}

fn project_type_definition(
    key: &str,
    label: &str,
    description: &str,
    category_key: &str,
    category_label: &str,
    category_rules: &str,
    type_rules: &str,
) -> CompanyProjectTypeDefinition {
    let rule_markdown = [PROJECT_GOVERNANCE_RULES, category_rules, type_rules]
        .into_iter()
        .filter(|section| !section.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    let (label_en, description_en, type_rules_en) = project_type_english_metadata(key);
    let category_label_en = project_type_category_label_en(category_key);
    let rule_markdown_en = [
        PROJECT_GOVERNANCE_RULES_EN,
        project_type_category_rules_en(category_key),
        type_rules_en,
    ]
    .into_iter()
    .filter(|section| !section.trim().is_empty())
    .collect::<Vec<_>>()
    .join("\n\n");
    CompanyProjectTypeDefinition {
        key: key.into(),
        label: label.into(),
        label_en: label_en.into(),
        description: description.into(),
        description_en: description_en.into(),
        category_key: category_key.into(),
        category_label: category_label.into(),
        category_label_en: category_label_en.into(),
        rule_markdown,
        rule_markdown_en,
    }
}

fn project_type_category_label_en(category_key: &str) -> &'static str {
    match category_key {
        "software_product" => "Software & Digital Products",
        "enterprise_system" => "Enterprise Business Systems",
        "data_research" => "Data, AI & Research",
        "design_content" => "Design, Content & Knowledge",
        "operations_delivery" => "Automation, Operations & Delivery",
        _ => "General Projects",
    }
}

fn project_type_category_rules_en(category_key: &str) -> &'static str {
    match category_key {
        "software_product" => SOFTWARE_PRODUCT_RULES_EN,
        "enterprise_system" => ENTERPRISE_SYSTEM_RULES_EN,
        "data_research" => DATA_RESEARCH_RULES_EN,
        "design_content" => DESIGN_CONTENT_RULES_EN,
        "operations_delivery" => OPERATIONS_DELIVERY_RULES_EN,
        _ => "",
    }
}

fn project_type_english_metadata(key: &str) -> (&'static str, &'static str, &'static str) {
    match key {
        PROJECT_TYPE_SOFTWARE_DEVELOPMENT => (
            "General Software Development",
            "Software products or engineering work that cannot yet be classified into a more specific delivery shape.",
            r#"## General Software Development Playbook

1. Convert the request into user scenarios, scope, non-goals, acceptance criteria, dependencies, and unresolved decisions before changing code.
2. Record architecture boundaries, data flow, interfaces, state, failure semantics, migration, and rollback. Produce reviewable SVG or equivalent high-fidelity design assets before implementing visible UI.
3. Reuse established components and repository conventions; keep the change set focused and never hide errors, credentials, or privacy-sensitive data.
4. Prove behavior with normal, boundary, failure, recovery, and regression tests. Run the relevant formatter, static checks, tests, and clean build before reporting completion.
5. Deliver code, documentation, operational guidance, verification evidence, residual risks, and an explicit next step. Do not mark incomplete acceptance criteria as done."#,
        ),
        PROJECT_TYPE_WEB_APPLICATION => (
            "Web Applications & Websites",
            "Browser-based business applications, portals, admin consoles, public sites, and full-stack web products.",
            r#"## Web Project Non-Skippable Execution Sequence

Every Web project must follow this order and represent it with task prerequisites. Each step leaves accessible project assets and acceptance evidence. When an earlier deliverable is missing, the Agent must stop and report the missing gate to the PM or Engineering Manager rather than adding a sentence and continuing to code.

1. **Write requirements.** Produce a product/requirements brief covering target users, core problem, page and route inventory, roles and permissions, primary and failure flows, business rules, data, SEO/analytics needs, browser/device scope, performance and accessibility targets, acceptance criteria, and non-goals.
2. **Create SVG designs.** Store editable SVG assets in `docs/design/` or the agreed asset location. Cover major pages and components at desktop and tablet/mobile layouts plus empty, loading, error, unauthorized, long-content, confirmation/undo, and success states. Do not enter technical implementation before Human, product, or design ownership accepts the direction.
3. **Complete technology selection.** Record CSR/SSR/SSG choice, frontend/backend frameworks and versions, database, authentication, state/cache, API contracts, file/upload behavior, localization, test stack, deployment shape, browser matrix, security boundaries, and material ADRs. Familiarity alone is not a sufficient selection rationale.
4. **Build the engineering scaffold.** Establish frontend/backend or full-stack structure, locked dependencies, configuration, environment examples, route skeleton, build, lint/format, type checking, test harness, logging, error pages, health checks, and CI foundation. Prove clean install, startup, and minimum tests.
5. **Build foundation modules.** Complete design tokens, global layout, navigation, responsive shell, shared components, forms and validation, authentication session, authorization guards, API client, unified errors, loading/empty states, pagination or virtualization, localization, and observability foundations before scaling core screens.
6. **Build core logic.** Deliver vertical slices along real user journeys, connecting page, API, authorization, data, and feedback. Complete normal, boundary, failure, duplicate submission, expired-session, and recovery behavior for each slice before expanding the next journey.
7. **Run system verification.** Validate requirements and SVG fidelity with real-interface end-to-end journeys. Cover supported viewports and browsers, keyboard/semantics/contrast, slow networks, refresh and back/forward behavior, cache invalidation, permission changes, XSS/CSRF, performance budgets, and regression.
8. **Complete Docker deployment.** After the test gate passes, finish production images and Compose with multi-stage builds, non-root users, precise directory copies, `.dockerignore`, health checks, environment and secret injection, migrations, persistence, reverse proxy, security headers, logs/monitoring, and rollback. Execute a no-cache clean build, actual startup, and smoke verification.
9. **Release and accept.** Provide access endpoint, version/commit, design comparison, test report, deployment and recovery steps, monitoring/alerts, known limitations, project assets, and residual risk. Only the responsible owners may sign and move the work to `done`.

## Web-specific Quality Gates

1. Server/client state must not create unexplained hydration divergence. URL, form, upload, cache, optimistic update, pagination, and API error behavior must be deterministic and testable.
2. Define budgets for Core Web Vitals, initial rendering, bundles, images, fonts, and requests. Avoid unbounded full-data loading; paginate or virtualize lists, search, and history by default.
3. Protect against XSS, CSRF, open redirects, clickjacking, sensitive caching, and client secret exposure. Session expiry, cross-tab behavior, refresh recovery, and permission changes require explicit behavior.
4. SVG, requirements, architecture, code, tests, and deployment evidence must refer to the same candidate version. Old screenshots, old containers, or tests from another branch do not prove the current candidate."#,
        ),
        PROJECT_TYPE_MOBILE_APPLICATION => (
            "Mobile Applications",
            "iOS, Android, Flutter, React Native, PDA, and other device-oriented applications.",
            r#"## Mobile Application Playbook

1. Define supported OS/device versions, navigation, lifecycle, permissions, deep links, notification behavior, offline capability, and data synchronization rules.
2. Account for safe areas, keyboard, orientation, accessibility scaling, localization, interrupted flows, process death, background execution, and limited storage or connectivity.
3. Protect credentials and personal data with platform security facilities; define certificate, API compatibility, migration, and remote-config rollback strategies.
4. Test on representative physical devices as well as simulators, including denied permissions, flaky networks, low memory, upgrades, reinstalls, duplicate scans, and background restoration.
5. Deliver signed-build instructions, store or enterprise distribution metadata, crash and performance monitoring, privacy disclosures, staged rollout, and rollback evidence."#,
        ),
        PROJECT_TYPE_DESKTOP_APPLICATION => (
            "Desktop Applications",
            "Windows, macOS, and Linux desktop clients and cross-platform desktop products.",
            r#"## Desktop Application Playbook

1. Define operating-system support, packaging, installation, auto-update, file/protocol associations, permissions, local storage, and multi-window behavior.
2. Preserve native keyboard, focus, menu, drag/drop, clipboard, accessibility, scaling, and window-state expectations across supported platforms.
3. Treat filesystem access, shell execution, embedded web content, plugins, and local IPC as security boundaries with explicit validation and least privilege.
4. Test clean install, upgrade, downgrade protection, damaged configuration, offline use, large files, crash recovery, multiple displays, and platform-specific signing.
5. Deliver reproducible packages, signatures, update channels, diagnostics, data backup/export, uninstall behavior, and a supported rollback path."#,
        ),
        PROJECT_TYPE_BACKEND_SERVICE => (
            "Backend Services & APIs",
            "Monoliths, microservices, public APIs, gateways, and integration services.",
            r#"## Backend Service Playbook

1. Specify ownership, trust boundaries, API and event contracts, authentication, authorization, tenancy, quotas, consistency, and compatibility windows.
2. Define transaction, idempotency, concurrency, timeout, retry, circuit-breaking, compensation, and partial-failure semantics for every external dependency.
3. Database changes use expand-migrate-contract or an equally safe sequence with locking, capacity, backfill, verification, rollback, and old-consumer protection.
4. Test contract, authorization, duplicate, race, overload, dependency outage, malformed data, migration, and recovery paths; security errors must not leak internal details.
5. Deliver structured logs, metrics, traces, SLOs, alerts, runbooks, capacity evidence, deployment ordering, and rollback or compensation procedures."#,
        ),
        PROJECT_TYPE_LIBRARY_SDK => (
            "Libraries, SDKs & Developer Tools",
            "Reusable libraries, SDKs, CLIs, compilers, plugins, and developer infrastructure.",
            r#"## Library, SDK & Tooling Playbook

1. Define the public API, supported runtimes, compatibility policy, versioning, deprecation window, error model, extension points, and non-goals before implementation.
2. Optimize for predictable defaults, composability, typed contracts, actionable diagnostics, deterministic output, and safe behavior in automated environments.
3. Maintain examples, reference documentation, migration guides, changelog entries, and compatibility fixtures as part of the product surface.
4. Test public API behavior, backward compatibility, packaging, installation, multiple runtime versions, malformed inputs, interruption, and reproducible builds.
5. Release signed or verifiable artifacts with provenance, licenses, dependency audit, upgrade and rollback guidance, and a documented support matrix."#,
        ),
        PROJECT_TYPE_GAME_DEVELOPMENT => (
            "Game Development",
            "2D/3D games, interactive experiences, levels, game services, and content pipelines.",
            r#"## Game Development Playbook

1. Define the player fantasy, core loop, input, feedback, progression, failure/win conditions, economy, session shape, and target hardware before expanding content.
2. Keep simulation, rendering, UI, content data, save state, and platform services separable enough to test and profile independently.
3. Validate fun and clarity through playable builds, not design prose alone. Track level flow, difficulty, onboarding, accessibility, controller support, and content consistency.
4. Every defect includes a minimal reproduction, engine and platform version, save or scene fixture, expected result, actual result, and regression verification.
5. Enforce frame-time, memory, loading, network, asset, save-compatibility, crash, telemetry, build, distribution, and rollback budgets for the target platforms."#,
        ),
        PROJECT_TYPE_IOT_EMBEDDED_SYSTEM => (
            "IoT & Embedded Systems",
            "Firmware, edge computing, industrial protocols, hardware control, and device-cloud systems.",
            r#"## IoT & Embedded Systems Playbook

1. Document hardware revisions, timing, power, memory, storage, sensor accuracy, actuator safety, protocols, provisioning, and physical failure assumptions.
2. Separate safety-critical control from cloud availability. Define watchdog, safe state, brownout, reconnect, clock drift, duplicate command, and degraded-mode behavior.
3. Secure identity, boot, firmware signing, key storage, transport, authorization, debug interfaces, and update channels; plan fleet-wide rollback and revoked-device handling.
4. Test with real hardware, simulated faults, noisy inputs, network loss, power interruption, partial upgrade, protocol fuzzing, and long-running soak scenarios.
5. Deliver manufacturing/provisioning instructions, firmware artifacts, compatibility matrix, calibration, telemetry, field diagnostics, staged OTA, and recovery procedures."#,
        ),
        PROJECT_TYPE_ENTERPRISE_ERP => (
            "ERP Enterprise Resource Planning",
            "Enterprise systems spanning finance, procurement, sales, inventory, manufacturing, HR, or project accounting.",
            r#"## ERP Playbook

1. Establish legal entities, organizations, fiscal calendars, charts of accounts, currencies, taxes, units, master data, document states, numbering, and approval authority.
2. Every posting defines debit/credit balance, subledger-to-ledger impact, period controls, source traceability, reversal, correction, and audit evidence.
3. Model procure-to-pay, order-to-cash, inventory, manufacturing, expense, asset, and project flows with explicit ownership and cross-module reconciliation.
4. Test close/reopen, foreign currency, tax rounding, partial fulfillment, returns, credit, duplicate integration, retroactive change, and segregation-of-duty violations.
5. Before go-live reconcile opening balances, open items, inventory, work in progress, fixed assets, tax, and historical documents; rehearse cutover and rollback with business owners."#,
        ),
        PROJECT_TYPE_WAREHOUSE_MANAGEMENT_SYSTEM => (
            "WMS Warehouse Management System",
            "Warehouse systems covering receiving, putaway, inventory, replenishment, waves, picking, checking, shipping, and counting.",
            r#"## WMS Playbook

1. Model ASN, receiving, inspection, putaway, replenishment, wave, allocation, picking, checking, packing, shipping, transfer, return, and cycle-count state machines.
2. Define invariants for on-hand, available, allocated, frozen, quality, damaged, in-transit, lot, serial, expiry, owner, location, container, and unit conversion.
3. PDA, barcode, printer, scale, conveyor, PLC/MFC, and automation flows must handle offline work, duplicate scans, wrong sequence, damaged labels, retries, and operator correction.
4. ERP/OMS/TMS and carrier integration requires business idempotency, acknowledgements, replay, dead letter handling, compensation, monitoring, and end-of-day reconciliation.
5. Test concurrent allocation, negative inventory prevention, over-allocation, short pick, expiry/FEFO, inventory adjustment, device outage, message disorder, peak waves, and recovery drills."#,
        ),
        PROJECT_TYPE_CUSTOMER_RELATIONSHIP_MANAGEMENT => (
            "CRM Customer Relationship Management",
            "Systems for leads, accounts, contacts, opportunities, quotes, activities, and customer success.",
            r#"## CRM Playbook

1. Define lifecycle and ownership for leads, accounts, contacts, opportunities, activities, quotes, consent, and customer-success records.
2. Establish duplicate matching, merge, assignment, territory, pipeline stage, forecast, scoring, SLA, and handoff rules with auditable overrides.
3. Protect personal and commercial data through purpose, consent, retention, field/data-scope permissions, export controls, and access auditing.
4. Integrations with marketing, email, telephony, support, ERP, and analytics require identity matching, idempotency, attribution, replay, and reconciliation.
5. Test reassignment, merge conflicts, stage reversal, partial sync, opt-out, deleted users, multi-region ownership, forecast changes, and bulk import recovery."#,
        ),
        PROJECT_TYPE_MANUFACTURING_EXECUTION_SYSTEM => (
            "MES Manufacturing Execution System",
            "Manufacturing systems for work orders, BOM, routing, WIP, quality, traceability, equipment, and production performance.",
            r#"## MES Playbook

1. Model plant, line, work center, equipment, shift, material, BOM, routing, recipe, work order, operation, WIP, quality, genealogy, and downtime semantics.
2. Preserve material and process traceability from issue through consumption, production, rework, scrap, inspection, release, and finished-goods receipt.
3. PLC/SCADA/device integration defines time synchronization, tag quality, buffering, duplicate events, command authority, safe states, and manual fallback.
4. Test routing deviation, substitute material, split/merge lots, rework, equipment outage, late events, quality hold, recall tracing, shift boundary, and OEE calculation.
5. Go-live requires master-data validation, shop-floor role UAT, label/device tests, capacity and offline drills, ERP reconciliation, cutover, and operator handover."#,
        ),
        PROJECT_TYPE_ECOMMERCE_PLATFORM => (
            "E-commerce & Transaction Platforms",
            "Storefronts and commerce platforms for catalog, cart, checkout, payment, promotion, fulfillment, and after-sales service.",
            r#"## Commerce Platform Playbook

1. Define catalog, price, promotion, inventory, cart, checkout, order, payment, tax, fulfillment, return, refund, dispute, and customer identity state machines.
2. Monetary calculations use explicit currency, rounding, tax, discount allocation, settlement, refund, and reconciliation rules; external callbacks are authenticated and idempotent.
3. Protect inventory and payment correctness under duplicate submission, concurrent checkout, delayed webhooks, partial capture, split shipment, cancellation, and retry.
4. Test guest/member, multi-address, coupons, stock race, payment failure, fraud review, return/refund, marketplace split, peak traffic, and degraded dependency paths.
5. Deliver conversion and reliability observability, financial reconciliation, privacy/PCI scope, operational consoles, release rollback, and customer-support recovery procedures."#,
        ),
        PROJECT_TYPE_DATA_ANALYSIS => (
            "Data Analysis",
            "Data cleaning, exploratory analysis, metrics, reports, visualization, and decision conclusions.",
            r#"## Data Analysis Playbook

1. Define the decision question, population, metric formulas, dimensions, time window, exclusions, comparison baseline, and expected action before querying data.
2. Preserve raw inputs, record extraction time and filters, validate joins and grain, quantify missingness and outliers, and distinguish data defects from business behavior.
3. Use appropriate statistical uncertainty, sensitivity checks, segmentation, and counter-explanations; correlation, significance, and dashboard movement are not causal proof.
4. Make every table and chart reproducible with source, transformation, denominator, unit, timezone, and caveat; avoid misleading axes, aggregation, or selective ranges.
5. Deliver the answer, evidence, limitations, confidence, decision impact, reproducible artifacts, and monitoring or follow-up questions."#,
        ),
        PROJECT_TYPE_DATA_ENGINEERING_PLATFORM => (
            "Data Engineering & Platforms",
            "Warehouses, lakehouses, ETL/ELT, streaming, orchestration, data quality, and data services.",
            r#"## Data Engineering Platform Playbook

1. Define source ownership, contracts, grain, keys, event time, schema evolution, lineage, privacy class, retention, freshness, and downstream consumers.
2. Pipelines must be deterministic, idempotent, restartable, backfillable, observable, and safe under duplicates, late data, missing partitions, partial writes, and source replay.
3. Separate raw, standardized, modeled, serving, and reporting layers; document incremental logic, slowly changing dimensions, partitioning, compaction, and cost controls.
4. Test contracts, quality assertions, historical replay, schema change, timezone, volume spikes, task retry, partial outage, reconciliation, and disaster recovery.
5. Deliver catalog, lineage, ownership, SLAs, alerts, runbooks, backfill controls, access policy, cost/performance evidence, and deprecation plans."#,
        ),
        PROJECT_TYPE_MACHINE_LEARNING_SYSTEM => (
            "Machine Learning & Model Systems",
            "Training, evaluation, inference, recommendation, search, forecasting, and MLOps systems.",
            r#"## Machine Learning System Playbook

1. Define the decision, target, population, label, baseline, offline and online metrics, error costs, latency, privacy, fairness, and human-override requirements.
2. Version data, features, code, configuration, model, environment, and evaluation artifacts; prevent leakage and preserve train/serve consistency.
3. Evaluate representative slices, calibration, robustness, drift, uncertainty, harmful failure modes, and comparison to simple baselines rather than optimizing one headline score.
4. Inference paths define availability, timeout, fallback, shadow/canary rollout, rollback, feedback capture, abuse controls, and reproducible incident diagnosis.
5. Deliver model/data cards, experiment lineage, approval evidence, monitoring, retraining triggers, rollback artifacts, ownership, and retirement criteria."#,
        ),
        PROJECT_TYPE_RESEARCH => (
            "Research & Investigation",
            "Literature, competitive, academic, industry, and feasibility research.",
            r#"## Research Playbook

1. State the research question, intended decision, scope, definitions, hypotheses, source criteria, method, time boundary, and stopping condition.
2. Distinguish primary, secondary, and anecdotal evidence; record author, date, version, incentives, methodology, sample, and access limitations.
3. Triangulate important claims, actively seek disconfirming evidence, and separate fact, interpretation, uncertainty, extrapolation, and recommendation.
4. Use ethical and privacy-safe collection; do not fabricate citations, interviews, measurements, consensus, or inaccessible-source conclusions.
5. Deliver an executive conclusion, evidence matrix, method, competing explanations, limitations, confidence, source list, and concrete decision implications."#,
        ),
        PROJECT_TYPE_PRODUCT_DESIGN => (
            "Product & Experience Design",
            "Product concepts, journeys, information architecture, interaction, visual, and service design.",
            r#"## Product & Experience Design Playbook

1. Define target users, jobs, context, pain, current journey, business objective, constraints, success measures, and accessibility needs before proposing screens.
2. Move from information architecture and task flows to wireframes and high-fidelity states; cover normal, empty, loading, error, permission, offline, edge-content, and responsive behavior.
3. Explain interaction rationale, hierarchy, feedback, validation, destructive actions, keyboard/focus behavior, content, and service touchpoints.
4. Validate risky assumptions with representative users or evidence, record findings and changes, and distinguish preference feedback from task-performance evidence.
5. Deliver source files, prototype, specifications, tokens/components, content, assets, accessibility notes, research evidence, open questions, and implementation acceptance criteria."#,
        ),
        PROJECT_TYPE_DESIGN_SYSTEM_BRAND => (
            "Design Systems & Brand",
            "Design tokens, component libraries, brand identity, visual standards, and cross-platform consistency.",
            r#"## Design System & Brand Playbook

1. Define brand principles, audiences, channels, foundations, token taxonomy, component ownership, contribution model, versioning, and adoption measures.
2. Components include anatomy, variants, states, behavior, content, accessibility, responsive rules, theming, localization, and do/don't guidance—not only screenshots.
3. Keep design and code sources synchronized through stable tokens, naming, release notes, migration guidance, visual regression, and documented exception handling.
4. Validate contrast, typography, spacing, motion, iconography, imagery, keyboard/focus, zoom, long content, dark mode, and multi-brand behavior.
5. Deliver editable brand assets, token files, component documentation, governance, release process, adoption plan, audit findings, and deprecation path."#,
        ),
        PROJECT_TYPE_NOVEL_WRITING => (
            "Novel Writing",
            "Long-form, short-form, serialized, and other narrative fiction.",
            r#"## Novel Writing Playbook

1. Establish premise, theme, audience, genre promise, narrative voice, point of view, character wants/needs, arcs, world rules, timeline, and ending direction.
2. Maintain a living outline and separate file for every chapter; track scene purpose, conflict, change, chronology, location, character state, clues, promises, and continuity.
3. Draft scenes around goal, obstacle, consequence, sensory specificity, subtext, and causal movement. Do not use word count or exposition as a substitute for story change.
4. Revise in passes for structure, character, pacing, causality, continuity, prose, dialogue, and copyediting; preserve intentional setup/payoff and remove accidental contradiction.
5. Deliver manuscript files, outline, character/world bible, timeline, continuity log, revision notes, unresolved decisions, and publication-format checks."#,
        ),
        PROJECT_TYPE_GENERAL_WRITING => (
            "Professional Writing",
            "Articles, reports, white papers, speeches, scripts, and nonfiction content.",
            r#"## Professional Writing Playbook

1. Define audience, purpose, desired action, channel, length, voice, evidence standard, legal/brand constraints, and approval owner.
2. Build a claim-driven outline before drafting; every section has a purpose, supports the central argument, and earns its place.
3. Verify facts, numbers, quotations, examples, and citations against accessible sources; clearly label uncertainty, opinion, projection, and sponsored claims.
4. Revise for logic, structure, clarity, specificity, tone, accessibility, bias, repetition, and publication format—not grammar alone.
5. Deliver editable source, final format, references, asset rights, metadata, revision notes, fact-check status, and unresolved approvals."#,
        ),
        PROJECT_TYPE_DOCUMENTATION => (
            "Documentation & Knowledge Bases",
            "Product docs, API references, operating manuals, tutorials, standards, and knowledge bases.",
            r#"## Documentation Playbook

1. Define audiences, tasks, prerequisites, supported versions, information architecture, ownership, review cadence, and retirement policy.
2. Separate concepts, tutorials, task procedures, reference, troubleshooting, and release/migration guidance; use consistent terminology and navigable structure.
3. Procedures include prerequisites, permissions, exact actions, expected results, failure recovery, safety warnings, and verification—not only happy-path screenshots.
4. Test code samples, commands, links, navigation, search terms, version labels, accessibility, localization, and representative user completion.
5. Deliver source, published output, metadata, redirects, ownership, review dates, feedback path, change log, and stale-content detection."#,
        ),
        PROJECT_TYPE_MARKETING_CONTENT => (
            "Marketing, Brand & Growth Content",
            "Campaigns, brand communication, social content, growth experiments, and sales enablement.",
            r#"## Marketing & Growth Content Playbook

1. Define segment, insight, objective, offer, channel, journey stage, message, evidence, brand constraints, compliance, and measurable success before production.
2. Maintain a message hierarchy and channel-specific variants while preserving factual consistency, consent, accessibility, and asset rights.
3. Experiments require a hypothesis, primary metric, guardrails, sample/exposure plan, attribution window, stopping rule, and decision threshold.
4. Review claims, testimonials, pricing, privacy, targeting, localization, links, tracking, rendering, frequency, and failure or opt-out paths before launch.
5. Deliver source assets, campaign matrix, approvals, tracking plan, launch checklist, results with uncertainty, learnings, and reuse/retirement decisions."#,
        ),
        PROJECT_TYPE_AUTOMATION => (
            "Automation, Agents & Integrations",
            "Workflows, scripts, agents, MCP services, robots, and repeated-task automation.",
            r#"## Automation & Agent Playbook

1. Define trigger, actor, input, output, permissions, state, timing, idempotency key, human decision points, success, stop, and escalation conditions.
2. Use preview or dry-run for risky actions; bound retries, concurrency, recursion, cost, time, and data scope. Never let a trigger silently create an uncontrolled loop.
3. Preserve durable state, audit, correlation, cancellation, resume, duplicate suppression, compensation, and clear ownership across tool or service boundaries.
4. Test malformed input, missing permission, partial completion, timeout, duplicate event, tool outage, stale state, human rejection, restart, and rollback.
5. Deliver observable execution history, alerts, runbook, manual fallback, security review, cost controls, versioned contracts, and safe disable/rollback procedures."#,
        ),
        PROJECT_TYPE_OPERATIONS => (
            "Operations & Continuous Services",
            "Product, platform, content, customer, and other ongoing operational services.",
            r#"## Operations Playbook

1. Define service objective, stakeholders, request channels, SLAs/SLOs, queues, roles, access, calendar, dependencies, metrics, and escalation paths.
2. Standardize recurring work with checklists, templates, approvals, audit, quality sampling, ownership, and exception handling while preserving expert judgment.
3. Monitor volume, backlog, aging, error, rework, satisfaction, cost, capacity, and risk; investigate root causes instead of optimizing vanity activity counts.
4. Incidents require containment, communication, evidence preservation, recovery, validation, review, and tracked prevention actions.
5. Deliver operating model, runbooks, dashboards, schedules, access roster, handover, continuity plan, improvement backlog, and review cadence."#,
        ),
        PROJECT_TYPE_IMPLEMENTATION_MIGRATION => (
            "Implementation, Migration & Go-live",
            "Enterprise implementation, data migration, system cutover, training, UAT, and operational handover.",
            r#"## Implementation, Migration & Go-live Playbook

1. Establish scope, process fit/gap, configuration, customizations, integrations, data, roles, environments, ownership, acceptance, adoption, and go-live criteria.
2. Migration requires source profiling, mapping, cleansing, transformation, reconciliation, exception handling, repeatable rehearsals, freeze rules, and business sign-off.
3. UAT uses realistic roles, scenarios, data, controls, defects, retest, and approval; training and support readiness are release gates, not post-launch tasks.
4. Cutover defines sequence, dependencies, owners, timing, checkpoints, communication, rollback threshold, contingency, and command-center operation.
5. Deliver configuration record, mapping, reconciliations, approvals, training, runbooks, support ownership, hypercare metrics, issue backlog, and formal handover."#,
        ),
        _ => (
            "General Project",
            "A collaborative project that does not yet fit a more specific category.",
            r#"## General Project Playbook

1. Clarify the objective, stakeholders, scope, constraints, deliverables, dependencies, risks, owner, and measurable acceptance criteria.
2. Choose a workflow appropriate to the actual work instead of forcing software-development assumptions onto research, writing, design, or operations.
3. Keep decisions, artifacts, evidence, status, and remaining risks traceable and accessible to the project team.
4. Validate the final output with the people and evidence appropriate to the domain; incomplete or unverified work cannot be marked done."#,
        ),
    }
}
