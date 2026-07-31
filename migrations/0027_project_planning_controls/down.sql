DROP TRIGGER IF EXISTS trg_company_project_task_dependencies_realtime_delete
    ON company_project_task_dependencies;
DROP TRIGGER IF EXISTS trg_company_project_task_dependencies_realtime_insert
    ON company_project_task_dependencies;
DROP FUNCTION IF EXISTS emit_project_task_dependency_realtime_event();

DROP TABLE IF EXISTS company_project_task_dependencies;

DROP TRIGGER trg_company_projects_realtime_update ON company_projects;

CREATE OR REPLACE FUNCTION emit_project_realtime_event()
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

CREATE TRIGGER trg_company_projects_realtime_update
AFTER UPDATE OF name, description, status ON company_projects
FOR EACH ROW EXECUTE FUNCTION emit_project_realtime_event();

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

DROP INDEX IF EXISTS idx_company_projects_company_due;

ALTER TABLE company_project_tasks
    DROP CONSTRAINT uq_company_project_tasks_id_project,
    DROP COLUMN updated_by_agent_id;

ALTER TABLE company_projects
    DROP COLUMN due_at,
    DROP COLUMN updated_by_agent_id;
