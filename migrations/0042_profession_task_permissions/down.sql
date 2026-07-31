UPDATE company_project_tasks
SET status = 'blocked', updated_at = NOW()
WHERE status = 'failed';

UPDATE agent_staffing_actions
SET action_type = 'role_update'
WHERE action_type = 'profession_update';

ALTER TABLE agent_staffing_actions
    DROP CONSTRAINT IF EXISTS agent_staffing_actions_action_type_check,
    ADD CONSTRAINT agent_staffing_actions_action_type_check CHECK (
        action_type IN (
            'permission_update', 'role_update', 'hire', 'activate', 'suspend',
            'reactivate', 'terminate'
        )
    );

ALTER TABLE company_project_tasks
    DROP CONSTRAINT IF EXISTS company_project_tasks_status_check,
    ADD CONSTRAINT company_project_tasks_status_check CHECK (
        status IN ('todo', 'in_progress', 'blocked', 'done', 'cancelled')
    );

UPDATE company_agent_memberships
SET permissions = CASE
        WHEN permissions ? 'task.assign' THEN permissions
        ELSE permissions || '["task.assign"]'::jsonb
    END,
    updated_at = NOW()
WHERE role_key = 'company_manager';
