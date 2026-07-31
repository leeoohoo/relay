ALTER TABLE company_projects
    ADD COLUMN updated_by_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    ADD COLUMN due_at TIMESTAMPTZ;

ALTER TABLE company_project_tasks
    ADD COLUMN updated_by_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    ADD CONSTRAINT uq_company_project_tasks_id_project UNIQUE (id, project_id);

CREATE INDEX idx_company_projects_company_due
    ON company_projects(company_id, due_at)
    WHERE due_at IS NOT NULL
      AND status NOT IN ('completed', 'cancelled');

CREATE TABLE company_project_task_dependencies (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES company_projects(id) ON DELETE CASCADE,
    task_id UUID NOT NULL,
    depends_on_task_id UUID NOT NULL,
    created_by_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (task_id <> depends_on_task_id),
    UNIQUE (task_id, depends_on_task_id),
    FOREIGN KEY (task_id, project_id)
        REFERENCES company_project_tasks(id, project_id) ON DELETE CASCADE,
    FOREIGN KEY (depends_on_task_id, project_id)
        REFERENCES company_project_tasks(id, project_id) ON DELETE CASCADE
);

CREATE INDEX idx_company_project_task_dependencies_project
    ON company_project_task_dependencies(project_id, task_id, depends_on_task_id);

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
        COALESCE(NEW.updated_by_agent_id, NEW.created_by_agent_id),
        jsonb_build_object(
            'project_id', NEW.id,
            'name', NEW.name,
            'status', NEW.status,
            'due_at', NEW.due_at,
            'project_group_conversation_id', NEW.project_group_conversation_id
        ),
        NEW.updated_at
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER trg_company_projects_realtime_update ON company_projects;
CREATE TRIGGER trg_company_projects_realtime_update
AFTER UPDATE OF name, description, status, due_at ON company_projects
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

CREATE FUNCTION emit_project_task_dependency_realtime_event()
RETURNS TRIGGER AS $$
DECLARE
    source_row company_project_task_dependencies%ROWTYPE;
    project_company_id UUID;
BEGIN
    IF TG_OP = 'DELETE' THEN
        source_row := OLD;
    ELSE
        source_row := NEW;
    END IF;
    SELECT company_id INTO project_company_id
    FROM company_projects
    WHERE id = source_row.project_id;
    INSERT INTO realtime_events (
        company_id, event_type, aggregate_type, aggregate_id,
        actor_agent_id, payload, created_at
    ) VALUES (
        project_company_id,
        CASE
            WHEN TG_OP = 'DELETE' THEN 'project.task.dependency.removed'
            ELSE 'project.task.dependency.added'
        END,
        'project_task_dependency',
        source_row.id,
        source_row.created_by_agent_id,
        jsonb_build_object(
            'project_id', source_row.project_id,
            'task_id', source_row.task_id,
            'depends_on_task_id', source_row.depends_on_task_id
        ),
        CASE WHEN TG_OP = 'DELETE' THEN NOW() ELSE source_row.created_at END
    );
    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_company_project_task_dependencies_realtime_insert
AFTER INSERT ON company_project_task_dependencies
FOR EACH ROW EXECUTE FUNCTION emit_project_task_dependency_realtime_event();

CREATE TRIGGER trg_company_project_task_dependencies_realtime_delete
AFTER DELETE ON company_project_task_dependencies
FOR EACH ROW EXECUTE FUNCTION emit_project_task_dependency_realtime_event();
