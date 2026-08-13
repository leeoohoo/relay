import { afterEach, describe, expect, it, vi } from "vitest";
import { companyConsoleRegionsForEvent, fetchCompanyConsoleRegionPage } from "./companyConsole";

afterEach(() => vi.unstubAllGlobals());

describe("companyConsoleRegionsForEvent", () => {
  it("refreshes only the conversation region for messages", () => {
    expect(companyConsoleRegionsForEvent("message.created")).toEqual(["conversations"]);
  });

  it("refreshes project data without reloading agents for task changes", () => {
    expect(companyConsoleRegionsForEvent("project.task.updated")).toEqual(["projects"]);
  });

  it("refreshes project and conversation regions when membership changes", () => {
    expect(companyConsoleRegionsForEvent("project.member.added")).toEqual([
      "projects",
      "conversations",
    ]);
  });

  it("leaves approval refresh to the dedicated approval query", () => {
    expect(companyConsoleRegionsForEvent("agent.runtime.approval_requested")).toEqual([]);
  });

  it("leaves Codex runtime events to the runtime panels", () => {
    expect(companyConsoleRegionsForEvent("codex.run.updated")).toEqual([]);
    expect(companyConsoleRegionsForEvent("codex.trigger.updated")).toEqual([]);
  });

  it("requests bounded cursor pages and preserves continuation state", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({
      agents: [],
      next_cursor: "membership-20",
      has_more: true,
    }), { status: 200, headers: { "content-type": "application/json" } }));
    vi.stubGlobal("fetch", fetchMock);

    const page = await fetchCompanyConsoleRegionPage(
      "company-1",
      "session-token",
      "agents",
      "membership-10",
    );

    expect(fetchMock.mock.calls[0]?.[0]).toContain(
      "/api/v1/companies/company-1/agents?limit=20&after=membership-10",
    );
    expect(page.page).toEqual({ next_cursor: "membership-20", has_more: true });
  });
});
