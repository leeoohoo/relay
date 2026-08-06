use super::mapping::*;
use super::*;

impl ChatPlatformRepository for PostgresPlatformRepository {
    fn ensure_agent_conversation_bucket(&self, _agent_id: Uuid) -> AppResult<()> {
        Ok(())
    }

    fn insert_conversation_preview(
        &self,
        agent_id: Uuid,
        preview: ConversationPreview,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;

            let title: Option<&str> = if preview.title.trim().is_empty() {
                None
            } else {
                Some(preview.title.as_str())
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
                    &preview.id,
                    &conversation_type_to_str(&preview.conversation_type),
                    &title,
                    &agent_id,
                    &preview.updated_at,
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
                &[&Uuid::new_v4(), &preview.id, &agent_id, &preview.updated_at],
            )?;

            tx.commit()?;
            Ok(())
        })
    }

    fn complete_company_conversation_creation(
        &self,
        bundle: CompanyConversationCreationBundle,
    ) -> AppResult<()> {
        if bundle.members.is_empty() {
            return Err(AppError::Validation(
                "company conversation members required".into(),
            ));
        }
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            let first = &bundle.members[0];
            let title = Some(first.preview.title.as_str());
            tx.execute(
                r#"
                INSERT INTO conversations (
                    id, conversation_type, title, created_by_agent_id, status,
                    company_id, context_type, visibility,
                    last_message_at, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, 'active', $5, $6, $7, $8, $8, $8)
                "#,
                &[
                    &first.preview.id,
                    &conversation_type_to_str(&first.preview.conversation_type),
                    &title,
                    &bundle.created_by_agent_id,
                    &bundle.company_id,
                    &bundle.context_type,
                    &bundle.visibility,
                    &first.preview.updated_at,
                ],
            )?;
            for member in &bundle.members {
                let member_role = if member.agent_id == bundle.created_by_agent_id {
                    "owner"
                } else {
                    "member"
                };
                tx.execute(
                    r#"
                    INSERT INTO conversation_members (
                        id, conversation_id, agent_profile_id, member_role, joined_at
                    )
                    VALUES ($1, $2, $3, $4, $5)
                    "#,
                    &[
                        &Uuid::new_v4(),
                        &member.preview.id,
                        &member.agent_id,
                        &member_role,
                        &member.preview.updated_at,
                    ],
                )?;
            }
            if let Some((left_agent_id, right_agent_id)) = bundle.direct_pair {
                tx.execute(
                    r#"
                    INSERT INTO company_direct_conversations (
                        company_id, left_agent_id, right_agent_id, conversation_id, created_at
                    )
                    VALUES ($1, $2, $3, $4, $5)
                    "#,
                    &[
                        &bundle.company_id,
                        &left_agent_id,
                        &right_agent_id,
                        &first.preview.id,
                        &first.preview.updated_at,
                    ],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
    }

    fn complete_human_company_direct_conversation_creation(
        &self,
        bundle: HumanCompanyDirectConversationCreationBundle,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            let title = Some(bundle.preview.title.as_str());
            tx.execute(
                r#"
                INSERT INTO conversations (
                    id, conversation_type, title, created_by_agent_id,
                    created_by_human_user_id, status, company_id, context_type,
                    visibility, last_message_at, created_at, updated_at
                )
                VALUES ($1, $2, $3, NULL, $4, 'active', $5, 'company_direct',
                        'members', $6, $6, $6)
                "#,
                &[
                    &bundle.preview.id,
                    &conversation_type_to_str(&bundle.preview.conversation_type),
                    &title,
                    &bundle.human_user_id,
                    &bundle.company_id,
                    &bundle.preview.updated_at,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO conversation_members (
                    id, conversation_id, agent_profile_id, member_role, joined_at
                )
                VALUES ($1, $2, $3, 'member', $4)
                "#,
                &[
                    &Uuid::new_v4(),
                    &bundle.preview.id,
                    &bundle.target_agent_id,
                    &bundle.preview.updated_at,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO company_human_direct_conversations (
                    company_id, human_user_id, target_agent_id, conversation_id, created_at
                )
                VALUES ($1, $2, $3, $4, $5)
                "#,
                &[
                    &bundle.company_id,
                    &bundle.human_user_id,
                    &bundle.target_agent_id,
                    &bundle.preview.id,
                    &bundle.preview.updated_at,
                ],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn find_company_direct_conversation(
        &self,
        company_id: Uuid,
        left_agent_id: Uuid,
        right_agent_id: Uuid,
    ) -> Option<ConversationPreview> {
        let (pair_left_agent_id, pair_right_agent_id) = if left_agent_id < right_agent_id {
            (left_agent_id, right_agent_id)
        } else {
            (right_agent_id, left_agent_id)
        };
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT c.id,
                       COALESCE(peer.display_name, c.title, '') AS title,
                       c.conversation_type,
                       latest.content_text AS last_message_preview,
                       COALESCE(latest.created_at, c.updated_at, c.created_at) AS updated_at
                FROM company_direct_conversations direct_pair
                INNER JOIN conversations c ON c.id = direct_pair.conversation_id
                LEFT JOIN agent_profiles peer
                    ON peer.id = CASE
                        WHEN direct_pair.left_agent_id = $2 THEN direct_pair.right_agent_id
                        ELSE direct_pair.left_agent_id
                    END
                LEFT JOIN LATERAL (
                    SELECT content_text, created_at
                    FROM messages
                    WHERE conversation_id = c.id
                    ORDER BY created_at DESC
                    LIMIT 1
                ) latest ON TRUE
                WHERE direct_pair.company_id = $1
                  AND direct_pair.left_agent_id = $3
                  AND direct_pair.right_agent_id = $4
                  AND c.context_type = 'company_direct'
                LIMIT 1
                "#,
                &[
                    &company_id,
                    &left_agent_id,
                    &pair_left_agent_id,
                    &pair_right_agent_id,
                ],
            )
        })
        .ok()
        .flatten()
        .map(map_conversation_preview)
    }

    fn find_human_company_direct_conversation(
        &self,
        company_id: Uuid,
        human_user_id: Uuid,
        target_agent_id: Uuid,
    ) -> Option<ConversationPreview> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT c.id,
                       COALESCE(c.title, '') AS title,
                       c.conversation_type,
                       latest.content_text AS last_message_preview,
                       COALESCE(latest.created_at, c.updated_at, c.created_at) AS updated_at
                FROM company_human_direct_conversations direct_pair
                INNER JOIN conversations c ON c.id = direct_pair.conversation_id
                LEFT JOIN LATERAL (
                    SELECT content_text, created_at
                    FROM messages
                    WHERE conversation_id = c.id
                    ORDER BY created_at DESC, id DESC
                    LIMIT 1
                ) latest ON TRUE
                WHERE direct_pair.company_id = $1
                  AND direct_pair.human_user_id = $2
                  AND direct_pair.target_agent_id = $3
                "#,
                &[&company_id, &human_user_id, &target_agent_id],
            )
        })
        .ok()
        .flatten()
        .map(map_conversation_preview)
    }

    fn get_conversation_context(&self, conversation_id: Uuid) -> Option<ConversationContext> {
        self.get_conversation_context_result(conversation_id)
            .ok()
            .flatten()
    }

    fn get_conversation_context_result(
        &self,
        conversation_id: Uuid,
    ) -> AppResult<Option<ConversationContext>> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, project_id, context_type, visibility
                FROM conversations
                WHERE id = $1
                "#,
                &[&conversation_id],
            )
        })
        .map(|row| {
            row.map(|row| ConversationContext {
                conversation_id: row.get("id"),
                company_id: row.get("company_id"),
                project_id: row.get("project_id"),
                context_type: row.get("context_type"),
                visibility: row.get("visibility"),
            })
        })
    }

    fn list_conversation_member_ids(&self, conversation_id: Uuid) -> Vec<Uuid> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT agent_profile_id
                FROM conversation_members
                WHERE conversation_id = $1
                  AND left_at IS NULL
                ORDER BY joined_at, agent_profile_id
                "#,
                &[&conversation_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(|row| row.get("agent_profile_id"))
        .collect()
    }

    fn list_owner_agents(&self, human_user_id: Uuid) -> Vec<AgentProfile> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, owner_user_id, display_name, handle, persona,
                       collaboration_preference, status, created_at
                FROM agent_profiles
                WHERE owner_user_id = $1
                ORDER BY created_at DESC
                "#,
                &[&human_user_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_profile)
        .collect()
    }

    fn agent_exists(&self, agent_id: Uuid) -> bool {
        self.with_client(|client| {
            client
                .query_one(
                    "SELECT COUNT(1) FROM agent_profiles WHERE id = $1",
                    &[&agent_id],
                )
                .map(|row| row.get::<_, i64>(0) > 0)
        })
        .unwrap_or(false)
    }

    fn get_agent_profile(&self, agent_id: Uuid) -> Option<AgentProfile> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, owner_user_id, display_name, handle, persona,
                       collaboration_preference, status, created_at
                FROM agent_profiles
                WHERE id = $1
                "#,
                &[&agent_id],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_profile)
    }

    fn list_agent_conversations(&self, agent_id: Uuid) -> Vec<ConversationPreview> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT c.id,
                       CASE
                           WHEN c.conversation_type = 'direct' THEN COALESCE(peer.display_name, c.title, '')
                           ELSE COALESCE(c.title, '')
                       END AS title,
                       c.conversation_type,
                       latest.content_text AS last_message_preview,
                       COALESCE(latest.created_at, c.updated_at, c.created_at) AS updated_at
                FROM conversation_members cm
                INNER JOIN conversations c ON c.id = cm.conversation_id
                LEFT JOIN conversation_members cm_peer
                    ON cm_peer.conversation_id = c.id
                   AND cm_peer.agent_profile_id <> $1
                   AND cm_peer.left_at IS NULL
                LEFT JOIN agent_profiles peer ON peer.id = cm_peer.agent_profile_id
                LEFT JOIN LATERAL (
                    SELECT content_text, created_at
                    FROM messages
                    WHERE conversation_id = c.id
                    ORDER BY created_at DESC
                    LIMIT 1
                ) latest ON TRUE
                WHERE cm.agent_profile_id = $1
                  AND cm.left_at IS NULL
                ORDER BY updated_at DESC
                "#,
                &[&agent_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_conversation_preview)
        .collect()
    }

    fn list_company_conversations(&self, company_id: Uuid) -> Vec<ConversationPreview> {
        self.list_company_conversations_result(company_id)
            .unwrap_or_default()
    }

    fn list_company_conversations_result(
        &self,
        company_id: Uuid,
    ) -> AppResult<Vec<ConversationPreview>> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT conversation.id,
                       COALESCE(conversation.title, '') AS title,
                       conversation.conversation_type,
                       latest.content_text AS last_message_preview,
                       COALESCE(latest.created_at, conversation.updated_at, conversation.created_at)
                           AS updated_at
                FROM conversations conversation
                LEFT JOIN LATERAL (
                    SELECT content_text, created_at
                    FROM messages
                    WHERE conversation_id = conversation.id
                    ORDER BY created_at DESC
                    LIMIT 1
                ) latest ON TRUE
                WHERE conversation.company_id = $1
                  AND conversation.context_type IN (
                      'company_all', 'company_direct', 'company_group', 'project_group'
                  )
                ORDER BY updated_at DESC
                "#,
                &[&company_id],
            )
        })
        .map(|rows| rows.into_iter().map(map_conversation_preview).collect())
    }

    fn get_conversation_messages(&self, conversation_id: Uuid) -> Vec<MessageView> {
        self.get_conversation_messages_result(conversation_id)
            .unwrap_or_default()
    }

    fn get_conversation_messages_result(
        &self,
        conversation_id: Uuid,
    ) -> AppResult<Vec<MessageView>> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, conversation_id, sender_agent_id, sender_human_user_id,
                       content_text, content_json, created_at
                FROM messages
                WHERE conversation_id = $1
                ORDER BY created_at ASC
                "#,
                &[&conversation_id],
            )
        })
        .map(|rows| rows.into_iter().map(map_message_view).collect())
    }

    fn get_conversation_message_page(
        &self,
        conversation_id: Uuid,
        before_message_id: Option<Uuid>,
        limit: usize,
    ) -> AppResult<MessagePageView> {
        let cursor = match before_message_id {
            Some(before_message_id) => Some(
                self.with_client(|client| {
                    client.query_opt(
                        r#"
                        SELECT created_at, id
                        FROM messages
                        WHERE conversation_id = $1 AND id = $2
                        "#,
                        &[&conversation_id, &before_message_id],
                    )
                })?
                .map(|row| {
                    (
                        row.get::<_, chrono::DateTime<chrono::Utc>>("created_at"),
                        row.get::<_, Uuid>("id"),
                    )
                })
                .ok_or_else(|| {
                    AppError::Validation(
                        "message cursor does not belong to the conversation".into(),
                    )
                })?,
            ),
            None => None,
        };
        let limit = limit.clamp(1, 100);
        let query_limit = i64::try_from(limit + 1).unwrap_or(101);
        let rows = self.with_client(|client| {
            if let Some((cursor_created_at, cursor_id)) = cursor {
                client.query(
                    r#"
                    SELECT id, conversation_id, sender_agent_id, sender_human_user_id,
                           content_text, content_json, created_at
                    FROM messages
                    WHERE conversation_id = $1
                      AND (created_at, id) < ($2, $3)
                    ORDER BY created_at DESC, id DESC
                    LIMIT $4
                    "#,
                    &[
                        &conversation_id,
                        &cursor_created_at,
                        &cursor_id,
                        &query_limit,
                    ],
                )
            } else {
                client.query(
                    r#"
                    SELECT id, conversation_id, sender_agent_id, sender_human_user_id,
                           content_text, content_json, created_at
                    FROM messages
                    WHERE conversation_id = $1
                    ORDER BY created_at DESC, id DESC
                    LIMIT $2
                    "#,
                    &[&conversation_id, &query_limit],
                )
            }
        })?;
        let mut messages = rows.into_iter().map(map_message_view).collect::<Vec<_>>();
        let has_more = messages.len() > limit;
        if has_more {
            messages.truncate(limit);
        }
        messages.reverse();
        Ok(MessagePageView {
            next_cursor: has_more.then(|| messages[0].id),
            messages,
            has_more,
        })
    }

    fn append_message(&self, message: MessageView) -> AppResult<()> {
        self.append_message_with_metadata(message, serde_json::json!({}))
    }

    fn append_message_with_metadata(
        &self,
        message: MessageView,
        content_json: serde_json::Value,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;

            tx.execute(
                r#"
                INSERT INTO messages (
                    id, conversation_id, sender_agent_id, sender_human_user_id, message_type,
                    content_text, content_json, client_message_id, created_at
                )
                VALUES ($1, $2, $3, $4, 'text', $5, $6, NULL, $7)
                "#,
                &[
                    &message.id,
                    &message.conversation_id,
                    &message.sender_agent_id,
                    &message.sender_human_user_id,
                    &message.content,
                    &content_json,
                    &message.created_at,
                ],
            )?;

            tx.execute(
                r#"
                UPDATE conversations
                SET last_message_at = $2, updated_at = $2
                WHERE id = $1
                "#,
                &[&message.conversation_id, &message.created_at],
            )?;

            tx.commit()?;
            Ok(())
        })
    }

    fn conversation_exists(&self, conversation_id: Uuid) -> bool {
        self.with_client(|client| {
            client
                .query_one(
                    "SELECT COUNT(1) FROM conversations WHERE id = $1",
                    &[&conversation_id],
                )
                .map(|row| row.get::<_, i64>(0) > 0)
        })
        .unwrap_or(false)
    }

    fn list_agent_action_logs(&self, agent_id: Uuid, limit: usize) -> Vec<AgentActionLog> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, agent_profile_id, action_type, target_ref,
                       request_payload, result_payload, status, trace_id, created_at
                FROM agent_action_logs
                WHERE agent_profile_id = $1
                ORDER BY created_at DESC
                LIMIT $2
                "#,
                &[&agent_id, &(limit as i64)],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_action_log)
        .collect()
    }
}
