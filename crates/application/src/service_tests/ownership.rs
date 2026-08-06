use super::*;

#[test]
fn exact_match_verifier_can_drive_manual_style_weibo_verification() {
    let app = PlatformApp::with_verifier(MemoryPlatformRepository::default(), ExactMatchVerifier);

    let user = app
        .dev_login(DevLoginInput {
            email: "manual-owner@example.com".into(),
            display_name: "Manual Owner".into(),
        })
        .expect("dev login should succeed");

    let challenge_view = app
        .create_weibo_challenge(CreateWeiboChallengeInput {
            human_user_id: user.id,
            desired_handle: "@manual-agent".into(),
            desired_display_name: "Manual Agent".into(),
            persona: "careful operator".into(),
            weibo_handle: "manual_weibo".into(),
        })
        .expect("challenge creation should succeed");

    let result = app
        .verify_weibo_challenge(VerifyWeiboChallengeInput {
            challenge_id: challenge_view.challenge.id,
            submitted_text: challenge_view.challenge.template_text.clone(),
            source_url: Some("https://weibo.example/manual-post-1".into()),
        })
        .expect("manual-style verification should succeed");

    assert_eq!(result.verification_mode, "manual");
    assert_eq!(
        result.verification_evidence.as_deref(),
        Some("verified via https://weibo.example/manual-post-1")
    );
    assert!(result.agent_key_plaintext.starts_with("agk_"));
}

#[test]
fn exact_match_verifier_rejects_mutated_submitted_text() {
    let app = PlatformApp::with_verifier(MemoryPlatformRepository::default(), ExactMatchVerifier);

    let user = app
        .dev_login(DevLoginInput {
            email: "manual-reject-owner@example.com".into(),
            display_name: "Manual Reject Owner".into(),
        })
        .expect("dev login should succeed");

    let challenge_view = app
        .create_weibo_challenge(CreateWeiboChallengeInput {
            human_user_id: user.id,
            desired_handle: "@manual-reject-agent".into(),
            desired_display_name: "Manual Reject Agent".into(),
            persona: "careful operator".into(),
            weibo_handle: "manual_reject_weibo".into(),
        })
        .expect("challenge creation should succeed");

    let error = app
        .verify_weibo_challenge(VerifyWeiboChallengeInput {
            challenge_id: challenge_view.challenge.id,
            submitted_text: format!("{} #extra", challenge_view.challenge.template_text),
            source_url: Some("https://weibo.example/manual-post-2".into()),
        })
        .expect_err("manual-style verification should reject mutated text");

    assert!(matches!(error, AppError::Validation(message) if message.contains("exactly match")));
}

#[test]
fn create_weibo_challenge_rejects_existing_handle() {
    let app = PlatformApp::new(MemoryPlatformRepository::default());

    let user = app
        .dev_login(DevLoginInput {
            email: "duplicate-owner@example.com".into(),
            display_name: "Duplicate Owner".into(),
        })
        .expect("dev login should succeed");

    let first = app
        .create_weibo_challenge(CreateWeiboChallengeInput {
            human_user_id: user.id,
            desired_handle: "@duplicate-agent".into(),
            desired_display_name: "Duplicate Agent".into(),
            persona: "first persona".into(),
            weibo_handle: "duplicate_owner_weibo".into(),
        })
        .expect("challenge creation should succeed");

    app.verify_weibo_challenge(VerifyWeiboChallengeInput {
        challenge_id: first.challenge.id,
        submitted_text: first.challenge.template_text.clone(),
        source_url: Some("https://weibo.example/duplicate-post-1".into()),
    })
    .expect("first verification should succeed");

    let error = app
        .create_weibo_challenge(CreateWeiboChallengeInput {
            human_user_id: user.id,
            desired_handle: "@duplicate-agent".into(),
            desired_display_name: "Duplicate Agent Two".into(),
            persona: "second persona".into(),
            weibo_handle: "duplicate_owner_weibo".into(),
        })
        .expect_err("duplicate handle should be rejected before challenge creation");

    assert!(matches!(
        error,
        AppError::Conflict(message)
            if message.contains("already registered to this owner")
    ));
}

#[test]
fn failed_registration_completion_does_not_leave_half_written_agent() {
    #[derive(Clone, Default)]
    struct FailingCompletionRepo {
        inner: MemoryPlatformRepository,
    }

    impl PlatformRepository for FailingCompletionRepo {
        fn find_human_user_by_email(&self, email: &str) -> Option<HumanUser> {
            self.inner.find_human_user_by_email(email)
        }

        fn get_human_user(&self, user_id: Uuid) -> Option<HumanUser> {
            self.inner.get_human_user(user_id)
        }

        fn list_human_users(&self) -> Vec<HumanUser> {
            self.inner.list_human_users()
        }

        fn insert_human_user(&self, user: HumanUser) -> AppResult<HumanUser> {
            self.inner.insert_human_user(user)
        }

        fn human_user_exists(&self, user_id: Uuid) -> bool {
            self.inner.human_user_exists(user_id)
        }

        fn insert_registration_request(&self, request: AgentRegistrationRequest) -> AppResult<()> {
            self.inner.insert_registration_request(request)
        }

        fn insert_ownership_challenge(&self, challenge: OwnershipProofChallenge) -> AppResult<()> {
            self.inner.insert_ownership_challenge(challenge)
        }

        fn get_challenge(&self, challenge_id: Uuid) -> Option<OwnershipProofChallenge> {
            self.inner.get_challenge(challenge_id)
        }

        fn list_challenges(&self) -> Vec<OwnershipProofChallenge> {
            self.inner.list_challenges()
        }

        fn update_challenge(&self, challenge: OwnershipProofChallenge) -> AppResult<()> {
            self.inner.update_challenge(challenge)
        }

        fn get_registration_request(&self, request_id: Uuid) -> Option<AgentRegistrationRequest> {
            self.inner.get_registration_request(request_id)
        }

        fn list_registration_requests(&self) -> Vec<AgentRegistrationRequest> {
            self.inner.list_registration_requests()
        }

        fn update_registration_request(&self, request: AgentRegistrationRequest) -> AppResult<()> {
            self.inner.update_registration_request(request)
        }

        fn insert_agent_profile(&self, profile: AgentProfile) -> AppResult<()> {
            self.inner.insert_agent_profile(profile)
        }

        fn complete_registration_bundle(
            &self,
            _bundle: RegistrationCompletionBundle,
        ) -> AppResult<()> {
            Err(AppError::Validation("simulated completion failure".into()))
        }

        fn list_agent_profiles(&self) -> Vec<AgentProfile> {
            self.inner.list_agent_profiles()
        }

        fn update_agent_profile_status(
            &self,
            agent_id: Uuid,
            status: AgentStatus,
        ) -> AppResult<()> {
            self.inner.update_agent_profile_status(agent_id, status)
        }

        fn update_agent_profile(
            &self,
            agent_id: Uuid,
            display_name: String,
            persona: String,
            collaboration_preference: String,
        ) -> AppResult<()> {
            self.inner.update_agent_profile(
                agent_id,
                display_name,
                persona,
                collaboration_preference,
            )
        }

        fn insert_owner_binding(&self, binding: AgentOwnerBinding) -> AppResult<()> {
            self.inner.insert_owner_binding(binding)
        }

        fn insert_agent_key(&self, key_record: AgentKeyRecord) -> AppResult<()> {
            self.inner.insert_agent_key(key_record)
        }

        fn find_agent_key_by_plaintext(&self, plaintext_key: &str) -> Option<AgentKeyRecord> {
            self.inner.find_agent_key_by_plaintext(plaintext_key)
        }

        fn list_agent_keys(&self, agent_id: Uuid) -> Vec<AgentKeyRecord> {
            self.inner.list_agent_keys(agent_id)
        }

        fn touch_agent_key_usage(
            &self,
            key_id: Uuid,
            used_at: chrono::DateTime<chrono::Utc>,
        ) -> AppResult<()> {
            self.inner.touch_agent_key_usage(key_id, used_at)
        }

        fn revoke_agent_keys(
            &self,
            agent_id: Uuid,
            revoked_at: chrono::DateTime<chrono::Utc>,
        ) -> AppResult<()> {
            self.inner.revoke_agent_keys(agent_id, revoked_at)
        }

        fn insert_agent_key_issue_log(&self, log: AgentKeyIssueLog) -> AppResult<()> {
            self.inner.insert_agent_key_issue_log(log)
        }

        fn list_agent_key_issue_logs(&self, agent_id: Uuid, limit: usize) -> Vec<AgentKeyIssueLog> {
            self.inner.list_agent_key_issue_logs(agent_id, limit)
        }

        fn insert_social_proof_submission(
            &self,
            submission: SocialProofSubmission,
        ) -> AppResult<()> {
            self.inner.insert_social_proof_submission(submission)
        }

        fn list_social_proof_submissions(&self) -> Vec<SocialProofSubmission> {
            self.inner.list_social_proof_submissions()
        }

        fn insert_agent_inbox_event(&self, event: AgentInboxEvent) -> AppResult<()> {
            self.inner.insert_agent_inbox_event(event)
        }

        fn list_agent_inbox_events(
            &self,
            agent_id: Uuid,
            status: Option<AgentInboxEventStatus>,
            limit: usize,
        ) -> Vec<AgentInboxEvent> {
            self.inner.list_agent_inbox_events(agent_id, status, limit)
        }

        fn get_agent_inbox_event(&self, event_id: Uuid) -> Option<AgentInboxEvent> {
            self.inner.get_agent_inbox_event(event_id)
        }

        fn update_agent_inbox_event(&self, event: AgentInboxEvent) -> AppResult<()> {
            self.inner.update_agent_inbox_event(event)
        }

        fn insert_agent_action_log(&self, log: AgentActionLog) -> AppResult<()> {
            self.inner.insert_agent_action_log(log)
        }

        fn list_all_agent_action_logs(&self) -> Vec<AgentActionLog> {
            self.inner.list_all_agent_action_logs()
        }

        fn ensure_agent_conversation_bucket(&self, agent_id: Uuid) -> AppResult<()> {
            self.inner.ensure_agent_conversation_bucket(agent_id)
        }

        fn insert_conversation_preview(
            &self,
            agent_id: Uuid,
            preview: ConversationPreview,
        ) -> AppResult<()> {
            self.inner.insert_conversation_preview(agent_id, preview)
        }

        fn list_owner_agents(&self, human_user_id: Uuid) -> Vec<AgentProfile> {
            self.inner.list_owner_agents(human_user_id)
        }

        fn agent_exists(&self, agent_id: Uuid) -> bool {
            self.inner.agent_exists(agent_id)
        }

        fn get_agent_profile(&self, agent_id: Uuid) -> Option<AgentProfile> {
            self.inner.get_agent_profile(agent_id)
        }

        fn list_agent_conversations(&self, agent_id: Uuid) -> Vec<ConversationPreview> {
            self.inner.list_agent_conversations(agent_id)
        }

        fn get_conversation_messages(&self, conversation_id: Uuid) -> Vec<MessageView> {
            self.inner.get_conversation_messages(conversation_id)
        }

        fn append_message(&self, message: MessageView) -> AppResult<()> {
            self.inner.append_message(message)
        }

        fn conversation_exists(&self, conversation_id: Uuid) -> bool {
            self.inner.conversation_exists(conversation_id)
        }

        fn list_agent_action_logs(&self, agent_id: Uuid, limit: usize) -> Vec<AgentActionLog> {
            self.inner.list_agent_action_logs(agent_id, limit)
        }
    }

    let app = PlatformApp::with_verifier(FailingCompletionRepo::default(), ExactMatchVerifier);

    let owner = app
        .dev_login(DevLoginInput {
            email: "failing-completion-owner@example.com".into(),
            display_name: "Failing Completion Owner".into(),
        })
        .expect("owner login should succeed");

    let challenge = app
        .create_weibo_challenge(CreateWeiboChallengeInput {
            human_user_id: owner.id,
            desired_handle: "@half-written-agent".into(),
            desired_display_name: "Half Written Agent".into(),
            persona: "should not be partially persisted".into(),
            weibo_handle: "half_written_weibo".into(),
        })
        .expect("challenge creation should succeed");

    let error = app
        .verify_weibo_challenge(VerifyWeiboChallengeInput {
            challenge_id: challenge.challenge.id,
            submitted_text: challenge.challenge.template_text.clone(),
            source_url: Some("https://weibo.example/half-written".into()),
        })
        .expect_err("verification should fail during completion");

    assert!(matches!(
        error,
        AppError::Validation(message) if message.contains("simulated completion failure")
    ));

    assert!(app
        .find_owner_agent_by_handle(owner.id, "@half-written-agent")
        .expect("lookup should succeed")
        .is_none());
}
