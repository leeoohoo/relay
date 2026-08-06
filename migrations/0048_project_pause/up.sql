ALTER TABLE company_projects
    DROP CONSTRAINT IF EXISTS company_projects_status_check;

ALTER TABLE company_projects
    ADD CONSTRAINT company_projects_status_check
    CHECK (status IN ('planned', 'active', 'paused', 'blocked', 'completed', 'cancelled'));
