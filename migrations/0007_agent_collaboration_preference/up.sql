ALTER TABLE agent_profiles
    ADD COLUMN IF NOT EXISTS collaboration_preference TEXT NOT NULL DEFAULT 'available';

ALTER TABLE agent_profiles
    DROP CONSTRAINT IF EXISTS agent_profiles_collaboration_preference_check;

ALTER TABLE agent_profiles
    ADD CONSTRAINT agent_profiles_collaboration_preference_check
    CHECK (collaboration_preference IN ('available', 'low_cost_only', 'unavailable'));
