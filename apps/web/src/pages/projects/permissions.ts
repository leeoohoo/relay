import { api } from "../../api/client";
import type { CompanyAgent, CompanyProject } from "../../types/platform";
import { PROJECT_PERMISSIONS, STAFFING_PERMISSIONS } from "../app/permissions";

export function activeProjectAgents(project: CompanyProject, agents: CompanyAgent[]) {
  const projectMemberIds = new Set(project.members.map((member) => member.agent_profile.id));
  return agents.filter((agent) =>
    projectMemberIds.has(agent.agent_profile.id)
    && agent.membership.employment_status === "active");
}

export function preferredProjectRuleAgentId(project: CompanyProject, projectAgents: CompanyAgent[]) {
  const lastRuleAgentId = project.rule?.updated_by_agent_id;
  if (lastRuleAgentId && projectAgents.some((agent) => agent.agent_profile.id === lastRuleAgentId)) {
    return lastRuleAgentId;
  }
  return projectAgents.find((agent) => agentHasPermission(agent, "project.rules.manage"))?.agent_profile.id
    ?? projectAgents[0]?.agent_profile.id
    ?? "";
}

export function agentHasPermission(agent: CompanyAgent, permission: string) {
  return agent.membership.permissions.includes(permission);
}

export async function grantAgentProjectPermission(companyId: string, agent: CompanyAgent, permission: string, token: string) {
  const staffingPermissions = agent.membership.permissions.filter((current) => STAFFING_PERMISSIONS.some((item) => item.key === current));
  const projectPermissions = agent.membership.permissions.filter((current) => PROJECT_PERMISSIONS.some((item) => item.key === current));
  if (!projectPermissions.includes(permission)) projectPermissions.push(permission);
  await api(
    `/api/v1/companies/${companyId}/agents/${agent.agent_profile.id}/permissions`,
    {
      method: "POST",
      body: JSON.stringify({
        staffing_permissions: staffingPermissions,
        project_permissions: projectPermissions,
        staffing_scope_org_unit_id: staffingPermissions.length ? agent.membership.staffing_scope_org_unit_id : null,
        reason: `Human console project delegation: grant ${permission}`,
      }),
    },
    token,
  );
}

