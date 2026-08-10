use super::mapping::map_postgres_error;
use super::*;

pub(super) fn insert_company_project_creation(
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
            ) VALUES ($1, $2, $3, $4, $5)
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
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
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
                id, project_id, agent_profile_id, role, joined_at, left_at, added_by_agent_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
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

pub(super) fn upsert_company_project_git_config(
    client: &mut impl GenericClient,
    config: &CompanyProjectGitConfig,
) -> AppResult<()> {
    client.execute(
        r#"
        INSERT INTO company_project_git_configs (
            project_id, remote_url, default_branch, git_host, host_local_path,
            auth_profile, allow_agent_push, branch_prefix, created_by_agent_id,
            created_by_human_user_id, updated_by_agent_id,
            updated_by_human_user_id, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        ON CONFLICT (project_id) DO UPDATE SET
            remote_url = EXCLUDED.remote_url, default_branch = EXCLUDED.default_branch,
            git_host = EXCLUDED.git_host, host_local_path = EXCLUDED.host_local_path,
            auth_profile = EXCLUDED.auth_profile, allow_agent_push = EXCLUDED.allow_agent_push,
            branch_prefix = EXCLUDED.branch_prefix, created_by_agent_id = EXCLUDED.created_by_agent_id,
            created_by_human_user_id = EXCLUDED.created_by_human_user_id,
            updated_by_agent_id = EXCLUDED.updated_by_agent_id,
            updated_by_human_user_id = EXCLUDED.updated_by_human_user_id,
            created_at = EXCLUDED.created_at, updated_at = EXCLUDED.updated_at
        "#,
        &[&config.project_id, &config.remote_url, &config.default_branch, &config.git_host,
          &config.host_local_path, &config.auth_profile, &config.allow_agent_push,
          &config.branch_prefix, &config.created_by_agent_id, &config.created_by_human_user_id,
          &config.updated_by_agent_id, &config.updated_by_human_user_id,
          &config.created_at, &config.updated_at],
    ).map_err(map_postgres_error)?;
    Ok(())
}

pub(super) fn map_project_provisioning_cleanup_job(row: Row) -> ProjectProvisioningCleanupJob {
    ProjectProvisioningCleanupJob {
        id: row.get("id"),
        human_user_id: row.get("human_user_id"),
        company_id: row.get("company_id"),
        project_id: row.get("project_id"),
        managed_local_path: row.get("managed_local_path"),
        repository_identifier: row.get("repository_identifier"),
        access_token_identifier: row.get("access_token_identifier"),
        status: row.get("status"),
        attempts: row.get("attempts"),
        next_attempt_at: row.get("next_attempt_at"),
        lease_expires_at: row.get("lease_expires_at"),
        last_error: row.get("last_error"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        completed_at: row.get("completed_at"),
    }
}
