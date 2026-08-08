use super::*;

impl CompanyPlatformRepository for MemoryPlatformRepository {
    fn insert_company_bundle(&self, bundle: CompanyCreationBundle) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard
            .companies
            .values()
            .any(|company| company.slug == bundle.company.slug)
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "company slug is already in use".into(),
            ));
        }
        let company_id = bundle.company.id;
        let default_group_id = bundle.default_group.preview.id;
        guard.company_human_members.insert(
            (
                bundle.owner_membership.company_id,
                bundle.owner_membership.human_user_id,
            ),
            bundle.owner_membership,
        );
        guard
            .org_units
            .insert(bundle.root_org_unit.id, bundle.root_org_unit);
        guard.companies.insert(bundle.company.id, bundle.company);
        guard
            .company_default_groups
            .insert(company_id, bundle.default_group.preview);
        guard
            .conversation_contexts
            .insert(default_group_id, bundle.default_group.context);
        Ok(())
    }

    fn get_company(&self, company_id: Uuid) -> Option<Company> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.companies.get(&company_id).cloned()
    }

    fn get_company_by_slug(&self, slug: &str) -> Option<Company> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .companies
            .values()
            .find(|company| company.slug.eq_ignore_ascii_case(slug))
            .cloned()
    }

    fn list_human_companies(&self, human_user_id: Uuid) -> Vec<Company> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut companies = guard
            .company_human_members
            .values()
            .filter(|membership| {
                membership.human_user_id == human_user_id && membership.status == "active"
            })
            .filter_map(|membership| guard.companies.get(&membership.company_id).cloned())
            .collect::<Vec<_>>();
        companies.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        companies
    }

    fn get_company_human_member(
        &self,
        company_id: Uuid,
        human_user_id: Uuid,
    ) -> Option<CompanyHumanMember> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard
            .company_human_members
            .get(&(company_id, human_user_id))
            .cloned()
    }

    fn get_org_unit(&self, org_unit_id: Uuid) -> Option<OrgUnit> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.org_units.get(&org_unit_id).cloned()
    }

    fn insert_org_unit(&self, org_unit: OrgUnit) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.companies.contains_key(&org_unit.company_id) {
            return Err(ai_chat_shared::AppError::NotFound(
                "company not found".into(),
            ));
        }
        guard.org_units.insert(org_unit.id, org_unit);
        Ok(())
    }

    fn list_company_org_units(&self, company_id: Uuid) -> Vec<OrgUnit> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut org_units = guard
            .org_units
            .values()
            .filter(|org_unit| org_unit.company_id == company_id)
            .cloned()
            .collect::<Vec<_>>();
        org_units.sort_by(|left, right| {
            left.sort_order
                .cmp(&right.sort_order)
                .then_with(|| left.created_at.cmp(&right.created_at))
        });
        org_units
    }

    fn get_company_agent_membership(&self, agent_id: Uuid) -> Option<CompanyAgentMembership> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.company_agent_memberships.get(&agent_id).cloned()
    }

    fn list_company_agent_memberships(&self, company_id: Uuid) -> Vec<CompanyAgentMembership> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut memberships = guard
            .company_agent_memberships
            .values()
            .filter(|membership| membership.company_id == company_id)
            .cloned()
            .collect::<Vec<_>>();
        memberships.sort_by(|left, right| right.joined_at.cmp(&left.joined_at));
        memberships
    }

    fn update_company_agent_work_profile(
        &self,
        agent_id: Uuid,
        responsibilities: Vec<String>,
        skills: Vec<String>,
        current_focus: String,
        collaboration_preference: String,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let membership = guard
            .company_agent_memberships
            .get_mut(&agent_id)
            .ok_or_else(|| {
                ai_chat_shared::AppError::NotFound("company membership not found".into())
            })?;
        membership.responsibilities = responsibilities;
        membership.skills = skills;
        membership.current_focus = current_focus;
        membership.updated_at = updated_at;

        let profile = guard
            .agent_profiles
            .get_mut(&agent_id)
            .ok_or_else(|| ai_chat_shared::AppError::NotFound("agent profile not found".into()))?;
        profile.collaboration_preference = collaboration_preference;
        Ok(())
    }

    fn complete_company_agent_creation_bundle(
        &self,
        bundle: CompanyAgentCreationBundle,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard
            .agent_profiles
            .values()
            .any(|profile| profile.handle == bundle.agent_profile.handle)
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "agent handle is already in use".into(),
            ));
        }
        if guard
            .company_agent_memberships
            .contains_key(&bundle.agent_profile.id)
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "agent already belongs to a company".into(),
            ));
        }
        if guard
            .agent_keys_by_hash
            .contains_key(&bundle.key_record.key_hash)
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "agent key already exists".into(),
            ));
        }

        let company_id = bundle.membership.company_id;
        let self_notes_conversation_id = bundle.self_notes_conversation.id;
        guard.owner_bindings.push(bundle.owner_binding);
        guard
            .company_agent_memberships
            .insert(bundle.membership.agent_profile_id, bundle.membership);
        guard
            .agent_keys_by_hash
            .insert(bundle.key_record.key_hash.clone(), bundle.key_record.id);
        guard
            .agent_keys
            .insert(bundle.key_record.id, bundle.key_record);
        guard.agent_key_issue_logs.push(bundle.key_issue_log);
        guard
            .conversations
            .entry(bundle.agent_profile.id)
            .or_default()
            .push(bundle.self_notes_conversation);
        if let Some(default_group) = guard.company_default_groups.get(&company_id).cloned() {
            guard
                .conversations
                .entry(bundle.agent_profile.id)
                .or_default()
                .push(default_group);
        }
        guard.conversation_contexts.insert(
            self_notes_conversation_id,
            ConversationContext {
                conversation_id: self_notes_conversation_id,
                company_id: Some(company_id),
                project_id: None,
                context_type: CONVERSATION_CONTEXT_SELF_NOTES.into(),
                visibility: "private".into(),
            },
        );
        guard
            .agent_profiles
            .insert(bundle.agent_profile.id, bundle.agent_profile);
        Ok(())
    }

    fn complete_company_agent_membership_update(
        &self,
        bundle: CompanyAgentMembershipUpdateBundle,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard
            .company_agent_memberships
            .contains_key(&bundle.membership.agent_profile_id)
        {
            return Err(ai_chat_shared::AppError::NotFound(
                "company agent membership not found".into(),
            ));
        }
        guard
            .company_agent_memberships
            .insert(bundle.membership.agent_profile_id, bundle.membership);
        guard
            .agent_staffing_actions
            .insert(bundle.action.id, bundle.action);
        Ok(())
    }

    fn complete_agent_staffing_hire(&self, bundle: AgentStaffingHireBundle) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard
            .agent_profiles
            .values()
            .any(|profile| profile.handle == bundle.agent_profile.handle)
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "agent handle is already in use".into(),
            ));
        }
        if guard
            .company_agent_memberships
            .contains_key(&bundle.agent_profile.id)
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "agent already belongs to a company".into(),
            ));
        }
        if guard
            .agent_keys_by_hash
            .contains_key(&bundle.key_record.key_hash)
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "agent key already exists".into(),
            ));
        }

        let company_id = bundle.membership.company_id;
        let agent_id = bundle.agent_profile.id;
        let self_notes_conversation_id = bundle.self_notes_conversation.id;
        guard.owner_bindings.push(bundle.owner_binding);
        guard
            .company_agent_memberships
            .insert(bundle.membership.agent_profile_id, bundle.membership);
        guard
            .agent_keys_by_hash
            .insert(bundle.key_record.key_hash.clone(), bundle.key_record.id);
        guard
            .agent_keys
            .insert(bundle.key_record.id, bundle.key_record);
        guard.agent_key_issue_logs.push(bundle.key_issue_log);
        guard
            .conversations
            .entry(agent_id)
            .or_default()
            .push(bundle.self_notes_conversation);
        if let Some(default_group) = guard.company_default_groups.get(&company_id).cloned() {
            guard
                .conversations
                .entry(agent_id)
                .or_default()
                .push(default_group);
        }
        guard.conversation_contexts.insert(
            self_notes_conversation_id,
            ConversationContext {
                conversation_id: self_notes_conversation_id,
                company_id: Some(company_id),
                project_id: None,
                context_type: CONVERSATION_CONTEXT_SELF_NOTES.into(),
                visibility: "private".into(),
            },
        );
        guard
            .agent_staffing_actions
            .insert(bundle.action.id, bundle.action);
        guard
            .agent_profiles
            .insert(bundle.agent_profile.id, bundle.agent_profile);
        Ok(())
    }

    fn complete_company_agent_activation(
        &self,
        bundle: CompanyAgentActivationBundle,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        let company_id = bundle.membership.company_id;
        let agent_id = bundle.membership.agent_profile_id;
        if !guard.agent_profiles.contains_key(&bundle.agent_profile.id)
            || !guard
                .company_agent_memberships
                .contains_key(&bundle.membership.agent_profile_id)
        {
            return Err(ai_chat_shared::AppError::NotFound(
                "company agent not found".into(),
            ));
        }
        if guard
            .agent_keys_by_hash
            .contains_key(&bundle.key_record.key_hash)
        {
            return Err(ai_chat_shared::AppError::Conflict(
                "agent key already exists".into(),
            ));
        }

        guard
            .agent_profiles
            .insert(bundle.agent_profile.id, bundle.agent_profile);
        guard
            .company_agent_memberships
            .insert(bundle.membership.agent_profile_id, bundle.membership);
        guard
            .agent_keys_by_hash
            .insert(bundle.key_record.key_hash.clone(), bundle.key_record.id);
        guard
            .agent_keys
            .insert(bundle.key_record.id, bundle.key_record);
        guard.agent_key_issue_logs.push(bundle.key_issue_log);
        guard
            .agent_staffing_actions
            .insert(bundle.action.id, bundle.action);
        if let Some(default_group) = guard.company_default_groups.get(&company_id).cloned() {
            let conversations = guard.conversations.entry(agent_id).or_default();
            if !conversations
                .iter()
                .any(|conversation| conversation.id == default_group.id)
            {
                conversations.push(default_group);
            }
        }
        Ok(())
    }

    fn complete_agent_staffing_status_change(
        &self,
        bundle: AgentStaffingStatusChangeBundle,
    ) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.agent_profiles.contains_key(&bundle.agent_profile.id)
            || !guard
                .company_agent_memberships
                .contains_key(&bundle.membership.agent_profile_id)
        {
            return Err(ai_chat_shared::AppError::NotFound(
                "company agent not found".into(),
            ));
        }
        if bundle
            .reassigned_tasks
            .iter()
            .any(|task| !guard.company_project_tasks.contains_key(&task.id))
        {
            return Err(ai_chat_shared::AppError::NotFound(
                "company project task not found".into(),
            ));
        }
        let changed_at = bundle.action.created_at;
        for key in guard.agent_keys.values_mut() {
            if key.agent_profile_id == bundle.agent_profile.id && key.revoked_at.is_none() {
                key.revoked_at = Some(changed_at);
            }
        }
        guard
            .agent_profiles
            .insert(bundle.agent_profile.id, bundle.agent_profile);
        guard
            .company_agent_memberships
            .insert(bundle.membership.agent_profile_id, bundle.membership);
        for task in bundle.reassigned_tasks {
            if let Some(project) = guard.company_projects.get_mut(&task.project_id) {
                project.updated_at = task.updated_at;
                project.updated_by_agent_id = task.updated_by_agent_id;
            }
            guard.company_project_tasks.insert(task.id, task);
        }
        guard.agent_key_issue_logs.extend(bundle.key_issue_logs);
        guard
            .agent_staffing_actions
            .insert(bundle.action.id, bundle.action);
        Ok(())
    }

    fn get_agent_staffing_action(&self, action_id: Uuid) -> Option<AgentStaffingAction> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        guard.agent_staffing_actions.get(&action_id).cloned()
    }

    fn list_agent_staffing_actions(&self, company_id: Uuid) -> Vec<AgentStaffingAction> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let mut actions = guard
            .agent_staffing_actions
            .values()
            .filter(|action| action.company_id == company_id)
            .cloned()
            .collect::<Vec<_>>();
        actions.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        actions
    }
}
