use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn record_agent_action(
        &self,
        agent_profile_id: Uuid,
        action_type: impl Into<String>,
        target_ref: Option<String>,
        request_payload: serde_json::Value,
        result_payload: serde_json::Value,
        status: AgentActionStatus,
    ) -> AppResult<()> {
        if !self.repo.agent_exists(agent_profile_id) {
            return Err(AppError::NotFound("agent not found".into()));
        }

        let log = AgentActionLog {
            id: Uuid::new_v4(),
            agent_profile_id,
            action_type: action_type.into(),
            target_ref,
            request_payload,
            result_payload,
            status,
            trace_id: None,
            created_at: now_utc(),
        };

        self.repo.insert_agent_action_log(log)
    }

    pub fn enforce_agent_action_budget(&self, agent_id: Uuid) -> AppResult<()> {
        if !self.repo.agent_exists(agent_id) {
            return Err(AppError::NotFound("agent not found".into()));
        }

        let now = now_utc();
        let minute_count = self
            .repo
            .count_agent_actions_since(agent_id, now - Duration::minutes(1));
        if minute_count >= 60 {
            return Err(AppError::RateLimited(
                "agent write budget exceeded: 60 actions per minute".into(),
            ));
        }

        let daily_count = self
            .repo
            .count_agent_actions_since(agent_id, now - Duration::days(1));
        if daily_count >= 1_000 {
            return Err(AppError::RateLimited(
                "agent write budget exceeded: 1000 actions per day".into(),
            ));
        }
        Ok(())
    }

    pub fn replay_agent_idempotency(
        &self,
        agent_id: Uuid,
        operation: &str,
        idempotency_key: &str,
        request_payload: &serde_json::Value,
    ) -> AppResult<Option<serde_json::Value>> {
        let idempotency_key = normalize_idempotency_key(idempotency_key)?;
        let Some(record) =
            self.repo
                .get_agent_idempotency_record(agent_id, operation, &idempotency_key)
        else {
            return Ok(None);
        };
        if record.expires_at <= now_utc() {
            return Ok(None);
        }

        let request_hash = hash_json_payload(request_payload)?;
        if record.request_hash != request_hash {
            return Err(AppError::Conflict(
                "idempotency key was already used with a different request".into(),
            ));
        }
        Ok(Some(record.response_json))
    }

    pub fn store_agent_idempotency(
        &self,
        agent_id: Uuid,
        operation: &str,
        idempotency_key: &str,
        request_payload: &serde_json::Value,
        response_json: &serde_json::Value,
    ) -> AppResult<()> {
        let idempotency_key = normalize_idempotency_key(idempotency_key)?;
        let now = now_utc();
        self.repo.delete_expired_agent_idempotency_records(now)?;
        self.repo
            .insert_agent_idempotency_record(AgentIdempotencyRecord {
                id: Uuid::new_v4(),
                agent_profile_id: agent_id,
                operation: operation.to_string(),
                idempotency_key,
                request_hash: hash_json_payload(request_payload)?,
                response_json: response_json.clone(),
                expires_at: now + Duration::days(1),
                created_at: now,
            })
    }

    pub fn cleanup_expired_security_records(&self) -> AppResult<usize> {
        let now = now_utc();
        Ok(self.repo.delete_expired_agent_idempotency_records(now)?
            + self.repo.delete_expired_human_sessions(now)?
            + self.repo.delete_expired_human_account_tokens(now)?
            + self.repo.delete_expired_agent_codex_run_tokens(now)?)
    }

    pub fn list_agent_action_logs(
        &self,
        agent_id: Uuid,
        limit: usize,
    ) -> AppResult<Vec<AgentActionLog>> {
        if !self.repo.agent_exists(agent_id) {
            return Err(AppError::NotFound("agent not found".into()));
        }

        Ok(self.repo.list_agent_action_logs(agent_id, limit))
    }

    pub fn list_agent_inbox_events(
        &self,
        agent_id: Uuid,
        pending_only: bool,
        limit: usize,
    ) -> AppResult<Vec<AgentInboxEvent>> {
        if !self.repo.agent_exists(agent_id) {
            return Err(AppError::NotFound("agent not found".into()));
        }

        let status = if pending_only {
            Some(AgentInboxEventStatus::Pending)
        } else {
            None
        };
        Ok(self.repo.list_agent_inbox_events(agent_id, status, limit))
    }

    pub fn mark_agent_inbox_event_processed(
        &self,
        input: MarkInboxEventProcessedInput,
    ) -> AppResult<AgentInboxEvent> {
        self.ensure_agent_can_act(input.actor_agent_id)?;

        let mut event = self
            .repo
            .get_agent_inbox_event(input.event_id)
            .ok_or_else(|| AppError::NotFound("agent inbox event not found".into()))?;
        if event.agent_profile_id != input.actor_agent_id {
            return Err(AppError::Unauthorized(
                "agent can only process its own inbox event".into(),
            ));
        }

        if event.event_type == "company.project.rule_generation_requested" {
            let project_id = payload_uuid_field_optional(&event.payload_json, "project_id")
                .ok_or_else(|| {
                    AppError::Validation(
                        "project rule generation event is missing project_id".into(),
                    )
                })?;
            let completed = self
                .repo
                .get_company_project_rule(project_id)
                .is_some_and(|rule| {
                    rule.updated_by_agent_id == Some(input.actor_agent_id)
                        && rule.updated_at >= event.created_at
                        && !rule.content.trim().is_empty()
                });
            if !completed {
                return Err(AppError::Conflict(
                    "project rule generation event cannot be acknowledged before this Agent successfully calls company.project rule_update with non-empty content"
                        .into(),
                ));
            }
        }

        event.status = AgentInboxEventStatus::Processed;
        event.processed_at = Some(now_utc());
        self.repo.update_agent_inbox_event(event.clone())?;
        Ok(event)
    }

    pub(super) fn record_agent_key_issue(
        &self,
        agent_profile_id: Uuid,
        agent_key_id: Option<Uuid>,
        issue_type: AgentKeyIssueType,
        issued_by_user_id: Option<Uuid>,
        metadata: serde_json::Value,
    ) -> AppResult<()> {
        self.repo.insert_agent_key_issue_log(AgentKeyIssueLog {
            id: Uuid::new_v4(),
            agent_profile_id,
            agent_key_id,
            issue_type,
            issued_by_user_id,
            metadata,
            created_at: now_utc(),
        })
    }

    pub(super) fn enqueue_agent_event(
        &self,
        agent_profile_id: Uuid,
        event_type: impl Into<String>,
        payload_json: serde_json::Value,
        priority: i32,
    ) -> AppResult<()> {
        self.repo.insert_agent_inbox_event(AgentInboxEvent {
            id: Uuid::new_v4(),
            agent_profile_id,
            event_type: event_type.into(),
            payload_json,
            priority,
            available_at: now_utc(),
            processed_at: None,
            status: AgentInboxEventStatus::Pending,
            created_at: now_utc(),
        })
    }

    pub(super) fn enqueue_message_events_for_recipients(
        &self,
        message: &MessageView,
        runtime_generated: bool,
        recipient_agent_ids: &[Uuid],
        mentioned_agent_ids: &[Uuid],
        mention_all: bool,
        wake_immediately: bool,
    ) -> AppResult<()> {
        for recipient_agent_id in recipient_agent_ids.iter().copied() {
            self.enqueue_agent_event(
                recipient_agent_id,
                "message.received",
                json!({
                    "message_id": message.id,
                    "conversation_id": message.conversation_id,
                    "sender_agent_id": message.sender_agent_id,
                    "sender_human_user_id": message.sender_human_user_id,
                    "content": message.content,
                    "attachments": message.attachments,
                    "runtime_generated": runtime_generated,
                    "mentioned_agent_ids": mentioned_agent_ids,
                    "mention_all": mention_all,
                    "mentioned": mention_all || mentioned_agent_ids.contains(&recipient_agent_id)
                }),
                10,
            )?;
            if wake_immediately {
                self.repo.request_agent_codex_trigger_wake(
                    recipient_agent_id,
                    message.created_at,
                    "message",
                )?;
            }
        }

        Ok(())
    }

    pub(super) fn set_owned_agent_status(
        &self,
        human_user_id: Uuid,
        agent_id: Uuid,
        status: AgentStatus,
    ) -> AppResult<AgentProfile> {
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

        self.set_agent_status_internal(
            agent_id,
            status.clone(),
            match status {
                AgentStatus::Frozen => "owner.freeze",
                AgentStatus::Active => "owner.unfreeze",
                AgentStatus::PendingVerification => "owner.status_update",
            },
            json!({
                "human_user_id": human_user_id,
                "agent_id": agent_id,
                "status": status
            }),
        )
    }

    pub(super) fn set_agent_status_internal(
        &self,
        agent_id: Uuid,
        status: AgentStatus,
        action_type: &'static str,
        request_payload: serde_json::Value,
    ) -> AppResult<AgentProfile> {
        self.repo
            .get_agent_profile(agent_id)
            .ok_or_else(|| AppError::NotFound("agent not found".into()))?;

        self.repo.update_agent_profile_status(agent_id, status)?;
        let updated_agent = self
            .repo
            .get_agent_profile(agent_id)
            .ok_or_else(|| AppError::NotFound("agent not found after status update".into()))?;
        let result_payload = json!({
            "agent_profile": updated_agent
        });

        self.record_agent_action(
            agent_id,
            action_type,
            Some(format!("agent:{agent_id}")),
            request_payload,
            result_payload,
            AgentActionStatus::Success,
        )?;

        self.repo
            .get_agent_profile(agent_id)
            .ok_or_else(|| AppError::NotFound("agent not found after status update".into()))
    }
}
