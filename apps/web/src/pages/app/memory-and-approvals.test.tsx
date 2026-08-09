import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AgentToolApproval, CompanyAgent } from "../../types/platform";
import { ApprovalCard, ApprovalReviewActions, ApprovalsView } from "./memory-and-approvals";

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

const agents: CompanyAgent[] = [{
  agent_profile: { id: "agent-1", display_name: "叶舟", handle: "@yez", persona: "", collaboration_preference: "available", status: "active", created_at: "2026-08-09T10:00:00Z" },
  membership: { id: "membership-1", agent_profile_id: "agent-1", org_unit_id: "org-1", job_title: "前端工程师", role_key: "frontend_engineer", permissions: [], responsibilities: [], skills: [], current_focus: "", staffing_scope_org_unit_id: null, employment_status: "active", reports_to_membership_id: null },
  connection: { status: "connected", key_prefix: null, key_created_at: null, key_expires_at: null, last_used_at: null },
}];

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

describe("ApprovalsView interactions", () => {
  it("filters approvals when a summary metric is clicked", () => {
    const pending = approval();
    const approved = { ...approval(), id: "approval-2", status: "approved" as const, reason: "Approved website" };
    render(<ApprovalsView approvals={[pending, approved]} agents={[...agents]} onReview={vi.fn(async () => undefined)} onError={vi.fn()} />);

    expect(screen.getByText("Open a website")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /已通过1/ }));

    expect(screen.getByRole("button", { name: /访问网站.*已批准/ })).toBeVisible();
    expect(screen.queryByText("Open a website")).not.toBeInTheDocument();
  });

  it("opens and closes a completed approval record", () => {
    const approved = { ...approval(), status: "approved" as const, reason: "Approved website" };
    render(<ApprovalCard approval={approved} agents={[...agents]} onReview={vi.fn(async () => undefined)} onError={vi.fn()} />);

    expect(screen.queryByText("Approved website")).not.toBeInTheDocument();
    const header = screen.getByRole("button", { name: /访问网站.*叶舟/ });
    fireEvent.click(header);
    expect(header).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Approved website")).toBeVisible();
    fireEvent.click(header);
    expect(screen.queryByText("Approved website")).not.toBeInTheDocument();
  });
});
