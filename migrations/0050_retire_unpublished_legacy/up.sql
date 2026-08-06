CREATE SCHEMA IF NOT EXISTS relay_legacy;

ALTER TABLE problem_workspace_invitations SET SCHEMA relay_legacy;
ALTER TABLE problem_workspace_progress_records SET SCHEMA relay_legacy;
ALTER TABLE problem_workspaces SET SCHEMA relay_legacy;

ALTER TABLE post_comments SET SCHEMA relay_legacy;
ALTER TABLE post_reactions SET SCHEMA relay_legacy;
ALTER TABLE posts SET SCHEMA relay_legacy;
ALTER TABLE diary_entries SET SCHEMA relay_legacy;

ALTER TABLE friend_profile_facts SET SCHEMA relay_legacy;
ALTER TABLE friend_profiles SET SCHEMA relay_legacy;
ALTER TABLE friend_requests SET SCHEMA relay_legacy;
ALTER TABLE friendships SET SCHEMA relay_legacy;
ALTER TABLE blocks SET SCHEMA relay_legacy;
ALTER TABLE relationship_states SET SCHEMA relay_legacy;
ALTER TABLE interaction_summaries SET SCHEMA relay_legacy;

ALTER TABLE agent_runtime_runs SET SCHEMA relay_legacy;
ALTER TABLE agent_runtime_configs SET SCHEMA relay_legacy;
ALTER TABLE agent_runtime_templates SET SCHEMA relay_legacy;
ALTER TABLE agent_model_price_catalog_entries SET SCHEMA relay_legacy;
ALTER TABLE company_model_budget_policies SET SCHEMA relay_legacy;

DO $$
BEGIN
    IF to_regclass('public.agent_memories_legacy_v1') IS NOT NULL THEN
        ALTER TABLE agent_memories_legacy_v1 SET SCHEMA relay_legacy;
    END IF;
END $$;
