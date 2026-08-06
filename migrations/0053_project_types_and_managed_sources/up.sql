ALTER TABLE company_projects
    ADD COLUMN project_type TEXT NOT NULL DEFAULT 'general',
    ADD COLUMN project_type_source TEXT NOT NULL DEFAULT 'system_default',
    ADD COLUMN project_type_confidence INTEGER NOT NULL DEFAULT 30,
    ADD COLUMN project_type_evidence JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD CONSTRAINT company_projects_type_check CHECK (project_type IN (
        'software_development', 'game_development', 'novel_writing',
        'general_writing', 'research', 'data_analysis', 'product_design',
        'marketing_content', 'documentation', 'automation', 'operations', 'general'
    )),
    ADD CONSTRAINT company_projects_type_source_check CHECK (project_type_source IN (
        'human', 'description_inference', 'folder_inference', 'system_default'
    )),
    ADD CONSTRAINT company_projects_type_confidence_check
        CHECK (project_type_confidence BETWEEN 0 AND 100),
    ADD CONSTRAINT company_projects_type_evidence_check
        CHECK (jsonb_typeof(project_type_evidence) = 'array');

CREATE INDEX idx_company_projects_company_type_updated
    ON company_projects(company_id, project_type, updated_at DESC);
