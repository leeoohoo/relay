use std::{net::SocketAddr, sync::Arc};

use axum::{routing::get, Json, Router};
use serde::Serialize;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use ai_chat_application::PlatformApp;
use ai_chat_infrastructure::config::{ApiConfig, McpConfig};
use ai_chat_infrastructure::git_credentials::GitCredentialStore;
use ai_chat_infrastructure::harness::{HarnessProjectGitProvisioner, HarnessProvisioner};
use ai_chat_infrastructure::ownership_proof::OwnershipProofVerifierAdapter;
use ai_chat_infrastructure::project_git::ProjectGitProvisioner;
use ai_chat_infrastructure::{build_ownership_proof_verifier, build_repository, RepositoryAdapter};
use ai_chat_mcp::{AiChatMcpHandler, McpGateway, STANDARD_MCP_PATH};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,tower_http=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let api_config = ApiConfig::from_env();
    let verifier = build_ownership_proof_verifier(&api_config);
    let mcp_config = McpConfig::from_env();
    let repository = build_repository(&api_config)?;
    let harness = HarnessProvisioner::from_config(repository.clone(), &api_config)?;
    let git_credentials = GitCredentialStore::from_env()?;
    let project_git_provisioner = if harness.is_enabled() {
        Some(
            Arc::new(HarnessProjectGitProvisioner::new(harness, git_credentials))
                as Arc<dyn ProjectGitProvisioner>,
        )
    } else {
        None
    };
    let platform = PlatformApp::with_verifier(repository, verifier);
    let gateway = McpGateway::new(platform, mcp_config.agent_key.clone())
        .with_project_git_provisioner(project_git_provisioner);
    let server_config = StreamableHttpServerConfig::default()
        .with_stateful_mode(false)
        .with_json_response(true)
        .with_sse_keep_alive(None)
        .with_allowed_hosts(mcp_config.allowed_hosts)
        .with_allowed_origins(mcp_config.allowed_origins);
    let standard_mcp: StreamableHttpService<
        AiChatMcpHandler<RepositoryAdapter, OwnershipProofVerifierAdapter>,
        LocalSessionManager,
    > = StreamableHttpService::new(
        move || Ok(AiChatMcpHandler::new(gateway.clone())),
        Default::default(),
        server_config,
    );

    let app = Router::new()
        .route("/health", get(health))
        .route_service(STANDARD_MCP_PATH, standard_mcp)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let port = std::env::var("MCP_SERVER_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8081);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(%addr, "agent company MCP server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "agent-company-mcp",
    })
}
