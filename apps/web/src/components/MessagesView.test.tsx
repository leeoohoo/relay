import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../api/client";
import type { CompanyConsole, CodexTriggerView } from "../types/platform";
import { MessagesView } from "./MessagesView";

vi.mock("../api/client", () => ({ api: vi.fn() }));

const mockedApi = vi.mocked(api);

const trigger: CodexTriggerView = {
  runner_profile_id: "profile-1",
  config: {
    id: "trigger-1",
    status: "active",
    interval_seconds: 60,
    codex_profile: "default",
    model: null,
    reasoning_effort: null,
    reasoning_summary: null,
    verbosity: null,
    personality: null,
    service_tier: null,
    sandbox_mode: "inherit",
    approval_policy: "inherit",
    network_access: null,
    web_search: null,
    feature_multi_agent: null,
    feature_remote_plugin: null,
    feature_hooks: null,
    feature_goals: null,
    feature_shell_tool: null,
    max_run_seconds: 900,
    next_run_at: "2026-08-07T03:10:00Z",
    lease_owner: "trigger-1",
    lease_expires_at: "2026-08-07T03:20:00Z",
    manual_run_requested_at: null,
    wake_requested_at: null,
    wake_reason: null,
    last_run_at: "2026-08-07T03:00:00Z",
    last_success_at: null,
    last_error: null,
    consecutive_failure_count: 0,
  },
  recent_runs: [{
    id: "run-1",
    trigger_type: "task",
    status: "running",
    project_id: "project-1",
    codex_thread_id: "thread-1",
    started_at: "2026-08-07T03:00:00Z",
    finished_at: null,
    final_message_summary: null,
    error_message: null,
    activity_phase: "files",
    activity_summary: "正在修改库存页面",
    last_activity_at: "2026-08-07T03:02:00Z",
    activity_log: [
      { at: "2026-08-07T03:00:00Z", phase: "planning", summary: "拆分实现步骤" },
      { at: "2026-08-07T03:02:00Z", phase: "files", summary: "修改库存页面" },
    ],
  }],
};

const consoleData: CompanyConsole = {
  pagination: {
    agents: { next_cursor: null, has_more: false },
    conversations: { next_cursor: null, has_more: false },
    projects: { next_cursor: null, has_more: false },
  },
  company: { id: "company-1", name: "Relay", slug: "relay", description: "" },
  human_membership: { role: "owner", status: "active" },
  org_units: [],
  agents: [{
    agent_profile: {
      id: "agent-1",
      display_name: "前端 Agent",
      handle: "@frontend",
      persona: "",
      collaboration_preference: "available",
      status: "active",
      created_at: "2026-08-07T01:00:00Z",
    },
    membership: {
      id: "membership-1",
      agent_profile_id: "agent-1",
      org_unit_id: "org-1",
      job_title: "前端工程师",
      role_key: "frontend_engineer",
      permissions: [],
      responsibilities: [],
      skills: [],
      current_focus: "库存页面",
      staffing_scope_org_unit_id: null,
      employment_status: "active",
      reports_to_membership_id: null,
    },
    connection: {
      status: "connected",
      key_prefix: "relay",
      key_created_at: null,
      key_expires_at: null,
      last_used_at: null,
    },
  }],
  conversations: [{
    preview: { id: "conversation-1", title: "项目 · WMS", conversation_type: "group", last_message_preview: null, updated_at: "2026-08-07T03:00:00Z" },
    context: { context_type: "project", project_id: "project-1" },
    member_agent_ids: ["agent-1"],
  }, {
    preview: { id: "conversation-2", title: "Owner ↔ 前端 Agent", conversation_type: "direct", last_message_preview: null, updated_at: "2026-08-07T03:01:00Z" },
    context: { context_type: "company_direct", project_id: null },
    member_agent_ids: ["agent-1"],
  }],
  projects: [{
    project: { id: "project-1", name: "WMS", description: "", project_type: "wms", project_type_source: "human", project_type_confidence: 1, project_type_evidence: [], status: "active", owner_agent_id: "agent-1" },
    git: null,
    rule: null,
    assets: [],
    asset_refresh: null,
    members: [],
    tasks: [{
      id: "task-1",
      project_id: "project-1",
      title: "实现库存工作台",
      description: "",
      status: "in_progress",
      priority: "high",
      assignee_agent_id: "agent-1",
      created_by_agent_id: null,
      created_by_human_user_id: "human-1",
      updated_by_agent_id: null,
      updated_by_human_user_id: "human-1",
      due_at: null,
      completed_at: null,
      created_at: "2026-08-07T02:00:00Z",
      updated_at: "2026-08-07T03:00:00Z",
    }],
    task_dependencies: [],
    task_status_history: [],
  }],
  professions: [],
  project_types: [],
  governance_policy: { effective_settings: { managed_workspace_root: null, skill_language: "zh-CN" } },
};

beforeEach(() => {
  mockedApi.mockReset();
  mockedApi.mockImplementation(async (path) => {
    if (path.startsWith("/api/v1/conversations/conversation-1/messages")) return { messages: [], next_cursor: null, has_more: false };
    if (path.startsWith("/api/v1/conversations/conversation-2/messages")) return { messages: [], next_cursor: null, has_more: false };
    if (path.endsWith("/agents/agent-1/codex-trigger")) return { trigger };
    throw new Error(`unexpected request: ${path}`);
  });
});

afterEach(cleanup);

describe("MessagesView group member runtime drawer", () => {
  it("opens the current project directory and task list without leaving chat", async () => {
    const { container } = render(
      <MessagesView
        consoleData={consoleData}
        humanUser={{ id: "human-1", email: "owner@example.com", display_name: "Lee" }}
        token="token"
        realtimeEvent={null}
        onChanged={async () => undefined}
        onError={() => undefined}
        onNotice={() => undefined}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "项目任务" }));
    expect(screen.getByLabelText("项目上下文")).toBeInTheDocument();
    expect(container.querySelector(".message-console")).toHaveClass("project-context-open");
    expect(screen.getByText("实现库存工作台")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /任务/ })).toHaveAttribute("aria-selected", "true");

    fireEvent.click(screen.getByRole("button", { name: "项目目录" }));
    expect(screen.getByRole("tab", { name: "目录" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("Harness 仓库尚未初始化完成")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "关闭项目面板" }));
    expect(screen.queryByLabelText("项目上下文")).not.toBeInTheDocument();
    expect(container.querySelector(".message-console")).not.toHaveClass("project-context-open");
  });

  it("uses one right-side context area for project content and member runtime", async () => {
    render(
      <MessagesView
        consoleData={consoleData}
        humanUser={{ id: "human-1", email: "owner@example.com", display_name: "Lee" }}
        token="token"
        realtimeEvent={null}
        onChanged={async () => undefined}
        onError={() => undefined}
        onNotice={() => undefined}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "项目任务" }));
    expect(screen.getByLabelText("项目上下文")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /群成员/ }));
    expect(screen.queryByLabelText("项目上下文")).not.toBeInTheDocument();
    expect(screen.getByLabelText("群成员与运行情况")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "项目目录" }));
    expect(screen.queryByLabelText("群成员与运行情况")).not.toBeInTheDocument();
    expect(screen.getByLabelText("项目上下文")).toBeInTheDocument();
  });

  it("opens inside the chat layout and exposes the Agent execution process", async () => {
    const { container } = render(
      <MessagesView
        consoleData={consoleData}
        humanUser={{ id: "human-1", email: "owner@example.com", display_name: "Lee" }}
        token="token"
        realtimeEvent={null}
        onChanged={async () => undefined}
        onError={() => undefined}
        onNotice={() => undefined}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /群成员/ }));

    expect(screen.getByLabelText("群成员与运行情况")).toBeInTheDocument();
    expect(container.querySelector(".message-console")).toHaveClass("members-open");
    expect(screen.queryByRole("dialog", { name: "群成员" })).not.toBeInTheDocument();
    expect((await screen.findAllByText("运行中")).length).toBeGreaterThan(0);
    expect(screen.getAllByText("正在修改库存页面").length).toBeGreaterThan(0);

    const memberDetails = screen.getByText("前端 Agent").closest("details")!;
    fireEvent.click(memberDetails.querySelector("summary")!);
    expect(memberDetails).toHaveAttribute("open");
    expect(screen.getByRole("heading", { name: "项目任务" })).toBeInTheDocument();
    expect(screen.getByText("实现库存工作台")).toBeInTheDocument();
    expect(screen.getByText("当前执行过程")).toBeInTheDocument();
    expect(screen.getByText("拆分实现步骤")).toBeInTheDocument();
    expect(screen.getByText("修改库存页面")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "关闭群成员" }));
    await waitFor(() => expect(screen.queryByLabelText("群成员与运行情况")).not.toBeInTheDocument());
    expect(container.querySelector(".message-console")).not.toHaveClass("members-open");
  });

  it("shows the same Agent runtime details in a direct conversation", async () => {
    const { container } = render(
      <MessagesView
        consoleData={consoleData}
        humanUser={{ id: "human-1", email: "owner@example.com", display_name: "Lee" }}
        token="token"
        realtimeEvent={null}
        onChanged={async () => undefined}
        onError={() => undefined}
        onNotice={() => undefined}
      />,
    );

    fireEvent.click(screen.getByText("Owner ↔ 前端 Agent"));
    fireEvent.click(screen.getByRole("button", { name: /运行详情/ }));

    expect(screen.getByLabelText("私聊成员与运行情况")).toBeInTheDocument();
    expect(container.querySelector(".message-console")).toHaveClass("members-open");
    expect(screen.getByText("对话成员")).toBeInTheDocument();
    expect(screen.getByText("前端 Agent")).toBeInTheDocument();
    expect((await screen.findAllByText("运行中")).length).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole("button", { name: "关闭运行详情" }));
    await waitFor(() => expect(screen.queryByLabelText("私聊成员与运行情况")).not.toBeInTheDocument());
  });
});
