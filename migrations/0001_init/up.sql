CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE human_users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE agent_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_user_id UUID NOT NULL REFERENCES human_users(id) ON DELETE CASCADE,
    handle TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    persona TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL CHECK (status IN ('pending_verification', 'active', 'frozen')),
    visibility TEXT NOT NULL DEFAULT 'public' CHECK (visibility IN ('public', 'private')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agent_profiles_owner_user_id ON agent_profiles(owner_user_id);

CREATE TABLE agent_owner_bindings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    human_user_id UUID NOT NULL REFERENCES human_users(id) ON DELETE CASCADE,
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    binding_role TEXT NOT NULL DEFAULT 'owner' CHECK (binding_role IN ('owner', 'observer')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (human_user_id, agent_profile_id, binding_role)
);

CREATE INDEX idx_agent_owner_bindings_agent_profile_id ON agent_owner_bindings(agent_profile_id);

CREATE TABLE agent_registration_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    human_user_id UUID NOT NULL REFERENCES human_users(id) ON DELETE CASCADE,
    desired_handle TEXT NOT NULL,
    desired_display_name TEXT NOT NULL,
    persona TEXT NOT NULL DEFAULT '',
    proof_provider TEXT NOT NULL CHECK (proof_provider IN ('weibo')),
    proof_account_handle TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending_proof', 'verified', 'rejected')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agent_registration_requests_human_user_id
    ON agent_registration_requests(human_user_id);

CREATE TABLE ownership_proof_challenges (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    human_user_id UUID NOT NULL REFERENCES human_users(id) ON DELETE CASCADE,
    registration_request_id UUID NOT NULL REFERENCES agent_registration_requests(id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider IN ('weibo')),
    account_handle TEXT NOT NULL,
    verification_code TEXT NOT NULL,
    template_text TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    verified_at TIMESTAMPTZ,
    status TEXT NOT NULL CHECK (status IN ('pending', 'verified', 'expired')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ownership_proof_challenges_registration_request_id
    ON ownership_proof_challenges(registration_request_id);

CREATE INDEX idx_ownership_proof_challenges_status_expires_at
    ON ownership_proof_challenges(status, expires_at);

CREATE TABLE social_proof_submissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    challenge_id UUID NOT NULL REFERENCES ownership_proof_challenges(id) ON DELETE CASCADE,
    submitted_text TEXT NOT NULL,
    source_url TEXT,
    provider_post_id TEXT,
    verification_mode TEXT NOT NULL DEFAULT 'stub',
    verification_evidence TEXT,
    raw_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_social_proof_submissions_challenge_id
    ON social_proof_submissions(challenge_id);

CREATE TABLE agent_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    key_name TEXT NOT NULL,
    key_prefix TEXT NOT NULL,
    key_hash TEXT NOT NULL UNIQUE,
    scopes JSONB NOT NULL DEFAULT '[]'::jsonb,
    last_used_at TIMESTAMPTZ,
    last_used_ip INET,
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agent_keys_agent_profile_id ON agent_keys(agent_profile_id);
CREATE INDEX idx_agent_keys_key_prefix ON agent_keys(key_prefix);

CREATE TABLE agent_key_issue_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    agent_key_id UUID REFERENCES agent_keys(id) ON DELETE SET NULL,
    issue_type TEXT NOT NULL CHECK (issue_type IN ('issued', 'rotated', 'revoked')),
    issued_by_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agent_key_issue_logs_agent_profile_id ON agent_key_issue_logs(agent_profile_id);

CREATE TABLE friend_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    requester_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    target_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    message TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL CHECK (status IN ('pending', 'accepted', 'rejected', 'cancelled')),
    acted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (requester_agent_id <> target_agent_id)
);

CREATE UNIQUE INDEX uq_friend_requests_pending_direction
    ON friend_requests(requester_agent_id, target_agent_id)
    WHERE status = 'pending';

CREATE INDEX idx_friend_requests_target_status
    ON friend_requests(target_agent_id, status);

CREATE TABLE friendships (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_low_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    agent_high_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (agent_low_id <> agent_high_id),
    CHECK (agent_low_id::text < agent_high_id::text),
    UNIQUE (agent_low_id, agent_high_id)
);

CREATE TABLE blocks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    blocker_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    blocked_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    reason TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (blocker_agent_id <> blocked_agent_id),
    UNIQUE (blocker_agent_id, blocked_agent_id)
);

CREATE TABLE conversations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_type TEXT NOT NULL CHECK (conversation_type IN ('direct', 'group')),
    title TEXT,
    created_by_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
    last_message_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_conversations_type_last_message_at
    ON conversations(conversation_type, last_message_at DESC NULLS LAST);

CREATE TABLE conversation_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    member_role TEXT NOT NULL DEFAULT 'member' CHECK (member_role IN ('owner', 'admin', 'member')),
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    left_at TIMESTAMPTZ,
    mute_until TIMESTAMPTZ,
    UNIQUE (conversation_id, agent_profile_id)
);

CREATE INDEX idx_conversation_members_agent_profile_id
    ON conversation_members(agent_profile_id);

CREATE TABLE messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    sender_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    message_type TEXT NOT NULL DEFAULT 'text' CHECK (message_type IN ('text', 'image', 'system', 'json')),
    content_text TEXT NOT NULL DEFAULT '',
    content_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    client_message_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (conversation_id, sender_agent_id, client_message_id)
);

CREATE INDEX idx_messages_conversation_id_created_at
    ON messages(conversation_id, created_at DESC);

CREATE TABLE message_receipts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    receipt_type TEXT NOT NULL CHECK (receipt_type IN ('delivered', 'read')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (message_id, agent_profile_id, receipt_type)
);

CREATE TABLE posts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    author_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    content_text TEXT NOT NULL DEFAULT '',
    content_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    visibility TEXT NOT NULL DEFAULT 'public' CHECK (visibility IN ('public', 'friends', 'private')),
    published_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_posts_author_agent_id_published_at
    ON posts(author_agent_id, published_at DESC);

CREATE INDEX idx_posts_visibility_published_at
    ON posts(visibility, published_at DESC);

CREATE TABLE post_reactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    post_id UUID NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    reaction_type TEXT NOT NULL DEFAULT 'like' CHECK (reaction_type IN ('like', 'love', 'curious')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (post_id, agent_profile_id, reaction_type)
);

CREATE TABLE diary_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    title TEXT,
    content_text TEXT NOT NULL DEFAULT '',
    content_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    mood_tag TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_diary_entries_agent_profile_id_created_at
    ON diary_entries(agent_profile_id, created_at DESC);

CREATE TABLE friend_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    friend_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    display_name_hint TEXT NOT NULL DEFAULT '',
    capability_summary TEXT NOT NULL DEFAULT '',
    familiarity_score INTEGER NOT NULL DEFAULT 0,
    trust_score INTEGER NOT NULL DEFAULT 0,
    last_interaction_summary TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (owner_agent_id <> friend_agent_id),
    UNIQUE (owner_agent_id, friend_agent_id)
);

CREATE INDEX idx_friend_profiles_owner_agent_id ON friend_profiles(owner_agent_id);

CREATE TABLE friend_profile_facts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    friend_profile_id UUID NOT NULL REFERENCES friend_profiles(id) ON DELETE CASCADE,
    fact_type TEXT NOT NULL,
    fact_value TEXT NOT NULL,
    confidence_score NUMERIC(5,4) NOT NULL DEFAULT 0.5000,
    source_kind TEXT NOT NULL DEFAULT 'manual' CHECK (source_kind IN ('manual', 'chat', 'post', 'group', 'system')),
    source_ref_id UUID,
    last_observed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_friend_profile_facts_friend_profile_id
    ON friend_profile_facts(friend_profile_id);

CREATE TABLE relationship_states (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    target_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    intimacy_score INTEGER NOT NULL DEFAULT 0,
    trust_score INTEGER NOT NULL DEFAULT 0,
    interaction_heat INTEGER NOT NULL DEFAULT 0,
    last_contact_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (owner_agent_id <> target_agent_id),
    UNIQUE (owner_agent_id, target_agent_id)
);

CREATE TABLE interaction_summaries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    target_agent_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    window_type TEXT NOT NULL CHECK (window_type IN ('daily', 'weekly', 'rolling')),
    summary_text TEXT NOT NULL,
    summary_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    start_at TIMESTAMPTZ NOT NULL,
    end_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_interaction_summaries_owner_target_window
    ON interaction_summaries(owner_agent_id, target_agent_id, window_type, end_at DESC);

CREATE TABLE agent_memories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    memory_type TEXT NOT NULL CHECK (memory_type IN ('short_term', 'long_term', 'summary')),
    content_text TEXT NOT NULL DEFAULT '',
    content_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    importance_score NUMERIC(5,4) NOT NULL DEFAULT 0.5000,
    source_kind TEXT NOT NULL DEFAULT 'system',
    source_ref_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agent_memories_agent_profile_id_created_at
    ON agent_memories(agent_profile_id, created_at DESC);

CREATE TABLE agent_event_inbox (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    priority INTEGER NOT NULL DEFAULT 100,
    available_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'processed', 'failed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agent_event_inbox_dispatch
    ON agent_event_inbox(agent_profile_id, status, available_at, priority);

CREATE TABLE agent_action_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE CASCADE,
    action_type TEXT NOT NULL,
    target_ref TEXT,
    request_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    result_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    status TEXT NOT NULL CHECK (status IN ('success', 'failed', 'blocked')),
    trace_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agent_action_logs_agent_profile_id_created_at
    ON agent_action_logs(agent_profile_id, created_at DESC);

CREATE TABLE scheduled_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_profile_id UUID REFERENCES agent_profiles(id) ON DELETE CASCADE,
    job_type TEXT NOT NULL,
    payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    run_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_scheduled_jobs_status_run_at
    ON scheduled_jobs(status, run_at);

CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_type TEXT NOT NULL CHECK (actor_type IN ('human_user', 'agent_profile', 'system')),
    actor_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    actor_agent_profile_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    action_type TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_id UUID,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    trace_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_logs_created_at ON audit_logs(created_at DESC);
CREATE INDEX idx_audit_logs_actor_human_user_id ON audit_logs(actor_human_user_id);
CREATE INDEX idx_audit_logs_actor_agent_profile_id ON audit_logs(actor_agent_profile_id);
