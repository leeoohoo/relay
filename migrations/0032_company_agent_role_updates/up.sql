ALTER TABLE agent_staffing_actions
    DROP CONSTRAINT agent_staffing_actions_action_type_check;

ALTER TABLE agent_staffing_actions
    ADD CONSTRAINT agent_staffing_actions_action_type_check
    CHECK (action_type IN (
        'permission_update', 'role_update', 'hire', 'activate', 'suspend', 'reactivate', 'terminate'
    ));
