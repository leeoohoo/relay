use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub(super) fn send_company_message_with_mentions_internal(
        &self,
        input: SendCompanyMessageWithMentionsInput,
        runtime_generated: bool,
    ) -> AppResult<MessageView> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_AGENT_COMMUNICATE,
        )?;
        let context = self
            .repo
            .get_conversation_context_result(input.conversation_id)?
            .filter(|context| {
                context.company_id == Some(input.company_id)
                    && matches!(
                        context.context_type.as_str(),
                        CONVERSATION_CONTEXT_COMPANY_DIRECT
                            | CONVERSATION_CONTEXT_COMPANY_ALL
                            | CONVERSATION_CONTEXT_COMPANY_GROUP
                            | CONVERSATION_CONTEXT_PROJECT_GROUP
                    )
            })
            .ok_or_else(|| {
                AppError::Unauthorized("conversation does not belong to the company".into())
            })?;
        if context.visibility != "members" {
            return Err(AppError::Unauthorized(
                "company conversation is not visible to members".into(),
            ));
        }
        self.ensure_project_conversation_not_paused(&context)?;
        if !self
            .repo
            .list_conversation_member_ids(input.conversation_id)
            .contains(&input.actor_agent_id)
        {
            return Err(AppError::Unauthorized(
                "agent is not a member of the target conversation".into(),
            ));
        }
        let content = input.content.trim();
        if content.is_empty() {
            return Err(AppError::Validation("message content is required".into()));
        }
        let (mentioned_agent_ids, notification_recipient_ids) = self
            .resolve_company_message_mentions(
                &context,
                input.conversation_id,
                Some(input.actor_agent_id),
                input.mentioned_agent_ids,
                input.mention_all,
            )?;
        let project_owner_broadcast = if context.context_type == CONVERSATION_CONTEXT_PROJECT_GROUP
        {
            let project_id = context.project_id.ok_or_else(|| {
                AppError::Internal("project group conversation is missing project_id".into())
            })?;
            self.repo
                .get_company_project_result(project_id)?
                .is_some_and(|project| project.owner_agent_id == input.actor_agent_id)
        } else {
            false
        };
        let message = MessageView {
            id: Uuid::new_v4(),
            conversation_id: input.conversation_id,
            sender_agent_id: Some(input.actor_agent_id),
            sender_human_user_id: None,
            content: content.to_string(),
            attachments: Vec::new(),
            created_at: now_utc(),
        };
        self.repo.append_message_with_metadata(
            message.clone(),
            json!({
                "mentioned_agent_ids": mentioned_agent_ids,
                "mention_all": input.mention_all,
            }),
        )?;
        self.enqueue_message_events_for_recipients(
            &message,
            runtime_generated,
            &notification_recipient_ids,
            &mentioned_agent_ids,
            input.mention_all,
            context.context_type == CONVERSATION_CONTEXT_COMPANY_DIRECT
                || input.mention_all
                || !mentioned_agent_ids.is_empty()
                || project_owner_broadcast,
        )?;
        Ok(message)
    }

    pub(super) fn resolve_company_message_mentions(
        &self,
        context: &ConversationContext,
        conversation_id: Uuid,
        sender_agent_id: Option<Uuid>,
        mentioned_agent_ids: Vec<Uuid>,
        mention_all: bool,
    ) -> AppResult<(Vec<Uuid>, Vec<Uuid>)> {
        if context.context_type == CONVERSATION_CONTEXT_COMPANY_DIRECT
            && (mention_all || !mentioned_agent_ids.is_empty())
        {
            return Err(AppError::Validation(
                "mentions are only supported in group conversations".into(),
            ));
        }

        let mut participants = self.repo.list_conversation_member_ids(conversation_id);
        participants.sort();
        participants.dedup();

        let mut normalized_mentions = mentioned_agent_ids;
        normalized_mentions.sort();
        normalized_mentions.dedup();
        if normalized_mentions.len() > 50 {
            return Err(AppError::Validation(
                "a message can mention at most 50 Agents".into(),
            ));
        }
        if mention_all {
            normalized_mentions.clear();
        }
        if let Some(sender_agent_id) = sender_agent_id {
            normalized_mentions.retain(|agent_id| *agent_id != sender_agent_id);
        }
        if normalized_mentions
            .iter()
            .any(|agent_id| !participants.contains(agent_id))
        {
            return Err(AppError::Validation(
                "mentioned Agent must be an active member of the conversation".into(),
            ));
        }

        let mut recipients = if context.context_type == CONVERSATION_CONTEXT_COMPANY_DIRECT
            || mention_all
            || normalized_mentions.is_empty()
        {
            participants
        } else {
            normalized_mentions.clone()
        };
        if let Some(sender_agent_id) = sender_agent_id {
            recipients.retain(|agent_id| *agent_id != sender_agent_id);
        }
        Ok((normalized_mentions, recipients))
    }
}
