import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { MessageAttachments } from "./MessageAttachments";

describe("MessageAttachments", () => {
  it("renders local folder references without uploading folder contents", () => {
    render(
      <MessageAttachments
        token="session-token"
        onError={vi.fn()}
        message={{
          id: "message-1",
          conversation_id: "conversation-1",
          sender_agent_id: null,
          sender_human_user_id: "human-1",
          content: "请导入这个目录",
          created_at: "2026-08-05T00:00:00Z",
          attachments: [{
            id: "attachment-1",
            kind: "local_folder",
            file_name: "legacy-project",
            relative_path: null,
            content_type: "application/x-local-directory-reference",
            byte_size: 0,
            local_path: "/workspace/legacy-project",
            directory_entries: ["src/", "src/main.rs", "Cargo.toml"],
            purpose: "project_import",
          }],
        }}
      />,
    );

    expect(screen.getByText("legacy-project")).toBeInTheDocument();
    expect(screen.getByText("/workspace/legacy-project")).toBeInTheDocument();
    expect(screen.getByText("3 项")).toBeInTheDocument();
  });
});
