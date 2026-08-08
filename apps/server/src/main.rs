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
    routing::{any, get, post},
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
    ChangeHumanPasswordInput, CreateCompanyAgentInput, CreateCompanyInput,
    CreateCompanyProjectForHumanInput, CreateCompanyProjectTaskForHumanInput, CreateOrgUnitInput,
    DeleteAgentMemoryForHumanInput, DeleteCompanyCodexRunnerProfileForHumanInput, DevLoginInput,
    GetCompanyAgentCodexTriggerForHumanInput, GetCompanyProjectGitForHumanInput,
    HumanCompanyStaffingStatusInput, ListCompanyAgentCodexRunsForHumanInput,
    ListCompanyAgentCodexSessionsForHumanInput, ListCompanyCodexPluginsForHumanInput,
    ListCompanyCodexRunnerProfilesForHumanInput, ListCompanyMemoriesForHumanInput, LoginHumanInput,
    OpenHumanCompanyDirectConversationInput, PlatformApp, PublishCompanyGovernancePolicyInput,
    RegisterHumanInput, RequestCodexPluginOperationForHumanInput,
    RequestCompanyProjectRuleGenerationForHumanInput, ResetHumanPasswordInput,
    ReviewAgentToolApprovalInput, SendHumanCompanyMessageWithAttachmentsInput,
    SetCompanyAgentCodexTriggerStatusForHumanInput, SetCompanyProjectPauseForHumanInput,
    TransferCompanyProjectOwnerForHumanInput, UpdateAgentMemoryForHumanInput,
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
use ai_chat_infrastructure::git_credentials::{managed_token_profile_name, GitCredentialStore};
use ai_chat_infrastructure::harness::{
    HarnessProjectGitProvisioner, HarnessProvisioner, HarnessRepositoryContent,
};
use ai_chat_infrastructure::ownership_proof::OwnershipProofVerifierAdapter;
use ai_chat_infrastructure::project_git::ProvisionedProjectGit;
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

mod account;
mod agents;
mod auth;
mod chat;
mod codex;
mod company;
mod dto;
mod project_files;
mod project_repository;
mod projects;

use account::*;
use agents::*;
use auth::*;
use chat::*;
use codex::*;
use company::*;
use dto::*;
use project_files::*;
use project_repository::*;
use projects::*;

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
    let project_git_provisioner = if harness_provisioner.is_enabled() {
        Some(Arc::new(HarnessProjectGitProvisioner::new(
            harness_provisioner.clone(),
            git_credential_store.clone(),
        ))
            as Arc<
                dyn ai_chat_infrastructure::project_git::ProjectGitProvisioner,
            >)
    } else {
        None
    };
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
        .with_project_git_provisioner(project_git_provisioner.clone());
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
            "/api/v1/companies/{company_id}/skill-catalog",
            get(get_company_skill_catalog),
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
            "/api/v1/companies/{company_id}/agents/{agent_id}/codex-sessions",
            get(list_company_agent_codex_sessions),
        )
        .route(
            "/api/v1/companies/{company_id}/governance-policy",
            get(get_company_governance_policy).post(publish_company_governance_policy),
        )
        .route(
            "/api/v1/companies/{company_id}/projects/{project_id}/git",
            get(get_company_project_git),
        )
        .route(
            "/api/v1/companies/{company_id}/projects/{project_id}/repository/refs",
            get(list_company_project_repository_refs),
        )
        .route(
            "/api/v1/companies/{company_id}/projects/{project_id}/repository/tree",
            get(list_company_project_repository_tree),
        )
        .route(
            "/api/v1/companies/{company_id}/projects/{project_id}/repository/file",
            get(get_company_project_repository_file),
        )
        .route(
            "/api/v1/companies/{company_id}/projects/{project_id}/pause",
            post(pause_company_project),
        )
        .route(
            "/api/v1/companies/{company_id}/projects/{project_id}/owner",
            axum::routing::put(transfer_company_project_owner),
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
        .route("/api/{*path}", any(api_not_found))
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

async fn api_not_found() -> ApiError {
    ApiError(AppError::NotFound("API route not found".into()))
}

#[cfg(test)]
mod tests;
