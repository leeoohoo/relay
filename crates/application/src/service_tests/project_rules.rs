use super::*;

#[test]
fn project_rules_assets_and_periodic_refresh_require_explicit_agent_permissions() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "project-content-owner@example.com".into(),
            display_name: "Project Content Owner".into(),
        })
        .expect("owner should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Project Content Company".into(),
            slug: Some("project-content-company".into()),
            description: None,
        })
        .expect("company should be created");
    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Content Manager".into(),
            handle: "content-manager".into(),
            persona: "负责项目".into(),
            org_unit_id: None,
            job_title: None,
            role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
            reports_to_membership_id: None,
        })
        .expect("manager should be created");
    let maintainer = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Asset Maintainer".into(),
            handle: "asset-maintainer".into(),
            persona: "维护规则和资产".into(),
            org_unit_id: None,
            job_title: None,
            role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reports_to_membership_id: Some(manager.membership.id),
        })
        .expect("maintainer should be created");
    let project = app
        .create_company_project(CreateCompanyProjectInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            name: "Project Content".into(),
            description: None,
            member_agent_ids: vec![maintainer.agent_profile.id],
        })
        .expect("project should be created");

    assert!(matches!(
        app.update_company_project_rule(UpdateCompanyProjectRuleInput {
            actor_agent_id: maintainer.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            content: "# 未授权".into(),
        }),
        Err(AppError::Unauthorized(_))
    ));
    app.update_company_agent_staffing_permissions(UpdateCompanyAgentPermissionsInput {
        human_user_id: owner.id,
        company_id: company.company.id,
        agent_id: maintainer.agent_profile.id,
        staffing_permissions: Vec::new(),
        project_permissions: vec![
            COMPANY_PERMISSION_PROJECT_RULES_MANAGE.into(),
            COMPANY_PERMISSION_PROJECT_ASSETS_MANAGE.into(),
        ],
        staffing_scope_org_unit_id: None,
        reason: Some("维护项目内容".into()),
    })
    .expect("human should grant project permissions");

    app.update_company_project_rule_for_human(UpdateCompanyProjectRuleForHumanInput {
        human_user_id: owner.id,
        company_id: company.company.id,
        project_id: project.project.id,
        content: "# Human Rule".into(),
    })
    .expect("human should write rule");
    app.request_company_project_rule_generation_for_human(
        RequestCompanyProjectRuleGenerationForHumanInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            project_id: project.project.id,
            agent_id: maintainer.agent_profile.id,
            instructions: Some("结合仓库补充测试要求".into()),
        },
    )
    .expect("human should delegate rule generation");
    let rule_generation_event = app
        .list_agent_inbox_events(maintainer.agent_profile.id, true, 50)
        .expect("inbox should load")
        .iter()
        .find(|event| event.event_type == "company.project.rule_generation_requested")
        .cloned()
        .expect("rule generation event should exist");
    assert!(matches!(
        app.mark_agent_inbox_event_processed(MarkInboxEventProcessedInput {
            actor_agent_id: maintainer.agent_profile.id,
            event_id: rule_generation_event.id,
        }),
        Err(AppError::Conflict(_))
    ));
    app.update_company_project_rule(UpdateCompanyProjectRuleInput {
        actor_agent_id: maintainer.agent_profile.id,
        company_id: company.company.id,
        project_id: project.project.id,
        content: "# Agent Rule\n\n- 运行测试".into(),
    })
    .expect("authorized maintainer should update rule");
    app.mark_agent_inbox_event_processed(MarkInboxEventProcessedInput {
        actor_agent_id: maintainer.agent_profile.id,
        event_id: rule_generation_event.id,
    })
    .expect("completed rule generation should be acknowledged");

    app.upsert_company_project_asset_refresh_for_human(
        UpsertCompanyProjectAssetRefreshForHumanInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            project_id: project.project.id,
            maintainer_agent_id: maintainer.agent_profile.id,
            interval_minutes: 60,
            enabled: true,
            run_now: true,
        },
    )
    .expect("human should configure asset refresh");
    for event in app
        .list_agent_inbox_events(maintainer.agent_profile.id, true, 100)
        .expect("pending events should load")
    {
        app.mark_agent_inbox_event_processed(MarkInboxEventProcessedInput {
            actor_agent_id: maintainer.agent_profile.id,
            event_id: event.id,
        })
        .expect("test should acknowledge pending event");
    }
    let now = now_utc();
    let trigger = AgentCodexTriggerConfig {
        id: Uuid::new_v4(),
        company_id: company.company.id,
        agent_profile_id: maintainer.agent_profile.id,
        status: AGENT_CODEX_TRIGGER_STATUS_ACTIVE.into(),
        interval_seconds: 60,
        codex_profile: "default".into(),
        model: None,
        reasoning_effort: None,
        reasoning_summary: None,
        verbosity: None,
        personality: None,
        service_tier: None,
        sandbox_mode: "workspace_write".into(),
        approval_policy: "never".into(),
        network_access: None,
        web_search: None,
        feature_multi_agent: None,
        feature_remote_plugin: None,
        feature_hooks: None,
        feature_goals: None,
        feature_shell_tool: None,
        max_run_seconds: 600,
        next_run_at: now,
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
        updated_by_human_user_id: None,
        created_at: now,
        updated_at: now,
    };
    let due = app
        .decide_agent_codex_work(&trigger)
        .expect("asset refresh should become due");
    assert!(due.should_run);
    assert!(due.asset_refresh_due);
    assert_eq!(due.trigger_type, AGENT_CODEX_TRIGGER_TYPE_ASSET_REFRESH);
    assert_eq!(
        due.project.map(|project| project.id),
        Some(project.project.id)
    );
    let claimed_again = app
        .decide_agent_codex_work(&trigger)
        .expect("claimed refresh should not immediately repeat");
    assert!(!claimed_again.asset_refresh_due);
    assert!(!claimed_again.should_run);

    let assets = app
        .replace_company_project_assets(ReplaceCompanyProjectAssetsInput {
            actor_agent_id: maintainer.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
            assets: vec![CompanyProjectAssetInput {
                name: "Server".into(),
                asset_type: "service".into(),
                locator: "apps/server".into(),
                description: Some("HTTP API".into()),
                status: Some("active".into()),
                metadata: Some(json!({ "language": "Rust" })),
            }],
        })
        .expect("authorized maintainer should replace assets");
    assert_eq!(assets.len(), 1);
    let view = app
        .get_company_project(GetCompanyProjectInput {
            actor_agent_id: maintainer.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
        })
        .expect("project view should include rule and assets");
    assert_eq!(
        view.rule.expect("rule should exist").content,
        "# Agent Rule\n\n- 运行测试"
    );
    assert_eq!(view.assets[0].locator, "apps/server");
    assert!(view
        .asset_refresh
        .expect("refresh config should exist")
        .last_completed_at
        .is_some());
}
