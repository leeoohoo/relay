import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CodexCliSettingsView } from "./CodexCliSettingsView";

const settings = {
  company_id: "company-1",
  model: "gpt-company",
  reasoning_effort: "high",
  reasoning_summary: "auto",
  verbosity: "medium",
  personality: "pragmatic",
  service_tier: null,
  approval_policy: "never",
  sandbox_mode: "workspace_write",
  network_access: true,
  web_search: "cached",
  feature_multi_agent: true,
  feature_remote_plugin: true,
  feature_hooks: true,
  feature_goals: true,
  feature_shell_tool: true,
  updated_at: "2026-08-06T00:00:00Z",
};

afterEach(() => vi.restoreAllMocks());

describe("CodexCliSettingsView", () => {
  it("shows company defaults, runner overrides, runtime state, and final sources", async () => {
    vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.endsWith("/codex-cli-settings")) return new Response(JSON.stringify({ settings }), { status: 200 });
      if (url.endsWith("/codex-runner-profiles")) return new Response(JSON.stringify({ profiles: [{ profile: {
        id: "runner-1", name: "开发运行器", is_default: true, model: null,
        reasoning_effort: "xhigh", reasoning_summary: null, verbosity: null,
        personality: null, service_tier: "fast", approval_policy: "inherit",
        sandbox_mode: "read_only", network_access: null, web_search: "live",
        feature_multi_agent: null, feature_remote_plugin: null, feature_hooks: null,
        feature_goals: null, feature_shell_tool: false,
      } }] }), { status: 200 });
      return new Response(JSON.stringify({ runtime: {
        installed: true, installed_version: "0.99.0", host_os: "macos", host_arch: "aarch64",
        executable_path: "/managed/codex", source: "managed",
        default_auth: { config: { codex_home: "/managed/home", config_path: "/managed/home/config.toml" } },
      } }), { status: 200 });
    }));

    render(<CodexCliSettingsView companyId="company-1" token="token" onError={() => undefined} onNotice={() => undefined} />);

    expect(await screen.findByText("Codex 0.99.0")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "公司默认" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "生效状态" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "开发运行器（默认）" })).toBeInTheDocument();
    expect(screen.getAllByText("features.shell_tool")).toHaveLength(2);
    expect(screen.getAllByText("运行配置").length).toBeGreaterThan(0);
    expect(screen.getAllByText("全局统一")).toHaveLength(8);
  });
});
