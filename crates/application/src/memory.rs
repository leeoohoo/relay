use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc, LockResult, Mutex, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use ai_chat_domain::agent_identity::{
    AgentActionLog, AgentIdempotencyRecord, AgentInboxEvent, AgentInboxEventStatus,
    AgentKeyIssueLog, AgentKeyRecord, AgentOwnerBinding, AgentProfile, AgentRegistrationRequest,
    AgentStatus, HumanAccountToken, HumanCredential, HumanHarnessAccount, HumanSession, HumanUser,
    OwnershipProofChallenge, SocialProofSubmission,
};
use ai_chat_domain::company::{
    AgentCodexRunActivity, AgentCodexRunToken, AgentCodexSession, AgentCodexTriggerConfig,
    AgentCodexTriggerRun, AgentExecutionIntent, AgentMemory, AgentStaffingAction,
    AgentToolApprovalRequest, CodexPluginCatalogSnapshot, CodexPluginOperation, Company,
    CompanyAgentMembership, CompanyCodexRunnerProfile, CompanyGovernancePolicyVersion,
    CompanyHumanMember, CompanyProject, CompanyProjectAsset, CompanyProjectAssetRefreshConfig,
    CompanyProjectGitConfig, CompanyProjectMember, CompanyProjectRule, CompanyProjectStatusUpdate,
    CompanyProjectTask, CompanyProjectTaskDependency, CompanyProjectTaskStatusHistory, OrgUnit,
    ProjectGate, ProjectTaskGateRequirement, AGENT_CODEX_RUN_STATUS_LEASE_LOST,
    AGENT_CODEX_RUN_STATUS_RUNNING, AGENT_CODEX_TRIGGER_STATUS_ACTIVE,
    AGENT_CODEX_TRIGGER_STATUS_ERROR, AGENT_EXECUTION_INTENT_STATUS_PENDING,
    AGENT_EXECUTION_INTENT_STATUS_RUNNING, AGENT_TOOL_APPROVAL_MODE_ALWAYS,
    AGENT_TOOL_APPROVAL_SOURCE_CODEX, AGENT_TOOL_APPROVAL_STATUS_APPROVED,
    AGENT_TOOL_APPROVAL_STATUS_EXECUTED, AGENT_TOOL_APPROVAL_STATUS_PENDING,
    CODEX_PLUGIN_OPERATION_STATUS_FAILED, CODEX_PLUGIN_OPERATION_STATUS_QUEUED,
    CODEX_PLUGIN_OPERATION_STATUS_RUNNING, CODEX_PLUGIN_OPERATION_STATUS_SUCCEEDED,
    COMPANY_GOVERNANCE_POLICY_STATUS_ACTIVE, COMPANY_GOVERNANCE_POLICY_STATUS_ARCHIVED,
    PROJECT_STATUS_PAUSED,
};
use ai_chat_domain::social::{
    ConversationContext, ConversationPreview, MessageView, CONVERSATION_CONTEXT_PROJECT_GROUP,
    CONVERSATION_CONTEXT_SELF_NOTES,
};
use ai_chat_shared::{hash_secret, now_utc, AppError, AppResult};

use crate::contracts::ProjectProvisioningCleanupJob;
use crate::service::{
    AgentPlatformRepository, AgentStaffingHireBundle, AgentStaffingStatusChangeBundle,
    AuthPlatformRepository, ChatPlatformRepository, CodexControlPlatformRepository,
    CodexRuntimePlatformRepository, CompanyAgentActivationBundle, CompanyAgentCreationBundle,
    CompanyAgentMembershipUpdateBundle, CompanyConversationCreationBundle, CompanyCreationBundle,
    CompanyPlatformRepository, CompanyProjectCreationBundle, CompanyProjectMemberAddBundle,
    CompanyProjectOwnerTransferBundle, CompleteAgentCodexTriggerLeaseInput, GatePlatformRepository,
    GovernancePlatformRepository, HumanCompanyDirectConversationCreationBundle,
    ManagedCompanyProjectCreationBundle, MemoryPlatformRepositoryPort, ProjectPlatformRepository,
    RegistrationCompletionBundle, TaskPlatformRepository,
};

#[derive(Default, Serialize, Deserialize)]
struct MemoryState {
    human_users: HashMap<Uuid, HumanUser>,
    human_credentials: HashMap<Uuid, HumanCredential>,
    human_harness_accounts: HashMap<Uuid, HumanHarnessAccount>,
    human_sessions: HashMap<Uuid, HumanSession>,
    human_account_tokens: HashMap<Uuid, HumanAccountToken>,
    human_email_verifications: HashMap<Uuid, chrono::DateTime<chrono::Utc>>,
    companies: HashMap<Uuid, Company>,
    company_human_members: HashMap<(Uuid, Uuid), CompanyHumanMember>,
    org_units: HashMap<Uuid, OrgUnit>,
    company_agent_memberships: HashMap<Uuid, CompanyAgentMembership>,
    agent_staffing_actions: HashMap<Uuid, AgentStaffingAction>,
    agent_profiles: HashMap<Uuid, AgentProfile>,
    owner_bindings: Vec<AgentOwnerBinding>,
    registration_requests: HashMap<Uuid, AgentRegistrationRequest>,
    challenges: HashMap<Uuid, OwnershipProofChallenge>,
    submissions: Vec<SocialProofSubmission>,
    inbox_events: HashMap<Uuid, AgentInboxEvent>,
    action_logs: Vec<AgentActionLog>,
    idempotency_records: HashMap<(Uuid, String, String), AgentIdempotencyRecord>,
    agent_key_issue_logs: Vec<AgentKeyIssueLog>,
    agent_keys: HashMap<Uuid, AgentKeyRecord>,
    agent_keys_by_hash: HashMap<String, Uuid>,
    conversations: HashMap<Uuid, Vec<ConversationPreview>>,
    messages: HashMap<Uuid, Vec<MessageView>>,
    direct_conversations: HashMap<(Uuid, Uuid), Uuid>,
    company_direct_conversations: HashMap<(Uuid, Uuid, Uuid), Uuid>,
    company_human_direct_conversations: HashMap<(Uuid, Uuid, Uuid), Uuid>,
    company_default_groups: HashMap<Uuid, ConversationPreview>,
    conversation_contexts: HashMap<Uuid, ConversationContext>,
    company_projects: HashMap<Uuid, CompanyProject>,
    project_provisioning_cleanup_jobs: HashMap<Uuid, ProjectProvisioningCleanupJob>,
    company_project_git_configs: HashMap<Uuid, CompanyProjectGitConfig>,
    company_project_rules: HashMap<Uuid, CompanyProjectRule>,
    company_project_assets: HashMap<Uuid, Vec<CompanyProjectAsset>>,
    agent_memories: HashMap<Uuid, AgentMemory>,
    company_project_asset_refresh_configs: HashMap<Uuid, CompanyProjectAssetRefreshConfig>,
    company_codex_runner_profiles: HashMap<Uuid, CompanyCodexRunnerProfile>,
    agent_codex_runner_profile_assignments: HashMap<Uuid, Uuid>,
    agent_codex_trigger_configs: HashMap<Uuid, AgentCodexTriggerConfig>,
    agent_codex_trigger_runs: HashMap<Uuid, AgentCodexTriggerRun>,
    agent_codex_sessions: HashMap<(Uuid, String), AgentCodexSession>,
    agent_execution_intents: HashMap<Uuid, AgentExecutionIntent>,
    agent_codex_run_tokens: HashMap<String, AgentCodexRunToken>,
    codex_plugin_catalogs: HashMap<String, CodexPluginCatalogSnapshot>,
    codex_plugin_operations: HashMap<Uuid, CodexPluginOperation>,
    company_project_members: HashMap<(Uuid, Uuid), CompanyProjectMember>,
    company_project_tasks: HashMap<Uuid, CompanyProjectTask>,
    company_project_task_dependencies: HashMap<Uuid, CompanyProjectTaskDependency>,
    company_project_task_status_history: Vec<CompanyProjectTaskStatusHistory>,
    company_project_status_updates: HashMap<Uuid, Vec<CompanyProjectStatusUpdate>>,
    project_gates: HashMap<Uuid, ProjectGate>,
    project_task_gate_requirements: HashMap<(Uuid, Uuid), ProjectTaskGateRequirement>,
    company_governance_policy_versions: HashMap<Uuid, CompanyGovernancePolicyVersion>,
    agent_tool_approval_requests: HashMap<Uuid, AgentToolApprovalRequest>,
}

pub trait MemorySnapshotWriteLease: Send {
    fn load(&mut self) -> AppResult<Option<(i64, Vec<u8>)>>;
    fn save(&mut self, expected_revision: i64, payload: &[u8]) -> AppResult<i64>;
}

pub trait MemorySnapshotPersistence: Send + Sync {
    fn health_check(&self) -> AppResult<()>;
    fn load_if_newer(&self, current_revision: i64) -> AppResult<Option<(i64, Vec<u8>)>>;
    fn begin_write(&self) -> AppResult<Box<dyn MemorySnapshotWriteLease>>;
}

struct MemoryStateStore {
    state: RwLock<MemoryState>,
    persistence: Option<Arc<dyn MemorySnapshotPersistence>>,
    revision: AtomicI64,
    last_persistence_error: Mutex<Option<String>>,
}

impl Default for MemoryStateStore {
    fn default() -> Self {
        Self {
            state: RwLock::new(MemoryState::default()),
            persistence: None,
            revision: AtomicI64::new(0),
            last_persistence_error: Mutex::new(None),
        }
    }
}

impl MemoryStateStore {
    fn persistent(persistence: Arc<dyn MemorySnapshotPersistence>) -> AppResult<Self> {
        persistence.health_check()?;
        let initial = persistence.load_if_newer(-1)?;
        let (revision, state) = match initial {
            Some((revision, payload)) => (revision, decode_state(&payload)?),
            None => (0, MemoryState::default()),
        };
        Ok(Self {
            state: RwLock::new(state),
            persistence: Some(persistence),
            revision: AtomicI64::new(revision),
            last_persistence_error: Mutex::new(None),
        })
    }

    fn health_check(&self) -> AppResult<()> {
        if let Some(message) = self
            .last_persistence_error
            .lock()
            .expect("memory persistence error lock poisoned")
            .clone()
        {
            return Err(AppError::Internal(message));
        }
        if let Some(persistence) = &self.persistence {
            persistence.health_check()?;
        }
        Ok(())
    }

    fn read(&self) -> LockResult<RwLockReadGuard<'_, MemoryState>> {
        if let Some(persistence) = &self.persistence {
            let current_revision = self.revision.load(Ordering::Acquire);
            match persistence.load_if_newer(current_revision) {
                Ok(Some((revision, payload))) if revision > current_revision => {
                    let state = decode_state(&payload).unwrap_or_else(|error| {
                        panic!("failed to decode persisted repository snapshot: {error}")
                    });
                    let mut guard = self
                        .state
                        .write()
                        .expect("memory repository lock poisoned while refreshing persisted state");
                    if revision > self.revision.load(Ordering::Acquire) {
                        *guard = state;
                        self.revision.store(revision, Ordering::Release);
                    }
                }
                Ok(_) => {}
                Err(error) => self.fail_persistence("failed to refresh persisted state", error),
            }
        }
        self.state.read()
    }

    fn write(&self) -> LockResult<MemoryStateWriteGuard<'_>> {
        let mut lease = self.persistence.as_ref().map(|persistence| {
            persistence.begin_write().unwrap_or_else(|error| {
                self.fail_persistence("failed to acquire repository write lease", error)
            })
        });

        if let Some(active_lease) = lease.as_mut() {
            match active_lease.load() {
                Ok(Some((revision, payload))) => {
                    let state = decode_state(&payload).unwrap_or_else(|error| {
                        panic!("failed to decode persisted repository snapshot: {error}")
                    });
                    let mut guard = self
                        .state
                        .write()
                        .expect("memory repository lock poisoned while preparing persisted write");
                    if revision >= self.revision.load(Ordering::Acquire) {
                        *guard = state;
                        self.revision.store(revision, Ordering::Release);
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    self.fail_persistence("failed to load persisted state for write", error)
                }
            }
        }

        match self.state.write() {
            Ok(guard) => Ok(MemoryStateWriteGuard {
                guard,
                lease,
                revision: &self.revision,
                last_persistence_error: &self.last_persistence_error,
            }),
            Err(poisoned) => Err(PoisonError::new(MemoryStateWriteGuard {
                guard: poisoned.into_inner(),
                lease,
                revision: &self.revision,
                last_persistence_error: &self.last_persistence_error,
            })),
        }
    }

    fn fail_persistence(&self, context: &str, error: AppError) -> ! {
        let message = format!("{context}: {error}");
        *self
            .last_persistence_error
            .lock()
            .expect("memory persistence error lock poisoned") = Some(message.clone());
        panic!("{message}");
    }
}

struct MemoryStateWriteGuard<'a> {
    guard: RwLockWriteGuard<'a, MemoryState>,
    lease: Option<Box<dyn MemorySnapshotWriteLease>>,
    revision: &'a AtomicI64,
    last_persistence_error: &'a Mutex<Option<String>>,
}

impl Deref for MemoryStateWriteGuard<'_> {
    type Target = MemoryState;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl DerefMut for MemoryStateWriteGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

impl Drop for MemoryStateWriteGuard<'_> {
    fn drop(&mut self) {
        let Some(lease) = self.lease.as_mut() else {
            return;
        };
        let expected_revision = self.revision.load(Ordering::Acquire);
        let result = (|| {
            let mut payload = Vec::new();
            ciborium::ser::into_writer(&*self.guard, &mut payload).map_err(|error| {
                AppError::Internal(format!("failed to encode repository state: {error}"))
            })?;
            Ok(payload)
        })()
        .and_then(|payload| lease.save(expected_revision, &payload));
        match result {
            Ok(revision) => {
                self.revision.store(revision, Ordering::Release);
                *self
                    .last_persistence_error
                    .lock()
                    .expect("memory persistence error lock poisoned") = None;
            }
            Err(error) => {
                let message = format!("failed to persist repository snapshot: {error}");
                *self
                    .last_persistence_error
                    .lock()
                    .expect("memory persistence error lock poisoned") = Some(message.clone());
                if !std::thread::panicking() {
                    panic!("{message}");
                }
            }
        }
    }
}

fn decode_state(payload: &[u8]) -> AppResult<MemoryState> {
    ciborium::de::from_reader(payload)
        .map_err(|error| AppError::Internal(format!("failed to decode repository state: {error}")))
}

#[derive(Clone)]
pub struct MemoryPlatformRepository {
    inner: Arc<MemoryStateStore>,
}

impl Default for MemoryPlatformRepository {
    fn default() -> Self {
        Self {
            inner: Arc::new(MemoryStateStore::default()),
        }
    }
}

impl MemoryPlatformRepository {
    pub fn with_snapshot_persistence(
        persistence: Arc<dyn MemorySnapshotPersistence>,
    ) -> AppResult<Self> {
        Ok(Self {
            inner: Arc::new(MemoryStateStore::persistent(persistence)?),
        })
    }
}

mod agent;
mod auth;
mod chat;
mod codex;
mod company;
mod gate;
mod governance;
mod memories;
mod project;
mod task;

#[cfg(test)]
mod tests;
