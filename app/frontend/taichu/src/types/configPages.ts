export type SkillStatusCode = 1 | 2 | 3;

export type SkillDescriptor = {
  id: string;
  name: string;
  description?: string;
  source: "data_dir" | "user_home" | "configured" | "runtime";
  origin: "skills_dir" | "legacy_commands_dir";
  path: string;
  status: SkillStatusCode;
  status_label: string;
  enabled: boolean;
  shadowed_by?: string;
};

export type SkillsConfigView = {
  enabled: boolean;
  roots: Array<{ source: string; path: string; origin: string }>;
  allowed_skills?: string[];
  max_prompt_chars: number;
  max_search_results: number;
  auto_inject_enabled: boolean;
  auto_inject_max_items: number;
  registered_count: number;
  exposed_count: number;
  skills: SkillDescriptor[];
};

export type SkillLoadResult = {
  id: string;
  name: string;
  description?: string;
  path: string;
  args?: string;
  prompt: string;
  truncated: boolean;
};

export type InstalledSkill = {
  id: string;
  name: string;
  source_path: string;
  installed_path: string;
};

export type McpTransport = "stdio" | "http" | "sse" | "ws";
export type McpServerStatus = "configured" | "resolved" | "command_not_found" | "unsupported_transport";

export type McpServerConfig =
  | {
      type: "stdio";
      command: string;
      args?: string[];
      env?: Record<string, string>;
      tool_call_timeout_ms?: number;
    }
  | {
      type: "http" | "sse" | "ws";
      url: string;
      headers?: Record<string, string>;
    };

export type McpServerAdminView = {
  name: string;
  config: McpServerConfig;
  status: McpServerStatus;
  detail?: string;
};

export type McpConfigView = {
  enabled: boolean;
  configured_count: number;
  stdio_count: number;
  unsupported_count: number;
  initialize_timeout_ms: number;
  list_tools_timeout_ms: number;
  default_tool_call_timeout_ms: number;
  resource_timeout_ms: number;
  degraded: boolean;
  servers: Array<{
    name: string;
    transport: McpTransport;
    status: McpServerStatus;
    detail?: string;
  }>;
};

export type McpDiscoveryReport = {
  tools: Array<{ server_name: string; raw_name: string; qualified_name: string }>;
  failed_servers: Array<{ server_name: string; error: string; recoverable: boolean }>;
  unsupported_servers: Array<{ server_name: string; transport: McpTransport; reason: string }>;
  degraded: boolean;
};
