use super::*;

#[test]
fn active_company_agents_can_chat_without_friendship() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "company-chat-owner@example.com".into(),
            display_name: "Company Chat Owner".into(),
        })
        .expect("owner should be created");
    let outsider_owner = app
        .dev_login(DevLoginInput {
            email: "company-chat-outsider@example.com".into(),
            display_name: "Company Chat Outsider".into(),
        })
        .expect("outsider owner should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Company Chat".into(),
            slug: Some("company-chat".into()),
            description: None,
        })
        .expect("company should be created");
    let outsider_company = app
        .create_company(CreateCompanyInput {
            human_user_id: outsider_owner.id,
            name: "Other Company".into(),
            slug: Some("other-company-chat".into()),
            description: None,
        })
        .expect("outsider company should be created");
    let alpha = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Alpha Manager".into(),
            handle: "company-chat-alpha".into(),
            persona: "负责协调".into(),
            org_unit_id: None,
            job_title: None,
            role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
            reports_to_membership_id: None,
        })
        .expect("alpha should be created");
    let beta = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Beta Engineer".into(),
            handle: "company-chat-beta".into(),
            persona: "负责工程".into(),
            org_unit_id: None,
            job_title: None,
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: Some(alpha.membership.id),
        })
        .expect("beta should be created");

    let gamma = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Gamma Designer".into(),
            handle: "company-chat-gamma".into(),
            persona: "负责设计".into(),
            org_unit_id: None,
            job_title: None,
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: Some(alpha.membership.id),
        })
        .expect("gamma should be created");
    let outsider = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: outsider_owner.id,
            company_id: outsider_company.company.id,
            display_name: "Outside Agent".into(),
            handle: "company-chat-outsider".into(),
            persona: "另一家公司".into(),
            org_unit_id: None,
            job_title: None,
            role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
            reports_to_membership_id: None,
        })
        .expect("outsider should be created");

    app.repo
        .save_agent_codex_trigger_config(AgentCodexTriggerConfig {
            id: Uuid::new_v4(),
            company_id: company.company.id,
            agent_profile_id: beta.agent_profile.id,
            status: AGENT_CODEX_TRIGGER_STATUS_ACTIVE.into(),
            interval_seconds: 21_600,
            codex_profile: "default".into(),
            model: None,
            reasoning_effort: None,
            reasoning_summary: None,
            verbosity: None,
            personality: None,
            service_tier: None,
            sandbox_mode: AGENT_CODEX_SANDBOX_WORKSPACE_WRITE.into(),
            approval_policy: AGENT_CODEX_APPROVAL_POLICY_NEVER.into(),
            network_access: None,
            web_search: None,
            feature_multi_agent: None,
            feature_remote_plugin: None,
            feature_hooks: None,
            feature_goals: None,
            feature_shell_tool: None,
            max_run_seconds: 1_800,
            next_run_at: now_utc() + Duration::hours(6),
            lease_owner: None,
            lease_expires_at: None,
            manual_run_requested_at: None,
            wake_requested_at: None,
            wake_reason: None,
            last_run_at: None,
            last_success_at: None,
            last_error: None,
            consecutive_failure_count: 0,
            created_by_human_user_id: owner.id,
            updated_by_human_user_id: Some(owner.id),
            created_at: now_utc(),
            updated_at: now_utc(),
        })
        .expect("test should configure beta's Agent Codex trigger");

    let beta_work_profile = app
        .update_company_agent_work_profile(UpdateCompanyAgentWorkProfileInput {
            actor_agent_id: beta.agent_profile.id,
            responsibilities: Some(vec!["实现公司 MCP 接口".into(), "维护消息投递链路".into()]),
            skills: Some(vec!["Rust".into(), "PostgreSQL".into(), "rust".into()]),
            current_focus: Some("完成 Agent 通信闭环".into()),
            collaboration_preference: Some("low_cost_only".into()),
        })
        .expect("agent should update its own structured work profile");
    assert_eq!(
        beta_work_profile.membership.skills,
        vec!["Rust", "PostgreSQL"]
    );
    assert_eq!(
        beta_work_profile.agent_profile.collaboration_preference,
        AGENT_COLLABORATION_PREFERENCE_LOW_COST_ONLY
    );

    let default_group = company
        .conversations
        .first()
        .expect("company should start with a default group");
    assert_eq!(
        default_group.context.context_type,
        CONVERSATION_CONTEXT_COMPANY_ALL
    );
    let initial_context = app
        .get_company_agent_context(GetCompanyAgentContextInput {
            actor_agent_id: alpha.agent_profile.id,
            company_id: company.company.id,
        })
        .expect("company context should include the default group");
    let visible_beta = initial_context
        .agents
        .iter()
        .find(|coworker| coworker.agent_profile.id == beta.agent_profile.id)
        .expect("coworkers should see beta's structured work profile");
    assert_eq!(visible_beta.membership.responsibilities.len(), 2);
    assert_eq!(visible_beta.membership.current_focus, "完成 Agent 通信闭环");
    let default_group = initial_context
        .conversations
        .iter()
        .find(|conversation| conversation.context.context_type == CONVERSATION_CONTEXT_COMPANY_ALL)
        .expect("active agents should be joined to the default company group");
    assert_eq!(default_group.member_agent_ids.len(), 3);
    app.send_company_message(SendCompanyMessageInput {
        actor_agent_id: alpha.agent_profile.id,
        company_id: company.company.id,
        conversation_id: default_group.preview.id,
        content: "欢迎加入公司全员群".into(),
    })
    .expect("company member should send to the default group");
    let beta_unread = app
        .list_company_group_unread_messages(ListCompanyGroupUnreadInput {
            actor_agent_id: beta.agent_profile.id,
            company_id: company.company.id,
            conversation_id: Some(default_group.preview.id),
            message_limit: 20,
        })
        .expect("group member should see unread company group messages");
    assert_eq!(beta_unread.total_unread_count, 1);
    assert_eq!(
        beta_unread.groups[0].unread_messages[0].content,
        "欢迎加入公司全员群"
    );
    let active_cycle_started_at = now_utc();
    let mut beta_trigger = app
        .repo
        .get_agent_codex_trigger_config_by_agent(beta.agent_profile.id)
        .expect("beta trigger should exist");
    beta_trigger.lease_owner = Some("active-test-cycle".into());
    beta_trigger.lease_expires_at = Some(active_cycle_started_at + Duration::minutes(30));
    beta_trigger.last_run_at = Some(active_cycle_started_at);
    app.repo
        .save_agent_codex_trigger_config(beta_trigger)
        .expect("test should simulate an active Codex cycle");
    let arrived_during_cycle = app
        .send_company_message(SendCompanyMessageInput {
            actor_agent_id: alpha.agent_profile.id,
            company_id: company.company.id,
            conversation_id: default_group.preview.id,
            content: "这条消息在 Agent 运行期间到达，不能被批量标记已读。".into(),
        })
        .expect("a message should arrive during the active cycle");
    let read = app
        .mark_company_group_read(MarkCompanyGroupReadInput {
            actor_agent_id: beta.agent_profile.id,
            company_id: company.company.id,
            conversation_id: default_group.preview.id,
        })
        .expect("group member should mark the company group as read");
    assert_eq!(read.marked_read_count, 1);
    let protected_unread = app
        .list_company_group_unread_messages(ListCompanyGroupUnreadInput {
            actor_agent_id: beta.agent_profile.id,
            company_id: company.company.id,
            conversation_id: Some(default_group.preview.id),
            message_limit: 20,
        })
        .expect("messages arriving during a run should remain pending");
    assert_eq!(protected_unread.total_unread_count, 1);
    assert_eq!(
        protected_unread.groups[0].unread_messages[0].id,
        arrived_during_cycle.id
    );
    assert_eq!(
        app.list_company_group_unread_messages(ListCompanyGroupUnreadInput {
            actor_agent_id: gamma.agent_profile.id,
            company_id: company.company.id,
            conversation_id: Some(default_group.preview.id),
            message_limit: 20,
        })
        .expect("one member reading must not clear another member's unread state")
        .total_unread_count,
        2
    );
    let protected_event = app
        .list_agent_inbox_events(beta.agent_profile.id, true, 20)
        .expect("beta inbox should retain the new message")
        .into_iter()
        .find(|event| {
            payload_uuid_field_optional(&event.payload_json, "message_id")
                == Some(arrived_during_cycle.id)
        })
        .expect("the in-run message event should remain pending");
    app.mark_agent_inbox_event_processed(MarkInboxEventProcessedInput {
        actor_agent_id: beta.agent_profile.id,
        event_id: protected_event.id,
    })
    .expect("the Agent should explicitly acknowledge the protected event");
    let mut beta_trigger = app
        .repo
        .get_agent_codex_trigger_config_by_agent(beta.agent_profile.id)
        .expect("beta trigger should exist");
    beta_trigger.lease_owner = None;
    beta_trigger.lease_expires_at = None;
    beta_trigger.wake_requested_at = None;
    beta_trigger.wake_reason = None;
    beta_trigger.next_run_at = now_utc() + Duration::hours(1);
    app.repo
        .save_agent_codex_trigger_config(beta_trigger)
        .expect("test should release the simulated Codex cycle");

    let direct = app
        .open_company_direct_conversation(OpenCompanyDirectConversationInput {
            actor_agent_id: alpha.agent_profile.id,
            company_id: company.company.id,
            target_agent_id: beta.agent_profile.id,
        })
        .expect("coworkers should open direct chat without friendship");
    assert_eq!(
        direct.context.context_type,
        CONVERSATION_CONTEXT_COMPANY_DIRECT
    );
    let replay = app
        .open_company_direct_conversation(OpenCompanyDirectConversationInput {
            actor_agent_id: alpha.agent_profile.id,
            company_id: company.company.id,
            target_agent_id: beta.agent_profile.id,
        })
        .expect("company direct chat should be reused");
    assert_eq!(replay.preview.id, direct.preview.id);

    let sent = app
        .send_company_message(SendCompanyMessageInput {
            actor_agent_id: alpha.agent_profile.id,
            company_id: company.company.id,
            conversation_id: direct.preview.id,
            content: "请同步今天的研发进度".into(),
        })
        .expect("alpha should message beta");
    assert_eq!(sent.conversation_id, direct.preview.id);
    assert_eq!(
        app.get_agent_conversation_messages(beta.agent_profile.id, direct.preview.id)
            .expect("beta should read company direct messages")
            .len(),
        1
    );
    let beta_event = app
        .list_agent_inbox_events(beta.agent_profile.id, true, 20)
        .expect("beta inbox should load")
        .into_iter()
        .find(|event| event.event_type == "message.received")
        .expect("beta should receive a message event");
    let reply = app
        .reply_to_company_inbox_message(ReplyCompanyInboxMessageInput {
            actor_agent_id: beta.agent_profile.id,
            event_id: beta_event.id,
            content: "研发任务已开始，当前没有阻塞".into(),
            auto_ack: true,
        })
        .expect("beta should reply directly from inbox");
    assert_eq!(reply.message.conversation_id, direct.preview.id);
    assert!(matches!(
        reply.event.status,
        AgentInboxEventStatus::Processed
    ));
    assert!(app
        .list_agent_inbox_events(alpha.agent_profile.id, true, 20)
        .expect("alpha inbox should load")
        .iter()
        .any(|event| event.event_type == "message.received"));

    let group = app
        .create_company_group_conversation(CreateCompanyGroupConversationInput {
            actor_agent_id: alpha.agent_profile.id,
            company_id: company.company.id,
            title: "研发项目群".into(),
            member_agent_ids: vec![beta.agent_profile.id],
        })
        .expect("coworkers should create a company group");
    assert_eq!(
        group.context.context_type,
        CONVERSATION_CONTEXT_COMPANY_GROUP
    );
    app.send_company_message(SendCompanyMessageInput {
        actor_agent_id: beta.agent_profile.id,
        company_id: company.company.id,
        conversation_id: group.preview.id,
        content: "研发任务已开始".into(),
    })
    .expect("group member should send a message");

    let context = app
        .get_company_agent_context(GetCompanyAgentContextInput {
            actor_agent_id: alpha.agent_profile.id,
            company_id: company.company.id,
        })
        .expect("company context should load");
    assert_eq!(context.agents.len(), 3);
    assert_eq!(context.conversations.len(), 3);

    let cross_company_direct = app
        .open_company_direct_conversation(OpenCompanyDirectConversationInput {
            actor_agent_id: alpha.agent_profile.id,
            company_id: company.company.id,
            target_agent_id: outsider.agent_profile.id,
        })
        .expect_err("cross-company direct chat must be rejected");
    assert!(matches!(cross_company_direct, AppError::Unauthorized(_)));
    let cross_company_group = app
        .create_company_group_conversation(CreateCompanyGroupConversationInput {
            actor_agent_id: alpha.agent_profile.id,
            company_id: company.company.id,
            title: "Invalid Cross Company Group".into(),
            member_agent_ids: vec![outsider.agent_profile.id],
        })
        .expect_err("cross-company group member must be rejected");
    assert!(matches!(cross_company_group, AppError::Unauthorized(_)));
}

#[test]
fn human_company_messages_open_direct_chats_and_enqueue_agent_inbox_events() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "human-message-owner@example.com".into(),
            display_name: "Human Owner".into(),
        })
        .expect("owner should be created");
    let outsider = app
        .dev_login(DevLoginInput {
            email: "human-message-outsider@example.com".into(),
            display_name: "Human Outsider".into(),
        })
        .expect("outsider should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Human Message Company".into(),
            slug: Some("human-message-company".into()),
            description: None,
        })
        .expect("company should be created");
    let other_company = app
        .create_company(CreateCompanyInput {
            human_user_id: outsider.id,
            name: "Other Human Message Company".into(),
            slug: Some("other-human-message-company".into()),
            description: None,
        })
        .expect("other company should be created");
    let alpha = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Alpha".into(),
            handle: "human-message-alpha".into(),
            persona: "负责协调".into(),
            org_unit_id: None,
            job_title: None,
            role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
            reports_to_membership_id: None,
        })
        .expect("alpha should be created");
    let beta = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Beta".into(),
            handle: "human-message-beta".into(),
            persona: "负责实现".into(),
            org_unit_id: None,
            job_title: None,
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: Some(alpha.membership.id),
        })
        .expect("beta should be created");

    let future_check = now_utc() + Duration::hours(6);
    for agent_id in [alpha.agent_profile.id, beta.agent_profile.id] {
        app.repo
            .save_agent_codex_trigger_config(AgentCodexTriggerConfig {
                id: Uuid::new_v4(),
                company_id: company.company.id,
                agent_profile_id: agent_id,
                status: AGENT_CODEX_TRIGGER_STATUS_ACTIVE.into(),
                interval_seconds: 21_600,
                codex_profile: "default".into(),
                model: None,
                reasoning_effort: None,
                reasoning_summary: None,
                verbosity: None,
                personality: None,
                service_tier: None,
                sandbox_mode: AGENT_CODEX_SANDBOX_WORKSPACE_WRITE.into(),
                approval_policy: AGENT_CODEX_APPROVAL_POLICY_NEVER.into(),
                network_access: None,
                web_search: None,
                feature_multi_agent: None,
                feature_remote_plugin: None,
                feature_hooks: None,
                feature_goals: None,
                feature_shell_tool: None,
                max_run_seconds: 1_800,
                next_run_at: future_check,
                lease_owner: None,
                lease_expires_at: None,
                manual_run_requested_at: None,
                wake_requested_at: None,
                wake_reason: None,
                last_run_at: None,
                last_success_at: None,
                last_error: None,
                consecutive_failure_count: 0,
                created_by_human_user_id: owner.id,
                updated_by_human_user_id: Some(owner.id),
                created_at: now_utc(),
                updated_at: now_utc(),
            })
            .expect("test should configure the Agent Codex trigger");
    }

    let direct = app
        .open_human_company_direct_conversation(OpenHumanCompanyDirectConversationInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            target_agent_id: beta.agent_profile.id,
        })
        .expect("owner should open a human-agent direct chat");
    let replay = app
        .open_human_company_direct_conversation(OpenHumanCompanyDirectConversationInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            target_agent_id: beta.agent_profile.id,
        })
        .expect("the same direct chat should be reused");
    assert_eq!(replay.preview.id, direct.preview.id);
    assert_eq!(direct.member_agent_ids, vec![beta.agent_profile.id]);

    let sent = app
        .send_human_company_message(SendHumanCompanyMessageInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            conversation_id: direct.preview.id,
            content: "请开始实现消息发送功能，并在完成后回复。".into(),
        })
        .expect("owner should send a direct message");
    assert_eq!(sent.sender_agent_id, None);
    assert_eq!(sent.sender_human_user_id, Some(owner.id));
    let beta_trigger = app
        .repo
        .get_agent_codex_trigger_config_by_agent(beta.agent_profile.id)
        .expect("beta trigger should exist");
    assert_eq!(beta_trigger.wake_reason.as_deref(), Some("message"));
    assert_eq!(beta_trigger.wake_requested_at, Some(sent.created_at));
    assert!(beta_trigger.next_run_at <= sent.created_at);
    let beta_decision = app
        .decide_agent_codex_work(&beta_trigger)
        .expect("an Agent without a project should still process direct messages");
    assert!(beta_decision.should_run);
    assert_eq!(beta_decision.trigger_type, AGENT_CODEX_TRIGGER_TYPE_MESSAGE);
    assert_eq!(beta_decision.pending_inbox_count, 1);
    assert!(beta_decision.project.is_none());
    assert!(beta_decision.git.is_none());
    assert!(app
        .repo
        .get_agent_codex_trigger_config_by_agent(alpha.agent_profile.id)
        .expect("alpha trigger should exist")
        .wake_requested_at
        .is_none());
    let direct_event = app
        .list_agent_inbox_events(beta.agent_profile.id, true, 20)
        .expect("beta inbox should load")
        .into_iter()
        .find(|event| {
            event.event_type == "message.received"
                && payload_uuid_field_optional(&event.payload_json, "conversation_id")
                    == Some(direct.preview.id)
        })
        .expect("human direct message should enqueue beta inbox event");
    assert_eq!(
        payload_uuid_field_optional(&direct_event.payload_json, "sender_human_user_id"),
        Some(owner.id)
    );

    assert!(matches!(
        app.mark_agent_inbox_event_processed(MarkInboxEventProcessedInput {
            actor_agent_id: beta.agent_profile.id,
            event_id: direct_event.id,
        }),
        Err(AppError::Conflict(_))
    ));
    assert!(app
        .list_agent_inbox_events(beta.agent_profile.id, true, 20)
        .expect("beta inbox should remain pending until a reply")
        .iter()
        .any(|event| event.id == direct_event.id));

    app.reply_to_company_inbox_message(ReplyCompanyInboxMessageInput {
        actor_agent_id: beta.agent_profile.id,
        event_id: direct_event.id,
        content: "已经开始处理，我会在这个会话继续同步。".into(),
        auto_ack: true,
    })
    .expect("agent should reply in the human direct conversation");
    let direct_messages = app
        .get_owned_conversation_messages(owner.id, direct.preview.id)
        .expect("company human should read the conversation");
    assert_eq!(direct_messages.len(), 2);
    assert_eq!(
        direct_messages
            .last()
            .and_then(|message| message.sender_agent_id),
        Some(beta.agent_profile.id)
    );

    let console = app
        .get_company_console(owner.id, company.company.id)
        .expect("company console should load");
    let default_group = console
        .conversations
        .iter()
        .find(|conversation| conversation.context.context_type == CONVERSATION_CONTEXT_COMPANY_ALL)
        .expect("default group should exist");
    for agent_id in [alpha.agent_profile.id, beta.agent_profile.id] {
        let mut trigger = app
            .repo
            .get_agent_codex_trigger_config_by_agent(agent_id)
            .expect("Agent trigger should exist");
        trigger.next_run_at = future_check;
        trigger.wake_requested_at = None;
        trigger.wake_reason = None;
        app.repo
            .save_agent_codex_trigger_config(trigger)
            .expect("test should reset the message wake state");
    }
    let _group_message = app
        .send_human_company_message(SendHumanCompanyMessageInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            conversation_id: default_group.preview.id,
            content: "请大家查看今天的公司公告。".into(),
        })
        .expect("owner should send to the company group");
    for agent_id in [alpha.agent_profile.id, beta.agent_profile.id] {
        assert!(app
            .list_agent_inbox_events(agent_id, true, 30)
            .expect("agent inbox should load")
            .iter()
            .any(|event| {
                event.event_type == "message.received"
                    && payload_uuid_field_optional(&event.payload_json, "conversation_id")
                        == Some(default_group.preview.id)
            }));
        let trigger = app
            .repo
            .get_agent_codex_trigger_config_by_agent(agent_id)
            .expect("Agent trigger should exist");
        assert!(trigger.wake_requested_at.is_none());
        assert!(trigger.wake_reason.is_none());
        assert_eq!(trigger.next_run_at, future_check);
    }

    let project = app
        .create_company_project(CreateCompanyProjectInput {
            actor_agent_id: alpha.agent_profile.id,
            company_id: company.company.id,
            name: "Selective Wake Project".into(),
            description: Some("验证项目群只唤醒 Ready 任务负责人".into()),
            member_agent_ids: vec![beta.agent_profile.id],
        })
        .expect("alpha should create a project group");
    let ready_task = app
        .create_company_project_task_for_human(CreateCompanyProjectTaskForHumanInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            project_id: project.project.id,
            title: "执行项目群唤醒验证".into(),
            description: None,
            priority: None,
            assignee_agent_id: Some(beta.agent_profile.id),
            due_at: None,
            depends_on_task_ids: Vec::new(),
        })
        .expect("owner should assign a ready project task");
    for agent_id in [alpha.agent_profile.id, beta.agent_profile.id] {
        let mut trigger = app
            .repo
            .get_agent_codex_trigger_config_by_agent(agent_id)
            .expect("Agent trigger should exist");
        trigger.next_run_at = future_check;
        trigger.wake_requested_at = None;
        trigger.wake_reason = None;
        app.repo
            .save_agent_codex_trigger_config(trigger)
            .expect("test should reset the project message wake state");
    }
    let project_message = app
        .send_human_company_message(SendHumanCompanyMessageInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            conversation_id: project.project_group.preview.id,
            content: "项目进入执行阶段，请按 Ready 任务推进。".into(),
        })
        .expect("owner should broadcast to the project group");
    let alpha_project_trigger = app
        .repo
        .get_agent_codex_trigger_config_by_agent(alpha.agent_profile.id)
        .expect("alpha trigger should exist");
    assert!(alpha_project_trigger.wake_requested_at.is_none());
    let beta_project_trigger = app
        .repo
        .get_agent_codex_trigger_config_by_agent(beta.agent_profile.id)
        .expect("beta trigger should exist");
    assert_eq!(
        beta_project_trigger.wake_requested_at,
        Some(project_message.created_at)
    );
    assert!(app
        .list_agent_inbox_events(beta.agent_profile.id, true, 100)
        .expect("beta inbox should load")
        .iter()
        .any(|event| {
            event.event_type == "message.received"
                && payload_uuid_field_optional(&event.payload_json, "message_id")
                    == Some(project_message.id)
                && payload_uuid_field_optional(&event.payload_json, "project_id").is_none()
        }));
    assert_eq!(ready_task.assignee_agent_id, Some(beta.agent_profile.id));

    for agent_id in [alpha.agent_profile.id, beta.agent_profile.id] {
        let mut trigger = app
            .repo
            .get_agent_codex_trigger_config_by_agent(agent_id)
            .expect("Agent trigger should exist");
        trigger.next_run_at = future_check;
        trigger.wake_requested_at = None;
        trigger.wake_reason = None;
        app.repo
            .save_agent_codex_trigger_config(trigger)
            .expect("test should reset the project owner follow-up wake state");
    }
    let member_update = app
        .send_company_message(SendCompanyMessageInput {
            actor_agent_id: beta.agent_profile.id,
            company_id: company.company.id,
            conversation_id: project.project_group.preview.id,
            content: "任务已经完成，请 Owner 验收并推进下一阶段。".into(),
        })
        .expect("a project member update should be sent");
    let owner_trigger = app
        .repo
        .get_agent_codex_trigger_config_by_agent(alpha.agent_profile.id)
        .expect("project owner trigger should exist");
    assert_eq!(
        owner_trigger.wake_requested_at,
        Some(member_update.created_at)
    );
    assert_eq!(owner_trigger.wake_reason.as_deref(), Some("message"));
    let owner_events = app
        .list_agent_inbox_events(alpha.agent_profile.id, true, 100)
        .expect("project owner inbox should load")
        .into_iter()
        .filter(|event| {
            event.event_type == "message.received"
                && payload_uuid_field_optional(&event.payload_json, "message_id")
                    == Some(member_update.id)
        })
        .collect::<Vec<_>>();
    assert_eq!(owner_events.len(), 1);
    assert_eq!(
        owner_events[0]
            .payload_json
            .get("project_owner_followup")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert!(app
        .repo
        .get_agent_codex_trigger_config_by_agent(beta.agent_profile.id)
        .expect("project member trigger should exist")
        .wake_requested_at
        .is_none());

    let mut owner_trigger = app
        .repo
        .get_agent_codex_trigger_config_by_agent(alpha.agent_profile.id)
        .expect("project owner trigger should exist");
    owner_trigger.next_run_at = future_check;
    owner_trigger.wake_requested_at = None;
    owner_trigger.wake_reason = None;
    app.repo
        .save_agent_codex_trigger_config(owner_trigger)
        .expect("test should reset project owner self-wake state");
    app.send_company_message(SendCompanyMessageInput {
        actor_agent_id: alpha.agent_profile.id,
        company_id: company.company.id,
        conversation_id: project.project_group.preview.id,
        content: "Owner 已完成验收。".into(),
    })
    .expect("project owner should send an update without self-waking");
    assert!(app
        .repo
        .get_agent_codex_trigger_config_by_agent(alpha.agent_profile.id)
        .expect("project owner trigger should exist")
        .wake_requested_at
        .is_none());

    for agent_id in [alpha.agent_profile.id, beta.agent_profile.id] {
        let mut trigger = app
            .repo
            .get_agent_codex_trigger_config_by_agent(agent_id)
            .expect("Agent trigger should exist");
        trigger.next_run_at = future_check;
        trigger.wake_requested_at = None;
        trigger.wake_reason = None;
        app.repo
            .save_agent_codex_trigger_config(trigger)
            .expect("test should reset the selective mention wake state");
    }
    let mentioned_message = app
        .send_human_company_message_with_mentions(SendHumanCompanyMessageWithMentionsInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            conversation_id: default_group.preview.id,
            content: "@Beta 请优先检查消息选择性唤醒。".into(),
            mentioned_agent_ids: vec![beta.agent_profile.id],
            mention_all: false,
        })
        .expect("owner should mention one Agent in the company group");
    let beta_mention_event = app
        .list_agent_inbox_events(beta.agent_profile.id, true, 50)
        .expect("mentioned Agent inbox should load")
        .into_iter()
        .find(|event| {
            event.event_type == "message.received"
                && payload_uuid_field_optional(&event.payload_json, "message_id")
                    == Some(mentioned_message.id)
        })
        .expect("mentioned Agent should receive an inbox event");
    assert_eq!(
        beta_mention_event
            .payload_json
            .get("mentioned")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert!(app
        .list_agent_inbox_events(alpha.agent_profile.id, true, 50)
        .expect("unmentioned Agent inbox should load")
        .iter()
        .all(|event| {
            payload_uuid_field_optional(&event.payload_json, "message_id")
                != Some(mentioned_message.id)
        }));
    assert!(app
        .repo
        .get_agent_codex_trigger_config_by_agent(alpha.agent_profile.id)
        .expect("unmentioned Agent trigger should exist")
        .wake_requested_at
        .is_none());
    assert_eq!(
        app.repo
            .get_agent_codex_trigger_config_by_agent(beta.agent_profile.id)
            .expect("mentioned Agent trigger should exist")
            .wake_requested_at,
        Some(mentioned_message.created_at)
    );

    assert!(matches!(
        app.send_human_company_message_with_mentions(SendHumanCompanyMessageWithMentionsInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            conversation_id: direct.preview.id,
            content: "私聊不需要 @。".into(),
            mentioned_agent_ids: vec![beta.agent_profile.id],
            mention_all: false,
        }),
        Err(AppError::Validation(_))
    ));
    assert!(matches!(
        app.send_human_company_message_with_mentions(SendHumanCompanyMessageWithMentionsInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            conversation_id: default_group.preview.id,
            content: "不能 @ 群外成员。".into(),
            mentioned_agent_ids: vec![Uuid::new_v4()],
            mention_all: false,
        }),
        Err(AppError::Validation(_))
    ));

    for agent_id in [alpha.agent_profile.id, beta.agent_profile.id] {
        let mut trigger = app
            .repo
            .get_agent_codex_trigger_config_by_agent(agent_id)
            .expect("Agent trigger should exist");
        trigger.next_run_at = future_check;
        trigger.wake_requested_at = None;
        trigger.wake_reason = None;
        app.repo
            .save_agent_codex_trigger_config(trigger)
            .expect("test should reset the Agent message wake state");
    }
    let agent_group_message = app
        .send_company_message(SendCompanyMessageInput {
            actor_agent_id: beta.agent_profile.id,
            company_id: company.company.id,
            conversation_id: default_group.preview.id,
            content: "Beta 已完成检查，请 Alpha 继续处理。".into(),
        })
        .expect("an Agent should send to the company group");
    let alpha_trigger = app
        .repo
        .get_agent_codex_trigger_config_by_agent(alpha.agent_profile.id)
        .expect("recipient Agent trigger should exist");
    assert!(alpha_trigger.wake_requested_at.is_none());
    assert!(alpha_trigger.wake_reason.is_none());
    assert_eq!(alpha_trigger.next_run_at, future_check);
    assert!(app
        .list_agent_inbox_events(alpha.agent_profile.id, true, 50)
        .expect("recipient Agent inbox should load")
        .iter()
        .any(|event| {
            event.event_type == "message.received"
                && payload_uuid_field_optional(&event.payload_json, "message_id")
                    == Some(agent_group_message.id)
        }));
    assert!(app
        .repo
        .get_agent_codex_trigger_config_by_agent(beta.agent_profile.id)
        .expect("sender Agent trigger should exist")
        .wake_requested_at
        .is_none());

    let mentioned_by_agent = app
        .send_company_message_with_mentions(SendCompanyMessageWithMentionsInput {
            actor_agent_id: beta.agent_profile.id,
            company_id: company.company.id,
            conversation_id: default_group.preview.id,
            content: "@Alpha 请立即接手这项工作。".into(),
            mentioned_agent_ids: vec![alpha.agent_profile.id],
            mention_all: false,
        })
        .expect("an Agent mention should wake the target Agent");
    let alpha_trigger = app
        .repo
        .get_agent_codex_trigger_config_by_agent(alpha.agent_profile.id)
        .expect("mentioned Agent trigger should exist");
    assert_eq!(
        alpha_trigger.wake_requested_at,
        Some(mentioned_by_agent.created_at)
    );
    assert_eq!(alpha_trigger.wake_reason.as_deref(), Some("message"));

    let claimed_at = now_utc();
    let mut busy_trigger = app
        .repo
        .get_agent_codex_trigger_config_by_agent(beta.agent_profile.id)
        .expect("beta trigger should exist");
    busy_trigger.lease_owner = Some("test-worker".into());
    busy_trigger.lease_expires_at = Some(claimed_at + Duration::minutes(30));
    busy_trigger.last_run_at = Some(claimed_at);
    busy_trigger.next_run_at = claimed_at + Duration::hours(1);
    app.repo
        .save_agent_codex_trigger_config(busy_trigger)
        .expect("test should simulate an active trigger lease");
    let queued_manual = app
        .run_company_agent_codex_trigger_now_for_human(
            SetCompanyAgentCodexTriggerStatusForHumanInput {
                human_user_id: owner.id,
                company_id: company.company.id,
                agent_id: beta.agent_profile.id,
            },
        )
        .expect("run-now during an active cycle should queue another cycle");
    assert_eq!(
        queued_manual.config.lease_owner.as_deref(),
        Some("test-worker")
    );
    assert!(queued_manual.config.manual_run_requested_at > Some(claimed_at));
    assert_eq!(
        queued_manual.config.next_run_at,
        queued_manual
            .config
            .manual_run_requested_at
            .expect("manual request should be recorded")
    );

    assert!(matches!(
        app.open_human_company_direct_conversation(OpenHumanCompanyDirectConversationInput {
            human_user_id: outsider.id,
            company_id: company.company.id,
            target_agent_id: beta.agent_profile.id,
        }),
        Err(AppError::Unauthorized(_))
    ));
    let other_group = other_company
        .conversations
        .first()
        .expect("other company default group should exist");
    assert!(matches!(
        app.send_human_company_message(SendHumanCompanyMessageInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            conversation_id: other_group.preview.id,
            content: "不能跨公司发送".into(),
        }),
        Err(AppError::Unauthorized(_))
    ));
}
