use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use ai_chat_domain::agent_identity::AgentProfile;
use ai_chat_domain::company::{
    CompanyProject, CompanyProjectAsset, CompanyProjectAssetRefreshConfig, CompanyProjectMember,
    CompanyProjectRule, CompanyProjectStatusUpdate, CompanyProjectTask,
    CompanyProjectTaskDependency, CompanyProjectTaskStatusHistory,
};

use super::{CompanyConversationMemberPreview, CompanyConversationView};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProjectMemberView {
    pub member: CompanyProjectMember,
    pub agent_profile: AgentProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProjectView {
    pub project: CompanyProject,
    pub git: Option<CompanyProjectGitView>,
    pub rule: Option<CompanyProjectRule>,
    pub assets: Vec<CompanyProjectAsset>,
    pub asset_refresh: Option<CompanyProjectAssetRefreshConfig>,
    pub members: Vec<CompanyProjectMemberView>,
    pub tasks: Vec<CompanyProjectTask>,
    pub task_dependencies: Vec<CompanyProjectTaskDependency>,
    pub task_status_history: Vec<CompanyProjectTaskStatusHistory>,
    pub status_updates: Vec<CompanyProjectStatusUpdate>,
    pub project_group: CompanyConversationView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProjectGitView {
    pub remote_url: String,
    pub default_branch: String,
    pub git_host: String,
    pub push_enabled: bool,
    pub branch_prefix: String,
    pub auth_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProjectGitAdminView {
    pub remote_url: String,
    pub default_branch: String,
    pub git_host: String,
    pub host_local_path: String,
    pub auth_profile: Option<String>,
    pub allow_agent_push: bool,
    pub branch_prefix: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CompanyProjectCreationBundle {
    pub project: CompanyProject,
    pub members: Vec<CompanyProjectMember>,
    pub conversation_members: Vec<CompanyConversationMemberPreview>,
}

#[derive(Debug, Clone)]
pub struct ManagedCompanyProjectCreationBundle {
    pub project_creation: CompanyProjectCreationBundle,
    pub git_config: ai_chat_domain::company::CompanyProjectGitConfig,
    pub cleanup_job_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectProvisioningCleanupJob {
    pub id: Uuid,
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub managed_local_path: String,
    pub repository_identifier: String,
    pub access_token_identifier: String,
    pub status: String,
    pub attempts: i32,
    pub next_attempt_at: DateTime<Utc>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct CompanyProjectMemberAddBundle {
    pub member: CompanyProjectMember,
    pub conversation_preview: CompanyConversationMemberPreview,
}

#[derive(Debug, Clone)]
pub struct CompanyProjectOwnerTransferBundle {
    pub project: CompanyProject,
    pub previous_owner_agent_id: Uuid,
    pub new_owner_member: CompanyProjectMember,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCompanyProjectInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub member_agent_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCompanyProjectForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub owner_agent_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub member_agent_ids: Vec<Uuid>,
    pub project_type: Option<String>,
    pub project_type_source: Option<String>,
    pub project_type_confidence: Option<i32>,
    pub project_type_evidence: Vec<String>,
    pub project_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateManagedCompanyProjectForHumanInput {
    pub project: CreateCompanyProjectForHumanInput,
    pub cleanup_job_id: Uuid,
    pub remote_url: String,
    pub host_local_path: String,
    pub default_branch: String,
    pub auth_profile: String,
    pub allow_agent_push: bool,
    pub branch_prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCompanyProjectInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub name: Option<String>,
    pub description: Option<String>,
    pub due_at: Option<chrono::DateTime<chrono::Utc>>,
    pub clear_due_at: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCompanyProjectPauseForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferCompanyProjectOwnerInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub owner_agent_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferCompanyProjectOwnerForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub owner_agent_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCompanyProjectGitForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertCompanyProjectGitForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub remote_url: String,
    pub host_local_path: Option<String>,
    pub default_branch: Option<String>,
    pub auth_profile: Option<String>,
    pub allow_agent_push: Option<bool>,
    pub branch_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigureManagedLocalProjectGitForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub managed_local_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertCompanyProjectGitInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub remote_url: String,
    pub host_local_path: Option<String>,
    pub default_branch: Option<String>,
    pub auth_profile: Option<String>,
    pub allow_agent_push: Option<bool>,
    pub branch_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCompanyProjectRuleInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCompanyProjectRuleForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestCompanyProjectRuleGenerationForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub agent_id: Uuid,
    pub instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProjectAssetInput {
    pub name: String,
    pub asset_type: String,
    pub locator: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceCompanyProjectAssetsInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub assets: Vec<CompanyProjectAssetInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertCompanyProjectAssetRefreshForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
    pub maintainer_agent_id: Uuid,
    pub interval_minutes: i32,
    pub enabled: bool,
    pub run_now: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct DeleteCompanyProjectGitForHumanInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
}
