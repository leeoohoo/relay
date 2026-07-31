DROP TRIGGER IF EXISTS trg_company_governance_policy_realtime_update
    ON company_governance_policy_versions;
DROP TRIGGER IF EXISTS trg_company_governance_policy_realtime_insert
    ON company_governance_policy_versions;
DROP FUNCTION IF EXISTS emit_company_governance_policy_realtime_event();

DROP TABLE IF EXISTS company_governance_policy_versions;
