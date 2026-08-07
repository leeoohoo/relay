import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CodexPluginCatalog, CodexPluginOperation } from "../../types/platform";
import { CodexPluginsView } from "./plugins";

const catalog: CodexPluginCatalog = {
  runner_id: "ubuntu-arm:ubuntu",
  target_selector: "default",
  hostname: "ubuntu-arm",
  codex_version: "0.146.1",
  fingerprint: "1234567890abcdef",
  discovery_status: "ready",
  diagnostic_message: null,
  installed: [],
  available: [{
    pluginId: "codex-security@openai-api-curated",
    name: "Codex Security",
    marketplaceName: "openai-api-curated",
    version: "1.0.0",
    installed: false,
    enabled: false,
    authPolicy: "NONE",
  }],
  marketplaces: [{ name: "openai-api-curated" }],
  discovered_at: "2026-08-07T01:00:00Z",
};

function operation(status: CodexPluginOperation["status"]): CodexPluginOperation {
  return {
    id: "operation-1",
    target_runner_id: catalog.runner_id,
    target_selector: catalog.target_selector,
    operation: "install",
    plugin_id: catalog.available[0].pluginId,
    status,
    attempt_count: status === "queued" ? 0 : 1,
    error_message: null,
    requested_at: "2026-08-07T01:01:00Z",
    finished_at: status === "succeeded" ? "2026-08-07T01:01:02Z" : null,
  };
}

afterEach(() => vi.restoreAllMocks());

describe("CodexPluginsView", () => {
  it("shows a submitted install immediately and polls until the Trigger finishes it", async () => {
    let pluginListRequests = 0;
    vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (url.endsWith("/codex-environments")) {
        return new Response(JSON.stringify({ profiles: [], mcp_environments: [], runtime: {} }), { status: 200 });
      }
      if (url.endsWith("/codex-plugins/operations") && init?.method === "POST") {
        return new Response(JSON.stringify({ operation: operation("queued") }), { status: 200 });
      }
      if (url.includes("/codex-plugins?")) {
        pluginListRequests += 1;
        return new Response(JSON.stringify({
          catalogs: [catalog],
          operations: pluginListRequests > 1 ? [operation("succeeded")] : [],
        }), { status: 200 });
      }
      return new Response(JSON.stringify({ message: "unexpected request" }), { status: 500 });
    }));

    render(
      <CodexPluginsView
        companyId="company-1"
        token="token"
        realtimeEvent={null}
        onError={(error) => { throw error; }}
        onNotice={() => undefined}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "可安装 1" }));
    fireEvent.click(screen.getByRole("button", { name: "安装" }));

    expect(await screen.findByText("排队中")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText("已完成")).toBeInTheDocument(), { timeout: 3_000 });
    expect(pluginListRequests).toBeGreaterThanOrEqual(2);
  });
});
