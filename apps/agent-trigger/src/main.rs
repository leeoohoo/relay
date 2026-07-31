use std::{
    collections::HashSet,
    fs,
    panic::{catch_unwind, AssertUnwindSafe},
    path::Path,
    sync::Arc,
    time::Duration as StdDuration,
};

use async_trait::async_trait;
use chrono::Duration;
use futures_util::{stream::FuturesUnordered, StreamExt};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

use ai_chat_application::{
    CompleteAgentCodexTriggerLeaseInput, CreateCodexApprovalRequestInput, PlatformApp,
};
use ai_chat_domain::{
    agent_identity::AgentProfile,
    company::{
        infer_company_profession, AgentCodexRunActivity, AgentCodexSession,
        AgentCodexTriggerConfig, AgentCodexTriggerRun, AGENT_CODEX_RUN_STATUS_FAILED,
        AGENT_CODEX_RUN_STATUS_RUNNING, AGENT_CODEX_RUN_STATUS_SUCCEEDED,
        AGENT_CODEX_RUN_STATUS_TIMED_OUT, AGENT_TOOL_APPROVAL_STATUS_APPROVED,
        AGENT_TOOL_APPROVAL_STATUS_EXECUTED, AGENT_TOOL_APPROVAL_STATUS_EXPIRED,
        AGENT_TOOL_APPROVAL_STATUS_FAILED, AGENT_TOOL_APPROVAL_STATUS_REJECTED,
        COMPANY_PROFESSION_BACKEND_ENGINEER, COMPANY_PROFESSION_BUSINESS_ANALYST,
        COMPANY_PROFESSION_DATA_ENGINEER, COMPANY_PROFESSION_DEVOPS_ENGINEER,
        COMPANY_PROFESSION_DOMAIN_EXPERT, COMPANY_PROFESSION_FRONTEND_ENGINEER,
        COMPANY_PROFESSION_GENERAL_MEMBER, COMPANY_PROFESSION_IMPLEMENTATION_CONSULTANT,
        COMPANY_PROFESSION_MOBILE_ENGINEER, COMPANY_PROFESSION_OPERATIONS_SPECIALIST,
        COMPANY_PROFESSION_PRODUCT_DESIGNER, COMPANY_PROFESSION_PRODUCT_MANAGER,
        COMPANY_PROFESSION_PROJECT_MANAGER, COMPANY_PROFESSION_QA_ENGINEER,
        COMPANY_PROFESSION_SOFTWARE_ENGINEER, COMPANY_PROFESSION_SOLUTION_ARCHITECT,
        COMPANY_PROFESSION_TECHNICAL_MANAGER, COMPANY_PROFESSION_UI_DESIGNER,
        COMPANY_PROFESSION_UX_DESIGNER,
    },
};
use ai_chat_infrastructure::{
    codex_trigger::{
        CodexApprovalDecision, CodexApprovalHandler, CodexApprovalRequest, CodexProgressEvent,
        CodexProgressHandler, CodexRunRequest, CodexRunStatus, CodexTriggerRunner,
    },
    config::ApiConfig,
    git_workspace::{GitWorkspaceManager, PreparedGitWorkspace},
    RepositoryAdapter,
};
use ai_chat_shared::{hash_secret, now_utc, AppError, AppResult};

type TriggerPlatform = PlatformApp<RepositoryAdapter>;
const CODEX_SESSION_POLICY_VERSION: &str = "relay-skills-v4";
const EMPLOYEE_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-company-employee/SKILL.md");
const STAFFING_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-company-staffing-manager/SKILL.md");
const PROJECT_MANAGER_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-project-manager/SKILL.md");
const PRODUCT_MANAGER_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-product-manager/SKILL.md");
const TECHNICAL_MANAGER_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-technical-manager/SKILL.md");
const SOLUTION_ARCHITECT_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-solution-architect/SKILL.md");
const SOFTWARE_ENGINEER_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-software-engineer/SKILL.md");
const FRONTEND_ENGINEER_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-frontend-engineer/SKILL.md");
const BACKEND_ENGINEER_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-backend-engineer/SKILL.md");
const MOBILE_ENGINEER_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-mobile-engineer/SKILL.md");
const DATA_ENGINEER_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-data-engineer/SKILL.md");
const DEVOPS_ENGINEER_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-devops-engineer/SKILL.md");
const QA_ENGINEER_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-qa-engineer/SKILL.md");
const PRODUCT_DESIGNER_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-product-designer/SKILL.md");
const UI_DESIGNER_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-ui-designer/SKILL.md");
const UX_DESIGNER_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-ux-designer/SKILL.md");
const BUSINESS_ANALYST_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-business-analyst/SKILL.md");
const IMPLEMENTATION_CONSULTANT_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-implementation-consultant/SKILL.md");
const DOMAIN_EXPERT_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-domain-expert/SKILL.md");
const OPERATIONS_SPECIALIST_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-operations-specialist/SKILL.md");
const GENERAL_MEMBER_SKILL_TEMPLATE: &str =
    include_str!("../../../skills/relay-profession-general-member/SKILL.md");

#[derive(Debug, Clone)]
struct TriggerServiceConfig {
    lease_owner: String,
    poll_interval: StdDuration,
    batch_size: usize,
    run_once: bool,
}

#[derive(Debug)]
struct TriggerExecution {
    succeeded: bool,
    error_message: Option<String>,
}

#[derive(Debug)]
struct PreparedRelaySkills {
    employee_name: String,
    profession_name: String,
    staffing_name: Option<String>,
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
    let platform = PlatformApp::new(RepositoryAdapter::build(&api_config)?);
    let workspace_manager = GitWorkspaceManager::from_env()?;
    let codex_runner = CodexTriggerRunner::from_env()?;
    let config = TriggerServiceConfig::from_env()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run_trigger_loop(
        &platform,
        &workspace_manager,
        &codex_runner,
        &config,
    ))
}

async fn run_trigger_loop(
    platform: &TriggerPlatform,
    workspace_manager: &GitWorkspaceManager,
    codex_runner: &CodexTriggerRunner,
    config: &TriggerServiceConfig,
) -> anyhow::Result<()> {
    tracing::info!(
        lease_owner = %config.lease_owner,
        poll_interval_seconds = config.poll_interval.as_secs(),
        batch_size = config.batch_size,
        "local Codex Agent Trigger started"
    );

    let mut running = FuturesUnordered::new();
    loop {
        let available_slots = config.batch_size.saturating_sub(running.len());
        if available_slots > 0 {
            let claimed = platform
                .claim_due_agent_codex_triggers(&config.lease_owner, available_slots)
                .map_err(anyhow::Error::msg)?;
            for trigger in claimed {
                running.push(process_claimed_trigger(
                    platform,
                    workspace_manager,
                    codex_runner,
                    config,
                    trigger,
                ));
            }
        }

        if config.run_once {
            while running.next().await.is_some() {}
            break;
        }

        if running.is_empty() {
            tokio::time::sleep(config.poll_interval).await;
        } else {
            tokio::select! {
                _ = tokio::time::sleep(config.poll_interval) => {}
                _ = running.next() => {}
            }
        }
    }
    Ok(())
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
        let poll_interval_seconds = std::env::var("AGENT_TRIGGER_POLL_INTERVAL_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map(|value| value.clamp(1, 60))
            .unwrap_or(2);
        let batch_size = std::env::var("AGENT_TRIGGER_BATCH_SIZE")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .map(|value| value.clamp(1, 100))
            .unwrap_or(10);
        let run_once = bool_env("AGENT_TRIGGER_RUN_ONCE", false);
        Ok(Self {
            lease_owner: format!("{host}:{instance}"),
            poll_interval: StdDuration::from_secs(poll_interval_seconds),
            batch_size,
            run_once,
        })
    }
}

async fn process_claimed_trigger(
    platform: &TriggerPlatform,
    workspace_manager: &GitWorkspaceManager,
    codex_runner: &CodexTriggerRunner,
    service_config: &TriggerServiceConfig,
    trigger: AgentCodexTriggerConfig,
) {
    let execution = execute_trigger(platform, workspace_manager, codex_runner, &trigger).await;
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
    trigger: &AgentCodexTriggerConfig,
) -> AppResult<TriggerExecution> {
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
    let relay_skills = prepare_relay_skills(
        &workspace.path,
        &agent,
        &membership.job_title,
        &membership.permissions,
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
    let session_key = codex_session_key(&workspace, &relay_skills.version_hash);
    let existing_thread_id = platform
        .get_agent_codex_session(trigger.agent_profile_id)
        .and_then(|session| {
            if session.worktree_key == session_key {
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
        model: trigger.model.clone(),
        reasoning_effort: trigger.reasoning_effort.clone(),
        sandbox_mode: trigger.sandbox_mode.clone(),
        approval_policy: trigger.approval_policy.clone(),
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
        approval_handler: (trigger.approval_policy == "on-request").then(|| {
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
    };
    record_run_activity(
        platform,
        run.id,
        final_phase,
        final_summary,
        run.codex_thread_id.clone(),
    );
    Ok(TriggerExecution {
        succeeded: result.status == CodexRunStatus::Succeeded,
        error_message: result.error_message,
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

fn codex_session_key(workspace: &PreparedGitWorkspace, skill_version_hash: &str) -> String {
    format!(
        "{CODEX_SESSION_POLICY_VERSION}:{}:{}",
        workspace.worktree_key, skill_version_hash
    )
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
    format!(
        "你是 Relay 公司 Agent @{handle}（{display_name}），这是定时触发器对同一个 Codex 会话的一次唤醒。{project_context}\n\
         当前工作目录是本次分配的隔离工作区，worktree key 为 {worktree_key}，当前 Agent 分支为 {branch}。触发器只负责唤醒，不会替你理解或处理业务。\n\
         本工作区已经生成与你当前身份、职业和权限一致的最新版 Relay Skill。必须先使用 `${employee_skill}` 和 `${profession_skill}`{staffing_skill}；Skill 与 MCP 返回的实时权限冲突时，以 MCP 权限和项目 Rule 为准。\n\
         请先调用 required Relay MCP 的 agent.bootstrap，再调用 company.task 的 my 区分可执行任务和等待前置任务，然后调用 agent.inbox.wait（不要无限等待）读取真实待办；当前快速检查发现 pending inbox {pending_inbox_count} 条、可执行 assigned tasks {active_task_count} 个、等待前置 tasks {waiting_task_count} 个。\n\
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
    )
}

fn prepare_relay_skills(
    workspace_path: &Path,
    agent: &AgentProfile,
    job_title: &str,
    permissions: &[String],
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

    let employee_content = bind_relay_skill(
        &tailor_relay_skill_to_permissions(EMPLOYEE_SKILL_TEMPLATE, permissions),
        &employee_name,
        agent,
        &employee_name,
    );
    let profession_content = bind_relay_skill(
        profession_skill_template(&profession.key),
        &profession_name,
        agent,
        &employee_name,
    );
    let staffing_content = staffing_name.as_ref().map(|name| {
        bind_relay_skill(
            &tailor_relay_skill_to_permissions(STAFFING_SKILL_TEMPLATE, permissions),
            name,
            agent,
            &employee_name,
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
    exclude_managed_skills_from_git(workspace_path, &managed_prefix)?;

    let version_source = format!(
        "{employee_name}\n{employee_content}\n{profession_name}\n{profession_content}\n{}\n{}",
        staffing_name.as_deref().unwrap_or_default(),
        staffing_content.as_deref().unwrap_or_default()
    );
    let version_hash = hash_secret(&version_source).chars().take(16).collect();
    Ok(PreparedRelaySkills {
        employee_name,
        profession_name,
        staffing_name,
        version_hash,
    })
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

fn profession_skill_template(profession_key: &str) -> &'static str {
    match profession_key {
        COMPANY_PROFESSION_PROJECT_MANAGER => PROJECT_MANAGER_SKILL_TEMPLATE,
        COMPANY_PROFESSION_PRODUCT_MANAGER => PRODUCT_MANAGER_SKILL_TEMPLATE,
        COMPANY_PROFESSION_TECHNICAL_MANAGER => TECHNICAL_MANAGER_SKILL_TEMPLATE,
        COMPANY_PROFESSION_SOLUTION_ARCHITECT => SOLUTION_ARCHITECT_SKILL_TEMPLATE,
        COMPANY_PROFESSION_SOFTWARE_ENGINEER => SOFTWARE_ENGINEER_SKILL_TEMPLATE,
        COMPANY_PROFESSION_FRONTEND_ENGINEER => FRONTEND_ENGINEER_SKILL_TEMPLATE,
        COMPANY_PROFESSION_BACKEND_ENGINEER => BACKEND_ENGINEER_SKILL_TEMPLATE,
        COMPANY_PROFESSION_MOBILE_ENGINEER => MOBILE_ENGINEER_SKILL_TEMPLATE,
        COMPANY_PROFESSION_DATA_ENGINEER => DATA_ENGINEER_SKILL_TEMPLATE,
        COMPANY_PROFESSION_DEVOPS_ENGINEER => DEVOPS_ENGINEER_SKILL_TEMPLATE,
        COMPANY_PROFESSION_QA_ENGINEER => QA_ENGINEER_SKILL_TEMPLATE,
        COMPANY_PROFESSION_PRODUCT_DESIGNER => PRODUCT_DESIGNER_SKILL_TEMPLATE,
        COMPANY_PROFESSION_UI_DESIGNER => UI_DESIGNER_SKILL_TEMPLATE,
        COMPANY_PROFESSION_UX_DESIGNER => UX_DESIGNER_SKILL_TEMPLATE,
        COMPANY_PROFESSION_BUSINESS_ANALYST => BUSINESS_ANALYST_SKILL_TEMPLATE,
        COMPANY_PROFESSION_IMPLEMENTATION_CONSULTANT => IMPLEMENTATION_CONSULTANT_SKILL_TEMPLATE,
        COMPANY_PROFESSION_DOMAIN_EXPERT => DOMAIN_EXPERT_SKILL_TEMPLATE,
        COMPANY_PROFESSION_OPERATIONS_SPECIALIST => OPERATIONS_SPECIALIST_SKILL_TEMPLATE,
        COMPANY_PROFESSION_GENERAL_MEMBER => GENERAL_MEMBER_SKILL_TEMPLATE,
        _ => GENERAL_MEMBER_SKILL_TEMPLATE,
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
    let identity_guide = format!(
        "\n\n## Relay 账号绑定\n\n- 本 Skill 只代表 Relay Agent `@{}`（`{}`）。\n- 每轮先调用 `agent.bootstrap` 核对返回身份；身份不一致时立即停止。\n- 只使用本轮 MCP 返回的公司、项目、任务和权限。",
        agent.handle.trim_start_matches('@'),
        agent.id
    );
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
    fn session_key_includes_execution_policy_and_skill_version() {
        let workspace = PreparedGitWorkspace {
            path: PathBuf::from("/tmp/relay-agent"),
            worktree_key: "project/agent".into(),
            branch: "relay/agent/inbox".into(),
            auth_environment: HashMap::new(),
        };
        assert_eq!(
            codex_session_key(&workspace, "skill123"),
            "relay-skills-v4:project/agent:skill123"
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
        let prepared =
            prepare_relay_skills(&workspace, &agent, "软件工程师", &["task.update".into()])
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
            sandbox_mode: "workspace_write".into(),
            approval_policy: "never".into(),
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
}
