export type HumanUser = {
  id: string;
  email: string;
  display_name: string;
};

export type Company = {
  id: string;
  name: string;
  slug: string;
  description: string;
};

export type RuntimeConfig = {
  dev_endpoints_enabled: boolean;
  email_verification_required: boolean;
  project_types: Array<{
    key: string;
    label: string;
    label_en: string;
    description: string;
    description_en: string;
    category_key: string;
    category_label: string;
    category_label_en: string;
    rule_markdown: string;
    rule_markdown_en: string;
  }>;
};

export type Session = {
  token: string;
  user: HumanUser;
};

export type View =
  | "agents"
  | "skills"
  | "projects"
  | "codex"
  | "messages";
