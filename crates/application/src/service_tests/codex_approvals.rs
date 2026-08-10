use super::*;

#[test]
fn website_access_is_a_supported_codex_approval_request() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let company_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let agent_id = Uuid::new_v4();

    let request = app
        .create_codex_approval_request(CreateCodexApprovalRequestInput {
            company_id,
            codex_trigger_run_id: run_id,
            requested_by_agent_id: agent_id,
            tool_name: AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS.into(),
            risk_level: "medium".into(),
            reason: "Agent requests browser navigation".into(),
            arguments: json!({
                "tool": "navigate_page",
                "url": "https://example.com"
            }),
            expires_at: now_utc() + Duration::minutes(5),
        })
        .expect("website access approval should be created");

    assert_eq!(request.company_id, company_id);
    assert_eq!(request.codex_trigger_run_id, Some(run_id));
    assert_eq!(request.requested_by_agent_id, agent_id);
    assert_eq!(request.tool_name, AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS);
    assert_eq!(request.status, AGENT_TOOL_APPROVAL_STATUS_PENDING);
}

#[test]
fn website_access_can_be_persistently_allowed_for_one_agent_project_and_origin() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "always-allow-owner@example.com".into(),
            display_name: "Approval Owner".into(),
        })
        .expect("owner");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Approval Company".into(),
            slug: Some("approval-company".into()),
            description: None,
        })
        .expect("company");
    let agent = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Browser Agent".into(),
            handle: "@browser-agent".into(),
            persona: "Tests websites".into(),
            org_unit_id: None,
            job_title: None,
            role_key: None,
            reports_to_membership_id: None,
        })
        .expect("agent");
    let scope = format!("project:{}", Uuid::new_v4());
    let target = "https://example.com";
    let mut arguments = json!({
        "tool": "new_page",
        "url": "https://example.com/dashboard",
    });
    arguments
        .as_object_mut()
        .expect("approval arguments")
        .insert(AGENT_CODEX_APPROVAL_SCOPE_KEY.into(), json!(scope.clone()));
    arguments
        .as_object_mut()
        .expect("approval arguments")
        .insert(AGENT_CODEX_APPROVAL_TARGET_KEY.into(), json!(target));
    let request = app
        .create_codex_approval_request(CreateCodexApprovalRequestInput {
            company_id: company.company.id,
            codex_trigger_run_id: Uuid::new_v4(),
            requested_by_agent_id: agent.agent_profile.id,
            tool_name: AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS.into(),
            risk_level: "medium".into(),
            reason: "Open the test site".into(),
            arguments,
            expires_at: now_utc() + Duration::minutes(5),
        })
        .expect("approval request");

    let approved = app
        .approve_agent_tool_approval(ReviewAgentToolApprovalInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            approval_request_id: request.id,
            review_note: None,
            approval_mode: Some(AGENT_TOOL_APPROVAL_MODE_ALWAYS.into()),
        })
        .expect("always allow approval");

    assert_eq!(approved.status, AGENT_TOOL_APPROVAL_STATUS_APPROVED);
    assert_eq!(
        approved.execution_result,
        json!({
            "approval_mode": AGENT_TOOL_APPROVAL_MODE_ALWAYS,
            "approval_scope": scope,
            "approval_target": target,
        })
    );
    assert!(app
        .has_codex_always_allow_approval(
            company.company.id,
            agent.agent_profile.id,
            AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS,
            &scope,
            target,
        )
        .expect("matching grant lookup"));
    assert!(!app
        .has_codex_always_allow_approval(
            company.company.id,
            agent.agent_profile.id,
            AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS,
            &scope,
            "https://other.example.com",
        )
        .expect("non-matching grant lookup"));

    let local_scope = format!("project:{}", Uuid::new_v4());
    let local_target = "http://localhost:*";
    let mut local_arguments = json!({
        "tool": "new_page",
        "url": "http://127.0.0.1:4177/",
    });
    let local_arguments_object = local_arguments
        .as_object_mut()
        .expect("local approval arguments");
    local_arguments_object.insert(
        AGENT_CODEX_APPROVAL_SCOPE_KEY.into(),
        json!(local_scope.clone()),
    );
    local_arguments_object.insert(
        AGENT_CODEX_APPROVAL_TARGET_KEY.into(),
        json!("http://127.0.0.1:4177"),
    );
    local_arguments_object.insert(
        AGENT_CODEX_APPROVAL_LOCAL_TARGET_KEY.into(),
        json!(local_target),
    );
    let local_request = app
        .create_codex_approval_request(CreateCodexApprovalRequestInput {
            company_id: company.company.id,
            codex_trigger_run_id: Uuid::new_v4(),
            requested_by_agent_id: agent.agent_profile.id,
            tool_name: AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS.into(),
            risk_level: "medium".into(),
            reason: "Open changing local preview ports".into(),
            arguments: local_arguments,
            expires_at: now_utc() + Duration::minutes(5),
        })
        .expect("local preview approval request");
    let local_approved = app
        .approve_agent_tool_approval(ReviewAgentToolApprovalInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            approval_request_id: local_request.id,
            review_note: None,
            approval_mode: Some(AGENT_TOOL_APPROVAL_MODE_ALWAYS_LOCALHOST.into()),
        })
        .expect("local preview ports should be persistently allowed");
    assert_eq!(
        local_approved.execution_result["approval_mode"],
        AGENT_TOOL_APPROVAL_MODE_ALWAYS_LOCALHOST
    );
    assert!(app
        .has_codex_always_allow_approval(
            company.company.id,
            agent.agent_profile.id,
            AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS,
            &local_scope,
            local_target,
        )
        .expect("local preview grant lookup"));
}
