DO $$
BEGIN
    IF to_regclass('relay_legacy.agent_memories_legacy_v1') IS NOT NULL THEN
        ALTER TABLE relay_legacy.agent_memories_legacy_v1 SET SCHEMA public;
    END IF;
END $$;

ALTER TABLE relay_legacy.company_model_budget_policies SET SCHEMA public;
ALTER TABLE relay_legacy.agent_model_price_catalog_entries SET SCHEMA public;
ALTER TABLE relay_legacy.agent_runtime_templates SET SCHEMA public;
ALTER TABLE relay_legacy.agent_runtime_configs SET SCHEMA public;
ALTER TABLE relay_legacy.agent_runtime_runs SET SCHEMA public;

ALTER TABLE relay_legacy.interaction_summaries SET SCHEMA public;
ALTER TABLE relay_legacy.relationship_states SET SCHEMA public;
ALTER TABLE relay_legacy.blocks SET SCHEMA public;
ALTER TABLE relay_legacy.friendships SET SCHEMA public;
ALTER TABLE relay_legacy.friend_requests SET SCHEMA public;
ALTER TABLE relay_legacy.friend_profiles SET SCHEMA public;
ALTER TABLE relay_legacy.friend_profile_facts SET SCHEMA public;

ALTER TABLE relay_legacy.diary_entries SET SCHEMA public;
ALTER TABLE relay_legacy.posts SET SCHEMA public;
ALTER TABLE relay_legacy.post_reactions SET SCHEMA public;
ALTER TABLE relay_legacy.post_comments SET SCHEMA public;

ALTER TABLE relay_legacy.problem_workspaces SET SCHEMA public;
ALTER TABLE relay_legacy.problem_workspace_progress_records SET SCHEMA public;
ALTER TABLE relay_legacy.problem_workspace_invitations SET SCHEMA public;

DROP SCHEMA relay_legacy;
