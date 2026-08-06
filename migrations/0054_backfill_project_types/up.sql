UPDATE company_projects
SET project_type = CASE
        WHEN lower(name || ' ' || description) ~ '(游戏|game|godot|unity|unreal|关卡|玩法)' THEN 'game_development'
        WHEN lower(name || ' ' || description) ~ '(小说|novel|章节|人物设定|世界观|大纲)' THEN 'novel_writing'
        WHEN lower(name || ' ' || description) ~ '(数据分析|data analysis|notebook|数据集|报表)' THEN 'data_analysis'
        WHEN lower(name || ' ' || description) ~ '(研究|调研|research|文献|竞品分析)' THEN 'research'
        WHEN lower(name || ' ' || description) ~ '(产品设计|交互设计|ui design|ux|figma|原型|设计系统)' THEN 'product_design'
        WHEN lower(name || ' ' || description) ~ '(营销|市场|campaign|marketing|社媒|增长|品牌传播)' THEN 'marketing_content'
        WHEN lower(name || ' ' || description) ~ '(文档|documentation|知识库|用户手册|api 文档)' THEN 'documentation'
        WHEN lower(name || ' ' || description) ~ '(自动化|automation|agent|workflow|mcp|机器人|爬虫)' THEN 'automation'
        WHEN lower(name || ' ' || description) ~ '(运营|运维|operations|migration|迁移|上线|交付)' THEN 'operations'
        WHEN lower(name || ' ' || description) ~ '(软件|开发|website|web app|api|frontend|backend|前端|后端|移动端)' THEN 'software_development'
        WHEN lower(name || ' ' || description) ~ '(写作|文章|文案|演讲稿|essay|article|script|稿件)' THEN 'general_writing'
        ELSE project_type
    END,
    project_type_source = CASE
        WHEN project_type = 'general' THEN 'description_inference'
        ELSE project_type_source
    END,
    project_type_confidence = CASE
        WHEN project_type = 'general' THEN 72
        ELSE project_type_confidence
    END,
    updated_at = NOW()
WHERE project_type = 'general'
  AND lower(name || ' ' || description) ~ '(游戏|game|godot|unity|unreal|关卡|玩法|小说|novel|章节|人物设定|世界观|大纲|数据分析|data analysis|notebook|数据集|报表|研究|调研|research|文献|竞品分析|产品设计|交互设计|ui design|ux|figma|原型|设计系统|营销|市场|campaign|marketing|社媒|增长|品牌传播|文档|documentation|知识库|用户手册|api 文档|自动化|automation|agent|workflow|mcp|机器人|爬虫|运营|运维|operations|migration|迁移|上线|交付|软件|开发|website|web app|api|frontend|backend|前端|后端|移动端|写作|文章|文案|演讲稿|essay|article|script|稿件)';
