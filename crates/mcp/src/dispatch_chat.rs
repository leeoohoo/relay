use super::handler::parse_input;
use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> McpGateway<R, V> {
    pub(super) fn execute_company_chat_tool(
        &self,
        agent_id: Uuid,
        tool_name: &str,
        input: Value,
        _idempotency_key: Option<String>,
    ) -> AppResult<Value> {
        match tool_name {
            "company.events" => {
                let input: CompanyEventsToolInput = parse_input(input)?;
                let events = self.platform.list_company_realtime_events_for_agent(
                    agent_id,
                    input.company_id,
                    input.after_sequence_id.unwrap_or(0),
                    input.limit.unwrap_or(100),
                )?;
                Ok(json!({ "events": events }))
            }
            "company.chat" => {
                let input: CompanyChatToolInput = parse_input(input)?;
                let _ = input.idempotency_key;
                match input.operation {
                    CompanyChatOperation::DirectOpen {
                        company_id,
                        target_agent_id,
                    } => {
                        let conversation = self.platform.open_company_direct_conversation(
                            OpenCompanyDirectConversationInput {
                                actor_agent_id: agent_id,
                                company_id,
                                target_agent_id,
                            },
                        )?;
                        Ok(json!({ "conversation": conversation }))
                    }
                    CompanyChatOperation::GroupCreate {
                        company_id,
                        title,
                        member_agent_ids,
                    } => {
                        let conversation = self.platform.create_company_group_conversation(
                            CreateCompanyGroupConversationInput {
                                actor_agent_id: agent_id,
                                company_id,
                                title,
                                member_agent_ids,
                            },
                        )?;
                        Ok(json!({ "conversation": conversation }))
                    }
                    CompanyChatOperation::Send {
                        company_id,
                        conversation_id,
                        content,
                        mentioned_agent_ids,
                        mention_all,
                    } => {
                        let message = self.platform.send_company_message_with_mentions(
                            SendCompanyMessageWithMentionsInput {
                                actor_agent_id: agent_id,
                                company_id,
                                conversation_id,
                                content,
                                mentioned_agent_ids,
                                mention_all,
                            },
                        )?;
                        Ok(json!({ "message": message }))
                    }
                    CompanyChatOperation::Reply {
                        event_id,
                        content,
                        auto_ack,
                    } => {
                        let result = self.platform.reply_to_company_inbox_message(
                            ReplyCompanyInboxMessageInput {
                                actor_agent_id: agent_id,
                                event_id,
                                content,
                                auto_ack: auto_ack.unwrap_or(true),
                            },
                        )?;
                        Ok(json!({ "message": result.message, "event": result.event }))
                    }
                    CompanyChatOperation::History {
                        company_id,
                        conversation_id,
                        before_message_id,
                        limit,
                    } => {
                        let context = self.platform.get_company_agent_context(
                            GetCompanyAgentContextInput {
                                actor_agent_id: agent_id,
                                company_id,
                            },
                        )?;
                        if !context
                            .conversations
                            .iter()
                            .any(|conversation| conversation.preview.id == conversation_id)
                        {
                            return Err(AppError::Unauthorized(
                                "agent is not a member of the requested company conversation"
                                    .into(),
                            ));
                        }
                        let page = self.platform.get_agent_conversation_message_page(
                            agent_id,
                            conversation_id,
                            before_message_id,
                            limit.unwrap_or(50),
                        )?;
                        Ok(json!({
                            "messages": page.messages,
                            "next_cursor": page.next_cursor,
                            "has_more": page.has_more
                        }))
                    }
                    CompanyChatOperation::Unread {
                        company_id,
                        conversation_id,
                        after_message_id,
                        message_limit,
                    } => {
                        let unread = self.platform.list_company_group_unread_messages(
                            ListCompanyGroupUnreadInput {
                                actor_agent_id: agent_id,
                                company_id,
                                conversation_id,
                                after_message_id,
                                message_limit: message_limit.unwrap_or(20),
                            },
                        )?;
                        Ok(json!({ "unread": unread }))
                    }
                    CompanyChatOperation::MarkRead {
                        company_id,
                        conversation_id,
                        only_if_no_mentions,
                        reviewed_through_message_id,
                    } => {
                        let result =
                            self.platform
                                .mark_company_group_read(MarkCompanyGroupReadInput {
                                    actor_agent_id: agent_id,
                                    company_id,
                                    conversation_id,
                                    only_if_no_mentions,
                                    reviewed_through_message_id,
                                })?;
                        Ok(json!({ "result": result }))
                    }
                }
            }

            _ => Err(AppError::NotFound(format!(
                "unknown standard MCP tool: {tool_name}"
            ))),
        }
    }
}
