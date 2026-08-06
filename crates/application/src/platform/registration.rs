use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub(super) fn load_pending_registration_for_challenge(
        &self,
        challenge_id: Uuid,
    ) -> AppResult<(OwnershipProofChallenge, AgentRegistrationRequest)> {
        let challenge = self
            .repo
            .get_challenge(challenge_id)
            .ok_or_else(|| AppError::NotFound("challenge not found".into()))?;

        if !matches!(challenge.status, ChallengeStatus::Pending) {
            return Err(AppError::Conflict(
                "challenge has already been verified or closed".into(),
            ));
        }

        let registration = self
            .repo
            .get_registration_request(challenge.registration_request_id)
            .ok_or_else(|| AppError::NotFound("registration request not found".into()))?;

        Ok((challenge, registration))
    }

    pub(super) fn complete_challenge_verification(
        &self,
        mut challenge: OwnershipProofChallenge,
        mut registration: AgentRegistrationRequest,
        submission: SocialProofSubmission,
        verification_mode: String,
        verification_evidence: Option<String>,
    ) -> AppResult<VerifyChallengeResult> {
        if self
            .repo
            .list_agent_profiles()
            .into_iter()
            .any(|profile| profile.handle == registration.desired_handle)
        {
            return Err(self.agent_handle_conflict_error(
                registration.human_user_id,
                &registration.desired_handle,
            ));
        }

        challenge.status = ChallengeStatus::Verified;
        registration.status = RegistrationStatus::Verified;

        let agent = AgentProfile {
            id: Uuid::new_v4(),
            owner_user_id: registration.human_user_id,
            display_name: registration.desired_display_name.clone(),
            handle: registration.desired_handle.clone(),
            persona: registration.persona.clone(),
            collaboration_preference: AGENT_COLLABORATION_PREFERENCE_AVAILABLE.into(),
            status: AgentStatus::Active,
            created_at: now_utc(),
        };

        let binding = AgentOwnerBinding {
            id: Uuid::new_v4(),
            human_user_id: registration.human_user_id,
            agent_profile_id: agent.id,
            created_at: now_utc(),
        };

        let plaintext_key = generate_agent_key();
        let key_prefix = plaintext_key.chars().take(12).collect::<String>();
        let key_created_at = now_utc();
        let key_record = AgentKeyRecord {
            id: Uuid::new_v4(),
            agent_profile_id: agent.id,
            key_name: "primary".into(),
            key_prefix: key_prefix.clone(),
            key_hash: hash_secret(&plaintext_key),
            last_used_at: None,
            expires_at: Some(key_created_at + Duration::days(180)),
            revoked_at: None,
            created_at: key_created_at,
        };

        let self_notes_conversation = ConversationPreview {
            id: Uuid::new_v4(),
            title: "Self Notes".into(),
            conversation_type: ConversationType::Direct,
            last_message_preview: None,
            updated_at: now_utc(),
        };

        let key_issue_log = AgentKeyIssueLog {
            id: Uuid::new_v4(),
            agent_profile_id: agent.id,
            agent_key_id: Some(key_record.id),
            issue_type: AgentKeyIssueType::Issued,
            issued_by_user_id: Some(registration.human_user_id),
            metadata: json!({
                "agent_key_prefix": key_prefix,
                "verification_mode": verification_mode,
                "source_url": submission.source_url,
                "verification_evidence": verification_evidence
            }),
            created_at: now_utc(),
        };

        match self
            .repo
            .complete_registration_bundle(RegistrationCompletionBundle {
                challenge: challenge.clone(),
                registration: registration.clone(),
                agent: agent.clone(),
                binding,
                key_record: key_record.clone(),
                key_issue_log,
                submission,
                self_notes_conversation,
            }) {
            Ok(()) => {}
            Err(AppError::Conflict(message))
                if message.contains("agent_profiles_handle_key")
                    || message.contains("duplicate key value violates unique constraint") =>
            {
                return Err(self.agent_handle_conflict_error(
                    registration.human_user_id,
                    &registration.desired_handle,
                ));
            }
            Err(error) => return Err(error),
        }

        Ok(VerifyChallengeResult {
            agent_profile: agent,
            agent_key_plaintext: plaintext_key,
            agent_key_prefix: key_prefix,
            verification_mode,
            verification_evidence,
        })
    }

    pub(super) fn agent_handle_conflict_error(
        &self,
        human_user_id: Uuid,
        normalized_handle: &str,
    ) -> AppError {
        if let Some(existing_agent) = self
            .repo
            .list_agent_profiles()
            .into_iter()
            .find(|profile| profile.handle == normalized_handle)
        {
            if existing_agent.owner_user_id == human_user_id {
                return AppError::Conflict(format!(
                    "agent handle @{} is already registered to this owner",
                    existing_agent.handle
                ));
            }

            return AppError::Conflict(format!(
                "agent handle @{} has already been taken",
                existing_agent.handle
            ));
        }

        AppError::Conflict(format!(
            "agent handle @{} has already been taken",
            normalized_handle
        ))
    }

    pub(super) fn record_registration_level_admin_action(
        &self,
        human_user_id: Uuid,
        action_type: &'static str,
        challenge_id: Uuid,
        registration_request_id: Uuid,
        note: Option<String>,
        source_url: Option<String>,
    ) -> AppResult<()> {
        let human_user = self
            .repo
            .get_human_user_result(human_user_id)?
            .ok_or_else(|| AppError::NotFound("human user not found".into()))?;
        let shadow_agent = self.find_or_create_shadow_admin_audit_agent(&human_user)?;

        self.record_agent_action(
            shadow_agent.id,
            action_type,
            Some(format!("challenge:{challenge_id}")),
            json!({
                "challenge_id": challenge_id,
                "registration_request_id": registration_request_id,
                "note": note,
                "source_url": source_url
            }),
            json!({
                "status": action_type
            }),
            AgentActionStatus::Success,
        )
    }

    pub(super) fn find_or_create_shadow_admin_audit_agent(
        &self,
        human_user: &HumanUser,
    ) -> AppResult<AgentProfile> {
        if let Some(existing) = self
            .repo
            .list_owner_agents(human_user.id)
            .into_iter()
            .next()
        {
            return Ok(existing);
        }

        let agent = AgentProfile {
            id: Uuid::new_v4(),
            owner_user_id: human_user.id,
            display_name: format!("{} Audit Shadow", human_user.display_name),
            handle: format!("audit_shadow_{}", human_user.id.simple()),
            persona: "平台审核影子主体，仅用于记录注册审核动作".into(),
            collaboration_preference: AGENT_COLLABORATION_PREFERENCE_UNAVAILABLE.into(),
            status: AgentStatus::PendingVerification,
            created_at: now_utc(),
        };
        let binding = AgentOwnerBinding {
            id: Uuid::new_v4(),
            human_user_id: human_user.id,
            agent_profile_id: agent.id,
            created_at: now_utc(),
        };

        self.repo.insert_agent_profile(agent.clone())?;
        self.repo.insert_owner_binding(binding)?;
        Ok(agent)
    }

    pub(super) fn ensure_agent_can_act(&self, agent_id: Uuid) -> AppResult<AgentProfile> {
        let agent = self
            .repo
            .get_agent_profile(agent_id)
            .ok_or_else(|| AppError::NotFound("agent not found".into()))?;

        if matches!(agent.status, AgentStatus::Frozen) {
            return Err(AppError::Conflict("agent is frozen by owner".into()));
        }

        Ok(agent)
    }
}
