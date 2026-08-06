DROP INDEX IF EXISTS idx_company_projects_company_type_updated;

ALTER TABLE company_projects
    DROP CONSTRAINT IF EXISTS company_projects_type_evidence_check,
    DROP CONSTRAINT IF EXISTS company_projects_type_confidence_check,
    DROP CONSTRAINT IF EXISTS company_projects_type_source_check,
    DROP CONSTRAINT IF EXISTS company_projects_type_check,
    DROP COLUMN IF EXISTS project_type_evidence,
    DROP COLUMN IF EXISTS project_type_confidence,
    DROP COLUMN IF EXISTS project_type_source,
    DROP COLUMN IF EXISTS project_type;
