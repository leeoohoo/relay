use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn list_agent_conversations(&self, agent_id: Uuid) -> AppResult<Vec<ConversationPreview>> {
        if !self.repo.agent_exists(agent_id) {
            return Err(AppError::NotFound("agent not found".into()));
        }

        Ok(self.repo.list_agent_conversations(agent_id))
    }

    pub fn get_conversation_messages(&self, conversation_id: Uuid) -> AppResult<Vec<MessageView>> {
        if !self.repo.conversation_exists(conversation_id) {
            return Err(AppError::NotFound("conversation not found".into()));
        }
        self.repo.get_conversation_messages_result(conversation_id)
    }

    pub fn get_agent_conversation_messages(
        &self,
        agent_id: Uuid,
        conversation_id: Uuid,
    ) -> AppResult<Vec<MessageView>> {
        if !self.repo.agent_exists(agent_id) {
            return Err(AppError::NotFound("agent not found".into()));
        }
        if !self.repo.conversation_exists(conversation_id) {
            return Err(AppError::NotFound("conversation not found".into()));
        }

        let conversations = self.repo.list_agent_conversations(agent_id);
        if !conversations.iter().any(|item| item.id == conversation_id) {
            return Err(AppError::Unauthorized(
                "agent is not a member of the target conversation".into(),
            ));
        }

        self.repo.get_conversation_messages_result(conversation_id)
    }

    pub fn get_agent_conversation_message_page(
        &self,
        agent_id: Uuid,
        conversation_id: Uuid,
        before_message_id: Option<Uuid>,
        limit: usize,
    ) -> AppResult<MessagePageView> {
        if !self.repo.agent_exists(agent_id) {
            return Err(AppError::NotFound("agent not found".into()));
        }
        if !self.repo.conversation_exists(conversation_id) {
            return Err(AppError::NotFound("conversation not found".into()));
        }
        if !self
            .repo
            .list_agent_conversations(agent_id)
            .iter()
            .any(|item| item.id == conversation_id)
        {
            return Err(AppError::Unauthorized(
                "agent is not a member of the target conversation".into(),
            ));
        }
        self.repo.get_conversation_message_page(
            conversation_id,
            before_message_id,
            normalize_message_page_limit(limit),
        )
    }

    pub fn update_agent_profile(&self, input: UpdateAgentProfileInput) -> AppResult<AgentProfile> {
        let current = self.ensure_agent_can_act(input.actor_agent_id)?;

        let next_display_name = match input.display_name {
            Some(value) => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err(AppError::Validation(
                        "display_name cannot be empty when provided".into(),
                    ));
                }
                trimmed.to_string()
            }
            None => current.display_name.clone(),
        };

        let next_persona = match input.persona {
            Some(value) => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err(AppError::Validation(
                        "persona cannot be empty when provided".into(),
                    ));
                }
                trimmed.to_string()
            }
            None => current.persona.clone(),
        };

        let next_collaboration_preference = match input.collaboration_preference {
            Some(value) => normalize_agent_collaboration_preference(&value)?,
            None => sanitize_agent_collaboration_preference(&current.collaboration_preference),
        };

        if next_display_name == current.display_name
            && next_persona == current.persona
            && next_collaboration_preference
                == sanitize_agent_collaboration_preference(&current.collaboration_preference)
        {
            return Err(AppError::Validation(
                "at least one profile field must change".into(),
            ));
        }

        self.repo.update_agent_profile(
            input.actor_agent_id,
            next_display_name,
            next_persona,
            next_collaboration_preference,
        )?;

        self.repo
            .get_agent_profile(input.actor_agent_id)
            .ok_or_else(|| AppError::NotFound("agent not found after profile update".into()))
    }

    pub fn update_company_agent_work_profile(
        &self,
        input: UpdateCompanyAgentWorkProfileInput,
    ) -> AppResult<CompanyAgentView> {
        let current_profile = self.ensure_agent_can_act(input.actor_agent_id)?;
        let current_membership = self.get_active_company_agent_membership(input.actor_agent_id)?;
        self.ensure_company_agent_permission(
            current_membership.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_AGENT_COMMUNICATE,
        )?;

        let responsibilities = match input.responsibilities {
            Some(values) => normalize_work_profile_items("responsibilities", values, 20, 160)?,
            None => current_membership.responsibilities.clone(),
        };
        let skills = match input.skills {
            Some(values) => normalize_work_profile_items("skills", values, 30, 80)?,
            None => current_membership.skills.clone(),
        };
        let current_focus = match input.current_focus {
            Some(value) => {
                let value = value.trim();
                if value.chars().count() > 500 {
                    return Err(AppError::Validation(
                        "current_focus must contain at most 500 characters".into(),
                    ));
                }
                value.to_string()
            }
            None => current_membership.current_focus.clone(),
        };
        let collaboration_preference = match input.collaboration_preference {
            Some(value) => normalize_agent_collaboration_preference(&value)?,
            None => {
                sanitize_agent_collaboration_preference(&current_profile.collaboration_preference)
            }
        };

        if responsibilities == current_membership.responsibilities
            && skills == current_membership.skills
            && current_focus == current_membership.current_focus
            && collaboration_preference
                == sanitize_agent_collaboration_preference(
                    &current_profile.collaboration_preference,
                )
        {
            return Err(AppError::Validation(
                "at least one work profile field must change".into(),
            ));
        }

        self.repo.update_company_agent_work_profile(
            input.actor_agent_id,
            responsibilities,
            skills,
            current_focus,
            collaboration_preference,
            now_utc(),
        )?;

        let agent_profile = self
            .repo
            .get_agent_profile(input.actor_agent_id)
            .ok_or_else(|| {
                AppError::NotFound("agent not found after work profile update".into())
            })?;
        let membership = self
            .repo
            .get_company_agent_membership(input.actor_agent_id)
            .ok_or_else(|| {
                AppError::NotFound("company membership not found after work profile update".into())
            })?;
        let profession = infer_company_profession(Some(&membership.job_title));
        Ok(CompanyAgentView {
            agent_profile,
            membership,
            profession,
        })
    }

    pub fn get_company_agent_context(
        &self,
        input: GetCompanyAgentContextInput,
    ) -> AppResult<CompanyAgentContextView> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        let actor_membership = self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_AGENT_COMMUNICATE,
        )?;
        let company = self
            .repo
            .get_company_result(input.company_id)?
            .filter(|company| company.status == "active")
            .ok_or_else(|| AppError::NotFound("active company not found".into()))?;
        let agents = self
            .repo
            .list_company_agent_memberships(company.id)
            .into_iter()
            .filter(|membership| membership.employment_status == "active")
            .map(|membership| {
                let agent_profile = self
                    .repo
                    .get_agent_profile(membership.agent_profile_id)
                    .filter(|profile| matches!(profile.status, AgentStatus::Active))
                    .ok_or_else(|| AppError::NotFound("active company agent not found".into()))?;
                let profession = infer_company_profession(Some(&membership.job_title));
                Ok(CompanyAgentView {
                    agent_profile,
                    membership,
                    profession,
                })
            })
            .collect::<AppResult<Vec<_>>>()?;
        let mut conversations = Vec::new();
        for preview in self.repo.list_agent_conversations(input.actor_agent_id) {
            let Some(context) = self.repo.get_conversation_context_result(preview.id)? else {
                continue;
            };
            if context.company_id == Some(company.id)
                && matches!(
                    context.context_type.as_str(),
                    CONVERSATION_CONTEXT_COMPANY_DIRECT
                        | CONVERSATION_CONTEXT_COMPANY_ALL
                        | CONVERSATION_CONTEXT_COMPANY_GROUP
                        | CONVERSATION_CONTEXT_PROJECT_GROUP
                )
            {
                conversations.push(CompanyConversationView {
                    member_agent_ids: self.repo.list_conversation_member_ids(preview.id),
                    preview,
                    context,
                });
            }
        }
        let can_manage_projects = actor_membership
            .permissions
            .iter()
            .any(|permission| permission == COMPANY_PERMISSION_PROJECT_MANAGE);
        let projects = self
            .repo
            .list_company_projects_result(company.id)?
            .into_iter()
            .filter(|project| {
                can_manage_projects
                    || self
                        .repo
                        .get_company_project_member(project.id, input.actor_agent_id)
                        .is_some_and(|member| member.left_at.is_none())
            })
            .map(|project| self.company_project_view(project))
            .collect::<AppResult<Vec<_>>>()?;

        Ok(CompanyAgentContextView {
            company,
            actor_membership,
            org_units: self.repo.list_company_org_units(input.company_id),
            agents,
            conversations,
            projects,
        })
    }

    pub fn open_company_direct_conversation(
        &self,
        input: OpenCompanyDirectConversationInput,
    ) -> AppResult<CompanyConversationView> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_AGENT_COMMUNICATE,
        )?;
        if input.actor_agent_id == input.target_agent_id {
            return Err(AppError::Validation(
                "company direct conversation target must be another agent".into(),
            ));
        }
        self.ensure_active_company_conversation_member(input.company_id, input.target_agent_id)?;

        if let Some(preview) = self.repo.find_company_direct_conversation(
            input.company_id,
            input.actor_agent_id,
            input.target_agent_id,
        ) {
            let context = self
                .repo
                .get_conversation_context_result(preview.id)?
                .ok_or_else(|| {
                    AppError::NotFound("company conversation context not found".into())
                })?;
            return Ok(CompanyConversationView {
                member_agent_ids: self.repo.list_conversation_member_ids(preview.id),
                preview,
                context,
            });
        }

        let actor = self
            .repo
            .get_agent_profile(input.actor_agent_id)
            .ok_or_else(|| AppError::NotFound("actor agent not found".into()))?;
        let target = self
            .repo
            .get_agent_profile(input.target_agent_id)
            .ok_or_else(|| AppError::NotFound("target agent not found".into()))?;
        let conversation_id = Uuid::new_v4();
        let created_at = now_utc();
        let actor_preview = ConversationPreview {
            id: conversation_id,
            title: target.display_name.clone(),
            conversation_type: ConversationType::Direct,
            last_message_preview: None,
            updated_at: created_at,
        };
        let target_preview = ConversationPreview {
            id: conversation_id,
            title: actor.display_name,
            conversation_type: ConversationType::Direct,
            last_message_preview: None,
            updated_at: created_at,
        };
        let direct_pair = if input.actor_agent_id < input.target_agent_id {
            (input.actor_agent_id, input.target_agent_id)
        } else {
            (input.target_agent_id, input.actor_agent_id)
        };
        self.repo
            .complete_company_conversation_creation(CompanyConversationCreationBundle {
                company_id: input.company_id,
                context_type: CONVERSATION_CONTEXT_COMPANY_DIRECT.into(),
                visibility: "members".into(),
                created_by_agent_id: input.actor_agent_id,
                members: vec![
                    CompanyConversationMemberPreview {
                        agent_id: input.actor_agent_id,
                        preview: actor_preview.clone(),
                    },
                    CompanyConversationMemberPreview {
                        agent_id: input.target_agent_id,
                        preview: target_preview,
                    },
                ],
                direct_pair: Some(direct_pair),
            })?;
        Ok(CompanyConversationView {
            preview: actor_preview,
            context: ConversationContext {
                conversation_id,
                company_id: Some(input.company_id),
                project_id: None,
                context_type: CONVERSATION_CONTEXT_COMPANY_DIRECT.into(),
                visibility: "members".into(),
            },
            member_agent_ids: vec![input.actor_agent_id, input.target_agent_id],
        })
    }

    pub fn open_human_company_direct_conversation(
        &self,
        input: OpenHumanCompanyDirectConversationInput,
    ) -> AppResult<CompanyConversationView> {
        self.ensure_human_can_send_company_messages(input.human_user_id, input.company_id)?;
        self.ensure_active_company_conversation_member(input.company_id, input.target_agent_id)?;

        if let Some(preview) = self.repo.find_human_company_direct_conversation(
            input.company_id,
            input.human_user_id,
            input.target_agent_id,
        ) {
            let context = self
                .repo
                .get_conversation_context_result(preview.id)?
                .ok_or_else(|| {
                    AppError::NotFound("human company conversation context not found".into())
                })?;
            return Ok(CompanyConversationView {
                member_agent_ids: self.repo.list_conversation_member_ids(preview.id),
                preview,
                context,
            });
        }

        let human = self
            .repo
            .get_human_user_result(input.human_user_id)?
            .ok_or_else(|| AppError::NotFound("human user not found".into()))?;
        let target = self
            .repo
            .get_agent_profile(input.target_agent_id)
            .ok_or_else(|| AppError::NotFound("target agent not found".into()))?;
        let conversation_id = Uuid::new_v4();
        let created_at = now_utc();
        let preview = ConversationPreview {
            id: conversation_id,
            title: format!("{} ↔ {}", human.display_name, target.display_name),
            conversation_type: ConversationType::Direct,
            last_message_preview: None,
            updated_at: created_at,
        };
        self.repo
            .complete_human_company_direct_conversation_creation(
                HumanCompanyDirectConversationCreationBundle {
                    company_id: input.company_id,
                    human_user_id: input.human_user_id,
                    target_agent_id: input.target_agent_id,
                    preview: preview.clone(),
                },
            )?;

        Ok(CompanyConversationView {
            preview,
            context: ConversationContext {
                conversation_id,
                company_id: Some(input.company_id),
                project_id: None,
                context_type: CONVERSATION_CONTEXT_COMPANY_DIRECT.into(),
                visibility: "members".into(),
            },
            member_agent_ids: vec![input.target_agent_id],
        })
    }

    pub fn create_company_group_conversation(
        &self,
        input: CreateCompanyGroupConversationInput,
    ) -> AppResult<CompanyConversationView> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_AGENT_COMMUNICATE,
        )?;
        let title = input.title.trim();
        if title.is_empty() || title.chars().count() > 120 {
            return Err(AppError::Validation(
                "company group title must contain 1 to 120 characters".into(),
            ));
        }
        let mut member_agent_ids = vec![input.actor_agent_id];
        for member_agent_id in input.member_agent_ids {
            if !member_agent_ids.contains(&member_agent_id) {
                member_agent_ids.push(member_agent_id);
            }
        }
        if member_agent_ids.len() < 2 {
            return Err(AppError::Validation(
                "company group requires at least one additional member".into(),
            ));
        }
        if member_agent_ids.len() > 50 {
            return Err(AppError::Validation(
                "company group supports at most 50 agents".into(),
            ));
        }
        for member_agent_id in member_agent_ids.iter().copied() {
            self.ensure_active_company_conversation_member(input.company_id, member_agent_id)?;
        }

        let conversation_id = Uuid::new_v4();
        let created_at = now_utc();
        let preview = ConversationPreview {
            id: conversation_id,
            title: title.to_string(),
            conversation_type: ConversationType::Group,
            last_message_preview: None,
            updated_at: created_at,
        };
        self.repo
            .complete_company_conversation_creation(CompanyConversationCreationBundle {
                company_id: input.company_id,
                context_type: CONVERSATION_CONTEXT_COMPANY_GROUP.into(),
                visibility: "members".into(),
                created_by_agent_id: input.actor_agent_id,
                members: member_agent_ids
                    .iter()
                    .copied()
                    .map(|agent_id| CompanyConversationMemberPreview {
                        agent_id,
                        preview: preview.clone(),
                    })
                    .collect(),
                direct_pair: None,
            })?;
        Ok(CompanyConversationView {
            preview,
            context: ConversationContext {
                conversation_id,
                company_id: Some(input.company_id),
                project_id: None,
                context_type: CONVERSATION_CONTEXT_COMPANY_GROUP.into(),
                visibility: "members".into(),
            },
            member_agent_ids,
        })
    }

    pub fn send_company_message(&self, input: SendCompanyMessageInput) -> AppResult<MessageView> {
        self.send_company_message_with_mentions(SendCompanyMessageWithMentionsInput {
            actor_agent_id: input.actor_agent_id,
            company_id: input.company_id,
            conversation_id: input.conversation_id,
            content: input.content,
            mentioned_agent_ids: Vec::new(),
            mention_all: false,
        })
    }

    pub fn send_company_message_with_mentions(
        &self,
        input: SendCompanyMessageWithMentionsInput,
    ) -> AppResult<MessageView> {
        self.send_company_message_with_mentions_internal(input, false)
    }

    pub fn send_human_company_message(
        &self,
        input: SendHumanCompanyMessageInput,
    ) -> AppResult<MessageView> {
        self.send_human_company_message_with_mentions(SendHumanCompanyMessageWithMentionsInput {
            human_user_id: input.human_user_id,
            company_id: input.company_id,
            conversation_id: input.conversation_id,
            content: input.content,
            mentioned_agent_ids: Vec::new(),
            mention_all: false,
        })
    }

    pub fn send_human_company_message_with_mentions(
        &self,
        input: SendHumanCompanyMessageWithMentionsInput,
    ) -> AppResult<MessageView> {
        self.send_human_company_message_with_attachments(
            SendHumanCompanyMessageWithAttachmentsInput {
                human_user_id: input.human_user_id,
                company_id: input.company_id,
                conversation_id: input.conversation_id,
                content: input.content,
                mentioned_agent_ids: input.mentioned_agent_ids,
                mention_all: input.mention_all,
                attachments: Vec::new(),
            },
        )
    }

    pub fn send_human_company_message_with_attachments(
        &self,
        input: SendHumanCompanyMessageWithAttachmentsInput,
    ) -> AppResult<MessageView> {
        self.ensure_human_can_send_company_messages(input.human_user_id, input.company_id)?;
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
        let content = input.content.trim();
        if content.is_empty() && input.attachments.is_empty() {
            return Err(AppError::Validation(
                "message content or attachment is required".into(),
            ));
        }
        if input.attachments.len() > 20 {
            return Err(AppError::Validation(
                "a message can contain at most 20 attachments".into(),
            ));
        }
        let (mentioned_agent_ids, notification_recipient_ids) = self
            .resolve_company_message_mentions(
                &context,
                input.conversation_id,
                None,
                input.mentioned_agent_ids,
                input.mention_all,
            )?;
        let wake_recipient_agent_ids = self.resolve_company_message_wake_recipients(
            &context,
            &notification_recipient_ids,
            &mentioned_agent_ids,
            input.mention_all,
        )?;

        let message = MessageView {
            id: Uuid::new_v4(),
            conversation_id: input.conversation_id,
            sender_agent_id: None,
            sender_human_user_id: Some(input.human_user_id),
            content: content.to_string(),
            attachments: input.attachments,
            created_at: now_utc(),
        };
        self.repo.append_message_with_metadata(
            message.clone(),
            json!({
                "mentioned_agent_ids": mentioned_agent_ids,
                "mention_all": input.mention_all,
                "attachments": message.attachments,
            }),
        )?;
        self.enqueue_message_events_for_recipients(
            &message,
            false,
            &notification_recipient_ids,
            &mentioned_agent_ids,
            input.mention_all,
            &wake_recipient_agent_ids,
        )?;
        Ok(message)
    }

    pub fn reply_to_company_inbox_message(
        &self,
        input: ReplyCompanyInboxMessageInput,
    ) -> AppResult<ReplyCompanyInboxMessageResult> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        let event = self
            .repo
            .get_agent_inbox_event(input.event_id)
            .ok_or_else(|| AppError::NotFound("agent inbox event not found".into()))?;
        if event.agent_profile_id != input.actor_agent_id {
            return Err(AppError::Unauthorized(
                "agent can only reply to its own inbox event".into(),
            ));
        }
        if event.event_type != "message.received" {
            return Err(AppError::Validation(
                "only message.received inbox events can be replied to".into(),
            ));
        }
        if !matches!(event.status, AgentInboxEventStatus::Pending) {
            return Err(AppError::Conflict(
                "inbox event has already been handled".into(),
            ));
        }
        let conversation_id = payload_uuid_field(&event.payload_json, "conversation_id")?;
        let company_id = self
            .repo
            .get_conversation_context_result(conversation_id)?
            .and_then(|context| context.company_id)
            .ok_or_else(|| {
                AppError::Unauthorized("inbox event is not from a company conversation".into())
            })?;
        let message = self.send_company_message(SendCompanyMessageInput {
            actor_agent_id: input.actor_agent_id,
            company_id,
            conversation_id,
            content: input.content,
        })?;
        let event = if input.auto_ack {
            self.mark_agent_inbox_event_processed(MarkInboxEventProcessedInput {
                actor_agent_id: input.actor_agent_id,
                event_id: input.event_id,
            })?
        } else {
            event
        };
        Ok(ReplyCompanyInboxMessageResult { message, event })
    }

    pub fn list_company_group_unread_messages(
        &self,
        input: ListCompanyGroupUnreadInput,
    ) -> AppResult<CompanyGroupUnreadResult> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_AGENT_COMMUNICATE,
        )?;
        let previews = self.repo.list_agent_conversations(input.actor_agent_id);
        let previews_by_id = previews
            .into_iter()
            .map(|preview| (preview.id, preview))
            .collect::<HashMap<_, _>>();
        let mut contexts_by_id = HashMap::new();
        for conversation_id in previews_by_id.keys().copied() {
            if let Some(context) = self.repo.get_conversation_context_result(conversation_id)? {
                contexts_by_id.insert(conversation_id, context);
            }
        }

        if let Some(conversation_id) = input.conversation_id {
            let context = contexts_by_id
                .get(&conversation_id)
                .filter(|context| {
                    company_group_context_matches(context, input.company_id)
                        && previews_by_id.contains_key(&conversation_id)
                })
                .ok_or_else(|| {
                    AppError::Unauthorized(
                        "agent is not a member of the requested company group".into(),
                    )
                })?;
            debug_assert_eq!(context.conversation_id, conversation_id);
        }

        let mut messages_by_conversation = HashMap::<Uuid, Vec<MessageView>>::new();
        for event in self.repo.list_agent_inbox_events(
            input.actor_agent_id,
            Some(AgentInboxEventStatus::Pending),
            10_000,
        ) {
            if event.event_type != "message.received" {
                continue;
            }
            let Ok(conversation_id) = payload_uuid_field(&event.payload_json, "conversation_id")
            else {
                continue;
            };
            if input
                .conversation_id
                .is_some_and(|requested_id| requested_id != conversation_id)
            {
                continue;
            }
            if !previews_by_id.contains_key(&conversation_id)
                || !contexts_by_id
                    .get(&conversation_id)
                    .is_some_and(|context| company_group_context_matches(context, input.company_id))
            {
                continue;
            }
            let Ok(message_id) = payload_uuid_field(&event.payload_json, "message_id") else {
                continue;
            };
            let sender_agent_id =
                payload_uuid_field_optional(&event.payload_json, "sender_agent_id");
            let sender_human_user_id =
                payload_uuid_field_optional(&event.payload_json, "sender_human_user_id");
            if sender_agent_id.is_none() && sender_human_user_id.is_none() {
                continue;
            }
            messages_by_conversation
                .entry(conversation_id)
                .or_default()
                .push(MessageView {
                    id: message_id,
                    conversation_id,
                    sender_agent_id,
                    sender_human_user_id,
                    content: payload_string_field(&event.payload_json, "content")
                        .unwrap_or_default(),
                    attachments: event
                        .payload_json
                        .get("attachments")
                        .cloned()
                        .and_then(|value| serde_json::from_value(value).ok())
                        .unwrap_or_default(),
                    created_at: event.created_at,
                });
        }

        let message_limit = input.message_limit.clamp(1, 100);
        let total_unread_count = messages_by_conversation.values().map(Vec::len).sum();
        let mut groups = Vec::new();
        for (conversation_id, mut unread_messages) in messages_by_conversation {
            let Some(preview) = previews_by_id.get(&conversation_id).cloned() else {
                continue;
            };
            let Some(context) = contexts_by_id.get(&conversation_id).cloned() else {
                continue;
            };
            let unread_count = unread_messages.len();
            unread_messages.sort_by(|left, right| left.created_at.cmp(&right.created_at));
            unread_messages.truncate(message_limit);
            groups.push(CompanyGroupUnreadView {
                conversation: CompanyConversationView {
                    preview,
                    context,
                    member_agent_ids: self.repo.list_conversation_member_ids(conversation_id),
                },
                unread_count,
                unread_messages,
            });
        }
        groups.sort_by(|left, right| {
            right
                .conversation
                .preview
                .updated_at
                .cmp(&left.conversation.preview.updated_at)
        });

        Ok(CompanyGroupUnreadResult {
            total_unread_count,
            groups,
        })
    }

    pub fn mark_company_group_read(
        &self,
        input: MarkCompanyGroupReadInput,
    ) -> AppResult<MarkCompanyGroupReadResult> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        self.ensure_company_agent_permission(
            input.company_id,
            input.actor_agent_id,
            COMPANY_PERMISSION_AGENT_COMMUNICATE,
        )?;
        let is_member = self
            .repo
            .list_agent_conversations(input.actor_agent_id)
            .iter()
            .any(|preview| preview.id == input.conversation_id);
        let context = self
            .repo
            .get_conversation_context_result(input.conversation_id)?
            .filter(|context| is_member && company_group_context_matches(context, input.company_id))
            .ok_or_else(|| {
                AppError::Unauthorized(
                    "agent is not a member of the requested company group".into(),
                )
            })?;
        debug_assert_eq!(context.conversation_id, input.conversation_id);

        let read_at = now_utc();
        let read_through_at = self
            .repo
            .get_agent_codex_trigger_config_by_agent(input.actor_agent_id)
            .filter(|config| {
                config.lease_owner.is_some()
                    && config
                        .lease_expires_at
                        .is_some_and(|lease_expires_at| lease_expires_at > read_at)
            })
            .and_then(|config| config.last_run_at)
            .unwrap_or(read_at);
        let mut marked_read_count = 0;
        for mut event in self.repo.list_agent_inbox_events(
            input.actor_agent_id,
            Some(AgentInboxEventStatus::Pending),
            10_000,
        ) {
            let conversation_matches = payload_uuid_field(&event.payload_json, "conversation_id")
                .is_ok_and(|conversation_id| conversation_id == input.conversation_id);
            if event.event_type != "message.received"
                || !conversation_matches
                || event.created_at > read_through_at
            {
                continue;
            }
            event.status = AgentInboxEventStatus::Processed;
            event.processed_at = Some(read_at);
            self.repo.update_agent_inbox_event(event)?;
            marked_read_count += 1;
        }

        Ok(MarkCompanyGroupReadResult {
            conversation_id: input.conversation_id,
            marked_read_count,
            read_at,
        })
    }
}
