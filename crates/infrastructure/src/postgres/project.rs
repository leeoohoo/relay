use super::mapping::*;
use super::*;

impl ProjectPlatformRepository for PostgresPlatformRepository {
    fn complete_company_project_creation(
        &self,
        bundle: CompanyProjectCreationBundle,
    ) -> AppResult<()> {
        self.with_transaction(|tx| insert_company_project_creation(tx, &bundle))
    }

    fn complete_managed_company_project_creation(
        &self,
        bundle: ManagedCompanyProjectCreationBundle,
    ) -> AppResult<()> {
        if bundle.git_config.project_id != bundle.project_creation.project.id {
            return Err(AppError::Validation(
                "managed Git configuration must belong to the created project".into(),
            ));
        }
        self.with_transaction(|tx| {
            insert_company_project_creation(tx, &bundle.project_creation)?;
            upsert_company_project_git_config(tx, &bundle.git_config)
        })
    }

    fn get_company_project(&self, project_id: Uuid) -> Option<CompanyProject> {
        self.get_company_project_result(project_id).ok().flatten()
    }

    fn get_company_project_result(&self, project_id: Uuid) -> AppResult<Option<CompanyProject>> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, name, description, status, owner_agent_id,
                       project_type, project_type_source, project_type_confidence,
                       project_type_evidence,
                       project_group_conversation_id, created_by_agent_id,
                       updated_by_agent_id, due_at, created_at, updated_at, completed_at
                FROM company_projects
                WHERE id = $1
                "#,
                &[&project_id],
            )
        })
        .map(|row| row.map(map_company_project))
    }

    fn list_company_projects(&self, company_id: Uuid) -> Vec<CompanyProject> {
        self.list_company_projects_result(company_id)
            .unwrap_or_default()
    }

    fn list_company_projects_result(&self, company_id: Uuid) -> AppResult<Vec<CompanyProject>> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, company_id, name, description, status, owner_agent_id,
                       project_type, project_type_source, project_type_confidence,
                       project_type_evidence,
                       project_group_conversation_id, created_by_agent_id,
                       updated_by_agent_id, due_at, created_at, updated_at, completed_at
                FROM company_projects
                WHERE company_id = $1
                ORDER BY updated_at DESC, created_at DESC
                "#,
                &[&company_id],
            )
        })
        .map(|rows| rows.into_iter().map(map_company_project).collect())
    }

    fn save_company_project_git_config(&self, config: CompanyProjectGitConfig) -> AppResult<()> {
        self.with_transaction(|tx| upsert_company_project_git_config(tx, &config))
    }

    fn get_company_project_git_config(&self, project_id: Uuid) -> Option<CompanyProjectGitConfig> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT project_id, remote_url, default_branch, git_host, host_local_path,
                       auth_profile, allow_agent_push, branch_prefix, created_by_agent_id,
                       created_by_human_user_id, updated_by_agent_id,
                       updated_by_human_user_id, created_at, updated_at
                FROM company_project_git_configs
                WHERE project_id = $1
                "#,
                &[&project_id],
            )
        })
        .ok()
        .flatten()
        .map(map_company_project_git_config)
    }

    fn delete_company_project_git_config(&self, project_id: Uuid) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                "DELETE FROM company_project_git_configs WHERE project_id = $1",
                &[&project_id],
            )?;
            Ok(())
        })
    }

    fn save_company_project_rule(&self, rule: CompanyProjectRule) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO company_project_rules (
                    project_id, content, updated_by_agent_id, updated_by_human_user_id,
                    created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (project_id) DO UPDATE
                SET content = EXCLUDED.content,
                    updated_by_agent_id = EXCLUDED.updated_by_agent_id,
                    updated_by_human_user_id = EXCLUDED.updated_by_human_user_id,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &rule.project_id,
                    &rule.content,
                    &rule.updated_by_agent_id,
                    &rule.updated_by_human_user_id,
                    &rule.created_at,
                    &rule.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_company_project_rule(&self, project_id: Uuid) -> Option<CompanyProjectRule> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT project_id, content, updated_by_agent_id, updated_by_human_user_id,
                       created_at, updated_at
                FROM company_project_rules
                WHERE project_id = $1
                "#,
                &[&project_id],
            )
        })
        .ok()
        .flatten()
        .map(map_company_project_rule)
    }

    fn replace_company_project_assets(
        &self,
        project_id: Uuid,
        assets: Vec<CompanyProjectAsset>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                "DELETE FROM company_project_assets WHERE project_id = $1",
                &[&project_id],
            )?;
            for asset in assets {
                let metadata = Json(&asset.metadata);
                tx.execute(
                    r#"
                    INSERT INTO company_project_assets (
                        id, project_id, name, asset_type, locator, description, status,
                        metadata, updated_by_agent_id, updated_by_human_user_id,
                        created_at, updated_at
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                    "#,
                    &[
                        &asset.id,
                        &asset.project_id,
                        &asset.name,
                        &asset.asset_type,
                        &asset.locator,
                        &asset.description,
                        &asset.status,
                        &metadata,
                        &asset.updated_by_agent_id,
                        &asset.updated_by_human_user_id,
                        &asset.created_at,
                        &asset.updated_at,
                    ],
                )?;
            }
            tx.execute(
                r#"
                UPDATE company_project_asset_refresh_configs
                SET last_completed_at = CURRENT_TIMESTAMP,
                    next_refresh_at = CURRENT_TIMESTAMP + make_interval(mins => interval_minutes),
                    updated_at = CURRENT_TIMESTAMP
                WHERE project_id = $1
                "#,
                &[&project_id],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn list_company_project_assets(&self, project_id: Uuid) -> Vec<CompanyProjectAsset> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, project_id, name, asset_type, locator, description, status,
                       metadata, updated_by_agent_id, updated_by_human_user_id,
                       created_at, updated_at
                FROM company_project_assets
                WHERE project_id = $1
                ORDER BY asset_type, name, locator
                "#,
                &[&project_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_company_project_asset)
        .collect()
    }

    fn save_company_project_asset_refresh_config(
        &self,
        config: CompanyProjectAssetRefreshConfig,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO company_project_asset_refresh_configs (
                    project_id, maintainer_agent_id, interval_minutes, enabled,
                    next_refresh_at, last_requested_at, last_completed_at,
                    created_by_human_user_id, updated_by_human_user_id,
                    created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                ON CONFLICT (project_id) DO UPDATE
                SET maintainer_agent_id = EXCLUDED.maintainer_agent_id,
                    interval_minutes = EXCLUDED.interval_minutes,
                    enabled = EXCLUDED.enabled,
                    next_refresh_at = EXCLUDED.next_refresh_at,
                    last_requested_at = EXCLUDED.last_requested_at,
                    last_completed_at = EXCLUDED.last_completed_at,
                    updated_by_human_user_id = EXCLUDED.updated_by_human_user_id,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &config.project_id,
                    &config.maintainer_agent_id,
                    &config.interval_minutes,
                    &config.enabled,
                    &config.next_refresh_at,
                    &config.last_requested_at,
                    &config.last_completed_at,
                    &config.created_by_human_user_id,
                    &config.updated_by_human_user_id,
                    &config.created_at,
                    &config.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_company_project_asset_refresh_config(
        &self,
        project_id: Uuid,
    ) -> Option<CompanyProjectAssetRefreshConfig> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT project_id, maintainer_agent_id, interval_minutes, enabled,
                       next_refresh_at, last_requested_at, last_completed_at,
                       created_by_human_user_id, updated_by_human_user_id,
                       created_at, updated_at
                FROM company_project_asset_refresh_configs
                WHERE project_id = $1
                "#,
                &[&project_id],
            )
        })
        .ok()
        .flatten()
        .map(map_company_project_asset_refresh_config)
    }

    fn claim_due_company_project_asset_refresh(
        &self,
        agent_id: Uuid,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<Option<CompanyProjectAssetRefreshConfig>> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                WITH due AS (
                    SELECT config.project_id
                    FROM company_project_asset_refresh_configs config
                    INNER JOIN company_projects project ON project.id = config.project_id
                    WHERE config.enabled = TRUE
                      AND config.maintainer_agent_id = $1
                      AND config.next_refresh_at <= $2
                      AND project.status <> 'paused'
                    ORDER BY config.next_refresh_at, config.project_id
                    FOR UPDATE SKIP LOCKED
                    LIMIT 1
                )
                UPDATE company_project_asset_refresh_configs config
                SET last_requested_at = $2,
                    next_refresh_at = $2 + make_interval(mins => config.interval_minutes),
                    updated_at = $2
                FROM due
                WHERE config.project_id = due.project_id
                RETURNING config.project_id, config.maintainer_agent_id,
                          config.interval_minutes, config.enabled, config.next_refresh_at,
                          config.last_requested_at, config.last_completed_at,
                          config.created_by_human_user_id, config.updated_by_human_user_id,
                          config.created_at, config.updated_at
                "#,
                &[&agent_id, &now],
            )
        })
        .map(|row| row.map(map_company_project_asset_refresh_config))
    }

    fn mark_company_project_asset_refresh_completed(
        &self,
        project_id: Uuid,
        completed_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE company_project_asset_refresh_configs
                SET last_completed_at = $2::timestamptz,
                    next_refresh_at = $2::timestamptz + make_interval(mins => interval_minutes),
                    updated_at = $2::timestamptz
                WHERE project_id = $1
                "#,
                &[&project_id, &completed_at],
            )?;
            Ok(())
        })
    }

    fn update_company_project(&self, project: CompanyProject) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE company_projects
                SET name = $2,
                    description = $3,
                    status = $4,
                    owner_agent_id = $5,
                    updated_by_agent_id = $6,
                    due_at = $7,
                    updated_at = $8,
                    completed_at = $9
                WHERE id = $1
                "#,
                &[
                    &project.id,
                    &project.name,
                    &project.description,
                    &project.status,
                    &project.owner_agent_id,
                    &project.updated_by_agent_id,
                    &project.due_at,
                    &project.updated_at,
                    &project.completed_at,
                ],
            )?;
            Ok(())
        })
    }

    fn update_company_project_metadata(
        &self,
        project: CompanyProject,
        project_group_title: String,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                UPDATE company_projects
                SET name = $2,
                    description = $3,
                    status = $4,
                    owner_agent_id = $5,
                    updated_by_agent_id = $6,
                    due_at = $7,
                    updated_at = $8,
                    completed_at = $9
                WHERE id = $1
                "#,
                &[
                    &project.id,
                    &project.name,
                    &project.description,
                    &project.status,
                    &project.owner_agent_id,
                    &project.updated_by_agent_id,
                    &project.due_at,
                    &project.updated_at,
                    &project.completed_at,
                ],
            )?;
            tx.execute(
                "UPDATE conversations SET title = $2, updated_at = $3 WHERE id = $1",
                &[
                    &project.project_group_conversation_id,
                    &project_group_title,
                    &project.updated_at,
                ],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn get_company_project_member(
        &self,
        project_id: Uuid,
        agent_id: Uuid,
    ) -> Option<CompanyProjectMember> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, project_id, agent_profile_id, role, joined_at, left_at,
                       added_by_agent_id
                FROM company_project_members
                WHERE project_id = $1 AND agent_profile_id = $2
                "#,
                &[&project_id, &agent_id],
            )
        })
        .ok()
        .flatten()
        .map(map_company_project_member)
    }

    fn list_company_project_members(&self, project_id: Uuid) -> Vec<CompanyProjectMember> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, project_id, agent_profile_id, role, joined_at, left_at,
                       added_by_agent_id
                FROM company_project_members
                WHERE project_id = $1
                ORDER BY joined_at, agent_profile_id
                "#,
                &[&project_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_company_project_member)
        .collect()
    }

    fn complete_company_project_member_add(
        &self,
        bundle: CompanyProjectMemberAddBundle,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                INSERT INTO company_project_members (
                    id, project_id, agent_profile_id, role, joined_at, left_at,
                    added_by_agent_id
                )
                VALUES ($1, $2, $3, $4, $5, NULL, $6)
                ON CONFLICT (project_id, agent_profile_id) DO UPDATE
                SET role = EXCLUDED.role,
                    joined_at = EXCLUDED.joined_at,
                    left_at = NULL,
                    added_by_agent_id = EXCLUDED.added_by_agent_id
                "#,
                &[
                    &bundle.member.id,
                    &bundle.member.project_id,
                    &bundle.member.agent_profile_id,
                    &bundle.member.role,
                    &bundle.member.joined_at,
                    &bundle.member.added_by_agent_id,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO conversation_members (
                    id, conversation_id, agent_profile_id, member_role, joined_at, left_at
                )
                VALUES ($1, $2, $3, 'member', $4, NULL)
                ON CONFLICT (conversation_id, agent_profile_id) DO UPDATE
                SET member_role = 'member',
                    joined_at = EXCLUDED.joined_at,
                    left_at = NULL
                "#,
                &[
                    &Uuid::new_v4(),
                    &bundle.conversation_preview.preview.id,
                    &bundle.conversation_preview.agent_id,
                    &bundle.conversation_preview.preview.updated_at,
                ],
            )?;
            tx.execute(
                "UPDATE company_projects SET updated_at = $2 WHERE id = $1",
                &[
                    &bundle.member.project_id,
                    &bundle.conversation_preview.preview.updated_at,
                ],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn complete_company_project_owner_transfer(
        &self,
        bundle: CompanyProjectOwnerTransferBundle,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                UPDATE company_projects
                SET owner_agent_id = $2,
                    updated_by_agent_id = $3,
                    updated_at = $4
                WHERE id = $1
                "#,
                &[
                    &bundle.project.id,
                    &bundle.project.owner_agent_id,
                    &bundle.project.updated_by_agent_id,
                    &bundle.project.updated_at,
                ],
            )?;
            tx.execute(
                r#"
                UPDATE company_project_members
                SET role = 'member', left_at = NULL
                WHERE project_id = $1 AND agent_profile_id = $2
                "#,
                &[&bundle.project.id, &bundle.previous_owner_agent_id],
            )?;
            tx.execute(
                r#"
                INSERT INTO company_project_members (
                    id, project_id, agent_profile_id, role, joined_at, left_at,
                    added_by_agent_id
                )
                VALUES ($1, $2, $3, 'owner', $4, NULL, $5)
                ON CONFLICT (project_id, agent_profile_id) DO UPDATE
                SET role = 'owner',
                    joined_at = EXCLUDED.joined_at,
                    left_at = NULL,
                    added_by_agent_id = EXCLUDED.added_by_agent_id
                "#,
                &[
                    &bundle.new_owner_member.id,
                    &bundle.new_owner_member.project_id,
                    &bundle.new_owner_member.agent_profile_id,
                    &bundle.new_owner_member.joined_at,
                    &bundle.new_owner_member.added_by_agent_id,
                ],
            )?;
            tx.execute(
                r#"
                UPDATE conversation_members
                SET member_role = 'member', left_at = NULL
                WHERE conversation_id = $1 AND agent_profile_id = $2
                "#,
                &[
                    &bundle.project.project_group_conversation_id,
                    &bundle.previous_owner_agent_id,
                ],
            )?;
            tx.execute(
                r#"
                INSERT INTO conversation_members (
                    id, conversation_id, agent_profile_id, member_role, joined_at, left_at
                )
                VALUES ($1, $2, $3, 'owner', $4, NULL)
                ON CONFLICT (conversation_id, agent_profile_id) DO UPDATE
                SET member_role = 'owner',
                    joined_at = EXCLUDED.joined_at,
                    left_at = NULL
                "#,
                &[
                    &Uuid::new_v4(),
                    &bundle.project.project_group_conversation_id,
                    &bundle.new_owner_member.agent_profile_id,
                    &bundle.new_owner_member.joined_at,
                ],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn complete_company_project_member_remove(
        &self,
        project_id: Uuid,
        agent_id: Uuid,
        conversation_id: Uuid,
        left_at: chrono::DateTime<Utc>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                UPDATE company_project_members
                SET left_at = $3
                WHERE project_id = $1 AND agent_profile_id = $2 AND left_at IS NULL
                "#,
                &[&project_id, &agent_id, &left_at],
            )?;
            tx.execute(
                r#"
                UPDATE conversation_members
                SET left_at = $3
                WHERE conversation_id = $1 AND agent_profile_id = $2 AND left_at IS NULL
                "#,
                &[&conversation_id, &agent_id, &left_at],
            )?;
            tx.execute(
                "UPDATE company_projects SET updated_at = $2 WHERE id = $1",
                &[&project_id, &left_at],
            )?;
            tx.commit()?;
            Ok(())
        })
    }
}

fn insert_company_project_creation(
    client: &mut impl GenericClient,
    bundle: &CompanyProjectCreationBundle,
) -> AppResult<()> {
    if bundle.conversation_members.is_empty() || bundle.members.is_empty() {
        return Err(AppError::Validation(
            "company project members required".into(),
        ));
    }
    let project_type_evidence = serde_json::to_value(&bundle.project.project_type_evidence)
        .map_err(|error| {
            AppError::Internal(format!(
                "failed to serialize project type evidence: {error}"
            ))
        })?;
    let preview = &bundle.conversation_members[0].preview;
    let title = Some(preview.title.as_str());
    client
        .execute(
            r#"
            INSERT INTO conversations (
                id, conversation_type, title, created_by_agent_id, status,
                company_id, project_id, context_type, visibility,
                last_message_at, created_at, updated_at
            )
            VALUES ($1, 'group', $2, $3, 'active', $4, NULL, 'project_group',
                    'members', $5, $5, $5)
            "#,
            &[
                &preview.id,
                &title,
                &bundle.project.created_by_agent_id,
                &bundle.project.company_id,
                &preview.updated_at,
            ],
        )
        .map_err(map_postgres_error)?;
    for conversation_member in &bundle.conversation_members {
        let role = if conversation_member.agent_id == bundle.project.owner_agent_id {
            "owner"
        } else {
            "member"
        };
        client
            .execute(
                r#"
                INSERT INTO conversation_members (
                    id, conversation_id, agent_profile_id, member_role, joined_at
                )
                VALUES ($1, $2, $3, $4, $5)
                "#,
                &[
                    &Uuid::new_v4(),
                    &preview.id,
                    &conversation_member.agent_id,
                    &role,
                    &preview.updated_at,
                ],
            )
            .map_err(map_postgres_error)?;
    }
    client
        .execute(
            r#"
            INSERT INTO company_projects (
                id, company_id, name, description, project_type, project_type_source,
                project_type_confidence, project_type_evidence, status, owner_agent_id,
                project_group_conversation_id, created_by_agent_id,
                updated_by_agent_id, due_at, created_at, updated_at, completed_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
            "#,
            &[
                &bundle.project.id,
                &bundle.project.company_id,
                &bundle.project.name,
                &bundle.project.description,
                &bundle.project.project_type,
                &bundle.project.project_type_source,
                &bundle.project.project_type_confidence,
                &project_type_evidence,
                &bundle.project.status,
                &bundle.project.owner_agent_id,
                &bundle.project.project_group_conversation_id,
                &bundle.project.created_by_agent_id,
                &bundle.project.updated_by_agent_id,
                &bundle.project.due_at,
                &bundle.project.created_at,
                &bundle.project.updated_at,
                &bundle.project.completed_at,
            ],
        )
        .map_err(map_postgres_error)?;
    client
        .execute(
            "UPDATE conversations SET project_id = $1 WHERE id = $2",
            &[&bundle.project.id, &preview.id],
        )
        .map_err(map_postgres_error)?;
    for member in &bundle.members {
        client
            .execute(
                r#"
                INSERT INTO company_project_members (
                    id, project_id, agent_profile_id, role, joined_at, left_at,
                    added_by_agent_id
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
                &[
                    &member.id,
                    &member.project_id,
                    &member.agent_profile_id,
                    &member.role,
                    &member.joined_at,
                    &member.left_at,
                    &member.added_by_agent_id,
                ],
            )
            .map_err(map_postgres_error)?;
    }
    Ok(())
}

fn upsert_company_project_git_config(
    client: &mut impl GenericClient,
    config: &CompanyProjectGitConfig,
) -> AppResult<()> {
    client
        .execute(
            r#"
            INSERT INTO company_project_git_configs (
                project_id, remote_url, default_branch, git_host, host_local_path,
                auth_profile, allow_agent_push, branch_prefix, created_by_agent_id,
                created_by_human_user_id, updated_by_agent_id,
                updated_by_human_user_id, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            ON CONFLICT (project_id) DO UPDATE
            SET remote_url = EXCLUDED.remote_url,
                default_branch = EXCLUDED.default_branch,
                git_host = EXCLUDED.git_host,
                host_local_path = EXCLUDED.host_local_path,
                auth_profile = EXCLUDED.auth_profile,
                allow_agent_push = EXCLUDED.allow_agent_push,
                branch_prefix = EXCLUDED.branch_prefix,
                created_by_agent_id = EXCLUDED.created_by_agent_id,
                created_by_human_user_id = EXCLUDED.created_by_human_user_id,
                updated_by_agent_id = EXCLUDED.updated_by_agent_id,
                updated_by_human_user_id = EXCLUDED.updated_by_human_user_id,
                created_at = EXCLUDED.created_at,
                updated_at = EXCLUDED.updated_at
            "#,
            &[
                &config.project_id,
                &config.remote_url,
                &config.default_branch,
                &config.git_host,
                &config.host_local_path,
                &config.auth_profile,
                &config.allow_agent_push,
                &config.branch_prefix,
                &config.created_by_agent_id,
                &config.created_by_human_user_id,
                &config.updated_by_agent_id,
                &config.updated_by_human_user_id,
                &config.created_at,
                &config.updated_at,
            ],
        )
        .map_err(map_postgres_error)?;
    Ok(())
}
