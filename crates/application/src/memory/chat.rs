use super::*;
use crate::contracts::ProjectDiscussionThreadCreationBundle;
use ai_chat_domain::company::ProjectDiscussionThread;
use ai_chat_domain::social::ConversationType;

impl ChatPlatformRepository for MemoryPlatformRepository {
    fn ensure_agent_conversation_bucket(&self, agent_id: Uuid) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.conversations.entry(agent_id).or_default();
        Ok(())
    }

    fn insert_conversation_preview(
        &self,
        agent_id: Uuid,
        preview: ConversationPreview,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let preview_id = preview.id;
        guard
            .conversations
            .entry(agent_id)
            .or_default()
            .push(preview);
        let all_lists = guard.conversations.values().collect::<Vec<_>>();
        let participants = all_lists
            .iter()
            .filter(|items| items.iter().any(|item| item.id == preview_id))
            .count();
        if participants >= 2 {
            let owners = guard
                .conversations
                .iter()
                .filter_map(|(owner_id, items)| {
                    items
                        .iter()
                        .any(|item| item.id == preview_id)
                        .then_some(*owner_id)
                })
                .collect::<Vec<_>>();
            if owners.len() == 2 {
                let pair = ordered_pair(owners[0], owners[1]);
                guard.direct_conversations.insert(pair, preview_id);
            }
        }
        Ok(())
    }

    fn get_project_discussion_thread(
        &self,
        project_id: Uuid,
        scope_type: &str,
        subject_id: Uuid,
    ) -> Option<ProjectDiscussionThread> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .project_discussion_threads
            .get(&(project_id, scope_type.to_string(), subject_id))
            .cloned()
    }

    fn complete_project_discussion_thread_creation(
        &self,
        bundle: ProjectDiscussionThreadCreationBundle,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let key = (
            bundle.thread.project_id,
            bundle.thread.scope_type.clone(),
            bundle.thread.subject_id,
        );
        if guard.project_discussion_threads.contains_key(&key) {
            return Err(ai_chat_shared::AppError::Conflict(
                "project discussion thread already exists".into(),
            ));
        }
        let now = bundle.thread.created_at;
        for agent_id in &bundle.member_agent_ids {
            guard
                .conversations
                .entry(*agent_id)
                .or_default()
                .push(ConversationPreview {
                    id: bundle.thread.conversation_id,
                    title: bundle.title.clone(),
                    conversation_type: ConversationType::Group,
                    last_message_preview: None,
                    updated_at: now,
                });
        }
        guard.conversation_contexts.insert(
            bundle.thread.conversation_id,
            ConversationContext {
                conversation_id: bundle.thread.conversation_id,
                company_id: Some(bundle.company_id),
                project_id: Some(bundle.thread.project_id),
                context_type: format!("{}_thread", bundle.thread.scope_type),
                visibility: "members".into(),
            },
        );
        guard.project_discussion_threads.insert(key, bundle.thread);
        Ok(())
    }

    fn complete_company_conversation_creation(
        &self,
        bundle: CompanyConversationCreationBundle,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let first = bundle.members.first().ok_or_else(|| {
            ai_chat_shared::AppError::Validation("conversation members required".into())
        })?;
        let conversation_id = first.preview.id;
        if bundle
            .members
            .iter()
            .any(|member| member.preview.id != conversation_id)
        {
            return Err(ai_chat_shared::AppError::Validation(
                "company conversation previews must share one id".into(),
            ));
        }
        if bundle
            .members
            .iter()
            .any(|member| !guard.agent_profiles.contains_key(&member.agent_id))
        {
            return Err(ai_chat_shared::AppError::NotFound(
                "company conversation member not found".into(),
            ));
        }
        if let Some((left_agent_id, right_agent_id)) = bundle.direct_pair {
            let key = (bundle.company_id, left_agent_id, right_agent_id);
            if guard.company_direct_conversations.contains_key(&key) {
                return Err(ai_chat_shared::AppError::Conflict(
                    "company direct conversation already exists".into(),
                ));
            }
            guard
                .company_direct_conversations
                .insert(key, conversation_id);
            guard
                .direct_conversations
                .insert(ordered_pair(left_agent_id, right_agent_id), conversation_id);
        }
        for member in bundle.members {
            guard
                .conversations
                .entry(member.agent_id)
                .or_default()
                .push(member.preview);
        }
        guard.conversation_contexts.insert(
            conversation_id,
            ConversationContext {
                conversation_id,
                company_id: Some(bundle.company_id),
                project_id: None,
                context_type: bundle.context_type,
                visibility: bundle.visibility,
            },
        );
        Ok(())
    }

    fn complete_human_company_direct_conversation_creation(
        &self,
        bundle: HumanCompanyDirectConversationCreationBundle,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.human_users.contains_key(&bundle.human_user_id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "human conversation sender not found".into(),
            ));
        }
        if !guard.agent_profiles.contains_key(&bundle.target_agent_id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "human conversation target not found".into(),
            ));
        }
        let key = (
            bundle.company_id,
            bundle.human_user_id,
            bundle.target_agent_id,
        );
        if guard.company_human_direct_conversations.contains_key(&key) {
            return Err(ai_chat_shared::AppError::Conflict(
                "human company direct conversation already exists".into(),
            ));
        }
        let conversation_id = bundle.preview.id;
        guard
            .company_human_direct_conversations
            .insert(key, conversation_id);
        guard
            .conversations
            .entry(bundle.target_agent_id)
            .or_default()
            .push(bundle.preview);
        guard.conversation_contexts.insert(
            conversation_id,
            ConversationContext {
                conversation_id,
                company_id: Some(bundle.company_id),
                project_id: None,
                context_type: ai_chat_domain::social::CONVERSATION_CONTEXT_COMPANY_DIRECT.into(),
                visibility: "members".into(),
            },
        );
        Ok(())
    }

    fn find_company_direct_conversation(
        &self,
        company_id: Uuid,
        left_agent_id: Uuid,
        right_agent_id: Uuid,
    ) -> Option<ConversationPreview> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let (left_agent_id, right_agent_id) = ordered_pair(left_agent_id, right_agent_id);
        let conversation_id =
            guard
                .company_direct_conversations
                .get(&(company_id, left_agent_id, right_agent_id))?;
        guard
            .conversations
            .get(&left_agent_id)
            .and_then(|items| items.iter().find(|preview| preview.id == *conversation_id))
            .cloned()
    }

    fn find_human_company_direct_conversation(
        &self,
        company_id: Uuid,
        human_user_id: Uuid,
        target_agent_id: Uuid,
    ) -> Option<ConversationPreview> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let conversation_id = guard.company_human_direct_conversations.get(&(
            company_id,
            human_user_id,
            target_agent_id,
        ))?;
        guard
            .conversations
            .get(&target_agent_id)
            .and_then(|items| items.iter().find(|preview| preview.id == *conversation_id))
            .cloned()
    }

    fn get_conversation_context(&self, conversation_id: Uuid) -> Option<ConversationContext> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.conversation_contexts.get(&conversation_id).cloned()
    }

    fn list_conversation_member_ids(&self, conversation_id: Uuid) -> Vec<Uuid> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut member_ids = guard
            .conversations
            .iter()
            .filter_map(|(agent_id, previews)| {
                previews
                    .iter()
                    .any(|preview| preview.id == conversation_id)
                    .then_some(*agent_id)
            })
            .collect::<Vec<_>>();
        member_ids.sort();
        member_ids
    }

    fn list_owner_agents(&self, human_user_id: Uuid) -> Vec<AgentProfile> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .agent_profiles
            .values()
            .filter(|agent| agent.owner_user_id == human_user_id)
            .cloned()
            .collect()
    }

    fn agent_exists(&self, agent_id: Uuid) -> bool {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.agent_profiles.contains_key(&agent_id)
    }

    fn get_agent_profile(&self, agent_id: Uuid) -> Option<AgentProfile> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.agent_profiles.get(&agent_id).cloned()
    }

    fn list_agent_conversations(&self, agent_id: Uuid) -> Vec<ConversationPreview> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .conversations
            .get(&agent_id)
            .cloned()
            .unwrap_or_default()
    }

    fn list_company_conversations(&self, company_id: Uuid) -> Vec<ConversationPreview> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut conversations_by_id = HashMap::new();
        for previews in guard.conversations.values() {
            for preview in previews {
                if guard
                    .conversation_contexts
                    .get(&preview.id)
                    .is_some_and(|context| context.company_id == Some(company_id))
                {
                    conversations_by_id.insert(preview.id, preview.clone());
                }
            }
        }
        if let Some(default_group) = guard.company_default_groups.get(&company_id) {
            conversations_by_id
                .entry(default_group.id)
                .or_insert_with(|| default_group.clone());
        }
        let mut conversations = conversations_by_id.into_values().collect::<Vec<_>>();
        conversations.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| right.id.cmp(&left.id))
        });
        conversations
    }

    fn get_conversation_messages(&self, conversation_id: Uuid) -> Vec<MessageView> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .messages
            .get(&conversation_id)
            .cloned()
            .unwrap_or_default()
    }

    fn append_message(&self, message: MessageView) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard
            .messages
            .entry(message.conversation_id)
            .or_default()
            .push(message.clone());

        for previews in guard.conversations.values_mut() {
            for preview in previews.iter_mut() {
                if preview.id == message.conversation_id {
                    preview.last_message_preview = Some(if message.content.is_empty() {
                        format!("[{} 个附件]", message.attachments.len())
                    } else {
                        message.content.clone()
                    });
                    preview.updated_at = message.created_at;
                }
            }
        }
        Ok(())
    }

    fn conversation_exists(&self, conversation_id: Uuid) -> bool {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .conversations
            .values()
            .any(|items| items.iter().any(|item| item.id == conversation_id))
    }

    fn list_agent_action_logs(&self, agent_id: Uuid, limit: usize) -> Vec<AgentActionLog> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut logs = guard
            .action_logs
            .iter()
            .filter(|item| item.agent_profile_id == agent_id)
            .cloned()
            .collect::<Vec<_>>();
        logs.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        logs.truncate(limit);
        logs
    }
}

fn ordered_pair(left: Uuid, right: Uuid) -> (Uuid, Uuid) {
    if left.as_bytes() <= right.as_bytes() {
        (left, right)
    } else {
        (right, left)
    }
}
