use super::mapping::*;
use super::*;

impl CodexControlPlatformRepository for PostgresPlatformRepository {
    fn save_company_codex_runner_profile(
        &self,
        profile: CompanyCodexRunnerProfile,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO company_codex_runner_profiles (
                    id, company_id, name, interval_seconds, codex_profile,
                    model, reasoning_effort, reasoning_summary, verbosity, personality,
                    service_tier, sandbox_mode, approval_policy, network_access, web_search,
                    feature_multi_agent, feature_remote_plugin, feature_hooks, feature_goals,
                    feature_shell_tool, max_run_seconds, is_default,
                    created_by_human_user_id, updated_by_human_user_id,
                    created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                        $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                        $21, $22, $23, $24, $25, $26)
                ON CONFLICT (id) DO UPDATE
                SET name = EXCLUDED.name,
                    interval_seconds = EXCLUDED.interval_seconds,
                    codex_profile = EXCLUDED.codex_profile,
                    model = EXCLUDED.model,
                    reasoning_effort = EXCLUDED.reasoning_effort,
                    reasoning_summary = EXCLUDED.reasoning_summary,
                    verbosity = EXCLUDED.verbosity,
                    personality = EXCLUDED.personality,
                    service_tier = EXCLUDED.service_tier,
                    sandbox_mode = EXCLUDED.sandbox_mode,
                    approval_policy = EXCLUDED.approval_policy,
                    network_access = EXCLUDED.network_access,
                    web_search = EXCLUDED.web_search,
                    feature_multi_agent = EXCLUDED.feature_multi_agent,
                    feature_remote_plugin = EXCLUDED.feature_remote_plugin,
                    feature_hooks = EXCLUDED.feature_hooks,
                    feature_goals = EXCLUDED.feature_goals,
                    feature_shell_tool = EXCLUDED.feature_shell_tool,
                    max_run_seconds = EXCLUDED.max_run_seconds,
                    is_default = EXCLUDED.is_default,
                    updated_by_human_user_id = EXCLUDED.updated_by_human_user_id,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &profile.id,
                    &profile.company_id,
                    &profile.name,
                    &profile.interval_seconds,
                    &profile.codex_profile,
                    &profile.model,
                    &profile.reasoning_effort,
                    &profile.reasoning_summary,
                    &profile.verbosity,
                    &profile.personality,
                    &profile.service_tier,
                    &profile.sandbox_mode,
                    &profile.approval_policy,
                    &profile.network_access,
                    &profile.web_search,
                    &profile.feature_multi_agent,
                    &profile.feature_remote_plugin,
                    &profile.feature_hooks,
                    &profile.feature_goals,
                    &profile.feature_shell_tool,
                    &profile.max_run_seconds,
                    &profile.is_default,
                    &profile.created_by_human_user_id,
                    &profile.updated_by_human_user_id,
                    &profile.created_at,
                    &profile.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get_company_codex_runner_profile(
        &self,
        profile_id: Uuid,
    ) -> Option<CompanyCodexRunnerProfile> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT id, company_id, name, interval_seconds, codex_profile,
                       model, reasoning_effort, reasoning_summary, verbosity, personality,
                       service_tier, sandbox_mode, approval_policy, network_access, web_search,
                       feature_multi_agent, feature_remote_plugin, feature_hooks, feature_goals,
                       feature_shell_tool, max_run_seconds, is_default,
                       created_by_human_user_id, updated_by_human_user_id,
                       created_at, updated_at
                FROM company_codex_runner_profiles
                WHERE id = $1
                "#,
                &[&profile_id],
            )
        })
        .ok()
        .flatten()
        .map(map_company_codex_runner_profile)
    }

    fn list_company_codex_runner_profiles(
        &self,
        company_id: Uuid,
    ) -> Vec<CompanyCodexRunnerProfile> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, company_id, name, interval_seconds, codex_profile,
                       model, reasoning_effort, reasoning_summary, verbosity, personality,
                       service_tier, sandbox_mode, approval_policy, network_access, web_search,
                       feature_multi_agent, feature_remote_plugin, feature_hooks, feature_goals,
                       feature_shell_tool, max_run_seconds, is_default,
                       created_by_human_user_id, updated_by_human_user_id,
                       created_at, updated_at
                FROM company_codex_runner_profiles
                WHERE company_id = $1
                ORDER BY is_default DESC, name, id
                "#,
                &[&company_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(map_company_codex_runner_profile)
        .collect()
    }

    fn delete_company_codex_runner_profile(&self, profile_id: Uuid) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                "DELETE FROM company_codex_runner_profiles WHERE id = $1",
                &[&profile_id],
            )?;
            Ok(())
        })
    }

    fn clear_company_codex_runner_profile_defaults(
        &self,
        company_id: Uuid,
        except_profile_id: Uuid,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                UPDATE company_codex_runner_profiles
                SET is_default = FALSE, updated_at = NOW()
                WHERE company_id = $1 AND id <> $2 AND is_default
                "#,
                &[&company_id, &except_profile_id],
            )?;
            Ok(())
        })
    }

    fn assign_agent_codex_runner_profile(
        &self,
        agent_id: Uuid,
        profile_id: Uuid,
        human_user_id: Uuid,
        assigned_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO agent_codex_runner_profile_assignments (
                    agent_profile_id, runner_profile_id,
                    assigned_by_human_user_id, assigned_at
                )
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (agent_profile_id) DO UPDATE
                SET runner_profile_id = EXCLUDED.runner_profile_id,
                    assigned_by_human_user_id = EXCLUDED.assigned_by_human_user_id,
                    assigned_at = EXCLUDED.assigned_at
                "#,
                &[&agent_id, &profile_id, &human_user_id, &assigned_at],
            )?;
            Ok(())
        })
    }

    fn get_agent_codex_runner_profile_assignment(&self, agent_id: Uuid) -> Option<Uuid> {
        self.with_client(|client| {
            client.query_opt(
                r#"
                SELECT runner_profile_id
                FROM agent_codex_runner_profile_assignments
                WHERE agent_profile_id = $1
                "#,
                &[&agent_id],
            )
        })
        .ok()
        .flatten()
        .map(|row| row.get("runner_profile_id"))
    }

    fn list_codex_runner_profile_agent_ids(&self, profile_id: Uuid) -> Vec<Uuid> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT agent_profile_id
                FROM agent_codex_runner_profile_assignments
                WHERE runner_profile_id = $1
                ORDER BY agent_profile_id
                "#,
                &[&profile_id],
            )
        })
        .unwrap_or_default()
        .into_iter()
        .map(|row| row.get("agent_profile_id"))
        .collect()
    }

    fn save_codex_plugin_catalog_snapshot(
        &self,
        snapshot: CodexPluginCatalogSnapshot,
    ) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO codex_plugin_catalog_snapshots (
                    runner_id, target_selector, hostname, codex_version, fingerprint,
                    discovery_status, diagnostic_message, installed, available,
                    marketplaces, discovered_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                ON CONFLICT (runner_id, target_selector) DO UPDATE
                SET hostname = EXCLUDED.hostname,
                    codex_version = EXCLUDED.codex_version,
                    fingerprint = EXCLUDED.fingerprint,
                    discovery_status = EXCLUDED.discovery_status,
                    diagnostic_message = EXCLUDED.diagnostic_message,
                    installed = EXCLUDED.installed,
                    available = EXCLUDED.available,
                    marketplaces = EXCLUDED.marketplaces,
                    discovered_at = EXCLUDED.discovered_at,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &snapshot.runner_id,
                    &snapshot.target_selector,
                    &snapshot.hostname,
                    &snapshot.codex_version,
                    &snapshot.fingerprint,
                    &snapshot.discovery_status,
                    &snapshot.diagnostic_message,
                    &Json(snapshot.installed),
                    &Json(snapshot.available),
                    &Json(snapshot.marketplaces),
                    &snapshot.discovered_at,
                    &snapshot.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_codex_plugin_catalog_snapshots(&self) -> AppResult<Vec<CodexPluginCatalogSnapshot>> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT runner_id, target_selector, hostname, codex_version, fingerprint,
                       discovery_status, diagnostic_message, installed, available,
                       marketplaces, discovered_at, updated_at
                FROM codex_plugin_catalog_snapshots
                ORDER BY discovered_at DESC, runner_id, target_selector
                "#,
                &[],
            )
        })
        .map(|rows| {
            rows.into_iter()
                .map(map_codex_plugin_catalog_snapshot)
                .collect()
        })
    }

    fn insert_codex_plugin_operation(&self, operation: CodexPluginOperation) -> AppResult<()> {
        self.with_client(|client| {
            client.execute(
                r#"
                INSERT INTO codex_plugin_operations (
                    id, company_id, target_runner_id, target_selector, operation, plugin_id, status,
                    requested_by_human_user_id, lease_owner, lease_expires_at,
                    attempt_count, error_message, result, requested_at,
                    started_at, finished_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9,
                        $10, $11, $12, $13, $14, $15, $16, $17)
                "#,
                &[
                    &operation.id,
                    &operation.company_id,
                    &operation.target_runner_id,
                    &operation.target_selector,
                    &operation.operation,
                    &operation.plugin_id,
                    &operation.status,
                    &operation.requested_by_human_user_id,
                    &operation.lease_owner,
                    &operation.lease_expires_at,
                    &operation.attempt_count,
                    &operation.error_message,
                    &Json(operation.result),
                    &operation.requested_at,
                    &operation.started_at,
                    &operation.finished_at,
                    &operation.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn list_company_codex_plugin_operations(
        &self,
        company_id: Uuid,
        limit: usize,
    ) -> AppResult<Vec<CodexPluginOperation>> {
        self.with_client(|client| {
            client.query(
                r#"
                SELECT id, company_id, target_runner_id, target_selector, operation, plugin_id, status,
                       requested_by_human_user_id, lease_owner, lease_expires_at,
                       attempt_count, error_message, result, requested_at,
                       started_at, finished_at, updated_at
                FROM codex_plugin_operations
                WHERE company_id = $1
                ORDER BY requested_at DESC, id
                LIMIT $2
                "#,
                &[&company_id, &(limit as i64)],
            )
        })
        .map(|rows| rows.into_iter().map(map_codex_plugin_operation).collect())
    }

    fn claim_codex_plugin_operations(
        &self,
        target_runner_id: &str,
        lease_owner: &str,
        now: chrono::DateTime<chrono::Utc>,
        limit: usize,
    ) -> AppResult<Vec<CodexPluginOperation>> {
        self.with_client(|client| {
            client.query(
                r#"
                WITH due AS (
                    SELECT id
                    FROM codex_plugin_operations
                    WHERE target_runner_id = $1
                      AND (
                        status = 'queued'
                        OR (status = 'running' AND lease_expires_at <= $3)
                      )
                    ORDER BY requested_at, id
                    FOR UPDATE SKIP LOCKED
                    LIMIT $4
                )
                UPDATE codex_plugin_operations operation
                SET status = 'running',
                    lease_owner = $2,
                    lease_expires_at = $3 + INTERVAL '2 minutes',
                    attempt_count = operation.attempt_count + 1,
                    started_at = COALESCE(operation.started_at, $3),
                    updated_at = $3
                FROM due
                WHERE operation.id = due.id
                RETURNING operation.id, operation.company_id, operation.target_runner_id,
                          operation.target_selector, operation.operation, operation.plugin_id,
                          operation.status,
                          operation.requested_by_human_user_id, operation.lease_owner,
                          operation.lease_expires_at, operation.attempt_count,
                          operation.error_message, operation.result, operation.requested_at,
                          operation.started_at, operation.finished_at, operation.updated_at
                "#,
                &[&target_runner_id, &lease_owner, &now, &(limit as i64)],
            )
        })
        .map(|rows| rows.into_iter().map(map_codex_plugin_operation).collect())
    }

    fn finish_codex_plugin_operation(
        &self,
        operation_id: Uuid,
        lease_owner: &str,
        succeeded: bool,
        result: Value,
        error_message: Option<String>,
        finished_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let updated = self.with_client(|client| {
            client.execute(
                r#"
                UPDATE codex_plugin_operations
                SET status = CASE
                        WHEN $3 THEN 'succeeded'
                        WHEN attempt_count < 3 THEN 'queued'
                        ELSE 'failed'
                    END,
                    lease_owner = NULL,
                    lease_expires_at = NULL,
                    error_message = $5,
                    result = $4,
                    finished_at = CASE
                        WHEN $3 OR attempt_count >= 3 THEN $6::TIMESTAMPTZ
                        ELSE NULL
                    END,
                    updated_at = $6::TIMESTAMPTZ
                WHERE id = $1
                  AND status = 'running'
                  AND lease_owner = $2
                "#,
                &[
                    &operation_id,
                    &lease_owner,
                    &succeeded,
                    &Json(result),
                    &error_message,
                    &finished_at,
                ],
            )
        })?;
        if updated == 0 {
            return Err(AppError::Conflict(
                "Codex plugin operation lease is no longer owned by this Trigger".into(),
            ));
        }
        Ok(())
    }
}
