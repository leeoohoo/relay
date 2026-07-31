use ai_chat_application::{
    OwnershipProofVerifier, OwnershipVerificationInput, OwnershipVerificationOutcome,
};
use ai_chat_shared::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::process::Command;

use crate::config::{ApiConfig, OwnershipProofMode};

#[derive(Debug, Clone)]
pub enum OwnershipProofVerifierAdapter {
    Stub,
    Manual,
    Remote(RemoteOwnershipProofVerifier),
}

impl OwnershipProofVerifierAdapter {
    pub fn from_config(config: &ApiConfig) -> AppResult<Self> {
        match config.ownership_proof_mode {
            OwnershipProofMode::Stub => Self::Stub,
            OwnershipProofMode::Manual => Self::Manual,
            OwnershipProofMode::Remote => {
                let endpoint = config.ownership_proof_remote_url.clone().ok_or_else(|| {
                    AppError::Validation(
                        "WEIBO_PROOF_REMOTE_URL is required when WEIBO_PROOF_PROVIDER_MODE=remote"
                            .into(),
                    )
                })?;
                Self::Remote(RemoteOwnershipProofVerifier {
                    endpoint,
                    bearer_token: config.ownership_proof_remote_token.clone(),
                })
            }
        }
        .pipe(Ok)
    }
}

impl OwnershipProofVerifier for OwnershipProofVerifierAdapter {
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

        match self {
            Self::Stub => Ok(OwnershipVerificationOutcome {
                verification_mode: "stub".into(),
                provider_post_id: None,
                verification_evidence: Some(
                    "Stub verifier accepted the submitted text because it contained the expected verification code."
                        .into(),
                ),
                raw_payload: json!({
                    "mode": "stub",
                    "matched_code": input.challenge.verification_code,
                }),
            }),
            Self::Manual => {
                let source_url = input.source_url.clone().ok_or_else(|| {
                    AppError::Validation("manual verification mode requires source_url".into())
                })?;
                let account_handle = input.challenge.account_handle.trim().to_lowercase();
                let normalized_text = input.submitted_text.trim();
                let normalized_template = input.challenge.template_text.trim();

                if normalized_text != normalized_template {
                    return Err(AppError::Validation(
                        "manual verification mode requires submitted_text to exactly match challenge.template_text"
                            .into(),
                    ));
                }

                if source_url.trim().is_empty() {
                    return Err(AppError::Validation(
                        "manual verification mode requires a non-empty source_url".into(),
                    ));
                }

                Ok(OwnershipVerificationOutcome {
                    verification_mode: "manual".into(),
                    provider_post_id: extract_provider_post_id(&source_url),
                    verification_evidence: Some(format!(
                        "Manual verification accepted exact challenge text for @{account_handle} from source_url {source_url}."
                    )),
                    raw_payload: json!({
                        "mode": "manual",
                        "source_url": source_url,
                        "account_handle": account_handle,
                        "verification_code": input.challenge.verification_code,
                    }),
                })
            }
            Self::Remote(verifier) => verifier.verify_weibo_submission(input),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RemoteOwnershipProofVerifier {
    endpoint: String,
    bearer_token: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct RemoteVerificationRequest {
    provider: &'static str,
    challenge_id: String,
    registration_request_id: String,
    account_handle: String,
    verification_code: String,
    template_text: String,
    submitted_text: String,
    source_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RemoteVerificationResponse {
    verified: bool,
    verification_mode: Option<String>,
    provider_post_id: Option<String>,
    verification_evidence: Option<String>,
    raw_payload: Option<Value>,
    error_message: Option<String>,
}

impl OwnershipProofVerifier for RemoteOwnershipProofVerifier {
    fn verify_weibo_submission(
        &self,
        input: OwnershipVerificationInput,
    ) -> AppResult<OwnershipVerificationOutcome> {
        let request_body = RemoteVerificationRequest {
            provider: "weibo",
            challenge_id: input.challenge.id.to_string(),
            registration_request_id: input.challenge.registration_request_id.to_string(),
            account_handle: input.challenge.account_handle.clone(),
            verification_code: input.challenge.verification_code.clone(),
            template_text: input.challenge.template_text.clone(),
            submitted_text: input.submitted_text.clone(),
            source_url: input.source_url.clone(),
        };

        let body = serde_json::to_string(&request_body).map_err(|error| {
            AppError::Validation(format!(
                "remote ownership proof request could not be serialized: {error}"
            ))
        })?;

        let mut command = Command::new("curl");
        command
            .arg("-sS")
            .arg("-X")
            .arg("POST")
            .arg("-H")
            .arg("content-type: application/json");

        if let Some(token) = &self.bearer_token {
            command
                .arg("-H")
                .arg(format!("authorization: Bearer {token}"));
        }

        command.arg("-d").arg(body).arg(&self.endpoint);

        let output = command.output().map_err(|error| {
            AppError::Validation(format!(
                "remote ownership proof request failed to start curl: {error}"
            ))
        })?;

        if !output.status.success() {
            return Err(AppError::Validation(
                String::from_utf8_lossy(&output.stderr)
                    .trim()
                    .to_string()
                    .if_empty_then(|| "remote ownership proof request failed".into()),
            ));
        }

        let payload: RemoteVerificationResponse =
            serde_json::from_slice(&output.stdout).map_err(|error| {
                AppError::Validation(format!(
                    "remote ownership proof response could not be parsed: {error}"
                ))
            })?;

        if !payload.verified {
            return Err(AppError::Validation(payload.error_message.unwrap_or_else(
                || "remote ownership proof rejected the submission".into(),
            )));
        }

        Ok(OwnershipVerificationOutcome {
            verification_mode: payload.verification_mode.unwrap_or_else(|| "remote".into()),
            provider_post_id: payload.provider_post_id,
            verification_evidence: payload.verification_evidence,
            raw_payload: payload.raw_payload.unwrap_or_else(|| json!({})),
        })
    }
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl<T> Pipe for T {}

trait StringFallback {
    fn if_empty_then(self, f: impl FnOnce() -> String) -> String;
}

impl StringFallback for String {
    fn if_empty_then(self, f: impl FnOnce() -> String) -> String {
        if self.trim().is_empty() {
            f()
        } else {
            self
        }
    }
}

fn extract_provider_post_id(source_url: &str) -> Option<String> {
    let trimmed = source_url.trim().trim_end_matches('/');
    let last = trimmed.rsplit('/').next()?;
    (!last.is_empty()).then(|| last.to_string())
}
