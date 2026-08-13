import { api } from "./client";
import type { CodexSession, CodexTriggerView } from "../types/platform";

export type AgentCodexRuntimeOverview = {
  agent_id: string;
  trigger: CodexTriggerView | null;
  sessions: CodexSession[];
};

export function fetchCodexRuntimeOverview(
  companyId: string,
  agentIds: string[],
  token: string,
  projectId?: string,
) {
  const params = new URLSearchParams({ agent_ids: agentIds.join(",") });
  if (projectId) params.set("project_id", projectId);
  return api<{ agents: AgentCodexRuntimeOverview[] }>(
    `/api/v1/companies/${companyId}/codex-runtime-overview?${params.toString()}`,
    {},
    token,
  );
}
