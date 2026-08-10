use super::mapping::*;
use super::*;

impl CompanyPlatformRepository for PostgresPlatformRepository {
    fn insert_company_bundle(&self, bundle: CompanyCreationBundle) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                INSERT INTO companies (
                    id, owner_user_id, name, slug, description, status, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
                &[
                    &bundle.company.id,
                    &bundle.company.owner_user_id,
                    &bundle.company.name,
                    &bundle.company.slug,
                    &bundle.company.description,
                    &bundle.company.status,
                    &bundle.company.created_at,
                    &bundle.company.updated_at,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO company_human_members (
                    id, company_id, human_user_id, role, status, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
                &[
                    &bundle.owner_membership.id,
                    &bundle.owner_membership.company_id,
                    &bundle.owner_membership.human_user_id,
                    &bundle.owner_membership.role,
                    &bundle.owner_membership.status,
                    &bundle.owner_membership.created_at,
                    &bundle.owner_membership.updated_at,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO org_units (
                    id, company_id, parent_org_unit_id, name, unit_type,
                    sort_order, status, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
                &[
                    &bundle.root_org_unit.id,
                    &bundle.root_org_unit.company_id,
                    &bundle.root_org_unit.parent_org_unit_id,
                    &bundle.root_org_unit.name,
                    &bundle.root_org_unit.unit_type,
                    &bundle.root_org_unit.sort_order,
                    &bundle.root_org_unit.status,
                    &bundle.root_org_unit.created_at,
                    &bundle.root_org_unit.updated_at,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO conversations (
                    id, conversation_type, title, created_by_agent_id, status,
                    company_id, project_id, context_type, visibility,
                    last_message_at, created_at, updated_at
                )
                VALUES ($1, 'group', $2, NULL, 'active', $3, NULL,
                        'company_all', 'members', NULL, $4, $4)
                "#,
                &[
                    &bundle.default_group.preview.id,
                    &bundle.default_group.preview.title,
                    &bundle.company.id,
                    &bundle.default_group.preview.updated_at,
                ],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn get_company(&self, company_id: Uuid) -> Option<Company> {
        self.get_company_result(company_id).ok().flatten()
    }

    fn get_company_result(&self, company_id: Uuid) -> AppResult<Option<Company>> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, owner_user_id, name, slug, description, status, created_at, updated_at
                FROM companies
                WHERE id = $1
                "#,
                &[&company_id],
            )
        })
        .map(|row| row.map(map_company))
    }

    fn get_company_by_slug(&self, slug: &str) -> Option<Company> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, owner_user_id, name, slug, description, status, created_at, updated_at
                FROM companies
                WHERE lower(slug) = lower($1)
                "#,
                &[&slug],
            )
        })
        .ok()
        .flatten()
        .map(map_company)
    }

    fn list_human_companies(&self, human_user_id: Uuid) -> Vec<Company> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT c.id, c.owner_user_id, c.name, c.slug, c.description,
                       c.status, c.created_at, c.updated_at
                FROM company_human_members chm
                INNER JOIN companies c ON c.id = chm.company_id
                WHERE chm.human_user_id = $1
                  AND chm.status = 'active'
                ORDER BY c.created_at DESC
                "#,
                &[&human_user_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_company)
        .collect()
    }

    fn get_company_human_member(
        &self,
        company_id: Uuid,
        human_user_id: Uuid,
    ) -> Option<CompanyHumanMember> {
        self.get_company_human_member_result(company_id, human_user_id)
            .ok()
            .flatten()
    }

    fn get_company_human_member_result(
        &self,
        company_id: Uuid,
        human_user_id: Uuid,
    ) -> AppResult<Option<CompanyHumanMember>> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, human_user_id, role, status, created_at, updated_at
                FROM company_human_members
                WHERE company_id = $1 AND human_user_id = $2
                "#,
                &[&company_id, &human_user_id],
            )
        })
        .map(|row| row.map(map_company_human_member))
    }

    fn get_org_unit(&self, org_unit_id: Uuid) -> Option<OrgUnit> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, parent_org_unit_id, name, unit_type,
                       sort_order, status, created_at, updated_at
                FROM org_units
                WHERE id = $1
                "#,
                &[&org_unit_id],
            )
        })
        .ok()
        .flatten()
        .map(map_org_unit)
    }

    fn insert_org_unit(&self, org_unit: OrgUnit) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO org_units (
                    id, company_id, parent_org_unit_id, name, unit_type,
                    sort_order, status, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
                &[
                    &org_unit.id,
                    &org_unit.company_id,
                    &org_unit.parent_org_unit_id,
                    &org_unit.name,
                    &org_unit.unit_type,
                    &org_unit.sort_order,
                    &org_unit.status,
                    &org_unit.created_at,
                    &org_unit.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_company_org_units(&self, company_id: Uuid) -> Vec<OrgUnit> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, company_id, parent_org_unit_id, name, unit_type,
                       sort_order, status, created_at, updated_at
                FROM org_units
                WHERE company_id = $1
                ORDER BY sort_order, created_at
                "#,
                &[&company_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_org_unit)
        .collect()
    }

    fn get_company_agent_membership(&self, agent_id: Uuid) -> Option<CompanyAgentMembership> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, agent_profile_id, org_unit_id, job_title,
                       role_key, reports_to_membership_id, permissions, responsibilities,
                       skills, current_focus, employment_status, staffing_scope_org_unit_id,
                       joined_at, terminated_at, created_by_human_user_id,
                       created_by_agent_id, updated_at
                FROM company_agent_memberships
                WHERE agent_profile_id = $1
                "#,
                &[&agent_id],
            )
        })
        .ok()
        .flatten()
        .map(map_company_agent_membership)
    }

    fn list_company_agent_memberships(&self, company_id: Uuid) -> Vec<CompanyAgentMembership> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, company_id, agent_profile_id, org_unit_id, job_title,
                       role_key, reports_to_membership_id, permissions, responsibilities,
                       skills, current_focus, employment_status, staffing_scope_org_unit_id,
                       joined_at, terminated_at, created_by_human_user_id,
                       created_by_agent_id, updated_at
                FROM company_agent_memberships
                WHERE company_id = $1
                ORDER BY joined_at DESC
                "#,
                &[&company_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_company_agent_membership)
        .collect()
    }

    fn list_company_agent_membership_page(
        &self,
        company_id: Uuid,
        after_membership_id: Option<Uuid>,
        limit: usize,
    ) -> AppResult<CursorPage<CompanyAgentMembership>> {
        let cursor = match after_membership_id {
            Some(cursor_id) => self
                .with_client(|client| {
                    client.query_opt(
                        "SELECT joined_at, id FROM company_agent_memberships WHERE company_id = $1 AND id = $2",
                        &[&company_id, &cursor_id],
                    )
                })?
                .map(|row| {
                    (
                        row.get::<_, chrono::DateTime<chrono::Utc>>("joined_at"),
                        row.get::<_, Uuid>("id"),
                    )
                })
                .ok_or_else(|| {
                    AppError::Validation(
                        "Agent cursor does not belong to the selected company".into(),
                    )
                })?,
            None => (chrono::DateTime::<chrono::Utc>::MAX_UTC, Uuid::max()),
        };
        let limit = limit.clamp(1, 100);
        let query_limit = i64::try_from(limit + 1).unwrap_or(101);
        let rows = self.with_client(|client| {
            client.query(
                r#"
                SELECT id, company_id, agent_profile_id, org_unit_id, job_title,
                       role_key, reports_to_membership_id, permissions, responsibilities,
                       skills, current_focus, employment_status, staffing_scope_org_unit_id,
                       joined_at, terminated_at, created_by_human_user_id,
                       created_by_agent_id, updated_at
                FROM company_agent_memberships
                WHERE company_id = $1 AND (joined_at, id) < ($2, $3)
                ORDER BY joined_at DESC, id DESC
                LIMIT $4
                "#,
                &[&company_id, &cursor.0, &cursor.1, &query_limit],
            )
        })?;
        let mut items = rows
            .into_iter()
            .map(map_company_agent_membership)
            .collect::<Vec<_>>();
        let has_more = items.len() > limit;
        if has_more {
            items.pop();
        }
        Ok(CursorPage {
            next_cursor: has_more.then(|| items.last().map(|item| item.id)).flatten(),
            items,
            has_more,
        })
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
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                UPDATE company_agent_memberships
                SET responsibilities = $2,
                    skills = $3,
                    current_focus = $4,
                    updated_at = $5
                WHERE agent_profile_id = $1
                "#,
                &[
                    &agent_id,
                    &Json(responsibilities),
                    &Json(skills),
                    &current_focus,
                    &updated_at,
                ],
            )?;
            tx.execute(
                r#"
                UPDATE agent_profiles
                SET collaboration_preference = $2,
                    updated_at = $3
                WHERE id = $1
                "#,
                &[&agent_id, &collaboration_preference, &updated_at],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn complete_company_agent_creation_bundle(
        &self,
        bundle: CompanyAgentCreationBundle,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                INSERT INTO agent_profiles (
                    id, owner_user_id, handle, display_name, persona,
                    collaboration_preference, status, visibility, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, 'private', $8, $8)
                "#,
                &[
                    &bundle.agent_profile.id,
                    &bundle.agent_profile.owner_user_id,
                    &bundle.agent_profile.handle,
                    &bundle.agent_profile.display_name,
                    &bundle.agent_profile.persona,
                    &bundle.agent_profile.collaboration_preference,
                    &agent_status_to_str(&bundle.agent_profile.status),
                    &bundle.agent_profile.created_at,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO agent_owner_bindings (
                    id, human_user_id, agent_profile_id, binding_role, created_at
                )
                VALUES ($1, $2, $3, 'owner', $4)
                "#,
                &[
                    &bundle.owner_binding.id,
                    &bundle.owner_binding.human_user_id,
                    &bundle.owner_binding.agent_profile_id,
                    &bundle.owner_binding.created_at,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO company_agent_memberships (
                    id, company_id, agent_profile_id, org_unit_id, job_title, role_key,
                    reports_to_membership_id, permissions, responsibilities, skills,
                    current_focus, staffing_scope_org_unit_id, employment_status, joined_at,
                    terminated_at, created_by_human_user_id, created_by_agent_id, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
                "#,
                &[
                    &bundle.membership.id,
                    &bundle.membership.company_id,
                    &bundle.membership.agent_profile_id,
                    &bundle.membership.org_unit_id,
                    &bundle.membership.job_title,
                    &bundle.membership.role_key,
                    &bundle.membership.reports_to_membership_id,
                    &Json(bundle.membership.permissions.clone()),
                    &Json(bundle.membership.responsibilities.clone()),
                    &Json(bundle.membership.skills.clone()),
                    &bundle.membership.current_focus,
                    &bundle.membership.staffing_scope_org_unit_id,
                    &bundle.membership.employment_status,
                    &bundle.membership.joined_at,
                    &bundle.membership.terminated_at,
                    &bundle.membership.created_by_human_user_id,
                    &bundle.membership.created_by_agent_id,
                    &bundle.membership.updated_at,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO agent_keys (
                    id, agent_profile_id, key_name, key_prefix, key_hash,
                    scopes, last_used_at, last_used_ip, expires_at, revoked_at, created_at
                )
                VALUES ($1, $2, $3, $4, $5, '[]'::jsonb, $6, NULL, $7, $8, $9)
                "#,
                &[
                    &bundle.key_record.id,
                    &bundle.key_record.agent_profile_id,
                    &bundle.key_record.key_name,
                    &bundle.key_record.key_prefix,
                    &bundle.key_record.key_hash,
                    &bundle.key_record.last_used_at,
                    &bundle.key_record.expires_at,
                    &bundle.key_record.revoked_at,
                    &bundle.key_record.created_at,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO agent_key_issue_logs (
                    id, agent_profile_id, agent_key_id, issue_type,
                    issued_by_user_id, metadata, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
                &[
                    &bundle.key_issue_log.id,
                    &bundle.key_issue_log.agent_profile_id,
                    &bundle.key_issue_log.agent_key_id,
                    &agent_key_issue_type_to_str(&bundle.key_issue_log.issue_type),
                    &bundle.key_issue_log.issued_by_user_id,
                    &Json(bundle.key_issue_log.metadata.clone()),
                    &bundle.key_issue_log.created_at,
                ],
            )?;
            let title = Some(bundle.self_notes_conversation.title.as_str());
            tx.execute(
                r#"
                INSERT INTO conversations (
                    id, conversation_type, title, created_by_agent_id, status,
                    company_id, context_type, visibility,
                    last_message_at, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, 'active', $5, 'self_notes', 'private', $6, $6, $6)
                "#,
                &[
                    &bundle.self_notes_conversation.id,
                    &conversation_type_to_str(&bundle.self_notes_conversation.conversation_type),
                    &title,
                    &bundle.agent_profile.id,
                    &bundle.membership.company_id,
                    &bundle.self_notes_conversation.updated_at,
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
                    &bundle.self_notes_conversation.id,
                    &bundle.agent_profile.id,
                    &bundle.self_notes_conversation.updated_at,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO conversation_members (
                    id, conversation_id, agent_profile_id, member_role, joined_at
                )
                SELECT $1, conversation.id, $2, $3, $4
                FROM conversations conversation
                WHERE conversation.company_id = $5
                  AND conversation.context_type = 'company_all'
                ON CONFLICT (conversation_id, agent_profile_id) DO NOTHING
                "#,
                &[
                    &Uuid::new_v4(),
                    &bundle.agent_profile.id,
                    &if bundle.membership.role_key == COMPANY_AGENT_ROLE_MANAGER {
                        "admin"
                    } else {
                        "member"
                    },
                    &bundle.membership.joined_at,
                    &bundle.membership.company_id,
                ],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn complete_company_agent_membership_update(
        &self,
        bundle: CompanyAgentMembershipUpdateBundle,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                UPDATE company_agent_memberships
                SET role_key = $2, permissions = $3, staffing_scope_org_unit_id = $4,
                    job_title = $5, updated_at = $6
                WHERE agent_profile_id = $1
                "#,
                &[
                    &bundle.membership.agent_profile_id,
                    &bundle.membership.role_key,
                    &Json(bundle.membership.permissions.clone()),
                    &bundle.membership.staffing_scope_org_unit_id,
                    &bundle.membership.job_title,
                    &bundle.membership.updated_at,
                ],
            )?;
            insert_agent_staffing_action(&mut tx, &bundle.action)?;
            tx.commit()?;
            Ok(())
        })
    }

    fn complete_agent_staffing_hire(&self, bundle: AgentStaffingHireBundle) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                INSERT INTO agent_profiles (
                    id, owner_user_id, handle, display_name, persona,
                    collaboration_preference, status, visibility, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, 'private', $8, $8)
                "#,
                &[
                    &bundle.agent_profile.id,
                    &bundle.agent_profile.owner_user_id,
                    &bundle.agent_profile.handle,
                    &bundle.agent_profile.display_name,
                    &bundle.agent_profile.persona,
                    &bundle.agent_profile.collaboration_preference,
                    &agent_status_to_str(&bundle.agent_profile.status),
                    &bundle.agent_profile.created_at,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO agent_owner_bindings (
                    id, human_user_id, agent_profile_id, binding_role, created_at
                )
                VALUES ($1, $2, $3, 'owner', $4)
                "#,
                &[
                    &bundle.owner_binding.id,
                    &bundle.owner_binding.human_user_id,
                    &bundle.owner_binding.agent_profile_id,
                    &bundle.owner_binding.created_at,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO company_agent_memberships (
                    id, company_id, agent_profile_id, org_unit_id, job_title, role_key,
                    reports_to_membership_id, permissions, responsibilities, skills,
                    current_focus, staffing_scope_org_unit_id, employment_status, joined_at,
                    terminated_at, created_by_human_user_id, created_by_agent_id, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
                "#,
                &[
                    &bundle.membership.id,
                    &bundle.membership.company_id,
                    &bundle.membership.agent_profile_id,
                    &bundle.membership.org_unit_id,
                    &bundle.membership.job_title,
                    &bundle.membership.role_key,
                    &bundle.membership.reports_to_membership_id,
                    &Json(bundle.membership.permissions.clone()),
                    &Json(bundle.membership.responsibilities.clone()),
                    &Json(bundle.membership.skills.clone()),
                    &bundle.membership.current_focus,
                    &bundle.membership.staffing_scope_org_unit_id,
                    &bundle.membership.employment_status,
                    &bundle.membership.joined_at,
                    &bundle.membership.terminated_at,
                    &bundle.membership.created_by_human_user_id,
                    &bundle.membership.created_by_agent_id,
                    &bundle.membership.updated_at,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO agent_keys (
                    id, agent_profile_id, key_name, key_prefix, key_hash,
                    scopes, last_used_at, last_used_ip, expires_at, revoked_at, created_at
                )
                VALUES ($1, $2, $3, $4, $5, '[]'::jsonb, $6, NULL, $7, $8, $9)
                "#,
                &[
                    &bundle.key_record.id,
                    &bundle.key_record.agent_profile_id,
                    &bundle.key_record.key_name,
                    &bundle.key_record.key_prefix,
                    &bundle.key_record.key_hash,
                    &bundle.key_record.last_used_at,
                    &bundle.key_record.expires_at,
                    &bundle.key_record.revoked_at,
                    &bundle.key_record.created_at,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO agent_key_issue_logs (
                    id, agent_profile_id, agent_key_id, issue_type,
                    issued_by_user_id, metadata, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
                &[
                    &bundle.key_issue_log.id,
                    &bundle.key_issue_log.agent_profile_id,
                    &bundle.key_issue_log.agent_key_id,
                    &agent_key_issue_type_to_str(&bundle.key_issue_log.issue_type),
                    &bundle.key_issue_log.issued_by_user_id,
                    &Json(bundle.key_issue_log.metadata.clone()),
                    &bundle.key_issue_log.created_at,
                ],
            )?;
            let title = Some(bundle.self_notes_conversation.title.as_str());
            tx.execute(
                r#"
                INSERT INTO conversations (
                    id, conversation_type, title, created_by_agent_id, status,
                    company_id, context_type, visibility,
                    last_message_at, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, 'active', $5, 'self_notes', 'private', $6, $6, $6)
                "#,
                &[
                    &bundle.self_notes_conversation.id,
                    &conversation_type_to_str(&bundle.self_notes_conversation.conversation_type),
                    &title,
                    &bundle.agent_profile.id,
                    &bundle.membership.company_id,
                    &bundle.self_notes_conversation.updated_at,
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
                    &bundle.self_notes_conversation.id,
                    &bundle.agent_profile.id,
                    &bundle.self_notes_conversation.updated_at,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO conversation_members (
                    id, conversation_id, agent_profile_id, member_role, joined_at
                )
                SELECT $1, conversation.id, $2, 'member', $3
                FROM conversations conversation
                WHERE conversation.company_id = $4
                  AND conversation.context_type = 'company_all'
                ON CONFLICT (conversation_id, agent_profile_id) DO NOTHING
                "#,
                &[
                    &Uuid::new_v4(),
                    &bundle.agent_profile.id,
                    &bundle.membership.joined_at,
                    &bundle.membership.company_id,
                ],
            )?;
            insert_agent_staffing_action(&mut tx, &bundle.action)?;
            tx.commit()?;
            Ok(())
        })
    }

    fn complete_company_agent_activation(
        &self,
        bundle: CompanyAgentActivationBundle,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                UPDATE agent_profiles
                SET status = $2, updated_at = $3
                WHERE id = $1
                "#,
                &[
                    &bundle.agent_profile.id,
                    &agent_status_to_str(&bundle.agent_profile.status),
                    &bundle.action.created_at,
                ],
            )?;
            tx.execute(
                r#"
                UPDATE company_agent_memberships
                SET employment_status = $2, terminated_at = $3, updated_at = $4
                WHERE agent_profile_id = $1
                "#,
                &[
                    &bundle.membership.agent_profile_id,
                    &bundle.membership.employment_status,
                    &bundle.membership.terminated_at,
                    &bundle.membership.updated_at,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO agent_keys (
                    id, agent_profile_id, key_name, key_prefix, key_hash,
                    scopes, last_used_at, last_used_ip, expires_at, revoked_at, created_at
                )
                VALUES ($1, $2, $3, $4, $5, '[]'::jsonb, $6, NULL, $7, $8, $9)
                "#,
                &[
                    &bundle.key_record.id,
                    &bundle.key_record.agent_profile_id,
                    &bundle.key_record.key_name,
                    &bundle.key_record.key_prefix,
                    &bundle.key_record.key_hash,
                    &bundle.key_record.last_used_at,
                    &bundle.key_record.expires_at,
                    &bundle.key_record.revoked_at,
                    &bundle.key_record.created_at,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO agent_key_issue_logs (
                    id, agent_profile_id, agent_key_id, issue_type,
                    issued_by_user_id, metadata, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
                &[
                    &bundle.key_issue_log.id,
                    &bundle.key_issue_log.agent_profile_id,
                    &bundle.key_issue_log.agent_key_id,
                    &agent_key_issue_type_to_str(&bundle.key_issue_log.issue_type),
                    &bundle.key_issue_log.issued_by_user_id,
                    &Json(bundle.key_issue_log.metadata.clone()),
                    &bundle.key_issue_log.created_at,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO conversation_members (
                    id, conversation_id, agent_profile_id, member_role, joined_at
                )
                SELECT $1, conversation.id, $2, $3, $4
                FROM conversations conversation
                WHERE conversation.company_id = $5
                  AND conversation.context_type = 'company_all'
                ON CONFLICT (conversation_id, agent_profile_id) DO UPDATE
                SET left_at = NULL
                "#,
                &[
                    &Uuid::new_v4(),
                    &bundle.agent_profile.id,
                    &if bundle.membership.role_key == COMPANY_AGENT_ROLE_MANAGER {
                        "admin"
                    } else {
                        "member"
                    },
                    &bundle.membership.joined_at,
                    &bundle.membership.company_id,
                ],
            )?;
            insert_agent_staffing_action(&mut tx, &bundle.action)?;
            tx.commit()?;
            Ok(())
        })
    }

    fn complete_agent_staffing_status_change(
        &self,
        bundle: AgentStaffingStatusChangeBundle,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                UPDATE agent_profiles
                SET status = $2, updated_at = $3
                WHERE id = $1
                "#,
                &[
                    &bundle.agent_profile.id,
                    &agent_status_to_str(&bundle.agent_profile.status),
                    &bundle.action.created_at,
                ],
            )?;
            tx.execute(
                r#"
                UPDATE company_agent_memberships
                SET employment_status = $2, terminated_at = $3, updated_at = $4
                WHERE agent_profile_id = $1
                "#,
                &[
                    &bundle.membership.agent_profile_id,
                    &bundle.membership.employment_status,
                    &bundle.membership.terminated_at,
                    &bundle.membership.updated_at,
                ],
            )?;
            tx.execute(
                r#"
                UPDATE agent_keys
                SET revoked_at = COALESCE(revoked_at, $2)
                WHERE agent_profile_id = $1
                "#,
                &[&bundle.agent_profile.id, &bundle.action.created_at],
            )?;
            for log in &bundle.key_issue_logs {
                tx.execute(
                    r#"
                    INSERT INTO agent_key_issue_logs (
                        id, agent_profile_id, agent_key_id, issue_type,
                        issued_by_user_id, metadata, created_at
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7)
                    "#,
                    &[
                        &log.id,
                        &log.agent_profile_id,
                        &log.agent_key_id,
                        &agent_key_issue_type_to_str(&log.issue_type),
                        &log.issued_by_user_id,
                        &Json(log.metadata.clone()),
                        &log.created_at,
                    ],
                )?;
            }
            for task in &bundle.reassigned_tasks {
                tx.execute(
                    r#"
                    UPDATE company_project_tasks
                    SET assignee_agent_id = $2,
                        updated_by_agent_id = $3,
                        updated_by_human_user_id = $4,
                        updated_at = $5
                    WHERE id = $1
                    "#,
                    &[
                        &task.id,
                        &task.assignee_agent_id,
                        &task.updated_by_agent_id,
                        &task.updated_by_human_user_id,
                        &task.updated_at,
                    ],
                )?;
                tx.execute(
                    "UPDATE company_projects SET updated_at = $2, updated_by_agent_id = $3 WHERE id = $1",
                    &[&task.project_id, &task.updated_at, &task.updated_by_agent_id],
                )?;
            }
            insert_agent_staffing_action(&mut tx, &bundle.action)?;
            tx.commit()?;
            Ok(())
        })
    }

    fn get_agent_staffing_action(&self, action_id: Uuid) -> Option<AgentStaffingAction> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, action_type, actor_type, actor_human_user_id,
                       actor_agent_id, target_agent_id, requested_org_unit_id,
                       requested_role_key, reason, handoff_plan, status, approval_required,
                       approved_by_human_user_id, request_payload, result_payload,
                       idempotency_key, created_at, completed_at
                FROM agent_staffing_actions
                WHERE id = $1
                "#,
                &[&action_id],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_staffing_action)
    }

    fn list_agent_staffing_actions(&self, company_id: Uuid) -> Vec<AgentStaffingAction> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, company_id, action_type, actor_type, actor_human_user_id,
                       actor_agent_id, target_agent_id, requested_org_unit_id,
                       requested_role_key, reason, handoff_plan, status, approval_required,
                       approved_by_human_user_id, request_payload, result_payload,
                       idempotency_key, created_at, completed_at
                FROM agent_staffing_actions
                WHERE company_id = $1
                ORDER BY created_at DESC
                LIMIT 200
                "#,
                &[&company_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_staffing_action)
        .collect()
    }
}
