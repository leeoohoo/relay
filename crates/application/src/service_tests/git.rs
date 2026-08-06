use super::*;

#[test]
fn managed_project_path_is_generated_when_human_omits_the_advanced_path() {
    let project_id = Uuid::new_v4();
    let path = resolve_project_host_local_path(project_id, None, None)
        .expect("managed project path should be generated");
    assert!(Path::new(&path).is_absolute());
    assert!(path.ends_with(&project_id.to_string()));
}

#[test]
fn only_company_humans_can_configure_project_git_and_agents_receive_a_safe_view() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner = app
        .dev_login(DevLoginInput {
            email: "git-owner@example.com".into(),
            display_name: "Git Owner".into(),
        })
        .expect("owner should be created");
    let outsider = app
        .dev_login(DevLoginInput {
            email: "git-outsider@example.com".into(),
            display_name: "Git Outsider".into(),
        })
        .expect("outsider should be created");
    let company = app
        .create_company(CreateCompanyInput {
            human_user_id: owner.id,
            name: "Git Company".into(),
            slug: Some("git-company".into()),
            description: None,
        })
        .expect("company should be created");
    let manager = app
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            display_name: "Git Manager".into(),
            handle: "git-manager".into(),
            persona: "负责项目协作".into(),
            org_unit_id: None,
            job_title: None,
            role_key: Some(COMPANY_AGENT_ROLE_MANAGER.into()),
            reports_to_membership_id: None,
        })
        .expect("manager should be created");
    let project = app
        .create_company_project(CreateCompanyProjectInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            name: "Relay Trigger".into(),
            description: Some("由 Human 绑定仓库".into()),
            member_agent_ids: Vec::new(),
        })
        .expect("manager should create the project");
    assert!(project.git.is_none());

    assert!(matches!(
        app.get_company_project_git_for_human(GetCompanyProjectGitForHumanInput {
            human_user_id: outsider.id,
            company_id: company.company.id,
            project_id: project.project.id,
        }),
        Err(AppError::Unauthorized(_))
    ));

    let configured = app
        .upsert_company_project_git_for_human(UpsertCompanyProjectGitForHumanInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            project_id: project.project.id,
            remote_url: "https://git.example.com/relay/trigger.git".into(),
            host_local_path: Some(
                std::env::temp_dir()
                    .join("relay-trigger-project")
                    .to_string_lossy()
                    .into_owned(),
            ),
            default_branch: Some("develop".into()),
            auth_profile: Some("git-company-deploy-key".into()),
            allow_agent_push: Some(true),
            branch_prefix: Some("relay/team/".into()),
        })
        .expect("owner should configure project Git");
    assert_eq!(
        configured.auth_profile.as_deref(),
        Some("git-company-deploy-key")
    );
    assert_eq!(configured.git_host, "git.example.com");
    assert!(configured
        .host_local_path
        .ends_with("relay-trigger-project"));

    let agent_view = app
        .get_company_project(GetCompanyProjectInput {
            actor_agent_id: manager.agent_profile.id,
            company_id: company.company.id,
            project_id: project.project.id,
        })
        .expect("project member should read the project");
    let safe_git = agent_view
        .git
        .expect("agent should receive Git coordinates");
    assert_eq!(safe_git.default_branch, "develop");
    assert_eq!(safe_git.branch_prefix, "relay/team/");
    assert!(safe_git.push_enabled);
    assert!(safe_git.auth_configured);

    assert!(matches!(
        app.upsert_company_project_git_for_human(UpsertCompanyProjectGitForHumanInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            project_id: project.project.id,
            remote_url: "https://secret-token@git.example.com/relay/trigger.git".into(),
            host_local_path: Some(
                std::env::temp_dir()
                    .join("relay-trigger-project")
                    .to_string_lossy()
                    .into_owned(),
            ),
            default_branch: None,
            auth_profile: None,
            allow_agent_push: None,
            branch_prefix: None,
        }),
        Err(AppError::Validation(_))
    ));

    app.delete_company_project_git_for_human(DeleteCompanyProjectGitForHumanInput {
        human_user_id: owner.id,
        company_id: company.company.id,
        project_id: project.project.id,
    })
    .expect("owner should clear project Git");
    assert!(app
        .get_company_project_git_for_human(GetCompanyProjectGitForHumanInput {
            human_user_id: owner.id,
            company_id: company.company.id,
            project_id: project.project.id,
        })
        .expect("owner should read cleared Git configuration")
        .is_none());
}
