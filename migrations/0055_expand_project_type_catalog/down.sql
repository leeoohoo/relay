ALTER TABLE company_projects
    DROP CONSTRAINT IF EXISTS company_projects_type_check;

UPDATE company_projects
SET project_type = CASE
        WHEN project_type IN (
            'web_application', 'mobile_application', 'desktop_application',
            'backend_service', 'library_sdk', 'iot_embedded_system',
            'enterprise_erp', 'warehouse_management_system',
            'customer_relationship_management', 'manufacturing_execution_system',
            'ecommerce_platform', 'data_engineering_platform',
            'machine_learning_system', 'design_system_brand',
            'implementation_migration'
        ) THEN 'software_development'
        ELSE project_type
    END,
    updated_at = NOW();

ALTER TABLE company_projects
    ADD CONSTRAINT company_projects_type_check CHECK (project_type IN (
        'software_development', 'game_development', 'novel_writing',
        'general_writing', 'research', 'data_analysis', 'product_design',
        'marketing_content', 'documentation', 'automation', 'operations', 'general'
    ));
