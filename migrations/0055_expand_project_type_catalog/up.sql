ALTER TABLE company_projects
    DROP CONSTRAINT IF EXISTS company_projects_type_check;

ALTER TABLE company_projects
    ADD CONSTRAINT company_projects_type_check CHECK (project_type IN (
        'software_development', 'web_application', 'mobile_application',
        'desktop_application', 'backend_service', 'library_sdk',
        'game_development', 'iot_embedded_system', 'enterprise_erp',
        'warehouse_management_system', 'customer_relationship_management',
        'manufacturing_execution_system', 'ecommerce_platform',
        'novel_writing', 'general_writing', 'research', 'data_analysis',
        'data_engineering_platform', 'machine_learning_system',
        'product_design', 'design_system_brand', 'marketing_content',
        'documentation', 'automation', 'operations',
        'implementation_migration', 'general'
    ));

UPDATE company_projects
SET project_type = CASE
        WHEN lower(name || ' ' || COALESCE(description, '')) ~ '(\mwms\M|仓储管理|仓库管理|warehouse management|波次|拣选|库位|盘点)' THEN 'warehouse_management_system'
        WHEN lower(name || ' ' || COALESCE(description, '')) ~ '(\merp\M|erpnext|odoo|企业资源计划|财务供应链|总账|关账)' THEN 'enterprise_erp'
        WHEN lower(name || ' ' || COALESCE(description, '')) ~ '(\mcrm\M|客户关系管理|销售漏斗|销售线索|商机|客户成功)' THEN 'customer_relationship_management'
        WHEN lower(name || ' ' || COALESCE(description, '')) ~ '(\mmes\M|制造执行|生产执行|在制品|工艺路线|\moee\M|生产工单)' THEN 'manufacturing_execution_system'
        WHEN lower(name || ' ' || COALESCE(description, '')) ~ '(e-?commerce|电商平台|商城|购物车|checkout|订单履约)' THEN 'ecommerce_platform'
        WHEN lower(name || ' ' || COALESCE(description, '')) ~ '(flutter|react.?native|移动端应用|mobile app|android|ios)' THEN 'mobile_application'
        WHEN lower(name || ' ' || COALESCE(description, '')) ~ '(electron|tauri|桌面应用|desktop app|桌面端)' THEN 'desktop_application'
        WHEN lower(name || ' ' || COALESCE(description, '')) ~ '(嵌入式|firmware|embedded|esp32|stm32|\miot\M|mqtt|modbus|plc)' THEN 'iot_embedded_system'
        WHEN lower(name || ' ' || COALESCE(description, '')) ~ '(数据工程|数据平台|data warehouse|数据仓库|lakehouse|airflow|\mdbt\M|spark|\metl\M|\melt\M)' THEN 'data_engineering_platform'
        WHEN lower(name || ' ' || COALESCE(description, '')) ~ '(机器学习|machine learning|模型训练|模型推理|pytorch|tensorflow|mlflow)' THEN 'machine_learning_system'
        WHEN lower(name || ' ' || COALESCE(description, '')) ~ '(设计系统|design system|design token|storybook|组件库|品牌规范)' THEN 'design_system_brand'
        WHEN lower(name || ' ' || COALESCE(description, '')) ~ '(系统实施|数据迁移|cutover|上线切换|\muat\M|迁移演练)' THEN 'implementation_migration'
        WHEN lower(name || ' ' || COALESCE(description, '')) ~ '(微服务|microservice|后端服务|openapi|swagger|backend)' THEN 'backend_service'
        WHEN lower(name || ' ' || COALESCE(description, '')) ~ '(\msdk\M|公共库|开发工具|npm package|component library)' THEN 'library_sdk'
        WHEN lower(name || ' ' || COALESCE(description, '')) ~ '(web app|web 应用|管理后台|website|frontend|前端|网页)' THEN 'web_application'
        ELSE project_type
    END,
    project_type_source = CASE
        WHEN project_type_source = 'system_default' THEN 'description_inference'
        ELSE project_type_source
    END,
    project_type_confidence = CASE
        WHEN project_type_confidence < 80 THEN 80
        ELSE project_type_confidence
    END,
    updated_at = NOW()
WHERE project_type IN ('general', 'software_development')
  AND lower(name || ' ' || COALESCE(description, '')) ~ '(\mwms\M|仓储管理|仓库管理|warehouse management|波次|拣选|库位|盘点|\merp\M|erpnext|odoo|企业资源计划|财务供应链|总账|关账|\mcrm\M|客户关系管理|销售漏斗|销售线索|商机|客户成功|\mmes\M|制造执行|生产执行|在制品|工艺路线|\moee\M|生产工单|e-?commerce|电商平台|商城|购物车|checkout|订单履约|flutter|react.?native|移动端应用|mobile app|android|ios|electron|tauri|桌面应用|desktop app|桌面端|嵌入式|firmware|embedded|esp32|stm32|\miot\M|mqtt|modbus|plc|数据工程|数据平台|data warehouse|数据仓库|lakehouse|airflow|\mdbt\M|spark|\metl\M|\melt\M|机器学习|machine learning|模型训练|模型推理|pytorch|tensorflow|mlflow|设计系统|design system|design token|storybook|组件库|品牌规范|系统实施|数据迁移|cutover|上线切换|\muat\M|迁移演练|微服务|microservice|后端服务|openapi|swagger|backend|\msdk\M|公共库|开发工具|npm package|component library|web app|web 应用|管理后台|website|frontend|前端|网页)';
