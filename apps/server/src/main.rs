use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    convert::Infallible,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration as StdDuration, Instant},
};

use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
        HeaderMap, HeaderName, HeaderValue, Method, StatusCode,
    },
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{any, get, post},
    Json, Router,
};
use futures_util::Stream;
use serde::{Deserialize, Serialize};
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
    ChangeHumanPasswordInput, CreateCompanyAgentInput, CreateCompanyInput,
    CreateCompanyProjectTaskForHumanInput, CreateOrgUnitInput,
    DeleteCompanyCodexRunnerProfileForHumanInput, DeleteCompanyProjectGitForHumanInput,
    DevLoginInput, GetCompanyAgentCodexTriggerForHumanInput, GetCompanyProjectGitForHumanInput,
    HumanCompanyStaffingStatusInput, ListCompanyAgentCodexRunsForHumanInput,
    ListCompanyCodexRunnerProfilesForHumanInput, LoginHumanInput,
    OpenHumanCompanyDirectConversationInput, PlatformApp, PublishCompanyGovernancePolicyInput,
    RegisterHumanInput, RequestCompanyProjectRuleGenerationForHumanInput, ResetHumanPasswordInput,
    ReviewAgentToolApprovalInput, SendHumanCompanyMessageWithMentionsInput,
    SetCompanyAgentCodexTriggerStatusForHumanInput, UpdateCompanyAgentPermissionsInput,
    UpdateCompanyAgentProfessionInput, UpdateCompanyAgentRoleInput,
    UpdateCompanyProjectRuleForHumanInput, UpdateCompanyProjectTaskForHumanInput,
    UpsertCompanyAgentCodexTriggerForHumanInput, UpsertCompanyCodexRunnerProfileForHumanInput,
    UpsertCompanyProjectAssetRefreshForHumanInput, UpsertCompanyProjectGitForHumanInput,
};
use ai_chat_domain::agent_identity::HumanUser;
use ai_chat_domain::company::{
    company_profession_by_key, CompanyGovernancePolicySettings, CompanyRealtimeEvent,
    CompanyRealtimeSignal,
};
use ai_chat_infrastructure::config::{ApiConfig, McpConfig, RepositoryMode};
use ai_chat_infrastructure::git_credentials::{
    github_token_profile_name, validate_github_token, GitCredentialStore,
};
use ai_chat_infrastructure::ownership_proof::OwnershipProofVerifierAdapter;
use ai_chat_infrastructure::realtime::spawn_postgres_realtime_listener;
use ai_chat_infrastructure::{build_ownership_proof_verifier, RepositoryAdapter};
use ai_chat_shared::{hash_secret, AppError};

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
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpsertCompanyProjectGitRequest {
    remote_url: String,
    host_local_path: String,
    default_branch: Option<String>,
    auth_profile: Option<String>,
    github_token: Option<String>,
    #[serde(default)]
    clear_github_token: bool,
    allow_agent_push: Option<bool>,
    branch_prefix: Option<String>,
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
    sandbox_mode: Option<String>,
    approval_policy: Option<String>,
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
    sandbox_mode: String,
    approval_policy: String,
    max_run_seconds: i32,
    is_default: bool,
}

#[derive(Debug, Deserialize)]
struct LocalCodexModelsQuery {
    codex_profile: Option<String>,
    bundled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct LocalCodexReasoningEffort {
    effort: String,
    description: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct LocalCodexModelInfo {
    display_name: String,
    default_reasoning_effort: Option<String>,
    reasoning_efforts: Vec<LocalCodexReasoningEffort>,
}

#[derive(Debug, Deserialize)]
struct CodexRunsQuery {
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ApprovalRequestsQuery {
    status: Option<String>,
    limit: Option<usize>,
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
        if matches!(
            config.repository_mode,
            ai_chat_infrastructure::config::RepositoryMode::Memory
        ) {
            anyhow::bail!("REPOSITORY_MODE=postgres is required in production");
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
    let repository = RepositoryAdapter::build(&config)?;
    let verifier = build_ownership_proof_verifier(&config);
    let mcp_config = McpConfig::from_env();
    let platform = PlatformApp::with_verifier(repository, verifier);
    let git_credential_store = GitCredentialStore::from_env()?;
    let (realtime_sender, _) = broadcast::channel(2_048);
    let _realtime_listener =
        matches!(&config.repository_mode, RepositoryMode::Postgres).then(|| {
            spawn_postgres_realtime_listener(config.database_url.clone(), realtime_sender.clone())
        });
    let mcp_gateway = McpGateway::new(platform.clone(), mcp_config.agent_key.clone());
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
            "/api/v1/companies/{company_id}/conversations/direct",
            post(open_human_company_direct_conversation),
        )
        .route(
            "/api/v1/companies/{company_id}/conversations/{conversation_id}/messages",
            post(send_human_company_message),
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
        .route("/api/v1/agent-context", get(removed_legacy_feature))
        .route(
            "/api/v1/conversations/{conversation_id}/messages",
            get(get_conversation_messages),
        )
        .route(
            "/api/v1/friend-profiles/{owner_agent_id}/{friend_agent_id}",
            get(removed_legacy_feature),
        )
        .route(
            "/api/v1/humans/{human_user_id}/console",
            get(removed_legacy_feature),
        )
        .route("/api/v1/admin/console", get(removed_legacy_feature))
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
        .route("/mcp/{*rest}", any(removed_legacy_feature))
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
    })
}

async fn get_local_codex_models(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LocalCodexModelsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authenticate_human_request(&state, &headers)?;
    let executable = std::env::var("AGENT_TRIGGER_CODEX_BIN").unwrap_or_else(|_| "codex".into());
    let prefix_args = std::env::var("AGENT_TRIGGER_CODEX_PREFIX_ARGS_JSON")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            serde_json::from_str::<Vec<String>>(&value).map_err(|error| {
                AppError::Validation(format!(
                    "invalid AGENT_TRIGGER_CODEX_PREFIX_ARGS_JSON: {error}"
                ))
            })
        })
        .transpose()?
        .unwrap_or_default();
    let mut command = tokio::process::Command::new(&executable);
    command.args(prefix_args);
    if let Some(profile) = query
        .codex_profile
        .as_deref()
        .map(str::trim)
        .filter(|profile| !profile.is_empty() && *profile != "default")
    {
        if profile.len() > 64 || profile.chars().any(char::is_control) {
            return Err(AppError::Validation("invalid local Codex profile".into()).into());
        }
        command.arg("--profile").arg(profile);
    }
    command.arg("debug").arg("models");
    if query.bundled.unwrap_or(false) {
        command.arg("--bundled");
    }
    command.kill_on_drop(true);
    let output = tokio::time::timeout(StdDuration::from_secs(15), command.output())
        .await
        .map_err(|_| AppError::Validation("local Codex model discovery timed out".into()))?
        .map_err(|error| {
            AppError::Validation(format!(
                "failed to start local Codex model discovery: {error}"
            ))
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = if stderr.contains("ENOENT") {
            "Codex executable or platform binary was not found"
        } else if stderr.to_lowercase().contains("login")
            || stderr.to_lowercase().contains("authentication")
        {
            "local Codex is not authenticated"
        } else {
            "codex debug models exited unsuccessfully"
        };
        return Err(AppError::Validation(format!(
            "local Codex model discovery failed: {reason}; run `codex --version` on the host to diagnose it"
        ))
        .into());
    }
    let catalog: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|error| {
        AppError::Validation(format!(
            "local Codex returned an invalid model catalog: {error}"
        ))
    })?;
    let mut models = BTreeMap::new();
    collect_local_codex_models(&catalog, &mut models);
    if models.is_empty() {
        return Err(AppError::Validation(
            "local Codex model catalog did not contain any selectable models".into(),
        )
        .into());
    }
    Ok(Json(serde_json::json!({
        "models": models
            .into_iter()
            .map(|(id, info)| serde_json::json!({
                "id": id,
                "display_name": info.display_name,
                "default_reasoning_effort": info.default_reasoning_effort,
                "reasoning_efforts": info.reasoning_efforts
            }))
            .collect::<Vec<_>>(),
        "source": if query.bundled.unwrap_or(false) { "local_codex_bundled" } else { "local_codex" }
    })))
}

fn collect_local_codex_models(
    value: &serde_json::Value,
    models: &mut BTreeMap<String, LocalCodexModelInfo>,
) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                collect_local_codex_models(item, models);
            }
        }
        serde_json::Value::Object(object) => {
            let is_selectable = object
                .get("visibility")
                .and_then(serde_json::Value::as_str)
                .map(|visibility| visibility.eq_ignore_ascii_case("list"))
                .unwrap_or(true);
            let id = object
                .get("slug")
                .or_else(|| object.get("model"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|id| !id.is_empty() && id.len() <= 128);
            if let Some(id) = id.filter(|_| is_selectable) {
                let display_name = object
                    .get("display_name")
                    .or_else(|| object.get("name"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .unwrap_or(id);
                let default_reasoning_effort = object
                    .get("default_reasoning_level")
                    .or_else(|| object.get("defaultReasoningEffort"))
                    .and_then(serde_json::Value::as_str)
                    .filter(|effort| is_codex_reasoning_effort(effort))
                    .map(str::to_string);
                let reasoning_values = object
                    .get("supported_reasoning_levels")
                    .or_else(|| object.get("supportedReasoningEfforts"))
                    .and_then(serde_json::Value::as_array);
                let mut reasoning_efforts = Vec::new();
                if let Some(reasoning_values) = reasoning_values {
                    for reasoning in reasoning_values {
                        let effort = reasoning
                            .get("effort")
                            .or_else(|| reasoning.get("reasoningEffort"))
                            .and_then(serde_json::Value::as_str)
                            .filter(|effort| is_codex_reasoning_effort(effort));
                        let Some(effort) = effort else { continue };
                        if reasoning_efforts
                            .iter()
                            .any(|item: &LocalCodexReasoningEffort| item.effort == effort)
                        {
                            continue;
                        }
                        let description = reasoning
                            .get("description")
                            .and_then(serde_json::Value::as_str)
                            .map(str::trim)
                            .filter(|description| !description.is_empty())
                            .unwrap_or(effort);
                        reasoning_efforts.push(LocalCodexReasoningEffort {
                            effort: effort.to_string(),
                            description: description.to_string(),
                        });
                    }
                }
                models.insert(
                    id.to_string(),
                    LocalCodexModelInfo {
                        display_name: display_name.to_string(),
                        default_reasoning_effort,
                        reasoning_efforts,
                    },
                );
            }
            for nested in object.values() {
                if nested.is_array() || nested.is_object() {
                    collect_local_codex_models(nested, models);
                }
            }
        }
        serde_json::Value::String(id) if !id.trim().is_empty() && id.len() <= 128 => {
            if (id.contains("gpt-") || id.contains("codex"))
                && id.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
                })
            {
                models.insert(
                    id.clone(),
                    LocalCodexModelInfo {
                        display_name: id.clone(),
                        default_reasoning_effort: None,
                        reasoning_efforts: Vec::new(),
                    },
                );
            }
        }
        _ => {}
    }
}

fn is_codex_reasoning_effort(value: &str) -> bool {
    matches!(
        value,
        "minimal" | "low" | "medium" | "high" | "xhigh" | "max" | "ultra"
    )
}

async fn register_human(
    State(state): State<AppState>,
    Json(input): Json<RegisterHumanInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .login_limiter
        .check(format!("register:{}", input.email.trim().to_lowercase()))?;
    let auth = state.platform.register_human(input)?;
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
        "email_verification_sent": email_verification_sent
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
    let email_verified = state.platform.is_human_email_verified(auth.user.id)?;
    Ok(Json(serde_json::json!({
        "user": auth.user,
        "session_token": auth.session_token,
        "expires_at": auth.expires_at,
        "email_verified": email_verified
    })))
}

async fn get_authenticated_human(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = authenticate_human_session_request(&state, &headers)?;
    let email_verified = state.platform.is_human_email_verified(user.id)?;
    Ok(Json(serde_json::json!({
        "user": user,
        "email_verified": email_verified
    })))
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
    let message = state.platform.send_human_company_message_with_mentions(
        SendHumanCompanyMessageWithMentionsInput {
            human_user_id: human.id,
            company_id,
            conversation_id,
            content: input.content,
            mentioned_agent_ids: input.mentioned_agent_ids,
            mention_all: input.mention_all,
        },
    )?;
    Ok(Json(serde_json::json!({ "message": message })))
}

async fn stream_company_events_for_human(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Query(query): Query<RealtimeEventsQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
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
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (sender, receiver) = mpsc::channel::<CompanyRealtimeEvent>(256);
    let mut signal_receiver = state.realtime_sender.subscribe();
    tokio::spawn(async move {
        let mut cursor = after_sequence_id;
        loop {
            let events = state
                .platform
                .read_company_realtime_events(company_id, cursor, batch_limit)
                .unwrap_or_default();
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

async fn create_company_codex_runner_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<Uuid>,
    Json(input): Json<UpsertCompanyCodexRunnerProfileRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let human = authenticate_human_request(&state, &headers)?;
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
                sandbox_mode: input.sandbox_mode,
                approval_policy: input.approval_policy,
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
                sandbox_mode: input.sandbox_mode,
                approval_policy: input.approval_policy,
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
                sandbox_mode: input.sandbox_mode,
                approval_policy: input.approval_policy,
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
                },
                notes: input.notes,
            })?;
    Ok(Json(serde_json::json!({ "governance_policy": policy })))
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
    Ok(Json(serde_json::json!({
        "git": git,
        "github_token_configured": github_token_configured,
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
    Ok(Json(serde_json::json!({
        "git": git,
        "github_token_configured": github_token_configured,
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
    state.git_credential_store.remove_github_token(project_id)?;
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
                host_local_path: existing.host_local_path,
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

async fn removed_legacy_feature() -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError(AppError::NotFound(
        "this legacy feature has been removed from Relay".into(),
    )))
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
        let status = match self.0 {
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
        };

        let body = Json(ApiErrorResponse {
            code: self.0.code().to_string(),
            message: self.0.to_string(),
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
    fn local_codex_model_catalog_excludes_hidden_models_and_nested_ids() {
        let catalog = serde_json::json!({
            "models": [
                {
                    "slug": "gpt-5.6-sol",
                    "display_name": "GPT-5.6-Sol",
                    "visibility": "list",
                    "default_reasoning_level": "low",
                    "supported_reasoning_levels": [
                        { "effort": "low", "description": "Fast" },
                        { "effort": "high", "description": "Deep" }
                    ],
                    "service_tiers": [{ "id": "priority", "name": "Fast" }]
                },
                {
                    "slug": "codex-auto-review",
                    "display_name": "Codex Auto Review",
                    "visibility": "hide"
                }
            ]
        });
        let mut models = BTreeMap::new();

        collect_local_codex_models(&catalog, &mut models);

        assert_eq!(
            models,
            BTreeMap::from([(
                "gpt-5.6-sol".to_string(),
                LocalCodexModelInfo {
                    display_name: "GPT-5.6-Sol".to_string(),
                    default_reasoning_effort: Some("low".to_string()),
                    reasoning_efforts: vec![
                        LocalCodexReasoningEffort {
                            effort: "low".to_string(),
                            description: "Fast".to_string(),
                        },
                        LocalCodexReasoningEffort {
                            effort: "high".to_string(),
                            description: "Deep".to_string(),
                        },
                    ],
                }
            )])
        );
    }
}
