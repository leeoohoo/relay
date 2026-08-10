use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use ai_chat_domain::agent_identity::{
    AgentKeyIssueLog, AgentKeyRecord, AgentOwnerBinding, AgentProfile,
};
use ai_chat_domain::company::{
    AgentStaffingAction, Company, CompanyAgentMembership, CompanyGovernancePolicySettings,
    CompanyGovernancePolicyVersion, CompanyHumanMember, CompanyProfession, CompanyProjectTask,
    CompanyProjectTypeDefinition, OrgUnit,
};
use ai_chat_domain::social::ConversationPreview;

use super::{CompanyConversationView, CompanyProjectView};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorPage<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<Uuid>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCompanyInput {
    pub human_user_id: Uuid,
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CompanyCreationBundle {
    pub company: Company,
    pub owner_membership: CompanyHumanMember,
    pub root_org_unit: OrgUnit,
    pub default_group: CompanyConversationView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCompanyAgentInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub display_name: String,
    pub handle: String,
    pub persona: String,
    pub org_unit_id: Option<Uuid>,
    pub job_title: Option<String>,
    pub role_key: Option<String>,
    pub reports_to_membership_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrgUnitInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub parent_org_unit_id: Option<Uuid>,
    pub name: String,
    pub unit_type: String,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCompanyAgentResult {
    pub company: Company,
    pub agent_profile: AgentProfile,
    pub membership: CompanyAgentMembership,
    pub agent_key_plaintext: String,
    pub agent_key_prefix: String,
}

#[derive(Debug, Clone)]
pub struct CompanyAgentCreationBundle {
    pub agent_profile: AgentProfile,
    pub owner_binding: AgentOwnerBinding,
    pub membership: CompanyAgentMembership,
    pub key_record: AgentKeyRecord,
    pub key_issue_log: AgentKeyIssueLog,
    pub self_notes_conversation: ConversationPreview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCompanyAgentPermissionsInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub agent_id: Uuid,
    pub staffing_permissions: Vec<String>,
    pub project_permissions: Vec<String>,
    pub staffing_scope_org_unit_id: Option<Uuid>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCompanyAgentRoleInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub agent_id: Uuid,
    pub role_key: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCompanyAgentProfessionInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub agent_id: Uuid,
    pub profession_key: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStaffingHireInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub display_name: String,
    pub handle: String,
    pub persona: String,
    pub org_unit_id: Option<Uuid>,
    pub job_title: Option<String>,
    pub reports_to_membership_id: Option<Uuid>,
    pub reason: Option<String>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStaffingStatusInput {
    pub actor_agent_id: Uuid,
    pub company_id: Uuid,
    pub target_agent_id: Uuid,
    pub reason: Option<String>,
    pub handoff_plan: Option<String>,
    pub handoff_agent_id: Option<Uuid>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanCompanyStaffingStatusInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub target_agent_id: Uuid,
    pub reason: Option<String>,
    pub handoff_plan: Option<String>,
    pub handoff_agent_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStaffingHireResult {
    pub action: AgentStaffingAction,
    pub agent_profile: AgentProfile,
    pub membership: CompanyAgentMembership,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyAgentActivationResult {
    pub action: AgentStaffingAction,
    pub agent_profile: AgentProfile,
    pub membership: CompanyAgentMembership,
    pub agent_key_plaintext: String,
    pub agent_key_prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStaffingStatusResult {
    pub action: AgentStaffingAction,
    pub agent_profile: AgentProfile,
    pub membership: CompanyAgentMembership,
    pub revoked_key_count: usize,
    pub reassigned_task_count: usize,
}

#[derive(Debug, Clone)]
pub struct AgentStaffingHireBundle {
    pub agent_profile: AgentProfile,
    pub owner_binding: AgentOwnerBinding,
    pub membership: CompanyAgentMembership,
    pub key_record: AgentKeyRecord,
    pub key_issue_log: AgentKeyIssueLog,
    pub self_notes_conversation: ConversationPreview,
    pub action: AgentStaffingAction,
}

#[derive(Debug, Clone)]
pub struct CompanyAgentMembershipUpdateBundle {
    pub membership: CompanyAgentMembership,
    pub action: AgentStaffingAction,
}

#[derive(Debug, Clone)]
pub struct CompanyAgentActivationBundle {
    pub agent_profile: AgentProfile,
    pub membership: CompanyAgentMembership,
    pub key_record: AgentKeyRecord,
    pub key_issue_log: AgentKeyIssueLog,
    pub action: AgentStaffingAction,
}

#[derive(Debug, Clone)]
pub struct AgentStaffingStatusChangeBundle {
    pub agent_profile: AgentProfile,
    pub membership: CompanyAgentMembership,
    pub key_issue_logs: Vec<AgentKeyIssueLog>,
    pub reassigned_tasks: Vec<CompanyProjectTask>,
    pub action: AgentStaffingAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyAgentView {
    pub agent_profile: AgentProfile,
    pub membership: CompanyAgentMembership,
    pub profession: CompanyProfession,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyAgentConnectionView {
    pub status: String,
    pub key_prefix: Option<String>,
    pub key_created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub key_expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProfessionSummary {
    pub key: String,
    pub label: String,
    pub label_en: String,
    pub description: String,
    pub description_en: String,
    pub category_key: String,
    pub category_label: String,
    pub category_label_en: String,
    pub skill_name: String,
    pub can_create_tasks: bool,
}

impl From<CompanyProfession> for CompanyProfessionSummary {
    fn from(profession: CompanyProfession) -> Self {
        Self {
            key: profession.key,
            label: profession.label,
            label_en: profession.label_en,
            description: profession.description,
            description_en: profession.description_en,
            category_key: profession.category_key,
            category_label: profession.category_label,
            category_label_en: profession.category_label_en,
            skill_name: profession.skill_name,
            can_create_tasks: profession.can_create_tasks,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProjectTypeSummary {
    pub key: String,
    pub label: String,
    pub label_en: String,
    pub description: String,
    pub description_en: String,
    pub category_key: String,
    pub category_label: String,
    pub category_label_en: String,
}

impl From<CompanyProjectTypeDefinition> for CompanyProjectTypeSummary {
    fn from(project_type: CompanyProjectTypeDefinition) -> Self {
        Self {
            key: project_type.key,
            label: project_type.label,
            label_en: project_type.label_en,
            description: project_type.description,
            description_en: project_type.description_en,
            category_key: project_type.category_key,
            category_label: project_type.category_label,
            category_label_en: project_type.category_label_en,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyConsoleAgentView {
    pub agent_profile: AgentProfile,
    pub membership: CompanyAgentMembership,
    pub profession: CompanyProfessionSummary,
    pub connection: CompanyAgentConnectionView,
}

pub struct PublishCompanyGovernancePolicyInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub settings: CompanyGovernancePolicySettings,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyGovernancePolicyView {
    pub company_id: Uuid,
    pub configured: bool,
    pub active_version: Option<CompanyGovernancePolicyVersion>,
    pub effective_settings: CompanyGovernancePolicySettings,
    pub versions: Vec<CompanyGovernancePolicyVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyConsoleView {
    pub company: Company,
    pub human_membership: CompanyHumanMember,
    pub org_units: Vec<OrgUnit>,
    pub agents: Vec<CompanyConsoleAgentView>,
    pub conversations: Vec<CompanyConversationView>,
    pub projects: Vec<CompanyProjectView>,
    pub professions: Vec<CompanyProfessionSummary>,
    pub project_types: Vec<CompanyProjectTypeSummary>,
    pub governance_policy: CompanyGovernancePolicyView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanySummaryView {
    pub company: Company,
    pub human_membership: CompanyHumanMember,
    pub org_units: Vec<OrgUnit>,
    pub professions: Vec<CompanyProfessionSummary>,
    pub project_types: Vec<CompanyProjectTypeSummary>,
    pub governance_policy: CompanyGovernancePolicyView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanySkillCatalogView {
    pub professions: Vec<CompanyProfession>,
    pub project_types: Vec<CompanyProjectTypeDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewAgentToolApprovalInput {
    pub human_user_id: Uuid,
    pub company_id: Uuid,
    pub approval_request_id: Uuid,
    pub review_note: Option<String>,
    pub approval_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentToolApprovalProposal {
    pub(crate) tool: String,
    pub(crate) target_agent_id: Option<Uuid>,
    pub(crate) project_id: Option<Uuid>,
    pub(crate) task_id: Option<Uuid>,
    pub(crate) assignee_agent_id: Option<Uuid>,
    pub(crate) display_name: Option<String>,
    pub(crate) handle: Option<String>,
    pub(crate) persona: Option<String>,
    pub(crate) org_unit_id: Option<Uuid>,
    pub(crate) job_title: Option<String>,
    pub(crate) reports_to_membership_id: Option<Uuid>,
    pub(crate) handoff_plan: Option<String>,
    pub(crate) handoff_agent_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCodexApprovalRequestInput {
    pub company_id: Uuid,
    pub codex_trigger_run_id: Uuid,
    pub requested_by_agent_id: Uuid,
    pub tool_name: String,
    pub risk_level: String,
    pub reason: String,
    pub arguments: serde_json::Value,
    pub expires_at: DateTime<Utc>,
}
