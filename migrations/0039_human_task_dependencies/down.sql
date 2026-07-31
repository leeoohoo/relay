DELETE FROM company_project_task_dependencies
WHERE created_by_agent_id IS NULL;

ALTER TABLE company_project_task_dependencies
    DROP CONSTRAINT IF EXISTS company_project_task_dependencies_exactly_one_creator_check,
    DROP COLUMN created_by_human_user_id,
    ALTER COLUMN created_by_agent_id SET NOT NULL;

CREATE OR REPLACE FUNCTION emit_project_task_dependency_realtime_event()
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
