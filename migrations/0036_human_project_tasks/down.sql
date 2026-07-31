DELETE FROM company_project_tasks WHERE created_by_agent_id IS NULL;

ALTER TABLE company_project_tasks
    DROP CONSTRAINT IF EXISTS company_project_tasks_at_most_one_updater_check,
    DROP CONSTRAINT IF EXISTS company_project_tasks_exactly_one_creator_check,
    DROP COLUMN IF EXISTS updated_by_human_user_id,
    DROP COLUMN IF EXISTS created_by_human_user_id,
    ALTER COLUMN created_by_agent_id SET NOT NULL;

CREATE OR REPLACE FUNCTION emit_project_task_realtime_event()
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
        COALESCE(NEW.updated_by_agent_id, NEW.created_by_agent_id),
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
