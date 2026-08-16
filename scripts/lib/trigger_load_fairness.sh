#!/usr/bin/env bash

run_company_fairness_scenario() {
  post_json "/api/v1/companies/$COMPANY_ID/agent-trigger-preferences" \
    '{"batch_size":2}' -X PUT -H "authorization: Bearer $OWNER_TOKEN" >/dev/null
  sleep 0.5

  local candidates="'${AGENT_IDS[5]}','${AGENT_IDS[6]}','${FAIR_AGENT_IDS[0]}','${FAIR_AGENT_IDS[1]}'"
  local marker sample_file first_wave_companies
  marker="$(pg_query 'select clock_timestamp();')"
  sample_file="$TEMP_ROOT/fairness.samples"
  : >"$sample_file"
  pg_query "
    update agent_codex_trigger_configs
    set status = 'active', next_run_at = timestamptz '$marker',
        manual_run_requested_at = timestamptz '$marker', wake_requested_at = null,
        wake_reason = null, lease_owner = null, lease_expires_at = null
    where agent_profile_id in ($candidates);
  " >/dev/null
  post_json "/api/v1/companies/$COMPANY_ID/agents/${AGENT_IDS[5]}/codex-trigger/run-now" \
    '{}' -H "authorization: Bearer $OWNER_TOKEN" >/dev/null
  wait_for_runs "$marker" 4 "$sample_file"

  first_wave_companies="$(pg_query "
    with first_wave as (
      select config.company_id
      from agent_codex_trigger_runs run
      inner join agent_codex_trigger_configs config on config.id = run.trigger_config_id
      where run.started_at >= timestamptz '$marker'
        and run.agent_profile_id in ($candidates)
      order by run.started_at, run.id
      limit 2
    )
    select count(distinct company_id) from first_wave;
  ")"
  if [[ "$first_wave_companies" != "2" ]]; then
    echo "Company fairness failed: the first two claimed Agents came from one company" >&2
    return 1
  fi

  post_json "/api/v1/companies/$COMPANY_ID/agent-trigger-preferences" \
    '{"batch_size":10}' -X PUT -H "authorization: Bearer $OWNER_TOKEN" >/dev/null
  jq -cn \
    --argjson agents 4 \
    --argjson first_wave_distinct_companies "$first_wave_companies" \
    '{fairness_agents:$agents,first_wave_distinct_companies:$first_wave_distinct_companies}'
}
