use super::*;

impl AgentPlatformRepository for MemoryPlatformRepository {
    fn insert_registration_request(&self, request: AgentRegistrationRequest) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.registration_requests.insert(request.id, request);
        Ok(())
    }

    fn insert_ownership_challenge(&self, challenge: OwnershipProofChallenge) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.challenges.insert(challenge.id, challenge);
        Ok(())
    }

    fn get_challenge(&self, challenge_id: Uuid) -> Option<OwnershipProofChallenge> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.challenges.get(&challenge_id).cloned()
    }

    fn list_challenges(&self) -> Vec<OwnershipProofChallenge> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut challenges = guard.challenges.values().cloned().collect::<Vec<_>>();
        challenges.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        challenges
    }

    fn update_challenge(&self, challenge: OwnershipProofChallenge) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.challenges.insert(challenge.id, challenge);
        Ok(())
    }

    fn get_registration_request(&self, request_id: Uuid) -> Option<AgentRegistrationRequest> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.registration_requests.get(&request_id).cloned()
    }

    fn list_registration_requests(&self) -> Vec<AgentRegistrationRequest> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut requests = guard
            .registration_requests
            .values()
            .cloned()
            .collect::<Vec<_>>();
        requests.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        requests
    }

    fn update_registration_request(&self, request: AgentRegistrationRequest) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.registration_requests.insert(request.id, request);
        Ok(())
    }

    fn insert_agent_profile(&self, profile: AgentProfile) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.agent_profiles.insert(profile.id, profile);
        Ok(())
    }

    fn complete_registration_bundle(&self, bundle: RegistrationCompletionBundle) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let agent_id = bundle.agent.id;

        guard
            .challenges
            .insert(bundle.challenge.id, bundle.challenge);
        guard
            .registration_requests
            .insert(bundle.registration.id, bundle.registration);
        guard.agent_profiles.insert(bundle.agent.id, bundle.agent);
        guard.owner_bindings.push(bundle.binding);

        let key_id = bundle.key_record.id;
        let key_hash = bundle.key_record.key_hash.clone();
        guard
            .agent_keys
            .insert(bundle.key_record.id, bundle.key_record);
        guard.agent_keys_by_hash.insert(key_hash, key_id);

        guard.agent_key_issue_logs.push(bundle.key_issue_log);
        guard.submissions.push(bundle.submission);
        guard.conversations.entry(agent_id).or_default();
        guard
            .conversations
            .entry(agent_id)
            .or_default()
            .push(bundle.self_notes_conversation);
        Ok(())
    }

    fn list_agent_profiles(&self) -> Vec<AgentProfile> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut profiles = guard.agent_profiles.values().cloned().collect::<Vec<_>>();
        profiles.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        profiles
    }

    fn update_agent_profile_status(&self, agent_id: Uuid, status: AgentStatus) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let Some(profile) = guard.agent_profiles.get_mut(&agent_id) else {
            return Err(ai_chat_shared::AppError::NotFound("agent not found".into()));
        };
        profile.status = status;
        Ok(())
    }

    fn update_agent_profile(
        &self,
        agent_id: Uuid,
        display_name: String,
        persona: String,
        collaboration_preference: String,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let Some(profile) = guard.agent_profiles.get_mut(&agent_id) else {
            return Err(ai_chat_shared::AppError::NotFound("agent not found".into()));
        };
        profile.display_name = display_name;
        profile.persona = persona;
        profile.collaboration_preference = collaboration_preference;
        Ok(())
    }

    fn insert_owner_binding(&self, binding: AgentOwnerBinding) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.owner_bindings.push(binding);
        Ok(())
    }

    fn insert_agent_key(&self, key_record: AgentKeyRecord) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let key_id = key_record.id;
        let key_hash = key_record.key_hash.clone();
        guard.agent_keys.insert(key_record.id, key_record);
        guard.agent_keys_by_hash.insert(key_hash, key_id);
        Ok(())
    }

    fn find_agent_key_by_plaintext(&self, plaintext_key: &str) -> Option<AgentKeyRecord> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let key_id = guard.agent_keys_by_hash.get(&hash_secret(plaintext_key))?;
        let key = guard.agent_keys.get(key_id)?;
        if key.revoked_at.is_some() {
            return None;
        }
        if key
            .expires_at
            .is_some_and(|expires_at| now_utc() >= expires_at)
        {
            return None;
        }
        Some(key.clone())
    }

    fn list_agent_keys(&self, agent_id: Uuid) -> Vec<AgentKeyRecord> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut keys = guard
            .agent_keys
            .values()
            .filter(|key| key.agent_profile_id == agent_id)
            .cloned()
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        keys
    }

    fn touch_agent_key_usage(
        &self,
        key_id: Uuid,
        used_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let Some(key) = guard.agent_keys.get_mut(&key_id) else {
            return Err(ai_chat_shared::AppError::NotFound(
                "agent key not found".into(),
            ));
        };
        key.last_used_at = Some(used_at);
        Ok(())
    }

    fn revoke_agent_keys(
        &self,
        agent_id: Uuid,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        for key in guard.agent_keys.values_mut() {
            if key.agent_profile_id == agent_id && key.revoked_at.is_none() {
                key.revoked_at = Some(revoked_at);
            }
        }
        Ok(())
    }

    fn insert_agent_key_issue_log(&self, log: AgentKeyIssueLog) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.agent_key_issue_logs.push(log);
        Ok(())
    }

    fn list_agent_key_issue_logs(&self, agent_id: Uuid, limit: usize) -> Vec<AgentKeyIssueLog> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut logs = guard
            .agent_key_issue_logs
            .iter()
            .filter(|item| item.agent_profile_id == agent_id)
            .cloned()
            .collect::<Vec<_>>();
        logs.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        logs.truncate(limit);
        logs
    }

    fn insert_social_proof_submission(&self, submission: SocialProofSubmission) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.submissions.push(submission);
        Ok(())
    }

    fn list_social_proof_submissions(&self) -> Vec<SocialProofSubmission> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut submissions = guard.submissions.clone();
        submissions.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        submissions
    }

    fn insert_agent_inbox_event(&self, event: AgentInboxEvent) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.inbox_events.insert(event.id, event);
        Ok(())
    }

    fn list_agent_inbox_events(
        &self,
        agent_id: Uuid,
        status: Option<AgentInboxEventStatus>,
        limit: usize,
    ) -> Vec<AgentInboxEvent> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut items = guard
            .inbox_events
            .values()
            .filter(|item| item.agent_profile_id == agent_id)
            .filter(|item| match status.as_ref() {
                Some(value) => item.status == *value,
                None => true,
            })
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by(|left, right| {
            left.priority
                .cmp(&right.priority)
                .then_with(|| right.created_at.cmp(&left.created_at))
        });
        items.truncate(limit);
        items
    }

    fn get_agent_inbox_event(&self, event_id: Uuid) -> Option<AgentInboxEvent> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.inbox_events.get(&event_id).cloned()
    }

    fn update_agent_inbox_event(&self, event: AgentInboxEvent) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.inbox_events.insert(event.id, event);
        Ok(())
    }

    fn insert_agent_action_log(&self, log: AgentActionLog) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.action_logs.push(log);
        Ok(())
    }

    fn list_all_agent_action_logs(&self) -> Vec<AgentActionLog> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut logs = guard.action_logs.clone();
        logs.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        logs
    }

    fn count_agent_actions_since(
        &self,
        agent_id: Uuid,
        since: chrono::DateTime<chrono::Utc>,
    ) -> usize {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .action_logs
            .iter()
            .filter(|log| log.agent_profile_id == agent_id && log.created_at >= since)
            .count()
    }

    fn get_agent_idempotency_record(
        &self,
        agent_id: Uuid,
        operation: &str,
        idempotency_key: &str,
    ) -> Option<AgentIdempotencyRecord> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .idempotency_records
            .get(&(agent_id, operation.to_string(), idempotency_key.to_string()))
            .cloned()
    }

    fn insert_agent_idempotency_record(&self, record: AgentIdempotencyRecord) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard
            .idempotency_records
            .entry((
                record.agent_profile_id,
                record.operation.clone(),
                record.idempotency_key.clone(),
            ))
            .or_insert(record);
        Ok(())
    }

    fn delete_expired_agent_idempotency_records(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let before = guard.idempotency_records.len();
        guard
            .idempotency_records
            .retain(|_, record| record.expires_at > now);
        Ok(before - guard.idempotency_records.len())
    }
}
