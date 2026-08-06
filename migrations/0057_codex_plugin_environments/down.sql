DROP INDEX uq_codex_plugin_operations_active_action;

DELETE FROM codex_plugin_catalog_snapshots snapshot
USING codex_plugin_catalog_snapshots duplicate
WHERE snapshot.runner_id = duplicate.runner_id
  AND snapshot.target_selector > duplicate.target_selector;

ALTER TABLE codex_plugin_catalog_snapshots
    DROP CONSTRAINT codex_plugin_catalog_snapshots_pkey,
    ADD PRIMARY KEY (runner_id),
    DROP COLUMN diagnostic_message,
    DROP COLUMN discovery_status,
    DROP COLUMN target_selector;

ALTER TABLE codex_plugin_operations
    DROP COLUMN target_selector;

CREATE UNIQUE INDEX uq_codex_plugin_operations_active_action
    ON codex_plugin_operations (target_runner_id, operation, COALESCE(plugin_id, ''))
    WHERE status IN ('queued', 'running');
