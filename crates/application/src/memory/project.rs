use super::*;

impl ProjectPlatformRepository for MemoryPlatformRepository {
    fn complete_company_project_creation(
        &self,
        bundle: CompanyProjectCreationBundle,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.company_projects.contains_key(&bundle.project.id) {
            return Err(ai_chat_shared::AppError::Conflict(
                "company project already exists".into(),
            ));
        }
        if bundle.conversation_members.is_empty() {
            return Err(ai_chat_shared::AppError::Validation(
                "project conversation members required".into(),
            ));
        }
        let conversation_id = bundle.project.project_group_conversation_id;
        if bundle
            .conversation_members
            .iter()
            .any(|member| member.preview.id != conversation_id)
        {
            return Err(ai_chat_shared::AppError::Validation(
                "project conversation previews must share one id".into(),
            ));
        }
        for conversation_member in bundle.conversation_members {
            guard
                .conversations
                .entry(conversation_member.agent_id)
                .or_default()
                .push(conversation_member.preview);
        }
        guard.conversation_contexts.insert(
            conversation_id,
            ConversationContext {
                conversation_id,
                company_id: Some(bundle.project.company_id),
                project_id: Some(bundle.project.id),
                context_type: CONVERSATION_CONTEXT_PROJECT_GROUP.into(),
                visibility: "members".into(),
            },
        );
        for member in bundle.members {
            guard
                .company_project_members
                .insert((member.project_id, member.agent_profile_id), member);
        }
        guard
            .company_projects
            .insert(bundle.project.id, bundle.project);
        Ok(())
    }

    fn get_company_project(&self, project_id: Uuid) -> Option<CompanyProject> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.company_projects.get(&project_id).cloned()
    }

    fn list_company_projects(&self, company_id: Uuid) -> Vec<CompanyProject> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut projects = guard
            .company_projects
            .values()
            .filter(|project| project.company_id == company_id)
            .cloned()
            .collect::<Vec<_>>();
        projects.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        projects
    }

    fn save_company_project_git_config(&self, config: CompanyProjectGitConfig) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.company_projects.contains_key(&config.project_id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project not found".into(),
            ));
        }
        guard
            .company_project_git_configs
            .insert(config.project_id, config);
        Ok(())
    }

    fn get_company_project_git_config(&self, project_id: Uuid) -> Option<CompanyProjectGitConfig> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.company_project_git_configs.get(&project_id).cloned()
    }

    fn delete_company_project_git_config(&self, project_id: Uuid) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        guard.company_project_git_configs.remove(&project_id);
        Ok(())
    }

    fn save_company_project_rule(&self, rule: CompanyProjectRule) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.company_projects.contains_key(&rule.project_id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project not found".into(),
            ));
        }
        guard.company_project_rules.insert(rule.project_id, rule);
        Ok(())
    }

    fn get_company_project_rule(&self, project_id: Uuid) -> Option<CompanyProjectRule> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.company_project_rules.get(&project_id).cloned()
    }

    fn replace_company_project_assets(
        &self,
        project_id: Uuid,
        assets: Vec<CompanyProjectAsset>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.company_projects.contains_key(&project_id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project not found".into(),
            ));
        }
        let completed_at = assets
            .first()
            .map(|asset| asset.updated_at)
            .unwrap_or_else(Utc::now);
        guard.company_project_assets.insert(project_id, assets);
        if let Some(config) = guard
            .company_project_asset_refresh_configs
            .get_mut(&project_id)
        {
            config.last_completed_at = Some(completed_at);
            config.next_refresh_at =
                completed_at + chrono::Duration::minutes(i64::from(config.interval_minutes));
            config.updated_at = completed_at;
        }
        Ok(())
    }

    fn list_company_project_assets(&self, project_id: Uuid) -> Vec<CompanyProjectAsset> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .company_project_assets
            .get(&project_id)
            .cloned()
            .unwrap_or_default()
    }

    fn save_company_project_asset_refresh_config(
        &self,
        config: CompanyProjectAssetRefreshConfig,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.company_projects.contains_key(&config.project_id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project not found".into(),
            ));
        }
        guard
            .company_project_asset_refresh_configs
            .insert(config.project_id, config);
        Ok(())
    }

    fn get_company_project_asset_refresh_config(
        &self,
        project_id: Uuid,
    ) -> Option<CompanyProjectAssetRefreshConfig> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .company_project_asset_refresh_configs
            .get(&project_id)
            .cloned()
    }

    fn claim_due_company_project_asset_refresh(
        &self,
        agent_id: Uuid,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<Option<CompanyProjectAssetRefreshConfig>> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let project_id = guard
            .company_project_asset_refresh_configs
            .values()
            .filter(|config| {
                config.enabled
                    && config.maintainer_agent_id == agent_id
                    && config.next_refresh_at <= now
                    && guard
                        .company_projects
                        .get(&config.project_id)
                        .is_some_and(|project| project.status != PROJECT_STATUS_PAUSED)
            })
            .min_by(|left, right| {
                left.next_refresh_at
                    .cmp(&right.next_refresh_at)
                    .then_with(|| left.project_id.cmp(&right.project_id))
            })
            .map(|config| config.project_id);
        let Some(project_id) = project_id else {
            return Ok(None);
        };
        let config = guard
            .company_project_asset_refresh_configs
            .get_mut(&project_id)
            .expect("selected asset refresh config must exist");
        config.last_requested_at = Some(now);
        config.next_refresh_at =
            now + chrono::Duration::minutes(i64::from(config.interval_minutes));
        config.updated_at = now;
        Ok(Some(config.clone()))
    }

    fn mark_company_project_asset_refresh_completed(
        &self,
        project_id: Uuid,
        completed_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if let Some(config) = guard
            .company_project_asset_refresh_configs
            .get_mut(&project_id)
        {
            config.last_completed_at = Some(completed_at);
            config.next_refresh_at =
                completed_at + chrono::Duration::minutes(i64::from(config.interval_minutes));
            config.updated_at = completed_at;
        }
        Ok(())
    }

    fn update_company_project(&self, project: CompanyProject) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.company_projects.contains_key(&project.id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project not found".into(),
            ));
        }
        guard.company_projects.insert(project.id, project);
        Ok(())
    }

    fn update_company_project_metadata(
        &self,
        project: CompanyProject,
        project_group_title: String,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.company_projects.contains_key(&project.id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project not found".into(),
            ));
        }
        for previews in guard.conversations.values_mut() {
            if let Some(preview) = previews
                .iter_mut()
                .find(|preview| preview.id == project.project_group_conversation_id)
            {
                preview.title = project_group_title.clone();
                preview.updated_at = project.updated_at;
            }
        }
        guard.company_projects.insert(project.id, project);
        Ok(())
    }

    fn get_company_project_member(
        &self,
        project_id: Uuid,
        agent_id: Uuid,
    ) -> Option<CompanyProjectMember> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .company_project_members
            .get(&(project_id, agent_id))
            .cloned()
    }

    fn list_company_project_members(&self, project_id: Uuid) -> Vec<CompanyProjectMember> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut members = guard
            .company_project_members
            .values()
            .filter(|member| member.project_id == project_id)
            .cloned()
            .collect::<Vec<_>>();
        members.sort_by(|left, right| left.joined_at.cmp(&right.joined_at));
        members
    }

    fn complete_company_project_member_add(
        &self,
        bundle: CompanyProjectMemberAddBundle,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard
            .company_projects
            .contains_key(&bundle.member.project_id)
        {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project not found".into(),
            ));
        }
        let project_id = bundle.member.project_id;
        let joined_at = bundle.member.joined_at;
        guard
            .company_project_members
            .insert((project_id, bundle.member.agent_profile_id), bundle.member);
        let previews = guard
            .conversations
            .entry(bundle.conversation_preview.agent_id)
            .or_default();
        previews.retain(|preview| preview.id != bundle.conversation_preview.preview.id);
        previews.push(bundle.conversation_preview.preview);
        if let Some(project) = guard.company_projects.get_mut(&project_id) {
            project.updated_at = joined_at;
        }
        Ok(())
    }

    fn complete_company_project_member_remove(
        &self,
        project_id: Uuid,
        agent_id: Uuid,
        conversation_id: Uuid,
        left_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let member = guard
            .company_project_members
            .get_mut(&(project_id, agent_id))
            .ok_or_else(|| ai_chat_shared::AppError::NotFound("project member not found".into()))?;
        member.left_at = Some(left_at);
        if let Some(previews) = guard.conversations.get_mut(&agent_id) {
            previews.retain(|preview| preview.id != conversation_id);
        }
        if let Some(project) = guard.company_projects.get_mut(&project_id) {
            project.updated_at = left_at;
        }
        Ok(())
    }
}
