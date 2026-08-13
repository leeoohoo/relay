import { api } from "./client";
import type { CompanyConsole, CompanyConsolePageState } from "../types/platform";

export type CompanyConsoleRegion = "summary" | "agents" | "conversations" | "projects";

const ALL_REGIONS: CompanyConsoleRegion[] = ["summary", "agents", "conversations", "projects"];

type CompanySummary = Pick<
  CompanyConsole,
  "company" | "human_membership" | "org_units" | "professions" | "project_types" | "governance_policy"
>;

type PagedRegion = Exclude<CompanyConsoleRegion, "summary">;
type RegionPagination = Partial<Record<PagedRegion, CompanyConsolePageState>>;
export type CompanyConsoleRegionPatch = {
  data: Partial<CompanyConsole>;
  pagination: RegionPagination;
};

type RegionResponse<T> = {
  next_cursor: string | null;
  has_more: boolean;
} & T;

export function companyConsoleRegionsForEvent(eventType: string): CompanyConsoleRegion[] {
  if (eventType.startsWith("codex.")) return [];
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
  return {
    ...patch.data,
    pagination: {
      agents: patch.pagination.agents ?? emptyPage(),
      conversations: patch.pagination.conversations ?? emptyPage(),
      projects: patch.pagination.projects ?? emptyPage(),
    },
  } as CompanyConsole;
}

export async function fetchCompanyConsoleRegions(
  companyId: string,
  token: string,
  regions: Iterable<CompanyConsoleRegion>,
): Promise<CompanyConsoleRegionPatch> {
  const requested = new Set(regions);
  const patch: Partial<CompanyConsole> = {};
  const pagination: RegionPagination = {};
  await Promise.all([
    requested.has("summary")
      ? api<{ summary: CompanySummary }>(`/api/v1/companies/${companyId}/summary`, {}, token)
          .then(({ summary }) => Object.assign(patch, summary))
      : Promise.resolve(),
    requested.has("agents")
      ? fetchCompanyConsoleRegionPage(companyId, token, "agents")
          .then((page) => { patch.agents = page.items; pagination.agents = page.page; })
      : Promise.resolve(),
    requested.has("conversations")
      ? fetchCompanyConsoleRegionPage(companyId, token, "conversations")
          .then((page) => { patch.conversations = page.items; pagination.conversations = page.page; })
      : Promise.resolve(),
    requested.has("projects")
      ? fetchCompanyConsoleRegionPage(companyId, token, "projects")
          .then((page) => { patch.projects = page.items; pagination.projects = page.page; })
      : Promise.resolve(),
  ]);
  return { data: patch, pagination };
}

export function fetchCompanyConsoleRegionPage(
  companyId: string,
  token: string,
  region: "agents",
  after?: string | null,
): Promise<{ items: CompanyConsole["agents"]; page: CompanyConsolePageState }>;
export function fetchCompanyConsoleRegionPage(
  companyId: string,
  token: string,
  region: "conversations",
  after?: string | null,
): Promise<{ items: CompanyConsole["conversations"]; page: CompanyConsolePageState }>;
export function fetchCompanyConsoleRegionPage(
  companyId: string,
  token: string,
  region: "projects",
  after?: string | null,
): Promise<{ items: CompanyConsole["projects"]; page: CompanyConsolePageState }>;
export async function fetchCompanyConsoleRegionPage(
  companyId: string,
  token: string,
  region: PagedRegion,
  after?: string | null,
): Promise<{ items: CompanyConsole[PagedRegion]; page: CompanyConsolePageState }> {
  const params = new URLSearchParams({ limit: region === "projects" ? "12" : "20" });
  if (after) params.set("after", after);
  const response = await api<RegionResponse<Record<string, CompanyConsole[PagedRegion]>>>(
    `/api/v1/companies/${companyId}/${region}?${params.toString()}`,
    {},
    token,
  );
  return {
    items: response[region],
    page: { next_cursor: response.next_cursor, has_more: response.has_more },
  };
}

function emptyPage(): CompanyConsolePageState {
  return { next_cursor: null, has_more: false };
}
