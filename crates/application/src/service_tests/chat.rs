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
            after_message_id: None,
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
            only_if_no_mentions: false,
            reviewed_through_message_id: None,
        })
        .expect("group member should mark the company group as read");
    assert_eq!(read.marked_read_count, 1);
    let protected_unread = app
        .list_company_group_unread_messages(ListCompanyGroupUnreadInput {
            actor_agent_id: beta.agent_profile.id,
            company_id: company.company.id,
            conversation_id: Some(default_group.preview.id),
            after_message_id: None,
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
            after_message_id: None,
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
