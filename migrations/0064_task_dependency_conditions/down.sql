ALTER TABLE company_project_task_dependencies
    DROP CONSTRAINT IF EXISTS company_project_task_dependencies_condition_check,
    DROP COLUMN IF EXISTS dependency_condition;
