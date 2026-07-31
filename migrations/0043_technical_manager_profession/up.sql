UPDATE company_agent_memberships
SET permissions = CASE
        WHEN permissions ? 'task.assign' THEN permissions
        ELSE permissions || '["task.assign"]'::jsonb
    END,
    updated_at = NOW()
WHERE job_title LIKE '%技术经理%'
   OR job_title LIKE '%技术负责人%'
   OR job_title LIKE '%工程经理%'
   OR job_title LIKE '%研发经理%'
   OR job_title LIKE '%技术总监%'
   OR LOWER(job_title) IN ('cto', 'technical manager', 'engineering manager', 'tech lead')
   OR LOWER(job_title) LIKE '%technical manager%'
   OR LOWER(job_title) LIKE '%engineering manager%'
   OR LOWER(job_title) LIKE '%tech lead%';
