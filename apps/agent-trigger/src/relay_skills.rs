use super::*;
use ai_chat_application::AgentControlSnapshot;
use serde_json::json;

pub(super) struct WakeupPromptContext<'a> {
    pub(super) agent: &'a AgentProfile,
    pub(super) job_title: &'a str,
    pub(super) project_name: Option<&'a str>,
    pub(super) pending_inbox_count: usize,
    pub(super) active_task_count: usize,
    pub(super) waiting_task_count: usize,
    pub(super) asset_refresh_due: bool,
    pub(super) control_snapshot: &'a AgentControlSnapshot,
    pub(super) workspace: &'a PreparedGitWorkspace,
    pub(super) relay_skills: &'a PreparedRelaySkills,
}

pub(super) struct WorkerPromptContext<'a> {
    pub(super) agent: &'a AgentProfile,
    pub(super) job_title: &'a str,
    pub(super) project: &'a CompanyProject,
    pub(super) intent: &'a AgentExecutionIntent,
    pub(super) workspace: &'a PreparedGitWorkspace,
    pub(super) relay_skills: &'a PreparedRelaySkills,
    pub(super) previous_checkpoint: Option<&'a str>,
}

pub(super) fn build_wakeup_prompt(context: WakeupPromptContext<'_>) -> String {
    let WakeupPromptContext {
        agent,
        job_title,
        project_name,
        pending_inbox_count,
        active_task_count,
        waiting_task_count,
        asset_refresh_due,
        control_snapshot,
        workspace,
        relay_skills,
    } = context;
    let project_context = project_name
        .map(|name| format!("当前路由到项目：{name}。"))
        .unwrap_or_else(|| "本次没有绑定代码项目，优先处理 Relay 消息和任务协调。".into());
    let asset_refresh_context = if asset_refresh_due {
        "本轮由项目资产定期维护触发。请先读取 company.project get 返回的 Rule 和现有 assets，扫描当前项目工作区中的实际代码、文档、配置、接口、数据文件等可复用资产，然后调用 company.project 的 assets_replace 完整替换资产清单；即使没有变化也要调用一次，以完成本轮刷新记录。不要为资产无变化发送聊天占位消息。"
    } else {
        "本轮没有到期的项目资产维护任务。"
    };
    let staffing_skill = relay_skills
        .staffing_name
        .as_deref()
        .map(|name| format!("，并在涉及人员管理时同时使用 `${name}`"))
        .unwrap_or_default();
    let snapshot = render_control_snapshot(control_snapshot);
    format!(
        "你是 Relay 公司 Agent @{handle}（{display_name}），岗位为 {job_title}。Relay 已通过本轮专属 run token 固定并认证此身份，这是控制会话的一次有效唤醒。{project_context}\n\
         不要向 Human、同事或其他工具重新询问或确认“我是谁”；不要把身份核对作为工作步骤或状态汇报。`agent.bootstrap` 只用于刷新公司、权限、会话和工作状态，不用于协商身份；若 MCP 返回未认证或身份绑定错误，将其视为运行环境故障并停止本轮。\n\
         当前工作目录是专属于本 Agent 的 Relay 控制工作区，worktree key 为 {worktree_key}。这里用于消息分诊、协调和派工，不是项目代码工作区。\n\
         必须先使用 `${employee_skill}`、`${profession_skill}` 和 `${session_skill}`{staffing_skill}；职业执行 Skill 在控制会话中同样生效，用于判断职责、拆解、质量要求和是否需要启动项目工作。Skill 与 MCP 返回的实时权限冲突时，以 MCP 权限为准。\n\
         宿主机 Codex CLI 已加载管理员启用的插件。当前任务需要浏览器、文档、表格、设计、安全扫描或外部服务能力时，优先使用匹配的已安装插件及其 Skill/MCP；不要假设未安装的插件可用，也不要自行绕过插件认证策略。\n\
         Relay 已在启动前生成本轮一次性 Control Snapshot，版本为 `{snapshot_version}`。它已经包含属于你的未读消息、可行动事件、Ready/Waiting 任务、活动 Intent 和项目工作会话；不要重复调用 agent.bootstrap、company.task my 或 agent.inbox.wait。只有操作返回 stale/conflict，或本轮明确改变了相关状态后仍需继续决策时，才调用 agent.control_snapshot 刷新一次。Trigger 托管控制会话禁止长轮询，处理完当前快照后立即结束。当前快照统计：unread messages {unread_message_count} 条、actionable inbox {pending_inbox_count} 条、Ready tasks {active_task_count} 个、Waiting tasks {waiting_task_count} 个。\n\
         当前 Control Snapshot：{snapshot}\n\
         你的 Agent 核心与控制长期记忆已经固化在 `${employee_skill}` 中；短期记忆只在需要历史线索时通过 agent.memory search 查询。控制会话不得读取或固化其他项目的实现细节。\n\
         {asset_refresh_context} 如果它或其他事项需要项目执行，调用 agent.work_session 的 dispatch 创建结构化 Intent；项目工作会话由 Relay 按 Agent + Project 绑定解析。不要在控制工作区修改代码、运行项目测试、提交 Git，也不要自行选择 Thread ID。\n\
         处理消息时必须先阅读 `unread_messages`：如果一条 @、私聊或可行动消息属于某个会话，先按时间顺序理解该会话内更早的全部未读消息，不能只按最后一条 @ 判断需求。快照最多直接展示前 50 条；`unread_messages_truncated=true` 时，用 `company.chat unread` 按会话分页继续读取。每页都要检查 `remaining_has_mentions` 和 `remaining_mention_count`。Human 私聊必须给出实质回复后才能 ack：说明你理解的请求、当前处理结果或明确下一步；如果需要派发项目工作，先回复 Human 再 dispatch。不得用纯粹的“收到”敷衍。其他群消息仅在明确 @、正式任务要求沟通，或你掌握能避免交付失败的新证据时发送消息。\n\
         已经处理或确认无需行动的事件应 ack；派发给工作会话的事件可以在成功创建 Intent 后 ack。处理完某个群会话在本轮快照中的未读上下文后，调用 `company.chat mark_read` 标记该会话已读。如果分页结果 `can_quick_mark_read=true`，可以传 `only_if_no_mentions=true` 和本页 `next_cursor` 作为 `reviewed_through_message_id` 快速清理余下无 @ 消息；后端发现后续仍有 @ 时会拒绝。Relay 会保护本轮启动后新到达的消息，不会被旧一轮误清除。不要输出给 Trigger 解析的自定义 JSON，派工只能使用 agent.work_session。\n\
         如果没有分配给你的可执行工作、依赖尚未完成或还没有轮到你，不发送 Relay 消息，直接结束本轮。切勿操作当前工作目录之外的项目。",
        handle = agent.handle.trim_start_matches('@'),
        display_name = agent.display_name,
        job_title = job_title,
        worktree_key = workspace.worktree_key,
        employee_skill = relay_skills.employee_name,
        profession_skill = relay_skills.profession_name,
        session_skill = relay_skills.session_name,
        snapshot_version = control_snapshot.snapshot_version,
        unread_message_count = control_snapshot.unread_messages.len(),
    )
}

fn render_control_snapshot(snapshot: &AgentControlSnapshot) -> String {
    let unread_messages = snapshot
        .unread_messages
        .iter()
        .take(50)
        .map(|event| {
            json!({
                "event_id": event.id,
                "created_at": event.created_at,
                "conversation_id": event.payload_json.get("conversation_id"),
                "message_id": event.payload_json.get("message_id"),
                "sender_agent_id": event.payload_json.get("sender_agent_id"),
                "sender_human_user_id": event.payload_json.get("sender_human_user_id"),
                "content": event.payload_json.get("content"),
                "mentioned": event.payload_json.get("mentioned"),
                "requires_action": event.requires_action,
            })
        })
        .collect::<Vec<_>>();
    let actionable_events = snapshot
        .actionable_events
        .iter()
        .take(20)
        .map(|event| {
            json!({
                "id": event.id,
                "type": event.event_type,
                "class": event.event_class,
                "priority": event.priority,
                "payload": event.payload_json,
            })
        })
        .collect::<Vec<_>>();
    let ready_tasks = snapshot
        .ready_tasks
        .iter()
        .take(20)
        .map(|task| {
            json!({
                "id": task.id,
                "project_id": task.project_id,
                "title": task.title,
                "status": task.status,
                "priority": task.priority,
            })
        })
        .collect::<Vec<_>>();
    let waiting_tasks = snapshot
        .waiting_tasks
        .iter()
        .take(20)
        .map(|task| {
            json!({
                "id": task.id,
                "project_id": task.project_id,
                "title": task.title,
                "status": task.status,
            })
        })
        .collect::<Vec<_>>();
    let active_intents = snapshot
        .active_intents
        .iter()
        .take(10)
        .map(|intent| {
            json!({
                "id": intent.id,
                "project_id": intent.project_id,
                "status": intent.status,
                "objective": intent.objective,
                "task_ids": intent.task_ids,
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&json!({
        "unread_messages_total": snapshot.unread_messages.len(),
        "unread_messages_truncated": snapshot.unread_messages.len() > 50,
        "unread_messages": unread_messages,
        "actionable_events": actionable_events,
        "ready_tasks": ready_tasks,
        "waiting_tasks": waiting_tasks,
        "active_intents": active_intents,
        "work_sessions": snapshot.work_sessions.iter().take(10).map(|session| json!({
            "id": session.id,
            "kind": session.session_kind,
            "project_id": session.project_id,
            "status": session.status,
            "checkpoint": session.summary_short,
        })).collect::<Vec<_>>(),
    }))
    .unwrap_or_else(|_| "{}".into())
}

pub(super) fn build_worker_prompt(context: WorkerPromptContext<'_>) -> String {
    let criteria = if context.intent.acceptance_criteria.is_empty() {
        "- 按项目 Rule 和职业 Skill 完成可验证交付".into()
    } else {
        context
            .intent
            .acceptance_criteria
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let task_ids = context
        .intent
        .task_ids
        .iter()
        .map(Uuid::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let event_ids = context
        .intent
        .source_event_ids
        .iter()
        .map(Uuid::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let project_skill = context
        .relay_skills
        .project_name
        .as_deref()
        .unwrap_or("relay-project-context");
    let checkpoint = context
        .previous_checkpoint
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("上一代会话 checkpoint：\n{value}\n"))
        .unwrap_or_default();
    format!(
        "你是 Relay 公司 Agent @{handle}（{display_name}），岗位为 {job_title}。Relay 已通过本轮专属 run token 固定并认证此身份，本轮已进入项目 `{project_name}` 的独立工作会话。\n\
         不要重新确认、询问或汇报自己的身份，也不要为了身份调用 `agent.bootstrap`；认证异常应作为运行环境故障直接停止。\n\
         当前工作目录是该 Agent 在本项目的隔离工作区，worktree key 为 {worktree_key}，分支为 {branch}。\n\
         必须使用 `${employee_skill}`、`${profession_skill}`、`${session_skill}` 和 `${project_skill}`。职业 Skill 与项目 Rule 的流程和质量门槛不能省略。\n\
         本轮 Execution Intent ID：{intent_id}\n\
         目标：{objective}\n\
         优先级：{priority}\n\
         关联 Task IDs：{task_ids}\n\
         来源 Event IDs：{event_ids}\n\
         验收标准：\n{criteria}\n\
         {checkpoint}\
         直接用 company.project get 和 company.task get/list 核实当前项目与任务实时状态。只处理这个项目和本 Intent，不要重新处理控制会话的其他消息。\n\
         项目工作会话不承担 Inbox 分诊：忽略 Relay 工具响应中的 inbox_notice，不调用 agent.inbox.wait/ack，不因群聊、私聊或新事件中断当前 Intent。通信事件统一留给本 Agent 的控制会话；只有本 Intent 明确要求的最终项目同步可以在交付收口时发送一次。\n\
         完成必要的设计、实现、测试、文档和 Git 提交推送；不要直接写受保护默认分支。更新关联任务与项目状态。\n\
         长期记忆只保存稳定知识：跨项目通用内容使用 agent scope，当前项目特有内容使用 project scope 并带 project_id；阶段性线索使用 short_term。禁止保存聊天原文、任务正文、日志和凭证。\n\
         最终回复必须简洁列出：已完成、验证、未完成/阻塞、下一步、分支和 Commit。",
        handle = context.agent.handle.trim_start_matches('@'),
        display_name = context.agent.display_name,
        job_title = context.job_title,
        project_name = context.project.name,
        worktree_key = context.workspace.worktree_key,
        branch = context.workspace.branch,
        employee_skill = context.relay_skills.employee_name,
        profession_skill = context.relay_skills.profession_name,
        session_skill = context.relay_skills.session_name,
        project_skill = project_skill,
        intent_id = context.intent.id,
        objective = context.intent.objective,
        priority = context.intent.priority,
        checkpoint = checkpoint,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_relay_skills(
    workspace_path: &Path,
    bundle_kind: &str,
    agent: &AgentProfile,
    job_title: &str,
    permissions: &[String],
    long_term_memories: &[AgentMemory],
    project: Option<&CompanyProject>,
    project_rule: Option<&CompanyProjectRule>,
    skill_language: &str,
) -> AppResult<PreparedRelaySkills> {
    let profession = infer_company_profession(Some(job_title));
    let identity_token = relay_skill_identity_token(agent);
    let agent_id_token = agent
        .id
        .to_string()
        .replace('-', "")
        .chars()
        .take(8)
        .collect::<String>();
    let managed_prefix = format!("relay-{identity_token}-");
    let employee_name = format!("{managed_prefix}employee");
    let profession_name = format!(
        "{managed_prefix}profession-{}",
        profession.key.replace('_', "-")
    );
    let session_name = format!("{managed_prefix}{bundle_kind}");
    let is_control = bundle_kind == RELAY_SKILL_BUNDLE_CONTROL;
    let staffing_name = is_control
        .then_some(())
        .filter(|_| {
            permissions
                .iter()
                .any(|permission| permission.starts_with("agent.staff."))
        })
        .map(|_| format!("{managed_prefix}staffing"));
    let project_name = (!is_control).then_some(project).flatten().map(|project| {
        format!(
            "{managed_prefix}project-{}",
            project
                .id
                .to_string()
                .replace('-', "")
                .chars()
                .take(8)
                .collect::<String>()
        )
    });

    let employee_template = if skill_language == COMPANY_SKILL_LANGUAGE_EN {
        EMPLOYEE_SKILL_TEMPLATE_EN
    } else {
        EMPLOYEE_SKILL_TEMPLATE
    };
    let staffing_template = if skill_language == COMPANY_SKILL_LANGUAGE_EN {
        STAFFING_SKILL_TEMPLATE_EN
    } else {
        STAFFING_SKILL_TEMPLATE
    };
    let employee_base_content = append_agent_identity_card(
        &bind_relay_skill(
            &tailor_relay_skill_to_permissions(employee_template, permissions),
            &employee_name,
            &employee_name,
        ),
        agent,
        job_title,
        &profession.key,
        skill_language,
    );
    let employee_content =
        append_agent_long_term_memories(&employee_base_content, long_term_memories, skill_language);
    let profession_template = profession_skill_template(&profession.key, skill_language);
    let profession_content =
        bind_relay_skill(&profession_template, &profession_name, &employee_name);
    let session_content = bind_relay_skill(
        &session_skill_template(bundle_kind, skill_language),
        &session_name,
        &employee_name,
    );
    let staffing_content = staffing_name.as_ref().map(|name| {
        bind_relay_skill(
            &tailor_relay_skill_to_permissions(staffing_template, permissions),
            name,
            &employee_name,
        )
    });
    let project_content = project.zip(project_name.as_deref()).map(|(project, name)| {
        bind_relay_skill(
            &build_project_skill_template(project, project_rule, skill_language),
            name,
            &employee_name,
        )
    });

    let skills_root = workspace_path.join(".agents/skills");
    fs::create_dir_all(&skills_root).map_err(|error| {
        AppError::Validation(format!(
            "failed to create managed Relay skills directory {}: {error}",
            skills_root.display()
        ))
    })?;
    let runtime_skills_root = managed_runtime_skills_root(workspace_path, &identity_token)?;
    fs::create_dir_all(&runtime_skills_root).map_err(|error| {
        AppError::Validation(format!(
            "failed to create external Relay skill runtime {}: {error}",
            runtime_skills_root.display()
        ))
    })?;
    remove_stale_managed_skills(&skills_root, &managed_prefix, &agent_id_token)?;
    remove_stale_managed_skills(&runtime_skills_root, &managed_prefix, &agent_id_token)?;
    write_and_link_managed_skill(
        &runtime_skills_root,
        &skills_root,
        &employee_name,
        &employee_content,
    )?;
    write_and_link_managed_skill(
        &runtime_skills_root,
        &skills_root,
        &profession_name,
        &profession_content,
    )?;
    write_and_link_managed_skill(
        &runtime_skills_root,
        &skills_root,
        &session_name,
        &session_content,
    )?;
    if let (Some(name), Some(content)) = (staffing_name.as_deref(), staffing_content.as_deref()) {
        write_and_link_managed_skill(&runtime_skills_root, &skills_root, name, content)?;
    }
    if let (Some(name), Some(content)) = (project_name.as_deref(), project_content.as_deref()) {
        write_and_link_managed_skill(&runtime_skills_root, &skills_root, name, content)?;
    }
    exclude_managed_skills_from_git(workspace_path, &managed_prefix)?;

    let version_source = format!(
        "{employee_name}\n{employee_base_content}\n{profession_name}\n{profession_content}\n{session_name}\n{session_content}\n{}\n{}\n{}\n{}",
        staffing_name.as_deref().unwrap_or_default(),
        staffing_content.as_deref().unwrap_or_default(),
        project_name.as_deref().unwrap_or_default(),
        project_content.as_deref().unwrap_or_default()
    );
    let version_hash = hash_secret(&version_source).chars().take(16).collect();
    Ok(PreparedRelaySkills {
        employee_name,
        profession_name,
        session_name,
        project_name,
        staffing_name,
        version_hash,
    })
}

pub(super) fn session_skill_template(bundle_kind: &str, skill_language: &str) -> String {
    let english = skill_language == COMPANY_SKILL_LANGUAGE_EN;
    if bundle_kind == RELAY_SKILL_BUNDLE_CONTROL {
        if english {
            return "---\nname: relay-control-session\ndescription: Mandatory Relay control-plane workflow for inbox triage, communication, task coordination, work-session selection, and structured project dispatch. Use on every control-session wake.\n---\n\n# Relay Control Session\n\n- Inspect inbox, assigned tasks, project hints, and the work-session directory before deciding.\n- Apply the profession Skill when judging ownership, decomposition, quality expectations, and whether project execution is required.\n- Handle communication and coordination here. Do not edit project files or run project delivery work from the control workspace.\n- Use the Relay-managed `$TMPDIR` for temporary scripts and scratch data. Never address `/tmp`, `/private/tmp`, or host-specific temporary paths directly.\n- Start project work only through `agent.work_session` with action `dispatch`. Supply a project ID, concise objective, acceptance criteria, relevant task IDs, and source event IDs. Use `replace_session: true` only for stale permissions or unrecoverable session state.\n- Existing sessions are selected by project binding; never invent or pass a Codex thread ID.\n- If no project execution is needed, reply or acknowledge the event and finish without dispatching.\n- Agent and control long-term memories may guide routing. Project memory belongs to the selected worker session.\n".into();
        }
        return "---\nname: relay-control-session\ndescription: Relay 控制会话的强制工作流，用于 Inbox 分诊、通信、任务协调、工作会话选择和结构化项目派工。每次控制会话唤醒都必须使用。\n---\n\n# Relay 控制会话\n\n- 决策前检查 Inbox、分配任务、项目提示和工作会话目录。\n- 判断职责归属、任务拆解、质量要求和是否需要项目执行时，必须同时遵循职业 Skill。\n- 通信和协调在本会话完成；不得在控制工作区修改项目文件或执行项目交付。\n- 临时脚本和临时数据必须使用 Relay 托管的 `$TMPDIR`；禁止直接使用 `/tmp`、`/private/tmp` 或宿主机特定临时路径。\n- 只有确实需要项目工作时，才调用 `agent.work_session` 的 `dispatch`，提供项目 ID、精简目标、验收标准、关联任务 ID 和来源事件 ID；只有权限缓存过期或会话内部状态不可恢复时才使用 `replace_session: true`。\n- 会话由项目绑定解析，禁止自行编造或传递 Codex Thread ID。\n- 不需要项目执行时，直接回复或 Ack 后结束，不得创建占位派工。\n- Agent 与控制长期记忆可以指导路由；项目记忆只属于被选中的工作会话。\n".into();
    }
    if english {
        return "---\nname: relay-project-worker\ndescription: Mandatory Relay project-worker workflow for executing one structured intent in the project-bound workspace, validating the result, committing delivery, and updating Relay state. Use on every project worker turn.\n---\n\n# Relay Project Worker\n\n- Execute only the supplied project-bound intent and verify the live project, tasks, and Rule through Relay MCP.\n- Follow the profession Skill and project Skill throughout implementation.\n- This worker session never triages Inbox events. Ignore `inbox_notice`, do not call `agent.inbox.wait` or `agent.inbox.ack`, and leave chat/event handling to the Agent's control session. Only send one final project update when the current Intent explicitly requires it.\n- Use only the current project workspace; never inspect another project workspace.\n- Use the Relay-managed `$TMPDIR` for temporary scripts and scratch data. Never address `/tmp`, `/private/tmp`, or host-specific temporary paths directly.\n- Repository-wide format/lint commands must target tracked product paths or explicitly exclude `.agents/` and `.relay-runtime-skills/`; never mutate Relay runtime Skill files.\n- For browser automation or Web validation, use the Relay-managed `chrome-devtools` MCP tools. Do not use Codex desktop Browser/Chrome, Computer Use, or raw CDP as a fallback. Navigation to an explicit website URL, new pages with a target URL, and file uploads must continue through Relay's approval center; reload, back, forward, and blank-page operations do not require Human approval.\n- When `take_screenshot` or `take_snapshot` must save evidence, its `filePath` must be a relative path below `.relay/browser-artifacts/` (for example `.relay/browser-artifacts/login.png`). The browser container cannot write anywhere else in the project. After the tool succeeds, use the Shell tool to copy the artifact into the intended tracked project path. Do not retry project absolute paths, `/docs`, or container `/tmp`.\n- Complete the required design, implementation, validation, documentation, and Git delivery steps.\n- Update tasks and project status with verified results. For fixed Relay MCP fields, use only values exposed by the tool schema; if validation fails, read the returned allowed values and correct the input instead of repeating the same call or inventing another value. Save project-scoped memory only for durable project knowledge.\n- Finish with a concise checkpoint: completed work, pending work, blockers, next steps, branch, and commit.\n".into();
    }
    "---\nname: relay-project-worker\ndescription: Relay 项目工作会话的强制执行流程，用于在项目绑定工作区完成一个结构化 Intent、验证结果、提交交付并回写 Relay 状态。每次项目工作会话都必须使用。\n---\n\n# Relay 项目工作会话\n\n- 只执行本轮传入且已绑定当前项目的 Intent，并通过 Relay MCP 核实项目、任务和 Rule 的实时状态。\n- 实施全过程必须遵循职业 Skill 和当前项目 Skill。\n- 工作会话不分诊 Inbox：忽略 `inbox_notice`，不调用 `agent.inbox.wait` 或 `agent.inbox.ack`，群聊、私聊和事件统一留给控制会话；只有当前 Intent 明确要求时，才在交付收口时发送一次最终项目同步。\n- 只能使用当前项目工作区，禁止检查其他项目工作区。\n- 临时脚本和临时数据必须使用 Relay 托管的 `$TMPDIR`；禁止直接使用 `/tmp`、`/private/tmp` 或宿主机特定临时路径。\n- 全仓格式化或检查命令必须限定到已跟踪的业务目录，或明确排除 `.agents/` 与 `.relay-runtime-skills/`；禁止修改 Relay 运行时 Skill 文件。\n- 需要浏览器自动化或 Web 验收时，必须使用 Relay 托管的 `chrome-devtools` MCP；不得改用 Codex 桌面 Browser/Chrome、Computer Use 或 raw CDP 绕过。只有携带明确目标 URL 的网站导航、新建目标页面和文件上传需要进入 Relay 审批中心；刷新、前进、后退和空白页操作无需 Human 审批。\n- `take_screenshot` 或 `take_snapshot` 需要落盘保存证据时，`filePath` 必须使用 `.relay/browser-artifacts/` 下的相对路径（例如 `.relay/browser-artifacts/login.png`）；浏览器容器不能写入项目其他目录。工具成功后，再用 Shell 把证据复制到项目内需要提交的位置。禁止反复尝试项目绝对路径、`/docs` 或容器 `/tmp`。\n- 完成必要的设计、实现、验证、文档和 Git 交付步骤。\n- 使用已验证结果更新任务和项目状态。Relay MCP 的固定字段只能使用工具 Schema 暴露的枚举；遇到 validation error 时必须读取返回的合法值后修正参数，禁止原样重试或继续猜测新值。只有稳定的项目知识才能保存为 project scope 记忆。\n- 结束时提供精简检查点：已完成、未完成、阻塞、下一步、分支和 Commit。\n".into()
}

pub(super) fn build_project_skill_template(
    project: &CompanyProject,
    rule: Option<&CompanyProjectRule>,
    skill_language: &str,
) -> String {
    let definition = company_project_type_by_key(&project.project_type)
        .or_else(|| company_project_type_by_key("general"))
        .expect("general project type must exist");
    let custom_rule = rule
        .map(|rule| rule.content.trim())
        .filter(|content| !content.is_empty())
        .unwrap_or(if skill_language == COMPANY_SKILL_LANGUAGE_EN {
            "No additional Human project Rule is currently configured."
        } else {
            "当前没有 Human 补充 Rule。"
        });
    if skill_language == COMPANY_SKILL_LANGUAGE_EN {
        format!(
            "---\nname: relay-project-context\ndescription: Mandatory system project-type Rules plus additional Human project Rules for the current Relay project. Use for every task in this project.\n---\n\n# Project Skill: {project_name}\n\n## Project Identity\n\n- Project ID: `{project_id}`\n- Project type: {project_type_label} (`{project_type}`)\n- Type source: `{project_type_source}`; inference confidence: {confidence}%\n- System project-type Rules are mandatory. Human Rules may add stricter constraints but cannot remove, weaken, or bypass them.\n\n{system_rules}\n\n## Additional Human Project Rules\n\n{custom_rule}\n",
            project_name = project.name,
            project_id = project.id,
            project_type_label = definition.label_en,
            project_type = project.project_type,
            project_type_source = project.project_type_source,
            confidence = project.project_type_confidence,
            system_rules = definition.rule_markdown_en,
        )
    } else {
        format!(
        "---\nname: relay-project-context\ndescription: Relay 当前项目的系统类型规则与 Human 补充规则。每次处理本项目都必须使用。\n---\n\n# 项目 Skill：{project_name}\n\n## 项目身份\n\n- 项目 ID：`{project_id}`\n- 项目类型：{project_type_label}（`{project_type}`）\n- 类型来源：`{project_type_source}`；识别置信度：{confidence}%\n- 本 Skill 的系统类型规则是强制基线，Human 补充 Rule 只能增加约束，不能删除、弱化或绕过系统规则。\n\n{system_rules}\n\n## Human 项目补充 Rule\n\n{custom_rule}\n",
        project_name = project.name,
        project_id = project.id,
        project_type_label = definition.label,
        project_type = project.project_type,
        project_type_source = project.project_type_source,
        confidence = project.project_type_confidence,
        system_rules = definition.rule_markdown,
        )
    }
}

pub(super) fn append_agent_long_term_memories(
    base_skill: &str,
    memories: &[AgentMemory],
    skill_language: &str,
) -> String {
    let safety_limit = std::env::var("RELAY_MEMORY_INJECTION_SAFETY_LIMIT_CHARS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value >= 4_000)
        .unwrap_or(32_000);
    if skill_language == COMPANY_SKILL_LANGUAGE_EN {
        let mut section = String::from(
            "\n\n## Distilled Long-term Agent Memory\n\nThese entries belong only to the current Agent and are loaded on every Codex wake-up. Use them as durable guidance. If they conflict with the latest Human instruction, project Rule, repository state, or MCP state, follow current verified facts and update the memory after validation.\n",
        );
        if memories.is_empty() {
            section.push_str("\nNo distilled long-term memory is currently stored.\n");
        } else {
            let mut used_characters = section.chars().count();
            for memory in memories {
                let entry = format!(
                    "\n### {}\n\n- Topic key: `{}`\n- Conclusion: {}\n- When to use: {}\n- Importance: {}/5; confidence: {}%{}\n",
                    memory.title,
                    memory.topic_key,
                    memory.summary,
                    if memory.when_to_use.is_empty() { "Any work directly related to this topic" } else { &memory.when_to_use },
                    memory.importance,
                    memory.confidence,
                    if memory.tags.is_empty() { String::new() } else { format!("; tags: {}", memory.tags.join(", ")) }
                );
                if !memory.pinned && used_characters + entry.chars().count() > safety_limit {
                    section.push_str("\nAdditional lower-priority memories were moved to on-demand retrieval because the configurable safety limit was reached. Pinned memories are never omitted by this guard.\n");
                    break;
                }
                used_characters += entry.chars().count();
                section.push_str(&entry);
            }
        }
        return format!("{}{}\n", base_skill.trim(), section.trim_end());
    }
    let mut section = String::from(
        "\n\n## Agent 固化长期记忆\n\n这些内容只属于当前 Agent，并在每次 Codex 唤醒时自动进入本 Skill。它们用于长期指导工作；如果与 Human 最新指令、项目 Rule、当前代码或 MCP 实时状态冲突，以当前事实为准，并在核验后更新记忆。\n",
    );
    if memories.is_empty() {
        section.push_str("\n当前还没有固化长期记忆。\n");
    } else {
        let mut used_characters = section.chars().count();
        for memory in memories {
            let entry = format!(
                "\n### {}\n\n- 主题键：`{}`\n- 结论：{}\n- 使用场景：{}\n- 重要度：{}/5；置信度：{}%{}\n",
                memory.title,
                memory.topic_key,
                memory.summary,
                if memory.when_to_use.is_empty() {
                    "任何与该主题直接相关的工作"
                } else {
                    &memory.when_to_use
                },
                memory.importance,
                memory.confidence,
                if memory.tags.is_empty() {
                    String::new()
                } else {
                    format!("；标签：{}", memory.tags.join("、"))
                }
            );
            let entry_characters = entry.chars().count();
            if !memory.pinned && used_characters + entry_characters > safety_limit {
                section.push_str(
                    "\n其余低优先级记忆因达到可配置安全上限，已转为按需检索；Pinned 记忆不会被该保护规则省略。\n",
                );
                break;
            }
            used_characters += entry_characters;
            section.push_str(&entry);
        }
    }
    format!("{}{}\n", base_skill.trim(), section.trim_end())
}

pub(super) fn relay_skill_identity_token(agent: &AgentProfile) -> String {
    let handle = agent
        .handle
        .trim_start_matches('@')
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let handle = if handle.is_empty() { "agent" } else { &handle };
    let id = agent.id.to_string().replace('-', "");
    format!(
        "{}-{}",
        handle.chars().take(18).collect::<String>(),
        &id[..8]
    )
}

pub(super) fn profession_skill_template(profession_key: &str, skill_language: &str) -> String {
    let profession = company_profession_by_key(profession_key)
        .or_else(|| company_profession_by_key("general_member"))
        .expect("general member profession must exist");
    if skill_language == COMPANY_SKILL_LANGUAGE_EN {
        profession.skill_markdown_en
    } else {
        profession.skill_markdown
    }
}

pub(super) fn tailor_relay_skill_to_permissions(template: &str, permissions: &[String]) -> String {
    let permissions = permissions
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut remaining = template;
    let mut output = String::new();
    const PREFIX: &str = "<!-- relay-permission:";
    while let Some(start) = remaining.find(PREFIX) {
        output.push_str(&remaining[..start]);
        let marker = &remaining[start + PREFIX.len()..];
        let Some(permission_end) = marker.find(":start -->") else {
            output.push_str(&remaining[start..]);
            return output;
        };
        let permission = &marker[..permission_end];
        let block_start = start + PREFIX.len() + permission_end + ":start -->".len();
        let end_marker = format!("<!-- relay-permission:{permission}:end -->");
        let Some(relative_end) = remaining[block_start..].find(&end_marker) else {
            output.push_str(&remaining[start..]);
            return output;
        };
        if permissions.contains(permission) {
            output.push_str(remaining[block_start..block_start + relative_end].trim());
        }
        remaining = &remaining[block_start + relative_end + end_marker.len()..];
    }
    output.push_str(remaining);
    while output.contains("\n\n\n") {
        output = output.replace("\n\n\n", "\n\n");
    }
    format!("{}\n", output.trim())
}

pub(super) fn bind_relay_skill(
    template: &str,
    skill_name: &str,
    employee_skill_name: &str,
) -> String {
    let mut replaced_name = false;
    let content = template
        .lines()
        .map(|line| {
            if !replaced_name && line.starts_with("name:") {
                replaced_name = true;
                format!("name: {skill_name}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .replace("relay-company-employee", employee_skill_name);
    format!("{}\n", content.trim())
}

pub(super) fn append_agent_identity_card(
    content: &str,
    agent: &AgentProfile,
    job_title: &str,
    profession_key: &str,
    skill_language: &str,
) -> String {
    let persona = if agent.persona.trim().is_empty() {
        if skill_language == COMPANY_SKILL_LANGUAGE_EN {
            "Not specified"
        } else {
            "未设置"
        }
    } else {
        agent.persona.trim()
    };
    let identity_card = if skill_language == COMPANY_SKILL_LANGUAGE_EN {
        format!(
            "\n\n## Relay Authenticated Identity\n\n- Display name: `{}`\n- Handle: `@{}`\n- Agent ID: `{}`\n- Job title: `{}`\n- Profession key: `{}`\n- Persona: {}\n- Relay has already bound this identity to the current run token and MCP server. Treat it as a session invariant: do not ask a Human or coworker to confirm it, do not narrate identity checks, and do not call `agent.bootstrap` merely to discover who you are.\n- Use `agent.bootstrap` in the control session only to refresh dynamic company, permission, coworker, session, project, and inbox state. Authentication or binding errors are runtime failures, not identity questions.",
            agent.display_name,
            agent.handle.trim_start_matches('@'),
            agent.id,
            job_title,
            profession_key,
            persona,
        )
    } else {
        format!(
            "\n\n## Relay 已认证身份\n\n- 显示名称：`{}`\n- Handle：`@{}`\n- Agent ID：`{}`\n- 岗位：`{}`\n- 职业键：`{}`\n- Persona：{}\n- Relay 已将此身份绑定到当前 run token 和 MCP Server。它是会话不变量：不得向 Human 或同事再次确认，不得把身份核对写成执行步骤或状态汇报，也不得仅为了知道自己是谁而调用 `agent.bootstrap`。\n- 控制会话调用 `agent.bootstrap` 只为刷新公司、权限、同事、会话、项目和 Inbox 等动态状态。认证或绑定错误属于运行环境故障，不是身份问题。",
            agent.display_name,
            agent.handle.trim_start_matches('@'),
            agent.id,
            job_title,
            profession_key,
            persona,
        )
    };
    let mut output = content.trim().to_string();
    if let Some(heading_start) = output.find("\n# ") {
        let heading_start = heading_start + 1;
        let heading_end = output[heading_start..]
            .find('\n')
            .map(|offset| heading_start + offset)
            .unwrap_or(output.len());
        output.insert_str(heading_end, &identity_card);
    }
    format!("{}\n", output.trim())
}

pub(super) fn remove_stale_managed_skills(
    skills_root: &Path,
    managed_prefix: &str,
    agent_id_token: &str,
) -> AppResult<()> {
    for entry in fs::read_dir(skills_root).map_err(|error| {
        AppError::Validation(format!(
            "failed to inspect managed Relay skills in {}: {error}",
            skills_root.display()
        ))
    })? {
        let entry = entry.map_err(|error| {
            AppError::Validation(format!("failed to inspect managed Relay skill: {error}"))
        })?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let belongs_to_agent = file_name.starts_with("relay-")
            && file_name
                .split('-')
                .any(|component| component == agent_id_token);
        if file_name.starts_with(managed_prefix) || belongs_to_agent {
            remove_managed_skill_path(&entry.path()).map_err(|error| {
                AppError::Validation(format!(
                    "failed to replace managed Relay skill {}: {error}",
                    entry.path().display()
                ))
            })?;
        }
    }
    Ok(())
}

fn managed_runtime_skills_root(workspace_path: &Path, identity_token: &str) -> AppResult<PathBuf> {
    let parent = workspace_path.parent().ok_or_else(|| {
        AppError::Validation("Relay workspace must have a parent directory".into())
    })?;
    let workspace_name = workspace_path.file_name().ok_or_else(|| {
        AppError::Validation("Relay workspace must have a final path component".into())
    })?;
    Ok(parent
        .join(".relay-runtime-skills")
        .join(workspace_name)
        .join(identity_token))
}

fn write_and_link_managed_skill(
    runtime_skills_root: &Path,
    workspace_skills_root: &Path,
    name: &str,
    content: &str,
) -> AppResult<()> {
    write_managed_skill(runtime_skills_root, name, content)?;
    let source = runtime_skills_root.join(name);
    let target = workspace_skills_root.join(name);
    create_directory_link(&source, &target)
}

fn remove_managed_skill_path(path: &Path) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || metadata.is_file() {
        fs::remove_file(path)
    } else {
        fs::remove_dir_all(path)
    }
}

#[cfg(unix)]
fn create_directory_link(source: &Path, target: &Path) -> AppResult<()> {
    if fs::symlink_metadata(target).is_ok() {
        remove_managed_skill_path(target).map_err(|error| {
            AppError::Validation(format!(
                "failed to replace Relay skill link {}: {error}",
                target.display()
            ))
        })?;
    }
    std::os::unix::fs::symlink(source, target).map_err(|error| {
        AppError::Validation(format!(
            "failed to link external Relay skill {}: {error}",
            target.display()
        ))
    })
}

#[cfg(windows)]
fn create_directory_link(source: &Path, target: &Path) -> AppResult<()> {
    if fs::symlink_metadata(target).is_ok() {
        remove_managed_skill_path(target).map_err(|error| {
            AppError::Validation(format!(
                "failed to replace Relay skill link {}: {error}",
                target.display()
            ))
        })?;
    }
    if std::os::windows::fs::symlink_dir(source, target).is_ok() {
        return Ok(());
    }
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(target)
        .arg(source)
        .status()
        .map_err(|error| {
            AppError::Validation(format!(
                "failed to create Relay skill junction {}: {error}",
                target.display()
            ))
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "failed to create Relay skill junction {}",
            target.display()
        )))
    }
}

pub(super) fn write_managed_skill(skills_root: &Path, name: &str, content: &str) -> AppResult<()> {
    let directory = skills_root.join(name);
    fs::create_dir_all(&directory).map_err(|error| {
        AppError::Validation(format!(
            "failed to create managed Relay skill {}: {error}",
            directory.display()
        ))
    })?;
    fs::write(directory.join("SKILL.md"), content).map_err(|error| {
        AppError::Validation(format!(
            "failed to write managed Relay skill {}: {error}",
            directory.display()
        ))
    })
}

pub(super) fn exclude_managed_skills_from_git(
    workspace_path: &Path,
    managed_prefix: &str,
) -> AppResult<()> {
    let info_directory = workspace_path.join(".relay-git/info");
    fs::create_dir_all(&info_directory).map_err(|error| {
        AppError::Validation(format!(
            "failed to prepare Relay Git exclude directory {}: {error}",
            info_directory.display()
        ))
    })?;
    let exclude_path = info_directory.join("exclude");
    let mut existing = fs::read_to_string(&exclude_path).unwrap_or_default();
    let patterns = [
        format!("/.agents/skills/{managed_prefix}*/"),
        "/.relay/browser-artifacts/".into(),
    ];
    let mut changed = false;
    for pattern in patterns {
        if existing.lines().any(|line| line.trim() == pattern) {
            continue;
        }
        if !existing.is_empty() && !existing.ends_with('\n') {
            existing.push('\n');
        }
        existing.push_str(&pattern);
        existing.push('\n');
        changed = true;
    }
    if changed {
        fs::write(&exclude_path, existing).map_err(|error| {
            AppError::Validation(format!(
                "failed to update Relay Git exclude file {}: {error}",
                exclude_path.display()
            ))
        })?;
    }
    Ok(())
}

pub(super) fn bool_env(name: &str, default: bool) -> bool {
    std::env::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

pub(super) fn sanitize_error(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

pub(super) fn truncate(value: &str, max_characters: usize) -> String {
    value.chars().take(max_characters).collect()
}
