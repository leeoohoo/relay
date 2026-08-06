use uuid::Uuid;

use ai_chat_domain::{
    agent_identity::{HumanAccountToken, HumanCredential, HumanSession, HumanUser},
    company::{
        AgentCodexTriggerConfig, AgentCodexTriggerRun, AgentMemory, Company, CompanyHumanMember,
        CompanyProject, CompanyProjectTask,
    },
    social::{ConversationContext, ConversationPreview},
};
use ai_chat_shared::AppResult;

use crate::{MessagePageView, PlatformRepository};

pub trait AuthRepository: Send + Sync {
    fn find_human_by_email(&self, email: &str) -> AppResult<Option<HumanUser>>;
    fn human_credential(&self, human_user_id: Uuid) -> AppResult<Option<HumanCredential>>;
    fn human_session_by_hash(&self, token_hash: &str) -> AppResult<Option<HumanSession>>;
    fn human_account_token_by_hash(&self, token_hash: &str)
        -> AppResult<Option<HumanAccountToken>>;
}

pub trait CompanyRepository: Send + Sync {
    fn company(&self, company_id: Uuid) -> AppResult<Option<Company>>;
    fn company_human_member(
        &self,
        company_id: Uuid,
        human_user_id: Uuid,
    ) -> AppResult<Option<CompanyHumanMember>>;
}

pub trait ChatRepository: Send + Sync {
    fn conversation_context(&self, conversation_id: Uuid)
        -> AppResult<Option<ConversationContext>>;
    fn company_conversations(&self, company_id: Uuid) -> AppResult<Vec<ConversationPreview>>;
    fn conversation_message_page(
        &self,
        conversation_id: Uuid,
        before_message_id: Option<Uuid>,
        limit: usize,
    ) -> AppResult<MessagePageView>;
}

pub trait ProjectRepository: Send + Sync {
    fn project(&self, project_id: Uuid) -> AppResult<Option<CompanyProject>>;
    fn company_projects(&self, company_id: Uuid) -> AppResult<Vec<CompanyProject>>;
}

pub trait TaskRepository: Send + Sync {
    fn project_tasks(&self, project_id: Uuid) -> AppResult<Vec<CompanyProjectTask>>;
}

pub trait MemoryRepository: Send + Sync {
    fn memory(&self, memory_id: Uuid) -> AppResult<Option<AgentMemory>>;
    fn company_memories(&self, company_id: Uuid) -> AppResult<Vec<AgentMemory>>;
}

pub trait CodexRepository: Send + Sync {
    fn agent_trigger(&self, agent_id: Uuid) -> AppResult<Option<AgentCodexTriggerConfig>>;
    fn agent_runs(&self, agent_id: Uuid, limit: usize) -> AppResult<Vec<AgentCodexTriggerRun>>;
}

impl<T: PlatformRepository> AuthRepository for T {
    fn find_human_by_email(&self, email: &str) -> AppResult<Option<HumanUser>> {
        self.find_human_user_by_email_result(email)
    }

    fn human_credential(&self, human_user_id: Uuid) -> AppResult<Option<HumanCredential>> {
        self.get_human_credential_result(human_user_id)
    }

    fn human_session_by_hash(&self, token_hash: &str) -> AppResult<Option<HumanSession>> {
        self.find_human_session_by_token_hash_result(token_hash)
    }

    fn human_account_token_by_hash(
        &self,
        token_hash: &str,
    ) -> AppResult<Option<HumanAccountToken>> {
        self.find_human_account_token_by_hash_result(token_hash)
    }
}

impl<T: PlatformRepository> CompanyRepository for T {
    fn company(&self, company_id: Uuid) -> AppResult<Option<Company>> {
        self.get_company_result(company_id)
    }

    fn company_human_member(
        &self,
        company_id: Uuid,
        human_user_id: Uuid,
    ) -> AppResult<Option<CompanyHumanMember>> {
        self.get_company_human_member_result(company_id, human_user_id)
    }
}

impl<T: PlatformRepository> ChatRepository for T {
    fn conversation_context(
        &self,
        conversation_id: Uuid,
    ) -> AppResult<Option<ConversationContext>> {
        self.get_conversation_context_result(conversation_id)
    }

    fn company_conversations(&self, company_id: Uuid) -> AppResult<Vec<ConversationPreview>> {
        self.list_company_conversations_result(company_id)
    }

    fn conversation_message_page(
        &self,
        conversation_id: Uuid,
        before_message_id: Option<Uuid>,
        limit: usize,
    ) -> AppResult<MessagePageView> {
        self.get_conversation_message_page(conversation_id, before_message_id, limit)
    }
}

impl<T: PlatformRepository> ProjectRepository for T {
    fn project(&self, project_id: Uuid) -> AppResult<Option<CompanyProject>> {
        self.get_company_project_result(project_id)
    }

    fn company_projects(&self, company_id: Uuid) -> AppResult<Vec<CompanyProject>> {
        self.list_company_projects_result(company_id)
    }
}

impl<T: PlatformRepository> TaskRepository for T {
    fn project_tasks(&self, project_id: Uuid) -> AppResult<Vec<CompanyProjectTask>> {
        self.list_company_project_tasks_result(project_id)
    }
}

impl<T: PlatformRepository> MemoryRepository for T {
    fn memory(&self, memory_id: Uuid) -> AppResult<Option<AgentMemory>> {
        self.get_agent_memory_result(memory_id)
    }

    fn company_memories(&self, company_id: Uuid) -> AppResult<Vec<AgentMemory>> {
        self.list_company_agent_memories_result(company_id)
    }
}

impl<T: PlatformRepository> CodexRepository for T {
    fn agent_trigger(&self, agent_id: Uuid) -> AppResult<Option<AgentCodexTriggerConfig>> {
        self.get_agent_codex_trigger_config_by_agent_result(agent_id)
    }

    fn agent_runs(&self, agent_id: Uuid, limit: usize) -> AppResult<Vec<AgentCodexTriggerRun>> {
        self.list_agent_codex_trigger_runs_result(agent_id, limit)
    }
}

pub trait RelayRepository:
    AuthRepository
    + CompanyRepository
    + ChatRepository
    + ProjectRepository
    + TaskRepository
    + MemoryRepository
    + CodexRepository
{
}

impl<T> RelayRepository for T where
    T: AuthRepository
        + CompanyRepository
        + ChatRepository
        + ProjectRepository
        + TaskRepository
        + MemoryRepository
        + CodexRepository
{
}
