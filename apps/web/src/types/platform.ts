import type { Company } from "./appShell";
import type { Conversation } from "./chat";
import type { RelaySkillLanguage } from "../relaySkills";

export type CodexReasoningEffort = "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max" | "ultra";

export type CompanyProfession = {
  key: string;
  label: string;
  label_en: string;
  description: string;
  description_en: string;
  category_key: string;
  category_label: string;
  category_label_en: string;
  skill_name: string;
  skill_markdown: string;
  skill_markdown_en: string;
  can_create_tasks: boolean;
};

export type OrgUnit = {
  id: string;
  parent_org_unit_id: string | null;
  name: string;
  unit_type: string;
};

export type AgentProfile = {
  id: string;
  display_name: string;
  handle: string;
  persona: string;
  collaboration_preference: "available" | "low_cost_only" | "unavailable";
  status: string;
  created_at: string;
};

export type AgentMembership = {
  id: string;
  agent_profile_id: string;
  org_unit_id: string;
  job_title: string;
  role_key: string;
  permissions: string[];
  responsibilities: string[];
  skills: string[];
  current_focus: string;
  staffing_scope_org_unit_id: string | null;
  employment_status: string;
  reports_to_membership_id: string | null;
};

export type AgentConnection = {
  status: "connected" | "not_connected" | "awaiting_activation" | "suspended" | "terminated" | "key_revoked" | "key_expired" | "no_key";
  key_prefix: string | null;
  key_created_at: string | null;
  key_expires_at: string | null;
  last_used_at: string | null;
};

export type CompanyAgent = {
  agent_profile: AgentProfile;
  membership: AgentMembership;
  profession?: CompanyProfession;
  connection: AgentConnection;
};

export type ProjectGitView = {
  remote_url: string;
  default_branch: string;
  git_host: string;
  push_enabled: boolean;
  branch_prefix: string;
  auth_configured: boolean;
};

export type ProjectGitAdminView = {
  remote_url: string;
  default_branch: string;
  git_host: string;
  host_local_path: string;
  auth_profile: string | null;
  allow_agent_push: boolean;
  branch_prefix: string;
  created_at: string;
  updated_at: string;
};

export type CompanyProjectTask = {
  id: string;
  project_id: string;
  title: string;
  description: string;
  status: "todo" | "in_progress" | "blocked" | "done" | "failed" | "cancelled";
  priority: "low" | "normal" | "high" | "urgent";
  assignee_agent_id: string | null;
  created_by_agent_id: string | null;
  created_by_human_user_id: string | null;
  updated_by_agent_id: string | null;
  updated_by_human_user_id: string | null;
  due_at: string | null;
  completed_at: string | null;
  created_at: string;
  updated_at: string;
};

export type CompanyProject = {
  project: {
    id: string;
    name: string;
    description: string;
    project_type: string;
    project_type_source: string;
    project_type_confidence: number;
    project_type_evidence: string[];
    status: string;
  };
  git: ProjectGitView | null;
  rule: {
    project_id: string;
    content: string;
    updated_by_agent_id: string | null;
    updated_by_human_user_id: string | null;
    created_at: string;
    updated_at: string;
  } | null;
  assets: Array<{
    id: string;
    project_id: string;
    name: string;
    asset_type: string;
    locator: string;
    description: string;
    status: "active" | "missing" | "deprecated" | "unknown";
    metadata: Record<string, unknown>;
    updated_by_agent_id: string | null;
    updated_by_human_user_id: string | null;
    created_at: string;
    updated_at: string;
  }>;
  asset_refresh: {
    project_id: string;
    maintainer_agent_id: string;
    interval_minutes: number;
    enabled: boolean;
    next_refresh_at: string;
    last_requested_at: string | null;
    last_completed_at: string | null;
    created_at: string;
    updated_at: string;
  } | null;
  members: Array<{
    member: {
      id: string;
      project_id: string;
      agent_profile_id: string;
      role: string;
      left_at: string | null;
    };
    agent_profile: AgentProfile;
  }>;
  tasks: CompanyProjectTask[];
  task_dependencies: Array<{
    id: string;
    project_id: string;
    task_id: string;
    depends_on_task_id: string;
  }>;
  task_status_history: Array<{
    id: string;
    project_id: string;
    task_id: string;
    from_status: CompanyProjectTask["status"] | null;
    to_status: CompanyProjectTask["status"];
    changed_by_agent_id: string | null;
    changed_by_human_user_id: string | null;
    change_source: "agent" | "human" | "system";
    metadata: Record<string, unknown>;
    created_at: string;
  }>;
};

export type CompanyProjectType = {
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
};

export type CodexTriggerRun = {
  id: string;
  trigger_type: string;
  status: string;
  project_id: string | null;
  codex_thread_id: string | null;
  started_at: string;
  finished_at: string | null;
  final_message_summary: string | null;
  error_message: string | null;
  activity_phase: string;
  activity_summary: string | null;
  last_activity_at: string | null;
  activity_log: Array<{
    at: string;
    phase: string;
    summary: string;
  }>;
};

export type CodexTriggerView = {
  runner_profile_id: string | null;
  config: {
    id: string;
    status: "active" | "paused" | "error";
    interval_seconds: number;
    codex_profile: string;
    model: string | null;
    reasoning_effort: CodexReasoningEffort | null;
    reasoning_summary: CodexReasoningSummary | null;
    verbosity: CodexVerbosity | null;
    personality: CodexPersonality | null;
    service_tier: "fast" | null;
    sandbox_mode: CodexSandboxMode;
    approval_policy: CodexApprovalPolicy;
    network_access: boolean | null;
    web_search: CodexWebSearch | null;
    feature_multi_agent: boolean | null;
    feature_remote_plugin: boolean | null;
    feature_hooks: boolean | null;
    feature_goals: boolean | null;
    feature_shell_tool: boolean | null;
    max_run_seconds: number;
    next_run_at: string;
    lease_owner: string | null;
    lease_expires_at: string | null;
    manual_run_requested_at: string | null;
    wake_requested_at: string | null;
    wake_reason: string | null;
    last_run_at: string | null;
    last_success_at: string | null;
    last_error: string | null;
    consecutive_failure_count: number;
  };
  recent_runs: CodexTriggerRun[];
};

export type CodexRunnerProfileView = {
  profile: {
    id: string;
    company_id: string;
    name: string;
    interval_seconds: number;
    codex_profile: string;
    model: string | null;
    reasoning_effort: CodexReasoningEffort | null;
    reasoning_summary: CodexReasoningSummary | null;
    verbosity: CodexVerbosity | null;
    personality: CodexPersonality | null;
    service_tier: "fast" | null;
    sandbox_mode: CodexSandboxMode;
    approval_policy: CodexApprovalPolicy;
    network_access: boolean | null;
    web_search: CodexWebSearch | null;
    feature_multi_agent: boolean | null;
    feature_remote_plugin: boolean | null;
    feature_hooks: boolean | null;
    feature_goals: boolean | null;
    feature_shell_tool: boolean | null;
    max_run_seconds: number;
    is_default: boolean;
    created_at: string;
    updated_at: string;
  };
  assigned_agent_count: number;
};

export type CodexReasoningSummary = "auto" | "concise" | "detailed" | "none";
export type CodexVerbosity = "low" | "medium" | "high";
export type CodexPersonality = "none" | "friendly" | "pragmatic";
export type CodexWebSearch = "disabled" | "cached" | "indexed" | "live";
export type CodexSandboxMode = "inherit" | "read_only" | "workspace_write";
export type CodexApprovalPolicy = "inherit" | "never" | "on-request";

export type CodexCliRuntime = {
  installed: boolean;
  source: string;
  executable_path: string | null;
  installed_version: string | null;
  latest_version: string | null;
  update_available: boolean;
  operation_status: "idle" | "install_pending" | "installing" | "update_pending" | "updating" | "failed";
  last_checked_at: string | null;
  update_check_error: string | null;
  last_error: string | null;
  host_os: string;
  host_arch: string;
  installer_kind: "posix_shell" | "powershell" | "unsupported";
  installation_supported: boolean;
  default_auth: CodexDefaultAuthEnvironment;
  updated_at: string;
};

export type CodexDefaultAuthEnvironment = {
  selector: "default";
  name: string;
  status: "active" | "logged_out" | "unknown";
  method: "api_key" | "chatgpt" | "configured" | null;
  last_checked_at: string | null;
  last_error: string | null;
  config: {
    codex_home: string | null;
    config_path: string | null;
    config_exists: boolean;
    auth_path: string | null;
    auth_exists: boolean;
    credential_hint: string | null;
    openai_base_url: string | null;
    model_provider: string | null;
    model: string | null;
    reasoning_effort: string | null;
    sandbox_mode: string | null;
    approval_policy: string | null;
    mcp_servers: string[];
    named_profiles: string[];
    trusted_project_count: number;
    plugin_count: number;
  };
};

export type CodexAuthProfile = {
  id: string;
  company_id: string;
  name: string;
  selector: string;
  base_url: string | null;
  status: "pending" | "active" | "failed" | "deleting";
  last_error: string | null;
  created_at: string;
  updated_at: string;
};

export type CodexEnvironmentView = {
  runtime: CodexCliRuntime;
  profiles: CodexAuthProfile[];
  mcp_environments: CodexMcpEnvironmentSnapshot[];
};

export type CodexMcpServer = {
  name: string;
  transport: "stdio" | "streamable_http" | string;
  enabled: boolean;
  auth_status: string | null;
  address: string | null;
  command: string | null;
  argument_count: number;
  bearer_token_env_var: string | null;
  startup_timeout_sec: number | null;
  tool_timeout_sec: number | null;
  disabled_reason: string | null;
  configured_by_user: boolean;
};

export type CodexMcpEnvironmentSnapshot = {
  selector: string;
  status: "unknown" | "ready" | "failed";
  operation_status: "idle" | "refresh_pending" | "refreshing" | "add_pending" | "adding" | "remove_pending" | "removing" | "failed";
  pending_server_name: string | null;
  servers: CodexMcpServer[];
  last_checked_at: string | null;
  last_error: string | null;
};

export type CodexPluginItem = {
  pluginId: string;
  name: string;
  marketplaceName: string;
  version: string;
  installed: boolean;
  enabled: boolean;
  installPolicy?: string;
  authPolicy?: string;
};

export type CodexPluginCatalog = {
  runner_id: string;
  hostname: string;
  codex_version: string | null;
  fingerprint: string;
  installed: CodexPluginItem[];
  available: CodexPluginItem[];
  marketplaces: Array<{
    name: string;
    root?: string;
    marketplaceSource?: { sourceType?: string; source?: string };
  }>;
  discovered_at: string;
};

export type CodexPluginOperation = {
  id: string;
  target_runner_id: string;
  operation: "install" | "remove" | "refresh";
  plugin_id: string | null;
  status: "queued" | "running" | "succeeded" | "failed";
  attempt_count: number;
  error_message: string | null;
  requested_at: string;
  finished_at: string | null;
};

export type AgentToolApproval = {
  id: string;
  company_id: string;
  approval_source: "runtime_model" | "codex";
  runtime_config_id: string | null;
  runtime_run_id: string | null;
  codex_trigger_run_id: string | null;
  requested_by_agent_id: string;
  tool_name: string;
  risk_level: "low" | "medium" | "high";
  reason: string;
  arguments: Record<string, unknown>;
  status: "pending" | "approved" | "executing" | "executed" | "rejected" | "expired" | "failed";
  expires_at: string;
  reviewed_by_human_user_id: string | null;
  review_note: string;
  reviewed_at: string | null;
  execution_result: Record<string, unknown>;
  error_message: string | null;
  created_at: string;
  updated_at: string;
};

export type AgentMemory = {
  id: string;
  company_id: string;
  owner_agent_id: string;
  project_id: string | null;
  memory_tier: "short_term" | "long_term";
  memory_type: "fact" | "decision" | "lesson" | "preference" | "procedure" | "relationship" | "handoff";
  topic_key: string;
  title: string;
  summary: string;
  when_to_use: string;
  tags: string[];
  importance: number;
  confidence: number;
  pinned: boolean;
  status: "draft" | "active" | "archived" | "superseded";
  source_refs: Array<{ source_type: string; source_id: string; label: string | null }>;
  supersedes_memory_id: string | null;
  expires_at: string | null;
  verified_by_agent_id: string | null;
  verified_by_human_user_id: string | null;
  verified_at: string | null;
  created_at: string;
  updated_at: string;
};

export type CompanyConsole = {
  company: Company;
  human_membership: { role: "owner" | "admin" | "viewer"; status: string };
  org_units: OrgUnit[];
  agents: CompanyAgent[];
  conversations: Conversation[];
  projects: CompanyProject[];
  professions: CompanyProfession[];
  project_types: CompanyProjectType[];
  governance_policy: {
    effective_settings: {
      managed_workspace_root: string | null;
      skill_language: RelaySkillLanguage;
    };
  };
};

export type Credential = {
  agent: AgentProfile;
  key: string | null;
  keyPrefix: string;
  permissions: string[];
  professionKey: string;
  profession?: CompanyProfession;
  skillLanguage: RelaySkillLanguage;
};

export type BatchCredentialResult = {
  credentials: Credential[];
  failures: Array<{ agentName: string; message: string }>;
};
