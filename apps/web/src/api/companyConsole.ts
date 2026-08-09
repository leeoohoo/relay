import { api } from "./client";
import type { CompanyConsole } from "../types/platform";

export type CompanyConsoleRegion = "summary" | "agents" | "conversations" | "projects";

const ALL_REGIONS: CompanyConsoleRegion[] = ["summary", "agents", "conversations", "projects"];

type CompanySummary = Pick<
  CompanyConsole,
  "company" | "human_membership" | "org_units" | "professions" | "project_types" | "governance_policy"
>;

export function companyConsoleRegionsForEvent(eventType: string): CompanyConsoleRegion[] {
  if (eventType.startsWith("agent.runtime.approval_")) return [];
  if (eventType.startsWith("message.")) return ["conversations"];
  if (eventType.startsWith("project.task.") || eventType.startsWith("project.status.")) {
    return ["projects"];
  }
  if (eventType === "project.created" || eventType.startsWith("project.member.")) {
    return ["projects", "conversations"];
  }
  if (eventType.startsWith("project.")) return ["projects"];
  if (eventType.startsWith("staffing.") || eventType.startsWith("agent.")) return ["agents"];
  if (eventType.startsWith("company.")) return ["summary"];
  return ALL_REGIONS;
}

export async function fetchCompanyConsole(
  companyId: string,
  token: string,
): Promise<CompanyConsole> {
  const patch = await fetchCompanyConsoleRegions(companyId, token, ALL_REGIONS);
  return patch as CompanyConsole;
}

export async function fetchCompanyConsoleRegions(
  companyId: string,
  token: string,
  regions: Iterable<CompanyConsoleRegion>,
): Promise<Partial<CompanyConsole>> {
  const requested = new Set(regions);
  const patch: Partial<CompanyConsole> = {};
  await Promise.all([
    requested.has("summary")
      ? api<{ summary: CompanySummary }>(`/api/v1/companies/${companyId}/summary`, {}, token)
          .then(({ summary }) => Object.assign(patch, summary))
      : Promise.resolve(),
    requested.has("agents")
      ? api<{ agents: CompanyConsole["agents"] }>(`/api/v1/companies/${companyId}/agents`, {}, token)
          .then(({ agents }) => { patch.agents = agents; })
      : Promise.resolve(),
    requested.has("conversations")
      ? api<{ conversations: CompanyConsole["conversations"] }>(`/api/v1/companies/${companyId}/conversations`, {}, token)
          .then(({ conversations }) => { patch.conversations = conversations; })
      : Promise.resolve(),
    requested.has("projects")
      ? api<{ projects: CompanyConsole["projects"] }>(`/api/v1/companies/${companyId}/projects`, {}, token)
          .then(({ projects }) => { patch.projects = projects; })
      : Promise.resolve(),
  ]);
  return patch;
}
