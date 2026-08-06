ALTER TABLE codex_plugin_catalog_snapshots
    ADD COLUMN target_selector TEXT NOT NULL DEFAULT 'default',
    ADD COLUMN discovery_status TEXT NOT NULL DEFAULT 'ready'
        CHECK (discovery_status IN ('ready', 'empty')),
    ADD COLUMN diagnostic_message TEXT;

ALTER TABLE codex_plugin_catalog_snapshots
    DROP CONSTRAINT codex_plugin_catalog_snapshots_pkey,
    ADD PRIMARY KEY (runner_id, target_selector);

ALTER TABLE codex_plugin_operations
    ADD COLUMN target_selector TEXT NOT NULL DEFAULT 'default';

DROP INDEX uq_codex_plugin_operations_active_action;
CREATE UNIQUE INDEX uq_codex_plugin_operations_active_action
    ON codex_plugin_operations (
        target_runner_id,
        target_selector,
        operation,
        COALESCE(plugin_id, '')
    )
    WHERE status IN ('queued', 'running');

