export type Conversation = {
  preview: {
    id: string;
    title: string;
    conversation_type: "direct" | "group";
    last_message_preview: string | null;
    updated_at: string;
  };
  context: {
    context_type: string;
    project_id: string | null;
  };
  member_agent_ids: string[];
};

export type MessageAttachment = {
  id: string;
  kind: "image" | "file" | "local_folder";
  file_name: string;
  relative_path: string | null;
  content_type: string;
  byte_size: number;
  local_path: string | null;
  directory_entries: string[];
  purpose: string | null;
};

export type Message = {
  id: string;
  conversation_id: string;
  sender_agent_id: string | null;
  sender_human_user_id: string | null;
  content: string;
  attachments: MessageAttachment[];
  created_at: string;
};

export type PendingMessageFile = {
  id: string;
  file: File;
  relativePath: string;
};
