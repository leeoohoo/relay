use std::{
    collections::HashSet,
    fs,
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
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
        AgentMemory, CodexPluginCatalogSnapshot, CodexPluginOperation, CompanyProject,
        CompanyProjectRule, AGENT_CODEX_RUN_STATUS_CANCELLED, AGENT_CODEX_RUN_STATUS_FAILED,
        AGENT_CODEX_RUN_STATUS_RUNNING, AGENT_CODEX_RUN_STATUS_SUCCEEDED,
        AGENT_CODEX_RUN_STATUS_TIMED_OUT, AGENT_CODEX_SETTING_INHERIT,
        AGENT_TOOL_APPROVAL_STATUS_APPROVED, AGENT_TOOL_APPROVAL_STATUS_EXECUTED,
        AGENT_TOOL_APPROVAL_STATUS_EXPIRED, AGENT_TOOL_APPROVAL_STATUS_FAILED,
        AGENT_TOOL_APPROVAL_STATUS_REJECTED, CODEX_PLUGIN_OPERATION_REFRESH,
        COMPANY_SKILL_LANGUAGE_EN,
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
        CodexRunRequest, CodexRunStatus, CodexTriggerRunner,
    },
    config::ApiConfig,
    git_workspace::{GitWorkspaceManager, PreparedGitWorkspace},
    RepositoryAdapter,
};
use ai_chat_shared::{hash_secret, now_utc, AppError, AppResult};

type TriggerPlatform = PlatformApp<RepositoryAdapter>;
const CODEX_SESSION_POLICY_VERSION: &str = "relay-skills-v8";
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
    project_name: Option<String>,
    staffing_name: Option<String>,
    #[cfg(test)]
    version_hash: String,
}

#[derive(Clone)]
struct PlatformCodexApprovalHandler {
    platform: TriggerPlatform,
    company_id: Uuid,
    run_id: Uuid,
    agent_id: Uuid,
    expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
struct PlatformCodexProgressHandler {
    platform: TriggerPlatform,
    run_id: Uuid,
}

#[derive(Clone)]
struct PlatformProjectCancellationHandler {
    platform: TriggerPlatform,
    project_id: Uuid,
}

impl CodexCancellationHandler for PlatformProjectCancellationHandler {
    fn should_cancel(&self) -> bool {
        match self.platform.is_company_project_paused(self.project_id) {
            Ok(paused) => paused,
            Err(error) => {
                tracing::error!(
                    project_id = %self.project_id,
                    error = %error,
                    "failed to read project pause state; cancelling the Codex run defensively"
                );
                true
            }
        }
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
        request: CodexApprovalRequest,
    ) -> AppResult<CodexApprovalDecision> {
        record_run_activity(
            &self.platform,
            self.run_id,
            "waiting_approval",
            &format!("等待 Human 审批：{}", request.reason),
            None,
        );
        let approval =
            self.platform
                .create_codex_approval_request(CreateCodexApprovalRequestInput {
                    company_id: self.company_id,
                    codex_trigger_run_id: self.run_id,
                    requested_by_agent_id: self.agent_id,
                    tool_name: request.tool_name,
                    risk_level: request.risk_level,
                    reason: request.reason,
                    arguments: request.arguments,
                    expires_at: self.expires_at,
                })?;
        loop {
            let current = self
                .platform
                .get_codex_approval_request_for_runner(approval.id, self.run_id)?;
            match current.status.as_str() {
                AGENT_TOOL_APPROVAL_STATUS_APPROVED | AGENT_TOOL_APPROVAL_STATUS_EXECUTED => {
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

fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let api_config = ApiConfig::from_env();
    let platform = PlatformApp::new(build_repository(&api_config)?);
    let workspace_manager = GitWorkspaceManager::from_env()?;
    let codex_control = CodexControlStore::from_env()?;
    let codex_runner = CodexTriggerRunner::from_env()?;
    let config = TriggerServiceConfig::from_env()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run_trigger_loop(
        &platform,
        &workspace_manager,
        &codex_runner,
        &codex_control,
        &config,
    ))
}

async fn run_trigger_loop(
    platform: &TriggerPlatform,
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
            if let Err(error) = refresh_codex_plugin_catalog(platform, codex_runner, config).await {
                tracing::warn!(
                    error = %sanitize_error(&error.to_string()),
                    "failed to refresh the local Codex plugin catalog"
                );
            }
            next_plugin_discovery = tokio::time::Instant::now() + config.plugin_discovery_interval;
        }
        if plugin_operations.is_empty() {
            let claimed = platform
                .claim_codex_plugin_operations(&config.plugin_host_id, &config.lease_owner, 1)
                .map_err(anyhow::Error::msg)?;
            for operation in claimed {
                plugin_operations.push(process_codex_plugin_operation(
                    platform,
                    codex_runner,
                    config,
                    operation,
                ));
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

fn publish_codex_runtime_probe(
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
) -> AppResult<()> {
    codex_control.publish_runtime_probe(
        codex_runner.detect_version(),
        codex_runner.executable_source(),
        codex_runner.executable_path(),
    )?;
    Ok(())
}

async fn refresh_codex_default_auth(
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
) {
    let result = codex_runner.probe_default_auth().await;
    let publish_result = match result {
        Ok(probe) => codex_control.publish_default_auth_probe(
            &probe.status,
            probe.method,
            probe.config,
            None,
        ),
        Err(error) => codex_control.publish_default_auth_probe(
            CODEX_DEFAULT_AUTH_STATUS_UNKNOWN,
            None,
            CodexDefaultConfigSummary::default(),
            Some(sanitize_error(&error.to_string())),
        ),
    };
    if let Err(error) = publish_result {
        tracing::warn!(
            error = %sanitize_error(&error.to_string()),
            "failed to publish the host Codex authentication status"
        );
    }
}

async fn refresh_codex_mcp_catalog(
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
) {
    let selectors = match codex_control.list_mcp_target_selectors() {
        Ok(selectors) => selectors,
        Err(error) => {
            tracing::warn!(error = %sanitize_error(&error.to_string()), "failed to list Codex MCP target environments");
            return;
        }
    };
    for selector in selectors {
        refresh_codex_mcp_environment(codex_control, codex_runner, &selector).await;
    }
}

async fn refresh_codex_mcp_environment(
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
    selector: &str,
) {
    let result = codex_runner.discover_mcp_servers(selector).await;
    let publish_result = match result {
        Ok(servers) => codex_control.publish_mcp_snapshot(selector, servers, None),
        Err(error) => codex_control.publish_mcp_snapshot(
            selector,
            Vec::new(),
            Some(sanitize_error(&error.to_string())),
        ),
    };
    if let Err(error) = publish_result {
        tracing::warn!(
            selector,
            error = %sanitize_error(&error.to_string()),
            "failed to publish the Codex MCP catalog"
        );
    }
}

async fn process_codex_control_request(
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
    config: &TriggerServiceConfig,
    claimed: ClaimedCodexControlRequest,
) {
    if let Err(error) = codex_control.mark_request_processing(&claimed.request) {
        tracing::error!(error = %sanitize_error(&error.to_string()), "cannot mark Codex control request as processing");
        return;
    }
    let result =
        execute_codex_control_request(codex_control, codex_runner, config, &claimed.request).await;
    match result {
        Ok(()) => {
            if let Err(error) = codex_control.finish_request(claimed) {
                tracing::error!(error = %sanitize_error(&error.to_string()), "cannot finish Codex control request");
            }
        }
        Err(error) => {
            let message = sanitize_error(&error.to_string());
            tracing::warn!(error = %message, "Codex control request failed");
            if let Err(store_error) = codex_control.fail_request(claimed, &message) {
                tracing::error!(error = %sanitize_error(&store_error.to_string()), "cannot persist Codex control request failure");
            }
        }
    }
}

async fn execute_codex_control_request(
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
    config: &TriggerServiceConfig,
    request: &ai_chat_infrastructure::codex_control::CodexControlRequest,
) -> AppResult<()> {
    match request.kind {
        CodexControlRequestKind::InstallCli => {
            let version = if let Some(version) = codex_runner.detect_version() {
                version
            } else {
                install_managed_codex(codex_control, codex_runner, config).await?
            };
            codex_control.mark_cli_operation_succeeded(
                version,
                codex_runner.executable_source(),
                codex_runner.executable_path(),
            )?;
        }
        CodexControlRequestKind::UpdateCli => {
            let version = codex_runner.update_cli().await?;
            codex_control.mark_cli_operation_succeeded(
                version,
                codex_runner.executable_source(),
                codex_runner.executable_path(),
            )?;
        }
        CodexControlRequestKind::ProvisionAuth => {
            if codex_runner.detect_version().is_none() {
                if !config.codex_auto_install {
                    return Err(AppError::Conflict(
                        "Codex CLI is not installed; install it before configuring an API key"
                            .into(),
                    ));
                }
                let version = install_managed_codex(codex_control, codex_runner, config).await?;
                codex_control.mark_cli_operation_succeeded(
                    version,
                    codex_runner.executable_source(),
                    codex_runner.executable_path(),
                )?;
            }
            let profile_id = request.profile_id.ok_or_else(|| {
                AppError::Internal("Codex authentication request has no profile id".into())
            })?;
            if request.api_key.is_none() && request.base_url.is_none() {
                return Err(AppError::Internal(
                    "Codex authentication request has no configuration changes".into(),
                ));
            }
            codex_runner
                .provision_auth_profile(
                    profile_id,
                    request.api_key.as_deref(),
                    request.base_url.as_deref(),
                )
                .await?;
            codex_control.mark_auth_profile_active(profile_id)?;
        }
        CodexControlRequestKind::DeleteAuth => {
            let profile_id = request.profile_id.ok_or_else(|| {
                AppError::Internal("Codex authentication deletion has no profile id".into())
            })?;
            let home = codex_control.managed_profile_home(profile_id);
            if home.exists() {
                tokio::fs::remove_dir_all(&home).await.map_err(|error| {
                    AppError::Internal(format!("cannot remove managed Codex profile: {error}"))
                })?;
            }
            codex_control.remove_auth_profile(profile_id)?;
        }
        CodexControlRequestKind::RefreshMcp => {
            let selector = request.target_selector.as_deref().ok_or_else(|| {
                AppError::Internal("Codex MCP refresh has no target selector".into())
            })?;
            refresh_codex_mcp_environment(codex_control, codex_runner, selector).await;
            let snapshot = codex_control
                .environment_for_company(request.company_id.ok_or_else(|| {
                    AppError::Internal("Codex MCP refresh has no company id".into())
                })?)?
                .mcp_environments
                .into_iter()
                .find(|snapshot| snapshot.selector == selector)
                .ok_or_else(|| AppError::Internal("Codex MCP refresh was not published".into()))?;
            if snapshot.status == "failed" {
                return Err(AppError::Internal(
                    snapshot
                        .last_error
                        .unwrap_or_else(|| "Codex MCP refresh failed".into()),
                ));
            }
        }
        CodexControlRequestKind::AddMcp => {
            let input = request.mcp_server.as_ref().ok_or_else(|| {
                AppError::Internal("Codex MCP add request has no server configuration".into())
            })?;
            codex_runner.add_mcp_server(input).await?;
            refresh_codex_mcp_environment(codex_control, codex_runner, &input.target_selector)
                .await;
        }
        CodexControlRequestKind::RemoveMcp => {
            let selector = request.target_selector.as_deref().ok_or_else(|| {
                AppError::Internal("Codex MCP removal has no target selector".into())
            })?;
            let server_name = request
                .mcp_server_name
                .as_deref()
                .ok_or_else(|| AppError::Internal("Codex MCP removal has no server name".into()))?;
            codex_runner
                .remove_mcp_server(selector, server_name)
                .await?;
            refresh_codex_mcp_environment(codex_control, codex_runner, selector).await;
        }
    }
    Ok(())
}

async fn install_managed_codex(
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
    config: &TriggerServiceConfig,
) -> AppResult<String> {
    if codex_runner.executable_source() != "managed" {
        return Err(AppError::Validation(format!(
            "configured Codex executable {} is unavailable; clear AGENT_TRIGGER_CODEX_BIN to use Relay managed installation",
            codex_runner.executable_path().display()
        )));
    }
    let response = reqwest::Client::builder()
        .connect_timeout(StdDuration::from_secs(15))
        .timeout(StdDuration::from_secs(60))
        .build()
        .map_err(|error| {
            AppError::Internal(format!("cannot create Codex installer client: {error}"))
        })?
        .get(&config.codex_install_url)
        .send()
        .await
        .map_err(|error| AppError::Internal(format!("cannot download Codex installer: {error}")))?
        .error_for_status()
        .map_err(|error| AppError::Internal(format!("Codex installer download failed: {error}")))?;
    if response
        .content_length()
        .is_some_and(|length| length > 5 * 1024 * 1024)
    {
        return Err(AppError::Internal(
            "Codex installer is unexpectedly large".into(),
        ));
    }
    let installer = response
        .bytes()
        .await
        .map_err(|error| AppError::Internal(format!("cannot read Codex installer: {error}")))?;
    if installer.len() > 5 * 1024 * 1024 {
        return Err(AppError::Internal(
            "Codex installer is unexpectedly large".into(),
        ));
    }
    tokio::fs::create_dir_all(codex_control.managed_cli_bin_dir())
        .await
        .map_err(|error| {
            AppError::Internal(format!("cannot create Codex install directory: {error}"))
        })?;
    tokio::fs::create_dir_all(codex_control.managed_cli_home())
        .await
        .map_err(|error| AppError::Internal(format!("cannot create Codex home: {error}")))?;

    let installer_command = codex_installer_command(std::env::consts::OS)?;
    tracing::info!(
        host_os = std::env::consts::OS,
        host_arch = std::env::consts::ARCH,
        installer_kind = installer_command.kind,
        "installing the Relay-managed Codex CLI"
    );
    let mut command = Command::new(installer_command.program);
    command
        .args(installer_command.arguments)
        .env("CODEX_HOME", codex_control.managed_cli_home())
        .env("CODEX_INSTALL_DIR", codex_control.managed_cli_bin_dir())
        .env("CODEX_NON_INTERACTIVE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|error| {
        AppError::Internal(format!("cannot start official Codex installer: {error}"))
    })?;
    let mut stdin = child.stdin.take().ok_or_else(|| {
        AppError::Internal("official Codex installer stdin is unavailable".into())
    })?;
    stdin.write_all(&installer).await.map_err(|error| {
        AppError::Internal(format!("cannot provide official Codex installer: {error}"))
    })?;
    drop(stdin);
    let output = tokio::time::timeout(StdDuration::from_secs(300), child.wait_with_output())
        .await
        .map_err(|_| AppError::Internal("official Codex installation timed out".into()))?
        .map_err(|error| AppError::Internal(format!("official Codex installer failed: {error}")))?;
    if !output.status.success() {
        return Err(AppError::Internal(format!(
            "official Codex installer failed: {}",
            sanitize_error(&String::from_utf8_lossy(&output.stderr))
        )));
    }
    codex_runner.detect_version().ok_or_else(|| {
        AppError::Internal(format!(
            "Codex installer completed but {} is unavailable",
            codex_runner.executable_path().display()
        ))
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodexInstallerCommand {
    program: &'static str,
    arguments: &'static [&'static str],
    kind: &'static str,
}

fn default_codex_install_url(host_os: &str) -> &'static str {
    match host_os {
        "windows" => "https://chatgpt.com/codex/install.ps1",
        _ => "https://chatgpt.com/codex/install.sh",
    }
}

fn codex_installer_command(host_os: &str) -> AppResult<CodexInstallerCommand> {
    match host_os {
        "macos" | "linux" => Ok(CodexInstallerCommand {
            program: "sh",
            arguments: &["-s"],
            kind: CODEX_INSTALLER_POSIX_SHELL,
        }),
        "windows" => Ok(CodexInstallerCommand {
            program: "powershell.exe",
            arguments: &[
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "-",
            ],
            kind: CODEX_INSTALLER_POWERSHELL,
        }),
        other => Err(AppError::Validation(format!(
            "Relay managed Codex installation does not support host operating system {other}"
        ))),
    }
}

async fn refresh_codex_latest_version(
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
    config: &TriggerServiceConfig,
) {
    if codex_runner.detect_version().is_none() {
        return;
    }
    let result = async {
        let response = reqwest::Client::builder()
            .connect_timeout(StdDuration::from_secs(10))
            .timeout(StdDuration::from_secs(20))
            .build()
            .map_err(|error| AppError::Internal(format!("cannot create update client: {error}")))?
            .get(&config.codex_update_registry_url)
            .send()
            .await
            .map_err(|error| AppError::Internal(format!("cannot check Codex updates: {error}")))?
            .error_for_status()
            .map_err(|error| AppError::Internal(format!("Codex update check failed: {error}")))?;
        let body = response
            .json::<serde_json::Value>()
            .await
            .map_err(|error| {
                AppError::Internal(format!("invalid Codex update response: {error}"))
            })?;
        body.get("version")
            .and_then(serde_json::Value::as_str)
            .filter(|version| !version.is_empty() && version.len() <= 80)
            .map(str::to_string)
            .ok_or_else(|| AppError::Internal("Codex update response has no version".into()))
    }
    .await;
    match result {
        Ok(version) => {
            if let Err(error) = codex_control.publish_latest_version(Some(version), None) {
                tracing::warn!(error = %sanitize_error(&error.to_string()), "cannot publish Codex latest version");
            }
        }
        Err(error) => {
            let message = sanitize_error(&error.to_string());
            if let Err(store_error) = codex_control.publish_latest_version(None, Some(message)) {
                tracing::warn!(error = %sanitize_error(&store_error.to_string()), "cannot publish Codex update check failure");
            }
        }
    }
}

async fn refresh_codex_model_catalog(
    codex_control: &CodexControlStore,
    codex_runner: &CodexTriggerRunner,
    config: &TriggerServiceConfig,
) -> AppResult<()> {
    let mut catalog = CodexModelCatalogFile::default();
    let mut profiles = config.model_discovery_profiles.clone();
    profiles.extend(codex_control.list_active_profile_selectors()?);
    profiles.sort();
    profiles.dedup();
    for profile in &profiles {
        for bundled in [false, true] {
            match codex_runner.discover_models(profile, bundled).await {
                Ok(snapshot) => catalog.catalogs.push(snapshot),
                Err(error) if bundled => tracing::debug!(
                    codex_profile = profile,
                    error = %sanitize_error(&error.to_string()),
                    "bundled Codex model discovery is unavailable"
                ),
                Err(error) => return Err(error),
            }
        }
    }
    let parent = config
        .model_catalog_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    tokio::fs::create_dir_all(parent).await.map_err(|error| {
        AppError::Internal(format!(
            "cannot create Codex model catalog directory: {error}"
        ))
    })?;
    let bytes = serde_json::to_vec_pretty(&catalog).map_err(|error| {
        AppError::Internal(format!("cannot serialize Codex model catalog: {error}"))
    })?;
    let temporary_path = config.model_catalog_path.with_extension("json.tmp");
    tokio::fs::write(&temporary_path, bytes)
        .await
        .map_err(|error| {
            AppError::Internal(format!("cannot write Codex model catalog: {error}"))
        })?;
    tokio::fs::rename(&temporary_path, &config.model_catalog_path)
        .await
        .map_err(|error| {
            AppError::Internal(format!("cannot publish Codex model catalog: {error}"))
        })?;
    tracing::info!(
        path = %config.model_catalog_path.display(),
        catalogs = catalog.catalogs.len(),
        "local Codex model catalog refreshed by Trigger"
    );
    Ok(())
}

async fn refresh_codex_plugin_catalog(
    platform: &TriggerPlatform,
    codex_runner: &CodexTriggerRunner,
    config: &TriggerServiceConfig,
) -> AppResult<String> {
    let discovery = codex_runner.discover_plugins().await?;
    let fingerprint = codex_plugin_fingerprint(&discovery.installed);
    let installed = public_codex_plugin_items(&discovery.installed);
    let available = public_codex_plugin_items(&discovery.available);
    let marketplaces = public_codex_marketplaces(&discovery.marketplaces);
    let now = now_utc();
    platform.save_codex_plugin_catalog_snapshot(CodexPluginCatalogSnapshot {
        runner_id: config.plugin_host_id.clone(),
        hostname: config.hostname.clone(),
        codex_version: codex_runner.detect_version(),
        fingerprint: fingerprint.clone(),
        installed,
        available,
        marketplaces,
        discovered_at: now,
        updated_at: now,
    })?;
    tracing::info!(
        runner_id = %config.plugin_host_id,
        fingerprint = %fingerprint,
        "local Codex plugin catalog refreshed by Trigger"
    );
    Ok(fingerprint)
}

async fn process_codex_plugin_operation(
    platform: &TriggerPlatform,
    codex_runner: &CodexTriggerRunner,
    config: &TriggerServiceConfig,
    operation: CodexPluginOperation,
) {
    let command_result = codex_runner
        .apply_plugin_operation(&operation.operation, operation.plugin_id.as_deref())
        .await;
    let (succeeded, mut result, error_message) = match command_result {
        Ok(result) => (true, result, None),
        Err(error) => (
            false,
            serde_json::json!({}),
            Some(sanitize_error(&error.to_string())),
        ),
    };
    let mut final_succeeded = succeeded;
    let mut final_error = error_message;
    if succeeded {
        match refresh_codex_plugin_catalog(platform, codex_runner, config).await {
            Ok(fingerprint) => {
                if let Some(object) = result.as_object_mut() {
                    object.insert("catalog_fingerprint".into(), fingerprint.into());
                }
            }
            Err(error) if operation.operation == CODEX_PLUGIN_OPERATION_REFRESH => {
                final_succeeded = false;
                final_error = Some(sanitize_error(&error.to_string()));
            }
            Err(error) => {
                if let Some(object) = result.as_object_mut() {
                    object.insert(
                        "catalog_refresh_error".into(),
                        sanitize_error(&error.to_string()).into(),
                    );
                }
            }
        }
    }
    if let Err(error) = platform.finish_codex_plugin_operation(
        operation.id,
        &config.lease_owner,
        final_succeeded,
        result,
        final_error,
    ) {
        tracing::error!(
            operation_id = %operation.id,
            error = %sanitize_error(&error.to_string()),
            "failed to finish Codex plugin operation"
        );
    }
}

fn codex_plugin_fingerprint(installed: &serde_json::Value) -> String {
    let mut plugins = installed
        .as_array()
        .into_iter()
        .flatten()
        .filter(|plugin| plugin.get("enabled").and_then(serde_json::Value::as_bool) != Some(false))
        .filter_map(|plugin| {
            let plugin_id = plugin.get("pluginId")?.as_str()?;
            let version = plugin
                .get("version")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            Some(format!("{plugin_id}@{version}"))
        })
        .collect::<Vec<_>>();
    plugins.sort();
    hash_secret(&plugins.join("\n")).chars().take(24).collect()
}

fn public_codex_plugin_items(items: &serde_json::Value) -> serde_json::Value {
    serde_json::Value::Array(
        items
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|plugin| {
                Some(serde_json::json!({
                    "pluginId": plugin.get("pluginId")?.as_str()?,
                    "name": plugin.get("name")?.as_str()?,
                    "marketplaceName": plugin.get("marketplaceName")?.as_str()?,
                    "version": plugin.get("version").and_then(serde_json::Value::as_str).unwrap_or_default(),
                    "installed": plugin.get("installed").and_then(serde_json::Value::as_bool).unwrap_or(false),
                    "enabled": plugin.get("enabled").and_then(serde_json::Value::as_bool).unwrap_or(false),
                    "installPolicy": plugin.get("installPolicy").and_then(serde_json::Value::as_str),
                    "authPolicy": plugin.get("authPolicy").and_then(serde_json::Value::as_str),
                }))
            })
            .collect(),
    )
}

fn public_codex_marketplaces(items: &serde_json::Value) -> serde_json::Value {
    serde_json::Value::Array(
        items
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|marketplace| {
                Some(serde_json::json!({
                    "name": marketplace.get("name")?.as_str()?,
                    "sourceType": marketplace
                        .get("marketplaceSource")
                        .and_then(|source| source.get("sourceType"))
                        .and_then(serde_json::Value::as_str),
                }))
            })
            .collect(),
    )
}

async fn process_claimed_trigger(
    platform: &TriggerPlatform,
    workspace_manager: &GitWorkspaceManager,
    codex_runner: &CodexTriggerRunner,
    codex_control: &CodexControlStore,
    service_config: &TriggerServiceConfig,
    trigger: AgentCodexTriggerConfig,
) {
    let execution = execute_trigger(
        platform,
        workspace_manager,
        codex_runner,
        codex_control,
        &trigger,
    )
    .await;
    let finished_at = now_utc();
    let completion = match execution {
        Ok(execution) => execution,
        Err(error) => TriggerExecution {
            succeeded: false,
            error_message: Some(sanitize_error(&error.to_string())),
        },
    };
    let next_run_at = next_trigger_run_at(&trigger, finished_at, completion.succeeded);
    if let Err(error) =
        platform.complete_agent_codex_trigger_lease(CompleteAgentCodexTriggerLeaseInput {
            trigger_config_id: trigger.id,
            lease_owner: service_config.lease_owner.clone(),
            finished_at,
            next_run_at,
            succeeded: completion.succeeded,
            error_message: completion.error_message.clone(),
        })
    {
        tracing::error!(
            trigger_id = %trigger.id,
            agent_id = %trigger.agent_profile_id,
            error = %sanitize_error(&error.to_string()),
            "failed to release Codex trigger lease"
        );
    } else if completion.succeeded {
        tracing::info!(
            trigger_id = %trigger.id,
            agent_id = %trigger.agent_profile_id,
            "Codex trigger cycle completed"
        );
    } else {
        tracing::warn!(
            trigger_id = %trigger.id,
            agent_id = %trigger.agent_profile_id,
            error = completion.error_message.as_deref().unwrap_or("unknown error"),
            "Codex trigger cycle failed"
        );
    }
}

fn resolve_effective_cli_settings(
    trigger: &AgentCodexTriggerConfig,
    company: &CompanyCodexCliSettings,
) -> EffectiveCodexCliSettings {
    EffectiveCodexCliSettings {
        model: trigger.model.clone().or_else(|| company.model.clone()),
        reasoning_effort: trigger
            .reasoning_effort
            .clone()
            .or_else(|| company.reasoning_effort.clone()),
        reasoning_summary: trigger
            .reasoning_summary
            .clone()
            .or_else(|| Some(company.reasoning_summary.clone())),
        verbosity: trigger
            .verbosity
            .clone()
            .or_else(|| company.verbosity.clone()),
        personality: trigger
            .personality
            .clone()
            .or_else(|| company.personality.clone()),
        service_tier: trigger
            .service_tier
            .clone()
            .or_else(|| company.service_tier.clone()),
        sandbox_mode: if trigger.sandbox_mode == AGENT_CODEX_SETTING_INHERIT {
            company.sandbox_mode.clone()
        } else {
            trigger.sandbox_mode.clone()
        },
        approval_policy: if trigger.approval_policy == AGENT_CODEX_SETTING_INHERIT {
            company.approval_policy.clone()
        } else {
            trigger.approval_policy.clone()
        },
        network_access: trigger.network_access.unwrap_or(company.network_access),
        web_search: trigger
            .web_search
            .clone()
            .unwrap_or_else(|| company.web_search.clone()),
        feature_multi_agent: trigger
            .feature_multi_agent
            .unwrap_or(company.feature_multi_agent),
        feature_remote_plugin: trigger
            .feature_remote_plugin
            .unwrap_or(company.feature_remote_plugin),
        feature_hooks: trigger.feature_hooks.unwrap_or(company.feature_hooks),
        feature_goals: trigger.feature_goals.unwrap_or(company.feature_goals),
        feature_shell_tool: trigger
            .feature_shell_tool
            .unwrap_or(company.feature_shell_tool),
    }
}

fn next_trigger_run_at(
    trigger: &AgentCodexTriggerConfig,
    finished_at: chrono::DateTime<chrono::Utc>,
    succeeded: bool,
) -> chrono::DateTime<chrono::Utc> {
    if succeeded {
        return finished_at + Duration::seconds(i64::from(trigger.interval_seconds));
    }
    let retry_delay_seconds = match trigger.consecutive_failure_count {
        0 => 10,
        1 => 30,
        _ => i64::from(trigger.interval_seconds),
    };
    finished_at + Duration::seconds(retry_delay_seconds)
}

async fn execute_trigger(
    platform: &TriggerPlatform,
    workspace_manager: &GitWorkspaceManager,
    codex_runner: &CodexTriggerRunner,
    codex_control: &CodexControlStore,
    trigger: &AgentCodexTriggerConfig,
) -> AppResult<TriggerExecution> {
    let company_settings = codex_control.company_cli_settings(trigger.company_id)?;
    let effective_settings = resolve_effective_cli_settings(trigger, &company_settings);
    let decision = protect_trigger_decision(|| platform.decide_agent_codex_work(trigger))?;
    if !decision.should_run {
        return Ok(TriggerExecution {
            succeeded: true,
            error_message: None,
        });
    }
    if platform.has_running_agent_codex_trigger_run(trigger.agent_profile_id) {
        tracing::info!(
            trigger_id = %trigger.id,
            agent_id = %trigger.agent_profile_id,
            "skipping duplicate Codex wake-up because the Agent already has a running cycle"
        );
        return Ok(TriggerExecution {
            succeeded: true,
            error_message: None,
        });
    }
    let agent = platform.get_agent_profile_by_id(trigger.agent_profile_id)?;
    let membership = platform.get_active_company_agent_membership(trigger.agent_profile_id)?;
    let workspace = match (decision.project.as_ref(), decision.git.as_ref()) {
        (Some(project), Some(git)) => workspace_manager.prepare_project_workspace(
            trigger.company_id,
            project.id,
            trigger.agent_profile_id,
            &agent.handle,
            git,
        )?,
        _ => workspace_manager
            .prepare_general_workspace(trigger.company_id, trigger.agent_profile_id)?,
    };
    let long_term_memories =
        platform.agent_long_term_memories(trigger.agent_profile_id, trigger.company_id)?;
    let project_view = decision
        .project
        .as_ref()
        .map(|project| {
            platform.get_company_project(GetCompanyProjectInput {
                actor_agent_id: trigger.agent_profile_id,
                company_id: trigger.company_id,
                project_id: project.id,
            })
        })
        .transpose()?;
    let skill_language = platform.effective_company_skill_language(trigger.company_id);
    let relay_skills = prepare_relay_skills(
        &workspace.path,
        &agent,
        &membership.job_title,
        &membership.permissions,
        &long_term_memories,
        project_view.as_ref().map(|view| &view.project),
        project_view.as_ref().and_then(|view| view.rule.as_ref()),
        &skill_language,
    )?;
    let started_at = now_utc();
    let initial_activity = AgentCodexRunActivity {
        at: started_at,
        phase: "preparing".into(),
        summary: format!("正在准备工作区：{}", workspace.branch),
    };
    let mut run = AgentCodexTriggerRun {
        id: Uuid::new_v4(),
        trigger_config_id: trigger.id,
        agent_profile_id: trigger.agent_profile_id,
        project_id: decision.project.as_ref().map(|project| project.id),
        trigger_type: decision.trigger_type.clone(),
        status: AGENT_CODEX_RUN_STATUS_RUNNING.into(),
        codex_thread_id: None,
        codex_version: codex_runner.detect_version(),
        exit_code: None,
        started_at,
        finished_at: None,
        final_message_summary: None,
        error_message: None,
        activity_phase: initial_activity.phase.clone(),
        activity_summary: Some(initial_activity.summary.clone()),
        last_activity_at: Some(initial_activity.at),
        activity_log: vec![initial_activity],
    };
    platform.insert_agent_codex_trigger_run(run.clone())?;
    let token_expiry = started_at + Duration::seconds(i64::from(trigger.max_run_seconds) + 60);
    let token = match platform.issue_agent_codex_run_token(
        run.id,
        trigger.agent_profile_id,
        token_expiry,
    ) {
        Ok(token) => token,
        Err(error) => {
            fail_run(platform, &mut run, None, error.to_string())?;
            return Err(error);
        }
    };
    let session_key = codex_session_key(&workspace);
    let existing_thread_id = platform
        .get_agent_codex_session(trigger.agent_profile_id)
        .and_then(|session| {
            if codex_session_key_matches(&session.worktree_key, &session_key) {
                Some(session.codex_thread_id)
            } else {
                tracing::info!(
                    agent_id = %trigger.agent_profile_id,
                    "starting a new Codex session because the saved session uses an older workspace or execution policy"
                );
                None
            }
        });
    let request = CodexRunRequest {
        cwd: workspace.path.clone(),
        codex_profile: trigger.codex_profile.clone(),
        model: effective_settings.model.clone(),
        reasoning_effort: effective_settings.reasoning_effort.clone(),
        reasoning_summary: effective_settings.reasoning_summary.clone(),
        verbosity: effective_settings.verbosity.clone(),
        personality: effective_settings.personality.clone(),
        service_tier: effective_settings.service_tier.clone(),
        sandbox_mode: effective_settings.sandbox_mode.clone(),
        approval_policy: effective_settings.approval_policy.clone(),
        network_access: effective_settings.network_access,
        web_search: effective_settings.web_search.clone(),
        feature_multi_agent: effective_settings.feature_multi_agent,
        feature_remote_plugin: effective_settings.feature_remote_plugin,
        feature_hooks: effective_settings.feature_hooks,
        feature_goals: effective_settings.feature_goals,
        feature_shell_tool: effective_settings.feature_shell_tool,
        max_run_seconds: trigger.max_run_seconds as u64,
        prompt: build_wakeup_prompt(WakeupPromptContext {
            agent: &agent,
            project_name: decision
                .project
                .as_ref()
                .map(|project| project.name.as_str()),
            pending_inbox_count: decision.pending_inbox_count,
            active_task_count: decision.active_task_count,
            waiting_task_count: decision.waiting_task_count,
            asset_refresh_due: decision.asset_refresh_due,
            workspace: &workspace,
            relay_skills: &relay_skills,
        }),
        existing_thread_id,
        run_token: token.plaintext_token,
        environment: workspace.auth_environment.clone(),
        approval_handler: (effective_settings.approval_policy == "on-request").then(|| {
            Arc::new(PlatformCodexApprovalHandler {
                platform: platform.clone(),
                company_id: trigger.company_id,
                run_id: run.id,
                agent_id: trigger.agent_profile_id,
                expires_at: started_at + Duration::seconds(i64::from(trigger.max_run_seconds)),
            }) as Arc<dyn CodexApprovalHandler>
        }),
        progress_handler: Some(Arc::new(PlatformCodexProgressHandler {
            platform: platform.clone(),
            run_id: run.id,
        }) as Arc<dyn CodexProgressHandler>),
        cancellation_handler: decision.project.as_ref().map(|project| {
            Arc::new(PlatformProjectCancellationHandler {
                platform: platform.clone(),
                project_id: project.id,
            }) as Arc<dyn CodexCancellationHandler>
        }),
    };
    let result = codex_runner.run(request).await;
    let revoke_result = platform.revoke_agent_codex_run_tokens(run.id);
    if let Err(error) = revoke_result {
        tracing::error!(run_id = %run.id, error = %sanitize_error(&error.to_string()), "failed to revoke Agent Run Token");
    }
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            fail_run(platform, &mut run, None, error.to_string())?;
            return Err(error);
        }
    };
    run.codex_thread_id = result.thread_id.clone();
    run.exit_code = result.exit_code;
    run.finished_at = Some(now_utc());
    run.final_message_summary = result
        .final_message
        .as_deref()
        .map(|message| truncate(message, 2_000));
    run.error_message = result
        .error_message
        .as_deref()
        .map(|message| truncate(&sanitize_error(message), 2_000));
    run.status = match result.status {
        CodexRunStatus::Succeeded => AGENT_CODEX_RUN_STATUS_SUCCEEDED,
        CodexRunStatus::Failed => AGENT_CODEX_RUN_STATUS_FAILED,
        CodexRunStatus::TimedOut => AGENT_CODEX_RUN_STATUS_TIMED_OUT,
        CodexRunStatus::Cancelled => AGENT_CODEX_RUN_STATUS_CANCELLED,
    }
    .into();
    if result.status == CodexRunStatus::Succeeded {
        let Some(thread_id) = result.thread_id else {
            let error =
                AppError::Validation("successful Codex run did not return a thread ID".into());
            fail_run(platform, &mut run, result.exit_code, error.to_string())?;
            return Err(error);
        };
        if let Err(error) = platform.save_agent_codex_session(AgentCodexSession {
            agent_profile_id: trigger.agent_profile_id,
            current_project_id: decision.project.as_ref().map(|project| project.id),
            codex_thread_id: thread_id,
            worktree_key: session_key,
            last_used_at: now_utc(),
        }) {
            fail_run(platform, &mut run, result.exit_code, error.to_string())?;
            return Err(error);
        }
    }
    platform.update_agent_codex_trigger_run(run.clone())?;
    let (final_phase, final_summary) = match result.status {
        CodexRunStatus::Succeeded => ("completed", "Codex 已完成本轮工作"),
        CodexRunStatus::Failed => ("failed", "Codex 本轮执行失败"),
        CodexRunStatus::TimedOut => ("timed_out", "Codex 本轮执行超时"),
        CodexRunStatus::Cancelled => ("cancelled", "项目已暂停，Codex 本轮已停止"),
    };
    record_run_activity(
        platform,
        run.id,
        final_phase,
        final_summary,
        run.codex_thread_id.clone(),
    );
    Ok(TriggerExecution {
        succeeded: matches!(
            result.status,
            CodexRunStatus::Succeeded | CodexRunStatus::Cancelled
        ),
        error_message: (result.status != CodexRunStatus::Cancelled)
            .then_some(result.error_message)
            .flatten(),
    })
}

fn protect_trigger_decision<T>(decision: impl FnOnce() -> AppResult<T>) -> AppResult<T> {
    catch_unwind(AssertUnwindSafe(decision)).map_err(|_| {
        AppError::Validation(
            "Codex trigger could not decide Agent work because the decision handler panicked"
                .into(),
        )
    })?
}

fn codex_session_key(workspace: &PreparedGitWorkspace) -> String {
    format!("{CODEX_SESSION_POLICY_VERSION}:{}", workspace.worktree_key)
}

fn codex_session_key_matches(saved_key: &str, current_key: &str) -> bool {
    saved_key == current_key || saved_key.starts_with(&format!("{current_key}:"))
}

fn fail_run(
    platform: &TriggerPlatform,
    run: &mut AgentCodexTriggerRun,
    exit_code: Option<i32>,
    error_message: String,
) -> AppResult<()> {
    run.status = AGENT_CODEX_RUN_STATUS_FAILED.into();
    run.exit_code = exit_code;
    run.finished_at = Some(now_utc());
    run.error_message = Some(truncate(&sanitize_error(&error_message), 2_000));
    platform.update_agent_codex_trigger_run(run.clone())?;
    record_run_activity(
        platform,
        run.id,
        "failed",
        &format!("本轮失败：{}", sanitize_error(&error_message)),
        run.codex_thread_id.clone(),
    );
    Ok(())
}

fn record_run_activity(
    platform: &TriggerPlatform,
    run_id: Uuid,
    phase: &str,
    summary: &str,
    codex_thread_id: Option<String>,
) {
    if let Err(error) = platform.append_agent_codex_trigger_run_activity(
        run_id,
        AgentCodexRunActivity {
            at: now_utc(),
            phase: phase.into(),
            summary: truncate(&sanitize_error(summary), 500),
        },
        codex_thread_id,
    ) {
        tracing::warn!(
            run_id = %run_id,
            error = %sanitize_error(&error.to_string()),
            "failed to persist Codex activity"
        );
    }
}

struct WakeupPromptContext<'a> {
    agent: &'a AgentProfile,
    project_name: Option<&'a str>,
    pending_inbox_count: usize,
    active_task_count: usize,
    waiting_task_count: usize,
    asset_refresh_due: bool,
    workspace: &'a PreparedGitWorkspace,
    relay_skills: &'a PreparedRelaySkills,
}

fn build_wakeup_prompt(context: WakeupPromptContext<'_>) -> String {
    let WakeupPromptContext {
        agent,
        project_name,
        pending_inbox_count,
        active_task_count,
        waiting_task_count,
        asset_refresh_due,
        workspace,
        relay_skills,
    } = context;
    let project_context = project_name
        .map(|name| format!("当前路由到项目：{name}。"))
        .unwrap_or_else(|| "本次没有绑定代码项目，优先处理 Relay 消息和任务协调。".into());
    let asset_refresh_context = if asset_refresh_due {
        "本轮由项目资产定期维护触发。请先读取 company.project get 返回的 Rule 和现有 assets，扫描当前项目工作区中的实际代码、文档、配置、接口、数据文件等可复用资产，然后调用 company.project 的 assets_replace 完整替换资产清单；即使没有变化也要调用一次，以完成本轮刷新记录。不要为资产无变化发送聊天占位消息。"
    } else {
        "本轮没有到期的项目资产维护任务。"
    };
    let staffing_skill = relay_skills
        .staffing_name
        .as_deref()
        .map(|name| format!("，并在涉及人员管理时同时使用 `${name}`"))
        .unwrap_or_default();
    let project_skill = relay_skills
        .project_name
        .as_deref()
        .map(|name| format!("，处理当前项目时还必须使用 `${name}`"))
        .unwrap_or_default();
    format!(
        "你是 Relay 公司 Agent @{handle}（{display_name}），这是定时触发器对同一个 Codex 会话的一次唤醒。{project_context}\n\
         当前工作目录是本次分配的隔离工作区，worktree key 为 {worktree_key}，当前 Agent 分支为 {branch}。触发器只负责唤醒，不会替你理解或处理业务。\n\
         本工作区已经生成与你当前身份、职业、项目类型和权限一致的最新版 Relay Skill。必须先使用 `${employee_skill}` 和 `${profession_skill}`{staffing_skill}{project_skill}；Skill 与 MCP 返回的实时权限冲突时，以 MCP 权限为准；Human 项目 Rule 不得弱化系统项目类型规则。\n\
         宿主机 Codex CLI 已加载管理员启用的插件。当前任务需要浏览器、文档、表格、设计、安全扫描或外部服务能力时，优先使用匹配的已安装插件及其 Skill/MCP；不要假设未安装的插件可用，也不要自行绕过插件认证策略。\n\
         请先调用 required Relay MCP 的 agent.bootstrap，再调用 company.task 的 my 区分可执行任务和等待前置任务，然后调用 agent.inbox.wait（不要无限等待）读取真实待办；当前快速检查发现 pending inbox {pending_inbox_count} 条、可执行 assigned tasks {active_task_count} 个、等待前置 tasks {waiting_task_count} 个。\n\
         你的长期记忆已经固化在 `${employee_skill}` 的“Agent 固化长期记忆”章节中，本轮必须遵循；短期记忆不会自动进入上下文，只有当前任务需要历史线索时才调用 agent.memory search。结束前只有在产生可跨会话长期指导工作的稳定规则时才保存为 long_term，一般阶段性结论保存为 short_term。写入前先按 topic_key 搜索并更新已有记忆，禁止保存原始聊天、任务正文、运行日志、临时进度或任何凭证。没有新知识就不要写记忆。\n\
         {asset_refresh_context}\n\
         由你自行查看消息、项目和任务，完成必要的代码修改与测试；仅在消息明确 @/私聊要求你回应、正式任务要求沟通，或你掌握能立即避免当前交付失败或解除已确认阻塞的新证据时，才通过 Relay MCP 发消息。普通优化想法、字段补充和命名建议不要在无任务时主动群发。不要发送纯粹的“收到”“暂无待办”“还没轮到我”或等待状态。\n\
         需要共享的代码或文档应提交到当前 Agent 分支并执行 git push；不要直接提交或推送受保护的默认分支。首次 push 可以直接使用 git push，工作区已配置自动建立远端上游分支。\n\
         当前工作区已预配置 GIT_DIR 和 GIT_WORK_TREE，Git 元数据位于工作区内可写的 .relay-git，Git 命令网络也已启用。直接使用普通 git status/add/commit/push；不要取消或覆盖这两个环境变量，不要创建替代 gitdir、嵌套仓库、导出仓库或 bundle。如果标准命令仍失败，保留原始错误并报告，不要自行改造仓库结构。\n\
         需要处理的事项完成后更新任务并 ack Inbox；已经确认无需行动的事件也应 ack 或标记已读，避免重复触发。不要让触发器代发消息，也不要输出给触发器解析的自定义行动 JSON。\n\
         如果没有分配给你的可执行工作、依赖尚未完成或还没有轮到你，不发送 Relay 消息，直接结束本轮。切勿操作当前工作目录之外的项目。",
        handle = agent.handle.trim_start_matches('@'),
        display_name = agent.display_name,
        worktree_key = workspace.worktree_key,
        branch = workspace.branch,
        employee_skill = relay_skills.employee_name,
        profession_skill = relay_skills.profession_name,
        project_skill = project_skill,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_relay_skills(
    workspace_path: &Path,
    agent: &AgentProfile,
    job_title: &str,
    permissions: &[String],
    long_term_memories: &[AgentMemory],
    project: Option<&CompanyProject>,
    project_rule: Option<&CompanyProjectRule>,
    skill_language: &str,
) -> AppResult<PreparedRelaySkills> {
    let profession = infer_company_profession(Some(job_title));
    let identity_token = relay_skill_identity_token(agent);
    let managed_prefix = format!("relay-{identity_token}-");
    let employee_name = format!("{managed_prefix}employee");
    let profession_name = format!(
        "{managed_prefix}profession-{}",
        profession.key.replace('_', "-")
    );
    let staffing_name = permissions
        .iter()
        .any(|permission| permission.starts_with("agent.staff."))
        .then(|| format!("{managed_prefix}staffing"));
    let project_name = project.map(|project| {
        format!(
            "{managed_prefix}project-{}",
            project
                .id
                .to_string()
                .replace('-', "")
                .chars()
                .take(8)
                .collect::<String>()
        )
    });

    let employee_template = if skill_language == COMPANY_SKILL_LANGUAGE_EN {
        EMPLOYEE_SKILL_TEMPLATE_EN
    } else {
        EMPLOYEE_SKILL_TEMPLATE
    };
    let staffing_template = if skill_language == COMPANY_SKILL_LANGUAGE_EN {
        STAFFING_SKILL_TEMPLATE_EN
    } else {
        STAFFING_SKILL_TEMPLATE
    };
    let employee_base_content = bind_relay_skill(
        &tailor_relay_skill_to_permissions(employee_template, permissions),
        &employee_name,
        agent,
        &employee_name,
        skill_language,
    );
    let employee_content =
        append_agent_long_term_memories(&employee_base_content, long_term_memories, skill_language);
    let profession_template = profession_skill_template(&profession.key, skill_language);
    let profession_content = bind_relay_skill(
        &profession_template,
        &profession_name,
        agent,
        &employee_name,
        skill_language,
    );
    let staffing_content = staffing_name.as_ref().map(|name| {
        bind_relay_skill(
            &tailor_relay_skill_to_permissions(staffing_template, permissions),
            name,
            agent,
            &employee_name,
            skill_language,
        )
    });
    let project_content = project.zip(project_name.as_deref()).map(|(project, name)| {
        bind_relay_skill(
            &build_project_skill_template(project, project_rule, skill_language),
            name,
            agent,
            &employee_name,
            skill_language,
        )
    });

    let skills_root = workspace_path.join(".agents/skills");
    fs::create_dir_all(&skills_root).map_err(|error| {
        AppError::Validation(format!(
            "failed to create managed Relay skills directory {}: {error}",
            skills_root.display()
        ))
    })?;
    remove_stale_managed_skills(&skills_root, &managed_prefix)?;
    write_managed_skill(&skills_root, &employee_name, &employee_content)?;
    write_managed_skill(&skills_root, &profession_name, &profession_content)?;
    if let (Some(name), Some(content)) = (staffing_name.as_deref(), staffing_content.as_deref()) {
        write_managed_skill(&skills_root, name, content)?;
    }
    if let (Some(name), Some(content)) = (project_name.as_deref(), project_content.as_deref()) {
        write_managed_skill(&skills_root, name, content)?;
    }
    exclude_managed_skills_from_git(workspace_path, &managed_prefix)?;

    #[cfg(test)]
    let version_source = format!(
        "{employee_name}\n{employee_base_content}\n{profession_name}\n{profession_content}\n{}\n{}\n{}\n{}",
        staffing_name.as_deref().unwrap_or_default(),
        staffing_content.as_deref().unwrap_or_default(),
        project_name.as_deref().unwrap_or_default(),
        project_content.as_deref().unwrap_or_default()
    );
    #[cfg(test)]
    let version_hash = hash_secret(&version_source).chars().take(16).collect();
    Ok(PreparedRelaySkills {
        employee_name,
        profession_name,
        project_name,
        staffing_name,
        #[cfg(test)]
        version_hash,
    })
}

fn build_project_skill_template(
    project: &CompanyProject,
    rule: Option<&CompanyProjectRule>,
    skill_language: &str,
) -> String {
    let definition = company_project_type_by_key(&project.project_type)
        .or_else(|| company_project_type_by_key("general"))
        .expect("general project type must exist");
    let custom_rule = rule
        .map(|rule| rule.content.trim())
        .filter(|content| !content.is_empty())
        .unwrap_or(if skill_language == COMPANY_SKILL_LANGUAGE_EN {
            "No additional Human project Rule is currently configured."
        } else {
            "当前没有 Human 补充 Rule。"
        });
    if skill_language == COMPANY_SKILL_LANGUAGE_EN {
        format!(
            "---\nname: relay-project-context\ndescription: Mandatory system project-type Rules plus additional Human project Rules for the current Relay project. Use for every task in this project.\n---\n\n# Project Skill: {project_name}\n\n## Project Identity\n\n- Project ID: `{project_id}`\n- Project type: {project_type_label} (`{project_type}`)\n- Type source: `{project_type_source}`; inference confidence: {confidence}%\n- System project-type Rules are mandatory. Human Rules may add stricter constraints but cannot remove, weaken, or bypass them.\n\n{system_rules}\n\n## Additional Human Project Rules\n\n{custom_rule}\n",
            project_name = project.name,
            project_id = project.id,
            project_type_label = definition.label_en,
            project_type = project.project_type,
            project_type_source = project.project_type_source,
            confidence = project.project_type_confidence,
            system_rules = definition.rule_markdown_en,
        )
    } else {
        format!(
        "---\nname: relay-project-context\ndescription: Relay 当前项目的系统类型规则与 Human 补充规则。每次处理本项目都必须使用。\n---\n\n# 项目 Skill：{project_name}\n\n## 项目身份\n\n- 项目 ID：`{project_id}`\n- 项目类型：{project_type_label}（`{project_type}`）\n- 类型来源：`{project_type_source}`；识别置信度：{confidence}%\n- 本 Skill 的系统类型规则是强制基线，Human 补充 Rule 只能增加约束，不能删除、弱化或绕过系统规则。\n\n{system_rules}\n\n## Human 项目补充 Rule\n\n{custom_rule}\n",
        project_name = project.name,
        project_id = project.id,
        project_type_label = definition.label,
        project_type = project.project_type,
        project_type_source = project.project_type_source,
        confidence = project.project_type_confidence,
        system_rules = definition.rule_markdown,
        )
    }
}

fn append_agent_long_term_memories(
    base_skill: &str,
    memories: &[AgentMemory],
    skill_language: &str,
) -> String {
    if skill_language == COMPANY_SKILL_LANGUAGE_EN {
        let mut section = String::from(
            "\n\n## Distilled Long-term Agent Memory\n\nThese entries belong only to the current Agent and are loaded on every Codex wake-up. Use them as durable guidance. If they conflict with the latest Human instruction, project Rule, repository state, or MCP state, follow current verified facts and update the memory after validation.\n",
        );
        if memories.is_empty() {
            section.push_str("\nNo distilled long-term memory is currently stored.\n");
        } else {
            let mut used_characters = section.chars().count();
            for memory in memories {
                let entry = format!(
                    "\n### {}\n\n- Topic key: `{}`\n- Conclusion: {}\n- When to use: {}\n- Importance: {}/5; confidence: {}%{}\n",
                    memory.title,
                    memory.topic_key,
                    memory.summary,
                    if memory.when_to_use.is_empty() { "Any work directly related to this topic" } else { &memory.when_to_use },
                    memory.importance,
                    memory.confidence,
                    if memory.tags.is_empty() { String::new() } else { format!("; tags: {}", memory.tags.join(", ")) }
                );
                if used_characters + entry.chars().count() > 12_000 {
                    section.push_str("\nAdditional long-term memories were omitted because of the context budget. Archive low-value entries or reduce long-term memory volume.\n");
                    break;
                }
                used_characters += entry.chars().count();
                section.push_str(&entry);
            }
        }
        return format!("{}{}\n", base_skill.trim(), section.trim_end());
    }
    let mut section = String::from(
        "\n\n## Agent 固化长期记忆\n\n这些内容只属于当前 Agent，并在每次 Codex 唤醒时自动进入本 Skill。它们用于长期指导工作；如果与 Human 最新指令、项目 Rule、当前代码或 MCP 实时状态冲突，以当前事实为准，并在核验后更新记忆。\n",
    );
    if memories.is_empty() {
        section.push_str("\n当前还没有固化长期记忆。\n");
    } else {
        let mut used_characters = section.chars().count();
        for memory in memories {
            let entry = format!(
                "\n### {}\n\n- 主题键：`{}`\n- 结论：{}\n- 使用场景：{}\n- 重要度：{}/5；置信度：{}%{}\n",
                memory.title,
                memory.topic_key,
                memory.summary,
                if memory.when_to_use.is_empty() {
                    "任何与该主题直接相关的工作"
                } else {
                    &memory.when_to_use
                },
                memory.importance,
                memory.confidence,
                if memory.tags.is_empty() {
                    String::new()
                } else {
                    format!("；标签：{}", memory.tags.join("、"))
                }
            );
            let entry_characters = entry.chars().count();
            if used_characters + entry_characters > 12_000 {
                section.push_str(
                    "\n其余长期记忆因上下文预算未注入；请归档低价值记忆或降低长期记忆数量。\n",
                );
                break;
            }
            used_characters += entry_characters;
            section.push_str(&entry);
        }
    }
    format!("{}{}\n", base_skill.trim(), section.trim_end())
}

fn relay_skill_identity_token(agent: &AgentProfile) -> String {
    let handle = agent
        .handle
        .trim_start_matches('@')
        .to_ascii_lowercase()
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
    let handle = if handle.is_empty() { "agent" } else { &handle };
    let id = agent.id.to_string().replace('-', "");
    format!(
        "{}-{}",
        handle.chars().take(36).collect::<String>(),
        &id[..8]
    )
}

fn profession_skill_template(profession_key: &str, skill_language: &str) -> String {
    let profession = company_profession_by_key(profession_key)
        .or_else(|| company_profession_by_key("general_member"))
        .expect("general member profession must exist");
    if skill_language == COMPANY_SKILL_LANGUAGE_EN {
        profession.skill_markdown_en
    } else {
        profession.skill_markdown
    }
}

fn tailor_relay_skill_to_permissions(template: &str, permissions: &[String]) -> String {
    let permissions = permissions
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut remaining = template;
    let mut output = String::new();
    const PREFIX: &str = "<!-- relay-permission:";
    while let Some(start) = remaining.find(PREFIX) {
        output.push_str(&remaining[..start]);
        let marker = &remaining[start + PREFIX.len()..];
        let Some(permission_end) = marker.find(":start -->") else {
            output.push_str(&remaining[start..]);
            return output;
        };
        let permission = &marker[..permission_end];
        let block_start = start + PREFIX.len() + permission_end + ":start -->".len();
        let end_marker = format!("<!-- relay-permission:{permission}:end -->");
        let Some(relative_end) = remaining[block_start..].find(&end_marker) else {
            output.push_str(&remaining[start..]);
            return output;
        };
        if permissions.contains(permission) {
            output.push_str(remaining[block_start..block_start + relative_end].trim());
        }
        remaining = &remaining[block_start + relative_end + end_marker.len()..];
    }
    output.push_str(remaining);
    while output.contains("\n\n\n") {
        output = output.replace("\n\n\n", "\n\n");
    }
    format!("{}\n", output.trim())
}

fn bind_relay_skill(
    template: &str,
    skill_name: &str,
    agent: &AgentProfile,
    employee_skill_name: &str,
    skill_language: &str,
) -> String {
    let mut replaced_name = false;
    let mut content = template
        .lines()
        .map(|line| {
            if !replaced_name && line.starts_with("name:") {
                replaced_name = true;
                format!("name: {skill_name}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .replace("relay-company-employee", employee_skill_name);
    let identity_guide = if skill_language == COMPANY_SKILL_LANGUAGE_EN {
        format!(
            "\n\n## Relay Account Binding\n\n- This Skill represents only Relay Agent `@{}` (`{}`).\n- Call `agent.bootstrap` first on every cycle and stop immediately if the returned identity differs.\n- Use only company, project, task, and permission data returned by MCP in the current cycle.",
            agent.handle.trim_start_matches('@'),
            agent.id
        )
    } else {
        format!(
            "\n\n## Relay 账号绑定\n\n- 本 Skill 只代表 Relay Agent `@{}`（`{}`）。\n- 每轮先调用 `agent.bootstrap` 核对返回身份；身份不一致时立即停止。\n- 只使用本轮 MCP 返回的公司、项目、任务和权限。",
            agent.handle.trim_start_matches('@'),
            agent.id
        )
    };
    if let Some(heading_start) = content.find("\n# ") {
        let heading_start = heading_start + 1;
        let heading_end = content[heading_start..]
            .find('\n')
            .map(|offset| heading_start + offset)
            .unwrap_or(content.len());
        content.insert_str(heading_end, &identity_guide);
    }
    format!("{}\n", content.trim())
}

fn remove_stale_managed_skills(skills_root: &Path, managed_prefix: &str) -> AppResult<()> {
    for entry in fs::read_dir(skills_root).map_err(|error| {
        AppError::Validation(format!(
            "failed to inspect managed Relay skills in {}: {error}",
            skills_root.display()
        ))
    })? {
        let entry = entry.map_err(|error| {
            AppError::Validation(format!("failed to inspect managed Relay skill: {error}"))
        })?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if file_name.starts_with(managed_prefix) && entry.path().is_dir() {
            fs::remove_dir_all(entry.path()).map_err(|error| {
                AppError::Validation(format!(
                    "failed to replace managed Relay skill {}: {error}",
                    entry.path().display()
                ))
            })?;
        }
    }
    Ok(())
}

fn write_managed_skill(skills_root: &Path, name: &str, content: &str) -> AppResult<()> {
    let directory = skills_root.join(name);
    fs::create_dir_all(&directory).map_err(|error| {
        AppError::Validation(format!(
            "failed to create managed Relay skill {}: {error}",
            directory.display()
        ))
    })?;
    fs::write(directory.join("SKILL.md"), content).map_err(|error| {
        AppError::Validation(format!(
            "failed to write managed Relay skill {}: {error}",
            directory.display()
        ))
    })
}

fn exclude_managed_skills_from_git(workspace_path: &Path, managed_prefix: &str) -> AppResult<()> {
    let info_directory = workspace_path.join(".relay-git/info");
    fs::create_dir_all(&info_directory).map_err(|error| {
        AppError::Validation(format!(
            "failed to prepare Relay Git exclude directory {}: {error}",
            info_directory.display()
        ))
    })?;
    let exclude_path = info_directory.join("exclude");
    let mut existing = fs::read_to_string(&exclude_path).unwrap_or_default();
    let pattern = format!("/.agents/skills/{managed_prefix}*/");
    if !existing.lines().any(|line| line.trim() == pattern) {
        if !existing.is_empty() && !existing.ends_with('\n') {
            existing.push('\n');
        }
        existing.push_str(&pattern);
        existing.push('\n');
        fs::write(&exclude_path, existing).map_err(|error| {
            AppError::Validation(format!(
                "failed to update Relay Git exclude file {}: {error}",
                exclude_path.display()
            ))
        })?;
    }
    Ok(())
}

fn bool_env(name: &str, default: bool) -> bool {
    std::env::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn sanitize_error(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

fn truncate(value: &str, max_characters: usize) -> String {
    value.chars().take(max_characters).collect()
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::PathBuf};

    use ai_chat_domain::agent_identity::AgentStatus;

    use super::*;

    #[test]
    fn managed_codex_installer_supports_macos_linux_and_windows() {
        for host_os in ["macos", "linux"] {
            let command = codex_installer_command(host_os).expect("POSIX installer");
            assert_eq!(command.program, "sh");
            assert_eq!(command.arguments, &["-s"]);
            assert_eq!(command.kind, CODEX_INSTALLER_POSIX_SHELL);
            assert_eq!(
                default_codex_install_url(host_os),
                "https://chatgpt.com/codex/install.sh"
            );
        }

        let windows = codex_installer_command("windows").expect("Windows installer");
        assert_eq!(windows.program, "powershell.exe");
        assert!(windows.arguments.contains(&"-NonInteractive"));
        assert!(windows.arguments.contains(&"Bypass"));
        assert_eq!(windows.kind, CODEX_INSTALLER_POWERSHELL);
        assert_eq!(
            default_codex_install_url("windows"),
            "https://chatgpt.com/codex/install.ps1"
        );
    }

    #[test]
    fn managed_codex_installer_rejects_unknown_operating_systems() {
        let error = codex_installer_command("plan9").expect_err("unsupported platform");
        assert!(error.to_string().contains("does not support"));
    }

    #[test]
    fn session_key_is_stable_when_skills_language_or_plugins_change() {
        let workspace = PreparedGitWorkspace {
            path: PathBuf::from("/tmp/relay-agent"),
            worktree_key: "project/agent".into(),
            branch: "relay/agent/inbox".into(),
            auth_environment: HashMap::new(),
        };
        assert_eq!(
            codex_session_key(&workspace),
            "relay-skills-v8:project/agent"
        );
        assert!(codex_session_key_matches(
            "relay-skills-v8:project/agent:old-skill-hash:old-plugin-hash",
            "relay-skills-v8:project/agent"
        ));
        assert!(!codex_session_key_matches(
            "relay-skills-v8:another-project/agent:old-skill-hash:old-plugin-hash",
            "relay-skills-v8:project/agent"
        ));
    }

    #[test]
    fn plugin_fingerprint_is_stable_and_tracks_enabled_versions() {
        let first = serde_json::json!([
            {"pluginId": "browser@openai-bundled", "version": "2", "enabled": true},
            {"pluginId": "github@openai-api-curated", "version": "1", "enabled": true}
        ]);
        let reordered = serde_json::json!([
            {"pluginId": "github@openai-api-curated", "version": "1", "enabled": true},
            {"pluginId": "browser@openai-bundled", "version": "2", "enabled": true}
        ]);
        assert_eq!(
            codex_plugin_fingerprint(&first),
            codex_plugin_fingerprint(&reordered)
        );
        assert_ne!(
            codex_plugin_fingerprint(&first),
            codex_plugin_fingerprint(&serde_json::json!([
                {"pluginId": "browser@openai-bundled", "version": "3", "enabled": true}
            ]))
        );
    }

    #[test]
    fn permission_blocks_are_removed_when_the_agent_lacks_the_permission() {
        let template = "before\n<!-- relay-permission:task.assign:start -->secret\n<!-- relay-permission:task.assign:end -->\nafter";
        assert_eq!(
            tailor_relay_skill_to_permissions(template, &[]),
            "before\n\nafter\n"
        );
        assert!(
            tailor_relay_skill_to_permissions(template, &["task.assign".into()]).contains("secret")
        );
    }

    #[test]
    fn relay_skills_are_materialized_in_the_codex_repo_skill_location() {
        let workspace = std::env::temp_dir().join(format!("relay-skill-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&workspace).expect("test workspace should be created");
        let agent = AgentProfile {
            id: Uuid::new_v4(),
            owner_user_id: Uuid::new_v4(),
            display_name: "Luna".into(),
            handle: "luna-engineer".into(),
            persona: "负责实现".into(),
            collaboration_preference: "available".into(),
            status: AgentStatus::Active,
            created_at: now_utc(),
        };
        let prepared = prepare_relay_skills(
            &workspace,
            &agent,
            "软件工程师",
            &["task.update".into()],
            &[],
            None,
            None,
            "zh-CN",
        )
        .expect("managed skills should be generated");
        assert!(workspace
            .join(".agents/skills")
            .join(&prepared.employee_name)
            .join("SKILL.md")
            .is_file());
        assert!(workspace
            .join(".agents/skills")
            .join(&prepared.profession_name)
            .join("SKILL.md")
            .is_file());
        assert!(!prepared.version_hash.is_empty());
        fs::remove_dir_all(workspace).expect("test workspace should be removed");
    }

    #[test]
    fn english_relay_skills_are_materialized_without_chinese_operating_rules() {
        let workspace =
            std::env::temp_dir().join(format!("relay-skill-en-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&workspace).expect("test workspace should be created");
        let agent = AgentProfile {
            id: Uuid::new_v4(),
            owner_user_id: Uuid::new_v4(),
            display_name: "Luna".into(),
            handle: "luna-security".into(),
            persona: "Own application security".into(),
            collaboration_preference: "available".into(),
            status: AgentStatus::Active,
            created_at: now_utc(),
        };
        let prepared = prepare_relay_skills(
            &workspace,
            &agent,
            "Security Engineer",
            &["task.update".into()],
            &[],
            None,
            None,
            "en",
        )
        .expect("English managed skills should be generated");
        let employee_skill = fs::read_to_string(
            workspace
                .join(".agents/skills")
                .join(&prepared.employee_name)
                .join("SKILL.md"),
        )
        .expect("employee skill should be readable");
        let profession_skill = fs::read_to_string(
            workspace
                .join(".agents/skills")
                .join(&prepared.profession_name)
                .join("SKILL.md"),
        )
        .expect("profession skill should be readable");
        assert!(employee_skill.contains("Relay Account Binding"));
        assert!(profession_skill.contains("Shared Professional Operating Baseline"));
        assert!(profession_skill.contains("Security Engineer"));
        fs::remove_dir_all(workspace).expect("test workspace should be removed");
    }

    #[test]
    fn long_term_memories_are_injected_without_rotating_the_codex_session() {
        let workspace =
            std::env::temp_dir().join(format!("relay-memory-skill-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&workspace).expect("test workspace should be created");
        let agent = AgentProfile {
            id: Uuid::new_v4(),
            owner_user_id: Uuid::new_v4(),
            display_name: "Luna".into(),
            handle: "luna-memory".into(),
            persona: "负责实现".into(),
            collaboration_preference: "available".into(),
            status: AgentStatus::Active,
            created_at: now_utc(),
        };
        let without_memory = prepare_relay_skills(
            &workspace,
            &agent,
            "软件工程师",
            &[],
            &[],
            None,
            None,
            "zh-CN",
        )
        .expect("base skills should be generated");
        let now = now_utc();
        let memory = AgentMemory {
            id: Uuid::new_v4(),
            company_id: Uuid::new_v4(),
            owner_agent_id: agent.id,
            scope: "agent".into(),
            project_id: None,
            memory_tier: "long_term".into(),
            memory_type: "procedure".into(),
            topic_key: "always-run-migrations".into(),
            title: "发布前验证迁移".into(),
            summary: "数据库变更发布前必须验证向前迁移和回滚路径。".into(),
            when_to_use: "涉及数据库 schema 变更时".into(),
            tags: vec!["database".into()],
            importance: 5,
            confidence: 95,
            pinned: true,
            status: "active".into(),
            source_refs: Vec::new(),
            supersedes_memory_id: None,
            expires_at: None,
            verified_by_agent_id: Some(agent.id),
            verified_by_human_user_id: None,
            verified_at: Some(now),
            created_by_agent_id: Some(agent.id),
            created_by_human_user_id: None,
            updated_by_agent_id: Some(agent.id),
            updated_by_human_user_id: None,
            created_at: now,
            updated_at: now,
        };
        let with_memory = prepare_relay_skills(
            &workspace,
            &agent,
            "软件工程师",
            &[],
            std::slice::from_ref(&memory),
            None,
            None,
            "zh-CN",
        )
        .expect("memory-bound skills should be generated");
        let employee_skill = fs::read_to_string(
            workspace
                .join(".agents/skills")
                .join(&with_memory.employee_name)
                .join("SKILL.md"),
        )
        .expect("employee skill should be readable");
        assert!(employee_skill.contains("Agent 固化长期记忆"));
        assert!(employee_skill.contains("always-run-migrations"));
        assert!(employee_skill.contains("数据库变更发布前必须验证"));
        assert_eq!(without_memory.version_hash, with_memory.version_hash);
        fs::remove_dir_all(workspace).expect("test workspace should be removed");
    }

    #[test]
    fn project_skill_places_fixed_type_rules_before_human_supplements() {
        let now = now_utc();
        let project = CompanyProject {
            id: Uuid::new_v4(),
            company_id: Uuid::new_v4(),
            name: "Relay Web".into(),
            description: "开发管理控制台".into(),
            project_type: "software_development".into(),
            project_type_source: "human".into(),
            project_type_confidence: 100,
            project_type_evidence: vec!["package.json".into()],
            status: "active".into(),
            owner_agent_id: Uuid::new_v4(),
            project_group_conversation_id: Uuid::new_v4(),
            created_by_agent_id: Uuid::new_v4(),
            updated_by_agent_id: None,
            due_at: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        let rule = CompanyProjectRule {
            project_id: project.id,
            content: "所有页面文案使用中文。".into(),
            updated_by_agent_id: None,
            updated_by_human_user_id: Some(Uuid::new_v4()),
            created_at: now,
            updated_at: now,
        };
        let skill = build_project_skill_template(&project, Some(&rule), "zh-CN");
        let fixed_rule = skill.find("项目治理与完成定义").expect("fixed rule");
        let supplement = skill.find("所有页面文案使用中文").expect("human rule");
        assert!(fixed_rule < supplement);
        assert!(skill.contains("SVG"));
        assert!(skill.contains("不能删除、弱化或绕过系统规则"));

        let english_skill = build_project_skill_template(&project, Some(&rule), "en");
        assert!(english_skill.contains("Project Governance and Definition of Done"));
        assert!(english_skill.contains("System project-type Rules are mandatory"));
        assert!(english_skill.contains("Additional Human Project Rules"));
        assert!(english_skill.contains("所有页面文案使用中文"));
    }

    #[test]
    fn trigger_decision_panics_are_returned_as_errors_instead_of_stopping_the_service() {
        let error = protect_trigger_decision::<()>(|| panic!("test decision panic"))
            .expect_err("a decision panic should become a recoverable trigger error");
        assert!(matches!(error, AppError::Validation(_)));
    }

    #[test]
    fn failed_runs_retry_quickly_before_the_trigger_enters_error_state() {
        let mut trigger = AgentCodexTriggerConfig {
            id: Uuid::new_v4(),
            company_id: Uuid::new_v4(),
            agent_profile_id: Uuid::new_v4(),
            status: "active".into(),
            interval_seconds: 3_600,
            codex_profile: "default".into(),
            model: None,
            reasoning_effort: None,
            reasoning_summary: None,
            verbosity: None,
            personality: None,
            service_tier: None,
            sandbox_mode: "workspace_write".into(),
            approval_policy: "never".into(),
            network_access: None,
            web_search: None,
            feature_multi_agent: None,
            feature_remote_plugin: None,
            feature_hooks: None,
            feature_goals: None,
            feature_shell_tool: None,
            max_run_seconds: 1_800,
            next_run_at: now_utc(),
            lease_owner: None,
            lease_expires_at: None,
            manual_run_requested_at: None,
            wake_requested_at: None,
            wake_reason: None,
            last_run_at: None,
            last_success_at: None,
            last_error: None,
            consecutive_failure_count: 0,
            created_by_human_user_id: Uuid::new_v4(),
            updated_by_human_user_id: Some(Uuid::new_v4()),
            created_at: now_utc(),
            updated_at: now_utc(),
        };
        let finished_at = now_utc();
        assert_eq!(
            next_trigger_run_at(&trigger, finished_at, false),
            finished_at + Duration::seconds(10)
        );
        trigger.consecutive_failure_count = 1;
        assert_eq!(
            next_trigger_run_at(&trigger, finished_at, false),
            finished_at + Duration::seconds(30)
        );
        assert_eq!(
            next_trigger_run_at(&trigger, finished_at, true),
            finished_at + Duration::seconds(3_600)
        );
    }

    #[test]
    fn runner_overrides_resolve_without_losing_company_defaults() {
        let company_id = Uuid::new_v4();
        let mut company = CompanyCodexCliSettings::new(company_id);
        company.model = Some("company-model".into());
        company.reasoning_effort = Some("medium".into());
        company.approval_policy = "on-request".into();
        company.network_access = false;
        company.web_search = "indexed".into();
        let trigger = AgentCodexTriggerConfig {
            id: Uuid::new_v4(),
            company_id,
            agent_profile_id: Uuid::new_v4(),
            status: "active".into(),
            interval_seconds: 3_600,
            codex_profile: "default".into(),
            model: None,
            reasoning_effort: Some("high".into()),
            reasoning_summary: None,
            verbosity: None,
            personality: None,
            service_tier: Some("fast".into()),
            sandbox_mode: AGENT_CODEX_SETTING_INHERIT.into(),
            approval_policy: AGENT_CODEX_SETTING_INHERIT.into(),
            network_access: None,
            web_search: Some("live".into()),
            feature_multi_agent: None,
            feature_remote_plugin: Some(false),
            feature_hooks: None,
            feature_goals: None,
            feature_shell_tool: None,
            max_run_seconds: 1_800,
            next_run_at: now_utc(),
            lease_owner: None,
            lease_expires_at: None,
            manual_run_requested_at: None,
            wake_requested_at: None,
            wake_reason: None,
            last_run_at: None,
            last_success_at: None,
            last_error: None,
            consecutive_failure_count: 0,
            created_by_human_user_id: Uuid::new_v4(),
            updated_by_human_user_id: None,
            created_at: now_utc(),
            updated_at: now_utc(),
        };

        let effective = resolve_effective_cli_settings(&trigger, &company);
        assert_eq!(effective.model.as_deref(), Some("company-model"));
        assert_eq!(effective.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(effective.approval_policy, "on-request");
        assert!(!effective.network_access);
        assert_eq!(effective.web_search, "live");
        assert!(!effective.feature_remote_plugin);
        assert!(effective.feature_hooks);
    }

    #[test]
    fn managed_batch_size_overrides_the_environment_default() {
        let root = std::env::temp_dir().join(format!(
            "relay-trigger-batch-preferences-{}",
            Uuid::new_v4()
        ));
        let store = CodexControlStore::new(root.join("codex-control")).expect("control store");
        assert_eq!(
            effective_agent_trigger_batch_size(&store, 8).expect("fallback batch size"),
            8
        );
        store
            .save_agent_trigger_preferences(27, Uuid::new_v4(), 8)
            .expect("managed batch size");
        assert_eq!(
            effective_agent_trigger_batch_size(&store, 8).expect("managed batch size"),
            27
        );
        fs::remove_dir_all(root).expect("cleanup");
    }
}
