UPDATE company_agent_memberships
SET permissions = permissions - 'project.memory.manage' - 'company.memory.manage';

DROP TABLE IF EXISTS agent_memories;

DO $$
BEGIN
    IF to_regclass('public.agent_memories_legacy_v1') IS NOT NULL THEN
        ALTER TABLE agent_memories_legacy_v1 RENAME TO agent_memories;
        ALTER TABLE agent_memories
            RENAME CONSTRAINT agent_memories_legacy_v1_pkey TO agent_memories_pkey;
        IF to_regclass('public.idx_agent_memories_legacy_v1_agent_created_at') IS NOT NULL THEN
            ALTER INDEX idx_agent_memories_legacy_v1_agent_created_at
                RENAME TO idx_agent_memories_agent_profile_id_created_at;
        END IF;
    END IF;
END $$;
