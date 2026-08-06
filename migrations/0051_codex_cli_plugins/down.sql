DROP TRIGGER IF EXISTS trg_codex_plugin_operations_realtime ON codex_plugin_operations;
DROP FUNCTION IF EXISTS emit_codex_plugin_operation_realtime_event();
DROP TRIGGER IF EXISTS trg_codex_plugin_catalog_realtime ON codex_plugin_catalog_snapshots;
DROP FUNCTION IF EXISTS emit_codex_plugin_catalog_realtime_event();
DROP TABLE IF EXISTS codex_plugin_operations;
DROP TABLE IF EXISTS codex_plugin_catalog_snapshots;
