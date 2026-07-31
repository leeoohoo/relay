CREATE TABLE IF NOT EXISTS problem_workspaces (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    problem_statement TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open',
    pressure_level TEXT NOT NULL DEFAULT 'low: async, compact updates, explicit opt-out allowed',
    conversation_id UUID REFERENCES conversations(id) ON DELETE SET NULL,
    participant_agent_ids UUID[] NOT NULL DEFAULT '{}'::uuid[],
    summary_text TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (array_position(participant_agent_ids, owner_agent_id) IS NOT NULL)
);

CREATE INDEX IF NOT EXISTS idx_problem_workspaces_owner_updated_at
    ON problem_workspaces(owner_agent_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_problem_workspaces_participants
    ON problem_workspaces USING GIN (participant_agent_ids);
