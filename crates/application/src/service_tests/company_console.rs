use super::*;

#[test]
fn company_console_agent_pages_are_bounded_stable_and_non_overlapping() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let human = app
        .dev_login(DevLoginInput {
            email: "console-pages@example.com".into(),
            display_name: "Console Pages Human".into(),
        })
        .expect("human should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: human.id,
            name: "Console Pages Company".into(),
            slug: Some("console-pages-company".into()),
            description: None,
        })
        .expect("company should be created");

    for index in 0..23 {
        app.create_company_agent(CreateCompanyAgentInput {
            human_user_id: human.id,
            company_id: company.company.id,
            display_name: format!("Agent {index:02}"),
            handle: format!("console-agent-{index:02}"),
            persona: "验证 Console 游标分页".into(),
            org_unit_id: None,
            job_title: Some("软件工程师".into()),
            role_key: None,
            reports_to_membership_id: None,
        })
        .expect("Agent should be created");
    }

    let first = app
        .list_company_console_agent_page_for_human(human.id, company.company.id, None, 10)
        .expect("first page should load");
    assert_eq!(first.items.len(), 10);
    assert!(first.has_more);
    let second = app
        .list_company_console_agent_page_for_human(
            human.id,
            company.company.id,
            first.next_cursor,
            10,
        )
        .expect("second page should load");
    assert_eq!(second.items.len(), 10);
    assert!(second.has_more);

    let first_ids = first
        .items
        .iter()
        .map(|item| item.membership.id)
        .collect::<std::collections::HashSet<_>>();
    assert!(second
        .items
        .iter()
        .all(|item| !first_ids.contains(&item.membership.id)));
}
