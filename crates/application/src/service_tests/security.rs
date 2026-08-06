use super::*;

#[test]
fn human_email_verification_password_change_and_reset_work() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let registered = app
        .register_human(RegisterHumanInput {
            email: "account-ops@example.com".into(),
            display_name: "Account Ops".into(),
            password: "original-password".into(),
        })
        .expect("registration should succeed");

    let verification = app
        .issue_human_email_verification(registered.user.id)
        .expect("verification token should be issued");
    app.verify_human_email(&verification.token)
        .expect("verification token should work");
    assert!(app
        .is_human_email_verified(registered.user.id)
        .expect("verification state should load"));

    let second_session = app
        .login_human(LoginHumanInput {
            email: registered.user.email.clone(),
            password: "original-password".into(),
        })
        .expect("second session should be issued");
    let sessions = app
        .list_human_sessions(registered.user.id, &registered.session_token)
        .expect("sessions should list");
    assert_eq!(sessions.len(), 2);

    let revoked = app
        .change_human_password(
            registered.user.id,
            &registered.session_token,
            ChangeHumanPasswordInput {
                current_password: "original-password".into(),
                new_password: "changed-password".into(),
            },
        )
        .expect("password change should succeed");
    assert_eq!(revoked, 1);
    assert!(app
        .authenticate_human_session(&second_session.session_token)
        .is_err());
    assert!(app
        .login_human(LoginHumanInput {
            email: registered.user.email.clone(),
            password: "original-password".into(),
        })
        .is_err());

    let reset = app
        .issue_human_password_reset(&registered.user.email)
        .expect("reset request should succeed")
        .expect("known account should receive reset token")
        .1;
    app.reset_human_password(ResetHumanPasswordInput {
        token: reset.token,
        new_password: "reset-password".into(),
    })
    .expect("password reset should succeed");
    assert!(app
        .authenticate_human_session(&registered.session_token)
        .is_err());
    app.login_human(LoginHumanInput {
        email: registered.user.email,
        password: "reset-password".into(),
    })
    .expect("reset password should authenticate");
}

#[test]
fn idempotency_replays_same_request_and_rejects_key_reuse() {
    let repo = MemoryPlatformRepository::default();
    let app = PlatformApp::new(repo.clone());
    let owner = app
        .dev_login(DevLoginInput {
            email: "idempotency-owner@example.com".into(),
            display_name: "Idempotency Owner".into(),
        })
        .expect("owner should exist");
    let agent = AgentProfile {
        id: Uuid::new_v4(),
        owner_user_id: owner.id,
        display_name: "Idempotency Agent".into(),
        handle: "idempotency-agent".into(),
        persona: "test".into(),
        collaboration_preference: AGENT_COLLABORATION_PREFERENCE_AVAILABLE.into(),
        status: AgentStatus::Active,
        created_at: now_utc(),
    };
    repo.insert_agent_profile(agent.clone())
        .expect("agent fixture should be stored");

    let request = json!({"content": "hello", "idempotency_key": "retry-1"});
    let response = json!({"post_id": Uuid::new_v4()});
    assert!(app
        .replay_agent_idempotency(agent.id, "post.create", "retry-1", &request)
        .expect("initial lookup should succeed")
        .is_none());
    app.store_agent_idempotency(agent.id, "post.create", "retry-1", &request, &response)
        .expect("result should be stored");
    assert_eq!(
        app.replay_agent_idempotency(agent.id, "post.create", "retry-1", &request)
            .expect("replay should succeed"),
        Some(response)
    );
    assert!(matches!(
        app.replay_agent_idempotency(
            agent.id,
            "post.create",
            "retry-1",
            &json!({"content": "different", "idempotency_key": "retry-1"})
        ),
        Err(AppError::Conflict(_))
    ));
}

#[test]
fn agent_action_budget_rejects_the_sixty_first_write_per_minute() {
    let repo = MemoryPlatformRepository::default();
    let app = PlatformApp::new(repo.clone());
    let owner = app
        .dev_login(DevLoginInput {
            email: "rate-budget-owner@example.com".into(),
            display_name: "Rate Budget Owner".into(),
        })
        .expect("owner should exist");
    let agent = AgentProfile {
        id: Uuid::new_v4(),
        owner_user_id: owner.id,
        display_name: "Rate Budget Agent".into(),
        handle: "rate-budget-agent".into(),
        persona: "test".into(),
        collaboration_preference: AGENT_COLLABORATION_PREFERENCE_AVAILABLE.into(),
        status: AgentStatus::Active,
        created_at: now_utc(),
    };
    repo.insert_agent_profile(agent.clone())
        .expect("agent fixture should be stored");

    for index in 0..60 {
        app.record_agent_action(
            agent.id,
            "test.write",
            Some(index.to_string()),
            json!({}),
            json!({}),
            AgentActionStatus::Success,
        )
        .expect("action log should be stored");
    }

    assert!(matches!(
        app.enforce_agent_action_budget(agent.id),
        Err(AppError::RateLimited(message)) if message.contains("60 actions per minute")
    ));
}

#[test]
fn expired_human_session_is_rejected() {
    let repo = MemoryPlatformRepository::default();
    let app = PlatformApp::new(repo.clone());
    let user = app
        .dev_login(DevLoginInput {
            email: "expired-owner@example.com".into(),
            display_name: "Expired Owner".into(),
        })
        .expect("dev user should be created");
    let token = "hus_expired_test_token";
    repo.insert_human_session(HumanSession {
        id: Uuid::new_v4(),
        human_user_id: user.id,
        token_prefix: "hus_expired".into(),
        token_hash: hash_secret(token),
        expires_at: now_utc() - Duration::minutes(1),
        revoked_at: None,
        last_used_at: None,
        created_at: now_utc() - Duration::hours(1),
    })
    .expect("expired fixture should be stored");

    assert!(matches!(
        app.authenticate_human_session(token),
        Err(AppError::Unauthorized(_))
    ));
}

#[test]
fn owner_cannot_read_or_govern_another_owners_agent() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());
    let owner_a = app
        .dev_login(DevLoginInput {
            email: "owner-a@example.com".into(),
            display_name: "Owner A".into(),
        })
        .expect("owner A should exist");
    let owner_b = app
        .dev_login(DevLoginInput {
            email: "owner-b@example.com".into(),
            display_name: "Owner B".into(),
        })
        .expect("owner B should exist");
    let challenge = app
        .create_weibo_challenge(CreateWeiboChallengeInput {
            human_user_id: owner_b.id,
            desired_handle: "@owner-b-agent".into(),
            desired_display_name: "Owner B Agent".into(),
            persona: "private agent".into(),
            weibo_handle: "owner_b_weibo".into(),
        })
        .expect("challenge should be created");

    assert!(matches!(
        app.verify_owned_weibo_challenge(
            owner_a.id,
            VerifyWeiboChallengeInput {
                challenge_id: challenge.challenge.id,
                submitted_text: challenge.challenge.template_text.clone(),
                source_url: None,
            }
        ),
        Err(AppError::Unauthorized(_))
    ));

    let verified = app
        .verify_owned_weibo_challenge(
            owner_b.id,
            VerifyWeiboChallengeInput {
                challenge_id: challenge.challenge.id,
                submitted_text: challenge.challenge.template_text.clone(),
                source_url: None,
            },
        )
        .expect("owner B should verify their own challenge");
    assert!(matches!(
        app.list_owned_agent_conversations(owner_a.id, verified.agent_profile.id),
        Err(AppError::Unauthorized(_))
    ));
    assert!(matches!(
        app.freeze_owned_agent(owner_a.id, verified.agent_profile.id),
        Err(AppError::Unauthorized(_))
    ));
}

#[test]
fn agent_can_update_own_profile() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());

    let owner = app
        .dev_login(DevLoginInput {
            email: "profile-owner@example.com".into(),
            display_name: "Profile Owner".into(),
        })
        .expect("owner login should succeed");
    let agent = bootstrap_agent(
        &app,
        owner.id,
        "@profile-alpha",
        "Profile Alpha",
        "最初的人格描述",
        "profile_owner_weibo",
        "https://weibo.example/profile-alpha",
    );

    let updated = app
        .update_agent_profile(UpdateAgentProfileInput {
            actor_agent_id: agent.id,
            display_name: Some("Profile Alpha 2".into()),
            persona: Some("更新后更关注多智能体协作与关系维护".into()),
            collaboration_preference: Some("low".into()),
        })
        .expect("agent profile update should succeed");

    assert_eq!(updated.display_name, "Profile Alpha 2");
    assert_eq!(updated.persona, "更新后更关注多智能体协作与关系维护");
    assert_eq!(updated.collaboration_preference, "low_cost_only");

    let persisted = app
        .get_agent_profile_by_id(agent.id)
        .expect("updated profile should remain readable");
    assert_eq!(persisted.display_name, "Profile Alpha 2");
    assert_eq!(persisted.persona, "更新后更关注多智能体协作与关系维护");
    assert_eq!(persisted.collaboration_preference, "low_cost_only");
}

#[test]
fn agent_profile_update_requires_actual_change() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());

    let owner = app
        .dev_login(DevLoginInput {
            email: "profile-same-owner@example.com".into(),
            display_name: "Profile Same Owner".into(),
        })
        .expect("owner login should succeed");
    let agent = bootstrap_agent(
        &app,
        owner.id,
        "@profile-same-alpha",
        "Profile Same Alpha",
        "保持原样",
        "profile_same_owner_weibo",
        "https://weibo.example/profile-same-alpha",
    );

    let error = app
        .update_agent_profile(UpdateAgentProfileInput {
            actor_agent_id: agent.id,
            display_name: Some("Profile Same Alpha".into()),
            persona: Some("保持原样".into()),
            collaboration_preference: Some("available".into()),
        })
        .expect_err("unchanged profile update should fail");

    assert!(matches!(error, AppError::Validation(message) if message.contains("must change")));
}

#[test]
fn agent_profile_update_validates_collaboration_preference() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());

    let owner = app
        .dev_login(DevLoginInput {
            email: "profile-pref-owner@example.com".into(),
            display_name: "Profile Pref Owner".into(),
        })
        .expect("owner login should succeed");
    let agent = bootstrap_agent(
        &app,
        owner.id,
        "@profile-pref-alpha",
        "Profile Pref Alpha",
        "会显式声明协作偏好",
        "profile_pref_owner_weibo",
        "https://weibo.example/profile-pref-alpha",
    );

    assert_eq!(agent.collaboration_preference, "available");

    let updated = app
        .update_agent_profile(UpdateAgentProfileInput {
            actor_agent_id: agent.id,
            display_name: None,
            persona: None,
            collaboration_preference: Some("暂不接请求".into()),
        })
        .expect("localized preference alias should succeed");
    assert_eq!(updated.collaboration_preference, "unavailable");

    let error = app
        .update_agent_profile(UpdateAgentProfileInput {
            actor_agent_id: agent.id,
            display_name: None,
            persona: None,
            collaboration_preference: Some("anything_goes".into()),
        })
        .expect_err("invalid collaboration preference should fail");
    assert!(
        matches!(error, AppError::Validation(message) if message.contains("collaboration_preference"))
    );
}

#[test]
fn frozen_agent_cannot_update_profile() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());

    let owner = app
        .dev_login(DevLoginInput {
            email: "profile-frozen-owner@example.com".into(),
            display_name: "Profile Frozen Owner".into(),
        })
        .expect("owner login should succeed");
    let agent = bootstrap_agent(
        &app,
        owner.id,
        "@profile-frozen-alpha",
        "Profile Frozen Alpha",
        "等待冻结测试",
        "profile_frozen_owner_weibo",
        "https://weibo.example/profile-frozen-alpha",
    );

    app.freeze_owned_agent(owner.id, agent.id)
        .expect("freeze should succeed");

    let error = app
        .update_agent_profile(UpdateAgentProfileInput {
            actor_agent_id: agent.id,
            display_name: Some("不该更新成功".into()),
            persona: None,
            collaboration_preference: None,
        })
        .expect_err("frozen agent should not update profile");

    assert!(matches!(error, AppError::Conflict(message) if message.contains("frozen")));
}

#[test]
fn owner_can_rotate_agent_key_and_old_key_becomes_invalid() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());

    let owner = app
        .dev_login(DevLoginInput {
            email: "rotate-owner@example.com".into(),
            display_name: "Rotate Owner".into(),
        })
        .expect("owner login should succeed");

    let challenge = app
        .create_weibo_challenge(CreateWeiboChallengeInput {
            human_user_id: owner.id,
            desired_handle: "@rotate-alpha".into(),
            desired_display_name: "Rotate Alpha".into(),
            persona: "喜欢稳定接入 MCP".into(),
            weibo_handle: "rotate_owner_weibo".into(),
        })
        .expect("challenge creation should succeed");

    let verified = app
        .verify_weibo_challenge(VerifyWeiboChallengeInput {
            challenge_id: challenge.challenge.id,
            submitted_text: challenge.challenge.template_text.clone(),
            source_url: Some("https://weibo.example/rotate-alpha".into()),
        })
        .expect("challenge verification should succeed");

    let old_key = verified.agent_key_plaintext.clone();
    let rotate_result = app
        .rotate_owned_agent_key(owner.id, verified.agent_profile.id)
        .expect("key rotation should succeed");

    assert_ne!(rotate_result.agent_key_plaintext, old_key);
    assert!(
        app.authenticate_agent_key(&old_key).is_err(),
        "old key should be revoked after rotation"
    );

    let authenticated = app
        .authenticate_agent_key(&rotate_result.agent_key_plaintext)
        .expect("new key should authenticate");
    assert_eq!(authenticated.id, verified.agent_profile.id);

    let active_key = app
        .repo
        .list_agent_keys(verified.agent_profile.id)
        .into_iter()
        .find(agent_key_is_active_record)
        .expect("rotated key should be active");
    assert_eq!(active_key.key_prefix, rotate_result.agent_key_prefix);
    assert!(app
        .repo
        .list_agent_action_logs(verified.agent_profile.id, 20)
        .iter()
        .any(|action| {
            action.action_type == "owner.rotate_agent_key"
                && matches!(action.status, AgentActionStatus::Success)
        }));
    let audit_logs = app
        .repo
        .list_agent_key_issue_logs(verified.agent_profile.id, 20);
    assert!(audit_logs
        .iter()
        .any(|log| matches!(log.issue_type, AgentKeyIssueType::Rotated)));
    assert!(audit_logs
        .iter()
        .any(|log| matches!(log.issue_type, AgentKeyIssueType::Revoked)));
}
