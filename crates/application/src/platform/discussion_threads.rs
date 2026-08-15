use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn open_project_discussion_thread_for_human(
        &self,
        input: OpenProjectDiscussionThreadForHumanInput,
    ) -> AppResult<CompanyConversationView> {
        let project = self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        let (scope_type, title) = self.discussion_thread_scope(&project, &input)?;
        let thread = if let Some(thread) =
            self.repo
                .get_project_discussion_thread(project.id, scope_type, input.subject_id)
        {
            thread
        } else {
            let thread = ProjectDiscussionThread {
                id: Uuid::new_v4(),
                project_id: project.id,
                scope_type: scope_type.into(),
                subject_id: input.subject_id,
                conversation_id: Uuid::new_v4(),
                created_by_agent_id: None,
                created_by_human_user_id: Some(input.human_user_id),
                created_at: now_utc(),
            };
            let member_agent_ids = self
                .repo
                .list_company_project_members(project.id)
                .into_iter()
                .filter(|member| member.left_at.is_none())
                .map(|member| member.agent_profile_id)
                .collect();
            self.repo.complete_project_discussion_thread_creation(
                ProjectDiscussionThreadCreationBundle {
                    thread: thread.clone(),
                    company_id: project.company_id,
                    title: title.clone(),
                    member_agent_ids,
                },
            )?;
            thread
        };
        Ok(CompanyConversationView {
            preview: self
                .repo
                .list_company_conversations(project.company_id)
                .into_iter()
                .find(|preview| preview.id == thread.conversation_id)
                .unwrap_or(ConversationPreview {
                    id: thread.conversation_id,
                    title,
                    conversation_type: ConversationType::Group,
                    last_message_preview: None,
                    updated_at: thread.created_at,
                }),
            context: self
                .repo
                .get_conversation_context_result(thread.conversation_id)?
                .ok_or_else(|| AppError::NotFound("discussion conversation not found".into()))?,
            member_agent_ids: self
                .repo
                .list_conversation_member_ids(thread.conversation_id),
        })
    }

    fn discussion_thread_scope<'a>(
        &self,
        project: &CompanyProject,
        input: &'a OpenProjectDiscussionThreadForHumanInput,
    ) -> AppResult<(&'a str, String)> {
        match input.scope_type.as_str() {
            "task" => self
                .repo
                .get_company_project_task(input.subject_id)
                .filter(|task| task.project_id == project.id)
                .map(|task| ("task", format!("任务讨论 · {}", task.title)))
                .ok_or_else(|| AppError::NotFound("project task not found".into())),
            "blocker" => self
                .repo
                .get_project_task_blocker(input.subject_id)
                .filter(|blocker| blocker.project_id == project.id)
                .map(|blocker| ("blocker", format!("阻塞讨论 · {}", blocker.summary)))
                .ok_or_else(|| AppError::NotFound("project blocker not found".into())),
            "gate" => self
                .repo
                .get_project_gate(input.subject_id)
                .filter(|gate| gate.project_id == project.id)
                .map(|gate| ("gate", format!("Gate 讨论 · {}", gate.title)))
                .ok_or_else(|| AppError::NotFound("project Gate not found".into())),
            _ => Err(AppError::Validation(
                "unsupported discussion scope_type".into(),
            )),
        }
    }
}
