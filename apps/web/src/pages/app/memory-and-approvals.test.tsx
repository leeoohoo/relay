import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AgentToolApproval } from "../../types/platform";
import { ApprovalReviewActions } from "./memory-and-approvals";

afterEach(cleanup);

function approval(toolName = "codex.website_access"): AgentToolApproval {
  return {
    id: "approval-1",
    company_id: "company-1",
    approval_source: "codex",
    runtime_config_id: null,
    runtime_run_id: null,
    codex_trigger_run_id: "run-1",
    requested_by_agent_id: "agent-1",
    tool_name: toolName,
    risk_level: "medium",
    reason: "Open a website",
    arguments: toolName === "codex.website_access" ? {
      url: "https://example.com/page",
      relay_approval_scope: "project:project-1",
      relay_approval_target: "https://example.com",
    } : {},
    status: "pending",
    expires_at: "2026-08-09T12:00:00Z",
    reviewed_by_human_user_id: null,
    review_note: "",
    reviewed_at: null,
    execution_result: {},
    error_message: null,
    created_at: "2026-08-09T11:00:00Z",
    updated_at: "2026-08-09T11:00:00Z",
  };
}

describe("ApprovalReviewActions", () => {
  it("offers a persistent website approval and sends the always_allow decision", async () => {
    const onReview = vi.fn(async () => undefined);
    render(<ApprovalReviewActions approval={approval()} onReview={onReview} onError={vi.fn()} />);

    expect(screen.getByRole("button", { name: "允许一次" })).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "始终允许" }));

    await waitFor(() => expect(onReview).toHaveBeenCalledWith("approval-1", "always_allow", ""));
  });

  it("does not offer always allow for non-website approvals", () => {
    render(<ApprovalReviewActions approval={approval("codex.command_execution")} onReview={vi.fn()} onError={vi.fn()} />);
    expect(screen.queryByRole("button", { name: "始终允许" })).not.toBeInTheDocument();
  });
});
