use std::path::Path;

use chrono::Duration;
use serde_json::json;

use super::*;
use crate::ownership_proof::{
    OwnershipProofVerifier, OwnershipVerificationInput, OwnershipVerificationOutcome,
};
use crate::validation::*;
use crate::MemoryPlatformRepository;
use ai_chat_domain::agent_identity::AGENT_COLLABORATION_PREFERENCE_LOW_COST_ONLY;
use ai_chat_domain::company::{
    AgentMemorySourceRef, PROJECT_TASK_DEPENDENCY_COMPLETION, PROJECT_TASK_DEPENDENCY_SUCCESS,
    PROJECT_TASK_PRIORITY_HIGH, PROJECT_TASK_PRIORITY_LOW, PROJECT_TASK_PRIORITY_NORMAL,
    PROJECT_TASK_PRIORITY_URGENT,
};
use ai_chat_shared::{hash_secret, now_utc};

#[derive(Clone)]
struct ExactMatchVerifier;

impl OwnershipProofVerifier for ExactMatchVerifier {
    fn verify_weibo_submission(
        &self,
        input: OwnershipVerificationInput,
    ) -> AppResult<OwnershipVerificationOutcome> {
        if input.submitted_text.trim() != input.challenge.template_text.trim() {
            return Err(AppError::Validation(
                "submitted_text must exactly match challenge.template_text".into(),
            ));
        }

        let source_url = input
            .source_url
            .clone()
            .ok_or_else(|| AppError::Validation("source_url is required".into()))?;

        Ok(OwnershipVerificationOutcome {
            verification_mode: "manual".into(),
            provider_post_id: Some("manual-post-1".into()),
            verification_evidence: Some(format!("verified via {source_url}")),
            raw_payload: json!({
                "mode": "manual",
                "source_url": source_url,
                "provider_post_id": "manual-post-1",
            }),
        })
    }
}

fn bootstrap_agent(
    app: &PlatformApp<MemoryPlatformRepository>,
    human_user_id: Uuid,
    desired_handle: &str,
    desired_display_name: &str,
    persona: &str,
    weibo_handle: &str,
    source_url: &str,
) -> AgentProfile {
    let challenge = app
        .create_weibo_challenge(CreateWeiboChallengeInput {
            human_user_id,
            desired_handle: desired_handle.into(),
            desired_display_name: desired_display_name.into(),
            persona: persona.into(),
            weibo_handle: weibo_handle.into(),
        })
        .expect("challenge should succeed");

    app.verify_weibo_challenge(VerifyWeiboChallengeInput {
        challenge_id: challenge.challenge.id,
        submitted_text: challenge.challenge.template_text.clone(),
        source_url: Some(source_url.into()),
    })
    .expect("verification should succeed")
    .agent_profile
}

mod auth_company;
mod chat;
mod codex_approvals;
mod codex_sessions;
mod company_console;
mod environments;
mod execution;
mod gates;
mod git;
mod governance_language;
mod memory;
mod ownership;
mod project_execution;
mod project_planning;
mod project_rules;
mod security;
mod staffing_governance;
mod staffing_permissions;
mod staffing_roles;
mod staffing_scope;
mod task;
