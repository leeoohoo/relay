CREATE TABLE companies (
    id UUID PRIMARY KEY,
    owner_user_id UUID NOT NULL REFERENCES human_users(id) ON DELETE RESTRICT,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'suspended', 'archived')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_companies_owner_created_at
    ON companies(owner_user_id, created_at DESC);

CREATE TABLE company_human_members (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    human_user_id UUID NOT NULL REFERENCES human_users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'viewer')),
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'invited', 'removed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (company_id, human_user_id)
);

CREATE INDEX idx_company_human_members_user_status
    ON company_human_members(human_user_id, status, created_at DESC);

CREATE TABLE org_units (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    parent_org_unit_id UUID REFERENCES org_units(id) ON DELETE RESTRICT,
    name TEXT NOT NULL,
    unit_type TEXT NOT NULL
        CHECK (unit_type IN ('company', 'division', 'department', 'team')),
    sort_order INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'archived')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (company_id, parent_org_unit_id, name)
);

CREATE INDEX idx_org_units_company_parent_sort
    ON org_units(company_id, parent_org_unit_id, sort_order, created_at);

CREATE TABLE company_agent_memberships (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    agent_profile_id UUID NOT NULL REFERENCES agent_profiles(id) ON DELETE RESTRICT,
    org_unit_id UUID NOT NULL REFERENCES org_units(id) ON DELETE RESTRICT,
    job_title TEXT NOT NULL DEFAULT '',
    role_key TEXT NOT NULL DEFAULT 'member'
        CHECK (role_key IN ('company_manager', 'member')),
    reports_to_membership_id UUID REFERENCES company_agent_memberships(id) ON DELETE SET NULL,
    permissions JSONB NOT NULL DEFAULT '[]'::jsonb,
    employment_status TEXT NOT NULL DEFAULT 'active'
        CHECK (employment_status IN ('provisioning', 'active', 'suspended', 'terminated')),
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    terminated_at TIMESTAMPTZ,
    created_by_human_user_id UUID REFERENCES human_users(id) ON DELETE SET NULL,
    created_by_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (agent_profile_id)
);

CREATE INDEX idx_company_agent_memberships_company_status
    ON company_agent_memberships(company_id, employment_status, joined_at DESC);

CREATE INDEX idx_company_agent_memberships_org_status
    ON company_agent_memberships(org_unit_id, employment_status, joined_at DESC);

CREATE INDEX idx_company_agent_memberships_reports_to
    ON company_agent_memberships(reports_to_membership_id)
    WHERE reports_to_membership_id IS NOT NULL;

INSERT INTO companies (
    id, owner_user_id, name, slug, description, status, created_at, updated_at
)
SELECT
    gen_random_uuid(),
    hu.id,
    hu.display_name || ' 的公司',
    'company-' || replace(hu.id::text, '-', ''),
    '由历史 Owner 数据自动创建的默认公司',
    'active',
    hu.created_at,
    NOW()
FROM human_users hu
WHERE NOT EXISTS (
    SELECT 1 FROM companies c WHERE c.owner_user_id = hu.id
);

INSERT INTO company_human_members (
    id, company_id, human_user_id, role, status, created_at, updated_at
)
SELECT
    gen_random_uuid(), c.id, c.owner_user_id, 'owner', 'active', c.created_at, NOW()
FROM companies c
ON CONFLICT (company_id, human_user_id) DO NOTHING;

INSERT INTO org_units (
    id, company_id, parent_org_unit_id, name, unit_type, sort_order, status, created_at, updated_at
)
SELECT
    gen_random_uuid(), c.id, NULL, c.name, 'company', 0, 'active', c.created_at, NOW()
FROM companies c
WHERE NOT EXISTS (
    SELECT 1 FROM org_units ou
    WHERE ou.company_id = c.id AND ou.parent_org_unit_id IS NULL
);

WITH ranked_agents AS (
    SELECT
        ap.*,
        row_number() OVER (PARTITION BY ap.owner_user_id ORDER BY ap.created_at, ap.id) AS company_rank
    FROM agent_profiles ap
)
INSERT INTO company_agent_memberships (
    id,
    company_id,
    agent_profile_id,
    org_unit_id,
    job_title,
    role_key,
    permissions,
    employment_status,
    joined_at,
    created_by_human_user_id,
    updated_at
)
SELECT
    gen_random_uuid(),
    c.id,
    ap.id,
    root_unit.id,
    CASE WHEN ap.company_rank = 1 THEN '公司管理 Agent' ELSE '公司 Agent' END,
    CASE WHEN ap.company_rank = 1 THEN 'company_manager' ELSE 'member' END,
    CASE
        WHEN ap.company_rank = 1 THEN
            '["company.read","org.read","agent.directory.read","agent.communicate","project.create","project.manage","task.assign","task.update","message.send"]'::jsonb
        ELSE
            '["company.read","org.read","agent.directory.read","agent.communicate","task.update","message.send"]'::jsonb
    END,
    CASE
        WHEN ap.status = 'active' THEN 'active'
        ELSE 'suspended'
    END,
    ap.created_at,
    ap.owner_user_id,
    NOW()
FROM ranked_agents ap
INNER JOIN companies c ON c.owner_user_id = ap.owner_user_id
INNER JOIN org_units root_unit
    ON root_unit.company_id = c.id
   AND root_unit.parent_org_unit_id IS NULL
WHERE NOT EXISTS (
    SELECT 1 FROM company_agent_memberships cam
    WHERE cam.agent_profile_id = ap.id
);
