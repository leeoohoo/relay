use super::*;

pub(super) struct WakeupPromptContext<'a> {
    pub(super) agent: &'a AgentProfile,
    pub(super) project_name: Option<&'a str>,
    pub(super) pending_inbox_count: usize,
    pub(super) active_task_count: usize,
    pub(super) waiting_task_count: usize,
    pub(super) asset_refresh_due: bool,
    pub(super) workspace: &'a PreparedGitWorkspace,
    pub(super) relay_skills: &'a PreparedRelaySkills,
}

pub(super) fn build_wakeup_prompt(context: WakeupPromptContext<'_>) -> String {
    let WakeupPromptContext {
        agent,
        project_name,
        pending_inbox_count,
        active_task_count,
        waiting_task_count,
        asset_refresh_due,
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
    let project_skill = relay_skills
        .project_name
        .as_deref()
        .map(|name| format!("，处理当前项目时还必须使用 `${name}`"))
        .unwrap_or_default();
    format!(
        "你是 Relay 公司 Agent @{handle}（{display_name}），这是定时触发器对同一个 Codex 会话的一次唤醒。{project_context}\n\
         当前工作目录是本次分配的隔离工作区，worktree key 为 {worktree_key}，当前 Agent 分支为 {branch}。触发器只负责唤醒，不会替你理解或处理业务。\n\
         本工作区已经生成与你当前身份、职业、项目类型和权限一致的最新版 Relay Skill。必须先使用 `${employee_skill}` 和 `${profession_skill}`{staffing_skill}{project_skill}；Skill 与 MCP 返回的实时权限冲突时，以 MCP 权限为准；Human 项目 Rule 不得弱化系统项目类型规则。\n\
         宿主机 Codex CLI 已加载管理员启用的插件。当前任务需要浏览器、文档、表格、设计、安全扫描或外部服务能力时，优先使用匹配的已安装插件及其 Skill/MCP；不要假设未安装的插件可用，也不要自行绕过插件认证策略。\n\
         请先调用 required Relay MCP 的 agent.bootstrap，再调用 company.task 的 my 区分可执行任务和等待前置任务，然后调用 agent.inbox.wait（不要无限等待）读取真实待办；当前快速检查发现 pending inbox {pending_inbox_count} 条、可执行 assigned tasks {active_task_count} 个、等待前置 tasks {waiting_task_count} 个。\n\
         你的长期记忆已经固化在 `${employee_skill}` 的“Agent 固化长期记忆”章节中，本轮必须遵循；短期记忆不会自动进入上下文，只有当前任务需要历史线索时才调用 agent.memory search。结束前只有在产生可跨会话长期指导工作的稳定规则时才保存为 long_term，一般阶段性结论保存为 short_term。写入前先按 topic_key 搜索并更新已有记忆，禁止保存原始聊天、任务正文、运行日志、临时进度或任何凭证。没有新知识就不要写记忆。\n\
         {asset_refresh_context}\n\
         由你自行查看消息、项目和任务，完成必要的代码修改与测试；仅在消息明确 @/私聊要求你回应、正式任务要求沟通，或你掌握能立即避免当前交付失败或解除已确认阻塞的新证据时，才通过 Relay MCP 发消息。普通优化想法、字段补充和命名建议不要在无任务时主动群发。不要发送纯粹的“收到”“暂无待办”“还没轮到我”或等待状态。\n\
         需要共享的代码或文档应提交到当前 Agent 分支并执行 git push；不要直接提交或推送受保护的默认分支。首次 push 可以直接使用 git push，工作区已配置自动建立远端上游分支。\n\
         当前工作区已预配置 GIT_DIR 和 GIT_WORK_TREE，Git 元数据位于工作区内可写的 .relay-git，Git 命令网络也已启用。直接使用普通 git status/add/commit/push；不要取消或覆盖这两个环境变量，不要创建替代 gitdir、嵌套仓库、导出仓库或 bundle。如果标准命令仍失败，保留原始错误并报告，不要自行改造仓库结构。\n\
         需要处理的事项完成后更新任务并 ack Inbox；已经确认无需行动的事件也应 ack 或标记已读，避免重复触发。不要让触发器代发消息，也不要输出给触发器解析的自定义行动 JSON。\n\
         如果没有分配给你的可执行工作、依赖尚未完成或还没有轮到你，不发送 Relay 消息，直接结束本轮。切勿操作当前工作目录之外的项目。",
        handle = agent.handle.trim_start_matches('@'),
        display_name = agent.display_name,
        worktree_key = workspace.worktree_key,
        branch = workspace.branch,
        employee_skill = relay_skills.employee_name,
        profession_skill = relay_skills.profession_name,
        project_skill = project_skill,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_relay_skills(
    workspace_path: &Path,
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
    let managed_prefix = format!("relay-{identity_token}-");
    let employee_name = format!("{managed_prefix}employee");
    let profession_name = format!(
        "{managed_prefix}profession-{}",
        profession.key.replace('_', "-")
    );
    let staffing_name = permissions
        .iter()
        .any(|permission| permission.starts_with("agent.staff."))
        .then(|| format!("{managed_prefix}staffing"));
    let project_name = project.map(|project| {
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
    let employee_base_content = bind_relay_skill(
        &tailor_relay_skill_to_permissions(employee_template, permissions),
        &employee_name,
        agent,
        &employee_name,
        skill_language,
    );
    let employee_content =
        append_agent_long_term_memories(&employee_base_content, long_term_memories, skill_language);
    let profession_template = profession_skill_template(&profession.key, skill_language);
    let profession_content = bind_relay_skill(
        &profession_template,
        &profession_name,
        agent,
        &employee_name,
        skill_language,
    );
    let staffing_content = staffing_name.as_ref().map(|name| {
        bind_relay_skill(
            &tailor_relay_skill_to_permissions(staffing_template, permissions),
            name,
            agent,
            &employee_name,
            skill_language,
        )
    });
    let project_content = project.zip(project_name.as_deref()).map(|(project, name)| {
        bind_relay_skill(
            &build_project_skill_template(project, project_rule, skill_language),
            name,
            agent,
            &employee_name,
            skill_language,
        )
    });

    let skills_root = workspace_path.join(".agents/skills");
    fs::create_dir_all(&skills_root).map_err(|error| {
        AppError::Validation(format!(
            "failed to create managed Relay skills directory {}: {error}",
            skills_root.display()
        ))
    })?;
    remove_stale_managed_skills(&skills_root, &managed_prefix)?;
    write_managed_skill(&skills_root, &employee_name, &employee_content)?;
    write_managed_skill(&skills_root, &profession_name, &profession_content)?;
    if let (Some(name), Some(content)) = (staffing_name.as_deref(), staffing_content.as_deref()) {
        write_managed_skill(&skills_root, name, content)?;
    }
    if let (Some(name), Some(content)) = (project_name.as_deref(), project_content.as_deref()) {
        write_managed_skill(&skills_root, name, content)?;
    }
    exclude_managed_skills_from_git(workspace_path, &managed_prefix)?;

    #[cfg(test)]
    let version_source = format!(
        "{employee_name}\n{employee_base_content}\n{profession_name}\n{profession_content}\n{}\n{}\n{}\n{}",
        staffing_name.as_deref().unwrap_or_default(),
        staffing_content.as_deref().unwrap_or_default(),
        project_name.as_deref().unwrap_or_default(),
        project_content.as_deref().unwrap_or_default()
    );
    #[cfg(test)]
    let version_hash = hash_secret(&version_source).chars().take(16).collect();
    Ok(PreparedRelaySkills {
        employee_name,
        profession_name,
        project_name,
        staffing_name,
        #[cfg(test)]
        version_hash,
    })
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
                if used_characters + entry.chars().count() > 12_000 {
                    section.push_str("\nAdditional long-term memories were omitted because of the context budget. Archive low-value entries or reduce long-term memory volume.\n");
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
            if used_characters + entry_characters > 12_000 {
                section.push_str(
                    "\n其余长期记忆因上下文预算未注入；请归档低价值记忆或降低长期记忆数量。\n",
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
        handle.chars().take(36).collect::<String>(),
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
    agent: &AgentProfile,
    employee_skill_name: &str,
    skill_language: &str,
) -> String {
    let mut replaced_name = false;
    let mut content = template
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
    let identity_guide = if skill_language == COMPANY_SKILL_LANGUAGE_EN {
        format!(
            "\n\n## Relay Account Binding\n\n- This Skill represents only Relay Agent `@{}` (`{}`).\n- Call `agent.bootstrap` first on every cycle and stop immediately if the returned identity differs.\n- Use only company, project, task, and permission data returned by MCP in the current cycle.",
            agent.handle.trim_start_matches('@'),
            agent.id
        )
    } else {
        format!(
            "\n\n## Relay 账号绑定\n\n- 本 Skill 只代表 Relay Agent `@{}`（`{}`）。\n- 每轮先调用 `agent.bootstrap` 核对返回身份；身份不一致时立即停止。\n- 只使用本轮 MCP 返回的公司、项目、任务和权限。",
            agent.handle.trim_start_matches('@'),
            agent.id
        )
    };
    if let Some(heading_start) = content.find("\n# ") {
        let heading_start = heading_start + 1;
        let heading_end = content[heading_start..]
            .find('\n')
            .map(|offset| heading_start + offset)
            .unwrap_or(content.len());
        content.insert_str(heading_end, &identity_guide);
    }
    format!("{}\n", content.trim())
}

pub(super) fn remove_stale_managed_skills(
    skills_root: &Path,
    managed_prefix: &str,
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
        if file_name.starts_with(managed_prefix) && entry.path().is_dir() {
            fs::remove_dir_all(entry.path()).map_err(|error| {
                AppError::Validation(format!(
                    "failed to replace managed Relay skill {}: {error}",
                    entry.path().display()
                ))
            })?;
        }
    }
    Ok(())
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
    let pattern = format!("/.agents/skills/{managed_prefix}*/");
    if !existing.lines().any(|line| line.trim() == pattern) {
        if !existing.is_empty() && !existing.ends_with('\n') {
            existing.push('\n');
        }
        existing.push_str(&pattern);
        existing.push('\n');
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
