export type AiProvider = "openai-compatible" | "anthropic-compatible" | "custom";
export type ReasoningEffort = "low" | "medium" | "high";

export type AiParamSupport = {
  model: boolean;
  max_tokens: boolean;
  messages: boolean;
  system: boolean;
  stream: boolean;
  tools: boolean;
  tool_choice: boolean;
  temperature: boolean;
  top_p: boolean;
  frequency_penalty: boolean;
  presence_penalty: boolean;
  stop: boolean;
  reasoning_effort: boolean;
};

export type AiInstanceView = {
  id: string;
  name: string;
  enabled: boolean;
  provider: AiProvider;
  display_name: string;
  base_url: string;
  request_path: string;
  request_url: string;
  model: string;
  prompt_template: string;
  timeout_ms: number;
  auth_header: string;
  has_api_key: boolean;
  stream: boolean;
  max_tokens?: number;
  temperature?: number;
  top_p?: number;
  frequency_penalty?: number;
  presence_penalty?: number;
  stop?: string[];
  reasoning_effort?: ReasoningEffort;
  supported_params: AiParamSupport;
  created_at_ms: number;
  updated_at_ms: number;
};

export type AiInstancePayload = {
  name: string;
  enabled: boolean;
  provider: AiProvider;
  base_url: string;
  request_path: string;
  api_key?: string;
  model: string;
  prompt_template: string;
  timeout_ms: number;
  auth_header: string;
  stream: boolean;
  max_tokens?: number;
  temperature?: number;
  top_p?: number;
  frequency_penalty?: number;
  presence_penalty?: number;
  stop?: string[];
  reasoning_effort?: ReasoningEffort;
};

export type AiInstanceFormState = AiInstancePayload & {
  id?: string;
  stop_text: string;
};

export type AiInstanceTestResult = {
  instance: AiInstanceView;
  result: {
    success: boolean;
    status?: number;
    error?: string;
    body?: string;
    attempts: number;
    request_url: string;
  };
};
