ALTER TABLE agent_runtime_configs
    ADD COLUMN company_message_policy TEXT NOT NULL DEFAULT 'direct_ack'
        CHECK (company_message_policy IN ('observe', 'direct_ack', 'mentioned_ack')),
    ADD COLUMN auto_start_assigned_tasks BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN auto_announce_project_membership BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN context_max_projects INTEGER NOT NULL DEFAULT 10
        CHECK (context_max_projects BETWEEN 1 AND 50),
    ADD COLUMN context_max_conversations INTEGER NOT NULL DEFAULT 10
        CHECK (context_max_conversations BETWEEN 1 AND 50),
    ADD COLUMN context_max_inbox_events INTEGER NOT NULL DEFAULT 20
        CHECK (context_max_inbox_events BETWEEN 1 AND 100);

CREATE FUNCTION emit_agent_runtime_policy_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.company_message_policy IS NOT DISTINCT FROM NEW.company_message_policy
       AND OLD.auto_start_assigned_tasks IS NOT DISTINCT FROM NEW.auto_start_assigned_tasks
       AND OLD.auto_announce_project_membership IS NOT DISTINCT FROM NEW.auto_announce_project_membership
       AND OLD.context_max_projects IS NOT DISTINCT FROM NEW.context_max_projects
       AND OLD.context_max_conversations IS NOT DISTINCT FROM NEW.context_max_conversations
       AND OLD.context_max_inbox_events IS NOT DISTINCT FROM NEW.context_max_inbox_events THEN
        RETURN NEW;
    END IF;

    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_human_user_id, payload, created_at
    ) VALUES (
        NEW.company_id,
        'agent.runtime.policy_updated',
        'agent_runtime_config',
        NEW.id,
        NEW.updated_by_human_user_id,
        jsonb_build_object(
            'runtime_config_id', NEW.id,
            'agent_profile_id', NEW.agent_profile_id,
            'company_message_policy', NEW.company_message_policy,
            'auto_start_assigned_tasks', NEW.auto_start_assigned_tasks,
            'auto_announce_project_membership', NEW.auto_announce_project_membership,
            'context_max_projects', NEW.context_max_projects,
            'context_max_conversations', NEW.context_max_conversations,
            'context_max_inbox_events', NEW.context_max_inbox_events
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_agent_runtime_configs_policy_realtime_update
AFTER UPDATE OF company_message_policy, auto_start_assigned_tasks,
    auto_announce_project_membership, context_max_projects,
    context_max_conversations, context_max_inbox_events
ON agent_runtime_configs
FOR EACH ROW EXECUTE FUNCTION emit_agent_runtime_policy_realtime_event();
