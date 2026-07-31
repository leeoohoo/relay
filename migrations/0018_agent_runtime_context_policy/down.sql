DROP TRIGGER IF EXISTS trg_agent_runtime_configs_policy_realtime_update
    ON agent_runtime_configs;
DROP FUNCTION IF EXISTS emit_agent_runtime_policy_realtime_event();

ALTER TABLE agent_runtime_configs
    DROP COLUMN IF EXISTS context_max_inbox_events,
    DROP COLUMN IF EXISTS context_max_conversations,
    DROP COLUMN IF EXISTS context_max_projects,
    DROP COLUMN IF EXISTS auto_announce_project_membership,
    DROP COLUMN IF EXISTS auto_start_assigned_tasks,
    DROP COLUMN IF EXISTS company_message_policy;
