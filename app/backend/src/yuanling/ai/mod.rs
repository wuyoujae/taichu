use axum::{routing::{get, post}, Json, Router};
use reqwest::{header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE}, Client, Response, StatusCode};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{json, Map, Value};
use std::env;
use std::time::Duration;

pub type TopP = f64;
pub type Temperature = f64;

const DEFAULT_MAX_RETRIES: u32 = 8;
const DEFAULT_INITIAL_BACKOFF_SECS: u64 = 1;
const DEFAULT_MAX_BACKOFF_SECS: u64 = 128;
const OPENAI_MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
const ANTHROPIC_DEFAULT_CONTEXT_WINDOW: u32 = 200_000;

const OPENAI_COMPATIBLE_PROVIDER: &str = "openai-compatible";
const ANTHROPIC_COMPATIBLE_PROVIDER: &str = "anthropic-compatible";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiProvider {
  OpenAICompatible,
  AnthropicCompatible,
  Custom,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiParamSupport {
  pub model: bool,
  pub max_tokens: bool,
  pub messages: bool,
  pub system: bool,
  pub stream: bool,
  pub tools: bool,
  pub tool_choice: bool,
  pub temperature: bool,
  pub top_p: bool,
  pub frequency_penalty: bool,
  pub presence_penalty: bool,
  pub stop: bool,
  pub reasoning_effort: bool,
}

impl AiProvider {
  fn from_raw(raw: &str) -> Self {
    match raw.trim().to_lowercase().as_str() {
      "openai" | "openai-compatible" | "openai_compatible" => Self::OpenAICompatible,
      "anthropic" | "anthropic-compatible" | "anthropic_compatible" | "claude" => {
        Self::AnthropicCompatible
      }
      _ => Self::Custom,
    }
  }

  fn provider_name(self) -> &'static str {
    match self {
      Self::OpenAICompatible => OPENAI_COMPATIBLE_PROVIDER,
      Self::AnthropicCompatible => ANTHROPIC_COMPATIBLE_PROVIDER,
      Self::Custom => "custom",
    }
  }

  fn display_name(self) -> &'static str {
    match self {
      Self::OpenAICompatible => "OpenAI Compatible",
      Self::AnthropicCompatible => "Anthropic Compatible",
      Self::Custom => "Custom",
    }
  }

  fn default_endpoint(self) -> &'static str {
    match self {
      Self::OpenAICompatible => "https://api.openai.com/v1",
      Self::AnthropicCompatible => "https://api.anthropic.com",
      Self::Custom => "http://localhost:8000/v1",
    }
  }

  fn default_model(self) -> &'static str {
    match self {
      Self::OpenAICompatible => "gpt-4o-mini",
      Self::AnthropicCompatible => "claude-3-5-sonnet-20240620",
      Self::Custom => "local-default",
    }
  }

  fn default_request_path(self) -> &'static str {
    match self {
      Self::OpenAICompatible => "/chat/completions",
      Self::AnthropicCompatible => "/v1/messages",
      Self::Custom => "/chat/completions",
    }
  }

  fn default_auth_header(self) -> &'static str {
    match self {
      Self::OpenAICompatible => "Authorization",
      Self::AnthropicCompatible => "x-api-key",
      Self::Custom => "Authorization",
    }
  }

  fn supports(self) -> AiParamSupport {
    match self {
      Self::OpenAICompatible => AiParamSupport {
        model: true,
        max_tokens: true,
        messages: true,
        system: true,
        stream: true,
        tools: true,
        tool_choice: true,
        temperature: true,
        top_p: true,
        frequency_penalty: true,
        presence_penalty: true,
        stop: true,
        reasoning_effort: true,
      },
      Self::AnthropicCompatible => AiParamSupport {
        model: true,
        max_tokens: true,
        messages: true,
        system: true,
        stream: true,
        tools: true,
        tool_choice: true,
        temperature: true,
        top_p: false,
        frequency_penalty: false,
        presence_penalty: false,
        stop: true,
        reasoning_effort: false,
      },
      Self::Custom => AiParamSupport {
        model: true,
        max_tokens: true,
        messages: true,
        system: true,
        stream: true,
        tools: true,
        tool_choice: true,
        temperature: true,
        top_p: true,
        frequency_penalty: true,
        presence_penalty: true,
        stop: true,
        reasoning_effort: true,
      },
    }
  }

  fn is_reasoning_model(model: &str) -> bool {
    let lowered = model.to_ascii_lowercase();
    let canonical = lowered.rsplit('/').next().unwrap_or(lowered.as_str());
    canonical.starts_with("o1")
      || canonical.starts_with("o3")
      || canonical.starts_with("o4")
      || canonical == "grok-3-mini"
      || canonical.starts_with("qwen-qwq")
      || canonical.starts_with("qwq")
      || canonical.contains("thinking")
  }

  fn is_gpt5(model: &str) -> bool {
    model.to_ascii_lowercase().trim().starts_with("gpt-5")
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiModelConfig {
  pub provider: String,
  pub model_id: String,
  pub endpoint: String,
  pub request_path: String,
  pub prompt_template: String,
  pub timeout_ms: u64,
  pub auth_header: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AiModuleConfig {
  pub enabled: bool,
  pub model: AiModelConfig,
  pub has_api_key: bool,
}

#[derive(Clone, Serialize)]
pub struct AiModuleConfigView {
  pub enabled: bool,
  pub provider: String,
  pub display_name: &'static str,
  pub endpoint: String,
  pub request_path: String,
  pub request_url: String,
  pub model_id: String,
  pub auth_header: String,
  pub prompt_template: String,
  pub timeout_ms: u64,
  pub has_api_key: bool,
  pub supported_params: AiParamSupport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputContentBlock {
  Text { text: String },
  ToolUse {
    id: String,
    name: String,
    input: Value,
  },
  ToolResult {
    tool_use_id: String,
    content: Vec<ToolResultContentBlock>,
    is_error: bool,
  },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolResultContentBlock {
  Text { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputMessage {
  pub role: String,
  pub content: Vec<InputContentBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
  pub name: String,
  pub description: Option<String>,
  pub input_schema: Value,
}

#[derive(Debug, Clone)]
pub enum ToolChoice {
  Auto,
  Required,
  Tool {
    type_name: String,
    function_name: String,
  },
  Custom(Value),
}

impl ToolChoice {
  fn from_function(name: String) -> Self {
    Self::Tool {
      type_name: "function".to_string(),
      function_name: name,
    }
  }
}

impl Serialize for ToolChoice {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    match self {
      Self::Auto => serializer.serialize_str("auto"),
      Self::Required => serializer.serialize_str("required"),
      Self::Tool {
        type_name,
        function_name,
      } => {
        let payload = json!({
          "type": type_name,
          "function": { "name": function_name },
        });
        payload.serialize(serializer)
      }
      Self::Custom(value) => value.serialize(serializer),
    }
  }
}

impl<'de> Deserialize<'de> for ToolChoice {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let value = Value::deserialize(deserializer)?;
    match value {
      Value::String(value) => match value.as_str() {
        "auto" => Ok(Self::Auto),
        "required" => Ok(Self::Required),
        other => Ok(Self::Custom(Value::String(other.to_string()))),
      },
      Value::Object(object) => {
        let function_obj = object.get("function");
        if let (Some(Value::String(type_name)), Some(Value::Object(function_obj))) =
          (object.get("type"), function_obj)
        {
          let function_name = function_obj
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
          return Ok(Self::Tool {
            type_name: type_name.to_string(),
            function_name,
          });
        }
        Ok(Self::Custom(Value::Object(object)))
      }
      other => Ok(Self::Custom(other)),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
  Low,
  Medium,
  High,
}

impl ReasoningEffort {
  fn as_str(&self) -> &'static str {
    match self {
      Self::Low => "low",
      Self::Medium => "medium",
      Self::High => "high",
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatComposeRequest {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub max_tokens: Option<u32>,
  #[serde(default)]
  pub messages: Vec<InputMessage>,
  #[serde(default)]
  pub user_input: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub system: Option<String>,
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub stream: bool,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub tools: Option<Vec<ToolDefinition>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub tool_choice: Option<ToolChoice>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub temperature: Option<f64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub top_p: Option<f64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub frequency_penalty: Option<f64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub presence_penalty: Option<f64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub stop: Option<Vec<String>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub reasoning_effort: Option<ReasoningEffort>,
}

impl ChatComposeRequest {
  fn effective_messages(&self, default_system: &str, provider: AiProvider) -> Vec<InputMessage> {
    let mut messages = self.messages.clone();

    if matches!(provider, AiProvider::OpenAICompatible) {
      if let Some(system) = &self.system {
        messages.insert(
          0,
          InputMessage {
            role: "system".to_string(),
            content: vec![InputContentBlock::Text {
              text: system.clone(),
            }],
          },
        );
      }
    }

    if messages.is_empty() {
      if let Some(user_input) = &self.user_input {
        messages.push(InputMessage {
          role: "user".to_string(),
          content: vec![InputContentBlock::Text {
            text: user_input.clone(),
          }],
        });
      }
    }

    if messages.is_empty() && !default_system.is_empty() {
      messages.push(InputMessage {
        role: "system".to_string(),
        content: vec![InputContentBlock::Text {
          text: default_system.to_string(),
        }],
      });
    }

    messages
  }
}

#[derive(Serialize)]
pub struct ChatComposeResult {
  pub provider: String,
  pub provider_display_name: &'static str,
  pub request_url: String,
  pub request: Value,
  pub support: AiParamSupport,
  pub skipped_params: Vec<String>,
}

#[derive(Debug)]
struct TransportError {
  message: String,
  status: Option<StatusCode>,
  retryable: bool,
  attempts: u32,
}

#[derive(Serialize)]
pub struct ChatSendResult {
  pub provider: String,
  pub request_url: String,
  pub attempts: u32,
  pub status: Option<u16>,
  pub headers: Vec<(String, String)>,
  pub request: Value,
  pub support: AiParamSupport,
  pub skipped_params: Vec<String>,
  pub success: bool,
  pub error: Option<String>,
  pub body: Option<String>,
}

fn is_retryable_status(status: StatusCode) -> bool {
  matches!(
    status,
    StatusCode::TOO_MANY_REQUESTS | StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT
  )
}

fn is_retryable_error(err: &reqwest::Error) -> bool {
  err.is_timeout() || err.is_connect() || err.is_request()
}

fn jittered_backoff_for_attempt(attempt: u32) -> Duration {
  let shift = u64::try_from(attempt).unwrap_or(u64::MAX);
  let capped_shift = shift.saturating_sub(1).min(30);
  let base = (1u64 << capped_shift).min(DEFAULT_MAX_BACKOFF_SECS);
  Duration::from_secs(DEFAULT_INITIAL_BACKOFF_SECS.saturating_mul(base))
}

fn estimate_token_count(request_json: &Value) -> u32 {
  let serialized_len = request_json.to_string().len();
  u32::try_from(serialized_len / 4).unwrap_or(u32::MAX)
}

fn request_headers(auth_header: &str, api_key: &str) -> Result<HeaderMap, TransportError> {
  let mut headers = HeaderMap::new();
  headers.insert(
    CONTENT_TYPE,
    HeaderValue::from_str("application/json")
      .map_err(|error| TransportError {
        message: error.to_string(),
        status: None,
        retryable: false,
        attempts: 0,
      })?,
  );
  if api_key.is_empty() {
    return Ok(headers);
  }

  let auth_name = auth_header.to_ascii_lowercase();
  if auth_name == "authorization" {
    headers.insert(
      AUTHORIZATION,
      HeaderValue::from_str(&format!("Bearer {}", api_key))
        .map_err(|error| TransportError {
          message: error.to_string(),
          status: None,
          retryable: false,
          attempts: 0,
        })?,
    );
  } else {
    let header_name = HeaderName::from_bytes(auth_name.as_bytes()).map_err(|error| TransportError {
      message: error.to_string(),
      status: None,
      retryable: false,
      attempts: 0,
    })?;
    headers.insert(
      header_name,
      HeaderValue::from_str(api_key)
        .map_err(|error| TransportError {
          message: error.to_string(),
          status: None,
          retryable: false,
          attempts: 0,
        })?,
    );
  }

  Ok(headers)
}

fn env_or_optional(key: &str) -> Option<String> {
  env::var(key).ok().filter(|value| !value.trim().is_empty())
}

fn resolve_api_key_for_provider(provider: &AiProvider) -> Option<String> {
  let provider_key = match provider {
    AiProvider::OpenAICompatible => "YUANLING_OPENAI_API_KEY",
    AiProvider::AnthropicCompatible => "YUANLING_ANTHROPIC_API_KEY",
    AiProvider::Custom => "YUANLING_AI_API_KEY",
  };

  env_or_optional(provider_key)
    .or_else(|| env_or_optional("YUANLING_AI_API_KEY"))
    .or_else(|| match provider {
      AiProvider::OpenAICompatible => env_or_optional("OPENAI_API_KEY"),
      AiProvider::AnthropicCompatible => env_or_optional("ANTHROPIC_API_KEY"),
      AiProvider::Custom => None,
    })
}

fn resolve_endpoint_for_provider(provider: &AiProvider) -> String {
  match provider {
    AiProvider::OpenAICompatible => env_or_optional("YUANLING_OPENAI_BASE_URL")
      .or_else(|| env_or_optional("OPENAI_BASE_URL"))
      .or_else(|| env_or_optional("YUANLING_AI_BASE_URL"))
      .or_else(|| env_or_optional("YUANLING_AI_ENDPOINT"))
      .unwrap_or_else(|| provider.default_endpoint().to_string()),
    AiProvider::AnthropicCompatible => env_or_optional("YUANLING_ANTHROPIC_BASE_URL")
      .or_else(|| env_or_optional("ANTHROPIC_BASE_URL"))
      .or_else(|| env_or_optional("YUANLING_AI_BASE_URL"))
      .or_else(|| env_or_optional("YUANLING_AI_ENDPOINT"))
      .unwrap_or_else(|| provider.default_endpoint().to_string()),
    AiProvider::Custom => env_or("YUANLING_AI_ENDPOINT", provider.default_endpoint())
      .to_string(),
  }
}

async fn send_with_retry(
  request: &ChatComposeResult,
  config: &AiModuleConfig,
  request_url: &str,
) -> Result<(Response, u32), TransportError> {
  let mut attempts: u32 = 0;
  let mut last_error: Option<TransportError> = None;
  let timeout = Duration::from_millis(config.model.timeout_ms);
  let request_payload = request.request.clone();
  let request_body = request_payload.to_string();
  let provider = AiProvider::from_raw(&config.model.provider);
  let api_key = resolve_api_key_for_provider(&provider).unwrap_or_default();
  let headers = request_headers(&config.model.auth_header, &api_key)?;
  let client = Client::builder()
    .timeout(timeout)
    .build()
    .map_err(|error| TransportError {
      attempts: 0,
      message: error.to_string(),
      status: None,
      retryable: false,
    })?;

  while attempts < DEFAULT_MAX_RETRIES + 1 {
    attempts += 1;
    let response = client
      .post(request_url)
      .headers(headers.clone())
      .body(request_body.clone())
      .send()
      .await;

    match response {
      Ok(resp) => {
        let status = resp.status();
        if status.is_success() {
          return Ok((resp, attempts));
        }
        let transport_error = TransportError {
          attempts,
          status: Some(status),
          message: format!("http status {status}"),
          retryable: is_retryable_status(status),
        };
        if transport_error.retryable && attempts <= DEFAULT_MAX_RETRIES {
          last_error = Some(transport_error);
        } else {
          return Err(transport_error);
        }
      }
      Err(err) => {
        let transport_error = TransportError {
          attempts,
          message: err.to_string(),
          status: None,
          retryable: is_retryable_error(&err),
        };
        if transport_error.retryable && attempts <= DEFAULT_MAX_RETRIES {
          last_error = Some(transport_error);
        } else {
          return Err(transport_error);
        }
      }
    }

    if attempts <= DEFAULT_MAX_RETRIES {
      tokio::time::sleep(jittered_backoff_for_attempt(attempts)).await;
    }
  }

  Err(last_error.unwrap_or_else(|| TransportError {
    attempts: DEFAULT_MAX_RETRIES + 1,
    message: "retries exhausted".to_string(),
    status: None,
    retryable: false,
  }))
}

fn check_openai_request(payload: &Value) -> Result<(), TransportError> {
  let body_bytes = serde_json::to_vec(payload)
    .map_err(|error| TransportError {
      attempts: 0,
      message: format!("serialize request failed: {error}"),
      status: None,
      retryable: false,
    })?
    .len();
  let limit = OPENAI_MAX_BODY_BYTES;
  if body_bytes > limit {
    return Err(TransportError {
      attempts: 0,
      message: format!("request body too large: {body_bytes} bytes"),
      status: None,
      retryable: false,
    });
  }
  Ok(())
}

fn check_anthropic_request(payload: &Value, _config: &AiModuleConfig) -> Result<(), TransportError> {
  let model_tokens = payload.get("max_tokens").and_then(Value::as_u64).unwrap_or(0);
  let estimated_total = estimate_token_count(payload).saturating_add(u32::try_from(model_tokens).unwrap_or(u32::MAX));
  if estimated_total > ANTHROPIC_DEFAULT_CONTEXT_WINDOW {
    return Err(TransportError {
      attempts: 0,
      message: "estimated context window exceeded".to_string(),
      status: None,
      retryable: false,
    });
  }
  Ok(())
}

impl AiModelConfig {
  fn request_url(&self) -> String {
    let base = self.endpoint.trim_end_matches('/');
    let path = self.request_path.trim().to_string();

    if path.is_empty() {
      return base.to_string();
    }
    if path.starts_with("http://") || path.starts_with("https://") {
      return path;
    }

    let path_without_leading = path.trim_start_matches('/');
    if path_without_leading.is_empty() {
      return base.to_string();
    }

    let path_suffix = format!("/{}", path_without_leading);
    if base
      .to_ascii_lowercase()
      .ends_with(&path_suffix.to_ascii_lowercase())
    {
      return base.to_string();
    }

    let path = self.request_path.trim_start_matches('/');
    format!("{base}/{path}")
  }
}

impl AiModuleConfig {
  pub fn as_view(&self) -> AiModuleConfigView {
    let provider = AiProvider::from_raw(&self.model.provider);
    AiModuleConfigView {
      enabled: self.enabled,
      provider: self.model.provider.clone(),
      display_name: provider.display_name(),
      endpoint: self.model.endpoint.clone(),
      request_path: self.model.request_path.clone(),
      request_url: self.model.request_url(),
      model_id: self.model.model_id.clone(),
      auth_header: self.model.auth_header.clone(),
      prompt_template: self.model.prompt_template.clone(),
      timeout_ms: self.model.timeout_ms,
      has_api_key: self.has_api_key,
      supported_params: provider.supports(),
    }
  }
}

fn env_or(key: &str, default: &str) -> String {
  env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_or_u64(key: &str, default: u64) -> u64 {
  env::var(key)
    .ok()
    .and_then(|raw| raw.parse::<u64>().ok())
    .unwrap_or(default)
}

fn resolve_enabled() -> bool {
  env::var("YUANLING_AI_ENABLED")
    .ok()
    .and_then(|raw| raw.parse::<bool>().ok())
    .unwrap_or(true)
}

pub fn default_config() -> AiModuleConfig {
  AiModuleConfig {
    enabled: true,
    model: AiModelConfig {
      provider: "none".to_string(),
      model_id: "local-default".to_string(),
      endpoint: "http://localhost:8000/v1".to_string(),
      request_path: "/chat/completions".to_string(),
      prompt_template: "You are YUANLING, a practical assistant.".to_string(),
      timeout_ms: 60_000,
      auth_header: "Authorization".to_string(),
    },
    has_api_key: false,
  }
}

pub fn resolve_from_env() -> AiModuleConfig {
  let provider_input = env_or("YUANLING_AI_PROVIDER", OPENAI_COMPATIBLE_PROVIDER);
  let provider = AiProvider::from_raw(&provider_input);
  let endpoint = resolve_endpoint_for_provider(&provider);

  AiModuleConfig {
    enabled: resolve_enabled(),
    model: AiModelConfig {
      provider: provider_input,
      model_id: env_or("YUANLING_AI_MODEL", provider.default_model()).trim().to_string(),
      endpoint,
      request_path: env_or("YUANLING_AI_REQUEST_PATH", provider.default_request_path())
        .trim()
        .to_string(),
      prompt_template: env_or(
        "YUANLING_AI_PROMPT_TEMPLATE",
        "You are YUANLING, a practical assistant.",
      ),
      timeout_ms: env_or_u64("YUANLING_AI_TIMEOUT_MS", 60_000),
      auth_header: env_or("YUANLING_AI_AUTH_HEADER", provider.default_auth_header()),
    },
    has_api_key: resolve_api_key_for_provider(&provider)
      .map(|raw| !raw.trim().is_empty())
      .unwrap_or(false),
  }
}

fn push_skipped(skipped_params: &mut Vec<String>, params: &[&str]) {
  for param in params {
    skipped_params.push((*param).to_string());
  }
}

fn clean_anthropic_body(body: &mut Map<String, Value>, skipped_params: &mut Vec<String>) {
  if let Some(stop) = body.get("stop") {
    if stop.as_array().is_some_and(|items| !items.is_empty()) {
      let stop_sequences = stop.clone();
      body.insert("stop_sequences".to_string(), stop_sequences);
    }
    body.remove("stop");
  }

  if body.remove("frequency_penalty").is_some() {
    push_skipped(skipped_params, &["frequency_penalty"]);
  }

  if body.remove("presence_penalty").is_some() {
    push_skipped(skipped_params, &["presence_penalty"]);
  }

  if body.remove("top_p").is_some() {
    push_skipped(skipped_params, &["top_p"]);
  }

  if body.remove("reasoning_effort").is_some() {
    push_skipped(skipped_params, &["reasoning_effort"]);
  }

  body.remove("betas");
}

pub fn build_chat_payload(request: &ChatComposeRequest, config: &AiModuleConfig) -> ChatComposeResult {
  let provider = AiProvider::from_raw(&config.model.provider);
  let support = provider.supports();
  let mut skipped_params: Vec<String> = Vec::new();
  let model = request.model.clone().unwrap_or_else(|| config.model.model_id.clone());

  let mut body: Map<String, Value> = Map::new();

  let messages = request.effective_messages(&config.model.prompt_template, provider);

  if support.messages {
    body.insert("messages".to_string(), json!(messages));
  } else {
    skipped_params.push("messages".to_string());
  }

  if support.system && provider == AiProvider::AnthropicCompatible {
    if let Some(system) = request.system.clone() {
      body.insert("system".to_string(), json!(system));
    }
  } else if !support.system {
    if request.system.is_some() {
      push_skipped(&mut skipped_params, &["system"]);
    }
  }

  if support.model {
    body.insert("model".to_string(), json!(model));
  } else {
    skipped_params.push("model".to_string());
  }

  if support.max_tokens {
    if let Some(max_tokens) = request.max_tokens {
      if let AiProvider::OpenAICompatible = provider {
        if AiProvider::is_gpt5(&model) {
          body.insert("max_completion_tokens".to_string(), json!(max_tokens));
        } else {
          body.insert("max_tokens".to_string(), json!(max_tokens));
        }
      } else {
        body.insert("max_tokens".to_string(), json!(max_tokens));
      }
    }
  } else if request.max_tokens.is_some() {
    skipped_params.push("max_tokens".to_string());
  }

  if support.stream {
    if request.stream {
      body.insert("stream".to_string(), json!(true));
      if let AiProvider::OpenAICompatible = provider {
        body.insert("stream_options".to_string(), json!({"include_usage": true}));
      }
    }
  } else if request.stream {
    skipped_params.push("stream".to_string());
  }

  if support.tools {
    if let Some(tools) = request.tools.clone() {
      body.insert("tools".to_string(), json!(tools));
    }
  } else if request.tools.is_some() {
    skipped_params.push("tools".to_string());
  }

  if support.tool_choice {
    if let Some(choice) = request.tool_choice.clone() {
      body.insert("tool_choice".to_string(), json!(choice));
    }
  } else if request.tool_choice.is_some() {
    skipped_params.push("tool_choice".to_string());
  }

  if support.temperature {
    if let Some(temperature) = request.temperature {
      if let AiProvider::OpenAICompatible = provider {
        if AiProvider::is_reasoning_model(&model) {
          skipped_params.push("temperature".to_string());
        } else {
          body.insert("temperature".to_string(), json!(temperature));
        }
      } else {
        body.insert("temperature".to_string(), json!(temperature));
      }
    }
  } else if request.temperature.is_some() {
    skipped_params.push("temperature".to_string());
  }

  if support.top_p {
    if let Some(top_p) = request.top_p {
      if let AiProvider::OpenAICompatible = provider {
        if AiProvider::is_reasoning_model(&model) {
          skipped_params.push("top_p".to_string());
        } else {
          body.insert("top_p".to_string(), json!(top_p));
        }
      } else {
        body.insert("top_p".to_string(), json!(top_p));
      }
    }
  } else if request.top_p.is_some() {
    skipped_params.push("top_p".to_string());
  }

  if support.frequency_penalty {
    if let Some(value) = request.frequency_penalty {
      if let AiProvider::OpenAICompatible = provider {
        if AiProvider::is_reasoning_model(&model) {
          skipped_params.push("frequency_penalty".to_string());
        } else {
          body.insert("frequency_penalty".to_string(), json!(value));
        }
      } else {
        body.insert("frequency_penalty".to_string(), json!(value));
      }
    }
  } else if request.frequency_penalty.is_some() {
    skipped_params.push("frequency_penalty".to_string());
  }

  if support.presence_penalty {
    if let Some(value) = request.presence_penalty {
      if let AiProvider::OpenAICompatible = provider {
        if AiProvider::is_reasoning_model(&model) {
          skipped_params.push("presence_penalty".to_string());
        } else {
          body.insert("presence_penalty".to_string(), json!(value));
        }
      } else {
        body.insert("presence_penalty".to_string(), json!(value));
      }
    }
  } else if request.presence_penalty.is_some() {
    skipped_params.push("presence_penalty".to_string());
  }

  if support.stop {
    if let Some(stop) = request.stop.clone() {
      body.insert("stop".to_string(), json!(stop));
    }
  } else if request.stop.is_some() {
    skipped_params.push("stop".to_string());
  }

  if support.reasoning_effort {
    if let Some(effort) = request.reasoning_effort.clone() {
      if AiProvider::is_reasoning_model(&model) {
        body.insert("reasoning_effort".to_string(), json!(effort.as_str()));
      } else {
        skipped_params.push("reasoning_effort".to_string());
      }
    }
  } else if request.reasoning_effort.is_some() {
    skipped_params.push("reasoning_effort".to_string());
  }

  if let AiProvider::AnthropicCompatible = provider {
    clean_anthropic_body(&mut body, &mut skipped_params);
  }

  ChatComposeResult {
    provider: provider.provider_name().to_string(),
    provider_display_name: provider.display_name(),
    request_url: config.model.request_url(),
    request: Value::Object(body),
    support,
    skipped_params,
  }
}

async fn send_message(request: ChatComposeResult, config: &AiModuleConfig) -> ChatSendResult {
  let provider = AiProvider::from_raw(&config.model.provider);
  let request_url = config.model.request_url();

  let preflight_result = match provider {
    AiProvider::OpenAICompatible => check_openai_request(&request.request),
    AiProvider::AnthropicCompatible => check_anthropic_request(&request.request, config),
    AiProvider::Custom => Ok(()),
  };

  if let Err(error) = preflight_result {
    return ChatSendResult {
      provider: request.provider,
      request_url,
      attempts: 0,
      status: None,
      headers: vec![],
      request: request.request,
      support: request.support,
      skipped_params: request.skipped_params,
      success: false,
      error: Some(error.message),
      body: None,
    };
  }

  let send_result = send_with_retry(&request, config, &request_url).await;
  match send_result {
    Ok((response, attempts)) => {
      let status = response.status().as_u16();
      let response_headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
          value
            .to_str()
            .ok()
            .map(|value| (name.to_string(), value.to_string()))
        })
        .collect::<Vec<_>>();
      let body = response.text().await.ok();
      ChatSendResult {
        provider: request.provider,
        request_url,
        attempts,
        status: Some(status),
        headers: response_headers,
        request: request.request,
        support: request.support,
        skipped_params: request.skipped_params,
        success: true,
        error: None,
        body,
      }
    }
    Err(error) => ChatSendResult {
      provider: request.provider,
      request_url,
      attempts: error.attempts,
      status: error.status.map(|status| status.as_u16()),
      headers: vec![],
      request: request.request,
      support: request.support,
      skipped_params: request.skipped_params,
      success: false,
      error: Some(error.message),
      body: None,
    },
  }
}

pub async fn send_chat_request(
  request: ChatComposeRequest,
  config: &AiModuleConfig,
) -> ChatSendResult {
  let payload = build_chat_payload(&request, config);
  send_message(payload, config).await
}

async fn send(Json(request): Json<ChatComposeRequest>) -> Json<ChatSendResult> {
  let config = resolve_from_env();
  Json(send_chat_request(request, &config).await)
}

async fn config() -> Json<AiModuleConfigView> {
  Json(resolve_from_env().as_view())
}

async fn compose(Json(request): Json<ChatComposeRequest>) -> Json<ChatComposeResult> {
  Json(build_chat_payload(&request, &resolve_from_env()))
}

pub fn router() -> Router {
  Router::new()
    .route("/yuanling/ai/config", get(config))
    .route("/yuanling/ai/compose", post(compose))
    .route("/yuanling/ai/send", post(send))
}
