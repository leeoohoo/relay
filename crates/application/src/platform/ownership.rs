use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn create_weibo_challenge(
        &self,
        input: CreateWeiboChallengeInput,
    ) -> AppResult<ChallengeView> {
        if !self.repo.human_user_exists(input.human_user_id) {
            return Err(AppError::NotFound("human user not found".into()));
        }

        if input.desired_handle.trim().is_empty() || input.desired_display_name.trim().is_empty() {
            return Err(AppError::Validation(
                "desired_handle and desired_display_name are required".into(),
            ));
        }

        let normalized_handle = normalize_handle(&input.desired_handle);
        if self
            .repo
            .list_agent_profiles()
            .into_iter()
            .any(|profile| profile.handle == normalized_handle)
        {
            return Err(self.agent_handle_conflict_error(input.human_user_id, &normalized_handle));
        }

        let registration = AgentRegistrationRequest {
            id: Uuid::new_v4(),
            human_user_id: input.human_user_id,
            desired_handle: normalized_handle,
            desired_display_name: input.desired_display_name.trim().to_string(),
            persona: input.persona.trim().to_string(),
            weibo_handle: input.weibo_handle.trim().to_string(),
            status: RegistrationStatus::PendingProof,
            created_at: now_utc(),
        };

        let code = generate_code();
        let template_text = format!("我正在验证我的智能体接入资格，验证码：{}", code);

        let challenge = OwnershipProofChallenge {
            id: Uuid::new_v4(),
            human_user_id: input.human_user_id,
            registration_request_id: registration.id,
            provider: OwnershipProofProvider::Weibo,
            account_handle: input.weibo_handle.trim().to_string(),
            verification_code: code,
            template_text,
            expires_at: now_utc() + Duration::minutes(20),
            status: ChallengeStatus::Pending,
            created_at: now_utc(),
        };

        self.repo
            .insert_registration_request(registration.clone())?;
        self.repo.insert_ownership_challenge(challenge.clone())?;

        Ok(ChallengeView {
            registration_request_id: registration.id,
            challenge,
            verification_instructions:
                "请将 challenge.template_text 原样发布到对应微博账号，并在验证时提交微博正文和链接。"
                    .into(),
        })
    }

    pub fn verify_weibo_challenge(
        &self,
        input: VerifyWeiboChallengeInput,
    ) -> AppResult<VerifyChallengeResult> {
        let (challenge, registration) =
            self.load_pending_registration_for_challenge(input.challenge_id)?;

        if now_utc() > challenge.expires_at {
            let mut expired = challenge;
            expired.status = ChallengeStatus::Expired;
            self.repo.update_challenge(expired)?;
            return Err(AppError::Validation("challenge has expired".into()));
        }

        let verification = self
            .verifier
            .verify_weibo_submission(OwnershipVerificationInput {
                challenge: challenge.clone(),
                submitted_text: input.submitted_text.clone(),
                source_url: input.source_url.clone(),
            })?;

        let submission = SocialProofSubmission {
            id: Uuid::new_v4(),
            challenge_id: challenge.id,
            submitted_text: input.submitted_text,
            source_url: input.source_url,
            provider_post_id: verification.provider_post_id,
            verification_mode: verification.verification_mode.clone(),
            verification_evidence: verification.verification_evidence.clone(),
            raw_payload: if verification.raw_payload.is_null() {
                json!({})
            } else {
                verification.raw_payload
            },
            created_at: now_utc(),
        };

        self.complete_challenge_verification(
            challenge,
            registration,
            submission,
            verification.verification_mode,
            verification.verification_evidence,
        )
    }

    pub fn verify_owned_weibo_challenge(
        &self,
        human_user_id: Uuid,
        input: VerifyWeiboChallengeInput,
    ) -> AppResult<VerifyChallengeResult> {
        let challenge = self
            .repo
            .get_challenge(input.challenge_id)
            .ok_or_else(|| AppError::NotFound("ownership challenge not found".into()))?;
        if challenge.human_user_id != human_user_id {
            return Err(AppError::Unauthorized(
                "ownership challenge belongs to another human user".into(),
            ));
        }
        self.verify_weibo_challenge(input)
    }

    pub fn admin_approve_challenge(
        &self,
        input: AdminChallengeDecisionInput,
    ) -> AppResult<VerifyChallengeResult> {
        let note = sanitize_optional_note(input.note);
        let (challenge, registration) =
            self.load_pending_registration_for_challenge(input.challenge_id)?;
        let source_url = input.source_url;

        let submission = SocialProofSubmission {
            id: Uuid::new_v4(),
            challenge_id: challenge.id,
            submitted_text: challenge.template_text.clone(),
            source_url: source_url.clone(),
            provider_post_id: None,
            verification_mode: "admin_manual_approval".into(),
            verification_evidence: Some(match (&note, &source_url) {
                (Some(note), Some(source_url)) => {
                    format!("platform admin approved manually: {note}; source={source_url}")
                }
                (Some(note), None) => format!("platform admin approved manually: {note}"),
                (None, Some(source_url)) => {
                    format!("platform admin approved manually; source={source_url}")
                }
                (None, None) => "platform admin approved manually".into(),
            }),
            raw_payload: json!({
                "mode": "admin_manual_approval",
                "note": note,
                "source_url": source_url,
            }),
            created_at: now_utc(),
        };

        let result = self.complete_challenge_verification(
            challenge.clone(),
            registration,
            submission,
            "admin_manual_approval".into(),
            Some("由平台运营人工审核通过".into()),
        )?;

        self.record_registration_level_admin_action(
            challenge.human_user_id,
            "admin.approve_challenge",
            challenge.id,
            challenge.registration_request_id,
            note,
            source_url,
        )?;

        Ok(result)
    }

    pub fn admin_reject_challenge(&self, input: AdminChallengeDecisionInput) -> AppResult<()> {
        let note = sanitize_optional_note(input.note);
        let (mut challenge, mut registration) =
            self.load_pending_registration_for_challenge(input.challenge_id)?;

        challenge.status = ChallengeStatus::Expired;
        registration.status = RegistrationStatus::Rejected;
        self.repo.update_challenge(challenge.clone())?;
        self.repo
            .update_registration_request(registration.clone())?;

        self.record_registration_level_admin_action(
            registration.human_user_id,
            "admin.reject_challenge",
            challenge.id,
            registration.id,
            note,
            input.source_url,
        )
    }

    pub fn list_owner_agents(&self, human_user_id: Uuid) -> AppResult<Vec<AgentProfile>> {
        Ok(self.repo.list_owner_agents(human_user_id))
    }

    pub fn ensure_human_owns_agent(
        &self,
        human_user_id: Uuid,
        agent_id: Uuid,
    ) -> AppResult<AgentProfile> {
        let agent = self
            .repo
            .get_agent_profile(agent_id)
            .ok_or_else(|| AppError::NotFound("agent not found".into()))?;
        if agent.owner_user_id != human_user_id {
            return Err(AppError::Unauthorized(
                "human user does not own this agent".into(),
            ));
        }
        Ok(agent)
    }

    pub fn list_owned_agent_conversations(
        &self,
        human_user_id: Uuid,
        agent_id: Uuid,
    ) -> AppResult<Vec<ConversationPreview>> {
        self.ensure_human_owns_agent(human_user_id, agent_id)?;
        self.list_agent_conversations(agent_id)
    }

    pub fn get_owned_conversation_messages(
        &self,
        human_user_id: Uuid,
        conversation_id: Uuid,
    ) -> AppResult<Vec<MessageView>> {
        let company_id = self
            .repo
            .get_conversation_context_result(conversation_id)?
            .and_then(|context| context.company_id);
        let company_membership = match company_id {
            Some(company_id) => self
                .repo
                .get_company_human_member_result(company_id, human_user_id)?
                .is_some_and(|membership| membership.status == "active"),
            None => false,
        };
        let owns_agent_membership =
            self.repo
                .list_owner_agents(human_user_id)
                .into_iter()
                .any(|agent| {
                    self.repo
                        .list_agent_conversations(agent.id)
                        .iter()
                        .any(|conversation| conversation.id == conversation_id)
                });
        if !company_membership && !owns_agent_membership {
            return Err(AppError::Unauthorized(
                "human user cannot access this conversation".into(),
            ));
        }
        self.get_conversation_messages(conversation_id)
    }

    pub fn get_owned_conversation_message_page(
        &self,
        human_user_id: Uuid,
        conversation_id: Uuid,
        before_message_id: Option<Uuid>,
        limit: usize,
    ) -> AppResult<MessagePageView> {
        let company_id = self
            .repo
            .get_conversation_context_result(conversation_id)?
            .and_then(|context| context.company_id);
        let company_membership = match company_id {
            Some(company_id) => self
                .repo
                .get_company_human_member_result(company_id, human_user_id)?
                .is_some_and(|membership| membership.status == "active"),
            None => false,
        };
        let owns_agent_membership =
            self.repo
                .list_owner_agents(human_user_id)
                .into_iter()
                .any(|agent| {
                    self.repo
                        .list_agent_conversations(agent.id)
                        .iter()
                        .any(|conversation| conversation.id == conversation_id)
                });
        if !company_membership && !owns_agent_membership {
            return Err(AppError::Unauthorized(
                "human user cannot access this conversation".into(),
            ));
        }
        self.repo.get_conversation_message_page(
            conversation_id,
            before_message_id,
            normalize_message_page_limit(limit),
        )
    }

    pub fn find_owner_agent_by_handle(
        &self,
        human_user_id: Uuid,
        handle: &str,
    ) -> AppResult<Option<AgentProfile>> {
        let normalized = normalize_handle(handle);
        if normalized.is_empty() {
            return Ok(None);
        }

        Ok(self
            .repo
            .list_owner_agents(human_user_id)
            .into_iter()
            .find(|agent| agent.handle == normalized))
    }

    pub fn freeze_owned_agent(
        &self,
        human_user_id: Uuid,
        agent_id: Uuid,
    ) -> AppResult<AgentProfile> {
        self.set_owned_agent_status(human_user_id, agent_id, AgentStatus::Frozen)
    }

    pub fn unfreeze_owned_agent(
        &self,
        human_user_id: Uuid,
        agent_id: Uuid,
    ) -> AppResult<AgentProfile> {
        self.set_owned_agent_status(human_user_id, agent_id, AgentStatus::Active)
    }

    pub fn authenticate_agent_key(&self, plaintext_key: &str) -> AppResult<AgentProfile> {
        let key_record = self
            .repo
            .find_agent_key_by_plaintext(plaintext_key)
            .ok_or_else(|| AppError::Unauthorized("invalid agent key".into()))?;
        if key_record
            .expires_at
            .is_some_and(|expires_at| now_utc() >= expires_at)
        {
            return Err(AppError::Unauthorized("agent key has expired".into()));
        }
        self.repo.touch_agent_key_usage(key_record.id, now_utc())?;

        let agent = self
            .repo
            .get_agent_profile(key_record.agent_profile_id)
            .ok_or_else(|| AppError::Unauthorized("agent for key not found".into()))?;

        Ok(agent)
    }

    pub fn get_agent_profile_by_id(&self, agent_id: Uuid) -> AppResult<AgentProfile> {
        self.repo
            .get_agent_profile(agent_id)
            .ok_or_else(|| AppError::NotFound("agent not found".into()))
    }

    pub fn list_active_agents_for_worker(&self) -> AppResult<Vec<AgentProfile>> {
        self.repo.health_check()?;
        Ok(self
            .repo
            .list_agent_profiles()
            .into_iter()
            .filter(|agent| matches!(agent.status, AgentStatus::Active))
            .collect())
    }

    pub fn rotate_owned_agent_key(
        &self,
        human_user_id: Uuid,
        agent_id: Uuid,
    ) -> AppResult<RotateAgentKeyResult> {
        let human_user = self
            .repo
            .get_human_user_result(human_user_id)?
            .ok_or_else(|| AppError::NotFound("human user not found".into()))?;
        let agent = self
            .repo
            .get_agent_profile(agent_id)
            .ok_or_else(|| AppError::NotFound("agent not found".into()))?;

        if agent.owner_user_id != human_user.id {
            return Err(AppError::Unauthorized(
                "human user does not own this agent".into(),
            ));
        }

        self.rotate_agent_key_internal(
            agent_id,
            Some(human_user_id),
            "owner.rotate_agent_key",
            "rotated_by_owner",
            json!({
                "human_user_id": human_user_id,
                "agent_id": agent_id
            }),
            None,
        )
    }

    pub fn admin_freeze_agent(&self, agent_id: Uuid) -> AppResult<AgentProfile> {
        self.admin_freeze_agent_with_note(agent_id, None)
    }

    pub fn admin_freeze_agent_with_note(
        &self,
        agent_id: Uuid,
        note: Option<String>,
    ) -> AppResult<AgentProfile> {
        self.set_agent_status_internal(
            agent_id,
            AgentStatus::Frozen,
            "admin.freeze",
            json!({
                "agent_id": agent_id,
                "operator_role": "platform_admin",
                "note": sanitize_optional_note(note)
            }),
        )
    }

    pub fn admin_unfreeze_agent(&self, agent_id: Uuid) -> AppResult<AgentProfile> {
        self.admin_unfreeze_agent_with_note(agent_id, None)
    }

    pub fn admin_unfreeze_agent_with_note(
        &self,
        agent_id: Uuid,
        note: Option<String>,
    ) -> AppResult<AgentProfile> {
        self.set_agent_status_internal(
            agent_id,
            AgentStatus::Active,
            "admin.unfreeze",
            json!({
                "agent_id": agent_id,
                "operator_role": "platform_admin",
                "note": sanitize_optional_note(note)
            }),
        )
    }

    pub fn admin_rotate_agent_key(&self, agent_id: Uuid) -> AppResult<RotateAgentKeyResult> {
        self.admin_rotate_agent_key_with_note(agent_id, None)
    }

    pub fn admin_rotate_agent_key_with_note(
        &self,
        agent_id: Uuid,
        note: Option<String>,
    ) -> AppResult<RotateAgentKeyResult> {
        let note = sanitize_optional_note(note);
        self.rotate_agent_key_internal(
            agent_id,
            None,
            "admin.rotate_agent_key",
            "rotated_by_admin",
            json!({
                "agent_id": agent_id,
                "operator_role": "platform_admin",
                "note": note
            }),
            note,
        )
    }

    pub(super) fn rotate_agent_key_internal(
        &self,
        agent_id: Uuid,
        issued_by_user_id: Option<Uuid>,
        action_type: &'static str,
        revoke_reason: &'static str,
        request_payload: serde_json::Value,
        note: Option<String>,
    ) -> AppResult<RotateAgentKeyResult> {
        self.repo
            .get_agent_profile(agent_id)
            .ok_or_else(|| AppError::NotFound("agent not found".into()))?;

        let revoked_at = now_utc();
        let revoked_keys = self
            .repo
            .list_agent_keys(agent_id)
            .into_iter()
            .filter(agent_key_is_active_record)
            .collect::<Vec<_>>();
        self.repo.revoke_agent_keys(agent_id, revoked_at)?;
        for revoked_key in &revoked_keys {
            self.record_agent_key_issue(
                agent_id,
                Some(revoked_key.id),
                AgentKeyIssueType::Revoked,
                issued_by_user_id,
                json!({
                    "agent_key_prefix": revoked_key.key_prefix,
                    "reason": revoke_reason,
                    "note": note,
                    "revoked_at": revoked_at
                }),
            )?;
        }

        let plaintext_key = generate_agent_key();
        let key_prefix = plaintext_key.chars().take(12).collect::<String>();
        let key_created_at = now_utc();
        let key_record = AgentKeyRecord {
            id: Uuid::new_v4(),
            agent_profile_id: agent_id,
            key_name: "rotated".into(),
            key_prefix: key_prefix.clone(),
            key_hash: hash_secret(&plaintext_key),
            last_used_at: None,
            expires_at: Some(key_created_at + Duration::days(180)),
            revoked_at: None,
            created_at: key_created_at,
        };

        self.repo.insert_agent_key(key_record.clone())?;
        self.record_agent_key_issue(
            agent_id,
            Some(key_record.id),
            AgentKeyIssueType::Rotated,
            issued_by_user_id,
            json!({
                "agent_key_prefix": key_prefix,
                "revoked_key_count": revoked_keys.len(),
                "note": note
            }),
        )?;
        self.record_agent_action(
            agent_id,
            action_type,
            Some(format!("agent:{agent_id}")),
            request_payload,
            json!({
                "agent_key_prefix": key_prefix
            }),
            AgentActionStatus::Success,
        )?;

        Ok(RotateAgentKeyResult {
            agent_profile_id: agent_id,
            agent_key_plaintext: plaintext_key,
            agent_key_prefix: key_prefix,
        })
    }
}
