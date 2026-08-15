use super::*;

pub(crate) fn resolve_effective_cli_settings(
    trigger: &AgentCodexTriggerConfig,
    company: &CompanyCodexCliSettings,
) -> EffectiveCodexCliSettings {
    EffectiveCodexCliSettings {
        model: trigger.model.clone().or_else(|| company.model.clone()),
        reasoning_effort: trigger
            .reasoning_effort
            .clone()
            .or_else(|| company.reasoning_effort.clone()),
        reasoning_summary: trigger
            .reasoning_summary
            .clone()
            .or_else(|| Some(company.reasoning_summary.clone())),
        verbosity: trigger
            .verbosity
            .clone()
            .or_else(|| company.verbosity.clone()),
        personality: trigger
            .personality
            .clone()
            .or_else(|| company.personality.clone()),
        service_tier: company.service_tier.clone(),
        sandbox_mode: if trigger.sandbox_mode == AGENT_CODEX_SETTING_INHERIT {
            company.sandbox_mode.clone()
        } else {
            trigger.sandbox_mode.clone()
        },
        approval_policy: if trigger.approval_policy == AGENT_CODEX_SETTING_INHERIT {
            company.approval_policy.clone()
        } else {
            trigger.approval_policy.clone()
        },
        network_access: company.network_access,
        web_search: company.web_search.clone(),
        feature_multi_agent: company.feature_multi_agent,
        feature_remote_plugin: company.feature_remote_plugin,
        feature_hooks: company.feature_hooks,
        feature_goals: company.feature_goals,
        feature_shell_tool: company.feature_shell_tool,
    }
}
