import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CompanyAgent, CompanyConsole } from "../../types/platform";
import { api } from "../../api/client";
import { AgentRow } from "./agents";

vi.mock("../../api/client", () => ({ api: vi.fn(async () => ({})) }));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

function agentWithPermissions(permissions: string[]): CompanyAgent {
  return {
    agent_profile: {
      id: "agent-1",
      display_name: "林澈",
      handle: "lingche",
      persona: "负责公司管理",
      collaboration_preference: "available",
      status: "active",
      created_at: "2026-08-12T00:00:00Z",
    },
    membership: {
      id: "membership-1",
      agent_profile_id: "agent-1",
      org_unit_id: "org-1",
      job_title: "通用成员",
      role_key: "company_manager",
      permissions,
      responsibilities: [],
      skills: [],
      current_focus: "",
      staffing_scope_org_unit_id: null,
      employment_status: "active",
      reports_to_membership_id: null,
    },
    connection: {
      status: "connected",
      key_prefix: null,
      key_created_at: null,
      key_expires_at: null,
      last_used_at: null,
    },
  };
}

function consoleWithAgent(agent: CompanyAgent): CompanyConsole {
  return {
    company: { id: "company-1" },
    agents: [agent],
    org_units: [{ id: "org-1", name: "总部" }],
    professions: [{
      key: "general_member",
      label: "通用成员",
      label_en: "General Member",
      category_label: "通用",
      category_label_en: "General",
      description: "处理明确分配的工作",
      description_en: "Handle assigned work",
      can_create_tasks: false,
    }],
    governance_policy: { effective_settings: { skill_language: "zh" } },
  } as unknown as CompanyConsole;
}

describe("AgentRow permission editing", () => {
  it("preserves an unsaved permission selection across realtime console refreshes", async () => {
    const initialAgent = agentWithPermissions([]);
    const props = {
      consoleData: consoleWithAgent(initialAgent),
      token: "human-token",
      onChanged: vi.fn(async () => undefined),
      onError: vi.fn(),
      onNotice: vi.fn(),
    };
    const view = render(<AgentRow agent={initialAgent} {...props} />);

    fireEvent.click(screen.getByTitle("展开画像与权限"));
    const hireCheckbox = screen.getByRole("checkbox", { name: "扩招 Agent" });
    fireEvent.click(hireCheckbox);
    expect(hireCheckbox).toBeChecked();

    const refreshedAgent = agentWithPermissions([]);
    view.rerender(
      <AgentRow
        agent={refreshedAgent}
        {...props}
        consoleData={consoleWithAgent(refreshedAgent)}
      />,
    );

    expect(screen.getByRole("checkbox", { name: "扩招 Agent" })).toBeChecked();
    fireEvent.click(screen.getAllByRole("button", { name: "保存授权" })[0]);

    await waitFor(() => expect(api).toHaveBeenCalled());
    const [, request] = vi.mocked(api).mock.calls[0];
    expect(JSON.parse(String(request?.body))).toMatchObject({
      staffing_permissions: ["agent.staff.hire"],
      staffing_scope_org_unit_id: null,
    });
  });
});
