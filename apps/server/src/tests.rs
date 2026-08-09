use super::*;

#[test]
fn bearer_token_requires_authorization_bearer_scheme() {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        "Bearer hus_test".parse().expect("valid header"),
    );
    assert_eq!(bearer_token(&headers).expect("bearer token"), "hus_test");

    headers.insert(
        axum::http::header::AUTHORIZATION,
        "bearer   hus_lowercase".parse().expect("valid header"),
    );
    assert_eq!(
        bearer_token(&headers).expect("case-insensitive bearer token"),
        "hus_lowercase"
    );

    headers.insert(
        axum::http::header::AUTHORIZATION,
        "BEARER\thus_uppercase".parse().expect("valid header"),
    );
    assert_eq!(
        bearer_token(&headers).expect("uppercase bearer token"),
        "hus_uppercase"
    );

    headers.insert(
        axum::http::header::AUTHORIZATION,
        "Basic abc".parse().expect("valid header"),
    );
    assert!(bearer_token(&headers).is_err());

    headers.insert(
        axum::http::header::AUTHORIZATION,
        "Bearer".parse().expect("valid header"),
    );
    assert!(bearer_token(&headers).is_err());
}

#[test]
fn owner_path_cannot_select_another_human_user() {
    assert!(require_same_human(Uuid::new_v4(), Uuid::new_v4()).is_err());
    let user_id = Uuid::new_v4();
    assert!(require_same_human(user_id, user_id).is_ok());
}

#[test]
fn admin_credentials_support_legacy_root_and_scoped_tokens() {
    let scoped_token = "ops-token-with-at-least-24-chars";
    let configured = serde_json::json!([{
        "name": "observer",
        "token": scoped_token,
        "scopes": [ADMIN_SCOPE_READ]
    }])
    .to_string();
    let credentials = build_admin_credentials(Some("legacy-root-token"), Some(&configured))
        .expect("admin credentials should parse");

    let legacy = credentials
        .iter()
        .find(|credential| credential.name == "legacy-root")
        .expect("legacy root should exist");
    assert!(legacy.allows(ADMIN_SCOPE_AGENTS));

    let observer = credentials
        .iter()
        .find(|credential| credential.name == "observer")
        .expect("observer should exist");
    assert!(observer.allows(ADMIN_SCOPE_READ));
    assert!(!observer.allows(ADMIN_SCOPE_AGENTS));
    assert_eq!(observer.token_hash, hash_secret(scoped_token));
}

#[test]
fn admin_credentials_reject_weak_or_unknown_scope_entries() {
    let weak = r#"[{"name":"ops","token":"short","scopes":["admin:read"]}]"#;
    assert!(build_admin_credentials(None, Some(weak)).is_err());

    let unknown = r#"[{"name":"ops","token":"a-strong-token-with-24-characters","scopes":["admin:unknown"]}]"#;
    assert!(build_admin_credentials(None, Some(unknown)).is_err());
}

#[test]
fn sliding_window_rate_limiter_rejects_requests_over_budget() {
    let limiter = SlidingWindowRateLimiter::new(2, StdDuration::from_secs(60), "test");
    assert!(limiter.check("same-client").is_ok());
    assert!(limiter.check("same-client").is_ok());
    assert!(matches!(
        limiter.check("same-client"),
        Err(ApiError(AppError::RateLimited(_)))
    ));
    assert!(limiter.check("different-client").is_ok());
}

#[test]
fn local_project_import_copies_source_into_a_fresh_git_repository() {
    let root = std::env::temp_dir().join(format!("relay-project-import-{}", Uuid::new_v4()));
    let source = root.join("source");
    let destination = root.join("managed");
    fs::create_dir_all(source.join("src")).expect("source directories");
    fs::create_dir_all(source.join(".git")).expect("source git metadata");
    fs::create_dir_all(source.join("node_modules/pkg")).expect("source dependency cache");
    fs::write(
        source.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )
    .expect("manifest");
    fs::write(source.join("src/main.rs"), "fn main() {}\n").expect("source file");
    fs::write(source.join(".git/config"), "source metadata").expect("git metadata");
    fs::write(source.join("node_modules/pkg/index.js"), "cache").expect("cache file");

    import_project_folder(&source, &destination).expect("folder import should succeed");
    assert!(destination.join("Cargo.toml").is_file());
    assert!(destination.join("src/main.rs").is_file());
    assert!(destination.join(".git").is_dir());
    assert!(!destination.join("node_modules").exists());
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&destination)
        .output()
        .expect("git status");
    assert!(status.status.success());
    assert!(String::from_utf8_lossy(&status.stdout).trim().is_empty());
    fs::remove_dir_all(root).expect("test import should be removable");
}

#[test]
fn imported_project_is_published_to_the_provisioned_remote() {
    let root = std::env::temp_dir().join(format!("relay-project-publish-{}", Uuid::new_v4()));
    let project = root.join("project");
    let remote = root.join("remote.git");
    fs::create_dir_all(&project).expect("project directory");
    fs::write(project.join("README.md"), "# Imported\n").expect("project file");
    initialize_managed_project_git(&project).expect("local git initialization");
    let init_remote = Command::new("git")
        .args(["init", "--bare", remote.to_str().expect("remote path")])
        .output()
        .expect("bare remote initialization");
    assert!(init_remote.status.success());

    let project_id = Uuid::new_v4();
    let credential_store = GitCredentialStore::at(root.join("credentials")).expect("credentials");
    let auth_profile = credential_store
        .store_managed_git_token(
            project_id,
            "relay-test",
            "test-token-with-more-than-20-characters",
        )
        .expect("managed token");
    let provisioned = ProvisionedProjectGit {
        remote_url: format!("file://{}", remote.display()),
        push_url: None,
        default_branch: "main".into(),
        auth_profile,
        repository_identifier: "imported-project".into(),
        access_token_identifier: "relay-project-test-token".into(),
    };

    push_managed_project_to_remote(&project, &provisioned, &credential_store)
        .expect("project publish");

    let show = Command::new("git")
        .args([
            "--git-dir",
            remote.to_str().expect("remote path"),
            "show",
            "main:README.md",
        ])
        .output()
        .expect("read remote file");
    assert!(show.status.success());
    assert_eq!(String::from_utf8_lossy(&show.stdout), "# Imported\n");
    fs::remove_dir_all(root).expect("test publish should be removable");
}

#[test]
fn shallow_git_import_is_completed_before_harness_publish() {
    let root = std::env::temp_dir().join(format!("relay-shallow-publish-{}", Uuid::new_v4()));
    let source = root.join("source");
    let project = root.join("project");
    let remote = root.join("remote.git");
    fs::create_dir_all(&source).expect("source directory");

    let run = |path: &FsPath, args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .expect("git command should start");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run(&source, &["init", "-b", "main"]);
    run(&source, &["config", "user.name", "Relay Test"]);
    run(
        &source,
        &["config", "user.email", "relay-test@local.invalid"],
    );
    fs::write(source.join("README.md"), "first\n").expect("first revision");
    run(&source, &["add", "README.md"]);
    run(&source, &["commit", "-m", "first"]);
    fs::write(source.join("README.md"), "second\n").expect("second revision");
    run(&source, &["commit", "-am", "second"]);

    let clone = Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--single-branch",
            "--branch",
            "main",
            &format!("file://{}", source.display()),
            project.to_str().expect("project path"),
        ])
        .output()
        .expect("shallow clone should start");
    assert!(
        clone.status.success(),
        "shallow clone failed: {}",
        String::from_utf8_lossy(&clone.stderr)
    );

    let init_remote = Command::new("git")
        .args(["init", "--bare", remote.to_str().expect("remote path")])
        .output()
        .expect("bare remote initialization");
    assert!(init_remote.status.success());

    let project_id = Uuid::new_v4();
    let credential_store = GitCredentialStore::at(root.join("credentials")).expect("credentials");
    let auth_profile = credential_store
        .store_managed_git_token(
            project_id,
            "relay-test",
            "test-token-with-more-than-20-characters",
        )
        .expect("managed token");
    let provisioned = ProvisionedProjectGit {
        remote_url: format!("file://{}", remote.display()),
        push_url: None,
        default_branch: "main".into(),
        auth_profile,
        repository_identifier: "shallow-imported-project".into(),
        access_token_identifier: "relay-project-shallow-token".into(),
    };

    push_managed_project_to_remote(&project, &provisioned, &credential_store)
        .expect("shallow project publish");

    let count = Command::new("git")
        .args([
            "--git-dir",
            remote.to_str().expect("remote path"),
            "rev-list",
            "--count",
            "main",
        ])
        .output()
        .expect("read remote history");
    assert!(count.status.success());
    assert_eq!(String::from_utf8_lossy(&count.stdout).trim(), "2");
    fs::remove_dir_all(root).expect("test publish should be removable");
}

#[test]
fn uploaded_project_paths_are_normalized_and_exclude_generated_directories() {
    assert_eq!(
        normalize_uploaded_project_path("src\\main.rs").expect("valid path"),
        Some("src/main.rs".into())
    );
    assert_eq!(
        normalize_uploaded_project_path("node_modules/pkg/index.js").expect("excluded path"),
        None
    );
    assert_eq!(
        normalize_uploaded_project_path("client/target/debug/app").expect("excluded path"),
        None
    );
    assert!(normalize_uploaded_project_path("../secret.txt").is_err());
    assert!(normalize_uploaded_project_path("/absolute/path").is_err());
}

#[test]
fn repository_browser_accepts_only_enumerated_harness_refs_and_safe_paths() {
    let refs = vec![
        ProjectRepositoryRef {
            name: "main".into(),
            full_name: "main".into(),
            commit: "1111111111111111111111111111111111111111".into(),
            kind: "branch".into(),
            is_default: true,
        },
        ProjectRepositoryRef {
            name: "release".into(),
            full_name: "release".into(),
            commit: "2222222222222222222222222222222222222222".into(),
            kind: "branch".into(),
            is_default: false,
        },
    ];
    let selected = selected_repository_ref(&refs, None).expect("default ref");
    assert_eq!(selected.name, "main");
    assert_eq!(
        selected_repository_ref(&refs, Some("release"))
            .expect("release ref")
            .commit,
        "2222222222222222222222222222222222222222"
    );
    assert!(selected_repository_ref(&refs, Some("refs/heads/unknown")).is_err());
    assert_eq!(
        normalize_repository_path("src\\main.rs", false).expect("safe path"),
        "src/main.rs"
    );
    assert!(normalize_repository_path("../secret", false).is_err());
    assert!(normalize_repository_path("/absolute", false).is_err());
}

#[test]
fn console_pages_enforce_a_bounded_response_size() {
    let response = console_page_response(
        "agents",
        vec![serde_json::json!({ "id": Uuid::new_v4(), "name": "Agent" })],
        None,
        false,
    )
    .expect("small Console page should fit its budget");
    assert_eq!(response["agents"].as_array().map(Vec::len), Some(1));

    let oversized = "x".repeat(MAX_CONSOLE_PAGE_RESPONSE_BYTES + 1);
    let error = console_page_response("projects", vec![oversized], None, false)
        .expect_err("oversized Console page must be rejected");
    assert!(error.to_string().contains("response budget"));
}
