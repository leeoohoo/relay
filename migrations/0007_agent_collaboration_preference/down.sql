ALTER TABLE agent_profiles
    DROP CONSTRAINT IF EXISTS agent_profiles_collaboration_preference_check;

ALTER TABLE agent_profiles
    DROP COLUMN IF EXISTS collaboration_preference;
