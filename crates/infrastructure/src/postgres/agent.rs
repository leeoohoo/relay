use super::mapping::*;
use super::*;

impl AgentPlatformRepository for PostgresPlatformRepository {
    fn insert_registration_request(&self, request: AgentRegistrationRequest) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_registration_requests (
                    id, human_user_id, desired_handle, desired_display_name, persona,
                    proof_provider, proof_account_handle, status, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, 'weibo', $6, $7, $8, $8)
                "#,
                &[
                    &request.id,
                    &request.human_user_id,
                    &request.desired_handle,
                    &request.desired_display_name,
                    &request.persona,
                    &request.weibo_handle,
                    &registration_status_to_str(&request.status),
                    &request.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn insert_ownership_challenge(&self, challenge: OwnershipProofChallenge) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO ownership_proof_challenges (
                    id, human_user_id, registration_request_id, provider,
                    account_handle, verification_code, template_text, expires_at, verified_at,
                    status, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NULL, $9, $10)
                "#,
                &[
                    &challenge.id,
                    &challenge.human_user_id,
                    &challenge.registration_request_id,
                    &ownership_provider_to_str(&challenge.provider),
                    &challenge.account_handle,
                    &challenge.verification_code,
                    &challenge.template_text,
                    &challenge.expires_at,
                    &challenge_status_to_str(&challenge.status),
                    &challenge.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_challenge(&self, challenge_id: Uuid) -> Option<OwnershipProofChallenge> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, human_user_id, registration_request_id, provider,
                       account_handle, verification_code, template_text, expires_at, status, created_at
                FROM ownership_proof_challenges
                WHERE id = $1
                "#,
                &[&challenge_id],
            )
        })
        .ok()
        .flatten()
        .map(map_challenge)
    }

    fn list_challenges(&self) -> Vec<OwnershipProofChallenge> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, human_user_id, registration_request_id, provider,
                       account_handle, verification_code, template_text, expires_at, status, created_at
                FROM ownership_proof_challenges
                ORDER BY created_at DESC
                "#,
                &[],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_challenge)
        .collect()
    }

    fn update_challenge(&self, challenge: OwnershipProofChallenge) -> AppResult<()> {
        let verified_at = if matches!(challenge.status, ChallengeStatus::Verified) {
            Some(Utc::now())
        } else {
            None
        };

        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE ownership_proof_challenges
                SET status = $2, expires_at = $3, verified_at = $4
                WHERE id = $1
                "#,
                &[
                    &challenge.id,
                    &challenge_status_to_str(&challenge.status),
                    &challenge.expires_at,
                    &verified_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_registration_request(&self, request_id: Uuid) -> Option<AgentRegistrationRequest> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, human_user_id, desired_handle, desired_display_name,
                       persona, proof_account_handle, status, created_at
                FROM agent_registration_requests
                WHERE id = $1
                "#,
                &[&request_id],
            )
        })
        .ok()
        .flatten()
        .map(map_registration_request)
    }

    fn list_registration_requests(&self) -> Vec<AgentRegistrationRequest> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, human_user_id, desired_handle, desired_display_name,
                       persona, proof_account_handle, status, created_at
                FROM agent_registration_requests
                ORDER BY created_at DESC
                "#,
                &[],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_registration_request)
        .collect()
    }

    fn update_registration_request(&self, request: AgentRegistrationRequest) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_registration_requests
                SET desired_handle = $2,
                    desired_display_name = $3,
                    persona = $4,
                    proof_account_handle = $5,
                    status = $6,
                    updated_at = NOW()
                WHERE id = $1
                "#,
                &[
                    &request.id,
                    &request.desired_handle,
                    &request.desired_display_name,
                    &request.persona,
                    &request.weibo_handle,
                    &registration_status_to_str(&request.status),
                ],
            )?;
            Ok(())
        })
    }

    fn insert_agent_profile(&self, profile: AgentProfile) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_profiles (
                    id, owner_user_id, handle, display_name, persona,
                    collaboration_preference, status,
                    visibility, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, 'public', $8, $8)
                "#,
                &[
                    &profile.id,
                    &profile.owner_user_id,
                    &profile.handle,
                    &profile.display_name,
                    &profile.persona,
                    &profile.collaboration_preference,
                    &agent_status_to_str(&profile.status),
                    &profile.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn complete_registration_bundle(&self, bundle: RegistrationCompletionBundle) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;

            tx.execute(
                r#"
                UPDATE ownership_proof_challenges
                SET status = $2, expires_at = $3, verified_at = NOW()
                WHERE id = $1
                "#,
                &[
                    &bundle.challenge.id,
                    &challenge_status_to_str(&bundle.challenge.status),
                    &bundle.challenge.expires_at,
                ],
            )?;

            tx.execute(
                r#"
                UPDATE agent_registration_requests
                SET desired_handle = $2,
                    desired_display_name = $3,
                    persona = $4,
                    proof_account_handle = $5,
                    status = $6,
                    updated_at = NOW()
                WHERE id = $1
                "#,
                &[
                    &bundle.registration.id,
                    &bundle.registration.desired_handle,
                    &bundle.registration.desired_display_name,
                    &bundle.registration.persona,
                    &bundle.registration.weibo_handle,
                    &registration_status_to_str(&bundle.registration.status),
                ],
            )?;

            tx.execute(
                r#"
                INSERT INTO agent_profiles (
                    id, owner_user_id, handle, display_name, persona,
                    collaboration_preference, status,
                    visibility, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, 'public', $8, $8)
                "#,
                &[
                    &bundle.agent.id,
                    &bundle.agent.owner_user_id,
                    &bundle.agent.handle,
                    &bundle.agent.display_name,
                    &bundle.agent.persona,
                    &bundle.agent.collaboration_preference,
                    &agent_status_to_str(&bundle.agent.status),
                    &bundle.agent.created_at,
                ],
            )?;

            tx.execute(
                r#"
                INSERT INTO agent_owner_bindings (
                    id, human_user_id, agent_profile_id, binding_role, created_at
                )
                VALUES ($1, $2, $3, 'owner', $4)
                "#,
                &[
                    &bundle.binding.id,
                    &bundle.binding.human_user_id,
                    &bundle.binding.agent_profile_id,
                    &bundle.binding.created_at,
                ],
            )?;

            tx.execute(
                r#"
                INSERT INTO agent_keys (
                    id, agent_profile_id, key_name, key_prefix, key_hash,
                    scopes, last_used_at, last_used_ip, expires_at, revoked_at, created_at
                )
                VALUES ($1, $2, $3, $4, $5, '[]'::jsonb, $6, NULL, $7, $8, $9)
                "#,
                &[
                    &bundle.key_record.id,
                    &bundle.key_record.agent_profile_id,
                    &bundle.key_record.key_name,
                    &bundle.key_record.key_prefix,
                    &bundle.key_record.key_hash,
                    &bundle.key_record.last_used_at,
                    &bundle.key_record.expires_at,
                    &bundle.key_record.revoked_at,
                    &bundle.key_record.created_at,
                ],
            )?;

            tx.execute(
                r#"
                INSERT INTO agent_key_issue_logs (
                    id, agent_profile_id, agent_key_id, issue_type,
                    issued_by_user_id, metadata, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
                &[
                    &bundle.key_issue_log.id,
                    &bundle.key_issue_log.agent_profile_id,
                    &bundle.key_issue_log.agent_key_id,
                    &agent_key_issue_type_to_str(&bundle.key_issue_log.issue_type),
                    &bundle.key_issue_log.issued_by_user_id,
                    &Json(bundle.key_issue_log.metadata.clone()),
                    &bundle.key_issue_log.created_at,
                ],
            )?;

            tx.execute(
                r#"
                INSERT INTO social_proof_submissions (
                    id, challenge_id, submitted_text, source_url, provider_post_id,
                    verification_mode, verification_evidence, raw_payload, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
                &[
                    &bundle.submission.id,
                    &bundle.submission.challenge_id,
                    &bundle.submission.submitted_text,
                    &bundle.submission.source_url,
                    &bundle.submission.provider_post_id,
                    &bundle.submission.verification_mode,
                    &bundle.submission.verification_evidence,
                    &Json(bundle.submission.raw_payload.clone()),
                    &bundle.submission.created_at,
                ],
            )?;

            let title: Option<&str> = if bundle.self_notes_conversation.title.trim().is_empty() {
                None
            } else {
                Some(bundle.self_notes_conversation.title.as_str())
            };

            tx.execute(
                r#"
                INSERT INTO conversations (
                    id, conversation_type, title, created_by_agent_id, status,
                    last_message_at, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, 'active', $5, $5, $5)
                ON CONFLICT (id) DO UPDATE
                SET title = COALESCE(EXCLUDED.title, conversations.title),
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &bundle.self_notes_conversation.id,
                    &conversation_type_to_str(&bundle.self_notes_conversation.conversation_type),
                    &title,
                    &bundle.agent.id,
                    &bundle.self_notes_conversation.updated_at,
                ],
            )?;

            tx.execute(
                r#"
                INSERT INTO conversation_members (
                    id, conversation_id, agent_profile_id, member_role, joined_at
                )
                VALUES ($1, $2, $3, 'member', $4)
                ON CONFLICT (conversation_id, agent_profile_id) DO NOTHING
                "#,
                &[
                    &Uuid::new_v4(),
                    &bundle.self_notes_conversation.id,
                    &bundle.agent.id,
                    &bundle.self_notes_conversation.updated_at,
                ],
            )?;

            tx.commit()?;
            Ok(())
        })
    }

    fn list_agent_profiles(&self) -> Vec<AgentProfile> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, owner_user_id, display_name, handle, persona,
                       collaboration_preference, status, created_at
                FROM agent_profiles
                ORDER BY created_at DESC
                "#,
                &[],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_profile)
        .collect()
    }

    fn update_agent_profile_status(&self, agent_id: Uuid, status: AgentStatus) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_profiles
                SET status = $2,
                    updated_at = NOW()
                WHERE id = $1
                "#,
                &[&agent_id, &agent_status_to_str(&status)],
            )?;
            Ok(())
        })
    }

    fn update_agent_profile(
        &self,
        agent_id: Uuid,
        display_name: String,
        persona: String,
        collaboration_preference: String,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_profiles
                SET display_name = $2,
                    persona = $3,
                    collaboration_preference = $4,
                    updated_at = NOW()
                WHERE id = $1
                "#,
                &[
                    &agent_id,
                    &display_name,
                    &persona,
                    &collaboration_preference,
                ],
            )?;
            Ok(())
        })
    }

    fn insert_owner_binding(&self, binding: AgentOwnerBinding) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_owner_bindings (
                    id, human_user_id, agent_profile_id, binding_role, created_at
                )
                VALUES ($1, $2, $3, 'owner', $4)
                "#,
                &[
                    &binding.id,
                    &binding.human_user_id,
                    &binding.agent_profile_id,
                    &binding.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn insert_agent_key(&self, key_record: AgentKeyRecord) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_keys (
                    id, agent_profile_id, key_name, key_prefix, key_hash,
                    scopes, last_used_at, last_used_ip, expires_at, revoked_at, created_at
                )
                VALUES ($1, $2, $3, $4, $5, '[]'::jsonb, $6, NULL, $7, $8, $9)
                "#,
                &[
                    &key_record.id,
                    &key_record.agent_profile_id,
                    &key_record.key_name,
                    &key_record.key_prefix,
                    &key_record.key_hash,
                    &key_record.last_used_at,
                    &key_record.expires_at,
                    &key_record.revoked_at,
                    &key_record.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn find_agent_key_by_plaintext(&self, plaintext_key: &str) -> Option<AgentKeyRecord> {
        let key_hash = hash_secret(plaintext_key);
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, agent_profile_id, key_name, key_prefix, key_hash,
                       last_used_at, expires_at, revoked_at, created_at
                FROM agent_keys
                WHERE key_hash = $1
                  AND revoked_at IS NULL
                  AND (expires_at IS NULL OR expires_at > NOW())
                "#,
                &[&key_hash],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_key_record)
    }

    fn list_agent_keys(&self, agent_id: Uuid) -> Vec<AgentKeyRecord> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, agent_profile_id, key_name, key_prefix, key_hash,
                       last_used_at, expires_at, revoked_at, created_at
                FROM agent_keys
                WHERE agent_profile_id = $1
                ORDER BY created_at DESC
                "#,
                &[&agent_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_key_record)
        .collect()
    }

    fn touch_agent_key_usage(
        &self,
        key_id: Uuid,
        used_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_keys
                SET last_used_at = $2
                WHERE id = $1
                "#,
                &[&key_id, &used_at],
            )?;
            Ok(())
        })
    }

    fn revoke_agent_keys(
        &self,
        agent_id: Uuid,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_keys
                SET revoked_at = $2
                WHERE agent_profile_id = $1
                  AND revoked_at IS NULL
                "#,
                &[&agent_id, &revoked_at],
            )?;
            Ok(())
        })
    }

    fn insert_agent_key_issue_log(&self, log: AgentKeyIssueLog) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_key_issue_logs (
                    id, agent_profile_id, agent_key_id, issue_type,
                    issued_by_user_id, metadata, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
                &[
                    &log.id,
                    &log.agent_profile_id,
                    &log.agent_key_id,
                    &agent_key_issue_type_to_str(&log.issue_type),
                    &log.issued_by_user_id,
                    &Json(log.metadata.clone()),
                    &log.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_agent_key_issue_logs(&self, agent_id: Uuid, limit: usize) -> Vec<AgentKeyIssueLog> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, agent_profile_id, agent_key_id, issue_type,
                       issued_by_user_id, metadata, created_at
                FROM agent_key_issue_logs
                WHERE agent_profile_id = $1
                ORDER BY created_at DESC
                LIMIT $2
                "#,
                &[&agent_id, &(limit as i64)],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_key_issue_log)
        .collect()
    }

    fn insert_social_proof_submission(&self, submission: SocialProofSubmission) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO social_proof_submissions (
                    id, challenge_id, submitted_text, source_url, provider_post_id,
                    verification_mode, verification_evidence, raw_payload, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
                &[
                    &submission.id,
                    &submission.challenge_id,
                    &submission.submitted_text,
                    &submission.source_url,
                    &submission.provider_post_id,
                    &submission.verification_mode,
                    &submission.verification_evidence,
                    &Json(submission.raw_payload.clone()),
                    &submission.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_social_proof_submissions(&self) -> Vec<SocialProofSubmission> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, challenge_id, submitted_text, source_url, provider_post_id,
                       verification_mode, verification_evidence, raw_payload, created_at
                FROM social_proof_submissions
                ORDER BY created_at DESC
                "#,
                &[],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_social_proof_submission)
        .collect()
    }

    fn insert_agent_inbox_event(&self, event: AgentInboxEvent) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_event_inbox (
                    id, agent_profile_id, event_type, event_class, requires_action,
                    wake_policy, dedupe_key, coalesce_key, causation_id, correlation_id,
                    payload_json, priority, available_at, expires_at, handled_by_run_id,
                    processed_at, status, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
                ON CONFLICT (agent_profile_id, dedupe_key)
                    WHERE dedupe_key IS NOT NULL AND status IN ('pending', 'processing')
                DO NOTHING
                "#,
                &[
                    &event.id,
                    &event.agent_profile_id,
                    &event.event_type,
                    &event.event_class,
                    &event.requires_action,
                    &event.wake_policy,
                    &event.dedupe_key,
                    &event.coalesce_key,
                    &event.causation_id,
                    &event.correlation_id,
                    &Json(event.payload_json.clone()),
                    &event.priority,
                    &event.available_at,
                    &event.expires_at,
                    &event.handled_by_run_id,
                    &event.processed_at,
                    &agent_inbox_event_status_to_str(&event.status),
                    &event.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_agent_inbox_events(
        &self,
        agent_id: Uuid,
        status: Option<AgentInboxEventStatus>,
        limit: usize,
    ) -> Vec<AgentInboxEvent> {
        self.with_client(|client| {
            if let Some(status) = status.as_ref() {
                client.query(
                    r#"
                    SELECT id, agent_profile_id, event_type, event_class, requires_action,
                           wake_policy, dedupe_key, coalesce_key, causation_id, correlation_id,
                           payload_json, priority, available_at, expires_at, handled_by_run_id,
                           processed_at, status, created_at
                    FROM agent_event_inbox
                    WHERE agent_profile_id = $1
                      AND status = $2
                    ORDER BY priority ASC, created_at DESC
                    LIMIT $3
                    "#,
                    &[
                        &agent_id,
                        &agent_inbox_event_status_to_str(status),
                        &(limit as i64),
                    ],
                )
            } else {
                client.query(
                    r#"
                    SELECT id, agent_profile_id, event_type, event_class, requires_action,
                           wake_policy, dedupe_key, coalesce_key, causation_id, correlation_id,
                           payload_json, priority, available_at, expires_at, handled_by_run_id,
                           processed_at, status, created_at
                    FROM agent_event_inbox
                    WHERE agent_profile_id = $1
                    ORDER BY priority ASC, created_at DESC
                    LIMIT $2
                    "#,
                    &[&agent_id, &(limit as i64)],
                )
            }
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_inbox_event)
        .collect()
    }

    fn get_agent_inbox_event(&self, event_id: Uuid) -> Option<AgentInboxEvent> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, agent_profile_id, event_type, event_class, requires_action,
                       wake_policy, dedupe_key, coalesce_key, causation_id, correlation_id,
                       payload_json, priority, available_at, expires_at, handled_by_run_id,
                       processed_at, status, created_at
                FROM agent_event_inbox
                WHERE id = $1
                "#,
                &[&event_id],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_inbox_event)
    }

    fn update_agent_inbox_event(&self, event: AgentInboxEvent) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_event_inbox
                SET event_type = $2,
                    event_class = $3,
                    requires_action = $4,
                    wake_policy = $5,
                    dedupe_key = $6,
                    coalesce_key = $7,
                    causation_id = $8,
                    correlation_id = $9,
                    payload_json = $10,
                    priority = $11,
                    available_at = $12,
                    expires_at = $13,
                    handled_by_run_id = $14,
                    processed_at = $15,
                    status = $16
                WHERE id = $1
                "#,
                &[
                    &event.id,
                    &event.event_type,
                    &event.event_class,
                    &event.requires_action,
                    &event.wake_policy,
                    &event.dedupe_key,
                    &event.coalesce_key,
                    &event.causation_id,
                    &event.correlation_id,
                    &Json(event.payload_json.clone()),
                    &event.priority,
                    &event.available_at,
                    &event.expires_at,
                    &event.handled_by_run_id,
                    &event.processed_at,
                    &agent_inbox_event_status_to_str(&event.status),
                ],
            )?;
            Ok(())
        })
    }

    fn insert_agent_action_log(&self, log: AgentActionLog) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_action_logs (
                    id, agent_profile_id, action_type, target_ref,
                    request_payload, result_payload, status, trace_id, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
                &[
                    &log.id,
                    &log.agent_profile_id,
                    &log.action_type,
                    &log.target_ref,
                    &Json(log.request_payload.clone()),
                    &Json(log.result_payload.clone()),
                    &agent_action_status_to_str(&log.status),
                    &log.trace_id,
                    &log.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_all_agent_action_logs(&self) -> Vec<AgentActionLog> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, agent_profile_id, action_type, target_ref,
                       request_payload, result_payload, status, trace_id, created_at
                FROM agent_action_logs
                ORDER BY created_at DESC
                "#,
                &[],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_action_log)
        .collect()
    }

    fn count_agent_actions_since(
        &self,
        agent_id: Uuid,
        since: chrono::DateTime<chrono::Utc>,
    ) -> usize {
        self.with_client(|client| {
            client.query_one(
                r#"
                SELECT COUNT(1)
                FROM agent_action_logs
                WHERE agent_profile_id = $1 AND created_at >= $2
                "#,
                &[&agent_id, &since],
            )
        })
        .map(|row| row.get::<_, i64>(0).max(0) as usize)
        .unwrap_or(0)
    }

    fn get_agent_idempotency_record(
        &self,
        agent_id: Uuid,
        operation: &str,
        idempotency_key: &str,
    ) -> Option<AgentIdempotencyRecord> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, agent_profile_id, operation, idempotency_key,
                       request_hash, response_json, expires_at, created_at
                FROM agent_idempotency_records
                WHERE agent_profile_id = $1
                  AND operation = $2
                  AND idempotency_key = $3
                  AND expires_at > NOW()
                "#,
                &[&agent_id, &operation, &idempotency_key],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_idempotency_record)
    }

    fn insert_agent_idempotency_record(&self, record: AgentIdempotencyRecord) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_idempotency_records (
                    id, agent_profile_id, operation, idempotency_key,
                    request_hash, response_json, expires_at, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (agent_profile_id, operation, idempotency_key) DO NOTHING
                "#,
                &[
                    &record.id,
                    &record.agent_profile_id,
                    &record.operation,
                    &record.idempotency_key,
                    &record.request_hash,
                    &Json(record.response_json.clone()),
                    &record.expires_at,
                    &record.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn delete_expired_agent_idempotency_records(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        self.with_client(|client| {
            client
                .execute(
                    "DELETE FROM agent_idempotency_records WHERE expires_at <= $1",
                    &[&now],
                )
                .map(|count| count as usize)
        })
    }
}
