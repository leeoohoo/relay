CREATE TABLE project_environments (
    id uuid PRIMARY KEY,
    project_id uuid NOT NULL REFERENCES company_projects(id) ON DELETE CASCADE,
    environment_key text NOT NULL,
    display_name text NOT NULL,
    status text NOT NULL DEFAULT 'unknown',
    desired_revision text,
    observed_revision text,
    configuration_fingerprint text,
    health_summary_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    last_observed_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT project_environments_key_unique UNIQUE (project_id, environment_key),
    CONSTRAINT project_environments_key_check CHECK (
        char_length(environment_key) BETWEEN 1 AND 80
    ),
    CONSTRAINT project_environments_status_check CHECK (
        status IN ('unknown', 'provisioning', 'ready', 'degraded', 'offline')
    ),
    CONSTRAINT project_environments_health_object_check CHECK (
        jsonb_typeof(health_summary_json) = 'object'
    )
);

CREATE INDEX idx_project_environments_project_status
    ON project_environments(project_id, status, updated_at DESC);

CREATE TABLE project_environment_services (
    id uuid PRIMARY KEY,
    environment_id uuid NOT NULL REFERENCES project_environments(id) ON DELETE CASCADE,
    service_key text NOT NULL,
    desired_revision text,
    observed_revision text,
    image_digest text,
    configuration_fingerprint text,
    health_status text NOT NULL DEFAULT 'unknown',
    health_details_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    observed_at timestamptz NOT NULL,
    CONSTRAINT project_environment_services_key_unique UNIQUE (environment_id, service_key),
    CONSTRAINT project_environment_services_key_check CHECK (
        char_length(service_key) BETWEEN 1 AND 120
    ),
    CONSTRAINT project_environment_services_health_check CHECK (
        health_status IN ('unknown', 'healthy', 'unhealthy')
    ),
    CONSTRAINT project_environment_services_details_object_check CHECK (
        jsonb_typeof(health_details_json) = 'object'
    )
);

CREATE INDEX idx_project_environment_services_environment_health
    ON project_environment_services(environment_id, health_status, service_key);

CREATE TABLE project_task_environment_requirements (
    task_id uuid NOT NULL REFERENCES company_project_tasks(id) ON DELETE CASCADE,
    environment_id uuid NOT NULL REFERENCES project_environments(id) ON DELETE CASCADE,
    required_revision text,
    required_services_json jsonb NOT NULL DEFAULT '[]'::jsonb,
    require_healthy boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (task_id, environment_id),
    CONSTRAINT project_task_environment_services_array_check CHECK (
        jsonb_typeof(required_services_json) = 'array'
    )
);

CREATE INDEX idx_project_task_environment_requirements_environment
    ON project_task_environment_requirements(environment_id, task_id);
