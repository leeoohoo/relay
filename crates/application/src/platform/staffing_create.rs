use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn create_org_unit(&self, input: CreateOrgUnitInput) -> AppResult<OrgUnit> {
        let company = self
            .repo
            .get_company_result(input.company_id)?
            .ok_or_else(|| AppError::NotFound("company not found".into()))?;
        let human_membership = self
            .repo
            .get_company_human_member_result(company.id, input.human_user_id)?
            .filter(|membership| membership.status == "active")
            .ok_or_else(|| {
                AppError::Unauthorized("human user is not an active company member".into())
            })?;
        if !matches!(
            human_membership.role.as_str(),
            COMPANY_ROLE_OWNER | COMPANY_ROLE_ADMIN
        ) {
            return Err(AppError::Unauthorized(
                "company owner or admin role is required to manage organization units".into(),
            ));
        }
        let name = input.name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(AppError::Validation(
                "org unit name must contain 1 to 80 characters".into(),
            ));
        }
        if !matches!(input.unit_type.as_str(), "division" | "department" | "team") {
            return Err(AppError::Validation(
                "unit_type must be division, department, or team".into(),
            ));
        }
        let existing_units = self.repo.list_company_org_units(company.id);
        let parent_org_unit_id = match input.parent_org_unit_id {
            Some(parent_id) => {
                let parent = existing_units
                    .iter()
                    .find(|org_unit| org_unit.id == parent_id && org_unit.status == "active")
                    .ok_or_else(|| {
                        AppError::Unauthorized(
                            "parent org unit does not belong to the company".into(),
                        )
                    })?;
                Some(parent.id)
            }
            None => Some(
                existing_units
                    .iter()
                    .find(|org_unit| org_unit.parent_org_unit_id.is_none())
                    .ok_or_else(|| AppError::NotFound("company root org unit not found".into()))?
                    .id,
            ),
        };
        if existing_units.iter().any(|org_unit| {
            org_unit.parent_org_unit_id == parent_org_unit_id
                && org_unit.name.eq_ignore_ascii_case(name)
                && org_unit.status == "active"
        }) {
            return Err(AppError::Conflict(
                "an organization unit with this name already exists under the parent".into(),
            ));
        }
        let now = now_utc();
        let org_unit = OrgUnit {
            id: Uuid::new_v4(),
            company_id: company.id,
            parent_org_unit_id,
            name: name.to_string(),
            unit_type: input.unit_type,
            sort_order: input.sort_order.unwrap_or(0).clamp(-10_000, 10_000),
            status: "active".into(),
            created_at: now,
            updated_at: now,
        };
        self.repo.insert_org_unit(org_unit.clone())?;
        Ok(org_unit)
    }

    pub fn create_company_agent(
        &self,
        input: CreateCompanyAgentInput,
    ) -> AppResult<CreateCompanyAgentResult> {
        let company = self
            .repo
            .get_company_result(input.company_id)?
            .ok_or_else(|| AppError::NotFound("company not found".into()))?;
        let human_membership = self
            .repo
            .get_company_human_member_result(input.company_id, input.human_user_id)?
            .filter(|membership| membership.status == "active")
            .ok_or_else(|| {
                AppError::Unauthorized("human user is not an active company member".into())
            })?;
        if !matches!(
            human_membership.role.as_str(),
            COMPANY_ROLE_OWNER | COMPANY_ROLE_ADMIN
        ) {
            return Err(AppError::Unauthorized(
                "company owner or admin role is required to create an agent".into(),
            ));
        }

        let display_name = input.display_name.trim();
        if display_name.is_empty() || display_name.chars().count() > 80 {
            return Err(AppError::Validation(
                "agent display_name must contain 1 to 80 characters".into(),
            ));
        }
        let handle = normalize_handle(&input.handle);
        if handle.is_empty() || handle.chars().count() > 64 {
            return Err(AppError::Validation(
                "agent handle must contain 1 to 64 characters".into(),
            ));
        }
        if self
            .repo
            .list_agent_profiles()
            .into_iter()
            .any(|profile| profile.handle == handle)
        {
            return Err(AppError::Conflict("agent handle is already in use".into()));
        }

        let org_units = self.repo.list_company_org_units(company.id);
        let org_unit = match input.org_unit_id {
            Some(org_unit_id) => org_units
                .iter()
                .find(|org_unit| org_unit.id == org_unit_id && org_unit.status == "active")
                .cloned()
                .ok_or_else(|| {
                    AppError::Unauthorized("org unit does not belong to the company".into())
                })?,
            None => org_units
                .iter()
                .find(|org_unit| org_unit.parent_org_unit_id.is_none())
                .cloned()
                .ok_or_else(|| AppError::NotFound("company root org unit not found".into()))?,
        };

        let existing_memberships = self.repo.list_company_agent_memberships(company.id);
        let governance = self.effective_company_governance_policy_settings(company.id);
        let current_staff_count = existing_memberships
            .iter()
            .filter(|membership| membership.employment_status != "terminated")
            .count();
        if current_staff_count >= governance.agent_staff_limit as usize {
            return Err(AppError::RateLimited(format!(
                "company agent limit of {} has been reached",
                governance.agent_staff_limit
            )));
        }
        let requested_role_key = input
            .role_key
            .unwrap_or_else(|| COMPANY_AGENT_ROLE_MEMBER.into());
        if !matches!(
            requested_role_key.as_str(),
            COMPANY_AGENT_ROLE_MANAGER | COMPANY_AGENT_ROLE_MEMBER
        ) {
            return Err(AppError::Validation(
                "role_key must be company_manager or member".into(),
            ));
        }
        let has_active_manager = existing_memberships.iter().any(|membership| {
            membership.role_key == COMPANY_AGENT_ROLE_MANAGER
                && membership.employment_status == "active"
        });
        let role_key = if has_active_manager {
            requested_role_key
        } else {
            COMPANY_AGENT_ROLE_MANAGER.into()
        };
        if let Some(manager_membership_id) = input.reports_to_membership_id {
            let manager = existing_memberships
                .iter()
                .find(|membership| membership.id == manager_membership_id)
                .ok_or_else(|| {
                    AppError::Unauthorized(
                        "reporting manager does not belong to the company".into(),
                    )
                })?;
            if manager.employment_status != "active" {
                return Err(AppError::Conflict("reporting manager is not active".into()));
            }
        }

        let now = now_utc();
        let profession = infer_company_profession(input.job_title.as_deref());
        let agent_profile = AgentProfile {
            id: Uuid::new_v4(),
            owner_user_id: input.human_user_id,
            display_name: display_name.to_string(),
            handle,
            persona: input.persona.trim().to_string(),
            collaboration_preference: AGENT_COLLABORATION_PREFERENCE_AVAILABLE.into(),
            status: AgentStatus::Active,
            created_at: now,
        };
        let owner_binding = AgentOwnerBinding {
            id: Uuid::new_v4(),
            human_user_id: input.human_user_id,
            agent_profile_id: agent_profile.id,
            created_at: now,
        };
        let membership = CompanyAgentMembership {
            id: Uuid::new_v4(),
            company_id: company.id,
            agent_profile_id: agent_profile.id,
            org_unit_id: org_unit.id,
            job_title: profession.label.clone(),
            role_key: role_key.clone(),
            reports_to_membership_id: input.reports_to_membership_id,
            permissions: default_company_agent_permissions_for_profession(
                &role_key,
                &profession.key,
            ),
            responsibilities: if agent_profile.persona.is_empty() {
                Vec::new()
            } else {
                vec![agent_profile.persona.clone()]
            },
            skills: Vec::new(),
            current_focus: String::new(),
            staffing_scope_org_unit_id: None,
            employment_status: "active".into(),
            joined_at: now,
            terminated_at: None,
            created_by_human_user_id: Some(input.human_user_id),
            created_by_agent_id: None,
            updated_at: now,
        };
        let plaintext_key = generate_agent_key();
        let key_prefix = plaintext_key.chars().take(12).collect::<String>();
        let key_record = AgentKeyRecord {
            id: Uuid::new_v4(),
            agent_profile_id: agent_profile.id,
            key_name: "primary".into(),
            key_prefix: key_prefix.clone(),
            key_hash: hash_secret(&plaintext_key),
            last_used_at: None,
            expires_at: Some(now + Duration::days(180)),
            revoked_at: None,
            created_at: now,
        };
        let key_issue_log = AgentKeyIssueLog {
            id: Uuid::new_v4(),
            agent_profile_id: agent_profile.id,
            agent_key_id: Some(key_record.id),
            issue_type: AgentKeyIssueType::Issued,
            issued_by_user_id: Some(input.human_user_id),
            metadata: json!({
                "agent_key_prefix": key_prefix,
                "creation_mode": "company_direct",
                "company_id": company.id,
                "company_role": role_key,
                "org_unit_id": org_unit.id
            }),
            created_at: now,
        };
        let self_notes_conversation = ConversationPreview {
            id: Uuid::new_v4(),
            title: "Self Notes".into(),
            conversation_type: ConversationType::Direct,
            last_message_preview: None,
            updated_at: now,
        };
        self.repo
            .complete_company_agent_creation_bundle(CompanyAgentCreationBundle {
                agent_profile: agent_profile.clone(),
                owner_binding,
                membership: membership.clone(),
                key_record,
                key_issue_log,
                self_notes_conversation,
            })?;

        Ok(CreateCompanyAgentResult {
            company,
            agent_profile,
            membership,
            agent_key_plaintext: plaintext_key,
            agent_key_prefix: key_prefix,
        })
    }

    pub fn update_company_agent_staffing_permissions(
        &self,
        input: UpdateCompanyAgentPermissionsInput,
    ) -> AppResult<CompanyAgentMembership> {
        self.ensure_company_human_manager(input.company_id, input.human_user_id)?;
        let mut membership = self
            .repo
            .get_company_agent_membership_result(input.agent_id)?
            .filter(|membership| membership.company_id == input.company_id)
            .ok_or_else(|| AppError::NotFound("company agent membership not found".into()))?;
        if membership.employment_status == "terminated" {
            return Err(AppError::Conflict(
                "terminated agent permissions cannot be changed".into(),
            ));
        }

        let staffing_allowed = [
            COMPANY_PERMISSION_STAFF_HIRE,
            COMPANY_PERMISSION_STAFF_SUSPEND,
            COMPANY_PERMISSION_STAFF_TERMINATE,
        ];
        let project_allowed = [
            COMPANY_PERMISSION_PROJECT_RULES_MANAGE,
            COMPANY_PERMISSION_PROJECT_ASSETS_MANAGE,
        ];
        let mut staffing_permissions = input
            .staffing_permissions
            .into_iter()
            .map(|permission| permission.trim().to_string())
            .filter(|permission| !permission.is_empty())
            .collect::<Vec<_>>();
        staffing_permissions.sort();
        staffing_permissions.dedup();
        if let Some(permission) = staffing_permissions
            .iter()
            .find(|permission| !staffing_allowed.contains(&permission.as_str()))
        {
            return Err(AppError::Validation(format!(
                "unsupported staffing permission: {permission}"
            )));
        }
        let mut project_permissions = input
            .project_permissions
            .into_iter()
            .map(|permission| permission.trim().to_string())
            .filter(|permission| !permission.is_empty())
            .collect::<Vec<_>>();
        project_permissions.sort();
        project_permissions.dedup();
        if let Some(permission) = project_permissions
            .iter()
            .find(|permission| !project_allowed.contains(&permission.as_str()))
        {
            return Err(AppError::Validation(format!(
                "unsupported project permission: {permission}"
            )));
        }

        let staffing_scope_org_unit_id = match input.staffing_scope_org_unit_id {
            Some(org_unit_id) => Some(
                self.repo
                    .get_org_unit(org_unit_id)
                    .filter(|org_unit| {
                        org_unit.company_id == input.company_id && org_unit.status == "active"
                    })
                    .ok_or_else(|| {
                        AppError::Validation(
                            "staffing scope must be an active org unit in the company".into(),
                        )
                    })?
                    .id,
            ),
            None => None,
        };

        let previous_permissions = membership.permissions.clone();
        let previous_staffing_scope_org_unit_id = membership.staffing_scope_org_unit_id;
        membership.permissions.retain(|permission| {
            !staffing_allowed.contains(&permission.as_str())
                && !project_allowed.contains(&permission.as_str())
        });
        membership.permissions.extend(staffing_permissions.clone());
        membership.permissions.extend(project_permissions.clone());
        membership.permissions.sort();
        membership.permissions.dedup();
        membership.staffing_scope_org_unit_id = staffing_scope_org_unit_id;
        membership.updated_at = now_utc();

        let action = AgentStaffingAction {
            id: Uuid::new_v4(),
            company_id: input.company_id,
            action_type: STAFFING_ACTION_PERMISSION_UPDATE.into(),
            actor_type: STAFFING_ACTOR_HUMAN.into(),
            actor_human_user_id: Some(input.human_user_id),
            actor_agent_id: None,
            target_agent_id: Some(input.agent_id),
            requested_org_unit_id: Some(membership.org_unit_id),
            requested_role_key: Some(membership.role_key.clone()),
            reason: normalize_staffing_text(input.reason, 500, "reason")?,
            handoff_plan: String::new(),
            status: STAFFING_STATUS_COMPLETED.into(),
            approval_required: false,
            approved_by_human_user_id: Some(input.human_user_id),
            request_payload: json!({
                "previous_permissions": previous_permissions,
                "previous_staffing_scope_org_unit_id": previous_staffing_scope_org_unit_id,
                "staffing_permissions": staffing_permissions,
                "project_permissions": project_permissions,
                "staffing_scope_org_unit_id": membership.staffing_scope_org_unit_id,
            }),
            result_payload: json!({
                "permissions": membership.permissions,
                "staffing_scope_org_unit_id": membership.staffing_scope_org_unit_id,
            }),
            idempotency_key: None,
            created_at: membership.updated_at,
            completed_at: Some(membership.updated_at),
        };
        self.repo
            .complete_company_agent_membership_update(CompanyAgentMembershipUpdateBundle {
                membership: membership.clone(),
                action,
            })?;
        Ok(membership)
    }

    pub fn update_company_agent_role(
        &self,
        input: UpdateCompanyAgentRoleInput,
    ) -> AppResult<CompanyAgentMembership> {
        self.ensure_company_human_manager(input.company_id, input.human_user_id)?;
        let role_key = input.role_key.trim();
        if !matches!(
            role_key,
            COMPANY_AGENT_ROLE_MANAGER | COMPANY_AGENT_ROLE_MEMBER
        ) {
            return Err(AppError::Validation(
                "role_key must be company_manager or member".into(),
            ));
        }

        let memberships = self.repo.list_company_agent_memberships(input.company_id);
        let mut membership = memberships
            .iter()
            .find(|membership| membership.agent_profile_id == input.agent_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound("company agent membership not found".into()))?;
        if membership.employment_status == "terminated" {
            return Err(AppError::Conflict(
                "terminated agent role cannot be changed".into(),
            ));
        }
        if membership.role_key == COMPANY_AGENT_ROLE_MANAGER
            && role_key == COMPANY_AGENT_ROLE_MEMBER
            && membership.employment_status == "active"
            && memberships
                .iter()
                .filter(|membership| {
                    membership.role_key == COMPANY_AGENT_ROLE_MANAGER
                        && membership.employment_status == "active"
                })
                .count()
                <= 1
        {
            return Err(AppError::Conflict(
                "the last active company manager agent cannot be demoted".into(),
            ));
        }

        let previous_role_key = membership.role_key.clone();
        let previous_permissions = membership.permissions.clone();
        let explicit_permissions = membership
            .permissions
            .iter()
            .filter(|permission| {
                matches!(
                    permission.as_str(),
                    COMPANY_PERMISSION_STAFF_HIRE
                        | COMPANY_PERMISSION_STAFF_SUSPEND
                        | COMPANY_PERMISSION_STAFF_TERMINATE
                        | COMPANY_PERMISSION_PROJECT_RULES_MANAGE
                        | COMPANY_PERMISSION_PROJECT_ASSETS_MANAGE
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        membership.role_key = role_key.to_string();
        let profession = infer_company_profession(Some(&membership.job_title));
        membership.permissions =
            default_company_agent_permissions_for_profession(role_key, &profession.key);
        membership.permissions.extend(explicit_permissions);
        membership.permissions.sort();
        membership.permissions.dedup();
        membership.updated_at = now_utc();

        let action = AgentStaffingAction {
            id: Uuid::new_v4(),
            company_id: input.company_id,
            action_type: STAFFING_ACTION_ROLE_UPDATE.into(),
            actor_type: STAFFING_ACTOR_HUMAN.into(),
            actor_human_user_id: Some(input.human_user_id),
            actor_agent_id: None,
            target_agent_id: Some(input.agent_id),
            requested_org_unit_id: Some(membership.org_unit_id),
            requested_role_key: Some(membership.role_key.clone()),
            reason: normalize_staffing_text(input.reason, 500, "reason")?,
            handoff_plan: String::new(),
            status: STAFFING_STATUS_COMPLETED.into(),
            approval_required: false,
            approved_by_human_user_id: Some(input.human_user_id),
            request_payload: json!({
                "previous_role_key": previous_role_key,
                "previous_permissions": previous_permissions,
                "role_key": membership.role_key,
            }),
            result_payload: json!({
                "role_key": membership.role_key,
                "permissions": membership.permissions,
                "staffing_scope_org_unit_id": membership.staffing_scope_org_unit_id,
            }),
            idempotency_key: None,
            created_at: membership.updated_at,
            completed_at: Some(membership.updated_at),
        };
        self.repo
            .complete_company_agent_membership_update(CompanyAgentMembershipUpdateBundle {
                membership: membership.clone(),
                action,
            })?;
        Ok(membership)
    }

    pub fn update_company_agent_profession(
        &self,
        input: UpdateCompanyAgentProfessionInput,
    ) -> AppResult<CompanyAgentMembership> {
        self.ensure_company_human_manager(input.company_id, input.human_user_id)?;
        let profession = ai_chat_domain::company::company_profession_by_key(&input.profession_key)
            .ok_or_else(|| AppError::Validation("unsupported profession_key".into()))?;
        let mut membership = self
            .repo
            .get_company_agent_membership_result(input.agent_id)?
            .filter(|membership| membership.company_id == input.company_id)
            .ok_or_else(|| AppError::NotFound("company agent membership not found".into()))?;
        if membership.employment_status == "terminated" {
            return Err(AppError::Conflict(
                "terminated agent profession cannot be changed".into(),
            ));
        }

        let previous_job_title = membership.job_title.clone();
        let previous_permissions = membership.permissions.clone();
        let explicit_permissions = membership
            .permissions
            .iter()
            .filter(|permission| {
                matches!(
                    permission.as_str(),
                    COMPANY_PERMISSION_STAFF_HIRE
                        | COMPANY_PERMISSION_STAFF_SUSPEND
                        | COMPANY_PERMISSION_STAFF_TERMINATE
                        | COMPANY_PERMISSION_PROJECT_RULES_MANAGE
                        | COMPANY_PERMISSION_PROJECT_ASSETS_MANAGE
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        membership.job_title = profession.label.clone();
        membership.permissions =
            default_company_agent_permissions_for_profession(&membership.role_key, &profession.key);
        membership.permissions.extend(explicit_permissions);
        membership.permissions.sort();
        membership.permissions.dedup();
        membership.updated_at = now_utc();

        let action = AgentStaffingAction {
            id: Uuid::new_v4(),
            company_id: input.company_id,
            action_type: STAFFING_ACTION_PROFESSION_UPDATE.into(),
            actor_type: STAFFING_ACTOR_HUMAN.into(),
            actor_human_user_id: Some(input.human_user_id),
            actor_agent_id: None,
            target_agent_id: Some(input.agent_id),
            requested_org_unit_id: Some(membership.org_unit_id),
            requested_role_key: Some(membership.role_key.clone()),
            reason: normalize_staffing_text(input.reason, 500, "reason")?,
            handoff_plan: String::new(),
            status: STAFFING_STATUS_COMPLETED.into(),
            approval_required: false,
            approved_by_human_user_id: Some(input.human_user_id),
            request_payload: json!({
                "previous_job_title": previous_job_title,
                "previous_permissions": previous_permissions,
                "profession_key": profession.key,
            }),
            result_payload: json!({
                "job_title": membership.job_title,
                "profession_key": profession.key,
                "permissions": membership.permissions,
            }),
            idempotency_key: None,
            created_at: membership.updated_at,
            completed_at: Some(membership.updated_at),
        };
        self.repo
            .complete_company_agent_membership_update(CompanyAgentMembershipUpdateBundle {
                membership: membership.clone(),
                action,
            })?;
        Ok(membership)
    }

    pub fn hire_company_agent(
        &self,
        input: AgentStaffingHireInput,
    ) -> AppResult<AgentStaffingHireResult> {
        let actor = self.ensure_agent_can_act(input.actor_agent_id)?;
        let actor_membership = self.ensure_company_agent_permission(
            input.company_id,
            actor.id,
            COMPANY_PERMISSION_STAFF_HIRE,
        )?;
        let company = self
            .repo
            .get_company_result(input.company_id)?
            .filter(|company| company.status == "active")
            .ok_or_else(|| AppError::NotFound("active company not found".into()))?;
        let governance = self.effective_company_governance_policy_settings(company.id);
        if !governance.delegated_agent_hiring_enabled {
            return Err(AppError::Unauthorized(
                "company governance disables delegated agent hiring".into(),
            ));
        }
        self.enforce_delegated_staffing_daily_limit(
            company.id,
            STAFFING_ACTION_HIRE,
            governance.daily_delegated_hire_limit,
        )?;
        let existing_memberships = self.repo.list_company_agent_memberships(company.id);
        let current_staff_count = existing_memberships
            .iter()
            .filter(|membership| membership.employment_status != "terminated")
            .count();
        if current_staff_count >= governance.agent_staff_limit as usize {
            return Err(AppError::RateLimited(format!(
                "company agent limit of {} has been reached",
                governance.agent_staff_limit
            )));
        }

        let display_name = input.display_name.trim();
        if display_name.is_empty() || display_name.chars().count() > 80 {
            return Err(AppError::Validation(
                "agent display_name must contain 1 to 80 characters".into(),
            ));
        }
        let handle = normalize_handle(&input.handle);
        if handle.is_empty() || handle.chars().count() > 64 {
            return Err(AppError::Validation(
                "agent handle must contain 1 to 64 characters".into(),
            ));
        }
        if self
            .repo
            .list_agent_profiles()
            .into_iter()
            .any(|profile| profile.handle == handle)
        {
            return Err(AppError::Conflict("agent handle is already in use".into()));
        }

        let org_units = self.repo.list_company_org_units(company.id);
        let org_unit = match input.org_unit_id {
            Some(org_unit_id) => org_units
                .iter()
                .find(|org_unit| org_unit.id == org_unit_id && org_unit.status == "active")
                .cloned()
                .ok_or_else(|| {
                    AppError::Unauthorized("org unit does not belong to the company".into())
                })?,
            None => org_units
                .iter()
                .find(|org_unit| org_unit.parent_org_unit_id.is_none())
                .cloned()
                .ok_or_else(|| AppError::NotFound("company root org unit not found".into()))?,
        };
        self.ensure_staffing_org_scope(company.id, &actor_membership, org_unit.id)?;
        let reports_to_membership_id = input.reports_to_membership_id.or(Some(actor_membership.id));
        if let Some(manager_membership_id) = reports_to_membership_id {
            let manager = existing_memberships
                .iter()
                .find(|membership| membership.id == manager_membership_id)
                .ok_or_else(|| {
                    AppError::Unauthorized(
                        "reporting manager does not belong to the company".into(),
                    )
                })?;
            if manager.employment_status != "active" {
                return Err(AppError::Conflict("reporting manager is not active".into()));
            }
        }

        let now = now_utc();
        let profession = infer_company_profession(input.job_title.as_deref());
        let agent_profile = AgentProfile {
            id: Uuid::new_v4(),
            owner_user_id: company.owner_user_id,
            display_name: display_name.to_string(),
            handle,
            persona: input.persona.trim().to_string(),
            collaboration_preference: AGENT_COLLABORATION_PREFERENCE_AVAILABLE.into(),
            status: AgentStatus::Active,
            created_at: now,
        };
        let owner_binding = AgentOwnerBinding {
            id: Uuid::new_v4(),
            human_user_id: company.owner_user_id,
            agent_profile_id: agent_profile.id,
            created_at: now,
        };
        let membership = CompanyAgentMembership {
            id: Uuid::new_v4(),
            company_id: company.id,
            agent_profile_id: agent_profile.id,
            org_unit_id: org_unit.id,
            job_title: profession.label.clone(),
            role_key: COMPANY_AGENT_ROLE_MEMBER.into(),
            reports_to_membership_id,
            permissions: default_company_agent_permissions_for_profession(
                COMPANY_AGENT_ROLE_MEMBER,
                &profession.key,
            ),
            responsibilities: if agent_profile.persona.is_empty() {
                Vec::new()
            } else {
                vec![agent_profile.persona.clone()]
            },
            skills: Vec::new(),
            current_focus: String::new(),
            staffing_scope_org_unit_id: None,
            employment_status: "active".into(),
            joined_at: now,
            terminated_at: None,
            created_by_human_user_id: None,
            created_by_agent_id: Some(actor.id),
            updated_at: now,
        };
        let plaintext_key = generate_agent_key();
        let key_prefix = plaintext_key.chars().take(12).collect::<String>();
        let key_record = AgentKeyRecord {
            id: Uuid::new_v4(),
            agent_profile_id: agent_profile.id,
            key_name: "primary".into(),
            key_prefix: key_prefix.clone(),
            key_hash: hash_secret(&plaintext_key),
            last_used_at: None,
            expires_at: Some(now + Duration::days(180)),
            revoked_at: None,
            created_at: now,
        };
        let key_issue_log = AgentKeyIssueLog {
            id: Uuid::new_v4(),
            agent_profile_id: agent_profile.id,
            agent_key_id: Some(key_record.id),
            issue_type: AgentKeyIssueType::Issued,
            issued_by_user_id: Some(company.owner_user_id),
            metadata: json!({
                "agent_key_prefix": key_prefix,
                "creation_mode": "delegated_staffing",
                "company_id": company.id,
                "hired_by_agent_id": actor.id,
            }),
            created_at: now,
        };
        let self_notes_conversation = ConversationPreview {
            id: Uuid::new_v4(),
            title: "Self Notes".into(),
            conversation_type: ConversationType::Direct,
            last_message_preview: None,
            updated_at: now,
        };
        let action = AgentStaffingAction {
            id: Uuid::new_v4(),
            company_id: company.id,
            action_type: STAFFING_ACTION_HIRE.into(),
            actor_type: STAFFING_ACTOR_AGENT.into(),
            actor_human_user_id: None,
            actor_agent_id: Some(actor.id),
            target_agent_id: Some(agent_profile.id),
            requested_org_unit_id: Some(org_unit.id),
            requested_role_key: Some(COMPANY_AGENT_ROLE_MEMBER.into()),
            reason: normalize_staffing_text(input.reason, 500, "reason")?,
            handoff_plan: String::new(),
            status: STAFFING_STATUS_COMPLETED.into(),
            approval_required: false,
            approved_by_human_user_id: None,
            request_payload: json!({
                "display_name": agent_profile.display_name,
                "handle": agent_profile.handle,
                "job_title": membership.job_title,
                "reports_to_membership_id": membership.reports_to_membership_id,
            }),
            result_payload: json!({
                "agent_profile_id": agent_profile.id,
                "membership_id": membership.id,
                "employment_status": membership.employment_status,
                "agent_key_issued": true,
                "agent_key_prefix": key_prefix,
                "requires_human_activation": false,
            }),
            idempotency_key: normalize_optional_idempotency_key(input.idempotency_key)?,
            created_at: now,
            completed_at: Some(now),
        };
        self.repo
            .complete_agent_staffing_hire(AgentStaffingHireBundle {
                agent_profile: agent_profile.clone(),
                owner_binding,
                membership: membership.clone(),
                key_record,
                key_issue_log,
                self_notes_conversation,
                action: action.clone(),
            })?;
        self.assign_default_codex_runner_profile_to_agent(
            company.id,
            agent_profile.id,
            company.owner_user_id,
            now,
        )?;
        Ok(AgentStaffingHireResult {
            action,
            agent_profile,
            membership,
        })
    }
}
