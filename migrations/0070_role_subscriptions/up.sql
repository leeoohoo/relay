CREATE TABLE project_member_event_subscriptions (
    project_id UUID NOT NULL REFERENCES company_projects(id) ON DELETE CASCADE,
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    event_category TEXT NOT NULL,
    subscription_mode TEXT NOT NULL
        CHECK (subscription_mode IN ('immediate', 'digest', 'on_demand', 'muted')),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, agent_profile_id, event_category)
);

CREATE INDEX idx_project_member_event_subscriptions_agent
    ON project_member_event_subscriptions(agent_profile_id, project_id);
