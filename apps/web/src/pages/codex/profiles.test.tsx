import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CodexCompanyCliSettings, CodexRunnerProfileView } from "../../types/platform";
import { CodexRunnerProfilesPanel } from "./profiles";

const cliSettings: CodexCompanyCliSettings = {
  company_id: "company-1",
  model: "gpt-company",
  reasoning_effort: "high",
  reasoning_summary: "concise",
  verbosity: "medium",
  personality: "pragmatic",
  service_tier: null,
  approval_policy: "never",
  sandbox_mode: "workspace_write",
  network_access: true,
  web_search: "cached",
  feature_multi_agent: true,
  feature_remote_plugin: true,
  feature_hooks: false,
  feature_goals: true,
  feature_shell_tool: true,
  updated_at: "2026-08-06T00:00:00Z",
};

const profile: CodexRunnerProfileView = {
  profile: {
    id: "profile-1",
    company_id: "company-1",
    name: "默认运行配置",
    interval_seconds: 3600,
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
    max_run_seconds: 3600,
    is_default: true,
    created_at: "2026-08-06T00:00:00Z",
    updated_at: "2026-08-06T00:00:00Z",
  },
  assigned_agent_count: 0,
};

afterEach(() => vi.restoreAllMocks());

describe("CodexRunnerProfilesPanel", () => {
  it("names the CLI settings source and shows its effective values instead of an ambiguous inheritance label", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({ models: [] }), { status: 200 })));

    render(
      <CodexRunnerProfilesPanel
        companyId="company-1"
        profiles={[profile]}
        loading={false}
        authProfiles={[]}
        cliSettings={cliSettings}
        token="token"
        onChanged={async () => undefined}
        onError={() => undefined}
        onNotice={() => undefined}
      />,
    );

    expect(screen.getByText("默认 · 高 · high")).toBeInTheDocument();
    expect(screen.getByText("默认 · 可写工作区")).toBeInTheDocument();
    expect(screen.getByText("默认 · 无需审批")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "编辑" }));

    expect(await screen.findByRole("option", { name: "默认 · gpt-company" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "默认 · 高 · high" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "默认 · concise" })).toBeInTheDocument();
    expect(screen.getByText("新 Agent 默认使用")).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: "继承公司默认" })).not.toBeInTheDocument();
    expect(screen.getByText("高级设置").closest("details")).not.toHaveAttribute("open");
    expect(screen.getByLabelText("配置名称")).toBeVisible();
    expect(screen.getByLabelText("思考等级")).toBeVisible();
    expect(screen.getByLabelText("Sandbox")).toBeVisible();
    expect(screen.getByLabelText("运行审批策略")).toBeVisible();
    expect(screen.getByLabelText("推理摘要")).not.toBeVisible();
    expect(screen.queryByLabelText("Fast 模式")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("工作区网络")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Web Search")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("多 Agent")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("插件")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Hooks")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Goals")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Shell")).not.toBeInTheDocument();
  });
});
