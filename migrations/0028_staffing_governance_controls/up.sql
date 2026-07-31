ALTER TABLE org_units
    ADD CONSTRAINT uq_org_units_id_company UNIQUE (id, company_id);

ALTER TABLE company_agent_memberships
    ADD COLUMN staffing_scope_org_unit_id UUID,
    ADD CONSTRAINT fk_company_agent_staffing_scope
        FOREIGN KEY (staffing_scope_org_unit_id, company_id)
        REFERENCES org_units(id, company_id) ON DELETE RESTRICT;

CREATE INDEX idx_company_agent_memberships_staffing_scope
    ON company_agent_memberships(company_id, staffing_scope_org_unit_id)
    WHERE staffing_scope_org_unit_id IS NOT NULL;
