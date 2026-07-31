DROP INDEX IF EXISTS idx_company_agent_memberships_staffing_scope;

ALTER TABLE company_agent_memberships
    DROP CONSTRAINT fk_company_agent_staffing_scope,
    DROP COLUMN staffing_scope_org_unit_id;

ALTER TABLE org_units
    DROP CONSTRAINT uq_org_units_id_company;
