pub mod codex_control;
pub mod codex_trigger;
pub mod config;
pub mod git_credentials;
pub mod git_workspace;
pub mod gitness;
pub mod harness;
pub mod ownership_proof;
pub mod postgres;
pub mod realtime;

use anyhow::Context;

use crate::config::ApiConfig;
use crate::ownership_proof::OwnershipProofVerifierAdapter;
use crate::postgres::PostgresPlatformRepository;

pub type RepositoryAdapter = PostgresPlatformRepository;

pub fn build_repository(config: &ApiConfig) -> anyhow::Result<RepositoryAdapter> {
    PostgresPlatformRepository::connect(&config.database_url)
        .context("failed to connect PostgreSQL repository")
}

pub fn build_ownership_proof_verifier(config: &ApiConfig) -> OwnershipProofVerifierAdapter {
    OwnershipProofVerifierAdapter::from_config(config)
        .expect("failed to build ownership proof verifier from config")
}
