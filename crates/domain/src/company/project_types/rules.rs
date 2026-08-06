pub(super) const PROJECT_GOVERNANCE_RULES: &str = r#"## 项目治理与完成定义

1. 开始任何工作前，先读取项目 Rule、资产、任务、历史决定和当前仓库/文件状态；明确目标、用户、范围、非目标、约束、依赖、风险、负责人和可验证验收标准。没有任务、前置未完成或尚未轮到自己时保持静默，只在存在阻塞、风险或有价值建议时沟通。
2. 将工作拆成可独立验证的任务，并维护前置关系、状态和负责人。不得绕过未完成的前置任务；发现范围变化时先更新任务或请求 Human/PM 决策，不以隐性扩项替代沟通。
3. 重要架构、领域、数据、体验和运营决策必须记录背景、候选方案、取舍、后果与回退条件；不能让关键知识只存在于聊天或单个 Agent 的临时上下文中。
4. 任何结论、状态和“已完成”都必须附带证据。完成证据按项目形态包括但不限于：运行结果、测试、截图/设计稿、对账记录、来源、审阅记录、演练结果、发布链接或可复现步骤。
5. 失败、阻塞、未验证假设、数据缺口和剩余风险必须如实报告。禁止吞异常、伪造数据或引用、用占位内容冒充交付、忽略 Human 消息、把局部成功描述成整体完成。
6. 变更应可审阅、可追踪、可回退；保留必要的版本、迁移、兼容和审计信息。高风险、不可逆、外部发布、生产数据、资金、权限和批量动作必须经过明确审批。

## 安全、隐私与资产基线

1. 密钥、Token、个人信息和敏感业务数据使用专用安全机制，不进入代码、Rule、任务描述、日志、截图、示例数据或 Git 历史；展示和测试时使用脱敏或合成数据。
2. 所有外部依赖、素材、数据和引用都记录来源、许可、版本与使用限制；不得擅自复制受限内容或引入来源不明的资产。
3. 按最小权限工作，验证认证、授权、租户/组织隔离、审计和数据保留边界；发现越权、泄漏、破坏性风险时立即停止相关动作并上报。
4. 项目资产清单应持续维护，至少包含关键文档、设计、代码/脚本、数据、部署物、外部服务、负责人和当前状态。

## 所有项目通用的可视化与设计资产门禁

此门禁由交付形态触发，不由项目类型名称触发。任何项目只要交付物包含页面、屏幕、表单、管理后台、门户、网站、移动端、桌面端、PDA/设备界面、数据看板、可视化报表、游戏 HUD/菜单、控制台、落地页、交互原型或其他需要视觉布局与交互设计的内容，就必须在实现或正式制作前完成设计资产和评审；ERP、WMS、MES、CRM、数据、自动化、运营和通用项目都不能因为不叫“Web 项目”而跳过。

1. 先识别所有可视化交付面、使用角色、设备/媒介、关键任务和高风险状态，并把设计工作设置为实现工作的真实前置任务。
2. 页面、屏幕、HUD、控制台和交互界面必须交付可编辑源文件；二维界面默认至少提供可审阅的 SVG 线框或高保真 SVG，使用 Figma、Sketch 等工具时也要保留源文件，并提供 Agent、Human 和 Git 可访问的 SVG/PDF 导出。架构、流程、数据或状态设计使用适合该问题的可编辑图，不强行用页面 SVG 代替专业图示。
3. 设计必须覆盖信息架构、关键流程、布局、组件、真实内容、桌面/移动或目标设备尺寸，以及正常、空、加载、错误、无权限、离线、超长内容、确认/撤销、成功反馈和可访问性状态；不能只画一张理想首页。
4. 设计资产需记录版本、对应需求、评审结论、未决问题和实现约束。需求或流程变化后先更新设计和验收基线，再继续实现；旧设计图不能验收新版本。
5. 实现完成后必须对照同一候选版本的设计资产进行视觉、交互、响应式和可访问性验收。没有设计证据、设计未评审或实现与设计差异未闭环时，不得标记相关交付完成。

纯后端服务、纯数据管道、研究或文本交付如果确实没有任何视觉/交互产物，不要求虚构页面设计；但仍需按实际风险提供 API、状态机、数据流、架构、流程或内容结构等必要设计。"#;

pub(super) const SOFTWARE_PRODUCT_RULES: &str = r#"## 软件工程与架构基线

1. 实现前建立可审阅的技术方案：系统边界、模块职责、数据流、接口契约、状态机、错误模型、并发模型、外部依赖、容量目标、兼容策略、迁移与回滚。重要取舍使用 ADR 或等价决策记录。
2. 数据库、消息、缓存和外部调用必须明确一致性与失败语义；事务不能吞异常，重试必须幂等，迁移必须可恢复，异步流程必须可观测并能处理重复、乱序、超时与部分失败。
3. 面向用户的界面编码前先提交可审阅的 SVG 线框或高保真设计资产，覆盖正常、空、加载、错误、权限、离线、极端内容和响应式状态；实现后做视觉、键盘、语义和可访问性验收。
4. 测试必须覆盖正常、边界、失败、恢复、兼容和安全路径；缺陷修复必须有回归测试。交付前运行格式化、静态检查、单元/集成/端到端测试和干净构建，不能通过删除断言或跳过失败检查获得绿色结果。
5. 交付包含运行方式、配置、部署、监控、告警、备份、升级、回滚和故障排查说明；性能、可靠性、安全与兼容目标必须有实际测量或演练证据。

## 软件项目强制阶段流程

以下阶段是顺序门禁，不是建议清单。PM 或具备任务编排权限的 Agent 必须为阶段交付物创建任务和真实前置依赖；执行 Agent 每次开工前必须确认当前任务所属阶段、前置阶段状态和可访问证据。前一阶段未验收时，不得启动后一阶段，不得因为任务被误标为 `ready`、仓库已有部分代码或截止时间紧迫而跳过门禁。

1. **阶段 1：需求与验收基线**。先完成用户、场景、问题、目标、范围、非目标、业务规则、角色权限、主/异常流程、数据需求、非功能要求和可观察验收标准。交付需求说明、流程/场景、范围清单和未决问题；关键歧义由 Human 或产品负责人确认后才进入设计。
2. **阶段 2：体验与界面设计**。面向用户的界面必须先产出可编辑 SVG 线框或高保真设计资产，覆盖关键页面、桌面/移动断点、正常、空、加载、错误、权限、极端内容和破坏性操作状态，并记录设计评审结论。没有可见界面的项目也要先交付 API、状态机、时序或数据流设计，不能直接编码。
3. **阶段 3：技术选型与架构**。在已确认需求和设计基础上确定语言、框架、关键依赖、版本、系统边界、模块职责、数据模型、接口、状态、错误、认证授权、缓存/异步、测试、部署、监控和回滚方案；记录候选方案、取舍和 ADR。设计或需求变化后必须重新检查技术方案。
4. **阶段 4：工程骨架与质量基线**。建立目录与模块骨架、依赖锁定、配置加载、环境样例、格式化、静态检查、测试框架、日志、错误入口、健康检查和 CI 基础。骨架必须能在干净环境启动并通过最小检查，不能用大段业务实现掩盖基础设施尚不可用。
5. **阶段 5：基础模块开发**。实现后续核心逻辑依赖的共享能力，例如身份与权限、数据访问、迁移、API 客户端、路由、布局、设计 Token、通用组件、表单、错误处理、分页、国际化、审计和可观测性。每项能力需独立测试和可复用，不把临时桩当作完成品。
6. **阶段 6：核心逻辑开发**。按可运行的纵向切片实现核心业务流程，每个切片贯通真实界面/API/数据或对应系统边界，覆盖业务不变量、并发、幂等、权限和失败恢复。不得在基础契约未稳定时无边界并行堆叠功能。
7. **阶段 7：系统测试与缺陷闭环**。执行单元、集成、端到端、契约、回归、安全、性能、兼容、可访问性和恢复测试中与风险匹配的集合；使用真实接口和代表性数据验证验收标准。失败必须修复并增加回归证据，不能跳过、降级断言或仅凭截图判定通过。
8. **阶段 8：Docker 与部署验证**。只有阶段 7 达到发布门禁后，才完成生产化 Dockerfile/Compose、构建上下文、非 root 运行、健康检查、配置与密钥注入、迁移、持久化、网络、监控、备份和回滚；必须执行无缓存干净构建、启动、冒烟、升级/恢复与目录枚举检查。
9. **阶段 9：验收、发布与交接**。由对应产品、设计、技术、QA、运维或业务责任人基于证据签署；更新项目资产、运行手册、部署说明、已知限制、残余风险和后续责任。任一验收标准、生产门禁或交接项未完成时，项目不得标记完成。

允许同一阶段内对边界清晰的任务并行，但不得跨越未通过的阶段门禁。已有项目先做阶段审计，复用真实有效的既有证据并补齐缺口，不要求机械重做；紧急修复只有在 Human 明确授权、影响范围受控且同步建立补偿测试、文档和后续治理任务时才可例外。"#;

pub(super) const ENTERPRISE_SYSTEM_RULES: &str = r#"## 企业系统领域基线

1. 先建立领域词汇表、组织/角色、主数据、业务单据、状态流转、编号规则、审批点、例外流程、会计/库存影响和跨模块边界；界面字段和数据库结构不得替代领域建模。
2. 对数量、金额、税、币种、单位、批次、序列号、期间和状态定义不变量。关键台账只能通过受控业务动作变化，必须支持来源追溯、对账、冲销/更正和审计，禁止直接覆盖历史事实。
3. 权限必须覆盖组织、岗位、数据范围、字段、动作和审批，并考虑职责分离；批量导入、导出、删除、反审核、关账和调整属于高风险动作。
4. 与财务、库存、订单、设备和第三方平台的集成必须有版本化契约、幂等键、业务回执、重放、死信、对账和补偿方案；接口成功不等于业务入账成功。
5. 上线前必须完成主数据治理、历史数据迁移、期初/在途数据处理、真实角色 UAT、并发与容量测试、切换演练、回滚演练和上线后对账。"#;

pub(super) const DATA_RESEARCH_RULES: &str = r#"## 数据、模型与研究基线

1. 先定义问题、口径、假设、样本、时间窗口、来源、许可、评价方法和停止条件；区分事实、观察、推断、预测和建议。
2. 原始数据只读保存，转换过程可复现；维护数据字典、Schema、血缘、质量检查、缺失/异常处理和环境依赖。关键结果应通过独立方法、抽样或交叉来源复核。
3. 明确偏差、泄漏、代表性、统计不确定性、伦理和隐私限制；不得以图表美观、模型分数或来源数量冒充结论可靠性。
4. 交付必须包含可复现步骤、版本化输入、方法、结果、限制、置信度和决策影响；数据或模型变化后能够重跑、比较和解释差异。"#;

pub(super) const DESIGN_CONTENT_RULES: &str = r#"## 设计、内容与知识基线

1. 先确认目标受众、使用场景、媒介、语气、结构、品牌/风格约束、事实边界和成功标准；以用户任务和证据驱动方案，不以个人偏好代替判断。
2. 从信息架构、提纲、用户流程或低保真方案开始，方向确认后再进入高成本制作；关键内容拆分为稳定文件，维护版本、术语、素材、引用和状态。
3. 事实、数字、引文和案例必须可追溯；遵守版权、肖像、隐私和品牌规则。AI 生成内容需要人工可审阅，不得伪造来源、用户反馈或效果。
4. 交付覆盖目标媒介的格式、可访问性、极端状态、链接、排版和发布检查，并提供源文件、使用规范、已知限制和后续维护方式。"#;

pub(super) const OPERATIONS_DELIVERY_RULES: &str = r#"## 自动化、运营与交付基线

1. 先建立目标、对象、触发条件、输入输出、负责人、时间窗、权限、依赖、检查清单、成功指标、停止条件和升级路径。
2. 批量、生产、对外发布和不可逆动作必须支持预览或 dry-run、分批执行、审批、审计、幂等、超时、取消、重试上限和可执行回滚。
3. 执行中持续展示进度、当前动作、偏差、失败与下一步；异常先止损和保护数据，再定位根因。不得无限重试、静默跳过失败或在未核验结果时宣布完成。
4. 交付运行手册、监控告警、值守与接管方式、恢复步骤、复盘和可复用模板；一次性成功不能替代长期可运营性。"#;

pub(super) const PROJECT_GOVERNANCE_RULES_EN: &str = r#"## Project Governance and Definition of Done

1. Before acting, read the project Rule, assets, tasks, dependencies, decisions, messages, and current repository or file state. Establish the objective, users, scope, non-goals, constraints, risks, owner, and verifiable acceptance criteria. If no work is assigned, prerequisites are incomplete, or it is not your turn, remain silent unless you have evidence that can remove a blocker or prevent delivery failure.
2. Break work into independently verifiable outcomes with explicit ownership and prerequisites. Do not bypass incomplete dependencies or silently expand scope; update the plan or request a Human/PM decision.
3. Record material architecture, domain, data, experience, operational, and policy decisions with context, options, trade-offs, consequences, and rollback conditions. Critical knowledge must not exist only in chat or transient Agent context.
4. Every conclusion and completion claim requires evidence appropriate to the work: executed tests, screenshots or design assets, reconciliations, sources, review records, rehearsals, publication links, measurements, or reproducible steps.
5. Report failures, blockers, unverified assumptions, data gaps, and residual risks honestly. Never suppress exceptions, fabricate data or sources, ignore Human messages, use placeholders as deliverables, or describe partial success as complete.
6. Keep changes reviewable, traceable, and reversible. High-risk, irreversible, external publication, production-data, financial, permission, and bulk actions require explicit approval and a recovery plan.

## Security, Privacy, and Asset Baseline

1. Store secrets, tokens, personal data, and sensitive business information only in approved secure mechanisms. Do not place them in code, Rules, task text, logs, screenshots, examples, generated Skills, or Git history; use redacted or synthetic data for demonstrations and tests.
2. Record source, license, version, ownership, and usage restrictions for external dependencies, data, content, and assets. Do not import restricted or unverified material.
3. Work with least privilege and validate authentication, authorization, tenant/company separation, audit, retention, and destructive-action boundaries. Stop and escalate suspected privilege escalation, leakage, or irreversible damage.
4. Maintain a current project asset inventory covering important documents, designs, code/scripts, data, deployments, integrations, owners, status, and recovery-critical material.

## Universal Visual and Design Asset Gate

This gate is triggered by the delivery shape, not the project-type label. Any project that delivers pages, screens, forms, admin consoles, portals, websites, mobile or desktop UI, PDA/device interfaces, dashboards, visual reports, game HUDs or menus, operator consoles, landing pages, interactive prototypes, or any other visual layout and interaction must complete design assets and review before implementation or final production. ERP, WMS, MES, CRM, data, automation, operations, and general projects do not bypass the gate merely because they are not named “Web.”

1. Identify every visual surface, user role, device or medium, critical task, and high-risk state. Make design a real prerequisite of implementation work.
2. Pages, screens, HUDs, consoles, and interactive UI require editable source. Two-dimensional UI provides at least reviewable SVG wireframes or high-fidelity SVG; when using Figma, Sketch, or another design tool, retain source and export SVG/PDF that Agents, Humans, and Git can access. Architecture, process, data, and state design uses an appropriate editable diagram rather than forcing page SVG onto the wrong problem.
3. Cover information architecture, critical flows, layout, components, realistic content, desktop/mobile or target-device sizes, plus normal, empty, loading, error, unauthorized, offline, long-content, confirmation/undo, success-feedback, and accessibility states. One idealized home screen is not sufficient.
4. Record design version, linked requirements, review decision, open issues, and implementation constraints. Update design and acceptance before continuing when requirements or flows change; an old design cannot accept a new candidate.
5. After implementation, validate visual fidelity, interaction, responsiveness, and accessibility against design assets from the same candidate. Do not complete visual delivery without design evidence, design review, and closure of implementation differences.

Pure backend services, data pipelines, research, or text deliverables with no visual or interactive output do not invent page designs. They still provide API, state-machine, data-flow, architecture, process, or content-structure design appropriate to their real risks."#;

pub(super) const SOFTWARE_PRODUCT_RULES_EN: &str = r#"## Software Engineering and Architecture Baseline

1. Produce a reviewable technical plan before implementation: system boundaries, module responsibilities, data flow, interface contracts, state, error model, concurrency, dependencies, capacity, compatibility, migration, and rollback. Capture material trade-offs in ADRs or equivalent decision records.
2. Define consistency and failure semantics for databases, messaging, caches, files, and external calls. Transactions must not suppress errors; retries require idempotency; migrations require recovery; asynchronous flows must handle duplicate, disorder, timeout, and partial failure.
3. For user-visible interfaces, create reviewable SVG wireframes or equivalent high-fidelity assets before coding. Cover normal, empty, loading, error, permission, offline, extreme-content, and responsive states; verify visual quality, keyboard access, semantics, and accessibility after implementation.
4. Test normal, boundary, failure, recovery, compatibility, and security paths. Every defect fix needs a regression test. Run formatting, static analysis, unit/integration/end-to-end tests, and a clean build appropriate to the change; never obtain green status by deleting assertions or skipping failing checks.
5. Deliver run, configuration, deployment, monitoring, alerting, backup, upgrade, rollback, and troubleshooting guidance. Performance, reliability, security, and compatibility claims require measurements or rehearsals.

## Mandatory Phase-Gated Software Delivery Workflow

These phases are ordered gates, not optional advice. A PM or Agent with task-planning permission must create tasks and real prerequisites for phase deliverables. Before starting, every executing Agent must identify the task's phase, the preceding gate status, and accessible evidence. Do not start a later phase before the prior phase is accepted, even when a task is incorrectly marked `ready`, the repository already contains partial code, or schedule pressure exists.

1. **Phase 1 — Requirements and acceptance baseline.** Define users, scenarios, problem, outcome, scope, non-goals, business rules, roles and permissions, primary and failure flows, data needs, non-functional requirements, and observable acceptance criteria. Deliver a requirements brief, flows/scenarios, scope list, and open decisions; Human or product ownership resolves material ambiguity before design begins.
2. **Phase 2 — Experience and interface design.** User-visible work requires editable SVG wireframes or high-fidelity design assets covering key screens, desktop/mobile breakpoints, normal, empty, loading, error, permission, extreme-content, and destructive-action states, plus a recorded design review. Work without visible UI still requires API, state-machine, sequence, or data-flow design before coding.
3. **Phase 3 — Technology selection and architecture.** Based on approved requirements and design, decide language, framework, critical dependencies and versions, system boundaries, module ownership, data model, interfaces, state, errors, authentication/authorization, cache/async behavior, test strategy, deployment, monitoring, and rollback. Record alternatives, trade-offs, and ADRs; re-evaluate when requirements or design changes.
4. **Phase 4 — Engineering scaffold and quality baseline.** Establish repository/module structure, locked dependencies, configuration loading, environment examples, formatting, static analysis, test harness, logging, error entry points, health checks, and CI foundation. Prove a clean environment can start and run minimum checks before hiding missing foundations under business code.
5. **Phase 5 — Foundation modules.** Build reusable capabilities required by core behavior: identity and authorization, persistence and migrations, API client, routing, layout, design tokens, shared components, forms, error handling, pagination, localization, audit, and observability as applicable. Test each capability independently; temporary stubs are not finished modules.
6. **Phase 6 — Core logic.** Implement core journeys as runnable vertical slices across real UI/API/data or equivalent system boundaries. Each slice preserves domain invariants, concurrency, idempotency, permission, and failure recovery. Do not stack unbounded parallel features on unstable contracts.
7. **Phase 7 — System verification and defect closure.** Run the risk-appropriate combination of unit, integration, end-to-end, contract, regression, security, performance, compatibility, accessibility, and recovery tests using real interfaces and representative data. Fix failures and add regression evidence; never skip checks, weaken assertions, or use screenshots alone as proof.
8. **Phase 8 — Docker and deployment verification.** Only after Phase 7 reaches its release gate, complete production Dockerfile/Compose behavior, build context, non-root execution, health checks, configuration and secret injection, migrations, persistence, networking, monitoring, backup, and rollback. Execute a no-cache clean build, startup, smoke, upgrade/recovery, and directory-enumeration verification.
9. **Phase 9 — Acceptance, release, and handover.** Product, design, engineering, QA, operations, or business owners sign according to evidence. Update project assets, runbooks, deployment guidance, known limitations, residual risk, and follow-up ownership. Do not complete the project while any acceptance, production gate, or handover item remains open.

Parallel work is allowed only inside the same phase when boundaries are explicit; it may not bypass an unaccepted gate. Existing projects first perform a phase audit, reuse valid evidence, and backfill gaps rather than mechanically restarting. An emergency fix may bypass a gate only with explicit Human authorization, bounded impact, and compensating tests, documentation, and tracked follow-up governance."#;

pub(super) const ENTERPRISE_SYSTEM_RULES_EN: &str = r#"## Enterprise System Domain Baseline

1. Establish a domain glossary, organization and roles, master data, business documents, state transitions, numbering, approvals, exception flows, accounting/inventory impacts, and module boundaries before designing screens or tables.
2. Define invariants for quantity, amount, tax, currency, unit, lot, serial, period, and status. Critical ledgers change only through controlled business actions and support traceability, reconciliation, reversal/correction, and audit; never overwrite historical facts directly.
3. Authorization covers organization, role, data scope, field, action, and approval with segregation of duties. Bulk import/export, delete, reverse approval, period close, and adjustment are high-risk actions.
4. Finance, inventory, order, device, and third-party integrations require versioned contracts, idempotency, business acknowledgements, replay, dead letters, reconciliation, and compensation. Transport success does not prove business posting success.
5. Before launch complete master-data governance, historical/opening/in-transit migration, realistic-role UAT, concurrency and capacity testing, cutover and rollback rehearsals, and post-launch business reconciliation."#;

pub(super) const DATA_RESEARCH_RULES_EN: &str = r#"## Data, Model, and Research Baseline

1. Define the question, metric or claim, population, sample, time window, source, permission, method, evaluation, and stopping condition. Distinguish fact, observation, inference, prediction, and recommendation.
2. Preserve raw data as read-only and make transformations reproducible. Maintain schema, dictionary, lineage, environment, quality checks, and missing/outlier handling; independently verify important results through another method, sample, or source.
3. Address bias, leakage, representativeness, uncertainty, ethics, and privacy. Attractive charts, high model scores, or many sources do not by themselves make a conclusion reliable.
4. Deliver versioned inputs, reproducible steps, methods, results, limitations, confidence, and decision impact. Changes to data, code, or models must be rerunnable and comparable."#;

pub(super) const DESIGN_CONTENT_RULES_EN: &str = r#"## Design, Content, and Knowledge Baseline

1. Define audience, use context, medium, voice, structure, brand/style constraints, factual boundaries, accessibility, and success criteria. Use user tasks and evidence rather than personal preference.
2. Begin with information architecture, outline, journey, flow, or low-fidelity direction before high-cost production. Keep important content in stable files with version, terminology, source, asset, and status tracking.
3. Facts, numbers, quotations, and examples must be traceable. Respect copyright, likeness, privacy, and brand requirements. AI-generated material remains reviewable and cannot fabricate sources, feedback, or outcomes.
4. Validate target-format behavior, links, layout, accessibility, edge cases, and publication readiness. Deliver source files, usage guidance, known limitations, ownership, and maintenance expectations."#;

pub(super) const OPERATIONS_DELIVERY_RULES_EN: &str = r#"## Automation, Operations, and Delivery Baseline

1. Establish objective, actors, trigger, input/output, owner, timing, permissions, dependencies, checklist, success metric, stop condition, and escalation path.
2. Bulk, production, external, and irreversible actions require preview or dry-run, staged execution, approval, audit, idempotency, timeout, cancellation, bounded retries, and executable rollback.
3. Continuously expose progress, current action, deviation, failure, and next step. On failure, contain harm and protect data before root-cause analysis. Never retry indefinitely, silently skip failure, or claim success before verification.
4. Deliver runbooks, monitoring, alerting, on-call/takeover guidance, recovery steps, review findings, and reusable operating templates. A one-time success is not sustainable operation."#;

pub(super) const SOFTWARE_DEVELOPMENT_RULES: &str = r#"## 通用软件开发专项规则

1. 动手开发前先读取现有代码、文档和任务，整理目标、用户场景、范围、非目标、验收标准与未决问题；关键歧义必须先向 Human 澄清，不能用猜测替代需求。
2. 先设计再实现：说明架构边界、数据流、接口、状态、错误模型、迁移和回滚方案。涉及可见界面时，编码前必须提交可审阅的 SVG 设计图或等价高保真设计资产，并覆盖关键状态、响应式布局与交互。
3. 优先复用现有组件和工程约定；控制改动范围，不重复造轮子，不吞异常，不将密钥或隐私数据写入代码、日志和仓库。
4. 测试必须证明行为正确：至少覆盖正常路径、边界、失败与恢复路径；修复缺陷必须有回归测试。禁止只改展示、跳过失败测试或用“理论可行”代替运行验证。
5. 完成交付前运行与风险匹配的格式化、静态检查、单元/集成测试和干净构建；涉及数据库、并发、安全、部署或兼容性时必须增加专项验证与回滚说明。
6. 每次交付说明已完成内容、证据、剩余风险和明确下一步；未满足验收标准不得标记完成。"#;

pub(super) const WEB_APPLICATION_RULES: &str = r#"## Web 项目不可跳过的执行顺序

Web 项目必须按照下列顺序推进，并在任务系统中建立对应前置关系。每一步都要留下可访问的项目资产和验收证据；缺少上一步交付物时，Agent 必须停止当前任务并向 PM/技术经理指出缺失门禁，不能自行补一句说明后继续编码。

1. **写需求**：形成产品/需求说明，至少包含目标用户、核心问题、页面与路由清单、角色权限、主流程、异常流程、业务规则、数据、SEO/分析需求、浏览器设备范围、性能与可访问性目标、验收标准和非目标。
2. **画 SVG 设计图**：在 `docs/design/` 或项目约定资产目录保存可编辑 SVG，覆盖主要页面和关键组件的桌面、平板/移动布局，以及空、加载、错误、无权限、超长内容、确认/撤销和成功反馈。设计经 Human/产品/设计责任人确认前不得进入技术实现。
3. **完成技术选型**：记录 CSR/SSR/SSG 选择、前后端框架及版本、数据库、认证、状态与缓存、API 契约、文件/上传、国际化、测试栈、部署方案、浏览器矩阵、安全边界和主要 ADR；不得仅以“熟悉”作为选型依据。
4. **搭建工程框架**：建立前后端/全栈目录、依赖锁、配置、环境样例、路由骨架、构建、Lint/Format、类型检查、测试框架、日志、错误页、健康检查和 CI 基础，并证明干净安装、启动和最小测试通过。
5. **开发基础模块**：先完成设计 Token、全局布局、导航、响应式框架、通用组件、表单与校验、认证会话、权限守卫、API Client、统一错误处理、加载/空态、分页或虚拟化、国际化和可观测基础；未通过组件/集成验证不得开始大规模核心页面。
6. **开发核心逻辑**：按真实用户旅程逐个交付纵向切片，贯通页面、接口、权限、数据和反馈；每个切片完成正常、边界、失败、重复提交、会话过期和恢复行为后再扩展下一流程。
7. **执行系统测试**：对照需求和 SVG 做视觉/交互验收，使用真实接口完成关键旅程端到端测试；覆盖支持视口与浏览器、键盘/语义/对比度、慢网、刷新与前进后退、缓存失效、权限变化、XSS/CSRF、性能预算和回归。
8. **完成 Docker 部署**：测试门禁通过后再完成生产镜像与 Compose，使用多阶段构建、非 root 用户、精确复制目录、`.dockerignore`、健康检查、环境与密钥注入、数据库迁移、持久化、反向代理、安全头、日志监控和回滚；必须用无缓存干净构建实际启动并冒烟验证。
9. **发布与验收**：提供访问入口、版本/提交、设计对照结果、测试报告、部署与恢复步骤、监控告警、已知限制、项目资产和残余风险，由对应责任人完成签署后才可 `done`。

## Web 专项质量门禁

1. 服务端与客户端状态不得产生无法解释的水合差异；URL、表单、上传、缓存、乐观更新、分页和 API 错误行为必须确定且可测试。
2. 设定 Core Web Vitals、首屏、包体、图片、字体和请求预算；避免无边界全量加载，列表、搜索和历史数据默认分页或虚拟化。
3. 防护 XSS、CSRF、开放重定向、点击劫持、敏感缓存和前端密钥泄漏；认证失效、跨标签页、刷新恢复和权限变化必须有确定行为。
4. SVG、需求、架构、代码、测试和部署证据必须属于同一受测版本；旧截图、旧容器或不同分支的测试不能证明当前候选完成。"#;

pub(super) const MOBILE_APPLICATION_RULES: &str = r#"## 移动端专项规则

1. 明确 iOS/Android/设备版本矩阵、屏幕、方向、深色模式、辅助功能、系统权限、生命周期、后台限制和应用商店政策。
2. 设计离线、弱网、切后台、进程终止、重复点击、推送唤醒、深链和本地数据迁移；同步必须有冲突策略、幂等键和可恢复队列。
3. 相机、定位、蓝牙、扫码、通知和文件等能力按最小权限申请，并提供拒绝、永久拒绝和系统设置引导；敏感数据使用系统安全存储。
4. 在真实设备上验证启动、耗电、内存、流量、崩溃、升级和兼容性；发布前完成签名、版本号、隐私清单、灰度、回滚和商店素材检查。"#;

pub(super) const DESKTOP_APPLICATION_RULES: &str = r#"## 桌面应用专项规则

1. 明确 Windows/macOS/Linux 支持矩阵、安装位置、用户数据目录、文件关联、快捷键、窗口行为、系统托盘和原生权限边界。
2. 安装、首次启动、自动更新、降级、卸载和用户数据迁移必须可验证；更新失败不能破坏现有可运行版本，签名与发布产物需校验。
3. 文件系统、子进程、剪贴板、协议链接和本地服务属于高风险边界，必须校验输入、限制权限并避免命令注入和任意文件访问。
4. 在干净系统和升级路径上验证启动、崩溃恢复、休眠唤醒、多显示器、缩放、离线和大文件场景。"#;

pub(super) const BACKEND_SERVICE_RULES: &str = r#"## 后端服务与 API 专项规则

1. 先定义版本化 API/事件契约、认证授权、租户边界、幂等、分页、过滤、排序、限流、错误码和兼容策略，并提供可执行示例或契约测试。
2. 事务边界与领域不变量必须明确；数据库异常不能转换成成功或空结果。跨服务写入使用 outbox、Saga 或等价补偿，不能依赖理想网络。
3. 迁移采用向前/向后兼容的展开—迁移—收缩流程；大表变更、回填、索引和锁影响需要演练、限速、监控与恢复测试。
4. 提供结构化日志、指标、追踪、健康检查、超时、熔断和容量预算；负载、并发、资源耗尽和依赖故障必须有实测证据。"#;

pub(super) const LIBRARY_SDK_RULES: &str = r#"## 库、SDK 与开发工具专项规则

1. 先定义公共 API、支持语言/运行时/平台矩阵、稳定性承诺和弃用政策；公共符号、错误类型、配置和默认行为都属于兼容面。
2. 使用语义化版本和变更日志；破坏性变化必须提供迁移指南、弃用周期和兼容测试，不得悄悄改变行为。
3. 示例必须能在干净环境运行，覆盖安装、最小用法、常见集成、错误处理和安全配置；文档与发布包中的 API 保持一致。
4. 测试覆盖多版本矩阵、序列化/协议兼容、并发、资源释放和下游集成；发布产物需验证内容、签名、来源和可重复构建。"#;

pub(super) const IOT_EMBEDDED_RULES: &str = r#"## IoT 与嵌入式专项规则

1. 固化硬件版本、引脚/总线、时序、功耗、内存、存储、网络、协议和环境约束；软件假设必须能追溯到设备规格或实测。
2. 通信协议定义帧格式、版本、校验、重传、去重、时钟漂移、断网缓存和兼容策略；设备命令必须认证、授权并防重放。
3. OTA 升级采用签名、分批、双分区或等价恢复机制，断电和升级失败不得使设备不可恢复；保留安全回退与现场救援路径。
4. 使用硬件在环或等价测试覆盖传感器异常、边界值、掉电、弱网、长时间运行、温度/功耗和并发设备规模。"#;

pub(super) const ERP_RULES: &str = r#"## ERP 专项规则

1. 按财务、采购、销售、库存、制造、人力、项目等业务域划分边界，建立公司、组织、科目、物料、客户、供应商、税、币种和计量单位等主数据治理规则。
2. 单据必须定义草稿、提交、审核、过账、关闭、取消、冲销等状态，以及来源单、目标单、数量/金额传递和跨模块影响；已过账事实不得通过普通编辑覆盖。
3. 财务相关功能遵守借贷平衡、期间、汇率、税、成本和辅助核算不变量；关账、反关账、重估、冲销和期初导入必须审批、审计和对账。
4. 迁移按主数据、未结业务、库存余额、财务期初和历史档案分层验证；上线必须完成端到端业务场景 UAT、权限职责分离检查和新旧系统余额核对。"#;

pub(super) const WMS_RULES: &str = r#"## WMS 仓储管理专项规则

1. 先建模仓库、库区、库位、容器/LPN、货主、物料、包装、单位、批次、序列号、效期和库存状态；分别定义在库、可用、分配、冻结、质检、残损、在途数量及其转换不变量。
2. 覆盖 ASN/预约、收货、质检、上架、补货、移库、分配、波次、拣选、复核、包装、装车、发运、退货、盘点和调整，并为短收、超收、错货、缺货、破损和取消建立例外流程。
3. 库存变化使用可追溯台账和原子业务动作；并发分配、拣选确认、撤销和接口重试不得产生负库存、超分配或重复扣减。盘点差异、调整和冻结必须审批并可对账。
4. PDA、扫码枪、打印机、称重、输送线、PLC/MFC 和自动化设备需定义离线、重复扫码、超时、乱序、人工接管和设备降级；界面必须适配快速操作、手套/小屏和弱网。
5. ERP、OMS、TMS 和设备集成使用业务幂等键、回执、重放、死信和日终对账。测试必须覆盖同库存并发、批次/效期、单位换算、波次拆并、盘点冻结、峰值单量和设备故障。"#;

pub(super) const CRM_RULES: &str = r#"## CRM 专项规则

1. 明确定义线索、客户、联系人、商机、报价、合同、活动和客户成功对象的归属、状态、转换和重复判定；合并记录必须保留来源与审计。
2. 销售漏斗阶段、赢率、金额、预计日期和关闭原因要有统一口径；自动评分、分配和提醒可解释、可覆盖并防止循环触发。
3. 邮件、电话、会议、表单和营销来源需正确归因并处理退订、同意、隐私请求和数据保留；敏感客户数据按角色、团队和区域隔离。
4. 报表需区分活动量、管道、预测和实际收入，验证历史快照与当前状态差异；集成失败不能丢失客户互动。"#;

pub(super) const MES_RULES: &str = r#"## MES 制造执行专项规则

1. 建模工厂、产线、工作中心、设备、物料、BOM、工艺路线、工序、工单、批次/序列号、班次和人员资质，并明确 ERP、WMS、QMS、设备层的系统边界。
2. 工单下达、领料、开工、报工、暂停、返工、完工、入库和关闭必须形成在制品与物料消耗台账；禁止跳过必需工序或破坏正反向追溯。
3. 质量计划、检验、SPC、不合格、隔离、处置和 CAPA 要与批次、设备、人员、参数绑定；配方、工艺和参数版本必须按生效时间受控。
4. 设备采集处理时钟、断连、补传、重复、乱序和质量码；OEE、产量、良率和停机原因口径需可追溯。验证高频数据、长周期工单、换线、返工和设备离线。"#;

pub(super) const ECOMMERCE_RULES: &str = r#"## 电商与交易平台专项规则

1. 分离商品、SKU、库存地点、价格表、促销、购物车、结算、支付、订单、履约、退货和退款边界；所有金额明确币种、税、舍入和优惠分摊规则。
2. 结算必须以服务端重新定价为准；支付创建、回调、查询、取消和退款使用幂等键与签名校验，不能因重试重复扣款或重复发货。
3. 库存预占、释放、扣减和超卖策略与订单状态保持一致；部分发货、拆单、取消、拒收、退货和售后需完整可追溯。
4. 促销叠加、有效期、使用次数和滥用防护应可配置并有边界测试；搜索、推荐和埋点不得泄漏隐私或操纵关键交易事实。
5. 端到端验证峰值流量、支付故障、库存竞争、价格变化、税费、优惠、退款、Webhook 重放和对账。"#;

pub(super) const DATA_ENGINEERING_RULES: &str = r#"## 数据工程与平台专项规则

1. 为每个数据集定义所有者、数据契约、Schema、分区、主键、更新频率、SLA、保留和质量阈值；原始层不可被下游任务原地改写。
2. 批处理和流处理必须幂等，明确事件时间、水位线、迟到、重复、乱序、重放、回填和断点续跑语义；Schema 演进需要上下游兼容计划。
3. 数据质量覆盖完整性、唯一性、及时性、有效性和业务对账；失败阻断下游或显式降级，不能静默产出错误报表。
4. 维护血缘、运行元数据、成本、容量和告警；在生产规模上验证倾斜、小文件、反压、依赖故障和历史回填。"#;

pub(super) const MACHINE_LEARNING_RULES: &str = r#"## 机器学习与模型系统专项规则

1. 固化任务定义、基线、数据版本、特征、训练/验证/测试切分、指标和业务成本；主动检查标签泄漏、时间穿越、重复样本和群体偏差。
2. 实验必须记录代码、参数、随机种子、环境、数据和产物；模型选择不能只看单一离线指标，应包含误差分析、鲁棒性、校准和与基线比较。
3. 交付模型卡或等价说明，记录适用范围、限制、风险、训练数据来源和 Human 监督点；高影响场景需要公平性、隐私和滥用评估。
4. 推理服务定义版本、延迟、吞吐、降级、回滚和特征一致性；上线后监控数据漂移、性能、反馈回路和实际业务效果。"#;

pub(super) const DESIGN_SYSTEM_RULES: &str = r#"## 设计系统与品牌专项规则

1. 先审计现有界面与品牌资产，建立颜色、排版、间距、圆角、阴影、动效和语义 Token；命名表达用途，不绑定单一页面或当前颜色值。
2. 组件按解剖、变体、尺寸、状态、内容规则、交互、可访问性和平台差异定义，并提供设计与代码映射；避免只有截图没有可复用规范。
3. 每次变更评估视觉回归、下游使用、主题、国际化和破坏性影响，采用版本、迁移说明和废弃周期。
4. 品牌资产记录源文件、导出规格、安全区、最小尺寸、授权和错误用法，并在真实媒介和对比度条件下验证。"#;

pub(super) const IMPLEMENTATION_MIGRATION_RULES: &str = r#"## 实施、迁移与上线专项规则

1. 先完成现状调研、差距分析、目标流程、配置清单、定制边界、接口清单、数据清单、角色矩阵和验收场景，明确哪些需求通过流程调整而非定制实现。
2. 数据迁移分为提取、映射、清洗、转换、装载和对账；为每轮演练记录错误、修复和余额/数量差异，最终切换前冻结映射规则。
3. UAT 使用真实角色和端到端业务样例，缺陷按严重度闭环；培训、操作手册、权限、主数据和支持流程必须在上线前就绪。
4. 切换计划精确到时间窗、负责人、依赖、检查点、停止条件和回滚步骤；上线后执行业务对账、监控、Hypercare 和正式交接。"#;

pub(super) const GAME_DEVELOPMENT_RULES: &str = r#"## 游戏开发固定执行规则

1. 先定义目标玩家、平台、核心体验、核心玩法循环、胜负条件、操作方式和性能预算，再建立最小可玩原型验证“是否好玩”。
2. 玩法、模拟、渲染、UI、输入、音频、存档和资产管线保持清晰边界；随机性必须可复现，关键数值集中配置。
3. 有界面或 HUD 时先产出 SVG 线框/视觉稿，保证不遮挡主要玩法区域，并覆盖暂停、失败、胜利、加载和不同分辨率。
4. 美术与音频资产必须记录来源、授权、尺寸、锚点、压缩和加载策略；不得以临时占位素材冒充最终交付。
5. 测试需包含可重复的玩法冒烟、输入边界、存档恢复、关卡可达性、性能帧率和资源加载；实际试玩后记录问题再迭代。
6. 每个里程碑必须保持可运行、可试玩、可回退，不能只交付散落代码或未经验证的玩法描述。"#;

pub(super) const NOVEL_WRITING_RULES: &str = r#"## 小说创作固定执行规则

1. 正文前先建立创作意图、类型、受众、主题、叙事视角、篇幅目标、世界观、人物小传、人物关系和完整分卷/章节大纲。
2. 每个章节必须使用独立文件，文件名保持稳定顺序；另设大纲、人物、世界观、时间线、伏笔与术语表文件，不把全部内容堆在一个文档中。
3. 每章动笔前明确场景目标、冲突、转折、信息增量和结尾钩子；章节完成后检查人物动机、时间线、空间关系、称谓和设定连续性。
4. 伏笔要登记埋设与回收位置；新增设定必须同步维护设定集。禁止用无意义重复、空泛抒情或机械总结凑字数。
5. 修改分为结构修订、情节修订、人物修订和文字润色，保留版本记录；Human 未确认整体方向前不要大规模重写已批准章节。
6. 交付时提供章节状态、字数、关键变化、连续性风险和下一章计划。"#;

pub(super) const GENERAL_WRITING_RULES: &str = r#"## 通用写作固定执行规则

1. 先确认受众、目的、发布渠道、语气、长度、事实边界和成功标准，再列结构提纲。
2. 事实、引文和数字必须可追溯；不确定内容明确标注，不编造来源。涉及他人作品时遵守版权和引用规范。
3. 每一部分只承担一个清晰功能，标题层级、术语、叙述人称和格式保持一致；避免套话、重复和无证据结论。
4. 完成初稿后至少进行结构、事实、语言和格式四轮检查，并根据发布媒介校验链接、排版和可访问性。
5. 保存大纲、素材、初稿和定稿的清晰版本；交付说明面向谁、解决什么问题和仍需 Human 确认的事实。"#;

pub(super) const RESEARCH_RULES: &str = r#"## 研究调研固定执行规则

1. 先定义研究问题、范围、假设、评价维度、时间边界和停止条件，避免无目标搜集资料。
2. 优先使用一手、官方和近期来源；记录标题、作者、日期、链接和访问时间，区分事实、推断与观点。
3. 对关键结论进行交叉验证，主动寻找反例和冲突证据；样本、方法或来源存在偏差时必须说明。
4. 输出应包含方法、证据表、核心发现、置信度、限制、建议和待验证问题，不用来源数量冒充研究质量。
5. 不得伪造引用、数据或访谈；敏感信息需脱敏并遵守授权范围。"#;

pub(super) const DATA_ANALYSIS_RULES: &str = r#"## 数据分析固定执行规则

1. 先定义业务问题、指标口径、粒度、时间窗口、数据来源和验收标准；任何口径变化必须显式记录。
2. 原始数据只读保存，清洗与转换可复现；记录缺失、异常、重复、泄漏、偏差和采样处理。
3. 分析代码、查询和环境必须可运行；关键结果用独立方法或抽样复核，图表必须包含单位、范围和来源。
4. 区分相关与因果，报告不确定性、统计限制和可能的替代解释，不夸大结论。
5. 交付数据字典、方法、结果、可复现步骤和决策建议；涉及隐私与敏感数据时执行最小化访问和脱敏。"#;

pub(super) const PRODUCT_DESIGN_RULES: &str = r#"## 产品与设计固定执行规则

1. 先明确用户、场景、问题、约束、信息架构和成功指标，使用证据而不是个人偏好定义方案。
2. 从用户流程和低保真 SVG 线框开始，再进入视觉设计；覆盖空、加载、错误、权限、极端内容和响应式状态。
3. 复用并维护设计 Token、组件、间距、排版和交互模式；保证键盘操作、对比度、语义结构和可访问性。
4. 关键方案提供取舍依据并进行可用性检查；实现后必须对照设计进行视觉与交互验收。
5. 交付源文件、规格、状态说明、资产清单和实现注意事项，不能只提供截图。"#;

pub(super) const MARKETING_CONTENT_RULES: &str = r#"## 市场与内容固定执行规则

1. 先定义受众、定位、渠道、行动目标、核心信息、品牌语气、预算/时间和衡量指标。
2. 主张、价格、案例和效果数据必须有依据；不得制造虚假稀缺、伪造背书或隐瞒重要限制。
3. 按渠道设计内容矩阵、素材规格、发布节奏和实验变量，确保同一活动信息一致且可追踪。
4. 上线前检查品牌、法务、链接、埋点、移动端展示和无障碍；上线后基于数据复盘，不以曝光量代替业务效果。
5. 保存已批准文案和素材版本，敏感或不可逆发布必须获得 Human 确认。"#;

pub(super) const DOCUMENTATION_RULES: &str = r#"## 文档与知识库固定执行规则

1. 先确认读者、任务、前置知识、支持版本和信息架构；文档必须帮助读者完成具体目标。
2. 示例、命令、接口和截图必须与当前产品一致并实际验证；危险操作提供备份、回滚和结果检查。
3. 采用一致术语、标题层级、链接和代码格式；内容按主题拆分文件，避免单文件无限膨胀。
4. 标明适用版本、更新时间、负责人和已知限制；代码变化时同步更新文档并检查失效链接。
5. API/运维文档需覆盖认证、错误、限流、安全和故障恢复，不能只写成功路径。"#;

pub(super) const AUTOMATION_RULES: &str = r#"## 自动化与 Agent 固定执行规则

1. 先描述触发条件、输入、输出、权限、幂等性、重试、超时、取消、审计和 Human 接管点。
2. 自动化默认最小权限；密钥使用安全存储，不进入提示词、日志或仓库。高风险和不可逆动作必须设置审批。
3. 状态机、失败恢复和重复执行行为必须明确；外部依赖使用退避、限流和熔断，不能无限循环或静默吞错。
4. 测试覆盖正常、重复、并发、部分失败、超时和恢复；提供 dry-run 或沙箱路径验证实际效果。
5. 运行中提供可观察的进度、结构化日志和告警；交付包含部署、停用、回滚和数据清理说明。"#;

pub(super) const OPERATIONS_RULES: &str = r#"## 运营与交付固定执行规则

1. 先建立目标、范围、负责人、时间线、依赖、检查清单、风险和升级路径。
2. 上线、迁移和批量操作必须有备份、演练、分阶段执行、验收指标和可执行回滚方案。
3. 重要动作保留审批与审计记录；涉及客户、生产数据或对外发布时不得擅自扩大范围。
4. 执行中持续记录状态、偏差、阻塞和下一步；异常优先止损，再定位根因，禁止隐瞒失败。
5. 完成后进行结果核验、监控观察和复盘，将可复用流程沉淀为 runbook。"#;

pub(super) const GENERAL_PROJECT_RULES: &str = r#"## 通用项目固定执行规则

1. 开始前明确目标、范围、非目标、负责人、依赖、风险和可验证的完成标准。
2. 先检查现有资产和历史决定，再制定最小可行计划；关键歧义及时向 Human 澄清。
3. 将工作拆成可验证的小步骤，保留变更记录，不越过权限或执行未授权的不可逆操作。
4. 结论和交付必须有证据；失败、阻塞与未验证假设如实说明，不能把部分完成标记为完成。
5. 交付时总结结果、验证、遗留风险和后续行动。"#;
