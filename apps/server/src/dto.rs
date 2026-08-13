use super::*;

#[derive(Debug, Serialize)]
pub(super) struct HealthResponse {
    pub(super) status: &'static str,
    pub(super) service: &'static str,
}

#[derive(Debug, Serialize)]
pub(super) struct ReadinessResponse {
    pub(super) status: &'static str,
    pub(super) repository: &'static str,
}

#[derive(Debug, Serialize)]
pub(super) struct RuntimeConfigResponse {
    pub(super) dev_endpoints_enabled: bool,
    pub(super) admin_token_configured: bool,
    pub(super) email_verification_required: bool,
    pub(super) harness_mode: &'static str,
    pub(super) project_types: Vec<CompanyProjectTypeDefinition>,
}

#[derive(Debug, Deserialize)]
pub(super) struct HumanEmailInput {
    pub(super) email: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct HumanAccountTokenInput {
    pub(super) token: String,
}

#[derive(Debug, Serialize)]
pub(super) struct EmailDeliveryPayload {
    pub(super) to: String,
    pub(super) template: &'static str,
    pub(super) action_url: String,
    pub(super) expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub(super) struct ApiErrorResponse {
    pub(super) code: String,
    pub(super) message: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateCompanyRequest {
    pub(super) name: String,
    pub(super) slug: Option<String>,
    pub(super) description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ConsolePageQuery {
    pub(super) limit: Option<usize>,
    pub(super) after: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CodexRuntimeOverviewQuery {
    pub(super) agent_ids: String,
    pub(super) project_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateCompanyAgentRequest {
    pub(super) display_name: String,
    pub(super) org_unit_id: Option<Uuid>,
    pub(super) profession_key: String,
    pub(super) role_key: Option<String>,
    pub(super) reports_to_membership_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateOrgUnitRequest {
    pub(super) parent_org_unit_id: Option<Uuid>,
    pub(super) name: String,
    pub(super) unit_type: String,
    pub(super) sort_order: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub(super) struct OpenHumanDirectConversationRequest {
    pub(super) target_agent_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub(super) struct SendHumanCompanyMessageRequest {
    pub(super) content: String,
    #[serde(default)]
    pub(super) mentioned_agent_ids: Vec<Uuid>,
    #[serde(default)]
    pub(super) mention_all: bool,
    #[serde(default)]
    pub(super) folder_references: Vec<HumanFolderReferenceRequest>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct HumanFolderReferenceRequest {
    pub(super) local_path: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateCompanyAgentPermissionsRequest {
    #[serde(default)]
    pub(super) staffing_permissions: Vec<String>,
    #[serde(default)]
    pub(super) project_permissions: Vec<String>,
    pub(super) staffing_scope_org_unit_id: Option<Uuid>,
    pub(super) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateCompanyProjectRuleRequest {
    pub(super) content: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct RequestCompanyProjectRuleGenerationRequest {
    pub(super) agent_id: Uuid,
    pub(super) instructions: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpsertCompanyProjectAssetRefreshRequest {
    pub(super) maintainer_agent_id: Uuid,
    pub(super) interval_minutes: i32,
    pub(super) enabled: bool,
    #[serde(default)]
    pub(super) run_now: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateCompanyAgentRoleRequest {
    pub(super) role_key: String,
    pub(super) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateCompanyAgentProfessionRequest {
    pub(super) profession_key: String,
    pub(super) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CompanyAgentStaffingStatusRequest {
    pub(super) reason: Option<String>,
    pub(super) handoff_plan: Option<String>,
    pub(super) handoff_agent_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PublishCompanyGovernancePolicyRequest {
    pub(super) agent_staff_limit: i32,
    pub(super) delegated_agent_hiring_enabled: bool,
    pub(super) delegated_agent_suspension_enabled: bool,
    pub(super) delegated_agent_termination_enabled: bool,
    pub(super) max_active_projects: i32,
    pub(super) max_project_members: i32,
    pub(super) daily_delegated_hire_limit: Option<i32>,
    pub(super) daily_delegated_suspension_limit: Option<i32>,
    pub(super) daily_delegated_termination_limit: Option<i32>,
    pub(super) skill_language: Option<String>,
    pub(super) notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateCompanyProjectRequest {
    pub(super) name: String,
    pub(super) description: Option<String>,
    pub(super) owner_agent_id: Uuid,
    #[serde(default)]
    pub(super) member_agent_ids: Vec<Uuid>,
    pub(super) project_type: Option<String>,
    pub(super) source_kind: String,
    pub(super) source_local_path: Option<String>,
    pub(super) git_remote_url: Option<String>,
    pub(super) default_branch: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TransferCompanyProjectOwnerRequest {
    pub(super) owner_agent_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub(super) struct ImportCompanyProjectFolderRequest {
    pub(super) name: String,
    pub(super) description: Option<String>,
    pub(super) owner_agent_id: Uuid,
    #[serde(default)]
    pub(super) member_agent_ids: Vec<Uuid>,
    pub(super) project_type: Option<String>,
    #[serde(default)]
    pub(super) file_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateCompanyWorkspaceRequest {
    pub(super) managed_workspace_root: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateCompanySkillLanguageRequest {
    pub(super) skill_language: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateAgentTriggerPreferencesRequest {
    pub(super) batch_size: usize,
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateCompanyProjectTaskRequest {
    pub(super) title: String,
    pub(super) description: Option<String>,
    pub(super) priority: Option<String>,
    pub(super) assignee_agent_id: Option<Uuid>,
    pub(super) due_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub(super) depends_on_task_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub(super) struct OpenProjectTaskBlockerRequest {
    pub(super) attempt_id: Option<Uuid>,
    pub(super) blocker_type: String,
    pub(super) summary: String,
    pub(super) owner_agent_id: Option<Uuid>,
    pub(super) resolution_condition: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ResolveProjectTaskBlockerRequest {
    pub(super) status: String,
    pub(super) resolution_summary: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct OpenProjectDiscussionThreadRequest {
    pub(super) scope_type: String,
    pub(super) subject_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub(super) struct AddProjectTaskRelationRequest {
    pub(super) target_task_id: Uuid,
    pub(super) relation_type: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateProjectEvidenceRequest {
    pub(super) attempt_id: Option<Uuid>,
    pub(super) gate_id: Option<Uuid>,
    pub(super) environment_id: Option<Uuid>,
    pub(super) evidence_type: String,
    pub(super) title: String,
    pub(super) summary: String,
    pub(super) result: String,
    #[serde(default)]
    pub(super) artifact_refs: Vec<serde_json::Value>,
    #[serde(default)]
    pub(super) metrics: serde_json::Value,
    pub(super) dedupe_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateCompanyProjectTaskRequest {
    pub(super) title: Option<String>,
    pub(super) description: Option<String>,
    pub(super) status: Option<String>,
    pub(super) priority: Option<String>,
    pub(super) assignee_agent_id: Option<Uuid>,
    #[serde(default)]
    pub(super) clear_assignee: bool,
    pub(super) due_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub(super) clear_due_at: bool,
    pub(super) depends_on_task_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateProjectGateRequest {
    pub(super) gate_key: String,
    pub(super) gate_type: String,
    pub(super) title: String,
    pub(super) related_task_id: Option<Uuid>,
    #[serde(default)]
    pub(super) required_evidence: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct DecideProjectGateRequest {
    pub(super) status: String,
    pub(super) decision_summary: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct SetProjectTaskGateRequirementRequest {
    #[serde(default = "default_gate_required_status")]
    pub(super) required_status: String,
}

fn default_gate_required_status() -> String {
    "passed".into()
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateProjectEnvironmentRequest {
    pub(super) environment_key: String,
    pub(super) display_name: String,
    pub(super) desired_revision: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ObserveProjectEnvironmentRequest {
    pub(super) status: String,
    pub(super) desired_revision: Option<String>,
    pub(super) observed_revision: Option<String>,
    pub(super) configuration_fingerprint: Option<String>,
    #[serde(default)]
    pub(super) health_summary: serde_json::Value,
    pub(super) observed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub(super) services: Vec<ProjectEnvironmentServiceObservationInput>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SetProjectTaskEnvironmentRequirementRequest {
    pub(super) required_revision: Option<String>,
    #[serde(default)]
    pub(super) required_services: Vec<String>,
    #[serde(default = "default_true")]
    pub(super) require_healthy: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub(super) struct UpsertCompanyAgentCodexTriggerRequest {
    pub(super) interval_seconds: Option<i32>,
    pub(super) codex_profile: Option<String>,
    pub(super) model: Option<String>,
    pub(super) reasoning_effort: Option<String>,
    pub(super) reasoning_summary: Option<String>,
    pub(super) verbosity: Option<String>,
    pub(super) personality: Option<String>,
    pub(super) service_tier: Option<String>,
    pub(super) sandbox_mode: Option<String>,
    pub(super) approval_policy: Option<String>,
    pub(super) network_access: Option<bool>,
    pub(super) web_search: Option<String>,
    pub(super) feature_multi_agent: Option<bool>,
    pub(super) feature_remote_plugin: Option<bool>,
    pub(super) feature_hooks: Option<bool>,
    pub(super) feature_goals: Option<bool>,
    pub(super) feature_shell_tool: Option<bool>,
    pub(super) max_run_seconds: Option<i32>,
    pub(super) runner_profile_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpsertCompanyCodexRunnerProfileRequest {
    pub(super) name: String,
    pub(super) interval_seconds: i32,
    pub(super) codex_profile: String,
    pub(super) model: Option<String>,
    pub(super) reasoning_effort: Option<String>,
    pub(super) reasoning_summary: Option<String>,
    pub(super) verbosity: Option<String>,
    pub(super) personality: Option<String>,
    pub(super) service_tier: Option<String>,
    pub(super) sandbox_mode: String,
    pub(super) approval_policy: String,
    pub(super) network_access: Option<bool>,
    pub(super) web_search: Option<String>,
    pub(super) feature_multi_agent: Option<bool>,
    pub(super) feature_remote_plugin: Option<bool>,
    pub(super) feature_hooks: Option<bool>,
    pub(super) feature_goals: Option<bool>,
    pub(super) feature_shell_tool: Option<bool>,
    pub(super) max_run_seconds: i32,
    pub(super) is_default: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateCompanyCodexCliSettingsRequest {
    pub(super) model: Option<String>,
    pub(super) reasoning_effort: Option<String>,
    pub(super) reasoning_summary: String,
    pub(super) verbosity: Option<String>,
    pub(super) personality: Option<String>,
    pub(super) service_tier: Option<String>,
    pub(super) approval_policy: String,
    pub(super) sandbox_mode: String,
    pub(super) network_access: bool,
    pub(super) web_search: String,
    pub(super) feature_multi_agent: bool,
    pub(super) feature_remote_plugin: bool,
    pub(super) feature_hooks: bool,
    pub(super) feature_goals: bool,
    pub(super) feature_shell_tool: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct LocalCodexModelsQuery {
    pub(super) codex_profile: Option<String>,
    pub(super) bundled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CodexRunsQuery {
    pub(super) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CodexPluginsQuery {
    pub(super) operation_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CodexPluginOperationRequest {
    pub(super) target_runner_id: String,
    pub(super) target_selector: String,
    pub(super) operation: String,
    pub(super) plugin_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateCodexAuthProfileRequest {
    pub(super) name: String,
    pub(super) api_key: String,
    pub(super) base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateCodexAuthProfileRequest {
    pub(super) name: String,
    pub(super) api_key: Option<String>,
    pub(super) base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CodexMcpRefreshRequest {
    pub(super) target_selector: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ApprovalRequestsQuery {
    pub(super) status: Option<String>,
    pub(super) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CodexSessionsQuery {
    pub(super) project_id: Option<Uuid>,
    pub(super) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CompanyMemoriesQuery {
    pub(super) owner_agent_id: Option<Uuid>,
    pub(super) scope: Option<String>,
    pub(super) project_id: Option<Uuid>,
    pub(super) memory_tier: Option<String>,
    pub(super) status: Option<String>,
    pub(super) query: Option<String>,
    pub(super) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateAgentMemoryRequest {
    pub(super) memory_tier: Option<String>,
    pub(super) title: Option<String>,
    pub(super) summary: Option<String>,
    pub(super) when_to_use: Option<String>,
    pub(super) tags: Option<Vec<String>>,
    pub(super) importance: Option<i32>,
    pub(super) confidence: Option<i32>,
    pub(super) status: Option<String>,
    pub(super) pinned: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ReviewApprovalRequest {
    pub(super) review_note: Option<String>,
    pub(super) approval_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RealtimeEventsQuery {
    pub(super) after_sequence_id: Option<i64>,
    pub(super) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ConversationMessagesQuery {
    pub(super) before_message_id: Option<Uuid>,
    pub(super) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(super) struct DevBootstrapRequest {
    pub(super) email: String,
    pub(super) display_name: String,
    pub(super) desired_handle: String,
    pub(super) desired_agent_name: String,
    pub(super) persona: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct AgentStatusUpdateRequest {
    pub(super) status: String,
    pub(super) note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct AdminRotateKeyRequest {
    pub(super) note: Option<String>,
}
