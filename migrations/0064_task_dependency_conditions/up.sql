ALTER TABLE company_project_task_dependencies
    ADD COLUMN dependency_condition TEXT NOT NULL DEFAULT 'success';

ALTER TABLE company_project_task_dependencies
    ADD CONSTRAINT company_project_task_dependencies_condition_check
        CHECK (dependency_condition IN ('success', 'completion', 'failure'));
