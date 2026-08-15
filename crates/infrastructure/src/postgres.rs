use chrono::{DateTime, Utc};
use postgres::types::Json;
use postgres::{Client, GenericClient, NoTls, Row};
use r2d2::{Pool, PooledConnection};
use r2d2_postgres::PostgresConnectionManager;
use serde_json::Value;
use std::{
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration as StdDuration, Instant},
};
use tokio::runtime::Handle;
use uuid::Uuid;

use ai_chat_application::{
    AgentPlatformRepository, AgentStaffingHireBundle, AgentStaffingStatusChangeBundle,
    AuthPlatformRepository, ChatPlatformRepository, CodexControlPlatformRepository,
    CodexRuntimePlatformRepository, CompanyAgentActivationBundle, CompanyAgentCreationBundle,
    CompanyAgentMembershipUpdateBundle, CompanyConversationCreationBundle, CompanyCreationBundle,
    CompanyPlatformRepository, CompanyProjectCreationBundle, CompanyProjectMemberAddBundle,
    CompanyProjectOwnerTransferBundle, CompleteAgentCodexTriggerLeaseInput, CursorPage,
    EnvironmentPlatformRepository, ExecutionPlatformRepository, GatePlatformRepository,
    GovernancePlatformRepository, HumanCompanyDirectConversationCreationBundle,
    ManagedCompanyProjectCreationBundle, MemoryPlatformRepositoryPort, MessagePageView,
    ProjectDiscussionThreadCreationBundle, ProjectPlatformRepository,
    ProjectProvisioningCleanupJob, RegistrationCompletionBundle, TaskPlatformRepository,
};
use ai_chat_domain::agent_identity::{
    AgentActionLog, AgentActionStatus, AgentIdempotencyRecord, AgentInboxEvent,
    AgentInboxEventStatus, AgentKeyIssueLog, AgentKeyIssueType, AgentKeyRecord, AgentOwnerBinding,
    AgentProfile, AgentRegistrationRequest, AgentStatus, ChallengeStatus, HumanAccountToken,
    HumanCredential, HumanHarnessAccount, HumanSession, HumanUser, OwnershipProofChallenge,
    OwnershipProofProvider, RegistrationStatus, SocialProofSubmission,
};
use ai_chat_domain::company::{
    AgentCodexRunActivity, AgentCodexRunToken, AgentCodexSession, AgentCodexTriggerConfig,
    AgentCodexTriggerRun, AgentExecutionIntent, AgentMemory, AgentMemorySourceRef,
    AgentStaffingAction, AgentToolApprovalRequest, CodexPluginCatalogSnapshot,
    CodexPluginOperation, Company, CompanyAgentMembership, CompanyCodexRunnerProfile,
    CompanyGovernancePolicySettings, CompanyGovernancePolicyVersion, CompanyHumanMember,
    CompanyProject, CompanyProjectAsset, CompanyProjectAssetRefreshConfig, CompanyProjectGitConfig,
    CompanyProjectMember, CompanyProjectRule, CompanyProjectStatusUpdate, CompanyProjectTask,
    CompanyProjectTaskDependency, CompanyProjectTaskStatusHistory, CompanyRealtimeEvent, OrgUnit,
    ProjectDiscussionThread, ProjectEnvironment, ProjectEnvironmentService, ProjectEvidence,
    ProjectGate, ProjectMemberEventSubscription, ProjectTaskAttempt, ProjectTaskBlocker,
    ProjectTaskEnvironmentRequirement, ProjectTaskGateRequirement, ProjectTaskRelation,
    COMPANY_AGENT_ROLE_MANAGER,
};
use ai_chat_domain::social::{
    ConversationContext, ConversationPreview, ConversationType, MessageView,
};
use ai_chat_shared::{hash_secret, AppError, AppResult};

mod agent;
mod auth;
mod chat;
mod codex_control;
mod codex_runtime;
mod codex_runtime_schedule;
mod codex_runtime_tokens;
mod company;
mod environment;
mod execution;
mod gate;
mod governance;
mod mapping;
mod memory;
mod project;
mod project_persistence;
mod task;

#[cfg(test)]
mod postgres_tests;

use self::mapping::map_postgres_error;

pub struct PostgresPlatformRepository {
    pool: Arc<PostgresPool>,
}

struct PostgresPool {
    inner: Option<Pool<PostgresConnectionManager<NoTls>>>,
    application_name: String,
    waiting_checkouts: AtomicUsize,
    checkout_timeouts: AtomicU64,
}

impl Drop for PostgresPool {
    fn drop(&mut self) {
        let Some(pool) = self.inner.take() else {
            return;
        };
        if Handle::try_current().is_ok() {
            let _ = std::thread::spawn(move || drop(pool)).join();
        } else {
            drop(pool);
        }
    }
}

impl Clone for PostgresPlatformRepository {
    fn clone(&self) -> Self {
        Self {
            pool: Arc::clone(&self.pool),
        }
    }
}

impl PostgresPlatformRepository {
    pub fn connect(database_url: &str) -> anyhow::Result<Self> {
        Self::connect_named(database_url, "relay")
    }

    pub fn connect_named(database_url: &str, application_name: &str) -> anyhow::Result<Self> {
        let postgres_config = named_postgres_config(database_url, application_name)?;
        let manager = PostgresConnectionManager::new(postgres_config, NoTls);
        let (max_size, min_idle) = database_pool_sizes(
            std::env::var("DATABASE_POOL_SIZE").ok().as_deref(),
            std::env::var("DATABASE_POOL_MIN_IDLE").ok().as_deref(),
        );
        let inner = run_sync_postgres(|| {
            Pool::builder()
                .max_size(max_size)
                .min_idle(Some(min_idle))
                .build(manager)
                .map_err(|error| AppError::Internal(format!("postgres pool error: {error}")))
        })
        .map_err(anyhow::Error::msg)?;
        let pool = Arc::new(PostgresPool {
            inner: Some(inner),
            application_name: application_name.to_string(),
            waiting_checkouts: AtomicUsize::new(0),
            checkout_timeouts: AtomicU64::new(0),
        });
        run_sync_postgres(|| {
            let mut client = pool
                .checkout()
                .map_err(|error| AppError::Internal(format!("postgres pool error: {error}")))?;
            client
                .simple_query("SELECT 1")
                .map_err(map_postgres_error)?;
            Ok(())
        })
        .map_err(anyhow::Error::msg)?;
        pool.log_state("PostgreSQL connection pool ready");
        Ok(Self { pool })
    }

    fn with_client<T: Send>(
        &self,
        f: impl FnOnce(&mut Client) -> Result<T, postgres::Error> + Send,
    ) -> AppResult<T> {
        run_sync_postgres(|| {
            let mut client = self
                .pool
                .checkout()
                .map_err(|error| AppError::Internal(format!("postgres pool error: {error}")))?;
            f(&mut client).map_err(map_postgres_error)
        })
    }

    fn with_transaction<T: Send>(
        &self,
        f: impl FnOnce(&mut postgres::Transaction<'_>) -> AppResult<T> + Send,
    ) -> AppResult<T> {
        run_sync_postgres(|| {
            let mut client = self
                .pool
                .checkout()
                .map_err(|error| AppError::Internal(format!("postgres pool error: {error}")))?;
            let mut transaction = client.transaction().map_err(map_postgres_error)?;
            let output = f(&mut transaction)?;
            transaction.commit().map_err(map_postgres_error)?;
            Ok(output)
        })
    }
}

impl PostgresPool {
    fn checkout(&self) -> Result<PooledConnection<PostgresConnectionManager<NoTls>>, r2d2::Error> {
        let pool = self
            .inner
            .as_ref()
            .expect("postgres pool is available while repository is alive");
        self.waiting_checkouts.fetch_add(1, Ordering::Relaxed);
        let started = Instant::now();
        let result = pool.get();
        self.waiting_checkouts.fetch_sub(1, Ordering::Relaxed);
        let elapsed = started.elapsed();
        if result.is_err() {
            self.checkout_timeouts.fetch_add(1, Ordering::Relaxed);
            self.log_state("PostgreSQL connection checkout timed out");
        } else if elapsed >= StdDuration::from_millis(250) {
            self.log_state("PostgreSQL connection checkout was slow");
        }
        result
    }

    fn log_state(&self, message: &'static str) {
        let pool = self
            .inner
            .as_ref()
            .expect("postgres pool is available while repository is alive");
        let state = pool.state();
        tracing::info!(
            observation = message,
            application_name = %self.application_name,
            connections = state.connections,
            idle_connections = state.idle_connections,
            waiting_checkouts = self.waiting_checkouts.load(Ordering::Relaxed),
            checkout_timeouts = self.checkout_timeouts.load(Ordering::Relaxed),
            max_connections = pool.max_size(),
            min_idle_connections = pool.min_idle(),
            "PostgreSQL connection pool state"
        );
    }
}

pub(crate) fn named_postgres_config(
    database_url: &str,
    application_name: &str,
) -> anyhow::Result<postgres::Config> {
    if application_name.is_empty()
        || application_name.len() > 63
        || application_name.chars().any(char::is_control)
    {
        anyhow::bail!("PostgreSQL application_name is invalid");
    }
    let mut config = database_url.parse::<postgres::Config>()?;
    config.application_name(application_name);
    Ok(config)
}

fn database_pool_sizes(max_size: Option<&str>, min_idle: Option<&str>) -> (u32, u32) {
    let max_size = max_size
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(16);
    let min_idle = min_idle
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1)
        .min(max_size);
    (max_size, min_idle)
}

fn run_sync_postgres<T: Send>(f: impl FnOnce() -> AppResult<T> + Send) -> AppResult<T> {
    if Handle::try_current().is_err() {
        return f();
    }
    std::thread::scope(|scope| {
        scope
            .spawn(f)
            .join()
            .map_err(|_| AppError::Internal("postgres worker thread panicked".into()))?
    })
}
