use chrono::Utc;
use postgres::types::Json;
use postgres::{Client, GenericClient, NoTls, Row};
use r2d2::Pool;
use r2d2_postgres::PostgresConnectionManager;
use serde_json::Value;
use tokio::runtime::{Handle, RuntimeFlavor};
use uuid::Uuid;

use ai_chat_application::{
    AgentStaffingHireBundle, AgentStaffingStatusChangeBundle, CompanyAgentActivationBundle,
    CompanyAgentCreationBundle, CompanyAgentMembershipUpdateBundle,
    CompanyConversationCreationBundle, CompanyCreationBundle, CompanyProjectCreationBundle,
    CompanyProjectMemberAddBundle, CompleteAgentCodexTriggerLeaseInput,
    HumanCompanyDirectConversationCreationBundle, MessagePageView, PlatformRepository,
    ProblemWorkspace, ProblemWorkspaceInvitation, ProblemWorkspaceProgressRecord,
    RegistrationCompletionBundle,
};
use ai_chat_domain::agent_identity::{
    AgentActionLog, AgentActionStatus, AgentIdempotencyRecord, AgentInboxEvent,
    AgentInboxEventStatus, AgentKeyIssueLog, AgentKeyIssueType, AgentKeyRecord, AgentOwnerBinding,
    AgentProfile, AgentRegistrationRequest, AgentStatus, ChallengeStatus, HumanAccountToken,
    HumanCredential, HumanSession, HumanUser, OwnershipProofChallenge, OwnershipProofProvider,
    RegistrationStatus, SocialProofSubmission,
};
use ai_chat_domain::company::{
    AgentCodexRunActivity, AgentCodexRunToken, AgentCodexSession, AgentCodexTriggerConfig,
    AgentCodexTriggerRun, AgentModelPriceCatalogEntry, AgentRuntimeConfig, AgentRuntimeDailyUsage,
    AgentRuntimeRun, AgentRuntimeTemplate, AgentRuntimeTemplateSettings, AgentStaffingAction,
    AgentToolApprovalRequest, Company, CompanyAgentMembership, CompanyCodexRunnerProfile,
    CompanyGovernancePolicySettings, CompanyGovernancePolicyVersion, CompanyHumanMember,
    CompanyModelBudgetPolicy, CompanyModelDailyUsage, CompanyProject, CompanyProjectAsset,
    CompanyProjectAssetRefreshConfig, CompanyProjectGitConfig, CompanyProjectMember,
    CompanyProjectRule, CompanyProjectStatusUpdate, CompanyProjectTask,
    CompanyProjectTaskDependency, CompanyProjectTaskStatusHistory, CompanyRealtimeEvent,
    CompanyRuntimePolicy, OrgUnit, COMPANY_AGENT_ROLE_MANAGER,
};
use ai_chat_domain::social::{
    ConversationContext, ConversationPreview, ConversationType, DiaryEntryView, FriendProfileFact,
    FriendProfileFactSourceKind, FriendProfileFactType, FriendProfileSnapshot, FriendRequestView,
    FriendSummary, MessageView, PostCommentView, PostView,
};
use ai_chat_shared::{hash_secret, AppError, AppResult};

#[derive(Clone)]
pub struct PostgresPlatformRepository {
    pool: Pool<PostgresConnectionManager<NoTls>>,
}

impl PostgresPlatformRepository {
    pub fn connect(database_url: &str) -> anyhow::Result<Self> {
        let postgres_config = database_url.parse::<postgres::Config>()?;
        let manager = PostgresConnectionManager::new(postgres_config, NoTls);
        let max_size = std::env::var("DATABASE_POOL_SIZE")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(16);
        let pool = run_sync_postgres(|| {
            Pool::builder()
                .max_size(max_size)
                .build(manager)
                .map_err(|error| AppError::Validation(format!("postgres pool error: {error}")))
        })
        .map_err(anyhow::Error::msg)?;
        run_sync_postgres(|| {
            let mut client = pool
                .get()
                .map_err(|error| AppError::Validation(format!("postgres pool error: {error}")))?;
            client
                .simple_query("SELECT 1")
                .map_err(map_postgres_error)?;
            Ok(())
        })
        .map_err(anyhow::Error::msg)?;
        Ok(Self { pool })
    }

    fn with_client<T>(
        &self,
        f: impl FnOnce(&mut Client) -> Result<T, postgres::Error>,
    ) -> AppResult<T> {
        run_sync_postgres(|| {
            let mut client = self
                .pool
                .get()
                .map_err(|error| AppError::Validation(format!("postgres pool error: {error}")))?;
            f(&mut client).map_err(map_postgres_error)
        })
    }
}

fn run_sync_postgres<T>(f: impl FnOnce() -> AppResult<T>) -> AppResult<T> {
    match Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(f)
        }
        _ => f(),
    }
}

impl PlatformRepository for PostgresPlatformRepository {
    fn health_check(&self) -> AppResult<()> {
        self.with_client(|client| client.simple_query("SELECT 1").map(|_| ()))
    }

    fn find_human_user_by_email(&self, email: &str) -> Option<HumanUser> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, email, display_name, created_at
                FROM human_users
                WHERE lower(email) = lower($1)
                "#,
                &[&email],
            )
        })
        .ok()
        .flatten()
        .map(map_human_user)
    }

    fn get_human_user(&self, user_id: Uuid) -> Option<HumanUser> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, email, display_name, created_at
                FROM human_users
                WHERE id = $1
                "#,
                &[&user_id],
            )
        })
        .ok()
        .flatten()
        .map(map_human_user)
    }

    fn list_human_users(&self) -> Vec<HumanUser> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, email, display_name, created_at
                FROM human_users
                ORDER BY created_at DESC
                "#,
                &[],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_human_user)
        .collect()
    }

    fn insert_human_user(&self, user: HumanUser) -> AppResult<HumanUser> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO human_users (id, email, display_name, status, created_at, updated_at)
                VALUES ($1, $2, $3, 'active', $4, $4)
                "#,
                &[&user.id, &user.email, &user.display_name, &user.created_at],
            )?;
            Ok(user)
        })
    }

    fn human_user_exists(&self, user_id: Uuid) -> bool {
        self.with_client(|client| {
            client
                .query_one(
                    "SELECT COUNT(1) FROM human_users WHERE id = $1",
                    &[&user_id],
                )
                .map(|row| row.get::<_, i64>(0) > 0)
        })
        .unwrap_or(false)
    }

    fn get_human_credential(&self, user_id: Uuid) -> Option<HumanCredential> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT human_user_id, password_hash, created_at, updated_at
                FROM human_credentials
                WHERE human_user_id = $1
                "#,
                &[&user_id],
            )
        })
        .ok()
        .flatten()
        .map(map_human_credential)
    }

    fn insert_human_auth_bundle(
        &self,
        user: HumanUser,
        credential: HumanCredential,
    ) -> AppResult<HumanUser> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                INSERT INTO human_users (id, email, display_name, status, created_at, updated_at)
                VALUES ($1, $2, $3, 'active', $4, $4)
                "#,
                &[&user.id, &user.email, &user.display_name, &user.created_at],
            )?;
            tx.execute(
                r#"
                INSERT INTO human_credentials (
                    human_user_id, password_hash, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4)
                "#,
                &[
                    &credential.human_user_id,
                    &credential.password_hash,
                    &credential.created_at,
                    &credential.updated_at,
                ],
            )?;
            tx.commit()?;
            Ok(user)
        })
    }

    fn insert_human_session(&self, session: HumanSession) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO human_sessions (
                    id, human_user_id, token_prefix, token_hash, expires_at,
                    revoked_at, last_used_at, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
                &[
                    &session.id,
                    &session.human_user_id,
                    &session.token_prefix,
                    &session.token_hash,
                    &session.expires_at,
                    &session.revoked_at,
                    &session.last_used_at,
                    &session.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn find_human_session_by_token_hash(&self, token_hash: &str) -> Option<HumanSession> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, human_user_id, token_prefix, token_hash, expires_at,
                       revoked_at, last_used_at, created_at
                FROM human_sessions
                WHERE token_hash = $1
                "#,
                &[&token_hash],
            )
        })
        .ok()
        .flatten()
        .map(map_human_session)
    }

    fn touch_human_session(
        &self,
        session_id: Uuid,
        used_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                "UPDATE human_sessions SET last_used_at = $2 WHERE id = $1",
                &[&session_id, &used_at],
            )?;
            Ok(())
        })
    }

    fn revoke_human_session(
        &self,
        session_id: Uuid,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE human_sessions
                SET revoked_at = COALESCE(revoked_at, $2)
                WHERE id = $1
                "#,
                &[&session_id, &revoked_at],
            )?;
            Ok(())
        })
    }

    fn get_human_session(&self, session_id: Uuid) -> Option<HumanSession> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, human_user_id, token_prefix, token_hash, expires_at,
                       revoked_at, last_used_at, created_at
                FROM human_sessions
                WHERE id = $1
                "#,
                &[&session_id],
            )
        })
        .ok()
        .flatten()
        .map(map_human_session)
    }

    fn list_human_sessions(&self, human_user_id: Uuid) -> Vec<HumanSession> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, human_user_id, token_prefix, token_hash, expires_at,
                       revoked_at, last_used_at, created_at
                FROM human_sessions
                WHERE human_user_id = $1
                ORDER BY created_at DESC
                LIMIT 100
                "#,
                &[&human_user_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_human_session)
        .collect()
    }

    fn revoke_human_sessions(
        &self,
        human_user_id: Uuid,
        except_session_id: Option<Uuid>,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        self.with_client(|client| {
            client
                .execute(
                    r#"
                    UPDATE human_sessions
                    SET revoked_at = $3
                    WHERE human_user_id = $1
                      AND revoked_at IS NULL
                      AND ($2::uuid IS NULL OR id <> $2)
                    "#,
                    &[&human_user_id, &except_session_id, &revoked_at],
                )
                .map(|count| count as usize)
        })
    }

    fn update_human_password_hash(
        &self,
        human_user_id: Uuid,
        password_hash: String,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE human_credentials
                SET password_hash = $2, updated_at = $3
                WHERE human_user_id = $1
                "#,
                &[&human_user_id, &password_hash, &updated_at],
            )?;
            Ok(())
        })
    }

    fn delete_expired_human_sessions(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        self.with_client(|client| {
            client
                .execute(
                    r#"
                    DELETE FROM human_sessions
                    WHERE expires_at <= $1
                       OR (revoked_at IS NOT NULL AND revoked_at <= $1 - INTERVAL '30 days')
                    "#,
                    &[&now],
                )
                .map(|count| count as usize)
        })
    }

    fn insert_human_account_token(&self, token: HumanAccountToken) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO human_account_tokens (
                    id, human_user_id, purpose, token_prefix, token_hash,
                    expires_at, used_at, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
                &[
                    &token.id,
                    &token.human_user_id,
                    &token.purpose,
                    &token.token_prefix,
                    &token.token_hash,
                    &token.expires_at,
                    &token.used_at,
                    &token.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn find_human_account_token_by_hash(&self, token_hash: &str) -> Option<HumanAccountToken> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, human_user_id, purpose, token_prefix, token_hash,
                       expires_at, used_at, created_at
                FROM human_account_tokens
                WHERE token_hash = $1
                "#,
                &[&token_hash],
            )
        })
        .ok()
        .flatten()
        .map(map_human_account_token)
    }

    fn mark_human_account_token_used(
        &self,
        token_id: Uuid,
        used_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let updated = self.with_client(|client| {
            client.execute(
                r#"
                UPDATE human_account_tokens
                SET used_at = $2
                WHERE id = $1 AND used_at IS NULL
                "#,
                &[&token_id, &used_at],
            )
        })?;
        if updated == 0 {
            return Err(AppError::Conflict(
                "human account token was already used or not found".into(),
            ));
        }
        Ok(())
    }

    fn mark_human_email_verified(
        &self,
        human_user_id: Uuid,
        verified_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO human_email_verifications (human_user_id, verified_at)
                VALUES ($1, $2)
                ON CONFLICT (human_user_id)
                DO UPDATE SET verified_at = EXCLUDED.verified_at
                "#,
                &[&human_user_id, &verified_at],
            )?;
            Ok(())
        })
    }

    fn is_human_email_verified(&self, human_user_id: Uuid) -> bool {
        self.with_client(|client| {
            client
                .query_one(
                    "SELECT COUNT(1) FROM human_email_verifications WHERE human_user_id = $1",
                    &[&human_user_id],
                )
                .map(|row| row.get::<_, i64>(0) > 0)
        })
        .unwrap_or(false)
    }

    fn delete_expired_human_account_tokens(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        self.with_client(|client| {
            client
                .execute(
                    r#"
                    DELETE FROM human_account_tokens
                    WHERE expires_at <= $1
                       OR (used_at IS NOT NULL AND used_at <= $1 - INTERVAL '30 days')
                    "#,
                    &[&now],
                )
                .map(|count| count as usize)
        })
    }

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
        .ok()
        .flatten()
        .map(map_company)
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
        .ok()
        .flatten()
        .map(map_company_human_member)
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
            if let Some(runtime_config) = &bundle.runtime_config {
                upsert_agent_runtime_config(&mut tx, runtime_config)?;
            }
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
            if let Some(runtime_config) = &bundle.runtime_config {
                upsert_agent_runtime_config(&mut tx, runtime_config)?;
            }
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

    fn insert_registration_request(&self, request: AgentRegistrationRequest) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_registration_requests (
                    id, human_user_id, desired_handle, desired_display_name, persona,
                    proof_provider, proof_account_handle, status, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, 'weibo', $6, $7, $8, $8)
                "#,
                &[
                    &request.id,
                    &request.human_user_id,
                    &request.desired_handle,
                    &request.desired_display_name,
                    &request.persona,
                    &request.weibo_handle,
                    &registration_status_to_str(&request.status),
                    &request.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn insert_ownership_challenge(&self, challenge: OwnershipProofChallenge) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO ownership_proof_challenges (
                    id, human_user_id, registration_request_id, provider,
                    account_handle, verification_code, template_text, expires_at, verified_at,
                    status, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NULL, $9, $10)
                "#,
                &[
                    &challenge.id,
                    &challenge.human_user_id,
                    &challenge.registration_request_id,
                    &ownership_provider_to_str(&challenge.provider),
                    &challenge.account_handle,
                    &challenge.verification_code,
                    &challenge.template_text,
                    &challenge.expires_at,
                    &challenge_status_to_str(&challenge.status),
                    &challenge.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_challenge(&self, challenge_id: Uuid) -> Option<OwnershipProofChallenge> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, human_user_id, registration_request_id, provider,
                       account_handle, verification_code, template_text, expires_at, status, created_at
                FROM ownership_proof_challenges
                WHERE id = $1
                "#,
                &[&challenge_id],
            )
        })
        .ok()
        .flatten()
        .map(map_challenge)
    }

    fn list_challenges(&self) -> Vec<OwnershipProofChallenge> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, human_user_id, registration_request_id, provider,
                       account_handle, verification_code, template_text, expires_at, status, created_at
                FROM ownership_proof_challenges
                ORDER BY created_at DESC
                "#,
                &[],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_challenge)
        .collect()
    }

    fn update_challenge(&self, challenge: OwnershipProofChallenge) -> AppResult<()> {
        let verified_at = if matches!(challenge.status, ChallengeStatus::Verified) {
            Some(Utc::now())
        } else {
            None
        };

        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE ownership_proof_challenges
                SET status = $2, expires_at = $3, verified_at = $4
                WHERE id = $1
                "#,
                &[
                    &challenge.id,
                    &challenge_status_to_str(&challenge.status),
                    &challenge.expires_at,
                    &verified_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_registration_request(&self, request_id: Uuid) -> Option<AgentRegistrationRequest> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, human_user_id, desired_handle, desired_display_name,
                       persona, proof_account_handle, status, created_at
                FROM agent_registration_requests
                WHERE id = $1
                "#,
                &[&request_id],
            )
        })
        .ok()
        .flatten()
        .map(map_registration_request)
    }

    fn list_registration_requests(&self) -> Vec<AgentRegistrationRequest> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, human_user_id, desired_handle, desired_display_name,
                       persona, proof_account_handle, status, created_at
                FROM agent_registration_requests
                ORDER BY created_at DESC
                "#,
                &[],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_registration_request)
        .collect()
    }

    fn update_registration_request(&self, request: AgentRegistrationRequest) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_registration_requests
                SET desired_handle = $2,
                    desired_display_name = $3,
                    persona = $4,
                    proof_account_handle = $5,
                    status = $6,
                    updated_at = NOW()
                WHERE id = $1
                "#,
                &[
                    &request.id,
                    &request.desired_handle,
                    &request.desired_display_name,
                    &request.persona,
                    &request.weibo_handle,
                    &registration_status_to_str(&request.status),
                ],
            )?;
            Ok(())
        })
    }

    fn insert_agent_profile(&self, profile: AgentProfile) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_profiles (
                    id, owner_user_id, handle, display_name, persona,
                    collaboration_preference, status,
                    visibility, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, 'public', $8, $8)
                "#,
                &[
                    &profile.id,
                    &profile.owner_user_id,
                    &profile.handle,
                    &profile.display_name,
                    &profile.persona,
                    &profile.collaboration_preference,
                    &agent_status_to_str(&profile.status),
                    &profile.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn complete_registration_bundle(&self, bundle: RegistrationCompletionBundle) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;

            tx.execute(
                r#"
                UPDATE ownership_proof_challenges
                SET status = $2, expires_at = $3, verified_at = NOW()
                WHERE id = $1
                "#,
                &[
                    &bundle.challenge.id,
                    &challenge_status_to_str(&bundle.challenge.status),
                    &bundle.challenge.expires_at,
                ],
            )?;

            tx.execute(
                r#"
                UPDATE agent_registration_requests
                SET desired_handle = $2,
                    desired_display_name = $3,
                    persona = $4,
                    proof_account_handle = $5,
                    status = $6,
                    updated_at = NOW()
                WHERE id = $1
                "#,
                &[
                    &bundle.registration.id,
                    &bundle.registration.desired_handle,
                    &bundle.registration.desired_display_name,
                    &bundle.registration.persona,
                    &bundle.registration.weibo_handle,
                    &registration_status_to_str(&bundle.registration.status),
                ],
            )?;

            tx.execute(
                r#"
                INSERT INTO agent_profiles (
                    id, owner_user_id, handle, display_name, persona,
                    collaboration_preference, status,
                    visibility, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, 'public', $8, $8)
                "#,
                &[
                    &bundle.agent.id,
                    &bundle.agent.owner_user_id,
                    &bundle.agent.handle,
                    &bundle.agent.display_name,
                    &bundle.agent.persona,
                    &bundle.agent.collaboration_preference,
                    &agent_status_to_str(&bundle.agent.status),
                    &bundle.agent.created_at,
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
                    &bundle.binding.id,
                    &bundle.binding.human_user_id,
                    &bundle.binding.agent_profile_id,
                    &bundle.binding.created_at,
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
                INSERT INTO social_proof_submissions (
                    id, challenge_id, submitted_text, source_url, provider_post_id,
                    verification_mode, verification_evidence, raw_payload, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
                &[
                    &bundle.submission.id,
                    &bundle.submission.challenge_id,
                    &bundle.submission.submitted_text,
                    &bundle.submission.source_url,
                    &bundle.submission.provider_post_id,
                    &bundle.submission.verification_mode,
                    &bundle.submission.verification_evidence,
                    &Json(bundle.submission.raw_payload.clone()),
                    &bundle.submission.created_at,
                ],
            )?;

            let title: Option<&str> = if bundle.self_notes_conversation.title.trim().is_empty() {
                None
            } else {
                Some(bundle.self_notes_conversation.title.as_str())
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
                    &bundle.self_notes_conversation.id,
                    &conversation_type_to_str(&bundle.self_notes_conversation.conversation_type),
                    &title,
                    &bundle.agent.id,
                    &bundle.self_notes_conversation.updated_at,
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
                &[
                    &Uuid::new_v4(),
                    &bundle.self_notes_conversation.id,
                    &bundle.agent.id,
                    &bundle.self_notes_conversation.updated_at,
                ],
            )?;

            tx.commit()?;
            Ok(())
        })
    }

    fn list_agent_profiles(&self) -> Vec<AgentProfile> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, owner_user_id, display_name, handle, persona,
                       collaboration_preference, status, created_at
                FROM agent_profiles
                ORDER BY created_at DESC
                "#,
                &[],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_profile)
        .collect()
    }

    fn update_agent_profile_status(&self, agent_id: Uuid, status: AgentStatus) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_profiles
                SET status = $2,
                    updated_at = NOW()
                WHERE id = $1
                "#,
                &[&agent_id, &agent_status_to_str(&status)],
            )?;
            Ok(())
        })
    }

    fn update_agent_profile(
        &self,
        agent_id: Uuid,
        display_name: String,
        persona: String,
        collaboration_preference: String,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_profiles
                SET display_name = $2,
                    persona = $3,
                    collaboration_preference = $4,
                    updated_at = NOW()
                WHERE id = $1
                "#,
                &[
                    &agent_id,
                    &display_name,
                    &persona,
                    &collaboration_preference,
                ],
            )?;
            Ok(())
        })
    }

    fn insert_owner_binding(&self, binding: AgentOwnerBinding) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_owner_bindings (
                    id, human_user_id, agent_profile_id, binding_role, created_at
                )
                VALUES ($1, $2, $3, 'owner', $4)
                "#,
                &[
                    &binding.id,
                    &binding.human_user_id,
                    &binding.agent_profile_id,
                    &binding.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn insert_agent_key(&self, key_record: AgentKeyRecord) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_keys (
                    id, agent_profile_id, key_name, key_prefix, key_hash,
                    scopes, last_used_at, last_used_ip, expires_at, revoked_at, created_at
                )
                VALUES ($1, $2, $3, $4, $5, '[]'::jsonb, $6, NULL, $7, $8, $9)
                "#,
                &[
                    &key_record.id,
                    &key_record.agent_profile_id,
                    &key_record.key_name,
                    &key_record.key_prefix,
                    &key_record.key_hash,
                    &key_record.last_used_at,
                    &key_record.expires_at,
                    &key_record.revoked_at,
                    &key_record.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn find_agent_key_by_plaintext(&self, plaintext_key: &str) -> Option<AgentKeyRecord> {
        let key_hash = hash_secret(plaintext_key);
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, agent_profile_id, key_name, key_prefix, key_hash,
                       last_used_at, expires_at, revoked_at, created_at
                FROM agent_keys
                WHERE key_hash = $1
                  AND revoked_at IS NULL
                  AND (expires_at IS NULL OR expires_at > NOW())
                "#,
                &[&key_hash],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_key_record)
    }

    fn list_agent_keys(&self, agent_id: Uuid) -> Vec<AgentKeyRecord> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, agent_profile_id, key_name, key_prefix, key_hash,
                       last_used_at, expires_at, revoked_at, created_at
                FROM agent_keys
                WHERE agent_profile_id = $1
                ORDER BY created_at DESC
                "#,
                &[&agent_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_key_record)
        .collect()
    }

    fn touch_agent_key_usage(
        &self,
        key_id: Uuid,
        used_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_keys
                SET last_used_at = $2
                WHERE id = $1
                "#,
                &[&key_id, &used_at],
            )?;
            Ok(())
        })
    }

    fn revoke_agent_keys(
        &self,
        agent_id: Uuid,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_keys
                SET revoked_at = $2
                WHERE agent_profile_id = $1
                  AND revoked_at IS NULL
                "#,
                &[&agent_id, &revoked_at],
            )?;
            Ok(())
        })
    }

    fn insert_agent_key_issue_log(&self, log: AgentKeyIssueLog) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
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
            Ok(())
        })
    }

    fn list_agent_key_issue_logs(&self, agent_id: Uuid, limit: usize) -> Vec<AgentKeyIssueLog> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, agent_profile_id, agent_key_id, issue_type,
                       issued_by_user_id, metadata, created_at
                FROM agent_key_issue_logs
                WHERE agent_profile_id = $1
                ORDER BY created_at DESC
                LIMIT $2
                "#,
                &[&agent_id, &(limit as i64)],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_key_issue_log)
        .collect()
    }

    fn insert_social_proof_submission(&self, submission: SocialProofSubmission) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO social_proof_submissions (
                    id, challenge_id, submitted_text, source_url, provider_post_id,
                    verification_mode, verification_evidence, raw_payload, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
                &[
                    &submission.id,
                    &submission.challenge_id,
                    &submission.submitted_text,
                    &submission.source_url,
                    &submission.provider_post_id,
                    &submission.verification_mode,
                    &submission.verification_evidence,
                    &Json(submission.raw_payload.clone()),
                    &submission.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_social_proof_submissions(&self) -> Vec<SocialProofSubmission> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, challenge_id, submitted_text, source_url, provider_post_id,
                       verification_mode, verification_evidence, raw_payload, created_at
                FROM social_proof_submissions
                ORDER BY created_at DESC
                "#,
                &[],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_social_proof_submission)
        .collect()
    }

    fn insert_agent_inbox_event(&self, event: AgentInboxEvent) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_event_inbox (
                    id, agent_profile_id, event_type, payload_json,
                    priority, available_at, processed_at, status, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
                &[
                    &event.id,
                    &event.agent_profile_id,
                    &event.event_type,
                    &Json(event.payload_json.clone()),
                    &event.priority,
                    &event.available_at,
                    &event.processed_at,
                    &agent_inbox_event_status_to_str(&event.status),
                    &event.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_agent_inbox_events(
        &self,
        agent_id: Uuid,
        status: Option<AgentInboxEventStatus>,
        limit: usize,
    ) -> Vec<AgentInboxEvent> {
        self.with_client(|client| {
            if let Some(status) = status.as_ref() {
                client.query(
                    r#"
                    SELECT id, agent_profile_id, event_type, payload_json,
                           priority, available_at, processed_at, status, created_at
                    FROM agent_event_inbox
                    WHERE agent_profile_id = $1
                      AND status = $2
                    ORDER BY priority ASC, created_at DESC
                    LIMIT $3
                    "#,
                    &[
                        &agent_id,
                        &agent_inbox_event_status_to_str(status),
                        &(limit as i64),
                    ],
                )
            } else {
                client.query(
                    r#"
                    SELECT id, agent_profile_id, event_type, payload_json,
                           priority, available_at, processed_at, status, created_at
                    FROM agent_event_inbox
                    WHERE agent_profile_id = $1
                    ORDER BY priority ASC, created_at DESC
                    LIMIT $2
                    "#,
                    &[&agent_id, &(limit as i64)],
                )
            }
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_inbox_event)
        .collect()
    }

    fn get_agent_inbox_event(&self, event_id: Uuid) -> Option<AgentInboxEvent> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, agent_profile_id, event_type, payload_json,
                       priority, available_at, processed_at, status, created_at
                FROM agent_event_inbox
                WHERE id = $1
                "#,
                &[&event_id],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_inbox_event)
    }

    fn update_agent_inbox_event(&self, event: AgentInboxEvent) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_event_inbox
                SET event_type = $2,
                    payload_json = $3,
                    priority = $4,
                    available_at = $5,
                    processed_at = $6,
                    status = $7
                WHERE id = $1
                "#,
                &[
                    &event.id,
                    &event.event_type,
                    &Json(event.payload_json.clone()),
                    &event.priority,
                    &event.available_at,
                    &event.processed_at,
                    &agent_inbox_event_status_to_str(&event.status),
                ],
            )?;
            Ok(())
        })
    }

    fn insert_agent_action_log(&self, log: AgentActionLog) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_action_logs (
                    id, agent_profile_id, action_type, target_ref,
                    request_payload, result_payload, status, trace_id, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
                &[
                    &log.id,
                    &log.agent_profile_id,
                    &log.action_type,
                    &log.target_ref,
                    &Json(log.request_payload.clone()),
                    &Json(log.result_payload.clone()),
                    &agent_action_status_to_str(&log.status),
                    &log.trace_id,
                    &log.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_all_agent_action_logs(&self) -> Vec<AgentActionLog> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, agent_profile_id, action_type, target_ref,
                       request_payload, result_payload, status, trace_id, created_at
                FROM agent_action_logs
                ORDER BY created_at DESC
                "#,
                &[],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_action_log)
        .collect()
    }

    fn count_agent_actions_since(
        &self,
        agent_id: Uuid,
        since: chrono::DateTime<chrono::Utc>,
    ) -> usize {
        self.with_client(|client| {
            client.query_one(
                r#"
                SELECT COUNT(1)
                FROM agent_action_logs
                WHERE agent_profile_id = $1 AND created_at >= $2
                "#,
                &[&agent_id, &since],
            )
        })
        .map(|row| row.get::<_, i64>(0).max(0) as usize)
        .unwrap_or(0)
    }

    fn get_agent_idempotency_record(
        &self,
        agent_id: Uuid,
        operation: &str,
        idempotency_key: &str,
    ) -> Option<AgentIdempotencyRecord> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, agent_profile_id, operation, idempotency_key,
                       request_hash, response_json, expires_at, created_at
                FROM agent_idempotency_records
                WHERE agent_profile_id = $1
                  AND operation = $2
                  AND idempotency_key = $3
                  AND expires_at > NOW()
                "#,
                &[&agent_id, &operation, &idempotency_key],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_idempotency_record)
    }

    fn insert_agent_idempotency_record(&self, record: AgentIdempotencyRecord) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_idempotency_records (
                    id, agent_profile_id, operation, idempotency_key,
                    request_hash, response_json, expires_at, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (agent_profile_id, operation, idempotency_key) DO NOTHING
                "#,
                &[
                    &record.id,
                    &record.agent_profile_id,
                    &record.operation,
                    &record.idempotency_key,
                    &record.request_hash,
                    &Json(record.response_json.clone()),
                    &record.expires_at,
                    &record.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn delete_expired_agent_idempotency_records(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        self.with_client(|client| {
            client
                .execute(
                    "DELETE FROM agent_idempotency_records WHERE expires_at <= $1",
                    &[&now],
                )
                .map(|count| count as usize)
        })
    }

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
        .ok()
        .flatten()
        .map(|row| ConversationContext {
            conversation_id: row.get("id"),
            company_id: row.get("company_id"),
            project_id: row.get("project_id"),
            context_type: row.get("context_type"),
            visibility: row.get("visibility"),
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

    fn complete_company_project_creation(
        &self,
        bundle: CompanyProjectCreationBundle,
    ) -> AppResult<()> {
        if bundle.conversation_members.is_empty() || bundle.members.is_empty() {
            return Err(AppError::Validation(
                "company project members required".into(),
            ));
        }
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            let preview = &bundle.conversation_members[0].preview;
            let title = Some(preview.title.as_str());
            tx.execute(
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
            )?;
            for conversation_member in &bundle.conversation_members {
                let role = if conversation_member.agent_id == bundle.project.owner_agent_id {
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
                        &preview.id,
                        &conversation_member.agent_id,
                        &role,
                        &preview.updated_at,
                    ],
                )?;
            }
            tx.execute(
                r#"
                INSERT INTO company_projects (
                    id, company_id, name, description, status, owner_agent_id,
                    project_group_conversation_id, created_by_agent_id,
                    updated_by_agent_id, due_at, created_at, updated_at, completed_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
                "#,
                &[
                    &bundle.project.id,
                    &bundle.project.company_id,
                    &bundle.project.name,
                    &bundle.project.description,
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
            )?;
            tx.execute(
                "UPDATE conversations SET project_id = $1 WHERE id = $2",
                &[&bundle.project.id, &preview.id],
            )?;
            for member in &bundle.members {
                tx.execute(
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
                )?;
            }
            tx.commit()?;
            Ok(())
        })
    }

    fn get_company_project(&self, project_id: Uuid) -> Option<CompanyProject> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, name, description, status, owner_agent_id,
                       project_group_conversation_id, created_by_agent_id,
                       updated_by_agent_id, due_at, created_at, updated_at, completed_at
                FROM company_projects
                WHERE id = $1
                "#,
                &[&project_id],
            )
        })
        .ok()
        .flatten()
        .map(map_company_project)
    }

    fn list_company_projects(&self, company_id: Uuid) -> Vec<CompanyProject> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, company_id, name, description, status, owner_agent_id,
                       project_group_conversation_id, created_by_agent_id,
                       updated_by_agent_id, due_at, created_at, updated_at, completed_at
                FROM company_projects
                WHERE company_id = $1
                ORDER BY updated_at DESC, created_at DESC
                "#,
                &[&company_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_company_project)
        .collect()
    }

    fn save_company_project_git_config(&self, config: CompanyProjectGitConfig) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
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
            )?;
            Ok(())
        })
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
                        &asset.metadata,
                        &asset.updated_by_agent_id,
                        &asset.updated_by_human_user_id,
                        &asset.created_at,
                        &asset.updated_at,
                    ],
                )?;
            }
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
                    SELECT project_id
                    FROM company_project_asset_refresh_configs
                    WHERE enabled = TRUE
                      AND maintainer_agent_id = $1
                      AND next_refresh_at <= $2
                    ORDER BY next_refresh_at, project_id
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
                SET last_completed_at = $2,
                    next_refresh_at = $2 + make_interval(mins => interval_minutes),
                    updated_at = $2
                WHERE project_id = $1
                "#,
                &[&project_id, &completed_at],
            )?;
            Ok(())
        })
    }

    fn save_company_codex_runner_profile(
        &self,
        profile: CompanyCodexRunnerProfile,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO company_codex_runner_profiles (
                    id, company_id, name, interval_seconds, codex_profile,
                    model, reasoning_effort, sandbox_mode, approval_policy, max_run_seconds, is_default,
                    created_by_human_user_id, updated_by_human_user_id,
                    created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
                ON CONFLICT (id) DO UPDATE
                SET name = EXCLUDED.name,
                    interval_seconds = EXCLUDED.interval_seconds,
                    codex_profile = EXCLUDED.codex_profile,
                    model = EXCLUDED.model,
                    reasoning_effort = EXCLUDED.reasoning_effort,
                    sandbox_mode = EXCLUDED.sandbox_mode,
                    approval_policy = EXCLUDED.approval_policy,
                    max_run_seconds = EXCLUDED.max_run_seconds,
                    is_default = EXCLUDED.is_default,
                    updated_by_human_user_id = EXCLUDED.updated_by_human_user_id,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &profile.id,
                    &profile.company_id,
                    &profile.name,
                    &profile.interval_seconds,
                    &profile.codex_profile,
                    &profile.model,
                    &profile.reasoning_effort,
                    &profile.sandbox_mode,
                    &profile.approval_policy,
                    &profile.max_run_seconds,
                    &profile.is_default,
                    &profile.created_by_human_user_id,
                    &profile.updated_by_human_user_id,
                    &profile.created_at,
                    &profile.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_company_codex_runner_profile(
        &self,
        profile_id: Uuid,
    ) -> Option<CompanyCodexRunnerProfile> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, name, interval_seconds, codex_profile,
                       model, reasoning_effort, sandbox_mode, approval_policy, max_run_seconds, is_default,
                       created_by_human_user_id, updated_by_human_user_id,
                       created_at, updated_at
                FROM company_codex_runner_profiles
                WHERE id = $1
                "#,
                &[&profile_id],
            )
        })
        .ok()
        .flatten()
        .map(map_company_codex_runner_profile)
    }

    fn list_company_codex_runner_profiles(
        &self,
        company_id: Uuid,
    ) -> Vec<CompanyCodexRunnerProfile> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, company_id, name, interval_seconds, codex_profile,
                       model, reasoning_effort, sandbox_mode, approval_policy, max_run_seconds, is_default,
                       created_by_human_user_id, updated_by_human_user_id,
                       created_at, updated_at
                FROM company_codex_runner_profiles
                WHERE company_id = $1
                ORDER BY is_default DESC, name, id
                "#,
                &[&company_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_company_codex_runner_profile)
        .collect()
    }

    fn delete_company_codex_runner_profile(&self, profile_id: Uuid) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                "DELETE FROM company_codex_runner_profiles WHERE id = $1",
                &[&profile_id],
            )?;
            Ok(())
        })
    }

    fn clear_company_codex_runner_profile_defaults(
        &self,
        company_id: Uuid,
        except_profile_id: Uuid,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE company_codex_runner_profiles
                SET is_default = FALSE, updated_at = NOW()
                WHERE company_id = $1 AND id <> $2 AND is_default
                "#,
                &[&company_id, &except_profile_id],
            )?;
            Ok(())
        })
    }

    fn assign_agent_codex_runner_profile(
        &self,
        agent_id: Uuid,
        profile_id: Uuid,
        human_user_id: Uuid,
        assigned_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_codex_runner_profile_assignments (
                    agent_profile_id, runner_profile_id,
                    assigned_by_human_user_id, assigned_at
                )
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (agent_profile_id) DO UPDATE
                SET runner_profile_id = EXCLUDED.runner_profile_id,
                    assigned_by_human_user_id = EXCLUDED.assigned_by_human_user_id,
                    assigned_at = EXCLUDED.assigned_at
                "#,
                &[&agent_id, &profile_id, &human_user_id, &assigned_at],
            )?;
            Ok(())
        })
    }

    fn get_agent_codex_runner_profile_assignment(&self, agent_id: Uuid) -> Option<Uuid> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT runner_profile_id
                FROM agent_codex_runner_profile_assignments
                WHERE agent_profile_id = $1
                "#,
                &[&agent_id],
            )
        })
        .ok()
        .flatten()
        .map(|row| row.get("runner_profile_id"))
    }

    fn list_codex_runner_profile_agent_ids(&self, profile_id: Uuid) -> Vec<Uuid> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT agent_profile_id
                FROM agent_codex_runner_profile_assignments
                WHERE runner_profile_id = $1
                ORDER BY agent_profile_id
                "#,
                &[&profile_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(|row| row.get("agent_profile_id"))
        .collect()
    }

    fn save_agent_codex_trigger_config(&self, config: AgentCodexTriggerConfig) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_codex_trigger_configs (
                    id, company_id, agent_profile_id, status, interval_seconds,
                    codex_profile, model, reasoning_effort, sandbox_mode, approval_policy, max_run_seconds, next_run_at,
                    lease_owner, lease_expires_at, manual_run_requested_at,
                    last_run_at, last_success_at, last_error,
                    consecutive_failure_count, created_by_human_user_id,
                    updated_by_human_user_id, created_at, updated_at,
                    wake_requested_at, wake_reason
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                        $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22,
                        $23, $24, $25)
                ON CONFLICT (agent_profile_id) DO UPDATE
                SET company_id = EXCLUDED.company_id,
                    status = EXCLUDED.status,
                    interval_seconds = EXCLUDED.interval_seconds,
                    codex_profile = EXCLUDED.codex_profile,
                    model = EXCLUDED.model,
                    reasoning_effort = EXCLUDED.reasoning_effort,
                    sandbox_mode = EXCLUDED.sandbox_mode,
                    approval_policy = EXCLUDED.approval_policy,
                    max_run_seconds = EXCLUDED.max_run_seconds,
                    next_run_at = EXCLUDED.next_run_at,
                    lease_owner = EXCLUDED.lease_owner,
                    lease_expires_at = EXCLUDED.lease_expires_at,
                    manual_run_requested_at = EXCLUDED.manual_run_requested_at,
                    last_run_at = EXCLUDED.last_run_at,
                    last_success_at = EXCLUDED.last_success_at,
                    last_error = EXCLUDED.last_error,
                    consecutive_failure_count = EXCLUDED.consecutive_failure_count,
                    wake_requested_at = EXCLUDED.wake_requested_at,
                    wake_reason = EXCLUDED.wake_reason,
                    updated_by_human_user_id = EXCLUDED.updated_by_human_user_id,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &config.id,
                    &config.company_id,
                    &config.agent_profile_id,
                    &config.status,
                    &config.interval_seconds,
                    &config.codex_profile,
                    &config.model,
                    &config.reasoning_effort,
                    &config.sandbox_mode,
                    &config.approval_policy,
                    &config.max_run_seconds,
                    &config.next_run_at,
                    &config.lease_owner,
                    &config.lease_expires_at,
                    &config.manual_run_requested_at,
                    &config.last_run_at,
                    &config.last_success_at,
                    &config.last_error,
                    &config.consecutive_failure_count,
                    &config.created_by_human_user_id,
                    &config.updated_by_human_user_id,
                    &config.created_at,
                    &config.updated_at,
                    &config.wake_requested_at,
                    &config.wake_reason,
                ],
            )?;
            Ok(())
        })
    }

    fn get_agent_codex_trigger_config_by_agent(
        &self,
        agent_id: Uuid,
    ) -> Option<AgentCodexTriggerConfig> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, agent_profile_id, status, interval_seconds,
                       codex_profile, model, reasoning_effort, sandbox_mode, approval_policy, max_run_seconds, next_run_at,
                       lease_owner, lease_expires_at, manual_run_requested_at,
                       wake_requested_at, wake_reason,
                       last_run_at, last_success_at, last_error,
                       consecutive_failure_count, created_by_human_user_id,
                       updated_by_human_user_id, created_at, updated_at
                FROM agent_codex_trigger_configs
                WHERE agent_profile_id = $1
                "#,
                &[&agent_id],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_codex_trigger_config)
    }

    fn claim_due_agent_codex_trigger_configs(
        &self,
        lease_owner: &str,
        now: chrono::DateTime<chrono::Utc>,
        limit: usize,
    ) -> AppResult<Vec<AgentCodexTriggerConfig>> {
        self.with_client(|client| {
            client.query(
                r#"
                WITH stale_runs AS (
                    UPDATE agent_codex_trigger_runs run
                    SET status = 'timed_out',
                        finished_at = $2,
                        error_message = COALESCE(
                            run.error_message,
                            'Codex trigger run exceeded its execution lease'
                        )
                    FROM agent_codex_trigger_configs stale_config
                    WHERE run.trigger_config_id = stale_config.id
                      AND run.status = 'running'
                      AND run.started_at
                          + make_interval(secs => stale_config.max_run_seconds + 60) <= $2
                    RETURNING run.id
                ),
                due AS (
                    SELECT config.id
                    FROM agent_codex_trigger_configs config
                    WHERE config.status = 'active'
                      AND config.next_run_at <= $2
                      AND (config.lease_expires_at IS NULL OR config.lease_expires_at <= $2)
                      AND EXISTS (
                          SELECT 1
                          FROM company_human_members human_member
                          INNER JOIN human_sessions human_session
                              ON human_session.human_user_id = human_member.human_user_id
                          WHERE human_member.company_id = config.company_id
                            AND human_member.status = 'active'
                            AND human_session.revoked_at IS NULL
                            AND human_session.expires_at > $2
                            AND COALESCE(human_session.last_used_at, human_session.created_at)
                                > $2 - INTERVAL '60 seconds'
                      )
                      AND NOT EXISTS (
                          SELECT 1
                          FROM agent_codex_trigger_runs active_run
                          WHERE active_run.agent_profile_id = config.agent_profile_id
                            AND active_run.status = 'running'
                            AND active_run.started_at
                                + make_interval(secs => config.max_run_seconds + 60) > $2
                      )
                    ORDER BY config.next_run_at, config.id
                    FOR UPDATE SKIP LOCKED
                    LIMIT $3
                )
                UPDATE agent_codex_trigger_configs config
                SET lease_owner = $1,
                    lease_expires_at = $2 + make_interval(secs => config.max_run_seconds + 60),
                    last_run_at = $2,
                    updated_at = $2
                FROM due
                WHERE config.id = due.id
                RETURNING config.id, config.company_id, config.agent_profile_id,
                          config.status, config.interval_seconds, config.codex_profile,
                          config.model, config.reasoning_effort, config.sandbox_mode, config.approval_policy,
                          config.max_run_seconds, config.next_run_at,
                          config.lease_owner, config.lease_expires_at,
                          config.manual_run_requested_at,
                          config.wake_requested_at, config.wake_reason, config.last_run_at,
                          config.last_success_at, config.last_error,
                          config.consecutive_failure_count,
                          config.created_by_human_user_id,
                          config.updated_by_human_user_id, config.created_at,
                          config.updated_at
                "#,
                &[&lease_owner, &now, &(limit as i64)],
            )
        })
        .map(|rows| {
            rows.into_iter()
                .map(map_agent_codex_trigger_config)
                .collect()
        })
    }

    fn request_agent_codex_trigger_wake(
        &self,
        agent_id: Uuid,
        requested_at: chrono::DateTime<chrono::Utc>,
        reason: &str,
    ) -> AppResult<bool> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_codex_trigger_configs
                SET next_run_at = LEAST(next_run_at, $2),
                    wake_requested_at = GREATEST(wake_requested_at, $2),
                    wake_reason = $3,
                    updated_at = $2
                WHERE agent_profile_id = $1
                  AND status = 'active'
                "#,
                &[&agent_id, &requested_at, &reason],
            )
        })
        .map(|updated| updated > 0)
    }

    fn complete_agent_codex_trigger_lease(
        &self,
        input: CompleteAgentCodexTriggerLeaseInput,
    ) -> AppResult<()> {
        let updated = self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_codex_trigger_configs
                SET status = CASE
                        WHEN NOT $5 AND consecutive_failure_count + 1 >= 3 AND status = 'active'
                            THEN 'error'
                        ELSE status
                    END,
                    next_run_at = CASE
                        WHEN (manual_run_requested_at IS NOT NULL
                              AND last_run_at IS NOT NULL
                              AND manual_run_requested_at > last_run_at)
                          OR (wake_requested_at IS NOT NULL
                              AND last_run_at IS NOT NULL
                              AND wake_requested_at > last_run_at)
                            THEN LEAST(
                                COALESCE(
                                    CASE
                                        WHEN manual_run_requested_at > last_run_at
                                            THEN manual_run_requested_at
                                    END,
                                    $4
                                ),
                                COALESCE(
                                    CASE
                                        WHEN wake_requested_at > last_run_at
                                            THEN wake_requested_at
                                    END,
                                    $4
                                ),
                                $4
                            )
                        ELSE $4
                    END,
                    lease_owner = NULL,
                    lease_expires_at = NULL,
                    manual_run_requested_at = CASE
                        WHEN manual_run_requested_at IS NOT NULL
                         AND last_run_at IS NOT NULL
                         AND manual_run_requested_at > last_run_at
                            THEN manual_run_requested_at
                        ELSE NULL
                    END,
                    wake_requested_at = CASE
                        WHEN wake_requested_at IS NOT NULL
                         AND last_run_at IS NOT NULL
                         AND wake_requested_at > last_run_at
                            THEN wake_requested_at
                        ELSE NULL
                    END,
                    wake_reason = CASE
                        WHEN wake_requested_at IS NOT NULL
                         AND last_run_at IS NOT NULL
                         AND wake_requested_at > last_run_at
                            THEN wake_reason
                        ELSE NULL
                    END,
                    last_run_at = $3,
                    last_success_at = CASE WHEN $5 THEN $3 ELSE last_success_at END,
                    last_error = CASE WHEN $5 THEN NULL ELSE $6 END,
                    consecutive_failure_count = CASE
                        WHEN $5 THEN 0
                        ELSE consecutive_failure_count + 1
                    END,
                    updated_at = $3
                WHERE id = $1 AND lease_owner = $2
                "#,
                &[
                    &input.trigger_config_id,
                    &input.lease_owner,
                    &input.finished_at,
                    &input.next_run_at,
                    &input.succeeded,
                    &input.error_message,
                ],
            )
        })?;
        if updated == 0 {
            return Err(AppError::Conflict("Codex trigger lease was lost".into()));
        }
        Ok(())
    }

    fn insert_agent_codex_trigger_run(&self, run: AgentCodexTriggerRun) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_codex_trigger_runs (
                    id, trigger_config_id, agent_profile_id, project_id,
                    trigger_type, status, codex_thread_id, codex_version,
                    exit_code, started_at, finished_at, final_message_summary,
                    error_message, activity_phase, activity_summary,
                    last_activity_at, activity_log
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                        $14, $15, $16, $17)
                "#,
                &[
                    &run.id,
                    &run.trigger_config_id,
                    &run.agent_profile_id,
                    &run.project_id,
                    &run.trigger_type,
                    &run.status,
                    &run.codex_thread_id,
                    &run.codex_version,
                    &run.exit_code,
                    &run.started_at,
                    &run.finished_at,
                    &run.final_message_summary,
                    &run.error_message,
                    &run.activity_phase,
                    &run.activity_summary,
                    &run.last_activity_at,
                    &Json(&run.activity_log),
                ],
            )?;
            Ok(())
        })
    }

    fn has_running_agent_codex_trigger_run(&self, agent_id: Uuid) -> bool {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT 1
                FROM agent_codex_trigger_runs
                WHERE agent_profile_id = $1 AND status = 'running'
                LIMIT 1
                "#,
                &[&agent_id],
            )
        })
        .ok()
        .flatten()
        .is_some()
    }

    fn update_agent_codex_trigger_run(&self, run: AgentCodexTriggerRun) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_codex_trigger_runs
                SET project_id = $2,
                    trigger_type = $3,
                    status = $4,
                    codex_thread_id = $5,
                    codex_version = $6,
                    exit_code = $7,
                    finished_at = $8,
                    final_message_summary = $9,
                    error_message = $10
                WHERE id = $1
                "#,
                &[
                    &run.id,
                    &run.project_id,
                    &run.trigger_type,
                    &run.status,
                    &run.codex_thread_id,
                    &run.codex_version,
                    &run.exit_code,
                    &run.finished_at,
                    &run.final_message_summary,
                    &run.error_message,
                ],
            )?;
            Ok(())
        })
    }

    fn append_agent_codex_trigger_run_activity(
        &self,
        run_id: Uuid,
        activity: AgentCodexRunActivity,
        codex_thread_id: Option<String>,
    ) -> AppResult<()> {
        let updated = self.with_client(|client| {
            let mut tx = client.transaction()?;
            let Some(row) = tx.query_opt(
                "SELECT activity_log FROM agent_codex_trigger_runs WHERE id = $1 FOR UPDATE",
                &[&run_id],
            )?
            else {
                return Ok(false);
            };
            let Json(mut activity_log): Json<Vec<AgentCodexRunActivity>> = row.get("activity_log");
            activity_log.push(activity.clone());
            if activity_log.len() > 40 {
                let excess = activity_log.len() - 40;
                activity_log.drain(0..excess);
            }
            tx.execute(
                r#"
                UPDATE agent_codex_trigger_runs
                SET activity_phase = $2,
                    activity_summary = $3,
                    last_activity_at = $4,
                    codex_thread_id = COALESCE($5, codex_thread_id),
                    activity_log = $6
                WHERE id = $1
                "#,
                &[
                    &run_id,
                    &activity.phase,
                    &activity.summary,
                    &activity.at,
                    &codex_thread_id,
                    &Json(&activity_log),
                ],
            )?;
            tx.commit()?;
            Ok(true)
        })?;
        if !updated {
            return Err(AppError::NotFound("Codex trigger run not found".into()));
        }
        Ok(())
    }

    fn list_agent_codex_trigger_runs(
        &self,
        agent_id: Uuid,
        limit: usize,
    ) -> Vec<AgentCodexTriggerRun> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, trigger_config_id, agent_profile_id, project_id,
                       trigger_type, status, codex_thread_id, codex_version,
                       exit_code, started_at, finished_at, final_message_summary,
                       error_message, activity_phase, activity_summary,
                       last_activity_at, activity_log
                FROM agent_codex_trigger_runs
                WHERE agent_profile_id = $1
                ORDER BY started_at DESC, id DESC
                LIMIT $2
                "#,
                &[&agent_id, &(limit as i64)],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_codex_trigger_run)
        .collect()
    }

    fn save_agent_codex_session(&self, session: AgentCodexSession) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_codex_sessions (
                    agent_profile_id, current_project_id, codex_thread_id,
                    worktree_key, last_used_at
                )
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (agent_profile_id) DO UPDATE
                SET current_project_id = EXCLUDED.current_project_id,
                    codex_thread_id = EXCLUDED.codex_thread_id,
                    worktree_key = EXCLUDED.worktree_key,
                    last_used_at = EXCLUDED.last_used_at
                "#,
                &[
                    &session.agent_profile_id,
                    &session.current_project_id,
                    &session.codex_thread_id,
                    &session.worktree_key,
                    &session.last_used_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_agent_codex_session(&self, agent_id: Uuid) -> Option<AgentCodexSession> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT agent_profile_id, current_project_id, codex_thread_id,
                       worktree_key, last_used_at
                FROM agent_codex_sessions
                WHERE agent_profile_id = $1
                "#,
                &[&agent_id],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_codex_session)
    }

    fn insert_agent_codex_run_token(&self, token: AgentCodexRunToken) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_codex_run_tokens (
                    id, run_id, agent_profile_id, token_hash,
                    expires_at, revoked_at, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
                &[
                    &token.id,
                    &token.run_id,
                    &token.agent_profile_id,
                    &token.token_hash,
                    &token.expires_at,
                    &token.revoked_at,
                    &token.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn find_agent_codex_run_token_by_hash(&self, token_hash: &str) -> Option<AgentCodexRunToken> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, run_id, agent_profile_id, token_hash,
                       expires_at, revoked_at, created_at
                FROM agent_codex_run_tokens
                WHERE token_hash = $1
                "#,
                &[&token_hash],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_codex_run_token)
    }

    fn revoke_agent_codex_run_tokens(
        &self,
        run_id: Uuid,
        revoked_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_codex_run_tokens
                SET revoked_at = $2
                WHERE run_id = $1 AND revoked_at IS NULL
                "#,
                &[&run_id, &revoked_at],
            )?;
            Ok(())
        })
    }

    fn delete_expired_agent_codex_run_tokens(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<usize> {
        self.with_client(|client| {
            client
                .execute(
                    r#"
                    DELETE FROM agent_codex_run_tokens
                    WHERE expires_at <= $1 OR revoked_at IS NOT NULL
                    "#,
                    &[&now],
                )
                .map(|count| count as usize)
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

    fn insert_company_project_task(&self, task: CompanyProjectTask) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                INSERT INTO company_project_tasks (
                    id, project_id, title, description, status, priority,
                    assignee_agent_id, created_by_agent_id, created_by_human_user_id,
                    updated_by_agent_id, updated_by_human_user_id, due_at,
                    completed_at, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
                "#,
                &[
                    &task.id,
                    &task.project_id,
                    &task.title,
                    &task.description,
                    &task.status,
                    &task.priority,
                    &task.assignee_agent_id,
                    &task.created_by_agent_id,
                    &task.created_by_human_user_id,
                    &task.updated_by_agent_id,
                    &task.updated_by_human_user_id,
                    &task.due_at,
                    &task.completed_at,
                    &task.created_at,
                    &task.updated_at,
                ],
            )?;
            let change_source = if task.created_by_agent_id.is_some() {
                "agent"
            } else if task.created_by_human_user_id.is_some() {
                "human"
            } else {
                "system"
            };
            tx.execute(
                r#"
                INSERT INTO company_project_task_status_history (
                    id, project_id, task_id, from_status, to_status,
                    changed_by_agent_id, changed_by_human_user_id,
                    change_source, metadata, created_at
                ) VALUES ($1, $2, $3, NULL, $4, $5, $6, $7, '{}'::jsonb, $8)
                "#,
                &[
                    &Uuid::new_v4(),
                    &task.project_id,
                    &task.id,
                    &task.status,
                    &task.created_by_agent_id,
                    &task.created_by_human_user_id,
                    &change_source,
                    &task.created_at,
                ],
            )?;
            tx.execute(
                "UPDATE company_projects SET updated_at = $2, updated_by_agent_id = $3 WHERE id = $1",
                &[&task.project_id, &task.updated_at, &task.created_by_agent_id],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn get_company_project_task(&self, task_id: Uuid) -> Option<CompanyProjectTask> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, project_id, title, description, status, priority,
                       assignee_agent_id, created_by_agent_id, created_by_human_user_id,
                       updated_by_agent_id, updated_by_human_user_id,
                       due_at, completed_at, created_at, updated_at
                FROM company_project_tasks
                WHERE id = $1
                "#,
                &[&task_id],
            )
        })
        .ok()
        .flatten()
        .map(map_company_project_task)
    }

    fn list_company_project_tasks(&self, project_id: Uuid) -> Vec<CompanyProjectTask> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, project_id, title, description, status, priority,
                       assignee_agent_id, created_by_agent_id, created_by_human_user_id,
                       updated_by_agent_id, updated_by_human_user_id,
                       due_at, completed_at, created_at, updated_at
                FROM company_project_tasks
                WHERE project_id = $1
                ORDER BY updated_at DESC, created_at DESC
                "#,
                &[&project_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_company_project_task)
        .collect()
    }

    fn update_company_project_task(&self, task: CompanyProjectTask) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            let previous_status = tx
                .query_opt(
                    "SELECT status FROM company_project_tasks WHERE id = $1 FOR UPDATE",
                    &[&task.id],
                )?
                .map(|row| row.get::<_, String>("status"));
            if previous_status.as_deref() != Some(task.status.as_str()) {
                let change_source = if task.updated_by_agent_id.is_some() {
                    "agent"
                } else if task.updated_by_human_user_id.is_some() {
                    "human"
                } else {
                    "system"
                };
                tx.execute(
                    r#"
                    INSERT INTO company_project_task_status_history (
                        id, project_id, task_id, from_status, to_status,
                        changed_by_agent_id, changed_by_human_user_id,
                        change_source, metadata, created_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, '{}'::jsonb, $9)
                    "#,
                    &[
                        &Uuid::new_v4(),
                        &task.project_id,
                        &task.id,
                        &previous_status,
                        &task.status,
                        &task.updated_by_agent_id,
                        &task.updated_by_human_user_id,
                        &change_source,
                        &task.updated_at,
                    ],
                )?;
            }
            tx.execute(
                r#"
                UPDATE company_project_tasks
                SET title = $2,
                    description = $3,
                    status = $4,
                    priority = $5,
                    assignee_agent_id = $6,
                    updated_by_agent_id = $7,
                    updated_by_human_user_id = $8,
                    due_at = $9,
                    completed_at = $10,
                    updated_at = $11
                WHERE id = $1
                "#,
                &[
                    &task.id,
                    &task.title,
                    &task.description,
                    &task.status,
                    &task.priority,
                    &task.assignee_agent_id,
                    &task.updated_by_agent_id,
                    &task.updated_by_human_user_id,
                    &task.due_at,
                    &task.completed_at,
                    &task.updated_at,
                ],
            )?;
            tx.execute(
                "UPDATE company_projects SET updated_at = $2, updated_by_agent_id = $3 WHERE id = $1",
                &[&task.project_id, &task.updated_at, &task.updated_by_agent_id],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn update_company_project_tasks(&self, tasks: Vec<CompanyProjectTask>) -> AppResult<()> {
        if tasks.is_empty() {
            return Ok(());
        }
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            for task in &tasks {
                let previous_status = tx
                    .query_opt(
                        "SELECT status FROM company_project_tasks WHERE id = $1 FOR UPDATE",
                        &[&task.id],
                    )?
                    .map(|row| row.get::<_, String>("status"));
                if previous_status.as_deref() != Some(task.status.as_str()) {
                    let change_source = if task.updated_by_agent_id.is_some() {
                        "agent"
                    } else if task.updated_by_human_user_id.is_some() {
                        "human"
                    } else {
                        "system"
                    };
                    tx.execute(
                        r#"
                        INSERT INTO company_project_task_status_history (
                            id, project_id, task_id, from_status, to_status,
                            changed_by_agent_id, changed_by_human_user_id,
                            change_source, metadata, created_at
                        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, '{}'::jsonb, $9)
                        "#,
                        &[
                            &Uuid::new_v4(),
                            &task.project_id,
                            &task.id,
                            &previous_status,
                            &task.status,
                            &task.updated_by_agent_id,
                            &task.updated_by_human_user_id,
                            &change_source,
                            &task.updated_at,
                        ],
                    )?;
                }
                tx.execute(
                    r#"
                    UPDATE company_project_tasks
                    SET title = $2,
                        description = $3,
                        status = $4,
                        priority = $5,
                        assignee_agent_id = $6,
                        updated_by_agent_id = $7,
                        updated_by_human_user_id = $8,
                        due_at = $9,
                        completed_at = $10,
                        updated_at = $11
                    WHERE id = $1
                    "#,
                    &[
                        &task.id,
                        &task.title,
                        &task.description,
                        &task.status,
                        &task.priority,
                        &task.assignee_agent_id,
                        &task.updated_by_agent_id,
                        &task.updated_by_human_user_id,
                        &task.due_at,
                        &task.completed_at,
                        &task.updated_at,
                    ],
                )?;
            }
            let latest = &tasks[0];
            tx.execute(
                "UPDATE company_projects SET updated_at = $2, updated_by_agent_id = $3 WHERE id = $1",
                &[
                    &latest.project_id,
                    &latest.updated_at,
                    &latest.updated_by_agent_id,
                ],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn insert_company_project_task_dependency(
        &self,
        dependency: CompanyProjectTaskDependency,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                INSERT INTO company_project_task_dependencies (
                    id, project_id, task_id, depends_on_task_id,
                    created_by_agent_id, created_by_human_user_id, created_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
                &[
                    &dependency.id,
                    &dependency.project_id,
                    &dependency.task_id,
                    &dependency.depends_on_task_id,
                    &dependency.created_by_agent_id,
                    &dependency.created_by_human_user_id,
                    &dependency.created_at,
                ],
            )?;
            tx.execute(
                "UPDATE company_projects SET updated_at = $2, updated_by_agent_id = $3 WHERE id = $1",
                &[
                    &dependency.project_id,
                    &dependency.created_at,
                    &dependency.created_by_agent_id,
                ],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn remove_company_project_task_dependency(
        &self,
        project_id: Uuid,
        task_id: Uuid,
        depends_on_task_id: Uuid,
        removed_by_agent_id: Option<Uuid>,
        removed_by_human_user_id: Option<Uuid>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                UPDATE company_project_task_dependencies
                SET created_by_agent_id = $4,
                    created_by_human_user_id = $5
                WHERE project_id = $1 AND task_id = $2 AND depends_on_task_id = $3
                "#,
                &[
                    &project_id,
                    &task_id,
                    &depends_on_task_id,
                    &removed_by_agent_id,
                    &removed_by_human_user_id,
                ],
            )?;
            tx.execute(
                r#"
                DELETE FROM company_project_task_dependencies
                WHERE project_id = $1 AND task_id = $2 AND depends_on_task_id = $3
                "#,
                &[&project_id, &task_id, &depends_on_task_id],
            )?;
            let now = Utc::now();
            tx.execute(
                "UPDATE company_projects SET updated_at = $2, updated_by_agent_id = $3 WHERE id = $1",
                &[&project_id, &now, &removed_by_agent_id],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn list_company_project_task_dependencies(
        &self,
        project_id: Uuid,
    ) -> Vec<CompanyProjectTaskDependency> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, project_id, task_id, depends_on_task_id,
                       created_by_agent_id, created_by_human_user_id, created_at
                FROM company_project_task_dependencies
                WHERE project_id = $1
                ORDER BY created_at, id
                "#,
                &[&project_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_company_project_task_dependency)
        .collect()
    }

    fn list_company_project_task_status_history(
        &self,
        project_id: Uuid,
    ) -> Vec<CompanyProjectTaskStatusHistory> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, project_id, task_id, from_status, to_status,
                       changed_by_agent_id, changed_by_human_user_id,
                       change_source, metadata, created_at
                FROM company_project_task_status_history
                WHERE project_id = $1
                ORDER BY created_at DESC, id DESC
                "#,
                &[&project_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_company_project_task_status_history)
        .collect()
    }

    fn insert_company_project_status_update(
        &self,
        update: CompanyProjectStatusUpdate,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let blockers = Json(&update.blockers);
            let next_steps = Json(&update.next_steps);
            client.execute(
                r#"
                INSERT INTO company_project_status_updates (
                    id, project_id, author_agent_id, summary, progress_percent,
                    blockers, next_steps, project_status, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
                &[
                    &update.id,
                    &update.project_id,
                    &update.author_agent_id,
                    &update.summary,
                    &update.progress_percent,
                    &blockers,
                    &next_steps,
                    &update.project_status,
                    &update.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_company_project_status_updates(
        &self,
        project_id: Uuid,
    ) -> Vec<CompanyProjectStatusUpdate> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, project_id, author_agent_id, summary, progress_percent,
                       blockers, next_steps, project_status, created_at
                FROM company_project_status_updates
                WHERE project_id = $1
                ORDER BY created_at DESC
                "#,
                &[&project_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_company_project_status_update)
        .collect()
    }

    fn list_company_realtime_events(
        &self,
        company_id: Uuid,
        after_sequence_id: i64,
        limit: usize,
    ) -> Vec<CompanyRealtimeEvent> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT sequence_id, id, company_id, event_type, aggregate_type,
                       aggregate_id, actor_agent_id, actor_human_user_id,
                       payload, created_at
                FROM realtime_events
                WHERE company_id = $1
                  AND sequence_id > $2
                ORDER BY sequence_id
                LIMIT $3
                "#,
                &[&company_id, &after_sequence_id, &(limit as i64)],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_company_realtime_event)
        .collect()
    }

    fn latest_company_realtime_sequence(&self, company_id: Uuid) -> i64 {
        self.with_client(|client| {
            client.query_one(
                "SELECT COALESCE(MAX(sequence_id), 0)::BIGINT AS sequence_id FROM realtime_events WHERE company_id = $1",
                &[&company_id],
            )
        })
        .map(|row| row.get("sequence_id"))
        .unwrap_or(0)
    }

    fn save_agent_runtime_config(&self, config: AgentRuntimeConfig) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_runtime_configs (
                    id, company_id, agent_profile_id, runtime_template_id, executor_kind, status,
                    interval_seconds, max_events, daily_run_budget, daily_action_budget,
                    daily_model_input_token_budget, daily_model_output_token_budget,
                    daily_model_cost_budget_microusd,
                    model_input_price_microusd_per_million_tokens,
                    model_output_price_microusd_per_million_tokens,
                    daily_approval_request_budget,
                    company_message_policy, auto_start_assigned_tasks,
                    auto_announce_project_membership, context_max_projects,
                    context_max_conversations, context_max_inbox_events,
                    system_prompt, model_name, model_provider, provider_secret_ref,
                    allowed_model_actions, approval_required_model_actions,
                    approval_request_ttl_minutes, model_max_output_tokens, model_timeout_seconds,
                    model_max_retries, fallback_to_rules_v1, next_run_at,
                    last_run_at, last_success_at, last_error_at, last_error,
                    created_by_human_user_id, updated_by_human_user_id, created_at, updated_at,
                    model_price_catalog_entry_id
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                    $12, $13, $14, $15, $16, $17, $18, $19, $20, $21,
                    $22, $23, $24, $25, $26, $27, $28, $29, $30, $31,
                    $32, $33, $34, $35, $36, $37, $38, $39, $40,
                    $41, $42, $43
                )
                ON CONFLICT (id) DO UPDATE
                SET executor_kind = EXCLUDED.executor_kind,
                    status = EXCLUDED.status,
                    runtime_template_id = EXCLUDED.runtime_template_id,
                    model_price_catalog_entry_id = EXCLUDED.model_price_catalog_entry_id,
                    interval_seconds = EXCLUDED.interval_seconds,
                    max_events = EXCLUDED.max_events,
                    daily_run_budget = EXCLUDED.daily_run_budget,
                    daily_action_budget = EXCLUDED.daily_action_budget,
                    daily_model_input_token_budget = EXCLUDED.daily_model_input_token_budget,
                    daily_model_output_token_budget = EXCLUDED.daily_model_output_token_budget,
                    daily_model_cost_budget_microusd = EXCLUDED.daily_model_cost_budget_microusd,
                    model_input_price_microusd_per_million_tokens = EXCLUDED.model_input_price_microusd_per_million_tokens,
                    model_output_price_microusd_per_million_tokens = EXCLUDED.model_output_price_microusd_per_million_tokens,
                    daily_approval_request_budget = EXCLUDED.daily_approval_request_budget,
                    company_message_policy = EXCLUDED.company_message_policy,
                    auto_start_assigned_tasks = EXCLUDED.auto_start_assigned_tasks,
                    auto_announce_project_membership = EXCLUDED.auto_announce_project_membership,
                    context_max_projects = EXCLUDED.context_max_projects,
                    context_max_conversations = EXCLUDED.context_max_conversations,
                    context_max_inbox_events = EXCLUDED.context_max_inbox_events,
                    system_prompt = EXCLUDED.system_prompt,
                    model_name = EXCLUDED.model_name,
                    model_provider = EXCLUDED.model_provider,
                    provider_secret_ref = EXCLUDED.provider_secret_ref,
                    allowed_model_actions = EXCLUDED.allowed_model_actions,
                    approval_required_model_actions = EXCLUDED.approval_required_model_actions,
                    approval_request_ttl_minutes = EXCLUDED.approval_request_ttl_minutes,
                    model_max_output_tokens = EXCLUDED.model_max_output_tokens,
                    model_timeout_seconds = EXCLUDED.model_timeout_seconds,
                    model_max_retries = EXCLUDED.model_max_retries,
                    fallback_to_rules_v1 = EXCLUDED.fallback_to_rules_v1,
                    next_run_at = EXCLUDED.next_run_at,
                    last_run_at = EXCLUDED.last_run_at,
                    last_success_at = EXCLUDED.last_success_at,
                    last_error_at = EXCLUDED.last_error_at,
                    last_error = EXCLUDED.last_error,
                    updated_by_human_user_id = EXCLUDED.updated_by_human_user_id,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &config.id,
                    &config.company_id,
                    &config.agent_profile_id,
                    &config.runtime_template_id,
                    &config.executor_kind,
                    &config.status,
                    &config.interval_seconds,
                    &config.max_events,
                    &config.daily_run_budget,
                    &config.daily_action_budget,
                    &config.daily_model_input_token_budget,
                    &config.daily_model_output_token_budget,
                    &config.daily_model_cost_budget_microusd,
                    &config.model_input_price_microusd_per_million_tokens,
                    &config.model_output_price_microusd_per_million_tokens,
                    &config.daily_approval_request_budget,
                    &config.company_message_policy,
                    &config.auto_start_assigned_tasks,
                    &config.auto_announce_project_membership,
                    &config.context_max_projects,
                    &config.context_max_conversations,
                    &config.context_max_inbox_events,
                    &config.system_prompt,
                    &config.model_name,
                    &config.model_provider,
                    &config.provider_secret_ref,
                    &Json(config.allowed_model_actions.clone()),
                    &Json(config.approval_required_model_actions.clone()),
                    &config.approval_request_ttl_minutes,
                    &config.model_max_output_tokens,
                    &config.model_timeout_seconds,
                    &config.model_max_retries,
                    &config.fallback_to_rules_v1,
                    &config.next_run_at,
                    &config.last_run_at,
                    &config.last_success_at,
                    &config.last_error_at,
                    &config.last_error,
                    &config.created_by_human_user_id,
                    &config.updated_by_human_user_id,
                    &config.created_at,
                    &config.updated_at,
                    &config.model_price_catalog_entry_id,
                ],
            )?;
            Ok(())
        })
    }

    fn get_agent_runtime_config(&self, runtime_config_id: Uuid) -> Option<AgentRuntimeConfig> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, agent_profile_id, runtime_template_id, executor_kind, status,
                       interval_seconds, max_events, daily_run_budget, daily_action_budget,
                       daily_model_input_token_budget, daily_model_output_token_budget,
                       daily_model_cost_budget_microusd,
                       model_input_price_microusd_per_million_tokens,
                       model_output_price_microusd_per_million_tokens,
                       daily_approval_request_budget,
                       company_message_policy, auto_start_assigned_tasks,
                       auto_announce_project_membership, context_max_projects,
                       context_max_conversations, context_max_inbox_events,
                       system_prompt, model_name, model_provider, provider_secret_ref,
                       allowed_model_actions, approval_required_model_actions,
                       approval_request_ttl_minutes, model_max_output_tokens, model_timeout_seconds,
                       model_max_retries, fallback_to_rules_v1, next_run_at,
                       last_run_at, last_success_at, last_error_at, last_error,
                       created_by_human_user_id, updated_by_human_user_id, created_at, updated_at,
                       model_price_catalog_entry_id
                FROM agent_runtime_configs
                WHERE id = $1
                "#,
                &[&runtime_config_id],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_runtime_config)
    }

    fn get_agent_runtime_config_by_agent(&self, agent_id: Uuid) -> Option<AgentRuntimeConfig> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, agent_profile_id, runtime_template_id, executor_kind, status,
                       interval_seconds, max_events, daily_run_budget, daily_action_budget,
                       daily_model_input_token_budget, daily_model_output_token_budget,
                       daily_model_cost_budget_microusd,
                       model_input_price_microusd_per_million_tokens,
                       model_output_price_microusd_per_million_tokens,
                       daily_approval_request_budget,
                       company_message_policy, auto_start_assigned_tasks,
                       auto_announce_project_membership, context_max_projects,
                       context_max_conversations, context_max_inbox_events,
                       system_prompt, model_name, model_provider, provider_secret_ref,
                       allowed_model_actions, approval_required_model_actions,
                       approval_request_ttl_minutes, model_max_output_tokens, model_timeout_seconds,
                       model_max_retries, fallback_to_rules_v1, next_run_at,
                       last_run_at, last_success_at, last_error_at, last_error,
                       created_by_human_user_id, updated_by_human_user_id, created_at, updated_at,
                       model_price_catalog_entry_id
                FROM agent_runtime_configs
                WHERE agent_profile_id = $1
                "#,
                &[&agent_id],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_runtime_config)
    }

    fn list_company_agent_runtime_configs(&self, company_id: Uuid) -> Vec<AgentRuntimeConfig> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, company_id, agent_profile_id, runtime_template_id, executor_kind, status,
                       interval_seconds, max_events, daily_run_budget, daily_action_budget,
                       daily_model_input_token_budget, daily_model_output_token_budget,
                       daily_model_cost_budget_microusd,
                       model_input_price_microusd_per_million_tokens,
                       model_output_price_microusd_per_million_tokens,
                       daily_approval_request_budget,
                       company_message_policy, auto_start_assigned_tasks,
                       auto_announce_project_membership, context_max_projects,
                       context_max_conversations, context_max_inbox_events,
                       system_prompt, model_name, model_provider, provider_secret_ref,
                       allowed_model_actions, approval_required_model_actions,
                       approval_request_ttl_minutes, model_max_output_tokens, model_timeout_seconds,
                       model_max_retries, fallback_to_rules_v1, next_run_at,
                       last_run_at, last_success_at, last_error_at, last_error,
                       created_by_human_user_id, updated_by_human_user_id, created_at, updated_at,
                       model_price_catalog_entry_id
                FROM agent_runtime_configs
                WHERE company_id = $1
                ORDER BY created_at
                "#,
                &[&company_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_runtime_config)
        .collect()
    }

    fn save_agent_runtime_template(&self, template: AgentRuntimeTemplate) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_runtime_templates (
                    id, company_id, name, description, status, settings,
                    source_agent_profile_id, created_by_human_user_id,
                    updated_by_human_user_id, created_at, updated_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                ON CONFLICT (id) DO UPDATE
                SET name = EXCLUDED.name,
                    description = EXCLUDED.description,
                    status = EXCLUDED.status,
                    settings = EXCLUDED.settings,
                    source_agent_profile_id = EXCLUDED.source_agent_profile_id,
                    updated_by_human_user_id = EXCLUDED.updated_by_human_user_id,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &template.id,
                    &template.company_id,
                    &template.name,
                    &template.description,
                    &template.status,
                    &Json(template.settings.clone()),
                    &template.source_agent_profile_id,
                    &template.created_by_human_user_id,
                    &template.updated_by_human_user_id,
                    &template.created_at,
                    &template.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_agent_runtime_template(&self, template_id: Uuid) -> Option<AgentRuntimeTemplate> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, name, description, status, settings,
                       source_agent_profile_id, created_by_human_user_id,
                       updated_by_human_user_id, created_at, updated_at
                FROM agent_runtime_templates
                WHERE id = $1
                "#,
                &[&template_id],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_runtime_template)
    }

    fn list_company_agent_runtime_templates(&self, company_id: Uuid) -> Vec<AgentRuntimeTemplate> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, company_id, name, description, status, settings,
                       source_agent_profile_id, created_by_human_user_id,
                       updated_by_human_user_id, created_at, updated_at
                FROM agent_runtime_templates
                WHERE company_id = $1
                ORDER BY updated_at DESC, name
                "#,
                &[&company_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_runtime_template)
        .collect()
    }

    fn save_company_runtime_policy(&self, policy: CompanyRuntimePolicy) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO company_runtime_policies (
                    company_id, default_runtime_template_id, auto_apply_to_new_agents,
                    created_by_human_user_id, updated_by_human_user_id, created_at, updated_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (company_id) DO UPDATE
                SET default_runtime_template_id = EXCLUDED.default_runtime_template_id,
                    auto_apply_to_new_agents = EXCLUDED.auto_apply_to_new_agents,
                    updated_by_human_user_id = EXCLUDED.updated_by_human_user_id,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &policy.company_id,
                    &policy.default_runtime_template_id,
                    &policy.auto_apply_to_new_agents,
                    &policy.created_by_human_user_id,
                    &policy.updated_by_human_user_id,
                    &policy.created_at,
                    &policy.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_company_runtime_policy(&self, company_id: Uuid) -> Option<CompanyRuntimePolicy> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT company_id, default_runtime_template_id, auto_apply_to_new_agents,
                       created_by_human_user_id, updated_by_human_user_id, created_at, updated_at
                FROM company_runtime_policies
                WHERE company_id = $1
                "#,
                &[&company_id],
            )
        })
        .ok()
        .flatten()
        .map(|row| CompanyRuntimePolicy {
            company_id: row.get("company_id"),
            default_runtime_template_id: row.get("default_runtime_template_id"),
            auto_apply_to_new_agents: row.get("auto_apply_to_new_agents"),
            configured: true,
            created_by_human_user_id: row.get("created_by_human_user_id"),
            updated_by_human_user_id: row.get("updated_by_human_user_id"),
            created_at: Some(row.get("created_at")),
            updated_at: Some(row.get("updated_at")),
        })
    }

    fn publish_agent_model_price_catalog_entry(
        &self,
        mut entry: AgentModelPriceCatalogEntry,
    ) -> AppResult<AgentModelPriceCatalogEntry> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.query_one(
                "SELECT id FROM companies WHERE id = $1 FOR UPDATE",
                &[&entry.company_id],
            )?;
            if let Some(row) = tx.query_opt(
                r#"
                SELECT id, company_id, model_provider, model_name, version, status,
                       input_price_microusd_per_million_tokens,
                       output_price_microusd_per_million_tokens,
                       notes, source_agent_profile_id, created_by_human_user_id,
                       updated_by_human_user_id, created_at, updated_at
                FROM agent_model_price_catalog_entries
                WHERE company_id = $1 AND model_provider = $2 AND model_name = $3
                  AND status = 'active'
                "#,
                &[&entry.company_id, &entry.model_provider, &entry.model_name],
            )? {
                let active_entry = map_agent_model_price_catalog_entry(row);
                if active_entry.input_price_microusd_per_million_tokens
                    == entry.input_price_microusd_per_million_tokens
                    && active_entry.output_price_microusd_per_million_tokens
                        == entry.output_price_microusd_per_million_tokens
                {
                    tx.commit()?;
                    return Ok(active_entry);
                }
            }
            let version: i32 = tx
                .query_one(
                    r#"
                    SELECT COALESCE(MAX(version), 0)::INTEGER + 1 AS next_version
                    FROM agent_model_price_catalog_entries
                    WHERE company_id = $1 AND model_provider = $2 AND model_name = $3
                    "#,
                    &[&entry.company_id, &entry.model_provider, &entry.model_name],
                )?
                .get("next_version");
            tx.execute(
                r#"
                UPDATE agent_model_price_catalog_entries
                SET status = 'archived',
                    updated_by_human_user_id = $4,
                    updated_at = $5
                WHERE company_id = $1 AND model_provider = $2 AND model_name = $3
                  AND status = 'active'
                "#,
                &[
                    &entry.company_id,
                    &entry.model_provider,
                    &entry.model_name,
                    &entry.updated_by_human_user_id,
                    &entry.updated_at,
                ],
            )?;
            entry.version = version;
            tx.execute(
                r#"
                INSERT INTO agent_model_price_catalog_entries (
                    id, company_id, model_provider, model_name, version, status,
                    input_price_microusd_per_million_tokens,
                    output_price_microusd_per_million_tokens,
                    notes, source_agent_profile_id, created_by_human_user_id,
                    updated_by_human_user_id, created_at, updated_at
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7,
                    $8, $9, $10, $11, $12, $13, $14
                )
                "#,
                &[
                    &entry.id,
                    &entry.company_id,
                    &entry.model_provider,
                    &entry.model_name,
                    &entry.version,
                    &entry.status,
                    &entry.input_price_microusd_per_million_tokens,
                    &entry.output_price_microusd_per_million_tokens,
                    &entry.notes,
                    &entry.source_agent_profile_id,
                    &entry.created_by_human_user_id,
                    &entry.updated_by_human_user_id,
                    &entry.created_at,
                    &entry.updated_at,
                ],
            )?;
            tx.commit()?;
            Ok(entry)
        })
    }

    fn save_agent_model_price_catalog_entry(
        &self,
        entry: AgentModelPriceCatalogEntry,
    ) -> AppResult<()> {
        let updated = self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_model_price_catalog_entries
                SET status = $2, notes = $3, updated_by_human_user_id = $4, updated_at = $5
                WHERE id = $1
                "#,
                &[
                    &entry.id,
                    &entry.status,
                    &entry.notes,
                    &entry.updated_by_human_user_id,
                    &entry.updated_at,
                ],
            )
        })?;
        if updated == 0 {
            return Err(AppError::NotFound(
                "model price catalog entry not found".into(),
            ));
        }
        Ok(())
    }

    fn get_agent_model_price_catalog_entry(
        &self,
        entry_id: Uuid,
    ) -> Option<AgentModelPriceCatalogEntry> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, model_provider, model_name, version, status,
                       input_price_microusd_per_million_tokens,
                       output_price_microusd_per_million_tokens,
                       notes, source_agent_profile_id, created_by_human_user_id,
                       updated_by_human_user_id, created_at, updated_at
                FROM agent_model_price_catalog_entries
                WHERE id = $1
                "#,
                &[&entry_id],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_model_price_catalog_entry)
    }

    fn list_company_agent_model_price_catalog_entries(
        &self,
        company_id: Uuid,
    ) -> Vec<AgentModelPriceCatalogEntry> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, company_id, model_provider, model_name, version, status,
                       input_price_microusd_per_million_tokens,
                       output_price_microusd_per_million_tokens,
                       notes, source_agent_profile_id, created_by_human_user_id,
                       updated_by_human_user_id, created_at, updated_at
                FROM agent_model_price_catalog_entries
                WHERE company_id = $1
                ORDER BY model_provider, model_name, version DESC
                "#,
                &[&company_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_model_price_catalog_entry)
        .collect()
    }

    fn publish_company_governance_policy_version(
        &self,
        mut version: CompanyGovernancePolicyVersion,
    ) -> AppResult<CompanyGovernancePolicyVersion> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.query_one(
                "SELECT id FROM companies WHERE id = $1 FOR UPDATE",
                &[&version.company_id],
            )?;
            if let Some(row) = tx.query_opt(
                r#"
                SELECT id, company_id, version, status, settings, notes,
                       created_by_human_user_id, updated_by_human_user_id,
                       created_at, updated_at
                FROM company_governance_policy_versions
                WHERE company_id = $1 AND status = 'active'
                "#,
                &[&version.company_id],
            )? {
                let active = map_company_governance_policy_version(row);
                if active.settings == version.settings {
                    tx.commit()?;
                    return Ok(active);
                }
            }
            let next_version: i32 = tx
                .query_one(
                    r#"
                    SELECT COALESCE(MAX(version), 0)::INTEGER + 1 AS next_version
                    FROM company_governance_policy_versions
                    WHERE company_id = $1
                    "#,
                    &[&version.company_id],
                )?
                .get("next_version");
            tx.execute(
                r#"
                UPDATE company_governance_policy_versions
                SET status = 'archived',
                    updated_by_human_user_id = $2,
                    updated_at = $3
                WHERE company_id = $1 AND status = 'active'
                "#,
                &[
                    &version.company_id,
                    &version.updated_by_human_user_id,
                    &version.updated_at,
                ],
            )?;
            version.version = next_version;
            tx.execute(
                r#"
                INSERT INTO company_governance_policy_versions (
                    id, company_id, version, status, settings, notes,
                    created_by_human_user_id, updated_by_human_user_id,
                    created_at, updated_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
                &[
                    &version.id,
                    &version.company_id,
                    &version.version,
                    &version.status,
                    &Json(version.settings.clone()),
                    &version.notes,
                    &version.created_by_human_user_id,
                    &version.updated_by_human_user_id,
                    &version.created_at,
                    &version.updated_at,
                ],
            )?;
            tx.commit()?;
            Ok(version)
        })
    }

    fn get_active_company_governance_policy_version(
        &self,
        company_id: Uuid,
    ) -> Option<CompanyGovernancePolicyVersion> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, version, status, settings, notes,
                       created_by_human_user_id, updated_by_human_user_id,
                       created_at, updated_at
                FROM company_governance_policy_versions
                WHERE company_id = $1 AND status = 'active'
                "#,
                &[&company_id],
            )
        })
        .ok()
        .flatten()
        .map(map_company_governance_policy_version)
    }

    fn list_company_governance_policy_versions(
        &self,
        company_id: Uuid,
    ) -> Vec<CompanyGovernancePolicyVersion> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, company_id, version, status, settings, notes,
                       created_by_human_user_id, updated_by_human_user_id,
                       created_at, updated_at
                FROM company_governance_policy_versions
                WHERE company_id = $1
                ORDER BY version DESC
                "#,
                &[&company_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_company_governance_policy_version)
        .collect()
    }

    fn list_due_agent_runtime_configs(
        &self,
        now: chrono::DateTime<chrono::Utc>,
        limit: usize,
    ) -> Vec<AgentRuntimeConfig> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT r.id, r.company_id, r.agent_profile_id, r.runtime_template_id,
                       r.executor_kind, r.status,
                       r.interval_seconds, r.max_events, r.daily_run_budget, r.daily_action_budget,
                       r.daily_model_input_token_budget, r.daily_model_output_token_budget,
                       r.daily_model_cost_budget_microusd,
                       r.model_input_price_microusd_per_million_tokens,
                       r.model_output_price_microusd_per_million_tokens,
                       r.daily_approval_request_budget,
                       r.company_message_policy, r.auto_start_assigned_tasks,
                       r.auto_announce_project_membership, r.context_max_projects,
                       r.context_max_conversations, r.context_max_inbox_events,
                       r.system_prompt, r.model_name, r.model_provider, r.provider_secret_ref,
                       r.allowed_model_actions, r.approval_required_model_actions,
                       r.approval_request_ttl_minutes, r.model_max_output_tokens,
                       r.model_timeout_seconds, r.model_max_retries,
                       r.fallback_to_rules_v1, r.next_run_at,
                       r.last_run_at, r.last_success_at, r.last_error_at, r.last_error,
                       r.created_by_human_user_id, r.updated_by_human_user_id,
                       r.created_at, r.updated_at, r.model_price_catalog_entry_id
                FROM agent_runtime_configs r
                JOIN companies c ON c.id = r.company_id AND c.status = 'active'
                JOIN company_agent_memberships m
                  ON m.agent_profile_id = r.agent_profile_id
                 AND m.company_id = r.company_id
                 AND m.employment_status = 'active'
                JOIN agent_profiles a ON a.id = r.agent_profile_id AND a.status = 'active'
                WHERE r.status = 'active'
                  AND r.next_run_at <= $1
                ORDER BY r.next_run_at
                LIMIT $2
                "#,
                &[&now, &(limit as i64)],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_runtime_config)
        .collect()
    }

    fn claim_agent_runtime_config(
        &self,
        runtime_config_id: Uuid,
        now: chrono::DateTime<chrono::Utc>,
        next_run_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<bool> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_runtime_configs
                SET next_run_at = $3,
                    updated_at = $2
                WHERE id = $1
                  AND status = 'active'
                  AND next_run_at <= $2
                "#,
                &[&runtime_config_id, &now, &next_run_at],
            )
        })
        .map(|updated| updated > 0)
    }

    fn insert_agent_runtime_run(&self, run: AgentRuntimeRun) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_runtime_runs (
                    id, runtime_config_id, company_id, agent_profile_id,
                    trigger_type, executor_kind, status, input_event_count,
                    processed_event_count, action_count, approval_request_count,
                    model_request_count,
                    model_input_tokens, model_output_tokens, model_cost_microusd,
                    model_input_price_microusd_per_million_tokens,
                    model_output_price_microusd_per_million_tokens, model_pricing_status,
                    remaining_pending_count,
                    input_payload, output_payload, error_message, started_at, finished_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)
                "#,
                &[
                    &run.id,
                    &run.runtime_config_id,
                    &run.company_id,
                    &run.agent_profile_id,
                    &run.trigger_type,
                    &run.executor_kind,
                    &run.status,
                    &run.input_event_count,
                    &run.processed_event_count,
                    &run.action_count,
                    &run.approval_request_count,
                    &run.model_request_count,
                    &run.model_input_tokens,
                    &run.model_output_tokens,
                    &run.model_cost_microusd,
                    &run.model_input_price_microusd_per_million_tokens,
                    &run.model_output_price_microusd_per_million_tokens,
                    &run.model_pricing_status,
                    &run.remaining_pending_count,
                    &Json(run.input_payload.clone()),
                    &Json(run.output_payload.clone()),
                    &run.error_message,
                    &run.started_at,
                    &run.finished_at,
                ],
            )?;
            Ok(())
        })
    }

    fn update_agent_runtime_run(&self, run: AgentRuntimeRun) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_runtime_runs
                SET status = $2,
                    input_event_count = $3,
                    processed_event_count = $4,
                    action_count = $5,
                    approval_request_count = $6,
                    model_request_count = $7,
                    model_input_tokens = $8,
                    model_output_tokens = $9,
                    model_cost_microusd = $10,
                    model_input_price_microusd_per_million_tokens = $11,
                    model_output_price_microusd_per_million_tokens = $12,
                    model_pricing_status = $13,
                    remaining_pending_count = $14,
                    input_payload = $15,
                    output_payload = $16,
                    error_message = $17,
                    finished_at = $18
                WHERE id = $1
                "#,
                &[
                    &run.id,
                    &run.status,
                    &run.input_event_count,
                    &run.processed_event_count,
                    &run.action_count,
                    &run.approval_request_count,
                    &run.model_request_count,
                    &run.model_input_tokens,
                    &run.model_output_tokens,
                    &run.model_cost_microusd,
                    &run.model_input_price_microusd_per_million_tokens,
                    &run.model_output_price_microusd_per_million_tokens,
                    &run.model_pricing_status,
                    &run.remaining_pending_count,
                    &Json(run.input_payload.clone()),
                    &Json(run.output_payload.clone()),
                    &run.error_message,
                    &run.finished_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_agent_runtime_runs(
        &self,
        runtime_config_id: Uuid,
        limit: usize,
    ) -> Vec<AgentRuntimeRun> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, runtime_config_id, company_id, agent_profile_id,
                       trigger_type, executor_kind, status, input_event_count,
                       processed_event_count, action_count, approval_request_count,
                       model_request_count,
                       model_input_tokens, model_output_tokens, model_cost_microusd,
                       model_input_price_microusd_per_million_tokens,
                       model_output_price_microusd_per_million_tokens, model_pricing_status,
                       remaining_pending_count,
                       input_payload, output_payload, error_message, started_at, finished_at
                FROM agent_runtime_runs
                WHERE runtime_config_id = $1
                ORDER BY started_at DESC
                LIMIT $2
                "#,
                &[&runtime_config_id, &(limit as i64)],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_runtime_run)
        .collect()
    }

    fn get_agent_runtime_daily_usage(
        &self,
        runtime_config_id: Uuid,
        window_start: chrono::DateTime<chrono::Utc>,
    ) -> AgentRuntimeDailyUsage {
        self.with_client(|client| {
            client.query_one(
                r#"
                SELECT
                    COUNT(*) FILTER (WHERE status <> 'skipped_budget')::BIGINT AS run_count,
                    COALESCE(SUM(action_count) FILTER (WHERE status <> 'skipped_budget'), 0)::BIGINT
                        AS action_count,
                    COALESCE(SUM(approval_request_count) FILTER (WHERE status <> 'skipped_budget'), 0)::BIGINT
                        AS approval_request_count,
                    COUNT(*) FILTER (WHERE status = 'failed')::BIGINT AS failed_run_count,
                    COALESCE(SUM(model_request_count) FILTER (WHERE status <> 'skipped_budget'), 0)::BIGINT
                        AS model_request_count,
                    COALESCE(SUM(model_input_tokens) FILTER (WHERE status <> 'skipped_budget'), 0)::BIGINT
                        AS model_input_tokens,
                    COALESCE(SUM(model_output_tokens) FILTER (WHERE status <> 'skipped_budget'), 0)::BIGINT
                        AS model_output_tokens,
                    COALESCE(SUM(model_cost_microusd) FILTER (WHERE status <> 'skipped_budget'), 0)::BIGINT
                        AS model_cost_microusd
                FROM agent_runtime_runs
                WHERE runtime_config_id = $1
                  AND started_at >= $2
                "#,
                &[&runtime_config_id, &window_start],
            )
        })
        .map(|row| AgentRuntimeDailyUsage {
            window_start,
            run_count: row.get("run_count"),
            action_count: row.get("action_count"),
            approval_request_count: row.get("approval_request_count"),
            failed_run_count: row.get("failed_run_count"),
            model_request_count: row.get("model_request_count"),
            model_input_tokens: row.get("model_input_tokens"),
            model_output_tokens: row.get("model_output_tokens"),
            model_cost_microusd: row.get("model_cost_microusd"),
        })
        .unwrap_or(AgentRuntimeDailyUsage {
            window_start,
            run_count: 0,
            action_count: 0,
            approval_request_count: 0,
            failed_run_count: 0,
            model_request_count: 0,
            model_input_tokens: 0,
            model_output_tokens: 0,
            model_cost_microusd: 0,
        })
    }

    fn save_company_model_budget_policy(&self, policy: CompanyModelBudgetPolicy) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO company_model_budget_policies (
                    company_id, daily_model_cost_budget_microusd,
                    created_by_human_user_id, updated_by_human_user_id,
                    created_at, updated_at
                ) VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (company_id) DO UPDATE
                SET daily_model_cost_budget_microusd = EXCLUDED.daily_model_cost_budget_microusd,
                    updated_by_human_user_id = EXCLUDED.updated_by_human_user_id,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &policy.company_id,
                    &policy.daily_model_cost_budget_microusd,
                    &policy.created_by_human_user_id,
                    &policy.updated_by_human_user_id,
                    &policy.created_at,
                    &policy.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_company_model_budget_policy(
        &self,
        company_id: Uuid,
    ) -> Option<CompanyModelBudgetPolicy> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT company_id, daily_model_cost_budget_microusd,
                       created_by_human_user_id, updated_by_human_user_id,
                       created_at, updated_at
                FROM company_model_budget_policies
                WHERE company_id = $1
                "#,
                &[&company_id],
            )
        })
        .ok()
        .flatten()
        .map(|row| CompanyModelBudgetPolicy {
            company_id: row.get("company_id"),
            daily_model_cost_budget_microusd: row.get("daily_model_cost_budget_microusd"),
            configured: true,
            created_by_human_user_id: row.get("created_by_human_user_id"),
            updated_by_human_user_id: row.get("updated_by_human_user_id"),
            created_at: Some(row.get("created_at")),
            updated_at: Some(row.get("updated_at")),
        })
    }

    fn get_company_model_daily_usage(
        &self,
        company_id: Uuid,
        window_start: chrono::DateTime<chrono::Utc>,
    ) -> CompanyModelDailyUsage {
        self.with_client(|client| {
            client.query_one(
                r#"
                SELECT
                    COALESCE(SUM(model_request_count) FILTER (WHERE status <> 'skipped_budget'), 0)::BIGINT
                        AS model_request_count,
                    COALESCE(SUM(model_input_tokens) FILTER (WHERE status <> 'skipped_budget'), 0)::BIGINT
                        AS model_input_tokens,
                    COALESCE(SUM(model_output_tokens) FILTER (WHERE status <> 'skipped_budget'), 0)::BIGINT
                        AS model_output_tokens,
                    COALESCE(SUM(model_cost_microusd) FILTER (WHERE status <> 'skipped_budget'), 0)::BIGINT
                        AS model_cost_microusd
                FROM agent_runtime_runs
                WHERE company_id = $1
                  AND started_at >= $2
                "#,
                &[&company_id, &window_start],
            )
        })
        .map(|row| CompanyModelDailyUsage {
            window_start,
            model_request_count: row.get("model_request_count"),
            model_input_tokens: row.get("model_input_tokens"),
            model_output_tokens: row.get("model_output_tokens"),
            model_cost_microusd: row.get("model_cost_microusd"),
        })
        .unwrap_or(CompanyModelDailyUsage {
            window_start,
            model_request_count: 0,
            model_input_tokens: 0,
            model_output_tokens: 0,
            model_cost_microusd: 0,
        })
    }

    fn insert_agent_tool_approval_request(
        &self,
        request: AgentToolApprovalRequest,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_tool_approval_requests (
                    id, company_id, approval_source, runtime_config_id, runtime_run_id,
                    codex_trigger_run_id,
                    requested_by_agent_id, tool_name, risk_level, reason, arguments,
                    status, expires_at, reviewed_by_human_user_id, review_note,
                    reviewed_at, execution_result, error_message, created_at, updated_at
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                    $12, $13, $14, $15, $16, $17, $18, $19, $20, $21
                )
                "#,
                &[
                    &request.id,
                    &request.company_id,
                    &request.approval_source,
                    &request.runtime_config_id,
                    &request.runtime_run_id,
                    &request.codex_trigger_run_id,
                    &request.requested_by_agent_id,
                    &request.tool_name,
                    &request.risk_level,
                    &request.reason,
                    &Json(request.arguments.clone()),
                    &request.status,
                    &request.expires_at,
                    &request.reviewed_by_human_user_id,
                    &request.review_note,
                    &request.reviewed_at,
                    &Json(request.execution_result.clone()),
                    &request.error_message,
                    &request.created_at,
                    &request.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_agent_tool_approval_request(
        &self,
        approval_request_id: Uuid,
    ) -> Option<AgentToolApprovalRequest> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, approval_source, runtime_config_id, runtime_run_id,
                       codex_trigger_run_id,
                       requested_by_agent_id, tool_name, risk_level, reason, arguments,
                       status, expires_at, reviewed_by_human_user_id, review_note,
                       reviewed_at, execution_result, error_message, created_at, updated_at
                FROM agent_tool_approval_requests
                WHERE id = $1
                "#,
                &[&approval_request_id],
            )
        })
        .ok()
        .flatten()
        .map(map_agent_tool_approval_request)
    }

    fn list_company_agent_tool_approval_requests(
        &self,
        company_id: Uuid,
        status: Option<&str>,
        limit: usize,
    ) -> Vec<AgentToolApprovalRequest> {
        let status = status.map(str::to_string);
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, company_id, approval_source, runtime_config_id, runtime_run_id,
                       codex_trigger_run_id,
                       requested_by_agent_id, tool_name, risk_level, reason, arguments,
                       status, expires_at, reviewed_by_human_user_id, review_note,
                       reviewed_at, execution_result, error_message, created_at, updated_at
                FROM agent_tool_approval_requests
                WHERE company_id = $1
                  AND ($2::TEXT IS NULL OR status = $2)
                ORDER BY created_at DESC
                LIMIT $3
                "#,
                &[&company_id, &status, &(limit as i64)],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_agent_tool_approval_request)
        .collect()
    }

    fn claim_agent_tool_approval_request(
        &self,
        approval_request_id: Uuid,
        human_user_id: Uuid,
        status: &str,
        review_note: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<Option<AgentToolApprovalRequest>> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                UPDATE agent_tool_approval_requests
                SET status = $3,
                    reviewed_by_human_user_id = $2,
                    review_note = $4,
                    reviewed_at = $5,
                    updated_at = $5
                WHERE id = $1
                  AND status = 'pending'
                  AND expires_at > $5
                RETURNING id, company_id, approval_source, runtime_config_id, runtime_run_id,
                          codex_trigger_run_id,
                          requested_by_agent_id, tool_name, risk_level, reason, arguments,
                          status, expires_at, reviewed_by_human_user_id, review_note,
                          reviewed_at, execution_result, error_message, created_at, updated_at
                "#,
                &[
                    &approval_request_id,
                    &human_user_id,
                    &status,
                    &review_note,
                    &now,
                ],
            )
        })
        .map(|row| row.map(map_agent_tool_approval_request))
    }

    fn update_agent_tool_approval_request(
        &self,
        request: AgentToolApprovalRequest,
    ) -> AppResult<()> {
        let updated = self.with_client(|client| {
            client.execute(
                r#"
                UPDATE agent_tool_approval_requests
                SET status = $2,
                    reviewed_by_human_user_id = $3,
                    review_note = $4,
                    reviewed_at = $5,
                    execution_result = $6,
                    error_message = $7,
                    updated_at = $8
                WHERE id = $1
                "#,
                &[
                    &request.id,
                    &request.status,
                    &request.reviewed_by_human_user_id,
                    &request.review_note,
                    &request.reviewed_at,
                    &Json(request.execution_result.clone()),
                    &request.error_message,
                    &request.updated_at,
                ],
            )
        })?;
        if updated == 0 {
            return Err(AppError::NotFound(
                "agent tool approval request not found".into(),
            ));
        }
        Ok(())
    }

    fn count_agent_tool_approval_requests_since(
        &self,
        agent_id: Uuid,
        since: chrono::DateTime<chrono::Utc>,
    ) -> usize {
        self.with_client(|client| {
            client.query_one(
                "SELECT COUNT(*)::BIGINT AS count FROM agent_tool_approval_requests WHERE requested_by_agent_id = $1 AND created_at >= $2",
                &[&agent_id, &since],
            )
        })
        .map(|row| row.get::<_, i64>("count").max(0) as usize)
        .unwrap_or(0)
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
        .unwrap_or_default()
        .into_iter()
        .map(map_conversation_preview)
        .collect()
    }

    fn get_conversation_messages(&self, conversation_id: Uuid) -> Vec<MessageView> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, conversation_id, sender_agent_id, sender_human_user_id,
                       content_text, created_at
                FROM messages
                WHERE conversation_id = $1
                ORDER BY created_at ASC
                "#,
                &[&conversation_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_message_view)
        .collect()
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
                           content_text, created_at
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
                           content_text, created_at
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

    fn insert_post(&self, post: PostView) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO posts (
                    id, author_agent_id, content_text, content_json, visibility, published_at, created_at
                )
                VALUES ($1, $2, $3, '{}'::jsonb, 'public', $4, $4)
                "#,
                &[&post.id, &post.author_agent_id, &post.content, &post.created_at],
            )?;
            Ok(())
        })
    }

    fn insert_post_comment(&self, comment: PostCommentView) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO post_comments (
                    id, post_id, author_agent_id, content_text, created_at
                )
                VALUES ($1, $2, $3, $4, $5)
                "#,
                &[
                    &comment.id,
                    &comment.post_id,
                    &comment.author_agent_id,
                    &comment.content,
                    &comment.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn insert_diary_entry(&self, diary_entry: DiaryEntryView) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO diary_entries (
                    id, agent_profile_id, title, content_text, content_json, mood_tag, created_at
                )
                VALUES ($1, $2, $3, $4, '{}'::jsonb, NULL, $5)
                "#,
                &[
                    &diary_entry.id,
                    &diary_entry.agent_profile_id,
                    &diary_entry.title,
                    &diary_entry.content,
                    &diary_entry.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_posts(&self) -> Vec<PostView> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, author_agent_id, content_text, published_at
                FROM posts
                ORDER BY published_at DESC, created_at DESC
                "#,
                &[],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_post_view)
        .collect()
    }

    fn list_post_comments(&self, post_id: Uuid) -> Vec<PostCommentView> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, post_id, author_agent_id, content_text, created_at
                FROM post_comments
                WHERE post_id = $1
                ORDER BY created_at ASC
                "#,
                &[&post_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_post_comment_view)
        .collect()
    }

    fn list_diary_entries(&self) -> Vec<DiaryEntryView> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, agent_profile_id, title, content_text, created_at
                FROM diary_entries
                ORDER BY created_at DESC
                "#,
                &[],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_diary_entry_view)
        .collect()
    }

    fn insert_friend_request(&self, request: FriendRequestView) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO friend_requests (
                    id, requester_agent_id, target_agent_id, message, status, acted_at, created_at
                )
                VALUES ($1, $2, $3, $4, $5, NULL, $6)
                "#,
                &[
                    &request.id,
                    &request.requester_agent_id,
                    &request.target_agent_id,
                    &request.message,
                    &request.status,
                    &request.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_friend_requests(&self, agent_id: Uuid) -> Vec<FriendRequestView> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, requester_agent_id, target_agent_id, message, status, created_at
                FROM friend_requests
                WHERE requester_agent_id = $1 OR target_agent_id = $1
                ORDER BY created_at DESC
                "#,
                &[&agent_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_friend_request_view)
        .collect()
    }

    fn list_all_friend_requests(&self) -> Vec<FriendRequestView> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, requester_agent_id, target_agent_id, message, status, created_at
                FROM friend_requests
                ORDER BY created_at DESC
                "#,
                &[],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_friend_request_view)
        .collect()
    }

    fn list_friends(&self, agent_id: Uuid) -> Vec<FriendSummary> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT ap.id, ap.display_name, ap.handle
                FROM friendships f
                INNER JOIN agent_profiles ap
                    ON ap.id = CASE
                        WHEN f.agent_low_id = $1 THEN f.agent_high_id
                        ELSE f.agent_low_id
                    END
                WHERE f.agent_low_id = $1 OR f.agent_high_id = $1
                ORDER BY ap.created_at DESC
                "#,
                &[&agent_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_friend_summary)
        .collect()
    }

    fn get_friend_request(&self, request_id: Uuid) -> Option<FriendRequestView> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, requester_agent_id, target_agent_id, message, status, created_at
                FROM friend_requests
                WHERE id = $1
                "#,
                &[&request_id],
            )
        })
        .ok()
        .flatten()
        .map(map_friend_request_view)
    }

    fn update_friend_request(&self, request: FriendRequestView) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE friend_requests
                SET message = $2,
                    status = $3,
                    acted_at = CASE WHEN $3 = 'pending' THEN NULL ELSE NOW() END
                WHERE id = $1
                "#,
                &[&request.id, &request.message, &request.status],
            )?;
            Ok(())
        })
    }

    fn link_friends(&self, left_agent_id: Uuid, right_agent_id: Uuid) -> AppResult<()> {
        let (low, high) = ordered_pair(left_agent_id, right_agent_id);
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO friendships (id, agent_low_id, agent_high_id, created_at)
                VALUES ($1, $2, $3, NOW())
                ON CONFLICT (agent_low_id, agent_high_id) DO NOTHING
                "#,
                &[&Uuid::new_v4(), &low, &high],
            )?;
            Ok(())
        })
    }

    fn get_friend_profile(
        &self,
        owner_agent_id: Uuid,
        friend_agent_id: Uuid,
    ) -> Option<FriendProfileSnapshot> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT fp.owner_agent_id, fp.friend_agent_id, fp.display_name_hint,
                       fp.capability_summary, fp.familiarity_score, fp.trust_score,
                       fp.last_interaction_summary,
                       COALESCE((
                           SELECT json_agg(fpf.fact_value ORDER BY fpf.created_at)
                           FROM friend_profile_facts fpf
                           WHERE fpf.friend_profile_id = fp.id
                             AND fpf.fact_type = 'interest_tag'
                       ), '[]'::json) AS interest_tags
                     , COALESCE((
                           SELECT json_agg(
                               json_build_object(
                                   'fact_type', fpf.fact_type,
                                   'fact_value', fpf.fact_value,
                                   'confidence_score', fpf.confidence_score,
                                   'source_kind', fpf.source_kind,
                                   'source_ref_id', fpf.source_ref_id,
                                   'last_observed_at', fpf.last_observed_at
                               )
                               ORDER BY fpf.created_at
                           )
                           FROM friend_profile_facts fpf
                           WHERE fpf.friend_profile_id = fp.id
                       ), '[]'::json) AS known_facts
                FROM friend_profiles fp
                WHERE fp.owner_agent_id = $1 AND fp.friend_agent_id = $2
                "#,
                &[&owner_agent_id, &friend_agent_id],
            )
        })
        .ok()
        .flatten()
        .map(map_friend_profile_snapshot)
    }

    fn upsert_friend_profile(&self, profile: FriendProfileSnapshot) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            let persisted_facts = prepare_friend_profile_facts_for_storage(&profile);

            let row = tx.query_one(
                r#"
                INSERT INTO friend_profiles (
                    id, owner_agent_id, friend_agent_id, display_name_hint,
                    capability_summary, familiarity_score, trust_score,
                    last_interaction_summary, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW())
                ON CONFLICT (owner_agent_id, friend_agent_id) DO UPDATE
                SET display_name_hint = EXCLUDED.display_name_hint,
                    capability_summary = EXCLUDED.capability_summary,
                    familiarity_score = EXCLUDED.familiarity_score,
                    trust_score = EXCLUDED.trust_score,
                    last_interaction_summary = EXCLUDED.last_interaction_summary,
                    updated_at = NOW()
                RETURNING id
                "#,
                &[
                    &Uuid::new_v4(),
                    &profile.owner_agent_id,
                    &profile.friend_agent_id,
                    &profile.display_name_hint,
                    &profile.capability_summary,
                    &profile.familiarity_score,
                    &profile.trust_score,
                    &profile.last_interaction_summary,
                ],
            )?;

            let friend_profile_id: Uuid = row.get("id");
            tx.execute(
                "DELETE FROM friend_profile_facts WHERE friend_profile_id = $1",
                &[&friend_profile_id],
            )?;

            for fact in &persisted_facts {
                let confidence_score = format_numeric_5_4(fact.confidence_score);
                tx.execute(
                    r#"
                    INSERT INTO friend_profile_facts (
                        id, friend_profile_id, fact_type, fact_value, confidence_score,
                        source_kind, source_ref_id, last_observed_at, created_at
                    )
                    VALUES ($1, $2, $3, $4, ($5)::TEXT::NUMERIC(5,4), $6, $7, $8, NOW())
                    "#,
                    &[
                        &Uuid::new_v4(),
                        &friend_profile_id,
                        &friend_profile_fact_type_to_str(&fact.fact_type),
                        &fact.fact_value,
                        &confidence_score,
                        &friend_profile_fact_source_kind_to_str(&fact.source_kind),
                        &fact.source_ref_id,
                        &fact.last_observed_at,
                    ],
                )?;
            }

            tx.execute(
                r#"
                INSERT INTO relationship_states (
                    id, owner_agent_id, target_agent_id, intimacy_score, trust_score,
                    interaction_heat, last_contact_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
                ON CONFLICT (owner_agent_id, target_agent_id) DO UPDATE
                SET intimacy_score = EXCLUDED.intimacy_score,
                    trust_score = EXCLUDED.trust_score,
                    interaction_heat = EXCLUDED.interaction_heat,
                    last_contact_at = EXCLUDED.last_contact_at,
                    updated_at = NOW()
                "#,
                &[
                    &Uuid::new_v4(),
                    &profile.owner_agent_id,
                    &profile.friend_agent_id,
                    &profile.familiarity_score,
                    &profile.trust_score,
                    &profile.familiarity_score,
                ],
            )?;

            let interest_tags_json =
                Json(serde_json::to_value(&profile.interest_tags).unwrap_or(Value::Array(vec![])));
            tx.execute(
                r#"
                INSERT INTO interaction_summaries (
                    id, owner_agent_id, target_agent_id, window_type,
                    summary_text, summary_json, start_at, end_at, created_at
                )
                VALUES ($1, $2, $3, 'rolling', $4, $5, NOW(), NOW(), NOW())
                "#,
                &[
                    &Uuid::new_v4(),
                    &profile.owner_agent_id,
                    &profile.friend_agent_id,
                    &profile.last_interaction_summary,
                    &interest_tags_json,
                ],
            )?;

            tx.commit()?;
            Ok(())
        })
    }

    fn find_direct_conversation(
        &self,
        left_agent_id: Uuid,
        right_agent_id: Uuid,
    ) -> Option<ConversationPreview> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT c.id,
                       COALESCE(peer.display_name, c.title, '') AS title,
                       c.conversation_type,
                       latest.content_text AS last_message_preview,
                       COALESCE(latest.created_at, c.updated_at, c.created_at) AS updated_at
                FROM conversations c
                INNER JOIN conversation_members cm1
                    ON cm1.conversation_id = c.id
                   AND cm1.agent_profile_id = $1
                   AND cm1.left_at IS NULL
                INNER JOIN conversation_members cm2
                    ON cm2.conversation_id = c.id
                   AND cm2.agent_profile_id = $2
                   AND cm2.left_at IS NULL
                LEFT JOIN agent_profiles peer ON peer.id = $2
                LEFT JOIN LATERAL (
                    SELECT content_text, created_at
                    FROM messages
                    WHERE conversation_id = c.id
                    ORDER BY created_at DESC
                    LIMIT 1
                ) latest ON TRUE
                WHERE c.conversation_type = 'direct'
                ORDER BY updated_at DESC
                LIMIT 1
                "#,
                &[&left_agent_id, &right_agent_id],
            )
        })
        .ok()
        .flatten()
        .map(map_conversation_preview)
    }

    fn find_direct_conversation_peer(&self, conversation_id: Uuid, agent_id: Uuid) -> Option<Uuid> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT cm.agent_profile_id
                FROM conversations c
                INNER JOIN conversation_members cm ON cm.conversation_id = c.id
                WHERE c.id = $1
                  AND c.conversation_type = 'direct'
                  AND cm.left_at IS NULL
                  AND cm.agent_profile_id <> $2
                LIMIT 1
                "#,
                &[&conversation_id, &agent_id],
            )
        })
        .ok()
        .flatten()
        .map(|row| row.get(0))
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

    fn insert_problem_workspace(&self, workspace: ProblemWorkspace) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO problem_workspaces (
                    id, owner_agent_id, title, problem_statement, status,
                    pressure_level, conversation_id, participant_agent_ids,
                    summary_text, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                "#,
                &[
                    &workspace.id,
                    &workspace.owner_agent_id,
                    &workspace.title,
                    &workspace.problem_statement,
                    &workspace.status,
                    &workspace.pressure_level,
                    &workspace.conversation_id,
                    &workspace.participant_agent_ids,
                    &workspace.summary_text,
                    &workspace.created_at,
                    &workspace.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn update_problem_workspace(&self, workspace: ProblemWorkspace) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE problem_workspaces
                SET title = $2,
                    problem_statement = $3,
                    status = $4,
                    pressure_level = $5,
                    conversation_id = $6,
                    participant_agent_ids = $7,
                    summary_text = $8,
                    updated_at = $9
                WHERE id = $1
                "#,
                &[
                    &workspace.id,
                    &workspace.title,
                    &workspace.problem_statement,
                    &workspace.status,
                    &workspace.pressure_level,
                    &workspace.conversation_id,
                    &workspace.participant_agent_ids,
                    &workspace.summary_text,
                    &workspace.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_problem_workspace(&self, workspace_id: Uuid) -> Option<ProblemWorkspace> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, owner_agent_id, title, problem_statement, status,
                       pressure_level, conversation_id, participant_agent_ids,
                       summary_text, created_at, updated_at
                FROM problem_workspaces
                WHERE id = $1
                "#,
                &[&workspace_id],
            )
        })
        .ok()
        .flatten()
        .map(map_problem_workspace)
    }

    fn list_problem_workspaces(&self, agent_id: Uuid, limit: usize) -> Vec<ProblemWorkspace> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, owner_agent_id, title, problem_statement, status,
                       pressure_level, conversation_id, participant_agent_ids,
                       summary_text, created_at, updated_at
                FROM problem_workspaces
                WHERE owner_agent_id = $1 OR $1 = ANY(participant_agent_ids)
                ORDER BY updated_at DESC
                LIMIT $2
                "#,
                &[&agent_id, &(limit as i64)],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_problem_workspace)
        .collect()
    }

    fn insert_problem_workspace_invitation(
        &self,
        invitation: ProblemWorkspaceInvitation,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO problem_workspace_invitations (
                    id, workspace_id, inviter_agent_id, invitee_agent_id,
                    message, status, created_at, responded_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
                &[
                    &invitation.id,
                    &invitation.workspace_id,
                    &invitation.inviter_agent_id,
                    &invitation.invitee_agent_id,
                    &invitation.message,
                    &invitation.status,
                    &invitation.created_at,
                    &invitation.responded_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_problem_workspace_invitation(
        &self,
        invitation_id: Uuid,
    ) -> Option<ProblemWorkspaceInvitation> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, workspace_id, inviter_agent_id, invitee_agent_id,
                       message, status, created_at, responded_at
                FROM problem_workspace_invitations
                WHERE id = $1
                "#,
                &[&invitation_id],
            )
        })
        .ok()
        .flatten()
        .map(map_problem_workspace_invitation)
    }

    fn update_problem_workspace_invitation(
        &self,
        invitation: ProblemWorkspaceInvitation,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE problem_workspace_invitations
                SET message = $2,
                    status = $3,
                    responded_at = $4
                WHERE id = $1
                "#,
                &[
                    &invitation.id,
                    &invitation.message,
                    &invitation.status,
                    &invitation.responded_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_problem_workspace_invitations(
        &self,
        agent_id: Uuid,
        workspace_id: Option<Uuid>,
        limit: usize,
    ) -> Vec<ProblemWorkspaceInvitation> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, workspace_id, inviter_agent_id, invitee_agent_id,
                       message, status, created_at, responded_at
                FROM problem_workspace_invitations
                WHERE (inviter_agent_id = $1 OR invitee_agent_id = $1)
                  AND ($2::uuid IS NULL OR workspace_id = $2)
                ORDER BY created_at DESC
                LIMIT $3
                "#,
                &[&agent_id, &workspace_id, &(limit as i64)],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_problem_workspace_invitation)
        .collect()
    }

    fn insert_problem_workspace_progress_record(
        &self,
        record: ProblemWorkspaceProgressRecord,
    ) -> AppResult<()> {
        self.with_client(|client| {
            let mut tx = client.transaction()?;
            tx.execute(
                r#"
                INSERT INTO problem_workspace_progress_records (
                    id, workspace_id, author_agent_id, record_type,
                    content_text, artifact_ref, task_status, assignee_agent_id, due_at, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
                &[
                    &record.id,
                    &record.workspace_id,
                    &record.author_agent_id,
                    &record.record_type,
                    &record.content,
                    &record.artifact_ref,
                    &record.task_status,
                    &record.assignee_agent_id,
                    &record.due_at,
                    &record.created_at,
                ],
            )?;
            tx.execute(
                r#"
                UPDATE problem_workspaces
                SET updated_at = $2
                WHERE id = $1
                "#,
                &[&record.workspace_id, &record.created_at],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    fn get_problem_workspace_progress_record(
        &self,
        record_id: Uuid,
    ) -> Option<ProblemWorkspaceProgressRecord> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, workspace_id, author_agent_id, record_type,
                       content_text, artifact_ref, task_status, assignee_agent_id, due_at, created_at
                FROM problem_workspace_progress_records
                WHERE id = $1
                "#,
                &[&record_id],
            )
        })
        .ok()
        .flatten()
        .map(map_problem_workspace_progress_record)
    }

    fn update_problem_workspace_progress_record(
        &self,
        record: ProblemWorkspaceProgressRecord,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE problem_workspace_progress_records
                SET record_type = $2,
                    content_text = $3,
                    artifact_ref = $4,
                    task_status = $5,
                    assignee_agent_id = $6,
                    due_at = $7
                WHERE id = $1
                "#,
                &[
                    &record.id,
                    &record.record_type,
                    &record.content,
                    &record.artifact_ref,
                    &record.task_status,
                    &record.assignee_agent_id,
                    &record.due_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_problem_workspace_progress_records(
        &self,
        workspace_id: Uuid,
        limit: usize,
    ) -> Vec<ProblemWorkspaceProgressRecord> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, workspace_id, author_agent_id, record_type,
                       content_text, artifact_ref, task_status, assignee_agent_id, due_at, created_at
                FROM problem_workspace_progress_records
                WHERE workspace_id = $1
                ORDER BY created_at DESC
                LIMIT $2
                "#,
                &[&workspace_id, &(limit as i64)],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_problem_workspace_progress_record)
        .collect()
    }
}

fn insert_agent_staffing_action(
    client: &mut impl GenericClient,
    action: &AgentStaffingAction,
) -> Result<(), postgres::Error> {
    client.execute(
        r#"
        INSERT INTO agent_staffing_actions (
            id, company_id, action_type, actor_type, actor_human_user_id,
            actor_agent_id, target_agent_id, requested_org_unit_id,
            requested_role_key, reason, handoff_plan, status, approval_required,
            approved_by_human_user_id, request_payload, result_payload,
            idempotency_key, created_at, completed_at
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14, $15, $16, $17, $18, $19
        )
        "#,
        &[
            &action.id,
            &action.company_id,
            &action.action_type,
            &action.actor_type,
            &action.actor_human_user_id,
            &action.actor_agent_id,
            &action.target_agent_id,
            &action.requested_org_unit_id,
            &action.requested_role_key,
            &action.reason,
            &action.handoff_plan,
            &action.status,
            &action.approval_required,
            &action.approved_by_human_user_id,
            &Json(action.request_payload.clone()),
            &Json(action.result_payload.clone()),
            &action.idempotency_key,
            &action.created_at,
            &action.completed_at,
        ],
    )?;
    Ok(())
}

fn upsert_agent_runtime_config(
    client: &mut impl GenericClient,
    config: &AgentRuntimeConfig,
) -> Result<(), postgres::Error> {
    client.execute(
        r#"
        INSERT INTO agent_runtime_configs (
            id, company_id, agent_profile_id, runtime_template_id, executor_kind, status,
            interval_seconds, max_events, daily_run_budget, daily_action_budget,
            daily_model_input_token_budget, daily_model_output_token_budget,
            daily_model_cost_budget_microusd,
            model_input_price_microusd_per_million_tokens,
            model_output_price_microusd_per_million_tokens,
            daily_approval_request_budget,
            company_message_policy, auto_start_assigned_tasks,
            auto_announce_project_membership, context_max_projects,
            context_max_conversations, context_max_inbox_events,
            system_prompt, model_name, model_provider, provider_secret_ref,
            allowed_model_actions, approval_required_model_actions,
            approval_request_ttl_minutes, model_max_output_tokens, model_timeout_seconds,
            model_max_retries, fallback_to_rules_v1, next_run_at,
            last_run_at, last_success_at, last_error_at, last_error,
            created_by_human_user_id, updated_by_human_user_id, created_at, updated_at,
            model_price_catalog_entry_id
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
            $12, $13, $14, $15, $16, $17, $18, $19, $20, $21,
            $22, $23, $24, $25, $26, $27, $28, $29, $30, $31,
            $32, $33, $34, $35, $36, $37, $38, $39, $40,
            $41, $42, $43
        )
        ON CONFLICT (id) DO UPDATE
        SET executor_kind = EXCLUDED.executor_kind,
            status = EXCLUDED.status,
            runtime_template_id = EXCLUDED.runtime_template_id,
            model_price_catalog_entry_id = EXCLUDED.model_price_catalog_entry_id,
            interval_seconds = EXCLUDED.interval_seconds,
            max_events = EXCLUDED.max_events,
            daily_run_budget = EXCLUDED.daily_run_budget,
            daily_action_budget = EXCLUDED.daily_action_budget,
            daily_model_input_token_budget = EXCLUDED.daily_model_input_token_budget,
            daily_model_output_token_budget = EXCLUDED.daily_model_output_token_budget,
            daily_model_cost_budget_microusd = EXCLUDED.daily_model_cost_budget_microusd,
            model_input_price_microusd_per_million_tokens = EXCLUDED.model_input_price_microusd_per_million_tokens,
            model_output_price_microusd_per_million_tokens = EXCLUDED.model_output_price_microusd_per_million_tokens,
            daily_approval_request_budget = EXCLUDED.daily_approval_request_budget,
            company_message_policy = EXCLUDED.company_message_policy,
            auto_start_assigned_tasks = EXCLUDED.auto_start_assigned_tasks,
            auto_announce_project_membership = EXCLUDED.auto_announce_project_membership,
            context_max_projects = EXCLUDED.context_max_projects,
            context_max_conversations = EXCLUDED.context_max_conversations,
            context_max_inbox_events = EXCLUDED.context_max_inbox_events,
            system_prompt = EXCLUDED.system_prompt,
            model_name = EXCLUDED.model_name,
            model_provider = EXCLUDED.model_provider,
            provider_secret_ref = EXCLUDED.provider_secret_ref,
            allowed_model_actions = EXCLUDED.allowed_model_actions,
            approval_required_model_actions = EXCLUDED.approval_required_model_actions,
            approval_request_ttl_minutes = EXCLUDED.approval_request_ttl_minutes,
            model_max_output_tokens = EXCLUDED.model_max_output_tokens,
            model_timeout_seconds = EXCLUDED.model_timeout_seconds,
            model_max_retries = EXCLUDED.model_max_retries,
            fallback_to_rules_v1 = EXCLUDED.fallback_to_rules_v1,
            next_run_at = EXCLUDED.next_run_at,
            last_run_at = EXCLUDED.last_run_at,
            last_success_at = EXCLUDED.last_success_at,
            last_error_at = EXCLUDED.last_error_at,
            last_error = EXCLUDED.last_error,
            updated_by_human_user_id = EXCLUDED.updated_by_human_user_id,
            updated_at = EXCLUDED.updated_at
        "#,
        &[
            &config.id,
            &config.company_id,
            &config.agent_profile_id,
            &config.runtime_template_id,
            &config.executor_kind,
            &config.status,
            &config.interval_seconds,
            &config.max_events,
            &config.daily_run_budget,
            &config.daily_action_budget,
            &config.daily_model_input_token_budget,
            &config.daily_model_output_token_budget,
            &config.daily_model_cost_budget_microusd,
            &config.model_input_price_microusd_per_million_tokens,
            &config.model_output_price_microusd_per_million_tokens,
            &config.daily_approval_request_budget,
            &config.company_message_policy,
            &config.auto_start_assigned_tasks,
            &config.auto_announce_project_membership,
            &config.context_max_projects,
            &config.context_max_conversations,
            &config.context_max_inbox_events,
            &config.system_prompt,
            &config.model_name,
            &config.model_provider,
            &config.provider_secret_ref,
            &Json(config.allowed_model_actions.clone()),
            &Json(config.approval_required_model_actions.clone()),
            &config.approval_request_ttl_minutes,
            &config.model_max_output_tokens,
            &config.model_timeout_seconds,
            &config.model_max_retries,
            &config.fallback_to_rules_v1,
            &config.next_run_at,
            &config.last_run_at,
            &config.last_success_at,
            &config.last_error_at,
            &config.last_error,
            &config.created_by_human_user_id,
            &config.updated_by_human_user_id,
            &config.created_at,
            &config.updated_at,
            &config.model_price_catalog_entry_id,
        ],
    )?;
    Ok(())
}

fn map_postgres_error(error: postgres::Error) -> AppError {
    if let Some(db_error) = error.as_db_error() {
        match db_error.code().code() {
            "23505" => AppError::Conflict(db_error.message().to_string()),
            "23503" | "23514" => AppError::Validation(db_error.message().to_string()),
            _ => AppError::Validation(db_error.message().to_string()),
        }
    } else {
        AppError::Validation(error.to_string())
    }
}

fn map_human_user(row: Row) -> HumanUser {
    HumanUser {
        id: row.get("id"),
        email: row.get("email"),
        display_name: row.get("display_name"),
        created_at: row.get("created_at"),
    }
}

fn map_human_credential(row: Row) -> HumanCredential {
    HumanCredential {
        human_user_id: row.get("human_user_id"),
        password_hash: row.get("password_hash"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_human_session(row: Row) -> HumanSession {
    HumanSession {
        id: row.get("id"),
        human_user_id: row.get("human_user_id"),
        token_prefix: row.get("token_prefix"),
        token_hash: row.get("token_hash"),
        expires_at: row.get("expires_at"),
        revoked_at: row.get("revoked_at"),
        last_used_at: row.get("last_used_at"),
        created_at: row.get("created_at"),
    }
}

fn map_human_account_token(row: Row) -> HumanAccountToken {
    HumanAccountToken {
        id: row.get("id"),
        human_user_id: row.get("human_user_id"),
        purpose: row.get("purpose"),
        token_prefix: row.get("token_prefix"),
        token_hash: row.get("token_hash"),
        expires_at: row.get("expires_at"),
        used_at: row.get("used_at"),
        created_at: row.get("created_at"),
    }
}

fn map_company(row: Row) -> Company {
    Company {
        id: row.get("id"),
        owner_user_id: row.get("owner_user_id"),
        name: row.get("name"),
        slug: row.get("slug"),
        description: row.get("description"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_company_human_member(row: Row) -> CompanyHumanMember {
    CompanyHumanMember {
        id: row.get("id"),
        company_id: row.get("company_id"),
        human_user_id: row.get("human_user_id"),
        role: row.get("role"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_org_unit(row: Row) -> OrgUnit {
    OrgUnit {
        id: row.get("id"),
        company_id: row.get("company_id"),
        parent_org_unit_id: row.get("parent_org_unit_id"),
        name: row.get("name"),
        unit_type: row.get("unit_type"),
        sort_order: row.get("sort_order"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_company_agent_membership(row: Row) -> CompanyAgentMembership {
    let permissions: Value = row.get("permissions");
    let responsibilities: Value = row.get("responsibilities");
    let skills: Value = row.get("skills");
    CompanyAgentMembership {
        id: row.get("id"),
        company_id: row.get("company_id"),
        agent_profile_id: row.get("agent_profile_id"),
        org_unit_id: row.get("org_unit_id"),
        job_title: row.get("job_title"),
        role_key: row.get("role_key"),
        reports_to_membership_id: row.get("reports_to_membership_id"),
        permissions: serde_json::from_value(permissions).unwrap_or_default(),
        responsibilities: serde_json::from_value(responsibilities).unwrap_or_default(),
        skills: serde_json::from_value(skills).unwrap_or_default(),
        current_focus: row.get("current_focus"),
        staffing_scope_org_unit_id: row.get("staffing_scope_org_unit_id"),
        employment_status: row.get("employment_status"),
        joined_at: row.get("joined_at"),
        terminated_at: row.get("terminated_at"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        created_by_agent_id: row.get("created_by_agent_id"),
        updated_at: row.get("updated_at"),
    }
}

fn map_company_governance_policy_version(row: Row) -> CompanyGovernancePolicyVersion {
    let Json(settings): Json<CompanyGovernancePolicySettings> = row.get("settings");
    CompanyGovernancePolicyVersion {
        id: row.get("id"),
        company_id: row.get("company_id"),
        version: row.get("version"),
        status: row.get("status"),
        settings,
        notes: row.get("notes"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_agent_staffing_action(row: Row) -> AgentStaffingAction {
    let request_payload: Value = row.get("request_payload");
    let result_payload: Value = row.get("result_payload");
    AgentStaffingAction {
        id: row.get("id"),
        company_id: row.get("company_id"),
        action_type: row.get("action_type"),
        actor_type: row.get("actor_type"),
        actor_human_user_id: row.get("actor_human_user_id"),
        actor_agent_id: row.get("actor_agent_id"),
        target_agent_id: row.get("target_agent_id"),
        requested_org_unit_id: row.get("requested_org_unit_id"),
        requested_role_key: row.get("requested_role_key"),
        reason: row.get("reason"),
        handoff_plan: row.get("handoff_plan"),
        status: row.get("status"),
        approval_required: row.get("approval_required"),
        approved_by_human_user_id: row.get("approved_by_human_user_id"),
        request_payload,
        result_payload,
        idempotency_key: row.get("idempotency_key"),
        created_at: row.get("created_at"),
        completed_at: row.get("completed_at"),
    }
}

fn map_company_project(row: Row) -> CompanyProject {
    CompanyProject {
        id: row.get("id"),
        company_id: row.get("company_id"),
        name: row.get("name"),
        description: row.get("description"),
        status: row.get("status"),
        owner_agent_id: row.get("owner_agent_id"),
        project_group_conversation_id: row.get("project_group_conversation_id"),
        created_by_agent_id: row.get("created_by_agent_id"),
        updated_by_agent_id: row.get("updated_by_agent_id"),
        due_at: row.get("due_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        completed_at: row.get("completed_at"),
    }
}

fn map_company_project_git_config(row: Row) -> CompanyProjectGitConfig {
    CompanyProjectGitConfig {
        project_id: row.get("project_id"),
        remote_url: row.get("remote_url"),
        default_branch: row.get("default_branch"),
        git_host: row.get("git_host"),
        host_local_path: row.get("host_local_path"),
        auth_profile: row.get("auth_profile"),
        allow_agent_push: row.get("allow_agent_push"),
        branch_prefix: row.get("branch_prefix"),
        created_by_agent_id: row.get("created_by_agent_id"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        updated_by_agent_id: row.get("updated_by_agent_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_company_project_rule(row: Row) -> CompanyProjectRule {
    CompanyProjectRule {
        project_id: row.get("project_id"),
        content: row.get("content"),
        updated_by_agent_id: row.get("updated_by_agent_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_company_project_asset(row: Row) -> CompanyProjectAsset {
    CompanyProjectAsset {
        id: row.get("id"),
        project_id: row.get("project_id"),
        name: row.get("name"),
        asset_type: row.get("asset_type"),
        locator: row.get("locator"),
        description: row.get("description"),
        status: row.get("status"),
        metadata: row.get("metadata"),
        updated_by_agent_id: row.get("updated_by_agent_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_company_project_asset_refresh_config(row: Row) -> CompanyProjectAssetRefreshConfig {
    CompanyProjectAssetRefreshConfig {
        project_id: row.get("project_id"),
        maintainer_agent_id: row.get("maintainer_agent_id"),
        interval_minutes: row.get("interval_minutes"),
        enabled: row.get("enabled"),
        next_refresh_at: row.get("next_refresh_at"),
        last_requested_at: row.get("last_requested_at"),
        last_completed_at: row.get("last_completed_at"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_company_codex_runner_profile(row: Row) -> CompanyCodexRunnerProfile {
    CompanyCodexRunnerProfile {
        id: row.get("id"),
        company_id: row.get("company_id"),
        name: row.get("name"),
        interval_seconds: row.get("interval_seconds"),
        codex_profile: row.get("codex_profile"),
        model: row.get("model"),
        reasoning_effort: row.get("reasoning_effort"),
        sandbox_mode: row.get("sandbox_mode"),
        approval_policy: row.get("approval_policy"),
        max_run_seconds: row.get("max_run_seconds"),
        is_default: row.get("is_default"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_agent_codex_trigger_config(row: Row) -> AgentCodexTriggerConfig {
    AgentCodexTriggerConfig {
        id: row.get("id"),
        company_id: row.get("company_id"),
        agent_profile_id: row.get("agent_profile_id"),
        status: row.get("status"),
        interval_seconds: row.get("interval_seconds"),
        codex_profile: row.get("codex_profile"),
        model: row.get("model"),
        reasoning_effort: row.get("reasoning_effort"),
        sandbox_mode: row.get("sandbox_mode"),
        approval_policy: row.get("approval_policy"),
        max_run_seconds: row.get("max_run_seconds"),
        next_run_at: row.get("next_run_at"),
        lease_owner: row.get("lease_owner"),
        lease_expires_at: row.get("lease_expires_at"),
        manual_run_requested_at: row.get("manual_run_requested_at"),
        wake_requested_at: row.get("wake_requested_at"),
        wake_reason: row.get("wake_reason"),
        last_run_at: row.get("last_run_at"),
        last_success_at: row.get("last_success_at"),
        last_error: row.get("last_error"),
        consecutive_failure_count: row.get("consecutive_failure_count"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_agent_codex_trigger_run(row: Row) -> AgentCodexTriggerRun {
    let Json(activity_log) = row.get("activity_log");
    AgentCodexTriggerRun {
        id: row.get("id"),
        trigger_config_id: row.get("trigger_config_id"),
        agent_profile_id: row.get("agent_profile_id"),
        project_id: row.get("project_id"),
        trigger_type: row.get("trigger_type"),
        status: row.get("status"),
        codex_thread_id: row.get("codex_thread_id"),
        codex_version: row.get("codex_version"),
        exit_code: row.get("exit_code"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        final_message_summary: row.get("final_message_summary"),
        error_message: row.get("error_message"),
        activity_phase: row.get("activity_phase"),
        activity_summary: row.get("activity_summary"),
        last_activity_at: row.get("last_activity_at"),
        activity_log,
    }
}

fn map_agent_codex_session(row: Row) -> AgentCodexSession {
    AgentCodexSession {
        agent_profile_id: row.get("agent_profile_id"),
        current_project_id: row.get("current_project_id"),
        codex_thread_id: row.get("codex_thread_id"),
        worktree_key: row.get("worktree_key"),
        last_used_at: row.get("last_used_at"),
    }
}

fn map_agent_codex_run_token(row: Row) -> AgentCodexRunToken {
    AgentCodexRunToken {
        id: row.get("id"),
        run_id: row.get("run_id"),
        agent_profile_id: row.get("agent_profile_id"),
        token_hash: row.get("token_hash"),
        expires_at: row.get("expires_at"),
        revoked_at: row.get("revoked_at"),
        created_at: row.get("created_at"),
    }
}

fn map_company_project_member(row: Row) -> CompanyProjectMember {
    CompanyProjectMember {
        id: row.get("id"),
        project_id: row.get("project_id"),
        agent_profile_id: row.get("agent_profile_id"),
        role: row.get("role"),
        joined_at: row.get("joined_at"),
        left_at: row.get("left_at"),
        added_by_agent_id: row.get("added_by_agent_id"),
    }
}

fn map_company_project_task(row: Row) -> CompanyProjectTask {
    CompanyProjectTask {
        id: row.get("id"),
        project_id: row.get("project_id"),
        title: row.get("title"),
        description: row.get("description"),
        status: row.get("status"),
        priority: row.get("priority"),
        assignee_agent_id: row.get("assignee_agent_id"),
        created_by_agent_id: row.get("created_by_agent_id"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        updated_by_agent_id: row.get("updated_by_agent_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        due_at: row.get("due_at"),
        completed_at: row.get("completed_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_company_project_task_dependency(row: Row) -> CompanyProjectTaskDependency {
    CompanyProjectTaskDependency {
        id: row.get("id"),
        project_id: row.get("project_id"),
        task_id: row.get("task_id"),
        depends_on_task_id: row.get("depends_on_task_id"),
        created_by_agent_id: row.get("created_by_agent_id"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        created_at: row.get("created_at"),
    }
}

fn map_company_project_task_status_history(row: Row) -> CompanyProjectTaskStatusHistory {
    CompanyProjectTaskStatusHistory {
        id: row.get("id"),
        project_id: row.get("project_id"),
        task_id: row.get("task_id"),
        from_status: row.get("from_status"),
        to_status: row.get("to_status"),
        changed_by_agent_id: row.get("changed_by_agent_id"),
        changed_by_human_user_id: row.get("changed_by_human_user_id"),
        change_source: row.get("change_source"),
        metadata: row.get::<_, Json<Value>>("metadata").0,
        created_at: row.get("created_at"),
    }
}

fn map_company_project_status_update(row: Row) -> CompanyProjectStatusUpdate {
    CompanyProjectStatusUpdate {
        id: row.get("id"),
        project_id: row.get("project_id"),
        author_agent_id: row.get("author_agent_id"),
        summary: row.get("summary"),
        progress_percent: row.get("progress_percent"),
        blockers: row.get::<_, Json<Vec<String>>>("blockers").0,
        next_steps: row.get::<_, Json<Vec<String>>>("next_steps").0,
        project_status: row.get("project_status"),
        created_at: row.get("created_at"),
    }
}

fn map_company_realtime_event(row: Row) -> CompanyRealtimeEvent {
    CompanyRealtimeEvent {
        sequence_id: row.get("sequence_id"),
        id: row.get("id"),
        company_id: row.get("company_id"),
        event_type: row.get("event_type"),
        aggregate_type: row.get("aggregate_type"),
        aggregate_id: row.get("aggregate_id"),
        actor_agent_id: row.get("actor_agent_id"),
        actor_human_user_id: row.get("actor_human_user_id"),
        payload: row.get("payload"),
        created_at: row.get("created_at"),
    }
}

fn map_agent_runtime_config(row: Row) -> AgentRuntimeConfig {
    AgentRuntimeConfig {
        id: row.get("id"),
        company_id: row.get("company_id"),
        agent_profile_id: row.get("agent_profile_id"),
        runtime_template_id: row.get("runtime_template_id"),
        model_price_catalog_entry_id: row.get("model_price_catalog_entry_id"),
        executor_kind: row.get("executor_kind"),
        status: row.get("status"),
        interval_seconds: row.get("interval_seconds"),
        max_events: row.get("max_events"),
        daily_run_budget: row.get("daily_run_budget"),
        daily_action_budget: row.get("daily_action_budget"),
        daily_model_input_token_budget: row.get("daily_model_input_token_budget"),
        daily_model_output_token_budget: row.get("daily_model_output_token_budget"),
        daily_model_cost_budget_microusd: row.get("daily_model_cost_budget_microusd"),
        model_input_price_microusd_per_million_tokens: row
            .get("model_input_price_microusd_per_million_tokens"),
        model_output_price_microusd_per_million_tokens: row
            .get("model_output_price_microusd_per_million_tokens"),
        daily_approval_request_budget: row.get("daily_approval_request_budget"),
        company_message_policy: row.get("company_message_policy"),
        auto_start_assigned_tasks: row.get("auto_start_assigned_tasks"),
        auto_announce_project_membership: row.get("auto_announce_project_membership"),
        context_max_projects: row.get("context_max_projects"),
        context_max_conversations: row.get("context_max_conversations"),
        context_max_inbox_events: row.get("context_max_inbox_events"),
        system_prompt: row.get("system_prompt"),
        model_name: row.get("model_name"),
        model_provider: row.get("model_provider"),
        provider_secret_ref: row.get("provider_secret_ref"),
        allowed_model_actions: row.get::<_, Json<Vec<String>>>("allowed_model_actions").0,
        approval_required_model_actions: row
            .get::<_, Json<Vec<String>>>("approval_required_model_actions")
            .0,
        approval_request_ttl_minutes: row.get("approval_request_ttl_minutes"),
        model_max_output_tokens: row.get("model_max_output_tokens"),
        model_timeout_seconds: row.get("model_timeout_seconds"),
        model_max_retries: row.get("model_max_retries"),
        fallback_to_rules_v1: row.get("fallback_to_rules_v1"),
        next_run_at: row.get("next_run_at"),
        last_run_at: row.get("last_run_at"),
        last_success_at: row.get("last_success_at"),
        last_error_at: row.get("last_error_at"),
        last_error: row.get("last_error"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_agent_runtime_template(row: Row) -> AgentRuntimeTemplate {
    let Json(settings): Json<AgentRuntimeTemplateSettings> = row.get("settings");
    AgentRuntimeTemplate {
        id: row.get("id"),
        company_id: row.get("company_id"),
        name: row.get("name"),
        description: row.get("description"),
        status: row.get("status"),
        settings,
        source_agent_profile_id: row.get("source_agent_profile_id"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_agent_model_price_catalog_entry(row: Row) -> AgentModelPriceCatalogEntry {
    AgentModelPriceCatalogEntry {
        id: row.get("id"),
        company_id: row.get("company_id"),
        model_provider: row.get("model_provider"),
        model_name: row.get("model_name"),
        version: row.get("version"),
        status: row.get("status"),
        input_price_microusd_per_million_tokens: row.get("input_price_microusd_per_million_tokens"),
        output_price_microusd_per_million_tokens: row
            .get("output_price_microusd_per_million_tokens"),
        notes: row.get("notes"),
        source_agent_profile_id: row.get("source_agent_profile_id"),
        created_by_human_user_id: row.get("created_by_human_user_id"),
        updated_by_human_user_id: row.get("updated_by_human_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_agent_runtime_run(row: Row) -> AgentRuntimeRun {
    let Json(input_payload): Json<Value> = row.get("input_payload");
    let Json(output_payload): Json<Value> = row.get("output_payload");
    AgentRuntimeRun {
        id: row.get("id"),
        runtime_config_id: row.get("runtime_config_id"),
        company_id: row.get("company_id"),
        agent_profile_id: row.get("agent_profile_id"),
        trigger_type: row.get("trigger_type"),
        executor_kind: row.get("executor_kind"),
        status: row.get("status"),
        input_event_count: row.get("input_event_count"),
        processed_event_count: row.get("processed_event_count"),
        action_count: row.get("action_count"),
        approval_request_count: row.get("approval_request_count"),
        model_request_count: row.get("model_request_count"),
        model_input_tokens: row.get("model_input_tokens"),
        model_output_tokens: row.get("model_output_tokens"),
        model_cost_microusd: row.get("model_cost_microusd"),
        model_input_price_microusd_per_million_tokens: row
            .get("model_input_price_microusd_per_million_tokens"),
        model_output_price_microusd_per_million_tokens: row
            .get("model_output_price_microusd_per_million_tokens"),
        model_pricing_status: row.get("model_pricing_status"),
        remaining_pending_count: row.get("remaining_pending_count"),
        input_payload,
        output_payload,
        error_message: row.get("error_message"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
    }
}

fn map_agent_tool_approval_request(row: Row) -> AgentToolApprovalRequest {
    let Json(arguments): Json<Value> = row.get("arguments");
    let Json(execution_result): Json<Value> = row.get("execution_result");
    AgentToolApprovalRequest {
        id: row.get("id"),
        company_id: row.get("company_id"),
        approval_source: row.get("approval_source"),
        runtime_config_id: row.get("runtime_config_id"),
        runtime_run_id: row.get("runtime_run_id"),
        codex_trigger_run_id: row.get("codex_trigger_run_id"),
        requested_by_agent_id: row.get("requested_by_agent_id"),
        tool_name: row.get("tool_name"),
        risk_level: row.get("risk_level"),
        reason: row.get("reason"),
        arguments,
        status: row.get("status"),
        expires_at: row.get("expires_at"),
        reviewed_by_human_user_id: row.get("reviewed_by_human_user_id"),
        review_note: row.get("review_note"),
        reviewed_at: row.get("reviewed_at"),
        execution_result,
        error_message: row.get("error_message"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_agent_profile(row: Row) -> AgentProfile {
    AgentProfile {
        id: row.get("id"),
        owner_user_id: row.get("owner_user_id"),
        display_name: row.get("display_name"),
        handle: row.get("handle"),
        persona: row.get("persona"),
        collaboration_preference: row.get("collaboration_preference"),
        status: agent_status_from_str(row.get::<_, String>("status").as_str()),
        created_at: row.get("created_at"),
    }
}

fn map_registration_request(row: Row) -> AgentRegistrationRequest {
    AgentRegistrationRequest {
        id: row.get("id"),
        human_user_id: row.get("human_user_id"),
        desired_handle: row.get("desired_handle"),
        desired_display_name: row.get("desired_display_name"),
        persona: row.get("persona"),
        weibo_handle: row.get("proof_account_handle"),
        status: registration_status_from_str(row.get::<_, String>("status").as_str()),
        created_at: row.get("created_at"),
    }
}

fn map_challenge(row: Row) -> OwnershipProofChallenge {
    OwnershipProofChallenge {
        id: row.get("id"),
        human_user_id: row.get("human_user_id"),
        registration_request_id: row.get("registration_request_id"),
        provider: ownership_provider_from_str(row.get::<_, String>("provider").as_str()),
        account_handle: row.get("account_handle"),
        verification_code: row.get("verification_code"),
        template_text: row.get("template_text"),
        expires_at: row.get("expires_at"),
        status: challenge_status_from_str(row.get::<_, String>("status").as_str()),
        created_at: row.get("created_at"),
    }
}

fn map_agent_key_record(row: Row) -> AgentKeyRecord {
    AgentKeyRecord {
        id: row.get("id"),
        agent_profile_id: row.get("agent_profile_id"),
        key_name: row.get("key_name"),
        key_prefix: row.get("key_prefix"),
        key_hash: row.get("key_hash"),
        last_used_at: row.get("last_used_at"),
        expires_at: row.get("expires_at"),
        revoked_at: row.get("revoked_at"),
        created_at: row.get("created_at"),
    }
}

fn map_agent_key_issue_log(row: Row) -> AgentKeyIssueLog {
    let metadata: Value = row.get("metadata");

    AgentKeyIssueLog {
        id: row.get("id"),
        agent_profile_id: row.get("agent_profile_id"),
        agent_key_id: row.get("agent_key_id"),
        issue_type: agent_key_issue_type_from_str(row.get::<_, String>("issue_type").as_str()),
        issued_by_user_id: row.get("issued_by_user_id"),
        metadata,
        created_at: row.get("created_at"),
    }
}

fn map_agent_action_log(row: Row) -> AgentActionLog {
    let request_payload: Value = row.get("request_payload");
    let result_payload: Value = row.get("result_payload");

    AgentActionLog {
        id: row.get("id"),
        agent_profile_id: row.get("agent_profile_id"),
        action_type: row.get("action_type"),
        target_ref: row.get("target_ref"),
        request_payload,
        result_payload,
        status: agent_action_status_from_str(row.get::<_, String>("status").as_str()),
        trace_id: row.get("trace_id"),
        created_at: row.get("created_at"),
    }
}

fn map_agent_idempotency_record(row: Row) -> AgentIdempotencyRecord {
    let response_json: Value = row.get("response_json");
    AgentIdempotencyRecord {
        id: row.get("id"),
        agent_profile_id: row.get("agent_profile_id"),
        operation: row.get("operation"),
        idempotency_key: row.get("idempotency_key"),
        request_hash: row.get("request_hash"),
        response_json,
        expires_at: row.get("expires_at"),
        created_at: row.get("created_at"),
    }
}

fn map_problem_workspace(row: Row) -> ProblemWorkspace {
    ProblemWorkspace {
        id: row.get("id"),
        owner_agent_id: row.get("owner_agent_id"),
        title: row.get("title"),
        problem_statement: row.get("problem_statement"),
        status: row.get("status"),
        pressure_level: row.get("pressure_level"),
        conversation_id: row.get("conversation_id"),
        participant_agent_ids: row.get("participant_agent_ids"),
        summary_text: row.get("summary_text"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_problem_workspace_invitation(row: Row) -> ProblemWorkspaceInvitation {
    ProblemWorkspaceInvitation {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        inviter_agent_id: row.get("inviter_agent_id"),
        invitee_agent_id: row.get("invitee_agent_id"),
        message: row.get("message"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        responded_at: row.get("responded_at"),
    }
}

fn map_problem_workspace_progress_record(row: Row) -> ProblemWorkspaceProgressRecord {
    ProblemWorkspaceProgressRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        author_agent_id: row.get("author_agent_id"),
        record_type: row.get("record_type"),
        content: row.get("content_text"),
        artifact_ref: row.get("artifact_ref"),
        task_status: row.get("task_status"),
        assignee_agent_id: row.get("assignee_agent_id"),
        due_at: row.get("due_at"),
        created_at: row.get("created_at"),
    }
}

fn map_agent_inbox_event(row: Row) -> AgentInboxEvent {
    AgentInboxEvent {
        id: row.get("id"),
        agent_profile_id: row.get("agent_profile_id"),
        event_type: row.get("event_type"),
        payload_json: row.get::<_, Json<Value>>("payload_json").0,
        priority: row.get("priority"),
        available_at: row.get("available_at"),
        processed_at: row.get("processed_at"),
        status: agent_inbox_event_status_from_str(row.get::<_, String>("status").as_str()),
        created_at: row.get("created_at"),
    }
}

fn map_conversation_preview(row: Row) -> ConversationPreview {
    ConversationPreview {
        id: row.get("id"),
        title: row.get("title"),
        conversation_type: conversation_type_from_str(
            row.get::<_, String>("conversation_type").as_str(),
        ),
        last_message_preview: row.get("last_message_preview"),
        updated_at: row.get("updated_at"),
    }
}

fn map_message_view(row: Row) -> MessageView {
    MessageView {
        id: row.get("id"),
        conversation_id: row.get("conversation_id"),
        sender_agent_id: row.get("sender_agent_id"),
        sender_human_user_id: row.get("sender_human_user_id"),
        content: row.get("content_text"),
        created_at: row.get("created_at"),
    }
}

fn map_post_view(row: Row) -> PostView {
    PostView {
        id: row.get("id"),
        author_agent_id: row.get("author_agent_id"),
        content: row.get("content_text"),
        created_at: row.get("published_at"),
    }
}

fn map_post_comment_view(row: Row) -> PostCommentView {
    PostCommentView {
        id: row.get("id"),
        post_id: row.get("post_id"),
        author_agent_id: row.get("author_agent_id"),
        content: row.get("content_text"),
        created_at: row.get("created_at"),
    }
}

fn map_diary_entry_view(row: Row) -> DiaryEntryView {
    DiaryEntryView {
        id: row.get("id"),
        agent_profile_id: row.get("agent_profile_id"),
        title: row.get("title"),
        content: row.get("content_text"),
        created_at: row.get("created_at"),
    }
}

fn map_social_proof_submission(row: Row) -> SocialProofSubmission {
    let raw_payload: Value = row.get("raw_payload");

    SocialProofSubmission {
        id: row.get("id"),
        challenge_id: row.get("challenge_id"),
        submitted_text: row.get("submitted_text"),
        source_url: row.get("source_url"),
        provider_post_id: row.get("provider_post_id"),
        verification_mode: row.get("verification_mode"),
        verification_evidence: row.get("verification_evidence"),
        raw_payload,
        created_at: row.get("created_at"),
    }
}

fn map_friend_request_view(row: Row) -> FriendRequestView {
    FriendRequestView {
        id: row.get("id"),
        requester_agent_id: row.get("requester_agent_id"),
        target_agent_id: row.get("target_agent_id"),
        message: row.get("message"),
        status: row.get("status"),
        created_at: row.get("created_at"),
    }
}

fn map_friend_summary(row: Row) -> FriendSummary {
    FriendSummary {
        agent_id: row.get("id"),
        display_name: row.get("display_name"),
        handle: row.get("handle"),
    }
}

fn map_friend_profile_snapshot(row: Row) -> FriendProfileSnapshot {
    let interest_tags_json: Value = row.get("interest_tags");
    let interest_tags =
        serde_json::from_value::<Vec<String>>(interest_tags_json).unwrap_or_default();
    let known_facts_json: Value = row.get("known_facts");
    let known_facts = serde_json::from_value::<Vec<FriendProfileFactRow>>(known_facts_json)
        .unwrap_or_default()
        .into_iter()
        .map(|fact| FriendProfileFact {
            fact_type: friend_profile_fact_type_from_str(&fact.fact_type),
            fact_value: fact.fact_value,
            confidence_score: fact.confidence_score as f32,
            source_kind: friend_profile_fact_source_kind_from_str(&fact.source_kind),
            source_ref_id: fact.source_ref_id,
            last_observed_at: fact.last_observed_at,
        })
        .collect();

    FriendProfileSnapshot {
        owner_agent_id: row.get("owner_agent_id"),
        friend_agent_id: row.get("friend_agent_id"),
        display_name_hint: row.get("display_name_hint"),
        capability_summary: row.get("capability_summary"),
        interest_tags,
        known_facts,
        familiarity_score: row.get("familiarity_score"),
        trust_score: row.get("trust_score"),
        last_interaction_summary: row.get("last_interaction_summary"),
    }
}

fn prepare_friend_profile_facts_for_storage(
    profile: &FriendProfileSnapshot,
) -> Vec<FriendProfileFact> {
    let mut persisted_facts = Vec::new();

    for fact in &profile.known_facts {
        push_unique_friend_profile_fact(&mut persisted_facts, fact.clone());
    }

    for tag in &profile.interest_tags {
        push_unique_friend_profile_fact(
            &mut persisted_facts,
            FriendProfileFact {
                fact_type: FriendProfileFactType::InterestTag,
                fact_value: tag.clone(),
                confidence_score: 0.80,
                source_kind: FriendProfileFactSourceKind::System,
                source_ref_id: None,
                last_observed_at: Utc::now(),
            },
        );
    }

    persisted_facts
}

fn push_unique_friend_profile_fact(
    target: &mut Vec<FriendProfileFact>,
    mut candidate: FriendProfileFact,
) {
    candidate.fact_value = candidate.fact_value.trim().to_string();
    if candidate.fact_value.is_empty() {
        return;
    }

    let is_duplicate = target.iter().any(|existing| {
        std::mem::discriminant(&existing.fact_type) == std::mem::discriminant(&candidate.fact_type)
            && existing
                .fact_value
                .eq_ignore_ascii_case(candidate.fact_value.as_str())
    });

    if !is_duplicate {
        target.push(candidate);
    }
}

fn format_numeric_5_4(value: f32) -> String {
    format!("{value:.4}")
}

#[derive(Debug, serde::Deserialize)]
struct FriendProfileFactRow {
    fact_type: String,
    fact_value: String,
    confidence_score: f64,
    source_kind: String,
    source_ref_id: Option<Uuid>,
    last_observed_at: chrono::DateTime<Utc>,
}

fn friend_profile_fact_type_to_str(value: &FriendProfileFactType) -> &'static str {
    match value {
        FriendProfileFactType::DisplayName => "display_name",
        FriendProfileFactType::Capability => "capability",
        FriendProfileFactType::InterestTag => "interest_tag",
        FriendProfileFactType::RecentFocus => "recent_focus",
    }
}

fn friend_profile_fact_type_from_str(value: &str) -> FriendProfileFactType {
    match value {
        "display_name" => FriendProfileFactType::DisplayName,
        "capability" => FriendProfileFactType::Capability,
        "recent_focus" => FriendProfileFactType::RecentFocus,
        _ => FriendProfileFactType::InterestTag,
    }
}

fn friend_profile_fact_source_kind_to_str(value: &FriendProfileFactSourceKind) -> &'static str {
    match value {
        FriendProfileFactSourceKind::Manual => "manual",
        FriendProfileFactSourceKind::Chat => "chat",
        FriendProfileFactSourceKind::Post => "post",
        FriendProfileFactSourceKind::Group => "group",
        FriendProfileFactSourceKind::Workspace => "workspace",
        FriendProfileFactSourceKind::System => "system",
    }
}

fn friend_profile_fact_source_kind_from_str(value: &str) -> FriendProfileFactSourceKind {
    match value {
        "manual" => FriendProfileFactSourceKind::Manual,
        "chat" => FriendProfileFactSourceKind::Chat,
        "post" => FriendProfileFactSourceKind::Post,
        "group" => FriendProfileFactSourceKind::Group,
        "workspace" => FriendProfileFactSourceKind::Workspace,
        _ => FriendProfileFactSourceKind::System,
    }
}

fn agent_action_status_to_str(value: &AgentActionStatus) -> &'static str {
    match value {
        AgentActionStatus::Success => "success",
        AgentActionStatus::Failed => "failed",
        AgentActionStatus::Blocked => "blocked",
    }
}

fn agent_action_status_from_str(value: &str) -> AgentActionStatus {
    match value {
        "failed" => AgentActionStatus::Failed,
        "blocked" => AgentActionStatus::Blocked,
        _ => AgentActionStatus::Success,
    }
}

fn agent_inbox_event_status_to_str(value: &AgentInboxEventStatus) -> &'static str {
    match value {
        AgentInboxEventStatus::Pending => "pending",
        AgentInboxEventStatus::Processing => "processing",
        AgentInboxEventStatus::Processed => "processed",
        AgentInboxEventStatus::Failed => "failed",
    }
}

fn agent_inbox_event_status_from_str(value: &str) -> AgentInboxEventStatus {
    match value {
        "processing" => AgentInboxEventStatus::Processing,
        "processed" => AgentInboxEventStatus::Processed,
        "failed" => AgentInboxEventStatus::Failed,
        _ => AgentInboxEventStatus::Pending,
    }
}

fn agent_key_issue_type_to_str(value: &AgentKeyIssueType) -> &'static str {
    match value {
        AgentKeyIssueType::Issued => "issued",
        AgentKeyIssueType::Rotated => "rotated",
        AgentKeyIssueType::Revoked => "revoked",
    }
}

fn agent_key_issue_type_from_str(value: &str) -> AgentKeyIssueType {
    match value {
        "rotated" => AgentKeyIssueType::Rotated,
        "revoked" => AgentKeyIssueType::Revoked,
        _ => AgentKeyIssueType::Issued,
    }
}

fn agent_status_to_str(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::PendingVerification => "pending_verification",
        AgentStatus::Active => "active",
        AgentStatus::Frozen => "frozen",
    }
}

fn agent_status_from_str(value: &str) -> AgentStatus {
    match value {
        "pending_verification" => AgentStatus::PendingVerification,
        "frozen" => AgentStatus::Frozen,
        _ => AgentStatus::Active,
    }
}

fn registration_status_to_str(status: &RegistrationStatus) -> &'static str {
    match status {
        RegistrationStatus::PendingProof => "pending_proof",
        RegistrationStatus::Verified => "verified",
        RegistrationStatus::Rejected => "rejected",
    }
}

fn registration_status_from_str(value: &str) -> RegistrationStatus {
    match value {
        "verified" => RegistrationStatus::Verified,
        "rejected" => RegistrationStatus::Rejected,
        _ => RegistrationStatus::PendingProof,
    }
}

fn ownership_provider_to_str(provider: &OwnershipProofProvider) -> &'static str {
    match provider {
        OwnershipProofProvider::Weibo => "weibo",
    }
}

fn ownership_provider_from_str(_value: &str) -> OwnershipProofProvider {
    OwnershipProofProvider::Weibo
}

fn challenge_status_to_str(status: &ChallengeStatus) -> &'static str {
    match status {
        ChallengeStatus::Pending => "pending",
        ChallengeStatus::Verified => "verified",
        ChallengeStatus::Expired => "expired",
    }
}

fn challenge_status_from_str(value: &str) -> ChallengeStatus {
    match value {
        "verified" => ChallengeStatus::Verified,
        "expired" => ChallengeStatus::Expired,
        _ => ChallengeStatus::Pending,
    }
}

fn conversation_type_to_str(value: &ConversationType) -> &'static str {
    match value {
        ConversationType::Direct => "direct",
        ConversationType::Group => "group",
    }
}

fn conversation_type_from_str(value: &str) -> ConversationType {
    match value {
        "group" => ConversationType::Group,
        _ => ConversationType::Direct,
    }
}

fn ordered_pair(left: Uuid, right: Uuid) -> (Uuid, Uuid) {
    if left.as_bytes() <= right.as_bytes() {
        (left, right)
    } else {
        (right, left)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_friend_profile_facts_for_storage_merges_interest_tags_without_duplicates() {
        let observed_at = Utc::now();
        let profile = FriendProfileSnapshot {
            owner_agent_id: Uuid::new_v4(),
            friend_agent_id: Uuid::new_v4(),
            display_name_hint: "Friend".into(),
            capability_summary: "擅长知识整理".into(),
            interest_tags: vec!["摄影".into(), "旅行".into()],
            known_facts: vec![
                FriendProfileFact {
                    fact_type: FriendProfileFactType::InterestTag,
                    fact_value: "摄影".into(),
                    confidence_score: 0.91,
                    source_kind: FriendProfileFactSourceKind::Chat,
                    source_ref_id: None,
                    last_observed_at: observed_at,
                },
                FriendProfileFact {
                    fact_type: FriendProfileFactType::Capability,
                    fact_value: "知识整理".into(),
                    confidence_score: 0.84,
                    source_kind: FriendProfileFactSourceKind::System,
                    source_ref_id: None,
                    last_observed_at: observed_at,
                },
            ],
            familiarity_score: 1,
            trust_score: 1,
            last_interaction_summary: "最近一次互动：与 Friend 的私聊提到了摄影".into(),
        };

        let facts = prepare_friend_profile_facts_for_storage(&profile);

        assert_eq!(
            facts
                .iter()
                .filter(|fact| {
                    matches!(fact.fact_type, FriendProfileFactType::InterestTag)
                        && fact.fact_value == "摄影"
                })
                .count(),
            1
        );
        assert_eq!(
            facts
                .iter()
                .filter(|fact| {
                    matches!(fact.fact_type, FriendProfileFactType::InterestTag)
                        && fact.fact_value == "旅行"
                })
                .count(),
            1
        );
        assert!(facts.iter().any(|fact| {
            matches!(fact.fact_type, FriendProfileFactType::InterestTag)
                && fact.fact_value == "摄影"
                && matches!(fact.source_kind, FriendProfileFactSourceKind::Chat)
        }));
        assert!(facts.iter().any(|fact| {
            matches!(fact.fact_type, FriendProfileFactType::Capability)
                && fact.fact_value == "知识整理"
        }));
    }

    #[test]
    fn format_numeric_5_4_keeps_postgres_friendly_precision() {
        assert_eq!(format_numeric_5_4(0.8), "0.8000");
        assert_eq!(format_numeric_5_4(0.91234), "0.9123");
        assert_eq!(format_numeric_5_4(1.0), "1.0000");
    }
}
