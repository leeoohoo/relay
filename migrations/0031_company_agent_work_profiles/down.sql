ALTER TABLE company_agent_memberships
    DROP CONSTRAINT IF EXISTS company_agent_memberships_skills_array_check,
    DROP CONSTRAINT IF EXISTS company_agent_memberships_responsibilities_array_check,
    DROP COLUMN IF EXISTS current_focus,
    DROP COLUMN IF EXISTS skills,
    DROP COLUMN IF EXISTS responsibilities;
