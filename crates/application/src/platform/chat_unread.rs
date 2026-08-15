use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn list_company_group_unread_messages(
        &self,
        input: ListCompanyGroupUnreadInput,
    ) -> AppResult<CompanyGroupUnreadResult> {
        if input.after_message_id.is_some() && input.conversation_id.is_none() {
            return Err(AppError::Validation(
                "conversation_id is required when after_message_id is provided".into(),
            ));
        }
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

        let mut messages_by_conversation = HashMap::<Uuid, Vec<(MessageView, bool)>>::new();
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
                .push((
                    MessageView {
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
                    },
                    event
                        .payload_json
                        .get("mentioned")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                ));
        }

        let message_limit = input.message_limit.clamp(1, 100);
        let total_unread_count = messages_by_conversation.values().map(Vec::len).sum();
        let total_mention_count = messages_by_conversation
            .values()
            .flat_map(|messages| messages.iter())
            .filter(|(_, mentioned)| *mentioned)
            .count();
        let mut groups = Vec::new();
        for (conversation_id, mut unread_entries) in messages_by_conversation {
            let Some(preview) = previews_by_id.get(&conversation_id).cloned() else {
                continue;
            };
            let Some(context) = contexts_by_id.get(&conversation_id).cloned() else {
                continue;
            };
            unread_entries.sort_by(|(left, _), (right, _)| {
                left.created_at
                    .cmp(&right.created_at)
                    .then_with(|| left.id.cmp(&right.id))
            });
            let unread_count = unread_entries.len();
            let mention_count = unread_entries
                .iter()
                .filter(|(_, mentioned)| *mentioned)
                .count();
            let page_start = match input.after_message_id {
                Some(cursor) => unread_entries
                    .iter()
                    .position(|(message, _)| message.id == cursor)
                    .map(|position| position + 1)
                    .ok_or_else(|| {
                        AppError::Validation(
                            "after_message_id is not an unread message in the requested group"
                                .into(),
                        )
                    })?,
                None => 0,
            };
            let page_end = (page_start + message_limit).min(unread_entries.len());
            let page_entries = &unread_entries[page_start..page_end];
            let page_unread_count = page_entries.len();
            let page_mention_count = page_entries
                .iter()
                .filter(|(_, mentioned)| *mentioned)
                .count();
            let page_mentioned_message_ids = page_entries
                .iter()
                .filter_map(|(message, mentioned)| mentioned.then_some(message.id))
                .collect();
            let remaining_entries = &unread_entries[page_end..];
            let remaining_unread_count = remaining_entries.len();
            let remaining_mention_count = remaining_entries
                .iter()
                .filter(|(_, mentioned)| *mentioned)
                .count();
            let has_more = remaining_unread_count > 0;
            let next_cursor = has_more
                .then(|| page_entries.last().map(|(message, _)| message.id))
                .flatten();
            let unread_messages = page_entries
                .iter()
                .map(|(message, _)| message.clone())
                .collect();
            groups.push(CompanyGroupUnreadView {
                conversation: CompanyConversationView {
                    preview,
                    context,
                    member_agent_ids: self.repo.list_conversation_member_ids(conversation_id),
                },
                unread_count,
                mention_count,
                page_unread_count,
                page_mention_count,
                page_mentioned_message_ids,
                remaining_unread_count,
                remaining_mention_count,
                remaining_has_mentions: remaining_mention_count > 0,
                can_quick_mark_read: remaining_unread_count > 0 && remaining_mention_count == 0,
                next_cursor,
                has_more,
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
            total_mention_count,
            has_unread_mentions: total_mention_count > 0,
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
        let mut eligible_events = self
            .repo
            .list_agent_inbox_events(
                input.actor_agent_id,
                Some(AgentInboxEventStatus::Pending),
                10_000,
            )
            .into_iter()
            .filter(|event| {
                event.event_type == "message.received"
                    && payload_uuid_field(&event.payload_json, "conversation_id")
                        .is_ok_and(|conversation_id| conversation_id == input.conversation_id)
                    && event.created_at <= read_through_at
            })
            .collect::<Vec<_>>();
        eligible_events.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        let mention_guard_start = match input.reviewed_through_message_id {
            Some(message_id) => eligible_events
                .iter()
                .position(|event| {
                    payload_uuid_field_optional(&event.payload_json, "message_id")
                        == Some(message_id)
                })
                .map(|position| position + 1)
                .ok_or_else(|| {
                    AppError::Validation(
                        "reviewed_through_message_id is not an unread message in this group".into(),
                    )
                })?,
            None => 0,
        };
        if input.only_if_no_mentions
            && eligible_events[mention_guard_start..].iter().any(|event| {
                event
                    .payload_json
                    .get("mentioned")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false)
            })
        {
            return Err(AppError::Conflict(
                "quick mark-read refused because later unread messages still mention this Agent"
                    .into(),
            ));
        }
        let marked_read_count = eligible_events.len();
        for mut event in eligible_events.drain(..) {
            event.status = AgentInboxEventStatus::Processed;
            event.processed_at = Some(read_at);
            self.repo.update_agent_inbox_event(event)?;
        }

        Ok(MarkCompanyGroupReadResult {
            conversation_id: input.conversation_id,
            marked_read_count,
            quick_mark_read: input.only_if_no_mentions,
            read_at,
        })
    }
}
