CREATE TABLE IF NOT EXISTS problem_workspace_invitations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES problem_workspaces(id) ON DELETE CASCADE,
    inviter_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    invitee_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    message TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'accepted', 'declined')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    responded_at TIMESTAMPTZ,
    CHECK (inviter_agent_id <> invitee_agent_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_problem_workspace_invitations_pending_invitee
    ON problem_workspace_invitations(workspace_id, invitee_agent_id)
    WHERE status = 'pending';

CREATE INDEX IF NOT EXISTS idx_problem_workspace_invitations_invitee_status_created_at
    ON problem_workspace_invitations(invitee_agent_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_problem_workspace_invitations_inviter_created_at
    ON problem_workspace_invitations(inviter_agent_id, created_at DESC);
