use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use ai_chat_application::{AuthPlatformRepository, MemoryPlatformRepository};
use axum::{
    extract::State,
    routing::{patch, post},
    Json, Router,
};

#[test]
fn identity_is_stable_and_unique_per_human() {
    let user = HumanUser {
        id: Uuid::parse_str("12345678-90ab-cdef-1234-567890abcdef").unwrap(),
        email: "Human@Example.com".into(),
        display_name: "Human".into(),
        created_at: Utc::now(),
    };
    let identity = HarnessIdentity::for_user(&user, "u-");
    assert_eq!(identity.uid, "relay-1234567890ab");
    assert_eq!(identity.email, "human@example.com");
    assert_eq!(identity.space_identifier, "u-relay-1234567890ab");
}

#[test]
fn credential_store_keeps_secrets_out_of_repository_records() {
    let root = std::env::temp_dir().join(format!(
        "relay-harness-credentials-{}",
        Uuid::new_v4().simple()
    ));
    let store = HarnessCredentialStore::at(root.clone()).unwrap();
    let user_id = Uuid::new_v4();
    store.store_password(user_id, "secret-password").unwrap();
    assert_eq!(
        store.read_password(user_id).unwrap().as_deref(),
        Some("secret-password")
    );
    store.store_access_token(user_id, "secret-token").unwrap();
    assert!(store.has_access_token(user_id));
    store.remove_password(user_id).unwrap();
    assert_eq!(store.read_password(user_id).unwrap(), None);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn provisions_once_and_reuses_the_persisted_harness_account() {
    let register_calls = Arc::new(AtomicUsize::new(0));
    let register_counter = register_calls.clone();
    let app = Router::new()
        .route(
            "/api/v1/register",
            post(move || {
                register_counter.fetch_add(1, Ordering::SeqCst);
                async { Json(serde_json::json!({"access_token": "login-token"})) }
            }),
        )
        .route(
            "/api/v1/spaces",
            post(|| async { Json(serde_json::json!({"identifier": "space"})) }),
        )
        .route(
            "/api/v1/user/tokens",
            post(|| async { Json(serde_json::json!({"access_token": "project-token"})) }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let credentials_root = std::env::temp_dir().join(format!(
        "relay-harness-provision-test-{}",
        Uuid::new_v4().simple()
    ));
    let repo = MemoryPlatformRepository::default();
    let provisioner = HarnessProvisioner {
        repo: repo.clone(),
        config: HarnessProvisioningConfig {
            mode: HarnessMode::Official,
            api_base_url: Some(format!("http://{address}")),
            public_base_url: Some("https://harness.example.test".into()),
            space_prefix: "u-".into(),
            admin_email: None,
            admin_password: None,
        },
        credentials: HarnessCredentialStore::at(credentials_root.clone()).unwrap(),
        client: reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap(),
        user_locks: Arc::new(Mutex::new(HashMap::new())),
    };
    let user = HumanUser {
        id: Uuid::new_v4(),
        email: "human@example.test".into(),
        display_name: "Human".into(),
        created_at: Utc::now(),
    };
    repo.insert_human_user(user.clone()).unwrap();

    let first = provisioner.ensure_account(&user).await.unwrap().unwrap();
    assert_eq!(first.status, HUMAN_HARNESS_STATUS_ACTIVE);
    assert_eq!(first.provider_mode, "official");
    assert_eq!(first.harness_base_url, "https://harness.example.test");
    assert!(provisioner.credentials.has_access_token(user.id));

    let second = provisioner.ensure_account(&user).await.unwrap().unwrap();
    assert_eq!(second.attempt_count, 1);
    assert_eq!(register_calls.load(Ordering::SeqCst), 1);

    server.abort();
    let _ = fs::remove_dir_all(credentials_root);
}

#[tokio::test]
async fn failed_provisioning_is_persisted_and_can_be_retried() {
    let register_calls = Arc::new(AtomicUsize::new(0));
    let register_counter = register_calls.clone();
    let app = Router::new()
        .route(
            "/api/v1/register",
            post(move || {
                let attempt = register_counter.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt == 0 {
                        Err(StatusCode::SERVICE_UNAVAILABLE)
                    } else {
                        Ok(Json(serde_json::json!({"access_token": "login-token"})))
                    }
                }
            }),
        )
        .route(
            "/api/v1/spaces",
            post(|| async { Json(serde_json::json!({"identifier": "space"})) }),
        )
        .route(
            "/api/v1/user/tokens",
            post(|| async { Json(serde_json::json!({"access_token": "project-token"})) }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let credentials_root = std::env::temp_dir().join(format!(
        "relay-harness-retry-test-{}",
        Uuid::new_v4().simple()
    ));
    let repo = MemoryPlatformRepository::default();
    let provisioner = HarnessProvisioner {
        repo: repo.clone(),
        config: HarnessProvisioningConfig {
            mode: HarnessMode::SelfHosted,
            api_base_url: Some(format!("http://{address}")),
            public_base_url: Some("http://127.0.0.1:3000".into()),
            space_prefix: "u-".into(),
            admin_email: None,
            admin_password: None,
        },
        credentials: HarnessCredentialStore::at(credentials_root.clone()).unwrap(),
        client: reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap(),
        user_locks: Arc::new(Mutex::new(HashMap::new())),
    };
    let user = HumanUser {
        id: Uuid::new_v4(),
        email: "retry@example.test".into(),
        display_name: "Retry Human".into(),
        created_at: Utc::now(),
    };
    repo.insert_human_user(user.clone()).unwrap();

    assert!(provisioner.ensure_account(&user).await.is_err());
    let failed = provisioner.account(user.id).unwrap().unwrap();
    assert_eq!(failed.status, HUMAN_HARNESS_STATUS_FAILED);
    assert_eq!(failed.attempt_count, 1);
    assert!(provisioner
        .credentials
        .read_password(user.id)
        .unwrap()
        .is_some());

    let active = provisioner.ensure_account(&user).await.unwrap().unwrap();
    assert_eq!(active.status, HUMAN_HARNESS_STATUS_ACTIVE);
    assert_eq!(active.attempt_count, 2);
    assert_eq!(register_calls.load(Ordering::SeqCst), 2);
    assert!(provisioner
        .credentials
        .read_password(user.id)
        .unwrap()
        .is_some());

    server.abort();
    let _ = fs::remove_dir_all(credentials_root);
}

#[tokio::test]
async fn missing_access_token_is_recreated_with_the_retained_password() {
    let register_calls = Arc::new(AtomicUsize::new(0));
    let register_counter = register_calls.clone();
    let login_calls = Arc::new(AtomicUsize::new(0));
    let login_counter = login_calls.clone();
    let app = Router::new()
        .route(
            "/api/v1/register",
            post(move || {
                let attempt = register_counter.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt == 0 {
                        (
                            StatusCode::OK,
                            Json(serde_json::json!({"access_token": "login-token"})),
                        )
                    } else {
                        (
                            StatusCode::CONFLICT,
                            Json(serde_json::json!({"message": "already exists"})),
                        )
                    }
                }
            }),
        )
        .route(
            "/api/v1/login",
            post(move || {
                login_counter.fetch_add(1, Ordering::SeqCst);
                async { Json(serde_json::json!({"access_token": "login-token"})) }
            }),
        )
        .route(
            "/api/v1/spaces",
            post(|| async { Json(serde_json::json!({"identifier": "space"})) }),
        )
        .route(
            "/api/v1/user/tokens",
            post(|| async { Json(serde_json::json!({"access_token": "project-token"})) }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let credentials_root = std::env::temp_dir().join(format!(
        "relay-harness-token-recovery-test-{}",
        Uuid::new_v4().simple()
    ));
    let repo = MemoryPlatformRepository::default();
    let provisioner = HarnessProvisioner {
        repo: repo.clone(),
        config: HarnessProvisioningConfig {
            mode: HarnessMode::SelfHosted,
            api_base_url: Some(format!("http://{address}")),
            public_base_url: Some("http://127.0.0.1:3000".into()),
            space_prefix: "u-".into(),
            admin_email: None,
            admin_password: None,
        },
        credentials: HarnessCredentialStore::at(credentials_root.clone()).unwrap(),
        client: reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap(),
        user_locks: Arc::new(Mutex::new(HashMap::new())),
    };
    let user = HumanUser {
        id: Uuid::new_v4(),
        email: "recover@example.test".into(),
        display_name: "Recover Human".into(),
        created_at: Utc::now(),
    };
    repo.insert_human_user(user.clone()).unwrap();

    provisioner.ensure_active_account(&user).await.unwrap();
    let retained_password = provisioner
        .credentials
        .read_password(user.id)
        .unwrap()
        .unwrap();
    provisioner
        .credentials
        .remove_access_token(user.id)
        .unwrap();

    let recovered = provisioner.ensure_active_account(&user).await.unwrap();
    assert_eq!(recovered.status, HUMAN_HARNESS_STATUS_ACTIVE);
    assert_eq!(recovered.attempt_count, 2);
    assert_eq!(
        provisioner
            .credentials
            .read_password(user.id)
            .unwrap()
            .as_deref(),
        Some(retained_password.as_str())
    );
    assert!(provisioner.credentials.has_access_token(user.id));
    assert_eq!(register_calls.load(Ordering::SeqCst), 2);
    assert_eq!(login_calls.load(Ordering::SeqCst), 1);

    server.abort();
    let _ = fs::remove_dir_all(credentials_root);
}

#[tokio::test]
async fn refreshes_project_git_credentials_for_the_repository_owner() {
    let app = Router::new().route(
        "/api/v1/user/tokens",
        post(|| async {
            Json(serde_json::json!({
                "access_token": "fresh-project-token-1234567890"
            }))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let credentials_root = std::env::temp_dir().join(format!(
        "relay-harness-project-token-refresh-{}",
        Uuid::new_v4().simple()
    ));
    let git_credentials_root = std::env::temp_dir().join(format!(
        "relay-git-project-token-refresh-{}",
        Uuid::new_v4().simple()
    ));
    let repo = MemoryPlatformRepository::default();
    let user = HumanUser {
        id: Uuid::new_v4(),
        email: "project-owner@example.test".into(),
        display_name: "Project Owner".into(),
        created_at: Utc::now(),
    };
    repo.insert_human_user(user.clone()).unwrap();
    let identity = HarnessIdentity::for_user(&user, "u-");
    let now = Utc::now();
    repo.upsert_human_harness_account(HumanHarnessAccount {
        human_user_id: user.id,
        provider_mode: "self_hosted".into(),
        harness_base_url: format!("http://{address}"),
        harness_uid: identity.uid.clone(),
        harness_email: identity.email,
        space_identifier: identity.space_identifier,
        status: HUMAN_HARNESS_STATUS_ACTIVE.into(),
        attempt_count: 1,
        last_error: None,
        last_attempt_at: Some(now),
        provisioned_at: Some(now),
        created_at: now,
        updated_at: now,
    })
    .unwrap();
    let provisioner = HarnessProvisioner {
        repo,
        config: HarnessProvisioningConfig {
            mode: HarnessMode::SelfHosted,
            api_base_url: Some(format!("http://{address}")),
            public_base_url: Some(format!("http://{address}")),
            space_prefix: "u-".into(),
            admin_email: None,
            admin_password: None,
        },
        credentials: HarnessCredentialStore::at(credentials_root.clone()).unwrap(),
        client: reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap(),
        user_locks: Arc::new(Mutex::new(HashMap::new())),
    };
    provisioner
        .credentials
        .store_access_token(user.id, "account-token")
        .unwrap();
    let git_credentials = GitCredentialStore::at(git_credentials_root.clone()).unwrap();
    let project_id = Uuid::new_v4();

    let profile = provisioner
        .refresh_project_git_credentials(user.id, project_id, &git_credentials)
        .await
        .unwrap();
    let environment = git_credentials.auth_environment(&profile).unwrap();
    let username = fs::read_to_string(&environment["RELAY_GIT_USERNAME_FILE"]).unwrap();
    let token = fs::read_to_string(&environment["RELAY_GIT_TOKEN_FILE"]).unwrap();
    assert_eq!(username, identity.uid);
    assert_eq!(token, "fresh-project-token-1234567890");

    server.abort();
    let _ = fs::remove_dir_all(credentials_root);
    let _ = fs::remove_dir_all(git_credentials_root);
}

#[derive(Clone)]
struct AdminRecoveryState {
    expected_user_uid: String,
    user_password: Arc<Mutex<String>>,
    reset_calls: Arc<AtomicUsize>,
}

#[tokio::test]
async fn legacy_account_without_local_credentials_is_recovered_by_admin() {
    let user = HumanUser {
        id: Uuid::new_v4(),
        email: "legacy@example.test".into(),
        display_name: "Legacy Human".into(),
        created_at: Utc::now(),
    };
    let identity = HarnessIdentity::for_user(&user, "u-");
    let recovery_state = AdminRecoveryState {
        expected_user_uid: identity.uid.clone(),
        user_password: Arc::new(Mutex::new("old-unknown-password".into())),
        reset_calls: Arc::new(AtomicUsize::new(0)),
    };
    let app = Router::new()
        .route(
            "/api/v1/register",
            post(|| async {
                (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({"message": "already exists"})),
                )
            }),
        )
        .route(
            "/api/v1/login",
            post(
                |State(state): State<AdminRecoveryState>, Json(body): Json<Value>| async move {
                    let login_identifier = body
                        .get("login_identifier")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let password = body
                        .get("password")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let valid = (login_identifier == "admin@relay.local"
                        && password == "admin-password")
                        || (login_identifier == state.expected_user_uid
                            && password == state.user_password.lock().unwrap().as_str());
                    if valid {
                        (
                            StatusCode::OK,
                            Json(serde_json::json!({"access_token": "login-token"})),
                        )
                    } else {
                        (
                            StatusCode::UNAUTHORIZED,
                            Json(serde_json::json!({"message": "invalid credentials"})),
                        )
                    }
                },
            ),
        )
        .route(
            "/api/v1/admin/users/{user_uid}",
            patch(
                |State(state): State<AdminRecoveryState>, Json(body): Json<Value>| async move {
                    let password = body
                        .get("password")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    *state.user_password.lock().unwrap() = password;
                    state.reset_calls.fetch_add(1, Ordering::SeqCst);
                    Json(serde_json::json!({"uid": state.expected_user_uid}))
                },
            ),
        )
        .route(
            "/api/v1/spaces",
            post(|| async { Json(serde_json::json!({"identifier": "space"})) }),
        )
        .route(
            "/api/v1/user/tokens",
            post(|| async { Json(serde_json::json!({"access_token": "project-token"})) }),
        )
        .with_state(recovery_state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let credentials_root = std::env::temp_dir().join(format!(
        "relay-harness-admin-recovery-test-{}",
        Uuid::new_v4().simple()
    ));
    let repo = MemoryPlatformRepository::default();
    repo.insert_human_user(user.clone()).unwrap();
    let now = Utc::now();
    repo.upsert_human_harness_account(HumanHarnessAccount {
        human_user_id: user.id,
        provider_mode: "self_hosted".into(),
        harness_base_url: "http://127.0.0.1:3000".into(),
        harness_uid: identity.uid,
        harness_email: identity.email,
        space_identifier: identity.space_identifier,
        status: HUMAN_HARNESS_STATUS_ACTIVE.into(),
        attempt_count: 1,
        last_error: None,
        last_attempt_at: Some(now),
        provisioned_at: Some(now),
        created_at: now,
        updated_at: now,
    })
    .unwrap();
    let provisioner = HarnessProvisioner {
        repo,
        config: HarnessProvisioningConfig {
            mode: HarnessMode::SelfHosted,
            api_base_url: Some(format!("http://{address}")),
            public_base_url: Some("http://127.0.0.1:3000".into()),
            space_prefix: "u-".into(),
            admin_email: Some("admin@relay.local".into()),
            admin_password: Some("admin-password".into()),
        },
        credentials: HarnessCredentialStore::at(credentials_root.clone()).unwrap(),
        client: reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap(),
        user_locks: Arc::new(Mutex::new(HashMap::new())),
    };

    let recovered = provisioner.ensure_active_account(&user).await.unwrap();
    assert_eq!(recovered.status, HUMAN_HARNESS_STATUS_ACTIVE);
    assert_eq!(recovered.attempt_count, 2);
    assert_eq!(recovery_state.reset_calls.load(Ordering::SeqCst), 1);
    assert!(provisioner.credentials.has_access_token(user.id));
    assert!(provisioner
        .credentials
        .read_password(user.id)
        .unwrap()
        .is_some());

    server.abort();
    let _ = fs::remove_dir_all(credentials_root);
}
