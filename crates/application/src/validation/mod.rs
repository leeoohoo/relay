mod agent;
mod auth;
mod codex;
mod company;
mod git;
mod memory;
mod staffing;
mod task;

pub(crate) use agent::*;
pub(crate) use auth::*;
pub(crate) use codex::*;
pub(crate) use company::*;
pub(crate) use git::*;
pub(crate) use memory::*;
pub(crate) use staffing::*;
pub(crate) use task::*;

use std::{
    collections::{HashMap, HashSet},
    path::{Component, Path},
};

use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use url::Url;
use uuid::Uuid;

use ai_chat_domain::agent_identity::{
    AgentKeyRecord, AGENT_COLLABORATION_PREFERENCE_AVAILABLE,
    AGENT_COLLABORATION_PREFERENCE_LOW_COST_ONLY, AGENT_COLLABORATION_PREFERENCE_UNAVAILABLE,
};
use ai_chat_domain::company::{
    AgentMemorySourceRef, CompanyGovernancePolicySettings, CompanyProjectGitConfig,
    CompanyProjectTaskDependency, OrgUnit, AGENT_MEMORY_SCOPE_AGENT, AGENT_MEMORY_SCOPE_CONTROL,
    AGENT_MEMORY_SCOPE_PROJECT, AGENT_MEMORY_SCOPE_SESSION, AGENT_MEMORY_STATUS_ACTIVE,
    AGENT_MEMORY_STATUS_ARCHIVED, AGENT_MEMORY_STATUS_DRAFT, AGENT_MEMORY_STATUS_SUPERSEDED,
    AGENT_MEMORY_TIER_LONG_TERM, AGENT_MEMORY_TIER_SHORT_TERM, AGENT_TOOL_APPROVAL_STATUS_APPROVED,
    AGENT_TOOL_APPROVAL_STATUS_EXECUTED, AGENT_TOOL_APPROVAL_STATUS_EXECUTING,
    AGENT_TOOL_APPROVAL_STATUS_EXPIRED, AGENT_TOOL_APPROVAL_STATUS_FAILED,
    AGENT_TOOL_APPROVAL_STATUS_PENDING, AGENT_TOOL_APPROVAL_STATUS_REJECTED,
    COMPANY_AGENT_ROLE_MANAGER, PROJECT_STATUS_ACTIVE, PROJECT_STATUS_BLOCKED,
    PROJECT_STATUS_CANCELLED, PROJECT_STATUS_COMPLETED, PROJECT_STATUS_PLANNED,
    PROJECT_TASK_PRIORITY_HIGH, PROJECT_TASK_PRIORITY_LOW, PROJECT_TASK_PRIORITY_NORMAL,
    PROJECT_TASK_PRIORITY_URGENT, PROJECT_TASK_STATUS_BLOCKED, PROJECT_TASK_STATUS_CANCELLED,
    PROJECT_TASK_STATUS_DONE, PROJECT_TASK_STATUS_FAILED, PROJECT_TASK_STATUS_IN_PROGRESS,
    PROJECT_TASK_STATUS_TODO,
};
use ai_chat_domain::social::{
    ConversationContext, CONVERSATION_CONTEXT_COMPANY_ALL, CONVERSATION_CONTEXT_COMPANY_GROUP,
    CONVERSATION_CONTEXT_PROJECT_GROUP,
};
use ai_chat_shared::{hash_secret, now_utc, AppError, AppResult};

use crate::contracts::{
    CompanyGovernancePolicyView, CompanyProjectGitAdminView, CompanyProjectGitView,
};

const MAX_MESSAGE_PAGE_LIMIT: usize = 100;
