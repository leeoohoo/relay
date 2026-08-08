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
