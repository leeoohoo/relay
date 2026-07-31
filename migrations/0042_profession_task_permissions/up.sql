ALTER TABLE company_project_tasks
    DROP CONSTRAINT IF EXISTS company_project_tasks_status_check,
    ADD CONSTRAINT company_project_tasks_status_check CHECK (
        status IN ('todo', 'in_progress', 'blocked', 'done', 'failed', 'cancelled')
    );

ALTER TABLE agent_staffing_actions
    DROP CONSTRAINT IF EXISTS agent_staffing_actions_action_type_check,
    ADD CONSTRAINT agent_staffing_actions_action_type_check CHECK (
        action_type IN (
            'permission_update', 'role_update', 'profession_update', 'hire', 'activate',
            'suspend', 'reactivate', 'terminate'
        )
    );

UPDATE company_agent_memberships
SET permissions = permissions - 'task.assign',
    updated_at = NOW();

UPDATE company_agent_memberships
SET permissions = CASE
        WHEN permissions ? 'task.assign' THEN permissions
        ELSE permissions || '["task.assign"]'::jsonb
    END,
    job_title = CASE
        WHEN LOWER(job_title) IN ('pm', 'project manager')
          OR job_title LIKE '%项目经理%'
          OR job_title LIKE '%项目负责人%'
            THEN '项目经理'
        ELSE '产品经理'
    END,
    updated_at = NOW()
WHERE LOWER(job_title) IN ('pm', 'project manager', 'product manager', 'product owner')
   OR job_title LIKE '%项目经理%'
   OR job_title LIKE '%项目负责人%'
   OR job_title LIKE '%产品经理%'
   OR job_title LIKE '%产品负责人%';
