CREATE TABLE realtime_events (
    sequence_id BIGSERIAL PRIMARY KEY,
    id UUID NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    aggregate_type TEXT NOT NULL,
    aggregate_id UUID,
    actor_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    actor_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_realtime_events_company_sequence
    ON realtime_events(company_id, sequence_id);

CREATE INDEX idx_realtime_events_created_at
    ON realtime_events(created_at);

CREATE FUNCTION notify_realtime_event_insert()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify(
        'ai_chat_realtime_events',
        json_build_object(
            'sequence_id', NEW.sequence_id,
            'company_id', NEW.company_id
        )::text
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_realtime_events_notify
AFTER INSERT ON realtime_events
FOR EACH ROW EXECUTE FUNCTION notify_realtime_event_insert();

CREATE FUNCTION emit_message_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_agent_id, payload, created_at
    )
    SELECT
        c.company_id,
        'message.created',
        'conversation',
        NEW.conversation_id,
        NEW.sender_agent_id,
        jsonb_build_object(
            'message_id', NEW.id,
            'conversation_id', NEW.conversation_id,
            'context_type', c.context_type,
            'project_id', c.project_id,
            'content_preview', left(NEW.content_text, 240)
        ),
        NEW.created_at
    FROM conversations c
    WHERE c.id = NEW.conversation_id
      AND c.company_id IS NOT NULL;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_messages_realtime_event
AFTER INSERT ON messages
FOR EACH ROW EXECUTE FUNCTION emit_message_realtime_event();

CREATE FUNCTION emit_project_realtime_event()
RETURNS TRIGGER AS $$
DECLARE
    emitted_type TEXT;
BEGIN
    emitted_type := CASE WHEN TG_OP = 'INSERT' THEN 'project.created' ELSE 'project.updated' END;
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_agent_id, payload, created_at
    ) VALUES (
        NEW.company_id,
        emitted_type,
        'project',
        NEW.id,
        NEW.created_by_agent_id,
        jsonb_build_object(
            'project_id', NEW.id,
            'name', NEW.name,
            'status', NEW.status,
            'project_group_conversation_id', NEW.project_group_conversation_id
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_company_projects_realtime_insert
AFTER INSERT ON company_projects
FOR EACH ROW EXECUTE FUNCTION emit_project_realtime_event();

CREATE TRIGGER trg_company_projects_realtime_update
AFTER UPDATE OF name, description, status ON company_projects
FOR EACH ROW EXECUTE FUNCTION emit_project_realtime_event();

CREATE FUNCTION emit_project_member_realtime_event()
RETURNS TRIGGER AS $$
DECLARE
    project_company_id UUID;
    emitted_type TEXT;
BEGIN
    IF TG_OP = 'UPDATE' AND OLD.left_at IS NOT DISTINCT FROM NEW.left_at THEN
        RETURN NEW;
    END IF;
    SELECT company_id INTO project_company_id
    FROM company_projects
    WHERE id = NEW.project_id;
    emitted_type := CASE
        WHEN NEW.left_at IS NOT NULL THEN 'project.member.removed'
        WHEN TG_OP = 'UPDATE' THEN 'project.member.rejoined'
        ELSE 'project.member.added'
    END;
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_agent_id, payload, created_at
    ) VALUES (
        project_company_id,
        emitted_type,
        'project_member',
        NEW.id,
        NEW.added_by_agent_id,
        jsonb_build_object(
            'project_id', NEW.project_id,
            'agent_profile_id', NEW.agent_profile_id,
            'role', NEW.role,
            'left_at', NEW.left_at
        ),
        COALESCE(NEW.left_at, NEW.joined_at)
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_company_project_members_realtime_insert
AFTER INSERT ON company_project_members
FOR EACH ROW EXECUTE FUNCTION emit_project_member_realtime_event();

CREATE TRIGGER trg_company_project_members_realtime_update
AFTER UPDATE OF left_at ON company_project_members
FOR EACH ROW EXECUTE FUNCTION emit_project_member_realtime_event();

CREATE FUNCTION emit_project_task_realtime_event()
RETURNS TRIGGER AS $$
DECLARE
    project_company_id UUID;
    emitted_type TEXT;
BEGIN
    SELECT company_id INTO project_company_id
    FROM company_projects
    WHERE id = NEW.project_id;
    emitted_type := CASE WHEN TG_OP = 'INSERT' THEN 'project.task.created' ELSE 'project.task.updated' END;
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_agent_id, payload, created_at
    ) VALUES (
        project_company_id,
        emitted_type,
        'project_task',
        NEW.id,
        NEW.created_by_agent_id,
        jsonb_build_object(
            'project_id', NEW.project_id,
            'task_id', NEW.id,
            'title', NEW.title,
            'status', NEW.status,
            'priority', NEW.priority,
            'assignee_agent_id', NEW.assignee_agent_id,
            'due_at', NEW.due_at
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_company_project_tasks_realtime_insert
AFTER INSERT ON company_project_tasks
FOR EACH ROW EXECUTE FUNCTION emit_project_task_realtime_event();

CREATE TRIGGER trg_company_project_tasks_realtime_update
AFTER UPDATE ON company_project_tasks
FOR EACH ROW EXECUTE FUNCTION emit_project_task_realtime_event();

CREATE FUNCTION emit_project_status_realtime_event()
RETURNS TRIGGER AS $$
DECLARE
    project_company_id UUID;
BEGIN
    SELECT company_id INTO project_company_id
    FROM company_projects
    WHERE id = NEW.project_id;
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_agent_id, payload, created_at
    ) VALUES (
        project_company_id,
        'project.status.updated',
        'project_status_update',
        NEW.id,
        NEW.author_agent_id,
        jsonb_build_object(
            'project_id', NEW.project_id,
            'status_update_id', NEW.id,
            'summary', NEW.summary,
            'progress_percent', NEW.progress_percent,
            'blockers', NEW.blockers,
            'next_steps', NEW.next_steps,
            'project_status', NEW.project_status
        ),
        NEW.created_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_company_project_status_realtime_insert
AFTER INSERT ON company_project_status_updates
FOR EACH ROW EXECUTE FUNCTION emit_project_status_realtime_event();

CREATE FUNCTION emit_staffing_realtime_event()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_agent_id, actor_human_user_id, payload, created_at
    ) VALUES (
        NEW.company_id,
        'staffing.action.created',
        'staffing_action',
        NEW.id,
        NEW.actor_agent_id,
        NEW.actor_human_user_id,
        jsonb_build_object(
            'action_id', NEW.id,
            'action_type', NEW.action_type,
            'target_agent_id', NEW.target_agent_id,
            'status', NEW.status,
            'reason', NEW.reason
        ),
        NEW.created_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_agent_staffing_actions_realtime_insert
AFTER INSERT ON agent_staffing_actions
FOR EACH ROW EXECUTE FUNCTION emit_staffing_realtime_event();
