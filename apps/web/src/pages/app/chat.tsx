import { useState } from "react";
import type { CompanyRealtimeEvent } from "../../api/types";
import { MessagesView } from "../../components/MessagesView";
import { Icon } from "../../components/ui";
import type { HumanUser } from "../../types/appShell";
import type { AgentToolApproval, CompanyConsole } from "../../types/platform";
import { ApprovalsView, type ApprovalReviewDecision } from "./memory-and-approvals";

export function ChatCenter(props: {
  consoleData: CompanyConsole;
  humanUser: HumanUser;
  token: string;
  realtimeEvent: CompanyRealtimeEvent | null;
  approvals: AgentToolApproval[];
  onReview: (approvalId: string, decision: ApprovalReviewDecision, reviewNote: string) => Promise<void>;
  onChanged: () => Promise<void>;
  onOpenProject: (projectId: string, tab: "repository" | "tasks") => void;
  onError: (error: unknown) => void;
  onNotice: (notice: string) => void;
}) {
  const [tab, setTab] = useState<"messages" | "approvals">("messages");
  const pendingApprovalCount = props.approvals.filter((approval) => approval.status === "pending").length;

  return (
    <div className="control-center chat-center">
      <nav className="control-center-tabs two-tabs" role="tablist" aria-label="聊天">
        <button className={tab === "messages" ? "active" : ""} type="button" role="tab" aria-selected={tab === "messages"} onClick={() => setTab("messages")}>
          <span className="control-center-tab-icon"><Icon name="message" /></span>
          <span><strong>聊天</strong><small>Human 与 Agent</small></span>
        </button>
        <button className={tab === "approvals" ? "active" : ""} type="button" role="tab" aria-selected={tab === "approvals"} onClick={() => setTab("approvals")}>
          <span className="control-center-tab-icon"><Icon name="shield" /></span>
          <span><strong>审批</strong><small>权限升级请求</small></span>
          {pendingApprovalCount ? <span className="control-center-tab-badge">{pendingApprovalCount}</span> : null}
        </button>
      </nav>
      <div className="control-center-panel" role="tabpanel">
        {tab === "messages" ? (
          <MessagesView
            consoleData={props.consoleData}
            humanUser={props.humanUser}
            token={props.token}
            realtimeEvent={props.realtimeEvent}
            onChanged={props.onChanged}
            onOpenProject={props.onOpenProject}
            onError={props.onError}
            onNotice={props.onNotice}
          />
        ) : null}
        {tab === "approvals" ? (
          <ApprovalsView
            approvals={props.approvals}
            agents={props.consoleData.agents}
            onReview={props.onReview}
            onError={props.onError}
          />
        ) : null}
      </div>
    </div>
  );
}
