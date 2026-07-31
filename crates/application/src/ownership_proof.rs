use ai_chat_domain::agent_identity::OwnershipProofChallenge;
use ai_chat_shared::{AppError, AppResult};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct OwnershipVerificationInput {
    pub challenge: OwnershipProofChallenge,
    pub submitted_text: String,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OwnershipVerificationOutcome {
    pub verification_mode: String,
    pub provider_post_id: Option<String>,
    pub verification_evidence: Option<String>,
    pub raw_payload: Value,
}

pub trait OwnershipProofVerifier: Clone + Send + Sync + 'static {
    fn verify_weibo_submission(
        &self,
        input: OwnershipVerificationInput,
    ) -> AppResult<OwnershipVerificationOutcome>;
}

#[derive(Debug, Clone, Default)]
pub struct StubOwnershipProofVerifier;

impl OwnershipProofVerifier for StubOwnershipProofVerifier {
    fn verify_weibo_submission(
        &self,
        input: OwnershipVerificationInput,
    ) -> AppResult<OwnershipVerificationOutcome> {
        if !input
            .submitted_text
            .contains(&input.challenge.verification_code)
        {
            return Err(AppError::Validation(
                "submitted text does not contain the expected verification code".into(),
            ));
        }

        Ok(OwnershipVerificationOutcome {
            verification_mode: "stub".into(),
            provider_post_id: None,
            verification_evidence: Some(
                "Stub verifier accepted the submitted text because it contained the expected verification code."
                    .into(),
            ),
            raw_payload: serde_json::json!({
                "mode": "stub",
                "matched_code": input.challenge.verification_code,
            }),
        })
    }
}
