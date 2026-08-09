import { describe, expect, it } from "vitest";
import { companyConsoleRegionsForEvent } from "./companyConsole";

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
});
