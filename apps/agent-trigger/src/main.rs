use std::{
    collections::HashSet,
    fs,
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration as StdDuration,
};

use async_trait::async_trait;
use chrono::Duration;
use futures_util::{stream::FuturesUnordered, StreamExt};
use tokio::{io::AsyncWriteExt, process::Command};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

use ai_chat_application::{
    CompleteAgentCodexTriggerLeaseInput, CreateCodexApprovalRequestInput, GetCompanyProjectInput,
    PlatformApp,
};
use ai_chat_domain::{
    agent_identity::AgentProfile,
    company::{
        company_profession_by_key, company_project_type_by_key, infer_company_profession,
        AgentCodexRunActivity, AgentCodexSession, AgentCodexTriggerConfig, AgentCodexTriggerRun,
        AgentExecutionIntent, AgentMemory, CodexPluginCatalogSnapshot, CodexPluginOperation,
        CompanyProject, CompanyProjectRule, AGENT_CODEX_APPROVAL_LOCAL_TARGET_KEY,
        AGENT_CODEX_APPROVAL_POLICY_NEVER, AGENT_CODEX_APPROVAL_SCOPE_KEY,
        AGENT_CODEX_APPROVAL_TARGET_KEY, AGENT_CODEX_APPROVAL_TOOL_PERMISSIONS,
        AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS, AGENT_CODEX_RUN_STATUS_CANCELLED,
        AGENT_CODEX_RUN_STATUS_FAILED, AGENT_CODEX_RUN_STATUS_RUNNING,
        AGENT_CODEX_RUN_STATUS_SUCCEEDED, AGENT_CODEX_RUN_STATUS_TIMED_OUT,
        AGENT_CODEX_SANDBOX_READ_ONLY, AGENT_CODEX_SESSION_KIND_CONTROL,
        AGENT_CODEX_SESSION_KIND_PROJECT, AGENT_CODEX_SESSION_STATUS_ACTIVE,
        AGENT_CODEX_SESSION_STATUS_ARCHIVED, AGENT_CODEX_SETTING_INHERIT,
        AGENT_EXECUTION_INTENT_ACTION_REPLACE_SESSION, AGENT_EXECUTION_INTENT_STATUS_COMPLETED,
        AGENT_EXECUTION_INTENT_STATUS_FAILED, AGENT_EXECUTION_INTENT_STATUS_PENDING,
        AGENT_EXECUTION_INTENT_STATUS_RUNNING, AGENT_TOOL_APPROVAL_STATUS_APPROVED,
        AGENT_TOOL_APPROVAL_STATUS_EXECUTED, AGENT_TOOL_APPROVAL_STATUS_EXPIRED,
        AGENT_TOOL_APPROVAL_STATUS_FAILED, AGENT_TOOL_APPROVAL_STATUS_REJECTED,
        CODEX_PLUGIN_OPERATION_REFRESH, COMPANY_SKILL_LANGUAGE_EN,
    },
};
use ai_chat_infrastructure::{
    build_repository,
    codex_control::{
        agent_trigger_batch_size_from_env, ClaimedCodexControlRequest, CodexControlRequestKind,
        CodexControlStore, CodexDefaultConfigSummary, CompanyCodexCliSettings,
        CODEX_CLI_OPERATION_FAILED, CODEX_CLI_OPERATION_IDLE, CODEX_DEFAULT_AUTH_STATUS_UNKNOWN,
        CODEX_INSTALLER_POSIX_SHELL, CODEX_INSTALLER_POWERSHELL,
    },
    codex_trigger::{
        CodexApprovalDecision, CodexApprovalHandler, CodexApprovalRequest,
        CodexCancellationHandler, CodexModelCatalogFile, CodexProgressEvent, CodexProgressHandler,
        CodexRunRequest, CodexRunResult, CodexRunStatus, CodexTriggerRunner,
    },
    config::ApiConfig,
    git_credentials::is_managed_token_profile,
    git_workspace::{is_git_authentication_error, GitWorkspaceManager, PreparedGitWorkspace},
    harness::HarnessProvisioner,
    RepositoryAdapter,
};
use ai_chat_shared::{hash_secret, now_utc, AppError, AppResult};

type TriggerPlatform = PlatformApp<RepositoryAdapter>;
type TriggerHarnessProvisioner = HarnessProvisioner<RepositoryAdapter>;
const CODEX_SESSION_POLICY_VERSION: &str = "relay-scoped-sessions-v9";
const RELAY_SKILL_BUNDLE_CONTROL: &str = "control";
const RELAY_SKILL_BUNDLE_PROJECT: &str = "project";
const EMPLOYEE_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-company-employee/SKILL.md");
const EMPLOYEE_SKILL_TEMPLATE_EN: &str =
    include_str!("../../../skills/relay-company-employee/references/en.md");
const STAFFING_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-company-staffing-manager/SKILL.md");
const STAFFING_SKILL_TEMPLATE_EN: &str =
    include_str!("../../../skills/relay-company-staffing-manager/references/en.md");

#[derive(Debug, Clone)]
struct TriggerServiceConfig {
    lease_owner: String,
    plugin_host_id: String,
    hostname: String,
    poll_interval: StdDuration,
    batch_size: usize,
    run_once: bool,
    model_catalog_path: PathBuf,
    model_discovery_profiles: Vec<String>,
    model_discovery_interval: StdDuration,
    default_auth_discovery_interval: StdDuration,
    mcp_discovery_interval: StdDuration,
    plugin_discovery_interval: StdDuration,
    codex_auto_install: bool,
    codex_install_url: String,
    codex_update_registry_url: String,
    codex_update_check_interval: StdDuration,
}

#[derive(Debug)]
struct TriggerExecution {
    succeeded: bool,
    error_message: Option<String>,
    retry_after_seconds: Option<i64>,
}

#[derive(Debug, Clone)]
struct EffectiveCodexCliSettings {
    model: Option<String>,
    reasoning_effort: Option<String>,
    reasoning_summary: Option<String>,
    verbosity: Option<String>,
    personality: Option<String>,
    service_tier: Option<String>,
    sandbox_mode: String,
    approval_policy: String,
    network_access: bool,
    web_search: String,
    feature_multi_agent: bool,
    feature_remote_plugin: bool,
    feature_hooks: bool,
    feature_goals: bool,
    feature_shell_tool: bool,
}

#[derive(Debug)]
struct PreparedRelaySkills {
    employee_name: String,
    profession_name: String,
    session_name: String,
    project_name: Option<String>,
    staffing_name: Option<String>,
    version_hash: String,
}

#[derive(Clone)]
struct PlatformCodexApprovalHandler {
    platform: TriggerPlatform,
    company_id: Uuid,
    run_id: Uuid,
    agent_id: Uuid,
    project_id: Option<Uuid>,
    expires_at: chrono::DateTime<chrono::Utc>,
    general_approval_required: bool,
    session_website_grants: Arc<Mutex<HashSet<(String, String)>>>,
}

#[derive(Clone)]
struct PlatformCodexProgressHandler {
    platform: TriggerPlatform,
    run_id: Uuid,
}

#[derive(Clone)]
struct PlatformRunCancellationHandler {
    platform: TriggerPlatform,
    agent_id: Uuid,
    project_id: Option<Uuid>,
}

impl CodexCancellationHandler for PlatformRunCancellationHandler {
    fn should_cancel(&self) -> bool {
        match self.platform.is_agent_codex_trigger_active(self.agent_id) {
            Ok(false) => return true,
            Ok(true) => {}
            Err(error) => {
                tracing::error!(
                    agent_id = %self.agent_id,
                    error = %error,
                    "failed to read Agent Trigger state; cancelling the Codex run defensively"
                );
                return true;
            }
        }
        let Some(project_id) = self.project_id else {
            return false;
        };
        match self.platform.is_company_project_paused(project_id) {
            Ok(paused) => paused,
            Err(error) => {
                tracing::error!(
                    project_id = %project_id,
                    error = %error,
                    "failed to read project pause state; cancelling the Codex run defensively"
                );
                true
            }
        }
    }

    fn cancellation_reason(&self) -> String {
        if self
            .platform
            .is_agent_codex_trigger_active(self.agent_id)
            .is_ok_and(|active| !active)
        {
            return "Codex run cancelled because the Agent Trigger was paused by Human".into();
        }
        "Codex run cancelled because the project was paused".into()
    }
}

impl CodexProgressHandler for PlatformCodexProgressHandler {
    fn report(&self, event: CodexProgressEvent) {
        if let Err(error) = self.platform.append_agent_codex_trigger_run_activity(
            self.run_id,
            AgentCodexRunActivity {
                at: now_utc(),
                phase: event.phase,
                summary: event.summary,
            },
            event.thread_id,
        ) {
            tracing::warn!(
                run_id = %self.run_id,
                error = %sanitize_error(&error.to_string()),
                "failed to persist Codex activity"
            );
        }
    }
}

#[async_trait]
impl CodexApprovalHandler for PlatformCodexApprovalHandler {
    async fn request_approval(
        &self,
        mut request: CodexApprovalRequest,
    ) -> AppResult<CodexApprovalDecision> {
        if let Some(decision) =
            automatic_codex_approval_decision(&request.tool_name, self.general_approval_required)
        {
            return Ok(decision);
        }
        let website_grant = website_approval_grant_key(
            &mut request.arguments,
            &request.tool_name,
            self.agent_id,
            self.project_id,
        );
        if let Some((approval_scope, approval_target)) = website_grant.as_ref() {
            if session_website_grant_allowed(
                &self.session_website_grants,
                approval_scope,
                approval_target,
            ) {
                record_run_activity(
                    &self.platform,
                    self.run_id,
                    "running",
                    &format!("已按本次会话的网站授权访问 {approval_target}"),
                    None,
                );
                return Ok(CodexApprovalDecision::Accept);
            }
            if self.platform.has_codex_always_allow_approval(
                self.company_id,
                self.agent_id,
                &request.tool_name,
                approval_scope,
                approval_target,
            )? {
                record_run_activity(
                    &self.platform,
                    self.run_id,
                    "running",
                    &format!("已按始终允许规则访问 {approval_target}"),
                    None,
                );
                return Ok(CodexApprovalDecision::Accept);
            }
            if let Some(local_target) = request
                .arguments
                .get(AGENT_CODEX_APPROVAL_LOCAL_TARGET_KEY)
                .and_then(serde_json::Value::as_str)
            {
                if self.platform.has_codex_always_allow_approval(
                    self.company_id,
                    self.agent_id,
                    &request.tool_name,
                    approval_scope,
                    local_target,
                )? {
                    record_run_activity(
                        &self.platform,
                        self.run_id,
                        "running",
                        &format!("已按本地预览端口授权访问 {approval_target}"),
                        None,
                    );
                    return Ok(CodexApprovalDecision::Accept);
                }
            }
        }
        let approval =
            match self
                .platform
                .create_codex_approval_request(CreateCodexApprovalRequestInput {
                    company_id: self.company_id,
                    codex_trigger_run_id: self.run_id,
                    requested_by_agent_id: self.agent_id,
                    tool_name: request.tool_name,
                    risk_level: request.risk_level,
                    reason: request.reason.clone(),
                    arguments: request.arguments,
                    expires_at: self.expires_at,
                }) {
                Ok(approval) => approval,
                Err(error) => {
                    record_run_activity(
                        &self.platform,
                        self.run_id,
                        "approval_delivery_failed",
                        "审批请求未能送达 Human，当前操作已停止，可重新唤醒后重试",
                        None,
                    );
                    return Err(error);
                }
            };
        record_run_activity(
            &self.platform,
            self.run_id,
            "waiting_approval",
            &format!("等待 Human 审批：{}", request.reason),
            None,
        );
        loop {
            let current = self
                .platform
                .get_codex_approval_request_for_runner(approval.id, self.run_id)?;
            match current.status.as_str() {
                AGENT_TOOL_APPROVAL_STATUS_APPROVED | AGENT_TOOL_APPROVAL_STATUS_EXECUTED => {
                    if let Some(grant) = website_grant.as_ref() {
                        remember_session_website_grant(&self.session_website_grants, grant);
                    }
                    record_run_activity(
                        &self.platform,
                        self.run_id,
                        "running",
                        "审批已通过，Codex 继续执行",
                        None,
                    );
                    return Ok(CodexApprovalDecision::Accept);
                }
                AGENT_TOOL_APPROVAL_STATUS_REJECTED | AGENT_TOOL_APPROVAL_STATUS_EXPIRED => {
                    record_run_activity(
                        &self.platform,
                        self.run_id,
                        "approval_rejected",
                        "审批未通过，本轮将停止相关操作",
                        None,
                    );
                    return Ok(CodexApprovalDecision::Decline);
                }
                AGENT_TOOL_APPROVAL_STATUS_FAILED => {
                    return Err(AppError::Validation(
                        current
                            .error_message
                            .unwrap_or_else(|| "Codex approval request failed".into()),
                    ));
                }
                _ => tokio::time::sleep(StdDuration::from_millis(500)).await,
            }
        }
    }
}

fn website_approval_grant_key(
    arguments: &mut serde_json::Value,
    tool_name: &str,
    agent_id: Uuid,
    project_id: Option<Uuid>,
) -> Option<(String, String)> {
    if tool_name != AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS {
        return None;
    }
    let url = arguments.get("url").and_then(serde_json::Value::as_str)?;
    let parsed = reqwest::Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let approval_target = parsed.origin().ascii_serialization();
    let approval_scope = project_id
        .map(|project_id| format!("project:{project_id}"))
        .unwrap_or_else(|| format!("agent:{agent_id}:control"));
    let object = arguments.as_object_mut()?;
    object.insert(
        AGENT_CODEX_APPROVAL_SCOPE_KEY.into(),
        serde_json::Value::String(approval_scope.clone()),
    );
    object.insert(
        AGENT_CODEX_APPROVAL_TARGET_KEY.into(),
        serde_json::Value::String(approval_target.clone()),
    );
    if let Some(local_target) = localhost_approval_target(&parsed) {
        object.insert(
            AGENT_CODEX_APPROVAL_LOCAL_TARGET_KEY.into(),
            serde_json::Value::String(local_target),
        );
    }
    Some((approval_scope, approval_target))
}

fn session_website_grant_allowed(
    grants: &Mutex<HashSet<(String, String)>>,
    approval_scope: &str,
    approval_target: &str,
) -> bool {
    grants
        .lock()
        .expect("session website grants")
        .contains(&(approval_scope.to_string(), approval_target.to_string()))
}

fn remember_session_website_grant(
    grants: &Mutex<HashSet<(String, String)>>,
    grant: &(String, String),
) {
    grants
        .lock()
        .expect("session website grants")
        .insert(grant.clone());
}

fn localhost_approval_target(parsed: &reqwest::Url) -> Option<String> {
    let host = parsed
        .host_str()?
        .trim_start_matches('[')
        .trim_end_matches(']');
    let is_loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    let port = parsed.port()?;
    (is_loopback && port >= 1_024).then(|| format!("{}://localhost:*", parsed.scheme()))
}

fn automatic_codex_approval_decision(
    tool_name: &str,
    general_approval_required: bool,
) -> Option<CodexApprovalDecision> {
    if general_approval_required || tool_name == AGENT_CODEX_APPROVAL_TOOL_WEBSITE_ACCESS {
        return None;
    }
    Some(if tool_name == AGENT_CODEX_APPROVAL_TOOL_PERMISSIONS {
        CodexApprovalDecision::Decline
    } else {
        CodexApprovalDecision::Accept
    })
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let api_config = ApiConfig::from_env();
    let repository = build_repository(&api_config)?;
    let platform = PlatformApp::new(repository.clone());
    let harness = HarnessProvisioner::from_config(repository, &api_config)?;
    let workspace_manager = GitWorkspaceManager::from_env()?;
    let codex_control = CodexControlStore::from_env()?;
    let codex_runner = CodexTriggerRunner::from_env()?;
    let config = TriggerServiceConfig::from_env()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run_trigger_loop(
        &platform,
        &harness,
        &workspace_manager,
        &codex_runner,
        &codex_control,
        &config,
    ))
}

async fn run_trigger_loop(
    platform: &TriggerPlatform,
    harness: &TriggerHarnessProvisioner,
    workspace_manager: &GitWorkspaceManager,
    codex_runner: &CodexTriggerRunner,
    codex_control: &CodexControlStore,
    config: &TriggerServiceConfig,
) -> anyhow::Result<()> {
    let initial_batch_size = effective_agent_trigger_batch_size(codex_control, config.batch_size)
        .unwrap_or(config.batch_size);
    tracing::info!(
        lease_owner = %config.lease_owner,
        poll_interval_seconds = config.poll_interval.as_secs(),
        environment_batch_size = config.batch_size,
        effective_batch_size = initial_batch_size,
        "local Codex Agent Trigger started"
    );

    let mut running = FuturesUnordered::new();
    let mut plugin_operations = FuturesUnordered::new();
    let mut shutdown = Box::pin(shutdown_signal());
    let mut next_model_discovery = tokio::time::Instant::now();
    let mut next_default_auth_discovery = tokio::time::Instant::now();
    let mut next_mcp_discovery = tokio::time::Instant::now();
    let mut next_plugin_discovery = tokio::time::Instant::now();
    let mut next_update_check = tokio::time::Instant::now();
    publish_codex_runtime_probe(codex_control, codex_runner)?;
    if codex_runner.detect_version().is_none() && config.codex_auto_install {
        let runtime = codex_control.runtime()?;
        if matches!(
            runtime.operation_status.as_str(),
            CODEX_CLI_OPERATION_IDLE | CODEX_CLI_OPERATION_FAILED
        ) {
            if let Err(error) = codex_control.enqueue_cli_install() {
                tracing::warn!(error = %sanitize_error(&error.to_string()), "cannot queue managed Codex CLI installation");
            }
        }
    }
    loop {
        if running.is_empty() && plugin_operations.is_empty() {
            if let Some(request) = codex_control.claim_next_request()? {
                process_codex_control_request(codex_control, codex_runner, config, request).await;
                publish_codex_runtime_probe(codex_control, codex_runner)?;
                next_model_discovery = tokio::time::Instant::now();
            }
        }
        if tokio::time::Instant::now() >= next_update_check {
            refresh_codex_latest_version(codex_control, codex_runner, config).await;
            next_update_check = tokio::time::Instant::now() + config.codex_update_check_interval;
        }
        if tokio::time::Instant::now() >= next_model_discovery {
            if let Err(error) =
                refresh_codex_model_catalog(codex_control, codex_runner, config).await
            {
                tracing::warn!(
                    error = %sanitize_error(&error.to_string()),
                    "failed to refresh the local Codex model catalog"
                );
            }
            next_model_discovery = tokio::time::Instant::now() + config.model_discovery_interval;
        }
        if tokio::time::Instant::now() >= next_default_auth_discovery {
            refresh_codex_default_auth(codex_control, codex_runner).await;
            next_default_auth_discovery =
                tokio::time::Instant::now() + config.default_auth_discovery_interval;
        }
        if tokio::time::Instant::now() >= next_mcp_discovery {
            refresh_codex_mcp_catalog(codex_control, codex_runner).await;
            next_mcp_discovery = tokio::time::Instant::now() + config.mcp_discovery_interval;
        }
        if tokio::time::Instant::now() >= next_plugin_discovery {
            if let Err(error) =
                refresh_codex_plugin_catalogs(platform, codex_control, codex_runner, config).await
            {
                tracing::warn!(
                    error = %sanitize_error(&error.to_string()),
                    "failed to refresh the local Codex plugin catalog"
                );
            }
            next_plugin_discovery = tokio::time::Instant::now() + config.plugin_discovery_interval;
        }
        if plugin_operations.is_empty() {
            match platform.claim_codex_plugin_operations(
                &config.plugin_host_id,
                &config.lease_owner,
                1,
            ) {
                Ok(claimed) => {
                    for operation in claimed {
                        plugin_operations.push(process_codex_plugin_operation(
                            platform,
                            codex_runner,
                            config,
                            operation,
                        ));
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        error = %sanitize_error(&error.to_string()),
                        "failed to claim Codex plugin operations; will retry"
                    );
                }
            }
        }
        let effective_batch_size = match effective_agent_trigger_batch_size(
            codex_control,
            config.batch_size,
        ) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(
                    error = %sanitize_error(&error.to_string()),
                    environment_batch_size = config.batch_size,
                    "cannot read managed Agent Trigger batch size; using the environment default"
                );
                config.batch_size
            }
        };
        let available_slots = effective_batch_size.saturating_sub(running.len());
        if available_slots > 0 {
            let claimed = platform
                .claim_due_agent_codex_triggers(&config.lease_owner, available_slots)
                .map_err(anyhow::Error::msg)?;
            for trigger in claimed {
                running.push(process_claimed_trigger(
                    platform,
                    harness,
                    workspace_manager,
                    codex_runner,
                    codex_control,
                    config,
                    trigger,
                ));
            }
        }

        if config.run_once {
            while running.next().await.is_some() {}
            while plugin_operations.next().await.is_some() {}
            break;
        }

        let shutdown_requested = if running.is_empty() && plugin_operations.is_empty() {
            tokio::select! {
                _ = tokio::time::sleep(config.poll_interval) => false,
                _ = &mut shutdown => true,
            }
        } else {
            tokio::select! {
                _ = tokio::time::sleep(config.poll_interval) => false,
                _ = running.next(), if !running.is_empty() => false,
                _ = plugin_operations.next(), if !plugin_operations.is_empty() => false,
                _ = &mut shutdown => true,
            }
        };
        if shutdown_requested {
            tracing::info!(
                lease_owner = %config.lease_owner,
                running_cycles = running.len(),
                "local Codex Agent Trigger is shutting down"
            );
            drop(running);
            match platform.abandon_agent_codex_trigger_leases(&config.lease_owner) {
                Ok(abandoned_runs) => tracing::info!(
                    lease_owner = %config.lease_owner,
                    abandoned_runs,
                    "released Codex trigger leases during shutdown"
                ),
                Err(error) => tracing::error!(
                    lease_owner = %config.lease_owner,
                    error = %sanitize_error(&error.to_string()),
                    "failed to release Codex trigger leases during shutdown"
                ),
            }
            break;
        }
    }
    Ok(())
}

fn effective_agent_trigger_batch_size(
    codex_control: &CodexControlStore,
    environment_default: usize,
) -> AppResult<usize> {
    Ok(codex_control
        .agent_trigger_preferences(environment_default)?
        .batch_size)
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut terminate = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

impl TriggerServiceConfig {
    fn from_env() -> AppResult<Self> {
        let instance = std::env::var("AGENT_TRIGGER_INSTANCE_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| Uuid::new_v4().simple().to_string());
        if instance.chars().count() > 80 || instance.chars().any(char::is_control) {
            return Err(AppError::Validation(
                "AGENT_TRIGGER_INSTANCE_ID is invalid".into(),
            ));
        }
        let host = whoami::fallible::hostname().unwrap_or_else(|_| "unknown-host".into());
        let os_user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown-user".into());
        let plugin_host_id = std::env::var("AGENT_TRIGGER_PLUGIN_HOST_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("{host}:{os_user}"));
        if plugin_host_id.chars().count() > 160 || plugin_host_id.chars().any(char::is_control) {
            return Err(AppError::Validation(
                "AGENT_TRIGGER_PLUGIN_HOST_ID is invalid".into(),
            ));
        }
        let poll_interval_seconds = std::env::var("AGENT_TRIGGER_POLL_INTERVAL_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map(|value| value.clamp(1, 60))
            .unwrap_or(2);
        let batch_size = agent_trigger_batch_size_from_env();
        let run_once = bool_env("AGENT_TRIGGER_RUN_ONCE", false);
        let model_catalog_path = std::env::var("AGENT_TRIGGER_MODEL_CATALOG_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".relay-agent-trigger/codex-models.json"));
        let model_discovery_profiles = std::env::var("AGENT_TRIGGER_MODEL_DISCOVERY_PROFILES")
            .ok()
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .filter(|profiles| !profiles.is_empty())
            .unwrap_or_else(|| vec!["default".into()]);
        let model_discovery_interval = StdDuration::from_secs(
            std::env::var("AGENT_TRIGGER_MODEL_DISCOVERY_INTERVAL_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .map(|value| value.clamp(60, 86_400))
                .unwrap_or(900),
        );
        let default_auth_discovery_interval = StdDuration::from_secs(
            std::env::var("AGENT_TRIGGER_CODEX_AUTH_DISCOVERY_INTERVAL_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .map(|value| value.clamp(15, 3_600))
                .unwrap_or(60),
        );
        let mcp_discovery_interval = StdDuration::from_secs(
            std::env::var("AGENT_TRIGGER_MCP_DISCOVERY_INTERVAL_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .map(|value| value.clamp(15, 3_600))
                .unwrap_or(60),
        );
        let plugin_discovery_interval = StdDuration::from_secs(
            std::env::var("AGENT_TRIGGER_PLUGIN_DISCOVERY_INTERVAL_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .map(|value| value.clamp(30, 86_400))
                .unwrap_or(300),
        );
        let codex_auto_install = bool_env("AGENT_TRIGGER_CODEX_AUTO_INSTALL", false);
        let codex_install_url = std::env::var("AGENT_TRIGGER_CODEX_INSTALL_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| default_codex_install_url(std::env::consts::OS).into());
        if !codex_install_url.starts_with("https://") {
            return Err(AppError::Validation(
                "AGENT_TRIGGER_CODEX_INSTALL_URL must use https".into(),
            ));
        }
        let codex_update_registry_url = std::env::var("AGENT_TRIGGER_CODEX_UPDATE_REGISTRY_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "https://registry.npmjs.org/@openai%2Fcodex/latest".into());
        if !codex_update_registry_url.starts_with("https://") {
            return Err(AppError::Validation(
                "AGENT_TRIGGER_CODEX_UPDATE_REGISTRY_URL must use https".into(),
            ));
        }
        let codex_update_check_interval = StdDuration::from_secs(
            std::env::var("AGENT_TRIGGER_CODEX_UPDATE_CHECK_INTERVAL_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .map(|value| value.clamp(300, 86_400))
                .unwrap_or(3_600),
        );
        Ok(Self {
            lease_owner: format!("{host}:{instance}"),
            plugin_host_id,
            hostname: host,
            poll_interval: StdDuration::from_secs(poll_interval_seconds),
            batch_size,
            run_once,
            model_catalog_path,
            model_discovery_profiles,
            model_discovery_interval,
            default_auth_discovery_interval,
            mcp_discovery_interval,
            plugin_discovery_interval,
            codex_auto_install,
            codex_install_url,
            codex_update_registry_url,
            codex_update_check_interval,
        })
    }
}

mod codex_control;
mod execution;
mod execution_result;
mod relay_skills;

use codex_control::*;
use execution::*;
use execution_result::*;
use relay_skills::*;

#[cfg(test)]
mod tests;
