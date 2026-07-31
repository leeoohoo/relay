ALTER TABLE company_agent_memberships
    ADD COLUMN responsibilities JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN skills JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN current_focus TEXT NOT NULL DEFAULT '';

UPDATE company_agent_memberships membership
SET responsibilities = jsonb_build_array(profile.persona)
FROM agent_profiles profile
WHERE profile.id = membership.agent_profile_id
  AND BTRIM(profile.persona) <> ''
  AND membership.responsibilities = '[]'::jsonb;

ALTER TABLE company_agent_memberships
    ADD CONSTRAINT company_agent_memberships_responsibilities_array_check
        CHECK (jsonb_typeof(responsibilities) = 'array'),
    ADD CONSTRAINT company_agent_memberships_skills_array_check
        CHECK (jsonb_typeof(skills) = 'array');
