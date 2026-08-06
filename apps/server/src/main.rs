use std::{
    collections::{HashMap, HashSet, VecDeque},
    convert::Infallible,
    fs,
    net::SocketAddr,
    path::{Component, Path as FsPath, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    time::{Duration as StdDuration, Instant},
};

use axum::{
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{
        header::{ACCEPT, AUTHORIZATION, CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE},
        HeaderMap, HeaderName, HeaderValue, Method, StatusCode,
    },
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::{broadcast, mpsc};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    services::{ServeDir, ServeFile},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

use ai_chat_mcp::{agent_key_from_headers, AiChatMcpHandler, McpGateway, STANDARD_MCP_PATH};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};

use ai_chat_application::{
    ChangeHumanPasswordInput, ConfigureManagedLocalProjectGitForHumanInput,
    CreateCompanyAgentInput, CreateCompanyInput, CreateCompanyProjectForHumanInput,
    CreateCompanyProjectTaskForHumanInput, CreateOrgUnitInput, DeleteAgentMemoryForHumanInput,
    DeleteCompanyCodexRunnerProfileForHumanInput, DeleteCompanyProjectGitForHumanInput,
    DevLoginInput, GetCompanyAgentCodexTriggerForHumanInput, GetCompanyProjectGitForHumanInput,
    HumanCompanyStaffingStatusInput, ListCompanyAgentCodexRunsForHumanInput,
    ListCompanyCodexPluginsForHumanInput, ListCompanyCodexRunnerProfilesForHumanInput,
    ListCompanyMemoriesForHumanInput, LoginHumanInput, OpenHumanCompanyDirectConversationInput,
    PlatformApp, PublishCompanyGovernancePolicyInput, RegisterHumanInput,
    RequestCodexPluginOperationForHumanInput, RequestCompanyProjectRuleGenerationForHumanInput,
    ResetHumanPasswordInput, ReviewAgentToolApprovalInput,
    SendHumanCompanyMessageWithAttachmentsInput, SetCompanyAgentCodexTriggerStatusForHumanInput,
    SetCompanyProjectPauseForHumanInput, UpdateAgentMemoryForHumanInput,
    UpdateCompanyAgentPermissionsInput, UpdateCompanyAgentProfessionInput,
    UpdateCompanyAgentRoleInput, UpdateCompanyProjectRuleForHumanInput,
    UpdateCompanyProjectTaskForHumanInput, UpsertCompanyAgentCodexTriggerForHumanInput,
    UpsertCompanyCodexRunnerProfileForHumanInput, UpsertCompanyProjectAssetRefreshForHumanInput,
    UpsertCompanyProjectGitForHumanInput,
};
use ai_chat_domain::agent_identity::{HumanHarnessAccount, HumanUser};
use ai_chat_domain::company::{
    company_profession_by_key, company_project_type_catalog, infer_company_project_type,
    CompanyGovernancePolicySettings, CompanyProjectTypeDefinition, CompanyRealtimeEvent,
    CompanyRealtimeSignal, PROJECT_TYPE_SOURCE_DESCRIPTION, PROJECT_TYPE_SOURCE_FOLDER,
    PROJECT_TYPE_SOURCE_HUMAN,
};
use ai_chat_domain::social::MessageAttachmentView;
use ai_chat_infrastructure::codex_control::{
    agent_trigger_batch_size_from_env, CodexControlStore, CodexMcpServerInput,
    CompanyCodexCliSettings,
};
use ai_chat_infrastructure::codex_trigger::CodexModelCatalogFile;
use ai_chat_infrastructure::config::{ApiConfig, HarnessMode, McpConfig};
use ai_chat_infrastructure::git_credentials::{
    github_token_profile_name, managed_token_profile_name, validate_github_token,
    GitCredentialStore,
};
use ai_chat_infrastructure::gitness::GitnessProjectGitProvisioner;
use ai_chat_infrastructure::harness::HarnessProvisioner;
use ai_chat_infrastructure::ownership_proof::OwnershipProofVerifierAdapter;
use ai_chat_infrastructure::realtime::spawn_postgres_realtime_listener;
use ai_chat_infrastructure::{build_ownership_proof_verifier, build_repository, RepositoryAdapter};
use ai_chat_shared::{hash_secret, now_utc, AppError, AppResult};

#[derive(Clone)]
struct AppState {
    mcp_config: McpConfig,
    platform: PlatformApp<RepositoryAdapter, OwnershipProofVerifierAdapter>,
    enable_dev_endpoints: bool,
    admin_credentials: Vec<AdminCredential>,
    login_limiter: SlidingWindowRateLimiter,
    owner_api_limiter: SlidingWindowRateLimiter,
    admin_api_limiter: SlidingWindowRateLimiter,
    require_email_verification: bool,
    email_delivery_webhook_url: Option<String>,
    public_base_url: String,
    http_client: reqwest::Client,
    realtime_sender: broadcast::Sender<CompanyRealtimeSignal>,
    git_credential_store: GitCredentialStore,
    message_attachments_root: PathBuf,
    folder_reference_allowed_roots: Arc<Vec<PathBuf>>,
    codex_model_catalog_path: PathBuf,
    codex_control_store: CodexControlStore,
    harness_provisioner: HarnessProvisioner<RepositoryAdapter>,
}

const ADMIN_SCOPE_READ: &str = "admin:read";
const ADMIN_SCOPE_AGENTS: &str = "admin:agents";

#[derive(Clone)]
struct AdminCredential {
    name: String,
    token_hash: String,
    scopes: HashSet<String>,
}

impl AdminCredential {
    fn allows(&self, required_scope: &str) -> bool {
        self.scopes.contains("*") || self.scopes.contains(required_scope)
    }
}

#[derive(Debug, Deserialize)]
struct AdminCredentialEnv {
    name: String,
    token: String,
    scopes: Vec<String>,
}

#[derive(Clone)]
struct SlidingWindowRateLimiter {
    entries: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
    max_requests: usize,
    window: StdDuration,
    label: &'static str,
}

impl SlidingWindowRateLimiter {
    fn new(max_requests: usize, window: StdDuration, label: &'static str) -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window,
            label,
        }
    }

    fn check(&self, key: impl Into<String>) -> Result<(), ApiError> {
        let now = Instant::now();
        let cutoff = now.checked_sub(self.window).unwrap_or(now);
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| ApiError(AppError::Validation("rate limiter lock poisoned".into())))?;
        let bucket = entries.entry(key.into()).or_default();
        while bucket.front().is_some_and(|timestamp| *timestamp <= cutoff) {
            bucket.pop_front();
        }
        if bucket.len() >= self.max_requests {
            return Err(ApiError(AppError::RateLimited(format!(
                "{} limit exceeded",
                self.label
            ))));
        }
        bucket.push_back(now);
        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
}

#[derive(Debug, Serialize)]
struct ReadinessResponse {
    status: &'static str,
    repository: &'static str,
}

#[derive(Debug, Serialize)]
struct RuntimeConfigResponse {
    dev_endpoints_enabled: bool,
    admin_token_configured: bool,
    email_verification_required: bool,
    harness_mode: &'static str,
    project_types: Vec<CompanyProjectTypeDefinition>,
}

#[derive(Debug, Deserialize)]
struct HumanEmailInput {
    email: String,
}

#[derive(Debug, Deserialize)]
struct HumanAccountTokenInput {
    token: String,
}

#[derive(Debug, Serialize)]
struct EmailDeliveryPayload {
    to: String,
    template: &'static str,
    action_url: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
struct ApiErrorResponse {
    code: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct CreateCompanyRequest {
    name: String,
    slug: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateCompanyAgentRequest {
    display_name: String,
    handle: String,
    persona: String,
    org_unit_id: Option<Uuid>,
    profession_key: String,
    role_key: Option<String>,
    reports_to_membership_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct CreateOrgUnitRequest {
    parent_org_unit_id: Option<Uuid>,
    name: String,
    unit_type: String,
    sort_order: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct OpenHumanDirectConversationRequest {
    target_agent_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct SendHumanCompanyMessageRequest {
    content: String,
    #[serde(default)]
    mentioned_agent_ids: Vec<Uuid>,
    #[serde(default)]
    mention_all: bool,
    #[serde(default)]
    folder_references: Vec<HumanFolderReferenceRequest>,
}

#[derive(Debug, Clone, Deserialize)]
struct HumanFolderReferenceRequest {
    local_path: String,
}

#[derive(Debug, Deserialize)]
struct UpdateCompanyAgentPermissionsRequest {
    #[serde(default)]
    staffing_permissions: Vec<String>,
    #[serde(default)]
    project_permissions: Vec<String>,
    staffing_scope_org_unit_id: Option<Uuid>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateCompanyProjectRuleRequest {
    content: String,
}

#[derive(Debug, Deserialize)]
struct RequestCompanyProjectRuleGenerationRequest {
    agent_id: Uuid,
    instructions: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpsertCompanyProjectAssetRefreshRequest {
    maintainer_agent_id: Uuid,
    interval_minutes: i32,
    enabled: bool,
    #[serde(default)]
    run_now: bool,
}

#[derive(Debug, Deserialize)]
struct UpdateCompanyAgentRoleRequest {
    role_key: String,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateCompanyAgentProfessionRequest {
    profession_key: String,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CompanyAgentStaffingStatusRequest {
    reason: Option<String>,
    handoff_plan: Option<String>,
    handoff_agent_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct PublishCompanyGovernancePolicyRequest {
    agent_staff_limit: i32,
    delegated_agent_hiring_enabled: bool,
    delegated_agent_suspension_enabled: bool,
    delegated_agent_termination_enabled: bool,
    max_active_projects: i32,
    max_project_members: i32,
    daily_delegated_hire_limit: Option<i32>,
    daily_delegated_suspension_limit: Option<i32>,
    daily_delegated_termination_limit: Option<i32>,
    skill_language: Option<String>,
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpsertCompanyProjectGitRequest {
    remote_url: String,
    host_local_path: Option<String>,
    default_branch: Option<String>,
    auth_profile: Option<String>,
    github_token: Option<String>,
    #[serde(default)]
    clear_github_token: bool,
    allow_agent_push: Option<bool>,
    branch_prefix: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateCompanyProjectRequest {
    name: String,
    description: Option<String>,
    owner_agent_id: Uuid,
    #[serde(default)]
    member_agent_ids: Vec<Uuid>,
    project_type: Option<String>,
    source_kind: String,
    source_local_path: Option<String>,
    git_remote_url: Option<String>,
    default_branch: Option<String>,
    auth_profile: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ImportCompanyProjectFolderRequest {
    name: String,
    description: Option<String>,
    owner_agent_id: Uuid,
    #[serde(default)]
    member_agent_ids: Vec<Uuid>,
    project_type: Option<String>,
    #[serde(default)]
    file_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateCompanyWorkspaceRequest {
    managed_workspace_root: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateCompanySkillLanguageRequest {
    skill_language: String,
}

#[derive(Debug, Deserialize)]
struct UpdateAgentTriggerPreferencesRequest {
    batch_size: usize,
}

#[derive(Debug, Deserialize)]
struct CreateCompanyProjectTaskRequest {
    title: String,
    description: Option<String>,
    priority: Option<String>,
    assignee_agent_id: Option<Uuid>,
    due_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    depends_on_task_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
struct UpdateCompanyProjectTaskRequest {
    title: Option<String>,
    description: Option<String>,
    status: Option<String>,
    priority: Option<String>,
    assignee_agent_id: Option<Uuid>,
    #[serde(default)]
    clear_assignee: bool,
    due_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    clear_due_at: bool,
    depends_on_task_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
struct UpsertCompanyAgentCodexTriggerRequest {
    interval_seconds: Option<i32>,
    codex_profile: Option<String>,
    model: Option<String>,
    reasoning_effort: Option<String>,
    reasoning_summary: Option<String>,
    verbosity: Option<String>,
    personality: Option<String>,
    service_tier: Option<String>,
    sandbox_mode: Option<String>,
    approval_policy: Option<String>,
    network_access: Option<bool>,
    web_search: Option<String>,
    feature_multi_agent: Option<bool>,
    feature_remote_plugin: Option<bool>,
    feature_hooks: Option<bool>,
    feature_goals: Option<bool>,
    feature_shell_tool: Option<bool>,
    max_run_seconds: Option<i32>,
    runner_profile_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct UpsertCompanyCodexRunnerProfileRequest {
    name: String,
    interval_seconds: i32,
    codex_profile: String,
    model: Option<String>,
    reasoning_effort: Option<String>,
    reasoning_summary: Option<String>,
    verbosity: Option<String>,
    personality: Option<String>,
    service_tier: Option<String>,
    sandbox_mode: String,
    approval_policy: String,
    network_access: Option<bool>,
    web_search: Option<String>,
    feature_multi_agent: Option<bool>,
    feature_remote_plugin: Option<bool>,
    feature_hooks: Option<bool>,
    feature_goals: Option<bool>,
    feature_shell_tool: Option<bool>,
    max_run_seconds: i32,
    is_default: bool,
}

#[derive(Debug, Deserialize)]
struct UpdateCompanyCodexCliSettingsRequest {
    model: Option<String>,
    reasoning_effort: Option<String>,
    reasoning_summary: String,
    verbosity: Option<String>,
    personality: Option<String>,
    service_tier: Option<String>,
    approval_policy: String,
    sandbox_mode: String,
    network_access: bool,
    web_search: String,
    feature_multi_agent: bool,
    feature_remote_plugin: bool,
    feature_hooks: bool,
    feature_goals: bool,
    feature_shell_tool: bool,
}

#[derive(Debug, Deserialize)]
struct LocalCodexModelsQuery {
    codex_profile: Option<String>,
    bundled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CodexRunsQuery {
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct CodexPluginsQuery {
    operation_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct CodexPluginOperationRequest {
    target_runner_id: String,
    operation: String,
    plugin_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateCodexAuthProfileRequest {
    name: String,
    api_key: String,
    base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateCodexAuthProfileRequest {
    name: String,
    api_key: Option<String>,
    base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CodexMcpRefreshRequest {
    target_selector: String,
}

#[derive(Debug, Deserialize)]
struct ApprovalRequestsQuery {
    status: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct CompanyMemoriesQuery {
    owner_agent_id: Option<Uuid>,
    project_id: Option<Uuid>,
    memory_tier: Option<String>,
    status: Option<String>,
    query: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct UpdateAgentMemoryRequest {
    memory_tier: Option<String>,
    title: Option<String>,
    summary: Option<String>,
    when_to_use: Option<String>,
    tags: Option<Vec<String>>,
    importance: Option<i32>,
    confidence: Option<i32>,
    status: Option<String>,
    pinned: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ReviewApprovalRequest {
    review_note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RealtimeEventsQuery {
    after_sequence_id: Option<i64>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ConversationMessagesQuery {
    before_message_id: Option<Uuid>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct DevBootstrapRequest {
    email: String,
    display_name: String,
    desired_handle: String,
    desired_agent_name: String,
    persona: String,
}

#[derive(Debug, Deserialize)]
struct AgentStatusUpdateRequest {
    status: String,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AdminRotateKeyRequest {
    note: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,tower_http=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mut config = ApiConfig::from_env();
    let legacy_admin_token = config.admin_api_token.take();
    let legacy_admin_token_is_weak = legacy_admin_token
        .as_deref()
        .is_some_and(|token| token.trim().len() < 24);
    let admin_credentials = build_admin_credentials(
        legacy_admin_token.as_deref(),
        std::env::var("ADMIN_API_TOKENS_JSON").ok().as_deref(),
    )?;
    if config.app_env.eq_ignore_ascii_case("production") {
        if config.enable_dev_endpoints {
            anyhow::bail!("ENABLE_DEV_ENDPOINTS must be false in production");
        }
        if admin_credentials.is_empty() {
            anyhow::bail!("ADMIN_API_TOKEN or ADMIN_API_TOKENS_JSON is required in production");
        }
        if legacy_admin_token_is_weak {
            anyhow::bail!("ADMIN_API_TOKEN must contain at least 24 characters in production");
        }
        if !config.public_base_url.starts_with("https://") {
            anyhow::bail!("PUBLIC_BASE_URL must use https in production");
        }
        if config
            .email_delivery_webhook_url
            .as_deref()
            .is_some_and(|url| !url.starts_with("https://"))
        {
            anyhow::bail!("EMAIL_DELIVERY_WEBHOOK_URL must use https in production");
        }
        if config
            .allowed_origins
            .iter()
            .any(|origin| !origin.starts_with("https://"))
        {
            anyhow::bail!("API_ALLOWED_ORIGINS must use https origins in production");
        }
    }
    drop(legacy_admin_token);
    if config.require_email_verification
        && config.email_delivery_webhook_url.is_none()
        && !config.enable_dev_endpoints
    {
        anyhow::bail!(
            "REQUIRE_EMAIL_VERIFICATION=true requires EMAIL_DELIVERY_WEBHOOK_URL outside development mode"
        );
    }
    if config.harness_mode != HarnessMode::Disabled && config.harness_base_url.is_none() {
        anyhow::bail!("HARNESS_BASE_URL is required when HARNESS_MODE is official or self_hosted");
    }
    if config.app_env.eq_ignore_ascii_case("production")
        && config.harness_mode == HarnessMode::Official
        && config
            .harness_base_url
            .as_deref()
            .is_some_and(|url| !url.starts_with("https://"))
    {
        anyhow::bail!("HARNESS_BASE_URL must use https for official Harness in production");
    }
    let repository = build_repository(&config)?;
    let harness_provisioner = HarnessProvisioner::from_config(repository.clone(), &config)?;
    let verifier = build_ownership_proof_verifier(&config);
    let mcp_config = McpConfig::from_env();
    let platform = PlatformApp::with_verifier(repository, verifier);
    let git_credential_store = GitCredentialStore::from_env()?;
    let project_git_provisioner =
        GitnessProjectGitProvisioner::from_env(git_credential_store.clone())?;
    let message_attachments_root = std::env::var("MESSAGE_ATTACHMENTS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".relay/attachments"));
    tokio::fs::create_dir_all(&message_attachments_root).await?;
    let folder_reference_allowed_roots = Arc::new(load_folder_reference_allowed_roots()?);
    let codex_model_catalog_path = std::env::var("AGENT_TRIGGER_MODEL_CATALOG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".relay-agent-trigger/codex-models.json"));
    let codex_control_store = CodexControlStore::from_env()?;
    let (realtime_sender, _) = broadcast::channel(2_048);
    let _realtime_listener =
        spawn_postgres_realtime_listener(config.database_url.clone(), realtime_sender.clone());
    let mcp_gateway = McpGateway::new(platform.clone(), mcp_config.agent_key.clone())
        .with_project_git_provisioner(project_git_provisioner);
    let standard_mcp_config = StreamableHttpServerConfig::default()
        .with_stateful_mode(false)
        .with_json_response(true)
        .with_sse_keep_alive(None)
        .with_allowed_hosts(mcp_config.allowed_hosts.clone())
        .with_allowed_origins(mcp_config.allowed_origins.clone());
    let standard_mcp: StreamableHttpService<
        AiChatMcpHandler<RepositoryAdapter, OwnershipProofVerifierAdapter>,
        LocalSessionManager,
    > = StreamableHttpService::new(
        move || Ok(AiChatMcpHandler::new(mcp_gateway.clone())),
        Default::default(),
        standard_mcp_config,
    );
    let cors_layer = build_cors_layer(&config.allowed_origins)?;
    let app_state = AppState {
        mcp_config,
        platform,
        enable_dev_endpoints: config.enable_dev_endpoints,
        admin_credentials,
        login_limiter: SlidingWindowRateLimiter::new(
            10,
            StdDuration::from_secs(5 * 60),
            "authentication",
        ),
        owner_api_limiter: SlidingWindowRateLimiter::new(
            300,
            StdDuration::from_secs(60),
            "owner API",
        ),
        admin_api_limiter: SlidingWindowRateLimiter::new(
            120,
            StdDuration::from_secs(60),
            "admin API",
        ),
        require_email_verification: config.require_email_verification,
        email_delivery_webhook_url: config.email_delivery_webhook_url.clone(),
        public_base_url: config.public_base_url.clone(),
        http_client: reqwest::Client::new(),
        realtime_sender,
        git_credential_store,
        message_attachments_root,
        folder_reference_allowed_roots,
        codex_model_catalog_path,
        codex_control_store,
        harness_provisioner,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/ready", get(readiness))
        .route("/api/v1/runtime-config", get(runtime_config))
        .route("/api/v1/local-codex/models", get(get_local_codex_models))
        .route("/api/v1/auth/register", post(register_human))
        .route("/api/v1/auth/login", post(login_human))
        .route("/api/v1/auth/me", get(get_authenticated_human))
        .route("/api/v1/auth/logout", post(logout_human))
        .route(
            "/api/v1/auth/sessions",
            get(list_authenticated_human_sessions),
        )
        .route(
            "/api/v1/auth/sessions/{session_id}/revoke",
            post(revoke_authenticated_human_session),
        )
        .route(
            "/api/v1/auth/sessions/revoke-others",
            post(revoke_other_authenticated_human_sessions),
        )
        .route("/api/v1/auth/password/change", post(change_human_password))
        .route(
            "/api/v1/auth/email-verification/request",
            post(request_human_email_verification),
        )
        .route(
            "/api/v1/auth/email-verification/verify",
            post(verify_human_email),
        )
        .route(
            "/api/v1/auth/password/reset/request",
            post(request_human_password_reset),
        )
        .route(
            "/api/v1/auth/password/reset/confirm",
            post(confirm_human_password_reset),
        )
        .route("/api/v1/dev/login", post(dev_login))
        .route(
            "/api/v1/companies",
            get(list_companies).post(create_company),
        )
        .route(
            "/api/v1/companies/{company_id}/console",
            get(get_company_console),
        )
        .route(
            "/api/v1/companies/{company_id}/workspace-settings",
            post(update_company_workspace_settings),
        )
        .route(
            "/api/v1/companies/{company_id}/skill-language",
            post(update_company_skill_language),
        )
        .route(
            "/api/v1/companies/{company_id}/agent-trigger-preferences",
            get(get_agent_trigger_preferences).put(update_agent_trigger_preferences),
        )
        .route(
            "/api/v1/companies/{company_id}/projects",
            post(create_company_project_for_human),
        )
        .route(
            "/api/v1/companies/{company_id}/projects/import-folder",
            post(import_company_project_folder_for_human)
                .layer(DefaultBodyLimit::max(5 * 1024 * 1024 * 1024 + 8 * 1024 * 1024)),
        )
        .route(
            "/api/v1/companies/{company_id}/memories",
            get(list_company_memories),
        )
        .route(
            "/api/v1/companies/{company_id}/memories/{memory_id}",
            axum::routing::put(update_agent_memory_for_human).delete(delete_agent_memory_for_human),
        )
        .route(
            "/api/v1/companies/{company_id}/conversations/direct",
            post(open_human_company_direct_conversation),
        )
        .route(
            "/api/v1/companies/{company_id}/conversations/{conversation_id}/messages",
            post(send_human_company_message),
        )
        .route(
            "/api/v1/companies/{company_id}/conversations/{conversation_id}/messages/with-attachments",
            post(send_human_company_message_with_attachments)
                .layer(DefaultBodyLimit::max(101 * 1024 * 1024)),
        )
        .route(
            "/api/v1/conversations/{conversation_id}/messages/{message_id}/attachments/{attachment_id}",
            get(download_message_attachment),
        )
        .route(
            "/api/v1/companies/{company_id}/agents",
            post(create_company_agent),
        )
        .route(
            "/api/v1/companies/{company_id}/agents/{agent_id}/permissions",
            post(update_company_agent_permissions),
        )
        .route(
            "/api/v1/companies/{company_id}/agents/{agent_id}/role",
            post(update_company_agent_role),
        )
        .route(
            "/api/v1/companies/{company_id}/agents/{agent_id}/profession",
            post(update_company_agent_profession),
        )
        .route(
            "/api/v1/companies/{company_id}/agents/{agent_id}/codex-trigger",
            get(get_company_agent_codex_trigger).put(upsert_company_agent_codex_trigger),
        )
        .route(
            "/api/v1/companies/{company_id}/codex-runner-profiles",
            get(list_company_codex_runner_profiles).post(create_company_codex_runner_profile),
        )
        .route(
            "/api/v1/companies/{company_id}/codex-environments",
            get(get_company_codex_environments),
        )
        .route(
            "/api/v1/companies/{company_id}/codex-cli-settings",
            get(get_company_codex_cli_settings).put(update_company_codex_cli_settings),
        )
        .route(
            "/api/v1/companies/{company_id}/codex-cli/install",
            post(request_codex_cli_install),
        )
        .route(
            "/api/v1/companies/{company_id}/codex-cli/update",
            post(request_codex_cli_update),
        )
        .route(
            "/api/v1/companies/{company_id}/codex-auth-profiles",
            post(create_company_codex_auth_profile),
        )
        .route(
            "/api/v1/companies/{company_id}/codex-auth-profiles/{profile_id}",
            axum::routing::put(update_company_codex_auth_profile)
                .delete(delete_company_codex_auth_profile),
        )
        .route(
            "/api/v1/companies/{company_id}/codex-mcp-servers",
            post(add_company_codex_mcp_server),
        )
        .route(
            "/api/v1/companies/{company_id}/codex-mcp-servers/refresh",
            post(refresh_company_codex_mcp_servers),
        )
        .route(
            "/api/v1/companies/{company_id}/codex-mcp-servers/{target_selector}/{server_name}",
            axum::routing::delete(remove_company_codex_mcp_server),
        )
        .route(
            "/api/v1/companies/{company_id}/codex-plugins",
            get(list_company_codex_plugins),
        )
        .route(
            "/api/v1/companies/{company_id}/codex-plugins/operations",
            post(request_codex_plugin_operation),
        )
        .route(
            "/api/v1/companies/{company_id}/codex-runner-profiles/{profile_id}",
            axum::routing::put(update_company_codex_runner_profile)
                .delete(delete_company_codex_runner_profile),
        )
        .route(
            "/api/v1/companies/{company_id}/approvals",
            get(list_company_approvals),
        )
        .route(
            "/api/v1/companies/{company_id}/approvals/{approval_id}/approve",
            post(approve_company_approval),
        )
        .route(
            "/api/v1/companies/{company_id}/approvals/{approval_id}/reject",
            post(reject_company_approval),
        )
        .route(
            "/api/v1/companies/{company_id}/agents/{agent_id}/codex-trigger/pause",
            post(pause_company_agent_codex_trigger),
        )
        .route(
            "/api/v1/companies/{company_id}/agents/{agent_id}/codex-trigger/resume",
            post(resume_company_agent_codex_trigger),
        )
        .route(
            "/api/v1/companies/{company_id}/agents/{agent_id}/codex-trigger/run-now",
            post(run_company_agent_codex_trigger_now),
        )
        .route(
            "/api/v1/companies/{company_id}/agents/{agent_id}/codex-runs",
            get(list_company_agent_codex_runs),
        )
        .route(
            "/api/v1/companies/{company_id}/governance-policy",
            get(get_company_governance_policy).post(publish_company_governance_policy),
        )
        .route(
            "/api/v1/companies/{company_id}/projects/{project_id}/git",
            get(get_company_project_git)
                .put(upsert_company_project_git)
                .delete(delete_company_project_git),
        )
        .route(
            "/api/v1/companies/{company_id}/projects/{project_id}/pause",
            post(pause_company_project),
        )
        .route(
            "/api/v1/companies/{company_id}/projects/{project_id}/resume",
            post(resume_company_project),
        )
        .route(
            "/api/v1/companies/{company_id}/projects/{project_id}/rule",
            axum::routing::put(update_company_project_rule),
        )
        .route(
            "/api/v1/companies/{company_id}/projects/{project_id}/rule/generate",
            post(request_company_project_rule_generation),
        )
        .route(
            "/api/v1/companies/{company_id}/projects/{project_id}/asset-refresh",
            axum::routing::put(upsert_company_project_asset_refresh),
        )
        .route(
            "/api/v1/companies/{company_id}/tasks",
            get(list_company_project_tasks_for_human),
        )
        .route(
            "/api/v1/companies/{company_id}/projects/{project_id}/tasks",
            post(create_company_project_task_for_human),
        )
        .route(
            "/api/v1/companies/{company_id}/projects/{project_id}/tasks/{task_id}",
            axum::routing::put(update_company_project_task_for_human),
        )
        .route(
            "/api/v1/companies/{company_id}/agents/{agent_id}/activate",
            post(activate_company_agent),
        )
        .route(
            "/api/v1/companies/{company_id}/agents/{agent_id}/suspend",
            post(suspend_company_agent),
        )
        .route(
            "/api/v1/companies/{company_id}/agents/{agent_id}/reactivate",
            post(reactivate_company_agent),
        )
        .route(
            "/api/v1/companies/{company_id}/agents/{agent_id}/terminate",
            post(terminate_company_agent),
        )
        .route(
            "/api/v1/companies/{company_id}/staffing-actions",
            get(list_company_staffing_actions),
        )
        .route(
            "/api/v1/companies/{company_id}/org-units",
            post(create_org_unit),
        )
        .route(
            "/api/v1/humans/{human_user_id}/agents",
            get(list_owner_agents),
        )
        .route(
            "/api/v1/agents/{agent_id}/conversations",
            get(list_agent_conversations),
        )
        .route(
            "/api/v1/conversations/{conversation_id}/messages",
            get(get_conversation_messages),
        )
        .route(
            "/api/v1/humans/{human_user_id}/agents/{agent_id}/status",
            post(update_owned_agent_status),
        )
        .route(
            "/api/v1/humans/{human_user_id}/agents/{agent_id}/rotate-key",
            post(rotate_owned_agent_key),
        )
        .route(
            "/api/v1/admin/agents/{agent_id}/status",
            post(update_admin_agent_status),
        )
        .route(
            "/api/v1/admin/agents/{agent_id}/rotate-key",
            post(rotate_admin_agent_key),
        )
        .route("/api/v1/dev/bootstrap", post(dev_bootstrap_agent))
        .route_service(STANDARD_MCP_PATH, standard_mcp)
        .fallback_service(
            ServeDir::new(resolve_web_dist_dir())
                .not_found_service(ServeFile::new(resolve_web_dist_dir().join("index.html"))),
        )
        .layer(DefaultBodyLimit::max(config.max_request_body_bytes))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            StdDuration::from_secs(config.request_timeout_seconds),
        ))
        .route(
            "/api/v1/companies/{company_id}/events",
            get(stream_company_events_for_human),
        )
        .route(
            "/api/v1/agent/companies/{company_id}/events",
            get(stream_company_events_for_agent),
        )
        .layer(cors_layer)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let addr = SocketAddr::from((config.host.parse::<std::net::IpAddr>()?, config.port));
    tracing::info!("ai-chat server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "ai-chat-server",
    })
}

async fn readiness(State(state): State<AppState>) -> Result<Json<ReadinessResponse>, ApiError> {
    state.platform.health_check()?;
    Ok(Json(ReadinessResponse {
        status: "ready",
        repository: "ok",
    }))
}

fn build_cors_layer(origins: &[String]) -> anyhow::Result<CorsLayer> {
    let origins = origins
        .iter()
        .map(|origin| origin.parse::<HeaderValue>())
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            ACCEPT,
            AUTHORIZATION,
            CACHE_CONTROL,
            CONTENT_TYPE,
            HeaderName::from_static("x-agent-key"),
            HeaderName::from_static("x-agent-run-token"),
            HeaderName::from_static("idempotency-key"),
            HeaderName::from_static("last-event-id"),
            HeaderName::from_static("mcp-protocol-version"),
        ]))
}

async fn runtime_config(State(state): State<AppState>) -> Json<RuntimeConfigResponse> {
    Json(RuntimeConfigResponse {
        dev_endpoints_enabled: state.enable_dev_endpoints,
        admin_token_configured: !state.admin_credentials.is_empty(),
        email_verification_required: state.require_email_verification,
        harness_mode: state.harness_provisioner.mode_key(),
        project_types: company_project_type_catalog(),
    })
}

async fn get_local_codex_models(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LocalCodexModelsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authenticate_human_request(&state, &headers)?;
    let profile = query
        .codex_profile
        .as_deref()
        .map(str::trim)
        .filter(|profile| !profile.is_empty())
        .unwrap_or("default");
    if profile.len() > 64 || profile.chars().any(char::is_control) {
        return Err(AppError::Validation("invalid local Codex profile".into()).into());
    }
    let bundled = query.bundled.unwrap_or(false);
    let bytes = tokio::fs::read(&state.codex_model_catalog_path)
        .await
        .map_err(|error| {
            AppError::Internal(format!(
                "cannot read Trigger model catalog {}: {error}",
                state.codex_model_catalog_path.display()
            ))
        })?;
    let catalog: CodexModelCatalogFile = serde_json::from_slice(&bytes).map_err(|error| {
        AppError::Internal(format!("Trigger model catalog is invalid: {error}"))
    })?;
    let snapshot = catalog
        .catalogs
        .into_iter()
        .find(|snapshot| snapshot.codex_profile == profile && snapshot.bundled == bundled)
        .ok_or_else(|| {
            AppError::Validation(format!(
                "Trigger has not discovered models for Codex profile `{profile}` (bundled={bundled})"
            ))
        })?;
    Ok(Json(serde_json::json!({
        "models": snapshot.models,
        "source": snapshot.source,
        "discovered_at": snapshot.discovered_at,
    })))
}

async fn register_human(
    State(state): State<AppState>,
    Json(input): Json<RegisterHumanInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .login_limiter
        .check(format!("register:{}", input.email.trim().to_lowercase()))?;
    let auth = state.platform.register_human(input)?;
    let harness = ensure_harness_account(&state, &auth.user).await;
    let verification = state
        .platform
        .issue_human_email_verification(auth.user.id)?;
    let email_verification_sent = deliver_account_token(
        &state,
        &auth.user.email,
        "email_verification",
        &verification.token,
        verification.expires_at,
    )
    .await;
    Ok(Json(serde_json::json!({
        "user": auth.user,
        "session_token": auth.session_token,
        "expires_at": auth.expires_at,
        "email_verified": false,
        "email_verification_sent": email_verification_sent,
        "harness": harness
    })))
}

async fn login_human(
    State(state): State<AppState>,
    Json(input): Json<LoginHumanInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .login_limiter
        .check(format!("login:{}", input.email.trim().to_lowercase()))?;
    let auth = state.platform.login_human(input)?;
    let harness = ensure_harness_account(&state, &auth.user).await;
    let email_verified = state.platform.is_human_email_verified(auth.user.id)?;
    Ok(Json(serde_json::json!({
        "user": auth.user,
        "session_token": auth.session_token,
        "expires_at": auth.expires_at,
        "email_verified": email_verified,
        "harness": harness
    })))
}

async fn get_authenticated_human(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = authenticate_human_session_request(&state, &headers)?;
    let email_verified = state.platform.is_human_email_verified(user.id)?;
    let harness = state.harness_provisioner.account(user.id)?;
    Ok(Json(serde_json::json!({
        "user": user,
        "email_verified": email_verified,
        "harness": harness
    })))
}

async fn ensure_harness_account(state: &AppState, user: &HumanUser) -> Option<HumanHarnessAccount> {
    if !state.harness_provisioner.is_enabled() {
        return None;
    }
    match state.harness_provisioner.ensure_account(user).await {
        Ok(account) => account,
        Err(error) => {
            tracing::warn!(
                human_user_id = %user.id,
                error = %error,
                "Harness provisioning failed; Human authentication remains available and login will retry"
            );
            state.harness_provisioner.account(user.id).ok().flatten()
        }
    }
}

async fn logout_human(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let token = bearer_token(&headers)?;
    state
        .owner_api_limiter
        .check(format!("owner:{}", hash_secret(token)))?;
    state.platform.logout_human_session(token)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn list_authenticated_human_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_session_request(&state, &headers)?;
    let token = bearer_token(&headers)?;
    let sessions = state.platform.list_human_sessions(human.id, token)?;
    Ok(Json(serde_json::json!({ "sessions": sessions })))
}

async fn revoke_authenticated_human_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_session_request(&state, &headers)?;
    state
        .platform
        .revoke_owned_human_session(human.id, session_id)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn revoke_other_authenticated_human_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_session_request(&state, &headers)?;
    let token = bearer_token(&headers)?;
    let revoked_count = state
        .platform
        .revoke_other_human_sessions(human.id, token)?;
    Ok(Json(serde_json::json!({ "revoked_count": revoked_count })))
}

async fn change_human_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ChangeHumanPasswordInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_session_request(&state, &headers)?;
    let token = bearer_token(&headers)?;
    let revoked_count = state
        .platform
        .change_human_password(human.id, token, input)?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "revoked_other_sessions": revoked_count
    })))
}

async fn request_human_email_verification(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_session_request(&state, &headers)?;
    let verification = state.platform.issue_human_email_verification(human.id)?;
    let delivered = deliver_account_token(
        &state,
        &human.email,
        "email_verification",
        &verification.token,
        verification.expires_at,
    )
    .await;
    let mut response = serde_json::json!({
        "ok": true,
        "delivered": delivered,
        "expires_at": verification.expires_at
    });
    if state.enable_dev_endpoints {
        response["development_token"] = serde_json::Value::String(verification.token);
    }
    Ok(Json(response))
}

async fn verify_human_email(
    State(state): State<AppState>,
    Json(input): Json<HumanAccountTokenInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = state.platform.verify_human_email(&input.token)?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "user": user,
        "email_verified": true
    })))
}

async fn request_human_password_reset(
    State(state): State<AppState>,
    Json(input): Json<HumanEmailInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.login_limiter.check(format!(
        "password-reset:{}",
        input.email.trim().to_lowercase()
    ))?;
    let issued = state.platform.issue_human_password_reset(&input.email)?;
    let mut response = serde_json::json!({
        "ok": true,
        "message": "If the account exists, password reset instructions have been sent."
    });
    if let Some((user, reset)) = issued {
        let delivered = deliver_account_token(
            &state,
            &user.email,
            "password_reset",
            &reset.token,
            reset.expires_at,
        )
        .await;
        response["delivered"] = serde_json::Value::Bool(delivered);
        if state.enable_dev_endpoints {
            response["development_token"] = serde_json::Value::String(reset.token);
        }
    }
    Ok(Json(response))
}

async fn confirm_human_password_reset(
    State(state): State<AppState>,
    Json(input): Json<ResetHumanPasswordInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.login_limiter.check(format!(
        "password-reset-confirm:{}",
        hash_secret(&input.token)
    ))?;
    state.platform.reset_human_password(input)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn deliver_account_token(
    state: &AppState,
    email: &str,
    template: &'static str,
    token: &str,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> bool {
    let action_path = match template {
        "email_verification" => "/?verify_email_token=",
        "password_reset" => "/?password_reset_token=",
        _ => "/?account_token=",
    };
    let action_url = format!(
        "{}{}{}",
        state.public_base_url.trim_end_matches('/'),
        action_path,
        token
    );

    let Some(webhook_url) = state.email_delivery_webhook_url.as_deref() else {
        if state.enable_dev_endpoints {
            tracing::warn!(
                email,
                template,
                action_url,
                "development account email token"
            );
        }
        return false;
    };
    let payload = EmailDeliveryPayload {
        to: email.to_string(),
        template,
        action_url,
        expires_at,
    };
    match state
        .http_client
        .post(webhook_url)
        .json(&payload)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => true,
        Ok(response) => {
            tracing::error!(status = %response.status(), template, "email delivery webhook rejected request");
            false
        }
        Err(error) => {
            tracing::error!(%error, template, "email delivery webhook failed");
            false
        }
    }
}

fn resolve_web_dist_dir() -> PathBuf {
    if let Ok(value) = std::env::var("WEB_DIST_DIR") {
        return PathBuf::from(value);
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../web/dist")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/dist"))
}

async fn dev_login(
    State(state): State<AppState>,
    Json(input): Json<DevLoginInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_dev_endpoints(&state)?;
    state
        .login_limiter
        .check(format!("dev:{}", input.email.trim().to_lowercase()))?;
    let user = state.platform.dev_login(input)?;
    let auth = state.platform.issue_human_session(user.id)?;
    Ok(Json(serde_json::json!({
        "user": auth.user,
        "session_token": auth.session_token,
        "expires_at": auth.expires_at
    })))
}

async fn create_company(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateCompanyRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let company = state.platform.create_company(CreateCompanyInput {
        human_user_id: human.id,
        name: input.name,
        slug: input.slug,
        description: input.description,
    })?;
    Ok(Json(serde_json::json!({ "company_console": company })))
}

async fn list_companies(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let companies = state.platform.list_human_companies(human.id)?;
    Ok(Json(serde_json::json!({ "companies": companies })))
}

async fn get_company_console(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let company = state.platform.get_company_console(human.id, company_id)?;
    Ok(Json(serde_json::json!({ "company_console": company })))
}

async fn list_company_memories(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Query(query): Query<CompanyMemoriesQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let memories =
        state
            .platform
            .list_company_memories_for_human(ListCompanyMemoriesForHumanInput {
                human_user_id: human.id,
                company_id,
                owner_agent_id: query.owner_agent_id,
                project_id: query.project_id,
                memory_tier: query.memory_tier,
                status: query.status,
                query: query.query,
                limit: query.limit.unwrap_or(200),
            })?;
    Ok(Json(serde_json::json!({ "memories": memories })))
}

async fn update_agent_memory_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, memory_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateAgentMemoryRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let memory = state
        .platform
        .update_agent_memory_for_human(UpdateAgentMemoryForHumanInput {
            human_user_id: human.id,
            company_id,
            memory_id,
            memory_tier: input.memory_tier,
            title: input.title,
            summary: input.summary,
            when_to_use: input.when_to_use,
            tags: input.tags,
            importance: input.importance,
            confidence: input.confidence,
            status: input.status,
            pinned: input.pinned,
        })?;
    Ok(Json(serde_json::json!({ "memory": memory })))
}

async fn delete_agent_memory_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, memory_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .delete_agent_memory_for_human(DeleteAgentMemoryForHumanInput {
            human_user_id: human.id,
            company_id,
            memory_id,
        })?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

async fn open_human_company_direct_conversation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<OpenHumanDirectConversationRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let conversation = state.platform.open_human_company_direct_conversation(
        OpenHumanCompanyDirectConversationInput {
            human_user_id: human.id,
            company_id,
            target_agent_id: input.target_agent_id,
        },
    )?;
    Ok(Json(serde_json::json!({ "conversation": conversation })))
}

async fn send_human_company_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, conversation_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<SendHumanCompanyMessageRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_send_company_messages(human.id, company_id)?;
    let attachments = build_local_folder_attachments(
        &input.folder_references,
        &state.folder_reference_allowed_roots,
    )?;
    let message = state.platform.send_human_company_message_with_attachments(
        SendHumanCompanyMessageWithAttachmentsInput {
            human_user_id: human.id,
            company_id,
            conversation_id,
            content: input.content,
            mentioned_agent_ids: input.mentioned_agent_ids,
            mention_all: input.mention_all,
            attachments,
        },
    )?;
    Ok(Json(serde_json::json!({ "message": message })))
}

struct PendingUploadedFile {
    file_name: String,
    content_type: String,
    bytes: Bytes,
}

async fn send_human_company_message_with_attachments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, conversation_id)): Path<(Uuid, Uuid)>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_send_company_messages(human.id, company_id)?;
    let mut content = String::new();
    let mut mentioned_agent_ids = Vec::new();
    let mut mention_all = false;
    let mut relative_paths = Vec::<String>::new();
    let mut folder_references = Vec::<HumanFolderReferenceRequest>::new();
    let mut pending_files = Vec::<PendingUploadedFile>::new();
    let mut total_bytes = 0usize;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::Validation(format!("invalid attachment form: {error}")))?
    {
        let field_name = field.name().unwrap_or_default().to_string();
        match field_name.as_str() {
            "content" => {
                content = field.text().await.map_err(|error| {
                    AppError::Validation(format!("invalid message content: {error}"))
                })?;
            }
            "mentioned_agent_ids" => {
                let value = field.text().await.map_err(|error| {
                    AppError::Validation(format!("invalid mentioned Agent list: {error}"))
                })?;
                mentioned_agent_ids = serde_json::from_str(&value).map_err(|_| {
                    AppError::Validation("mentioned_agent_ids must be a UUID array".into())
                })?;
            }
            "mention_all" => {
                mention_all = field
                    .text()
                    .await
                    .map_err(|error| {
                        AppError::Validation(format!("invalid mention_all value: {error}"))
                    })?
                    .parse::<bool>()
                    .map_err(|_| AppError::Validation("mention_all must be boolean".into()))?;
            }
            "relative_paths" => {
                let value = field.text().await.map_err(|error| {
                    AppError::Validation(format!("invalid relative path list: {error}"))
                })?;
                relative_paths = serde_json::from_str(&value).map_err(|_| {
                    AppError::Validation("relative_paths must be a string array".into())
                })?;
            }
            "folder_references" => {
                let value = field.text().await.map_err(|error| {
                    AppError::Validation(format!("invalid folder reference list: {error}"))
                })?;
                folder_references = serde_json::from_str(&value).map_err(|_| {
                    AppError::Validation("folder_references must be an array".into())
                })?;
            }
            "file" => {
                if pending_files.len() >= 20 {
                    return Err(AppError::Validation(
                        "a message can contain at most 20 files".into(),
                    )
                    .into());
                }
                let file_name =
                    sanitize_attachment_file_name(field.file_name().unwrap_or("attachment.bin"))?;
                let content_type = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();
                let bytes = field.bytes().await.map_err(|error| {
                    AppError::Validation(format!("cannot read uploaded file: {error}"))
                })?;
                if bytes.len() > 20 * 1024 * 1024 {
                    return Err(AppError::Validation(format!(
                        "file {file_name} exceeds the 20 MiB limit"
                    ))
                    .into());
                }
                total_bytes = total_bytes.saturating_add(bytes.len());
                if total_bytes > 100 * 1024 * 1024 {
                    return Err(AppError::Validation(
                        "message attachments exceed the 100 MiB total limit".into(),
                    )
                    .into());
                }
                pending_files.push(PendingUploadedFile {
                    file_name,
                    content_type,
                    bytes,
                });
            }
            _ => {}
        }
    }

    if !relative_paths.is_empty() && relative_paths.len() != pending_files.len() {
        return Err(AppError::Validation(
            "relative_paths must match the uploaded file count".into(),
        )
        .into());
    }
    let mut attachments =
        build_local_folder_attachments(&folder_references, &state.folder_reference_allowed_roots)?;
    if attachments.len() + pending_files.len() > 20 {
        return Err(AppError::Validation(
            "a message can contain at most 20 attachments and folder references".into(),
        )
        .into());
    }

    let mut stored_paths = Vec::<PathBuf>::new();
    for (index, pending) in pending_files.into_iter().enumerate() {
        let attachment_id = Uuid::new_v4();
        let storage_key = format!("{}/{}", &attachment_id.to_string()[..2], attachment_id);
        let storage_path = state.message_attachments_root.join(&storage_key);
        if let Some(parent) = storage_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                AppError::Validation(format!("cannot create attachment directory: {error}"))
            })?;
        }
        tokio::fs::write(&storage_path, &pending.bytes)
            .await
            .map_err(|error| AppError::Validation(format!("cannot store attachment: {error}")))?;
        stored_paths.push(storage_path);
        let relative_path = relative_paths
            .get(index)
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(validate_attachment_relative_path)
            .transpose()?;
        attachments.push(MessageAttachmentView {
            id: attachment_id,
            kind: if pending.content_type.starts_with("image/") {
                "image".into()
            } else {
                "file".into()
            },
            file_name: pending.file_name,
            relative_path,
            content_type: pending.content_type,
            byte_size: i64::try_from(pending.bytes.len()).unwrap_or(i64::MAX),
            local_path: None,
            directory_entries: Vec::new(),
            purpose: None,
            storage_key,
        });
    }

    let send_result = state.platform.send_human_company_message_with_attachments(
        SendHumanCompanyMessageWithAttachmentsInput {
            human_user_id: human.id,
            company_id,
            conversation_id,
            content,
            mentioned_agent_ids,
            mention_all,
            attachments,
        },
    );
    let message = match send_result {
        Ok(message) => message,
        Err(error) => {
            for path in stored_paths {
                let _ = tokio::fs::remove_file(path).await;
            }
            return Err(error.into());
        }
    };
    Ok(Json(serde_json::json!({ "message": message })))
}

async fn download_message_attachment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((conversation_id, message_id, attachment_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Response, ApiError> {
    let messages = if bearer_token(&headers).is_ok() {
        let human = authenticate_human_request(&state, &headers)?;
        state
            .platform
            .get_owned_conversation_messages(human.id, conversation_id)?
    } else {
        let agent_key = agent_key_from_headers(&headers)
            .ok_or_else(|| AppError::Unauthorized("missing authentication token".into()))?;
        let agent = state.platform.authenticate_agent_key(&agent_key)?;
        state
            .platform
            .get_agent_conversation_messages(agent.id, conversation_id)?
    };
    let attachment = messages
        .into_iter()
        .find(|message| message.id == message_id)
        .and_then(|message| {
            message
                .attachments
                .into_iter()
                .find(|attachment| attachment.id == attachment_id)
        })
        .filter(|attachment| matches!(attachment.kind.as_str(), "file" | "image"))
        .ok_or_else(|| AppError::NotFound("message attachment not found".into()))?;
    if attachment.storage_key.is_empty()
        || attachment.storage_key.contains("..")
        || PathBuf::from(&attachment.storage_key).is_absolute()
    {
        return Err(AppError::NotFound("message attachment file is unavailable".into()).into());
    }
    let bytes = tokio::fs::read(state.message_attachments_root.join(&attachment.storage_key))
        .await
        .map_err(|_| AppError::NotFound("message attachment file is unavailable".into()))?;
    let mut response = Response::new(Body::from(bytes));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_str(&attachment.content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    response.headers_mut().insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(if attachment.kind == "image" {
            "inline"
        } else {
            "attachment"
        })
        .expect("static content disposition is valid"),
    );
    Ok(response)
}

fn sanitize_attachment_file_name(value: &str) -> Result<String, ApiError> {
    let file_name = std::path::Path::new(value)
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation("attachment file name is invalid".into()))?;
    if file_name.chars().any(char::is_control) || file_name.chars().count() > 255 {
        return Err(AppError::Validation("attachment file name is invalid".into()).into());
    }
    Ok(file_name.to_string())
}

fn validate_attachment_relative_path(value: &str) -> Result<String, ApiError> {
    let normalized = value.replace('\\', "/");
    if normalized.chars().any(char::is_control)
        || normalized.len() > 2_000
        || normalized.starts_with('/')
        || normalized
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(AppError::Validation("attachment relative path is invalid".into()).into());
    }
    Ok(normalized)
}

fn build_local_folder_attachments(
    requests: &[HumanFolderReferenceRequest],
    allowed_roots: &[PathBuf],
) -> Result<Vec<MessageAttachmentView>, ApiError> {
    requests
        .iter()
        .map(|request| {
            let requested_path = PathBuf::from(request.local_path.trim());
            if !requested_path.is_absolute() {
                return Err(AppError::Validation(
                    "folder reference must use a host absolute path".into(),
                )
                .into());
            }
            let local_path = std::fs::canonicalize(&requested_path).map_err(|_| {
                AppError::Validation(format!(
                    "folder does not exist on the server host: {}",
                    requested_path.display()
                ))
            })?;
            if !local_path.is_dir() {
                return Err(AppError::Validation(format!(
                    "folder reference is not a directory: {}",
                    local_path.display()
                ))
                .into());
            }
            if allowed_roots.is_empty() {
                return Err(AppError::Validation(
                    "local folder references are disabled; configure HUMAN_FOLDER_REFERENCE_ALLOWED_ROOTS"
                        .into(),
                )
                .into());
            }
            if !allowed_roots.iter().any(|root| local_path.starts_with(root)) {
                return Err(AppError::Unauthorized(
                    "folder reference is outside HUMAN_FOLDER_REFERENCE_ALLOWED_ROOTS".into(),
                )
                .into());
            }
            let directory_entries = collect_directory_structure(&local_path)?;
            let file_name = local_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("project")
                .to_string();
            Ok(MessageAttachmentView {
                id: Uuid::new_v4(),
                kind: "local_folder".into(),
                file_name,
                relative_path: None,
                content_type: "application/x-relay-local-folder".into(),
                byte_size: 0,
                local_path: Some(local_path.to_string_lossy().to_string()),
                directory_entries,
                purpose: Some("create_project_and_push_to_git".into()),
                storage_key: String::new(),
            })
        })
        .collect()
}

fn collect_directory_structure(root: &std::path::Path) -> Result<Vec<String>, ApiError> {
    const MAX_ENTRIES: usize = 5_000;
    const MAX_DEPTH: usize = 16;
    const MAX_PATH_BYTES: usize = 512_000;
    const EXCLUDED_DIRECTORY_NAMES: &[&str] = &[
        ".git",
        ".relay",
        ".relay-agent-trigger",
        "node_modules",
        "target",
    ];
    fn visit(
        root: &std::path::Path,
        directory: &std::path::Path,
        depth: usize,
        entries: &mut Vec<String>,
        path_bytes: &mut usize,
    ) -> Result<(), ApiError> {
        if depth > MAX_DEPTH {
            return Ok(());
        }
        let mut children = std::fs::read_dir(directory)
            .map_err(|error| {
                AppError::Validation(format!(
                    "cannot read folder {}: {error}",
                    directory.display()
                ))
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                AppError::Validation(format!(
                    "cannot enumerate folder {}: {error}",
                    directory.display()
                ))
            })?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            if entries.len() >= MAX_ENTRIES {
                return Err(AppError::Validation(format!(
                    "folder contains more than {MAX_ENTRIES} entries"
                ))
                .into());
            }
            let file_type = child.file_type().map_err(|error| {
                AppError::Validation(format!("cannot inspect folder entry: {error}"))
            })?;
            if file_type.is_symlink() {
                continue;
            }
            let file_name = child.file_name();
            if file_type.is_dir()
                && file_name
                    .to_str()
                    .is_some_and(|name| EXCLUDED_DIRECTORY_NAMES.contains(&name))
            {
                continue;
            }
            let path = child.path();
            let relative = path.strip_prefix(root).map_err(|_| {
                AppError::Validation("folder entry escaped the selected directory".into())
            })?;
            let mut display = relative.to_string_lossy().replace('\\', "/");
            if file_type.is_dir() {
                display.push('/');
            }
            *path_bytes = path_bytes.saturating_add(display.len());
            if *path_bytes > MAX_PATH_BYTES {
                return Err(AppError::Validation(format!(
                    "folder structure exceeds the {MAX_PATH_BYTES} byte metadata limit"
                ))
                .into());
            }
            entries.push(display);
            if file_type.is_dir() {
                visit(root, &path, depth + 1, entries, path_bytes)?;
            }
        }
        Ok(())
    }

    let mut entries = Vec::new();
    let mut path_bytes = 0usize;
    visit(root, root, 0, &mut entries, &mut path_bytes)?;
    Ok(entries)
}

fn validate_company_workspace_root(raw: String) -> AppResult<String> {
    let value = raw.trim();
    let path = FsPath::new(value);
    if value.is_empty()
        || value != raw
        || value.chars().count() > 4_096
        || value.chars().any(char::is_control)
        || !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        || path.parent().is_none()
    {
        return Err(AppError::Validation(
            "managed_workspace_root must be an absolute normalized non-root path".into(),
        ));
    }
    Ok(value.to_string())
}

fn resolve_company_workspace_root(
    configured: Option<&str>,
    company_id: Uuid,
) -> AppResult<PathBuf> {
    if let Some(configured) = configured.filter(|value| !value.trim().is_empty()) {
        return validate_company_workspace_root(configured.to_string()).map(PathBuf::from);
    }
    let base = std::env::var("RELAY_DEFAULT_WORKSPACE_ROOT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(|home| PathBuf::from(home).join(".relay"))
        })
        .unwrap_or_else(|| PathBuf::from("/tmp/.relay"));
    validate_company_workspace_root(
        base.join("companies")
            .join(company_id.to_string())
            .to_string_lossy()
            .into_owned(),
    )
    .map(PathBuf::from)
}

fn managed_project_directory_name(name: &str, project_id: Uuid) -> String {
    let slug = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let slug = if slug.is_empty() { "project" } else { &slug };
    format!(
        "{}-{}",
        slug.chars().take(48).collect::<String>(),
        &project_id.to_string()[..8]
    )
}

fn validate_project_source_folder(raw: &str, allowed_roots: &[PathBuf]) -> AppResult<PathBuf> {
    let requested = PathBuf::from(raw.trim());
    if !requested.is_absolute() {
        return Err(AppError::Validation(
            "selected project folder must resolve to a host absolute path".into(),
        ));
    }
    let source = fs::canonicalize(&requested).map_err(|error| {
        AppError::Validation(format!(
            "selected project folder is not accessible on this Relay host: {error}"
        ))
    })?;
    if !source.is_dir() {
        return Err(AppError::Validation(
            "selected project source is not a directory".into(),
        ));
    }
    let effective_roots = if allowed_roots.is_empty() {
        std::env::var("HOME")
            .ok()
            .and_then(|home| fs::canonicalize(home).ok())
            .into_iter()
            .collect::<Vec<_>>()
    } else {
        allowed_roots.to_vec()
    };
    if effective_roots.is_empty()
        || !effective_roots
            .iter()
            .any(|root| source.starts_with(root) && &source != root)
    {
        return Err(AppError::Unauthorized(
            "selected folder is outside the local directories allowed for Relay imports".into(),
        ));
    }
    Ok(source)
}

fn import_project_folder(source: &FsPath, destination: &FsPath) -> AppResult<()> {
    const MAX_FILES: usize = 100_000;
    const MAX_BYTES: u64 = 5 * 1024 * 1024 * 1024;
    const EXCLUDED: &[&str] = &[
        ".git",
        ".relay",
        ".relay-agent-trigger",
        "node_modules",
        "target",
    ];
    if destination.exists() {
        return Err(AppError::Conflict(
            "managed project destination already exists".into(),
        ));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| AppError::Validation("managed project destination has no parent".into()))?;
    fs::create_dir_all(parent).map_err(|error| {
        AppError::Internal(format!("failed to create managed workspace: {error}"))
    })?;
    let staging = parent.join(format!(".relay-import-{}", Uuid::new_v4()));
    fs::create_dir(&staging).map_err(|error| {
        AppError::Internal(format!(
            "failed to create project import staging directory: {error}"
        ))
    })?;

    fn copy_tree(
        source_root: &FsPath,
        source: &FsPath,
        destination_root: &FsPath,
        excluded: &[&str],
        files: &mut usize,
        bytes: &mut u64,
    ) -> AppResult<()> {
        for entry in fs::read_dir(source).map_err(|error| {
            AppError::Validation(format!("cannot read imported folder: {error}"))
        })? {
            let entry = entry.map_err(|error| {
                AppError::Validation(format!("cannot inspect imported folder entry: {error}"))
            })?;
            let file_type = entry.file_type().map_err(|error| {
                AppError::Validation(format!("cannot inspect imported file type: {error}"))
            })?;
            if file_type.is_symlink() {
                continue;
            }
            let file_name = entry.file_name();
            if file_type.is_dir()
                && file_name
                    .to_str()
                    .is_some_and(|name| excluded.contains(&name))
            {
                continue;
            }
            let source_path = entry.path();
            let relative = source_path.strip_prefix(source_root).map_err(|_| {
                AppError::Validation("imported folder entry escaped its source root".into())
            })?;
            let destination_path = destination_root.join(relative);
            if file_type.is_dir() {
                fs::create_dir_all(&destination_path).map_err(|error| {
                    AppError::Internal(format!("failed to create imported directory: {error}"))
                })?;
                copy_tree(
                    source_root,
                    &source_path,
                    destination_root,
                    excluded,
                    files,
                    bytes,
                )?;
            } else if file_type.is_file() {
                *files += 1;
                *bytes = bytes.saturating_add(
                    entry
                        .metadata()
                        .map_err(|error| {
                            AppError::Validation(format!("cannot inspect imported file: {error}"))
                        })?
                        .len(),
                );
                if *files > MAX_FILES || *bytes > MAX_BYTES {
                    return Err(AppError::Validation(
                        "selected folder exceeds the Relay import limit (100000 files / 5 GiB)"
                            .into(),
                    ));
                }
                fs::copy(&source_path, &destination_path).map_err(|error| {
                    AppError::Internal(format!("failed to copy imported file: {error}"))
                })?;
            }
        }
        Ok(())
    }

    let result = (|| {
        let mut files = 0usize;
        let mut bytes = 0u64;
        copy_tree(source, source, &staging, EXCLUDED, &mut files, &mut bytes)?;
        initialize_managed_project_git(&staging)?;
        fs::rename(&staging, destination).map_err(|error| {
            AppError::Internal(format!("failed to finalize imported project: {error}"))
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

fn initialize_managed_project_git(path: &FsPath) -> AppResult<()> {
    fn run(path: &FsPath, args: &[&str]) -> AppResult<()> {
        let output = Command::new("git")
            .args(args)
            .current_dir(path)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .map_err(|error| AppError::Internal(format!("failed to start git: {error}")))?;
        if output.status.success() {
            return Ok(());
        }
        Err(AppError::Validation(format!(
            "failed to initialize imported project Git repository: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
    run(path, &["init", "-b", "main"])?;
    run(path, &["config", "user.name", "Relay Import"])?;
    run(
        path,
        &["config", "user.email", "relay-import@local.invalid"],
    )?;
    run(path, &["add", "-A"])?;
    run(
        path,
        &["commit", "--allow-empty", "-m", "Import project into Relay"],
    )
}

fn load_folder_reference_allowed_roots() -> anyhow::Result<Vec<PathBuf>> {
    let configured = std::env::var("HUMAN_FOLDER_REFERENCE_ALLOWED_ROOTS")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| std::env::var("AGENT_TRIGGER_ALLOWED_LOCAL_ROOTS").ok())
        .unwrap_or_default();
    configured
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|path| {
            if !path.is_absolute() {
                anyhow::bail!("HUMAN_FOLDER_REFERENCE_ALLOWED_ROOTS entries must be absolute");
            }
            let canonical = std::fs::canonicalize(&path).map_err(|error| {
                anyhow::anyhow!(
                    "cannot access HUMAN_FOLDER_REFERENCE_ALLOWED_ROOTS entry {}: {error}",
                    path.display()
                )
            })?;
            if !canonical.is_dir() {
                anyhow::bail!(
                    "HUMAN_FOLDER_REFERENCE_ALLOWED_ROOTS entry is not a directory: {}",
                    canonical.display()
                );
            }
            Ok(canonical)
        })
        .collect()
}

async fn stream_company_events_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Query(query): Query<RealtimeEventsQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let session_token = bearer_token(&headers)?.to_string();
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .list_company_realtime_events_for_human(human.id, company_id, 0, 1)?;
    let after_sequence_id = requested_realtime_cursor(&headers, query.after_sequence_id)
        .map(Ok)
        .unwrap_or_else(|| state.platform.latest_company_realtime_sequence(company_id))?;
    Ok(build_company_event_sse(
        state,
        company_id,
        after_sequence_id,
        query.limit.unwrap_or(200).clamp(1, 500),
        Some(session_token),
    ))
}

async fn stream_company_events_for_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Query(query): Query<RealtimeEventsQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let agent_key = agent_key_from_headers(&headers)
        .or_else(|| state.mcp_config.agent_key.clone())
        .ok_or_else(|| AppError::Unauthorized("missing x-agent-key or bearer token".into()))?;
    let agent = state.platform.authenticate_agent_key(&agent_key)?;
    state
        .platform
        .list_company_realtime_events_for_agent(agent.id, company_id, 0, 1)?;
    let after_sequence_id = requested_realtime_cursor(&headers, query.after_sequence_id)
        .map(Ok)
        .unwrap_or_else(|| state.platform.latest_company_realtime_sequence(company_id))?;
    Ok(build_company_event_sse(
        state,
        company_id,
        after_sequence_id,
        query.limit.unwrap_or(200).clamp(1, 500),
        None,
    ))
}

fn requested_realtime_cursor(headers: &HeaderMap, query_cursor: Option<i64>) -> Option<i64> {
    query_cursor
        .or_else(|| {
            headers
                .get("last-event-id")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<i64>().ok())
        })
        .map(|value| value.max(0))
}

fn build_company_event_sse(
    state: AppState,
    company_id: Uuid,
    after_sequence_id: i64,
    batch_limit: usize,
    presence_session_token: Option<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (sender, receiver) = mpsc::channel::<CompanyRealtimeEvent>(256);
    let mut signal_receiver = state.realtime_sender.subscribe();
    tokio::spawn(async move {
        let mut cursor = after_sequence_id;
        let mut next_presence_refresh = tokio::time::Instant::now();
        loop {
            if tokio::time::Instant::now() >= next_presence_refresh {
                if let Some(session_token) = presence_session_token.as_deref() {
                    if let Err(error) = state.platform.authenticate_human_session(session_token) {
                        tracing::info!(
                            company_id = %company_id,
                            error = %error,
                            "company SSE stream stopped because the Human session is no longer active"
                        );
                        return;
                    }
                }
                next_presence_refresh = tokio::time::Instant::now() + StdDuration::from_secs(30);
            }
            let events =
                match state
                    .platform
                    .read_company_realtime_events(company_id, cursor, batch_limit)
                {
                    Ok(events) => events,
                    Err(error) => {
                        tracing::error!(
                            company_id = %company_id,
                            cursor,
                            error = %error,
                            "company SSE stream stopped after repository failure"
                        );
                        return;
                    }
                };
            let had_events = !events.is_empty();
            for event in events {
                cursor = cursor.max(event.sequence_id);
                if sender.send(event).await.is_err() {
                    return;
                }
            }
            if had_events {
                continue;
            }

            tokio::select! {
                signal = signal_receiver.recv() => {
                    match signal {
                        Ok(signal) if signal.company_id == company_id && signal.sequence_id > cursor => {}
                        Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => {
                            tokio::time::sleep(StdDuration::from_secs(1)).await;
                        }
                    }
                }
                _ = tokio::time::sleep(StdDuration::from_secs(1)) => {}
            }
        }
    });

    let stream = futures_util::stream::unfold(receiver, |mut receiver| async move {
        receiver.recv().await.map(|event| {
            let data = serde_json::to_string(&event).unwrap_or_else(|_| "{}".into());
            let sse_event = Event::default()
                .id(event.sequence_id.to_string())
                .event(event.event_type)
                .data(data);
            (Ok(sse_event), receiver)
        })
    });
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(StdDuration::from_secs(15))
            .text("keep-alive"),
    )
}

async fn create_company_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<CreateCompanyAgentRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let profession = company_profession_by_key(&input.profession_key)
        .ok_or_else(|| ApiError::from(AppError::Validation("unsupported profession_key".into())))?;
    let result = state
        .platform
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: human.id,
            company_id,
            display_name: input.display_name,
            handle: input.handle,
            persona: input.persona,
            org_unit_id: input.org_unit_id,
            job_title: Some(profession.label),
            role_key: input.role_key,
            reports_to_membership_id: input.reports_to_membership_id,
        })?;
    Ok(Json(serde_json::json!({ "result": result })))
}

async fn create_org_unit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<CreateOrgUnitRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let org_unit = state.platform.create_org_unit(CreateOrgUnitInput {
        human_user_id: human.id,
        company_id,
        parent_org_unit_id: input.parent_org_unit_id,
        name: input.name,
        unit_type: input.unit_type,
        sort_order: input.sort_order,
    })?;
    Ok(Json(serde_json::json!({ "org_unit": org_unit })))
}

async fn update_company_agent_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateCompanyAgentPermissionsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let membership = state.platform.update_company_agent_staffing_permissions(
        UpdateCompanyAgentPermissionsInput {
            human_user_id: human.id,
            company_id,
            agent_id,
            staffing_permissions: input.staffing_permissions,
            project_permissions: input.project_permissions,
            staffing_scope_org_unit_id: input.staffing_scope_org_unit_id,
            reason: input.reason,
        },
    )?;
    Ok(Json(serde_json::json!({ "membership": membership })))
}

async fn update_company_agent_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateCompanyAgentRoleRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let membership = state
        .platform
        .update_company_agent_role(UpdateCompanyAgentRoleInput {
            human_user_id: human.id,
            company_id,
            agent_id,
            role_key: input.role_key,
            reason: input.reason,
        })?;
    Ok(Json(serde_json::json!({ "membership": membership })))
}

async fn update_company_agent_profession(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateCompanyAgentProfessionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let membership =
        state
            .platform
            .update_company_agent_profession(UpdateCompanyAgentProfessionInput {
                human_user_id: human.id,
                company_id,
                agent_id,
                profession_key: input.profession_key,
                reason: input.reason,
            })?;
    Ok(Json(serde_json::json!({ "membership": membership })))
}

async fn list_company_codex_runner_profiles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let profiles = state
        .platform
        .list_company_codex_runner_profiles_for_human(
            ListCompanyCodexRunnerProfilesForHumanInput {
                human_user_id: human.id,
                company_id,
            },
        )?;
    Ok(Json(serde_json::json!({ "profiles": profiles })))
}

async fn get_company_codex_environments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    let environment = state
        .codex_control_store
        .environment_for_company(company_id)?;
    Ok(Json(serde_json::json!(environment)))
}

async fn get_company_codex_cli_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    let settings = state.codex_control_store.company_cli_settings(company_id)?;
    Ok(Json(serde_json::json!({ "settings": settings })))
}

async fn update_company_codex_cli_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<UpdateCompanyCodexCliSettingsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    let settings =
        state
            .codex_control_store
            .save_company_cli_settings(CompanyCodexCliSettings {
                company_id,
                model: input.model,
                reasoning_effort: input.reasoning_effort,
                reasoning_summary: input.reasoning_summary,
                verbosity: input.verbosity,
                personality: input.personality,
                service_tier: input.service_tier,
                approval_policy: input.approval_policy,
                sandbox_mode: input.sandbox_mode,
                network_access: input.network_access,
                web_search: input.web_search,
                feature_multi_agent: input.feature_multi_agent,
                feature_remote_plugin: input.feature_remote_plugin,
                feature_hooks: input.feature_hooks,
                feature_goals: input.feature_goals,
                feature_shell_tool: input.feature_shell_tool,
                updated_at: now_utc(),
            })?;
    Ok(Json(serde_json::json!({ "settings": settings })))
}

async fn request_codex_cli_install(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    let runtime = state.codex_control_store.enqueue_cli_install()?;
    Ok(Json(serde_json::json!({ "runtime": runtime })))
}

async fn request_codex_cli_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    let runtime = state.codex_control_store.enqueue_cli_update()?;
    Ok(Json(serde_json::json!({ "runtime": runtime })))
}

async fn refresh_company_codex_mcp_servers(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<CodexMcpRefreshRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    state
        .codex_control_store
        .enqueue_mcp_refresh(company_id, input.target_selector)?;
    Ok(Json(serde_json::json!({ "accepted": true })))
}

async fn add_company_codex_mcp_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<CodexMcpServerInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    state
        .codex_control_store
        .enqueue_mcp_add(company_id, input)?;
    Ok(Json(serde_json::json!({ "accepted": true })))
}

async fn remove_company_codex_mcp_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, target_selector, server_name)): Path<(Uuid, String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    state
        .codex_control_store
        .enqueue_mcp_remove(company_id, target_selector, server_name)?;
    Ok(Json(serde_json::json!({ "accepted": true })))
}

async fn create_company_codex_auth_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<CreateCodexAuthProfileRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    let profile = state.codex_control_store.create_auth_profile(
        company_id,
        input.name,
        input.api_key,
        input.base_url,
    )?;
    Ok(Json(serde_json::json!({ "profile": profile })))
}

async fn update_company_codex_auth_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, profile_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateCodexAuthProfileRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    let profile = state.codex_control_store.update_auth_profile(
        company_id,
        profile_id,
        input.name,
        input.api_key,
        input.base_url,
    )?;
    Ok(Json(serde_json::json!({ "profile": profile })))
}

async fn delete_company_codex_auth_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, profile_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    let environment = state
        .codex_control_store
        .environment_for_company(company_id)?;
    let profile = environment
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| AppError::NotFound("Codex authentication profile not found".into()))?;
    let runner_profiles = state
        .platform
        .list_company_codex_runner_profiles_for_human(
            ListCompanyCodexRunnerProfilesForHumanInput {
                human_user_id: human.id,
                company_id,
            },
        )?;
    if runner_profiles
        .iter()
        .any(|runner| runner.profile.codex_profile == profile.selector)
    {
        return Err(AppError::Conflict(
            "Codex authentication profile is still used by a runner profile".into(),
        )
        .into());
    }
    let profile = state
        .codex_control_store
        .request_delete_auth_profile(company_id, profile_id)?;
    Ok(Json(serde_json::json!({ "profile": profile })))
}

async fn list_company_codex_plugins(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Query(query): Query<CodexPluginsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let view = state.platform.list_company_codex_plugins_for_human(
        ListCompanyCodexPluginsForHumanInput {
            human_user_id: human.id,
            company_id,
            operation_limit: query.operation_limit.unwrap_or(50),
        },
    )?;
    Ok(Json(serde_json::json!(view)))
}

async fn request_codex_plugin_operation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<CodexPluginOperationRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let operation = state.platform.request_codex_plugin_operation_for_human(
        RequestCodexPluginOperationForHumanInput {
            human_user_id: human.id,
            company_id,
            target_runner_id: input.target_runner_id,
            operation: input.operation,
            plugin_id: input.plugin_id,
        },
    )?;
    Ok(Json(serde_json::json!({ "operation": operation })))
}

async fn create_company_codex_runner_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<UpsertCompanyCodexRunnerProfileRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    ensure_company_managed_codex_profile(&state, company_id, &input.codex_profile)?;
    let profile = state
        .platform
        .upsert_company_codex_runner_profile_for_human(
            UpsertCompanyCodexRunnerProfileForHumanInput {
                human_user_id: human.id,
                company_id,
                profile_id: None,
                name: input.name,
                interval_seconds: input.interval_seconds,
                codex_profile: input.codex_profile,
                model: input.model,
                reasoning_effort: input.reasoning_effort,
                reasoning_summary: input.reasoning_summary,
                verbosity: input.verbosity,
                personality: input.personality,
                service_tier: input.service_tier,
                sandbox_mode: input.sandbox_mode,
                approval_policy: input.approval_policy,
                network_access: input.network_access,
                web_search: input.web_search,
                feature_multi_agent: input.feature_multi_agent,
                feature_remote_plugin: input.feature_remote_plugin,
                feature_hooks: input.feature_hooks,
                feature_goals: input.feature_goals,
                feature_shell_tool: input.feature_shell_tool,
                max_run_seconds: input.max_run_seconds,
                is_default: input.is_default,
            },
        )?;
    Ok(Json(serde_json::json!({ "profile": profile })))
}

async fn update_company_codex_runner_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, profile_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpsertCompanyCodexRunnerProfileRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    ensure_company_managed_codex_profile(&state, company_id, &input.codex_profile)?;
    let profile = state
        .platform
        .upsert_company_codex_runner_profile_for_human(
            UpsertCompanyCodexRunnerProfileForHumanInput {
                human_user_id: human.id,
                company_id,
                profile_id: Some(profile_id),
                name: input.name,
                interval_seconds: input.interval_seconds,
                codex_profile: input.codex_profile,
                model: input.model,
                reasoning_effort: input.reasoning_effort,
                reasoning_summary: input.reasoning_summary,
                verbosity: input.verbosity,
                personality: input.personality,
                service_tier: input.service_tier,
                sandbox_mode: input.sandbox_mode,
                approval_policy: input.approval_policy,
                network_access: input.network_access,
                web_search: input.web_search,
                feature_multi_agent: input.feature_multi_agent,
                feature_remote_plugin: input.feature_remote_plugin,
                feature_hooks: input.feature_hooks,
                feature_goals: input.feature_goals,
                feature_shell_tool: input.feature_shell_tool,
                max_run_seconds: input.max_run_seconds,
                is_default: input.is_default,
            },
        )?;
    Ok(Json(serde_json::json!({ "profile": profile })))
}

async fn delete_company_codex_runner_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, profile_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .delete_company_codex_runner_profile_for_human(
            DeleteCompanyCodexRunnerProfileForHumanInput {
                human_user_id: human.id,
                company_id,
                profile_id,
            },
        )?;
    Ok(StatusCode::NO_CONTENT)
}

fn ensure_company_managed_codex_profile(
    state: &AppState,
    company_id: Uuid,
    selector: &str,
) -> AppResult<()> {
    if selector.starts_with("relay_")
        && state
            .codex_control_store
            .find_active_company_profile(company_id, selector)?
            .is_none()
    {
        return Err(AppError::Validation(
            "managed Codex authentication profile is unavailable or not active".into(),
        ));
    }
    Ok(())
}

async fn list_company_approvals(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Query(query): Query<ApprovalRequestsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let approvals = state.platform.list_agent_tool_approvals_for_human(
        human.id,
        company_id,
        query.status.as_deref(),
        query.limit.unwrap_or(100),
    )?;
    Ok(Json(serde_json::json!({ "approvals": approvals })))
}

async fn approve_company_approval(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, approval_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<ReviewApprovalRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let approval = state
        .platform
        .approve_agent_tool_approval(ReviewAgentToolApprovalInput {
            human_user_id: human.id,
            company_id,
            approval_request_id: approval_id,
            review_note: input.review_note,
        })?;
    Ok(Json(serde_json::json!({ "approval": approval })))
}

async fn reject_company_approval(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, approval_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<ReviewApprovalRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let approval = state
        .platform
        .reject_agent_tool_approval(ReviewAgentToolApprovalInput {
            human_user_id: human.id,
            company_id,
            approval_request_id: approval_id,
            review_note: input.review_note,
        })?;
    Ok(Json(serde_json::json!({ "approval": approval })))
}

async fn get_company_agent_codex_trigger(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let trigger = state.platform.get_company_agent_codex_trigger_for_human(
        GetCompanyAgentCodexTriggerForHumanInput {
            human_user_id: human.id,
            company_id,
            agent_id,
        },
    )?;
    Ok(Json(serde_json::json!({ "trigger": trigger })))
}

async fn upsert_company_agent_codex_trigger(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpsertCompanyAgentCodexTriggerRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    if let Some(runner_profile_id) = input.runner_profile_id {
        let runner_profiles = state
            .platform
            .list_company_codex_runner_profiles_for_human(
                ListCompanyCodexRunnerProfilesForHumanInput {
                    human_user_id: human.id,
                    company_id,
                },
            )?;
        let selector = runner_profiles
            .iter()
            .find(|view| view.profile.id == runner_profile_id)
            .map(|view| view.profile.codex_profile.as_str())
            .ok_or_else(|| AppError::NotFound("Codex runner profile not found".into()))?;
        ensure_company_managed_codex_profile(&state, company_id, selector)?;
    } else if let Some(selector) = input.codex_profile.as_deref() {
        ensure_company_managed_codex_profile(&state, company_id, selector)?;
    }
    let trigger = state
        .platform
        .upsert_company_agent_codex_trigger_for_human(
            UpsertCompanyAgentCodexTriggerForHumanInput {
                human_user_id: human.id,
                company_id,
                agent_id,
                interval_seconds: input.interval_seconds,
                codex_profile: input.codex_profile,
                model: input.model,
                reasoning_effort: input.reasoning_effort,
                reasoning_summary: input.reasoning_summary,
                verbosity: input.verbosity,
                personality: input.personality,
                service_tier: input.service_tier,
                sandbox_mode: input.sandbox_mode,
                approval_policy: input.approval_policy,
                network_access: input.network_access,
                web_search: input.web_search,
                feature_multi_agent: input.feature_multi_agent,
                feature_remote_plugin: input.feature_remote_plugin,
                feature_hooks: input.feature_hooks,
                feature_goals: input.feature_goals,
                feature_shell_tool: input.feature_shell_tool,
                max_run_seconds: input.max_run_seconds,
                runner_profile_id: input.runner_profile_id,
            },
        )?;
    Ok(Json(serde_json::json!({ "trigger": trigger })))
}

async fn pause_company_agent_codex_trigger(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let trigger = state.platform.pause_company_agent_codex_trigger_for_human(
        SetCompanyAgentCodexTriggerStatusForHumanInput {
            human_user_id: human.id,
            company_id,
            agent_id,
        },
    )?;
    Ok(Json(serde_json::json!({ "trigger": trigger })))
}

async fn resume_company_agent_codex_trigger(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let trigger = state
        .platform
        .resume_company_agent_codex_trigger_for_human(
            SetCompanyAgentCodexTriggerStatusForHumanInput {
                human_user_id: human.id,
                company_id,
                agent_id,
            },
        )?;
    Ok(Json(serde_json::json!({ "trigger": trigger })))
}

async fn run_company_agent_codex_trigger_now(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let trigger = state
        .platform
        .run_company_agent_codex_trigger_now_for_human(
            SetCompanyAgentCodexTriggerStatusForHumanInput {
                human_user_id: human.id,
                company_id,
                agent_id,
            },
        )?;
    Ok(Json(serde_json::json!({ "trigger": trigger })))
}

async fn list_company_agent_codex_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<CodexRunsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let runs = state.platform.list_company_agent_codex_runs_for_human(
        ListCompanyAgentCodexRunsForHumanInput {
            human_user_id: human.id,
            company_id,
            agent_id,
            limit: query.limit.unwrap_or(20),
        },
    )?;
    Ok(Json(serde_json::json!({ "runs": runs })))
}

async fn get_company_governance_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let policy = state
        .platform
        .get_company_governance_policy_for_human(human.id, company_id)?;
    Ok(Json(serde_json::json!({ "governance_policy": policy })))
}

async fn publish_company_governance_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<PublishCompanyGovernancePolicyRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let current = state
        .platform
        .get_company_governance_policy_for_human(human.id, company_id)?;
    let policy =
        state
            .platform
            .publish_company_governance_policy(PublishCompanyGovernancePolicyInput {
                human_user_id: human.id,
                company_id,
                settings: CompanyGovernancePolicySettings {
                    agent_staff_limit: input.agent_staff_limit,
                    delegated_agent_hiring_enabled: input.delegated_agent_hiring_enabled,
                    delegated_agent_suspension_enabled: input.delegated_agent_suspension_enabled,
                    delegated_agent_termination_enabled: input.delegated_agent_termination_enabled,
                    max_active_projects: input.max_active_projects,
                    max_project_members: input.max_project_members,
                    daily_delegated_hire_limit: input
                        .daily_delegated_hire_limit
                        .unwrap_or(current.effective_settings.daily_delegated_hire_limit),
                    daily_delegated_suspension_limit: input
                        .daily_delegated_suspension_limit
                        .unwrap_or(current.effective_settings.daily_delegated_suspension_limit),
                    daily_delegated_termination_limit: input
                        .daily_delegated_termination_limit
                        .unwrap_or(current.effective_settings.daily_delegated_termination_limit),
                    managed_workspace_root: current.effective_settings.managed_workspace_root,
                    skill_language: input
                        .skill_language
                        .unwrap_or(current.effective_settings.skill_language),
                },
                notes: input.notes,
            })?;
    Ok(Json(serde_json::json!({ "governance_policy": policy })))
}

async fn update_company_skill_language(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<UpdateCompanySkillLanguageRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let current = state
        .platform
        .get_company_governance_policy_for_human(human.id, company_id)?;
    let skill_language = input.skill_language.trim();
    if !matches!(skill_language, "zh-CN" | "en") {
        return Err(ApiError(AppError::Validation(
            "skill_language must be zh-CN or en".into(),
        )));
    }
    if current.effective_settings.skill_language == skill_language {
        return Ok(Json(serde_json::json!({ "governance_policy": current })));
    }
    let mut settings = current.effective_settings;
    settings.skill_language = skill_language.into();
    let policy =
        state
            .platform
            .publish_company_governance_policy(PublishCompanyGovernancePolicyInput {
                human_user_id: human.id,
                company_id,
                settings,
                notes: Some(format!(
                    "Human changed Relay Skill language to {skill_language}"
                )),
            })?;
    Ok(Json(serde_json::json!({ "governance_policy": policy })))
}

async fn get_agent_trigger_preferences(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .get_company_governance_policy_for_human(human.id, company_id)?;
    let preferences = state
        .codex_control_store
        .agent_trigger_preferences(agent_trigger_batch_size_from_env())?;
    Ok(Json(serde_json::json!({ "preferences": preferences })))
}

async fn update_agent_trigger_preferences(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<UpdateAgentTriggerPreferencesRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .ensure_human_can_manage_company_codex(human.id, company_id)?;
    let preferences = state.codex_control_store.save_agent_trigger_preferences(
        input.batch_size,
        human.id,
        agent_trigger_batch_size_from_env(),
    )?;
    Ok(Json(serde_json::json!({ "preferences": preferences })))
}

async fn update_company_workspace_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<UpdateCompanyWorkspaceRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let current = state
        .platform
        .get_company_governance_policy_for_human(human.id, company_id)?;
    let managed_workspace_root = input
        .managed_workspace_root
        .filter(|value| !value.trim().is_empty())
        .map(validate_company_workspace_root)
        .transpose()?;
    if current.effective_settings.managed_workspace_root == managed_workspace_root {
        return Ok(Json(serde_json::json!({
            "governance_policy": current,
            "resolved_workspace_root": resolve_company_workspace_root(
                managed_workspace_root.as_deref(),
                company_id,
            )?
        })));
    }
    let mut settings = current.effective_settings;
    settings.managed_workspace_root = managed_workspace_root;
    let policy =
        state
            .platform
            .publish_company_governance_policy(PublishCompanyGovernancePolicyInput {
                human_user_id: human.id,
                company_id,
                settings,
                notes: Some("Human updated the Relay managed project workspace".into()),
            })?;
    Ok(Json(serde_json::json!({
        "governance_policy": policy,
        "resolved_workspace_root": resolve_company_workspace_root(
            policy.effective_settings.managed_workspace_root.as_deref(),
            company_id,
        )?
    })))
}

async fn create_company_project_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<CreateCompanyProjectRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let policy = state
        .platform
        .get_company_governance_policy_for_human(human.id, company_id)?;
    let workspace_root = resolve_company_workspace_root(
        policy.effective_settings.managed_workspace_root.as_deref(),
        company_id,
    )?;
    fs::create_dir_all(&workspace_root).map_err(|error| {
        AppError::Internal(format!(
            "failed to create managed workspace {}: {error}",
            workspace_root.display()
        ))
    })?;

    let project_id = Uuid::new_v4();
    let destination = workspace_root.join(managed_project_directory_name(&input.name, project_id));
    let description = input.description.clone().unwrap_or_default();
    let mut type_evidence = Vec::new();
    let mut imported_local_folder = false;

    match input.source_kind.as_str() {
        "local_folder" => {
            let requested = input.source_local_path.as_deref().ok_or_else(|| {
                AppError::Validation("source_local_path is required for local_folder".into())
            })?;
            let source =
                validate_project_source_folder(requested, &state.folder_reference_allowed_roots)?;
            if destination.starts_with(&source) || source == workspace_root {
                return Err(AppError::Validation(
                    "managed destination cannot be inside the imported source folder".into(),
                )
                .into());
            }
            type_evidence = collect_directory_structure(&source)?;
            let source_for_copy = source.clone();
            let destination_for_copy = destination.clone();
            tokio::task::spawn_blocking(move || {
                import_project_folder(&source_for_copy, &destination_for_copy)
            })
            .await
            .map_err(|error| {
                AppError::Internal(format!("project import task failed: {error}"))
            })??;
            imported_local_folder = true;
        }
        "git" => {
            if input
                .git_remote_url
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                return Err(AppError::Validation(
                    "git_remote_url is required for git source".into(),
                )
                .into());
            }
        }
        _ => {
            return Err(
                AppError::Validation("source_kind must be local_folder or git".into()).into(),
            )
        }
    }

    let (inferred_type, inferred_confidence, inferred_evidence) =
        infer_company_project_type(&input.name, &description, &type_evidence);
    let human_selected_type = input
        .project_type
        .as_ref()
        .is_some_and(|value| !value.is_empty());
    let project_type = input.project_type.clone().unwrap_or(inferred_type);
    let project_result =
        state
            .platform
            .create_company_project_for_human(CreateCompanyProjectForHumanInput {
                human_user_id: human.id,
                company_id,
                owner_agent_id: input.owner_agent_id,
                name: input.name,
                description: input.description,
                member_agent_ids: input.member_agent_ids,
                project_type: Some(project_type),
                project_type_source: Some(if human_selected_type {
                    PROJECT_TYPE_SOURCE_HUMAN.into()
                } else if input.source_kind == "local_folder" {
                    PROJECT_TYPE_SOURCE_FOLDER.into()
                } else {
                    PROJECT_TYPE_SOURCE_DESCRIPTION.into()
                }),
                project_type_confidence: Some(if human_selected_type {
                    100
                } else {
                    inferred_confidence
                }),
                project_type_evidence: if type_evidence.is_empty() {
                    inferred_evidence
                } else {
                    type_evidence.into_iter().take(24).collect()
                },
                project_id: Some(project_id),
            });
    let project = match project_result {
        Ok(project) => project,
        Err(error) => {
            if imported_local_folder {
                let _ = fs::remove_dir_all(&destination);
            }
            return Err(error.into());
        }
    };

    let git = if imported_local_folder {
        Some(
            state
                .platform
                .configure_managed_local_project_git_for_human(
                    ConfigureManagedLocalProjectGitForHumanInput {
                        human_user_id: human.id,
                        company_id,
                        project_id,
                        managed_local_path: destination.to_string_lossy().into_owned(),
                    },
                )?,
        )
    } else {
        Some(state.platform.upsert_company_project_git_for_human(
            UpsertCompanyProjectGitForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
                remote_url: input.git_remote_url.unwrap_or_default(),
                host_local_path: Some(destination.to_string_lossy().into_owned()),
                default_branch: input.default_branch,
                auth_profile: input.auth_profile,
                allow_agent_push: Some(true),
                branch_prefix: Some("relay/".into()),
            },
        )?)
    };
    Ok(Json(serde_json::json!({
        "project": project,
        "git": git,
        "managed_local_path": destination,
    })))
}

fn normalize_uploaded_project_path(raw: &str) -> AppResult<Option<String>> {
    const EXCLUDED: &[&str] = &[
        ".git",
        ".relay",
        ".relay-agent-trigger",
        "node_modules",
        "target",
    ];
    let normalized = raw.replace('\\', "/");
    if normalized.is_empty()
        || normalized.len() > 4_096
        || normalized.starts_with('/')
        || normalized.ends_with('/')
        || normalized.chars().any(char::is_control)
    {
        return Err(AppError::Validation(
            "uploaded project contains an invalid relative path".into(),
        ));
    }
    let components = normalized.split('/').collect::<Vec<_>>();
    if components
        .iter()
        .any(|component| component.is_empty() || matches!(*component, "." | ".."))
    {
        return Err(AppError::Validation(
            "uploaded project path may not contain empty, current, or parent components".into(),
        ));
    }
    if components
        .iter()
        .any(|component| EXCLUDED.contains(component))
    {
        return Ok(None);
    }
    Ok(Some(normalized))
}

async fn import_company_project_folder_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    const MAX_FILES: usize = 100_000;
    const MAX_BYTES: u64 = 5 * 1024 * 1024 * 1024;

    let human = authenticate_human_request(&state, &headers)?;
    let metadata_field = multipart
        .next_field()
        .await
        .map_err(|error| AppError::Validation(format!("invalid project import form: {error}")))?
        .ok_or_else(|| AppError::Validation("project import metadata is required".into()))?;
    if metadata_field.name() != Some("metadata") {
        return Err(AppError::Validation(
            "project import metadata must be the first multipart field".into(),
        )
        .into());
    }
    let metadata: ImportCompanyProjectFolderRequest = serde_json::from_str(
        &metadata_field
            .text()
            .await
            .map_err(|error| AppError::Validation(format!("invalid project metadata: {error}")))?,
    )
    .map_err(|error| AppError::Validation(format!("invalid project metadata: {error}")))?;
    if metadata.file_paths.len() > MAX_FILES {
        return Err(AppError::Validation(
            "selected folder exceeds the Relay import limit (100000 files / 5 GiB)".into(),
        )
        .into());
    }

    let mut normalized_paths = Vec::with_capacity(metadata.file_paths.len());
    let mut unique_paths = HashSet::new();
    for raw in &metadata.file_paths {
        let normalized = normalize_uploaded_project_path(raw)?;
        if let Some(path) = normalized.as_ref() {
            if !unique_paths.insert(path.clone()) {
                return Err(AppError::Validation(format!(
                    "uploaded project contains a duplicate path: {path}"
                ))
                .into());
            }
        }
        normalized_paths.push(normalized);
    }

    let policy = state
        .platform
        .get_company_governance_policy_for_human(human.id, company_id)?;
    let workspace_root = resolve_company_workspace_root(
        policy.effective_settings.managed_workspace_root.as_deref(),
        company_id,
    )?;
    fs::create_dir_all(&workspace_root).map_err(|error| {
        AppError::Internal(format!(
            "failed to create managed workspace {}: {error}",
            workspace_root.display()
        ))
    })?;
    let project_id = Uuid::new_v4();
    let destination =
        workspace_root.join(managed_project_directory_name(&metadata.name, project_id));
    let staging = workspace_root.join(format!(".relay-upload-{project_id}"));
    fs::create_dir(&staging).map_err(|error| {
        AppError::Internal(format!(
            "failed to create project upload staging directory: {error}"
        ))
    })?;

    let upload_result: Result<(usize, u64), ApiError> = async {
        let mut received_indices = HashSet::new();
        let mut total_bytes = 0u64;
        while let Some(mut field) = multipart.next_field().await.map_err(|error| {
            AppError::Validation(format!("invalid project upload stream: {error}"))
        })? {
            let field_name = field.name().unwrap_or_default();
            let Some(index) = field_name
                .strip_prefix("file_")
                .and_then(|value| value.parse::<usize>().ok())
            else {
                return Err(AppError::Validation(
                    "project upload contains an unexpected multipart field".into(),
                )
                .into());
            };
            let relative_path = normalized_paths.get(index).ok_or_else(|| {
                AppError::Validation("project upload file index is invalid".into())
            })?;
            if !received_indices.insert(index) {
                return Err(AppError::Validation(
                    "project upload contains a duplicate file index".into(),
                )
                .into());
            }
            let Some(relative_path) = relative_path else {
                while field
                    .chunk()
                    .await
                    .map_err(|error| {
                        AppError::Validation(format!("invalid upload chunk: {error}"))
                    })?
                    .is_some()
                {}
                continue;
            };
            let output_path = staging.join(relative_path);
            if let Some(parent) = output_path.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|error| {
                    AppError::Internal(format!("failed to create imported directory: {error}"))
                })?;
            }
            let mut output = tokio::fs::File::create(&output_path)
                .await
                .map_err(|error| {
                    AppError::Internal(format!("failed to create imported file: {error}"))
                })?;
            while let Some(chunk) = field.chunk().await.map_err(|error| {
                AppError::Validation(format!("invalid project upload chunk: {error}"))
            })? {
                total_bytes = total_bytes.saturating_add(chunk.len() as u64);
                if total_bytes > MAX_BYTES {
                    return Err(AppError::Validation(
                        "selected folder exceeds the Relay import limit (100000 files / 5 GiB)"
                            .into(),
                    )
                    .into());
                }
                output.write_all(&chunk).await.map_err(|error| {
                    AppError::Internal(format!("failed to write imported file: {error}"))
                })?;
            }
            output.flush().await.map_err(|error| {
                AppError::Internal(format!("failed to flush imported file: {error}"))
            })?;
        }
        if received_indices.len() != normalized_paths.len() {
            return Err(AppError::Validation(
                "project upload did not include every selected file".into(),
            )
            .into());
        }
        Ok((received_indices.len(), total_bytes))
    }
    .await;
    if let Err(error) = upload_result {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(error);
    }

    let staging_for_finalize = staging.clone();
    let destination_for_finalize = destination.clone();
    let finalize_result = tokio::task::spawn_blocking(move || -> AppResult<()> {
        initialize_managed_project_git(&staging_for_finalize)?;
        fs::rename(&staging_for_finalize, &destination_for_finalize).map_err(|error| {
            AppError::Internal(format!("failed to finalize uploaded project: {error}"))
        })
    })
    .await
    .map_err(|error| AppError::Internal(format!("project finalization task failed: {error}")))?;
    if let Err(error) = finalize_result {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(error.into());
    }

    let description = metadata.description.clone().unwrap_or_default();
    let type_evidence = normalized_paths
        .iter()
        .filter_map(Clone::clone)
        .take(5_000)
        .collect::<Vec<_>>();
    let (inferred_type, inferred_confidence, inferred_evidence) =
        infer_company_project_type(&metadata.name, &description, &type_evidence);
    let human_selected_type = metadata
        .project_type
        .as_ref()
        .is_some_and(|value| !value.is_empty());
    let project_type = metadata.project_type.clone().unwrap_or(inferred_type);
    let project_result =
        state
            .platform
            .create_company_project_for_human(CreateCompanyProjectForHumanInput {
                human_user_id: human.id,
                company_id,
                owner_agent_id: metadata.owner_agent_id,
                name: metadata.name,
                description: metadata.description,
                member_agent_ids: metadata.member_agent_ids,
                project_type: Some(project_type),
                project_type_source: Some(if human_selected_type {
                    PROJECT_TYPE_SOURCE_HUMAN.into()
                } else {
                    PROJECT_TYPE_SOURCE_FOLDER.into()
                }),
                project_type_confidence: Some(if human_selected_type {
                    100
                } else {
                    inferred_confidence
                }),
                project_type_evidence: if type_evidence.is_empty() {
                    inferred_evidence
                } else {
                    type_evidence.into_iter().take(24).collect()
                },
                project_id: Some(project_id),
            });
    let project = match project_result {
        Ok(project) => project,
        Err(error) => {
            let _ = fs::remove_dir_all(&destination);
            return Err(error.into());
        }
    };
    let git = state
        .platform
        .configure_managed_local_project_git_for_human(
            ConfigureManagedLocalProjectGitForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
                managed_local_path: destination.to_string_lossy().into_owned(),
            },
        )?;
    Ok(Json(serde_json::json!({
        "project": project,
        "git": git,
        "managed_local_path": destination,
    })))
}

async fn get_company_project_git(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let git =
        state
            .platform
            .get_company_project_git_for_human(GetCompanyProjectGitForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
            })?;
    let automatic_profile = github_token_profile_name(project_id);
    let github_token_configured = git.as_ref().is_some_and(|git| {
        git.auth_profile.as_deref() == Some(automatic_profile.as_str())
            && state.git_credential_store.has_github_token(project_id)
    });
    let managed_profile = managed_token_profile_name(project_id);
    let managed_token_configured = git.as_ref().is_some_and(|git| {
        git.auth_profile.as_deref() == Some(managed_profile.as_str())
            && state.git_credential_store.has_managed_git_token(project_id)
    });
    Ok(Json(serde_json::json!({
        "git": git,
        "github_token_configured": github_token_configured,
        "managed_token_configured": managed_token_configured,
    })))
}

async fn upsert_company_project_git(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpsertCompanyProjectGitRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let existing =
        state
            .platform
            .get_company_project_git_for_human(GetCompanyProjectGitForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
            })?;
    let github_token = input
        .github_token
        .as_deref()
        .filter(|token| !token.is_empty())
        .map(str::to_string);
    if input.clear_github_token && github_token.is_some() {
        return Err(AppError::Validation(
            "github_token and clear_github_token cannot be used together".into(),
        )
        .into());
    }
    if let Some(token) = github_token.as_deref() {
        validate_github_token(token)?;
        if !input.remote_url.starts_with("https://") {
            return Err(AppError::Validation(
                "GitHub Token requires an https Git Remote URL".into(),
            )
            .into());
        }
    }
    let automatic_profile = github_token_profile_name(project_id);
    let auth_profile = if github_token.is_some() {
        Some(automatic_profile.clone())
    } else if input.clear_github_token {
        None
    } else {
        input
            .auth_profile
            .clone()
            .or_else(|| existing.as_ref().and_then(|git| git.auth_profile.clone()))
    };
    let should_remove_github_token = input.clear_github_token
        || (existing
            .as_ref()
            .and_then(|git| git.auth_profile.as_deref())
            == Some(automatic_profile.as_str())
            && auth_profile.as_deref() != Some(automatic_profile.as_str()));
    let git = state.platform.upsert_company_project_git_for_human(
        UpsertCompanyProjectGitForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            remote_url: input.remote_url,
            host_local_path: input.host_local_path,
            default_branch: input.default_branch,
            auth_profile,
            allow_agent_push: input.allow_agent_push,
            branch_prefix: input.branch_prefix,
        },
    )?;
    let credential_result = if let Some(token) = github_token.as_deref() {
        state
            .git_credential_store
            .store_github_token(project_id, token)
    } else if should_remove_github_token {
        state.git_credential_store.remove_github_token(project_id)
    } else {
        Ok(())
    };
    if let Err(error) = credential_result {
        if let Err(rollback_error) =
            rollback_company_project_git(&state, human.id, company_id, project_id, existing)
        {
            tracing::error!(
                project_id = %project_id,
                error = %rollback_error,
                "failed to roll back project Git configuration after credential write failure"
            );
        }
        return Err(error.into());
    }
    let github_token_configured = git.auth_profile.as_deref() == Some(automatic_profile.as_str())
        && state.git_credential_store.has_github_token(project_id);
    let managed_profile = managed_token_profile_name(project_id);
    let managed_token_configured = git.auth_profile.as_deref() == Some(managed_profile.as_str())
        && state.git_credential_store.has_managed_git_token(project_id);
    Ok(Json(serde_json::json!({
        "git": git,
        "github_token_configured": github_token_configured,
        "managed_token_configured": managed_token_configured,
    })))
}

async fn delete_company_project_git(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .delete_company_project_git_for_human(DeleteCompanyProjectGitForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
        })?;
    state
        .git_credential_store
        .remove_project_tokens(project_id)?;
    Ok(Json(serde_json::json!({ "configured": false })))
}

async fn update_company_project_rule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateCompanyProjectRuleRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let rule = state.platform.update_company_project_rule_for_human(
        UpdateCompanyProjectRuleForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            content: input.content,
        },
    )?;
    Ok(Json(serde_json::json!({ "rule": rule })))
}

async fn request_company_project_rule_generation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<RequestCompanyProjectRuleGenerationRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    state
        .platform
        .request_company_project_rule_generation_for_human(
            RequestCompanyProjectRuleGenerationForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
                agent_id: input.agent_id,
                instructions: input.instructions,
            },
        )?;
    Ok(Json(serde_json::json!({ "requested": true })))
}

async fn upsert_company_project_asset_refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpsertCompanyProjectAssetRefreshRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let asset_refresh = state
        .platform
        .upsert_company_project_asset_refresh_for_human(
            UpsertCompanyProjectAssetRefreshForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
                maintainer_agent_id: input.maintainer_agent_id,
                interval_minutes: input.interval_minutes,
                enabled: input.enabled,
                run_now: input.run_now,
            },
        )?;
    Ok(Json(serde_json::json!({ "asset_refresh": asset_refresh })))
}

fn rollback_company_project_git(
    state: &AppState,
    human_user_id: Uuid,
    company_id: Uuid,
    project_id: Uuid,
    existing: Option<ai_chat_application::CompanyProjectGitAdminView>,
) -> Result<(), AppError> {
    if let Some(existing) = existing {
        state.platform.upsert_company_project_git_for_human(
            UpsertCompanyProjectGitForHumanInput {
                human_user_id,
                company_id,
                project_id,
                remote_url: existing.remote_url,
                host_local_path: Some(existing.host_local_path),
                default_branch: Some(existing.default_branch),
                auth_profile: existing.auth_profile,
                allow_agent_push: Some(existing.allow_agent_push),
                branch_prefix: Some(existing.branch_prefix),
            },
        )?;
    } else {
        state.platform.delete_company_project_git_for_human(
            DeleteCompanyProjectGitForHumanInput {
                human_user_id,
                company_id,
                project_id,
            },
        )?;
    }
    Ok(())
}

async fn list_company_project_tasks_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let tasks = state
        .platform
        .list_company_project_tasks_for_human(human.id, company_id)?;
    Ok(Json(serde_json::json!({ "tasks": tasks })))
}

async fn create_company_project_task_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<CreateCompanyProjectTaskRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let task = state.platform.create_company_project_task_for_human(
        CreateCompanyProjectTaskForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            title: input.title,
            description: input.description,
            priority: input.priority,
            assignee_agent_id: input.assignee_agent_id,
            due_at: input.due_at,
            depends_on_task_ids: input.depends_on_task_ids,
        },
    )?;
    Ok(Json(serde_json::json!({ "task": task })))
}

async fn update_company_project_task_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id, task_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(input): Json<UpdateCompanyProjectTaskRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let task = state.platform.update_company_project_task_for_human(
        UpdateCompanyProjectTaskForHumanInput {
            human_user_id: human.id,
            company_id,
            project_id,
            task_id,
            title: input.title,
            description: input.description,
            status: input.status,
            priority: input.priority,
            assignee_agent_id: input.assignee_agent_id,
            clear_assignee: input.clear_assignee,
            due_at: input.due_at,
            clear_due_at: input.clear_due_at,
            depends_on_task_ids: input.depends_on_task_ids,
        },
    )?;
    Ok(Json(serde_json::json!({ "task": task })))
}

async fn activate_company_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<CompanyAgentStaffingStatusRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let result =
        state
            .platform
            .activate_provisioned_company_agent(HumanCompanyStaffingStatusInput {
                human_user_id: human.id,
                company_id,
                target_agent_id: agent_id,
                reason: input.reason,
                handoff_plan: input.handoff_plan,
                handoff_agent_id: input.handoff_agent_id,
            })?;
    Ok(Json(serde_json::json!({ "result": result })))
}

async fn suspend_company_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<CompanyAgentStaffingStatusRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let result =
        state
            .platform
            .suspend_company_agent_as_human(HumanCompanyStaffingStatusInput {
                human_user_id: human.id,
                company_id,
                target_agent_id: agent_id,
                reason: input.reason,
                handoff_plan: input.handoff_plan,
                handoff_agent_id: input.handoff_agent_id,
            })?;
    Ok(Json(serde_json::json!({ "result": result })))
}

async fn reactivate_company_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<CompanyAgentStaffingStatusRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let result = state
        .platform
        .reactivate_company_agent(HumanCompanyStaffingStatusInput {
            human_user_id: human.id,
            company_id,
            target_agent_id: agent_id,
            reason: input.reason,
            handoff_plan: input.handoff_plan,
            handoff_agent_id: input.handoff_agent_id,
        })?;
    Ok(Json(serde_json::json!({ "result": result })))
}

async fn terminate_company_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, agent_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<CompanyAgentStaffingStatusRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let result =
        state
            .platform
            .terminate_company_agent_as_human(HumanCompanyStaffingStatusInput {
                human_user_id: human.id,
                company_id,
                target_agent_id: agent_id,
                reason: input.reason,
                handoff_plan: input.handoff_plan,
                handoff_agent_id: input.handoff_agent_id,
            })?;
    Ok(Json(serde_json::json!({ "result": result })))
}

async fn list_company_staffing_actions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let actions = state
        .platform
        .list_company_staffing_actions_for_human(human.id, company_id)?;
    Ok(Json(serde_json::json!({ "staffing_actions": actions })))
}

async fn pause_company_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let project =
        state
            .platform
            .pause_company_project_for_human(SetCompanyProjectPauseForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
            })?;
    Ok(Json(serde_json::json!({ "project": project })))
}

async fn resume_company_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company_id, project_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let project =
        state
            .platform
            .resume_company_project_for_human(SetCompanyProjectPauseForHumanInput {
                human_user_id: human.id,
                company_id,
                project_id,
            })?;
    Ok(Json(serde_json::json!({ "project": project })))
}

async fn list_owner_agents(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(human_user_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    require_same_human(human.id, human_user_id)?;
    let agents = state.platform.list_owner_agents(human_user_id)?;
    Ok(Json(serde_json::json!({ "agents": agents })))
}

async fn list_agent_conversations(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let conversations = state
        .platform
        .list_owned_agent_conversations(human.id, agent_id)?;
    Ok(Json(serde_json::json!({ "conversations": conversations })))
}

async fn get_conversation_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(conversation_id): Path<Uuid>,
    Query(query): Query<ConversationMessagesQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    let page = state.platform.get_owned_conversation_message_page(
        human.id,
        conversation_id,
        query.before_message_id,
        query.limit.unwrap_or(50),
    )?;
    Ok(Json(serde_json::json!({
        "messages": page.messages,
        "next_cursor": page.next_cursor,
        "has_more": page.has_more,
    })))
}

async fn update_owned_agent_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((human_user_id, agent_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<AgentStatusUpdateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    require_same_human(human.id, human_user_id)?;
    let agent_profile = match input.status.trim() {
        "active" => state
            .platform
            .unfreeze_owned_agent(human_user_id, agent_id)?,
        "frozen" => state.platform.freeze_owned_agent(human_user_id, agent_id)?,
        _ => {
            return Err(ApiError(AppError::Validation(
                "status must be active or frozen".into(),
            )))
        }
    };

    Ok(Json(serde_json::json!({ "agent_profile": agent_profile })))
}

async fn rotate_owned_agent_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((human_user_id, agent_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
    require_same_human(human.id, human_user_id)?;
    let result = state
        .platform
        .rotate_owned_agent_key(human_user_id, agent_id)?;
    Ok(Json(serde_json::json!({ "result": result })))
}

async fn update_admin_agent_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
    Json(input): Json<AgentStatusUpdateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state, &headers, ADMIN_SCOPE_AGENTS)?;
    let agent_profile = match input.status.trim() {
        "active" => state
            .platform
            .admin_unfreeze_agent_with_note(agent_id, input.note)?,
        "frozen" => state
            .platform
            .admin_freeze_agent_with_note(agent_id, input.note)?,
        _ => {
            return Err(ApiError(AppError::Validation(
                "status must be active or frozen".into(),
            )))
        }
    };

    Ok(Json(serde_json::json!({ "agent_profile": agent_profile })))
}

async fn rotate_admin_agent_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<Uuid>,
    Json(input): Json<AdminRotateKeyRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state, &headers, ADMIN_SCOPE_AGENTS)?;
    let result = state
        .platform
        .admin_rotate_agent_key_with_note(agent_id, input.note)?;
    Ok(Json(serde_json::json!({ "result": result })))
}

async fn dev_bootstrap_agent(
    State(state): State<AppState>,
    Json(input): Json<DevBootstrapRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_dev_endpoints(&state)?;
    let DevBootstrapRequest {
        email,
        display_name,
        desired_handle,
        desired_agent_name,
        persona,
    } = input;
    let user = state.platform.dev_login(DevLoginInput {
        email,
        display_name: display_name.clone(),
    })?;
    let human_auth = state.platform.issue_human_session(user.id)?;
    if let Some(existing_agent) = state
        .platform
        .find_owner_agent_by_handle(user.id, &desired_handle)?
    {
        let existing_key = state
            .platform
            .rotate_owned_agent_key(user.id, existing_agent.id)?;
        return Ok(Json(serde_json::json!({
            "note": "Development bootstrap reused an existing agent and issued a fresh key.",
            "human_user": user,
            "human_session_token": human_auth.session_token,
            "agent_profile": existing_agent,
            "agent_key": existing_key.agent_key_plaintext,
            "verification_mode": "bootstrap_reused",
            "verification_evidence": "existing_agent_reused"
        })));
    }
    let company = match state
        .platform
        .list_human_companies(user.id)?
        .into_iter()
        .next()
    {
        Some(company) => company,
        None => {
            state
                .platform
                .create_company(CreateCompanyInput {
                    human_user_id: user.id,
                    name: format!("{display_name} 的公司"),
                    slug: Some(format!("dev-company-{}", user.id.simple())),
                    description: Some("Development bootstrap company".into()),
                })?
                .company
        }
    };
    let result = state
        .platform
        .create_company_agent(CreateCompanyAgentInput {
            human_user_id: user.id,
            company_id: company.id,
            display_name: desired_agent_name,
            handle: desired_handle,
            persona,
            org_unit_id: None,
            job_title: None,
            role_key: None,
            reports_to_membership_id: None,
        })?;
    Ok(Json(serde_json::json!({
        "note": "Development bootstrap completed for the current shared runtime.",
        "human_user": user,
        "human_session_token": human_auth.session_token,
        "agent_profile": result.agent_profile,
        "agent_key": result.agent_key_plaintext,
        "company": result.company,
        "company_membership": result.membership,
        "verification_mode": "company_direct",
        "verification_evidence": "development bootstrap created the agent directly inside a company"
    })))
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("missing bearer token".into()))?;
    value
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| ApiError(AppError::Unauthorized("invalid bearer token".into())))
}

fn authenticate_human_session_request(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<HumanUser, ApiError> {
    let token = bearer_token(headers)?;
    state
        .owner_api_limiter
        .check(format!("owner:{}", hash_secret(token)))?;
    state
        .platform
        .authenticate_human_session(token)
        .map_err(ApiError::from)
}

fn authenticate_human_request(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<HumanUser, ApiError> {
    let human = authenticate_human_session_request(state, headers)?;
    if state.require_email_verification
        && !state
            .platform
            .is_human_email_verified(human.id)
            .map_err(ApiError::from)?
    {
        return Err(ApiError(AppError::Unauthorized(
            "email verification is required".into(),
        )));
    }
    Ok(human)
}

fn require_same_human(authenticated_id: Uuid, requested_id: Uuid) -> Result<(), ApiError> {
    if authenticated_id != requested_id {
        return Err(ApiError(AppError::Unauthorized(
            "cannot access another human user's resources".into(),
        )));
    }
    Ok(())
}

fn build_admin_credentials(
    legacy_root_token: Option<&str>,
    credentials_json: Option<&str>,
) -> anyhow::Result<Vec<AdminCredential>> {
    let mut credentials = Vec::new();
    if let Some(token) = legacy_root_token.filter(|value| !value.trim().is_empty()) {
        credentials.push(AdminCredential {
            name: "legacy-root".into(),
            token_hash: hash_secret(token),
            scopes: HashSet::from(["*".to_string()]),
        });
    }

    if let Some(raw_json) = credentials_json.filter(|value| !value.trim().is_empty()) {
        let configured: Vec<AdminCredentialEnv> = serde_json::from_str(raw_json)
            .map_err(|error| anyhow::anyhow!("invalid ADMIN_API_TOKENS_JSON: {error}"))?;
        for item in configured {
            let name = item.name.trim();
            let token = item.token.trim();
            if name.is_empty() || token.len() < 24 {
                anyhow::bail!(
                    "each ADMIN_API_TOKENS_JSON entry needs a name and a token of at least 24 characters"
                );
            }
            let scopes = item
                .scopes
                .into_iter()
                .map(|scope| scope.trim().to_ascii_lowercase())
                .filter(|scope| !scope.is_empty())
                .collect::<HashSet<_>>();
            if scopes.is_empty()
                || scopes.iter().any(|scope| {
                    !matches!(scope.as_str(), "*" | ADMIN_SCOPE_READ | ADMIN_SCOPE_AGENTS)
                })
            {
                anyhow::bail!(
                    "admin credential {name} has no scopes or contains an unsupported scope"
                );
            }
            credentials.push(AdminCredential {
                name: name.to_string(),
                token_hash: hash_secret(token),
                scopes,
            });
        }
    }

    let mut token_hashes = HashSet::new();
    if credentials
        .iter()
        .any(|credential| !token_hashes.insert(credential.token_hash.clone()))
    {
        anyhow::bail!("admin tokens must be unique");
    }
    Ok(credentials)
}

fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
    required_scope: &str,
) -> Result<(), ApiError> {
    state.admin_api_limiter.check("admin")?;
    if state.admin_credentials.is_empty() {
        return Err(ApiError(AppError::Unauthorized(
            "admin API token is not configured".into(),
        )));
    }
    let actual_hash = hash_secret(bearer_token(headers)?);
    let credential = state
        .admin_credentials
        .iter()
        .find(|credential| credential.token_hash == actual_hash)
        .ok_or_else(|| ApiError(AppError::Unauthorized("invalid admin API token".into())))?;
    if !credential.allows(required_scope) {
        tracing::warn!(
            admin_credential = %credential.name,
            %required_scope,
            "admin credential denied by scope"
        );
        return Err(ApiError(AppError::Unauthorized(format!(
            "admin token lacks required scope {required_scope}"
        ))));
    }
    Ok(())
}

fn require_dev_endpoints(state: &AppState) -> Result<(), ApiError> {
    if !state.enable_dev_endpoints {
        return Err(ApiError(AppError::NotFound(
            "development endpoints are disabled".into(),
        )));
    }
    Ok(())
}

#[derive(Debug)]
struct ApiError(AppError);

impl From<AppError> for ApiError {
    fn from(value: AppError) -> Self {
        Self(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self.0 {
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
            AppError::Internal(message) => {
                tracing::error!(error = %message, "internal API error");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };

        let body = Json(ApiErrorResponse {
            code: self.0.code().to_string(),
            message: if matches!(self.0, AppError::Internal(_)) {
                "internal server error".into()
            } else {
                self.0.to_string()
            },
        });

        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_token_requires_authorization_bearer_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Bearer hus_test".parse().expect("valid header"),
        );
        assert_eq!(bearer_token(&headers).expect("bearer token"), "hus_test");

        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Basic abc".parse().expect("valid header"),
        );
        assert!(bearer_token(&headers).is_err());
    }

    #[test]
    fn owner_path_cannot_select_another_human_user() {
        assert!(require_same_human(Uuid::new_v4(), Uuid::new_v4()).is_err());
        let user_id = Uuid::new_v4();
        assert!(require_same_human(user_id, user_id).is_ok());
    }

    #[test]
    fn admin_credentials_support_legacy_root_and_scoped_tokens() {
        let scoped_token = "ops-token-with-at-least-24-chars";
        let configured = serde_json::json!([{
            "name": "observer",
            "token": scoped_token,
            "scopes": [ADMIN_SCOPE_READ]
        }])
        .to_string();
        let credentials = build_admin_credentials(Some("legacy-root-token"), Some(&configured))
            .expect("admin credentials should parse");

        let legacy = credentials
            .iter()
            .find(|credential| credential.name == "legacy-root")
            .expect("legacy root should exist");
        assert!(legacy.allows(ADMIN_SCOPE_AGENTS));

        let observer = credentials
            .iter()
            .find(|credential| credential.name == "observer")
            .expect("observer should exist");
        assert!(observer.allows(ADMIN_SCOPE_READ));
        assert!(!observer.allows(ADMIN_SCOPE_AGENTS));
        assert_eq!(observer.token_hash, hash_secret(scoped_token));
    }

    #[test]
    fn admin_credentials_reject_weak_or_unknown_scope_entries() {
        let weak = r#"[{"name":"ops","token":"short","scopes":["admin:read"]}]"#;
        assert!(build_admin_credentials(None, Some(weak)).is_err());

        let unknown = r#"[{"name":"ops","token":"a-strong-token-with-24-characters","scopes":["admin:unknown"]}]"#;
        assert!(build_admin_credentials(None, Some(unknown)).is_err());
    }

    #[test]
    fn sliding_window_rate_limiter_rejects_requests_over_budget() {
        let limiter = SlidingWindowRateLimiter::new(2, StdDuration::from_secs(60), "test");
        assert!(limiter.check("same-client").is_ok());
        assert!(limiter.check("same-client").is_ok());
        assert!(matches!(
            limiter.check("same-client"),
            Err(ApiError(AppError::RateLimited(_)))
        ));
        assert!(limiter.check("different-client").is_ok());
    }

    #[test]
    fn local_project_import_copies_source_into_a_fresh_git_repository() {
        let root = std::env::temp_dir().join(format!("relay-project-import-{}", Uuid::new_v4()));
        let source = root.join("source");
        let destination = root.join("managed");
        fs::create_dir_all(source.join("src")).expect("source directories");
        fs::create_dir_all(source.join(".git")).expect("source git metadata");
        fs::create_dir_all(source.join("node_modules/pkg")).expect("source dependency cache");
        fs::write(
            source.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .expect("manifest");
        fs::write(source.join("src/main.rs"), "fn main() {}\n").expect("source file");
        fs::write(source.join(".git/config"), "source metadata").expect("git metadata");
        fs::write(source.join("node_modules/pkg/index.js"), "cache").expect("cache file");

        import_project_folder(&source, &destination).expect("folder import should succeed");
        assert!(destination.join("Cargo.toml").is_file());
        assert!(destination.join("src/main.rs").is_file());
        assert!(destination.join(".git").is_dir());
        assert!(!destination.join("node_modules").exists());
        let status = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&destination)
            .output()
            .expect("git status");
        assert!(status.status.success());
        assert!(String::from_utf8_lossy(&status.stdout).trim().is_empty());
        fs::remove_dir_all(root).expect("test import should be removable");
    }

    #[test]
    fn uploaded_project_paths_are_normalized_and_exclude_generated_directories() {
        assert_eq!(
            normalize_uploaded_project_path("src\\main.rs").expect("valid path"),
            Some("src/main.rs".into())
        );
        assert_eq!(
            normalize_uploaded_project_path("node_modules/pkg/index.js").expect("excluded path"),
            None
        );
        assert_eq!(
            normalize_uploaded_project_path("client/target/debug/app").expect("excluded path"),
            None
        );
        assert!(normalize_uploaded_project_path("../secret.txt").is_err());
        assert!(normalize_uploaded_project_path("/absolute/path").is_err());
    }
}
