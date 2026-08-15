use super::*;

fn integration_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:15533/ai_chat".into())
}

#[test]
fn postgres_pool_keeps_a_small_idle_floor_and_expands_on_demand() {
    assert_eq!(database_pool_sizes(None, None), (16, 1));
    assert_eq!(database_pool_sizes(Some("8"), Some("2")), (8, 2));
    assert_eq!(database_pool_sizes(Some("4"), Some("20")), (4, 4));
    assert_eq!(
        database_pool_sizes(Some("invalid"), Some("invalid")),
        (16, 1)
    );
    assert_eq!(database_pool_sizes(Some("8"), Some("0")), (8, 0));
}

#[test]
#[ignore = "requires a migrated PostgreSQL database"]
fn project_cleanup_jobs_enforce_postgres_leases_retries_and_completion() {
    let repository = PostgresPlatformRepository::connect(&integration_database_url())
        .expect("connect to migrated PostgreSQL");
    let now = Utc::now();
    let human_user_id = Uuid::new_v4();
    let company_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let job_id = Uuid::new_v4();
    let provisioning_lease = now + chrono::Duration::minutes(1);

    repository
        .with_client(|client| {
            client.execute(
                r#"
                INSERT INTO human_users (id, email, display_name, status, created_at, updated_at)
                VALUES ($1, $2, 'Cleanup integration test', 'active', $3, $3)
                "#,
                &[
                    &human_user_id,
                    &format!("cleanup-{human_user_id}@example.invalid"),
                    &now,
                ],
            )?;
            client.execute(
                r#"
                INSERT INTO companies (
                    id, owner_user_id, name, slug, description, status, created_at, updated_at
                )
                VALUES ($1, $2, 'Cleanup integration test', $3, '', 'active', $4, $4)
                "#,
                &[
                    &company_id,
                    &human_user_id,
                    &format!("cleanup-{company_id}"),
                    &now,
                ],
            )?;
            Ok(())
        })
        .expect("seed integration test owners");

    repository
        .save_project_provisioning_cleanup_job(ProjectProvisioningCleanupJob {
            id: job_id,
            human_user_id,
            company_id,
            project_id,
            managed_local_path: format!("/tmp/relay-cleanup-{project_id}"),
            repository_identifier: format!("repository-{project_id}"),
            access_token_identifier: format!("token-{project_id}"),
            status: "running".into(),
            attempts: 0,
            next_attempt_at: now,
            lease_expires_at: Some(provisioning_lease),
            last_error: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        })
        .expect("save cleanup job");

    let lease_expires_at = provisioning_lease + chrono::Duration::minutes(5);
    assert!(repository
        .claim_due_project_provisioning_cleanup_job(now, lease_expires_at)
        .expect("respect active provisioning lease")
        .is_none());
    let claimed = repository
        .claim_due_project_provisioning_cleanup_job(provisioning_lease, lease_expires_at)
        .expect("claim cleanup job")
        .expect("expired provisioning lease should be recoverable");
    assert_eq!(claimed.id, job_id);
    assert_eq!(claimed.status, "running");
    assert_eq!(claimed.attempts, 1);
    assert_eq!(claimed.lease_expires_at, Some(lease_expires_at));
    assert!(repository
        .claim_due_project_provisioning_cleanup_job(provisioning_lease, lease_expires_at)
        .expect("respect active lease")
        .is_none());

    let retry_at = now + chrono::Duration::minutes(10);
    repository
        .retry_project_provisioning_cleanup_job(
            job_id,
            "temporary Harness failure".into(),
            retry_at,
            now,
        )
        .expect("schedule retry");
    assert!(repository
        .claim_due_project_provisioning_cleanup_job(now, lease_expires_at)
        .expect("respect retry schedule")
        .is_none());

    let second_lease = retry_at + chrono::Duration::minutes(5);
    let retried = repository
        .claim_due_project_provisioning_cleanup_job(retry_at, second_lease)
        .expect("claim retried cleanup job")
        .expect("retry is due");
    assert_eq!(retried.id, job_id);
    assert_eq!(retried.attempts, 2);

    repository
        .complete_project_provisioning_cleanup_job(job_id, retry_at)
        .expect("complete cleanup job");
    assert!(repository
        .claim_due_project_provisioning_cleanup_job(second_lease, second_lease)
        .expect("completed jobs stay terminal")
        .is_none());

    repository
        .with_client(|client| {
            client.execute("DELETE FROM companies WHERE id = $1", &[&company_id])?;
            client.execute("DELETE FROM human_users WHERE id = $1", &[&human_user_id])?;
            Ok(())
        })
        .expect("remove integration test records");
}
