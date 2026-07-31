DROP TRIGGER IF EXISTS trg_agent_staffing_actions_realtime_insert ON agent_staffing_actions;
DROP FUNCTION IF EXISTS emit_staffing_realtime_event();

DROP TRIGGER IF EXISTS trg_company_project_status_realtime_insert ON company_project_status_updates;
DROP FUNCTION IF EXISTS emit_project_status_realtime_event();

DROP TRIGGER IF EXISTS trg_company_project_tasks_realtime_update ON company_project_tasks;
DROP TRIGGER IF EXISTS trg_company_project_tasks_realtime_insert ON company_project_tasks;
DROP FUNCTION IF EXISTS emit_project_task_realtime_event();

DROP TRIGGER IF EXISTS trg_company_project_members_realtime_update ON company_project_members;
DROP TRIGGER IF EXISTS trg_company_project_members_realtime_insert ON company_project_members;
DROP FUNCTION IF EXISTS emit_project_member_realtime_event();

DROP TRIGGER IF EXISTS trg_company_projects_realtime_update ON company_projects;
DROP TRIGGER IF EXISTS trg_company_projects_realtime_insert ON company_projects;
DROP FUNCTION IF EXISTS emit_project_realtime_event();

DROP TRIGGER IF EXISTS trg_messages_realtime_event ON messages;
DROP FUNCTION IF EXISTS emit_message_realtime_event();

DROP TRIGGER IF EXISTS trg_realtime_events_notify ON realtime_events;
DROP FUNCTION IF EXISTS notify_realtime_event_insert();

DROP TABLE IF EXISTS realtime_events;
