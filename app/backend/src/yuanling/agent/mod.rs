use super::{ai, contact, context, mcp, tools};
use crate::spiritkind;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Mutex, OnceLock};

const DEFAULT_ENABLED: bool = true;
const DEFAULT_STREAMING_ENABLED: bool = true;
const DEFAULT_USER_ID: &str = "000000";
const DEFAULT_ENTRY_ID: &str = "000001";
const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentModuleConfig {
  pub enabled: bool,
  pub default_user_id: String,
  pub default_entry_id: String,
  pub user_ids: BTreeSet<String>,
  pub streaming_enabled: bool,
  pub max_tool_iterations: usize,
  pub max_output_tokens: u32,
}

impl AgentModuleConfig {
  pub fn as_view(&self) -> AgentModuleConfigView {
    AgentModuleConfigView {
      enabled: self.enabled,
      default_user_id: self.default_user_id.clone(),
      default_entry_id: self.default_entry_id.clone(),
      user_ids: self.user_ids.clone(),
      streaming_enabled: self.streaming_enabled,
      max_tool_iterations: self.max_tool_iterations,
      max_output_tokens: self.max_output_tokens,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentModuleConfigView {
  pub enabled: bool,
  pub default_user_id: String,
  pub default_entry_id: String,
  pub user_ids: BTreeSet<String>,
  pub streaming_enabled: bool,
  pub max_tool_iterations: usize,
  pub max_output_tokens: u32,
}

#[derive(Clone)]
pub struct AgentRunOptions {
  pub agent_config: AgentModuleConfig,
  pub ai_config: ai::AiModuleConfig,
  pub context_config: context::ContextModuleConfig,
  pub contact_config: contact::ContactModuleConfig,
  pub tools_config: tools::ToolsModuleConfig,
  pub mcp_tools_enabled: bool,
}

impl AgentRunOptions {
  pub fn from_env() -> Self {
    Self {
      agent_config: resolve_from_env(),
      ai_config: ai::resolve_from_env(),
      context_config: context::resolve_from_env(),
      contact_config: contact::resolve_from_env(),
      tools_config: tools::resolve_from_env(),
      mcp_tools_enabled: true,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDeliveredMessage {
  pub message_id: String,
  pub from_yuanling_id: String,
  pub to_yuanling_id: String,
  pub content: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRunResult {
  pub user_messages: Vec<AgentDeliveredMessage>,
  pub processed_yuanlings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentToolUse {
  pub id: String,
  pub name: String,
  pub input: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AgentAssistantTurn {
  pub text: Option<String>,
  pub tool_uses: Vec<AgentToolUse>,
  pub usage: Option<context::ContextTokenUsage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
  TextDelta { yuanling_id: String, text: String },
  ToolUse { yuanling_id: String, tool_name: String },
  ToolResult { yuanling_id: String, tool_name: String, is_error: bool },
  Done { yuanling_id: String },
  Error { yuanling_id: String, message: String },
}

pub trait AgentEventSink {
  fn emit(&mut self, event: AgentEvent);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentError {
  Disabled,
  Ai(String),
  Contact(String),
  Context(String),
  Tool(String),
  Parse(String),
  MaxToolIterations { limit: usize },
}

impl std::fmt::Display for AgentError {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Disabled => write!(formatter, "agent module is disabled"),
      Self::Ai(message)
      | Self::Contact(message)
      | Self::Context(message)
      | Self::Tool(message)
      | Self::Parse(message) => write!(formatter, "{message}"),
      Self::MaxToolIterations { limit } => {
        write!(formatter, "agent tool loop exceeded max iterations: {limit}")
      }
    }
  }
}

impl std::error::Error for AgentError {}

pub trait AgentAiClient {
  fn send<'a>(
    &'a mut self,
    request: ai::ChatComposeRequest,
    config: &'a ai::AiModuleConfig,
  ) -> Pin<Box<dyn Future<Output = ai::ChatSendResult> + 'a>>;
}

pub struct LiveAiClient;

impl AgentAiClient for LiveAiClient {
  fn send<'a>(
    &'a mut self,
    request: ai::ChatComposeRequest,
    config: &'a ai::AiModuleConfig,
  ) -> Pin<Box<dyn Future<Output = ai::ChatSendResult> + 'a>> {
    Box::pin(ai::send_chat_request(request, config))
  }
}

pub fn resolve_from_env() -> AgentModuleConfig {
  let default_user_id = env_or("YUANLING_AGENT_DEFAULT_USER_ID", DEFAULT_USER_ID);
  let mut user_ids = env_or_list("YUANLING_AGENT_USER_IDS")
    .into_iter()
    .collect::<BTreeSet<_>>();
  user_ids.insert(default_user_id.clone());

  AgentModuleConfig {
    enabled: env_or_bool("YUANLING_AGENT_ENABLED", DEFAULT_ENABLED),
    default_user_id,
    default_entry_id: env_or("YUANLING_AGENT_DEFAULT_ENTRY_ID", DEFAULT_ENTRY_ID),
    user_ids,
    streaming_enabled: env_or_bool("YUANLING_AGENT_STREAMING_ENABLED", DEFAULT_STREAMING_ENABLED),
    max_tool_iterations: env_or_usize("YUANLING_AGENT_MAX_TOOL_ITERATIONS", 0),
    max_output_tokens: env_or_u32("YUANLING_AGENT_MAX_OUTPUT_TOKENS", DEFAULT_MAX_OUTPUT_TOKENS),
  }
}

pub async fn receive_user_message(content: impl Into<String>) -> Result<AgentRunResult, AgentError> {
  let options = AgentRunOptions::from_env();
  let from = options.agent_config.default_user_id.clone();
  let to = options.agent_config.default_entry_id.clone();
  let mut ai_client = LiveAiClient;
  receive_message_with_client(&from, &to, content.into(), options, &mut ai_client, None, None).await
}

pub async fn receive_message(
  from_yuanling_id: &str,
  to_yuanling_id: &str,
  content: impl Into<String>,
) -> Result<AgentRunResult, AgentError> {
  let options = AgentRunOptions::from_env();
  let mut ai_client = LiveAiClient;
  receive_message_with_client(
    from_yuanling_id,
    to_yuanling_id,
    content.into(),
    options,
    &mut ai_client,
    None,
    None,
  )
  .await
}

pub async fn tick_yuanling(yuanling_id: &str) -> Result<AgentRunResult, AgentError> {
  let options = AgentRunOptions::from_env();
  let mut ai_client = LiveAiClient;
  dispatch_yuanling_queue(
    VecDeque::from([yuanling_id.to_string()]),
    options,
    &mut ai_client,
    None,
    None,
  )
  .await
}

pub async fn receive_message_with_client<C: AgentAiClient + ?Sized>(
  from_yuanling_id: &str,
  to_yuanling_id: &str,
  content: String,
  options: AgentRunOptions,
  ai_client: &mut C,
  event_sink: Option<&mut dyn AgentEventSink>,
  prompter: Option<&mut dyn tools::ToolPermissionPrompter>,
) -> Result<AgentRunResult, AgentError> {
  if !options.agent_config.enabled {
    return Err(AgentError::Disabled);
  }

  let sent = contact::send_message(
    from_yuanling_id,
    to_yuanling_id,
    &content,
    &options.contact_config,
  )
  .map_err(|error| AgentError::Contact(error.to_string()))?;

  let mut result = AgentRunResult::default();
  if options.agent_config.user_ids.contains(to_yuanling_id) {
    result.user_messages.push(AgentDeliveredMessage {
      message_id: sent.message_id,
      from_yuanling_id: from_yuanling_id.to_string(),
      to_yuanling_id: to_yuanling_id.to_string(),
      content,
    });
    return Ok(result);
  }

  dispatch_yuanling_queue(
    VecDeque::from([to_yuanling_id.to_string()]),
    options,
    ai_client,
    event_sink,
    prompter,
  )
  .await
}

async fn dispatch_yuanling_queue<C: AgentAiClient + ?Sized>(
  mut queue: VecDeque<String>,
  options: AgentRunOptions,
  ai_client: &mut C,
  mut event_sink: Option<&mut dyn AgentEventSink>,
  mut prompter: Option<&mut dyn tools::ToolPermissionPrompter>,
) -> Result<AgentRunResult, AgentError> {
  let mut result = AgentRunResult::default();

  while let Some(yuanling_id) = queue.pop_front() {
    if options.agent_config.user_ids.contains(&yuanling_id) {
      continue;
    }
    let outcome = process_yuanling(
      &yuanling_id,
      &options,
      ai_client,
      &mut event_sink,
      &mut prompter,
    )
    .await?;
    result.processed_yuanlings.extend(outcome.processed_yuanlings);
    result.user_messages.extend(outcome.user_messages);
    for target in outcome.dispatch_targets {
      if !options.agent_config.user_ids.contains(&target) {
        queue.push_back(target);
      }
    }
  }

  Ok(result)
}

#[derive(Default)]
struct ProcessOutcome {
  processed_yuanlings: Vec<String>,
  dispatch_targets: Vec<String>,
  user_messages: Vec<AgentDeliveredMessage>,
}

async fn process_yuanling<C: AgentAiClient + ?Sized>(
  yuanling_id: &str,
  options: &AgentRunOptions,
  ai_client: &mut C,
  event_sink: &mut Option<&mut dyn AgentEventSink>,
  prompter: &mut Option<&mut dyn tools::ToolPermissionPrompter>,
) -> Result<ProcessOutcome, AgentError> {
  let Some(_guard) = ActiveAgentGuard::enter(yuanling_id) else {
    return Ok(ProcessOutcome::default());
  };

  let mut outcome = ProcessOutcome::default();
  loop {
    let batch = contact::take_ready_messages(yuanling_id, &options.contact_config)
      .map_err(|error| AgentError::Contact(error.to_string()))?;
    if batch.status != "ready" {
      break;
    }

    for message in &batch.messages {
      context::append_message(
        yuanling_id,
        context::ContextMessage::user_text(format!(
          "Message from {}: {}",
          message.from_yuanling_id, message.content
        )),
        &options.context_config,
      )
      .map_err(|error| AgentError::Context(error.to_string()))?;
    }

    let loop_result = run_ai_tool_loop(
      yuanling_id,
      options,
      ai_client,
      event_sink,
      prompter,
    )
    .await;
    contact::finish_contact_processing(yuanling_id, &options.contact_config)
      .map_err(|error| AgentError::Contact(error.to_string()))?;

    let loop_outcome = loop_result?;
    outcome.processed_yuanlings.push(yuanling_id.to_string());
    outcome.dispatch_targets.extend(loop_outcome.dispatch_targets);
    outcome.user_messages.extend(loop_outcome.user_messages);
  }

  Ok(outcome)
}

async fn run_ai_tool_loop<C: AgentAiClient + ?Sized>(
  yuanling_id: &str,
  options: &AgentRunOptions,
  ai_client: &mut C,
  event_sink: &mut Option<&mut dyn AgentEventSink>,
  prompter: &mut Option<&mut dyn tools::ToolPermissionPrompter>,
) -> Result<ProcessOutcome, AgentError> {
  let mut outcome = ProcessOutcome::default();
  let mut tool_iterations = 0usize;

  loop {
    let built = context::build_context(yuanling_id, &options.context_config)
      .await
      .map_err(|error| AgentError::Context(error.to_string()))?;
    let (system, messages) = context_messages_to_ai_messages(built.messages);
    let registry = build_tool_registry(options.mcp_tools_enabled).await;
    let spiritkind_allowed_tools = spiritkind_tools_for_yuanling(yuanling_id, &registry);
    let tools = tools::definitions_for_yuanling(
      &registry,
      yuanling_id,
      &options.tools_config,
      spiritkind_allowed_tools.as_ref(),
    )
    .unwrap_or_default();
    let request = ai::ChatComposeRequest {
      model: None,
      max_tokens: Some(options.agent_config.max_output_tokens),
      messages,
      user_input: None,
      system,
      stream: options.agent_config.streaming_enabled,
      tools: (!tools.is_empty()).then_some(tools),
      tool_choice: Some(ai::ToolChoice::Auto),
      temperature: None,
      top_p: None,
      frequency_penalty: None,
      presence_penalty: None,
      stop: None,
      reasoning_effort: None,
    };

    let response = ai_client.send(request, &options.ai_config).await;
    let assistant_turn = parse_agent_assistant_turn(&response)?;
    if let Some(text) = assistant_turn.text.as_ref().filter(|value| !value.trim().is_empty()) {
      emit_event(
        event_sink,
        AgentEvent::TextDelta {
          yuanling_id: yuanling_id.to_string(),
          text: text.clone(),
        },
      );
    }

    append_assistant_turn(yuanling_id, &assistant_turn, &options.context_config)?;

    if assistant_turn.tool_uses.is_empty() {
      emit_event(
        event_sink,
        AgentEvent::Done {
          yuanling_id: yuanling_id.to_string(),
        },
      );
      break;
    }

    if options.agent_config.max_tool_iterations > 0
      && tool_iterations >= options.agent_config.max_tool_iterations
    {
      let error = AgentError::MaxToolIterations {
        limit: options.agent_config.max_tool_iterations,
      };
      emit_event(
        event_sink,
        AgentEvent::Error {
          yuanling_id: yuanling_id.to_string(),
          message: error.to_string(),
        },
      );
      return Err(error);
    }
    tool_iterations = tool_iterations.saturating_add(1);

    let mut executor = AgentToolExecutor::new(options)?;
    for tool_use in assistant_turn.tool_uses {
      emit_event(
        event_sink,
        AgentEvent::ToolUse {
          yuanling_id: yuanling_id.to_string(),
          tool_name: tool_use.name.clone(),
        },
      );
      let execution = if let Some(prompter) = prompter.as_mut() {
        registry.execute_for_yuanling(
          yuanling_id,
          &tool_use.name,
          &tool_use.input,
          &options.tools_config,
          spiritkind_allowed_tools.as_ref(),
          &mut executor,
          Some(&mut **prompter),
        )
      } else {
        registry.execute_for_yuanling(
          yuanling_id,
          &tool_use.name,
          &tool_use.input,
          &options.tools_config,
          spiritkind_allowed_tools.as_ref(),
          &mut executor,
          None,
        )
      };
      let (output, is_error) = match execution {
        Ok(output) => (
          serde_json::to_string_pretty(&output.output).unwrap_or_else(|_| output.output.to_string()),
          false,
        ),
        Err(error) => (error.to_string(), true),
      };
      context::append_message(
        yuanling_id,
        context::ContextMessage {
          role: context::ContextRole::Tool,
          blocks: vec![context::ContextBlock::ToolResult {
            tool_use_id: tool_use.id,
            tool_name: tool_use.name.clone(),
            output,
            is_error,
          }],
          usage: None,
        },
        &options.context_config,
      )
      .map_err(|error| AgentError::Context(error.to_string()))?;
      emit_event(
        event_sink,
        AgentEvent::ToolResult {
          yuanling_id: yuanling_id.to_string(),
          tool_name: tool_use.name,
          is_error,
        },
      );
    }

    outcome
      .dispatch_targets
      .extend(std::mem::take(&mut executor.dispatch_targets));
    outcome
      .user_messages
      .extend(std::mem::take(&mut executor.user_messages));
  }

  Ok(outcome)
}

fn spiritkind_tools_for_yuanling(
  yuanling_id: &str,
  registry: &tools::ToolRegistry,
) -> Option<BTreeSet<String>> {
  let config = spiritkind::resolve_from_env();
  let values = spiritkind::tools_for(yuanling_id, &config).ok()?;
  if values.is_empty() {
    return None;
  }
  registry.normalize_allowed_tools(&values).ok().flatten()
}

async fn build_tool_registry(mcp_tools_enabled: bool) -> tools::ToolRegistry {
  if !mcp_tools_enabled {
    return tools::ToolRegistry::builtin();
  }
  let mcp_config = mcp::resolve_from_env();
  if !mcp_config.enabled || mcp_config.servers.is_empty() {
    return tools::ToolRegistry::builtin();
  }
  tools::registry_with_mcp_tools(&mcp_config)
    .await
    .unwrap_or_else(|_| tools::ToolRegistry::builtin())
}

fn append_assistant_turn(
  yuanling_id: &str,
  assistant_turn: &AgentAssistantTurn,
  config: &context::ContextModuleConfig,
) -> Result<(), AgentError> {
  let mut blocks = Vec::new();
  if let Some(text) = assistant_turn.text.as_ref().filter(|value| !value.trim().is_empty()) {
    blocks.push(context::ContextBlock::Text { text: text.clone() });
  }
  blocks.extend(assistant_turn.tool_uses.iter().map(|tool| context::ContextBlock::ToolUse {
    id: tool.id.clone(),
    name: tool.name.clone(),
    input: tool.input.clone(),
  }));
  if blocks.is_empty() {
    return Ok(());
  }
  context::append_message(
    yuanling_id,
    context::ContextMessage {
      role: context::ContextRole::Assistant,
      blocks,
      usage: assistant_turn.usage,
    },
    config,
  )
  .map_err(|error| AgentError::Context(error.to_string()))?;
  Ok(())
}

fn context_messages_to_ai_messages(
  messages: Vec<context::ContextMessage>,
) -> (Option<String>, Vec<ai::InputMessage>) {
  let mut system_parts = Vec::new();
  let mut ai_messages = Vec::new();

  for message in messages {
    if message.role == context::ContextRole::System {
      for block in message.blocks {
        if let context::ContextBlock::Text { text } = block {
          system_parts.push(text);
        }
      }
      continue;
    }

    ai_messages.push(ai::InputMessage {
      role: match message.role {
        context::ContextRole::System => "system",
        context::ContextRole::User => "user",
        context::ContextRole::Assistant => "assistant",
        context::ContextRole::Tool => "tool",
      }
      .to_string(),
      content: message
        .blocks
        .into_iter()
        .map(context_block_to_ai_block)
        .collect(),
    });
  }

  let system = (!system_parts.is_empty()).then(|| system_parts.join("\n\n"));
  (system, ai_messages)
}

fn context_block_to_ai_block(block: context::ContextBlock) -> ai::InputContentBlock {
  match block {
    context::ContextBlock::Text { text } => ai::InputContentBlock::Text { text },
    context::ContextBlock::ToolUse { id, name, input } => {
      ai::InputContentBlock::ToolUse { id, name, input }
    }
    context::ContextBlock::ToolResult {
      tool_use_id,
      output,
      is_error,
      ..
    } => ai::InputContentBlock::ToolResult {
      tool_use_id,
      content: vec![ai::ToolResultContentBlock::Text { text: output }],
      is_error,
    },
  }
}

struct AgentToolExecutor {
  inner: Option<tools::BuiltinToolExecutor>,
  tools_config: tools::ToolsModuleConfig,
  contact_config: contact::ContactModuleConfig,
  user_ids: BTreeSet<String>,
  dispatch_targets: Vec<String>,
  user_messages: Vec<AgentDeliveredMessage>,
}

impl AgentToolExecutor {
  fn new(options: &AgentRunOptions) -> Result<Self, AgentError> {
    Ok(Self {
      inner: None,
      tools_config: options.tools_config.clone(),
      contact_config: options.contact_config.clone(),
      user_ids: options.agent_config.user_ids.clone(),
      dispatch_targets: Vec::new(),
      user_messages: Vec::new(),
    })
  }

  fn inner(&mut self) -> Result<&mut tools::BuiltinToolExecutor, tools::ToolError> {
    if self.inner.is_none() {
      self.inner = Some(tools::BuiltinToolExecutor::from_config(&self.tools_config)?);
    }
    Ok(self.inner.as_mut().expect("inner executor should exist"))
  }
}

impl Drop for AgentToolExecutor {
  fn drop(&mut self) {
    if let Some(inner) = self.inner.take() {
      let _ = std::thread::spawn(move || drop(inner)).join();
    }
  }
}

impl tools::ToolExecutor for AgentToolExecutor {
  fn execute(
    &mut self,
    tool_name: &str,
    input: &Value,
  ) -> Result<tools::ToolExecutionOutput, tools::ToolError> {
    if tool_name != "send_message" {
      return self.inner()?.execute(tool_name, input);
    }

    let from = input
      .get("from_yuanling_id")
      .and_then(Value::as_str)
      .ok_or_else(|| tools::ToolError::InvalidInput("from_yuanling_id is required".to_string()))?;
    let to = input
      .get("to_yuanling_id")
      .and_then(Value::as_str)
      .ok_or_else(|| tools::ToolError::InvalidInput("to_yuanling_id is required".to_string()))?;
    let content = input
      .get("content")
      .and_then(Value::as_str)
      .ok_or_else(|| tools::ToolError::InvalidInput("content is required".to_string()))?;
    let sent = contact::send_message(from, to, content, &self.contact_config)
      .map_err(|error| tools::ToolError::ExecutionFailed(error.to_string()))?;

    if self.user_ids.contains(to) {
      self.user_messages.push(AgentDeliveredMessage {
        message_id: sent.message_id.clone(),
        from_yuanling_id: from.to_string(),
        to_yuanling_id: to.to_string(),
        content: content.to_string(),
      });
    } else {
      self.dispatch_targets.push(to.to_string());
    }

    Ok(tools::ToolExecutionOutput {
      tool_name: tool_name.to_string(),
      output: json!(sent),
      permission_outcome: None,
    })
  }
}

pub fn parse_agent_assistant_turn(result: &ai::ChatSendResult) -> Result<AgentAssistantTurn, AgentError> {
  if !result.success {
    return Err(AgentError::Ai(
      result.error.clone().unwrap_or_else(|| "AI request failed".to_string()),
    ));
  }
  let body = result
    .body
    .as_deref()
    .ok_or_else(|| AgentError::Parse("AI response body is empty".to_string()))?;
  if body.lines().any(|line| line.trim_start().starts_with("data:")) {
    return parse_streaming_turn(&result.provider, body);
  }
  let value = serde_json::from_str::<Value>(body)
    .map_err(|error| AgentError::Parse(format!("failed to parse AI response: {error}")))?;
  if result.provider.contains("anthropic") {
    parse_anthropic_turn(&value)
  } else {
    parse_openai_turn(&value)
  }
}

fn parse_openai_turn(value: &Value) -> Result<AgentAssistantTurn, AgentError> {
  let message = value
    .get("choices")
    .and_then(Value::as_array)
    .and_then(|choices| choices.first())
    .and_then(|choice| choice.get("message"))
    .ok_or_else(|| AgentError::Parse("OpenAI response missing choices[0].message".to_string()))?;
  let text = message
    .get("content")
    .and_then(Value::as_str)
    .map(str::to_string)
    .filter(|value| !value.is_empty());
  let tool_uses = message
    .get("tool_calls")
    .and_then(Value::as_array)
    .map(|calls| calls.iter().filter_map(parse_openai_tool_call).collect())
    .unwrap_or_default();
  Ok(AgentAssistantTurn {
    text,
    tool_uses,
    usage: parse_openai_usage(value),
  })
}

fn parse_openai_tool_call(value: &Value) -> Option<AgentToolUse> {
  let function = value.get("function")?;
  let name = function.get("name")?.as_str()?.to_string();
  let arguments = function
    .get("arguments")
    .and_then(Value::as_str)
    .unwrap_or("{}");
  Some(AgentToolUse {
    id: value.get("id").and_then(Value::as_str).unwrap_or(&name).to_string(),
    name,
    input: serde_json::from_str(arguments).unwrap_or_else(|_| json!({})),
  })
}

fn parse_openai_usage(value: &Value) -> Option<context::ContextTokenUsage> {
  let usage = value.get("usage")?;
  let input_tokens = usage
    .get("prompt_tokens")
    .and_then(Value::as_u64)
    .and_then(|value| u32::try_from(value).ok())
    .unwrap_or(0);
  let output_tokens = usage
    .get("completion_tokens")
    .and_then(Value::as_u64)
    .and_then(|value| u32::try_from(value).ok())
    .unwrap_or(0);
  let total_tokens = usage
    .get("total_tokens")
    .and_then(Value::as_u64)
    .and_then(|value| u32::try_from(value).ok())
    .unwrap_or(input_tokens.saturating_add(output_tokens));
  Some(context::ContextTokenUsage {
    input_tokens,
    output_tokens,
    total_tokens,
  })
}

fn parse_anthropic_turn(value: &Value) -> Result<AgentAssistantTurn, AgentError> {
  let content = value
    .get("content")
    .and_then(Value::as_array)
    .ok_or_else(|| AgentError::Parse("Anthropic response missing content".to_string()))?;
  let mut text_parts = Vec::new();
  let mut tool_uses = Vec::new();
  for block in content {
    match block.get("type").and_then(Value::as_str) {
      Some("text") => {
        if let Some(text) = block.get("text").and_then(Value::as_str) {
          text_parts.push(text.to_string());
        }
      }
      Some("tool_use") => {
        let name = block
          .get("name")
          .and_then(Value::as_str)
          .unwrap_or("")
          .to_string();
        if !name.is_empty() {
          tool_uses.push(AgentToolUse {
            id: block
              .get("id")
              .and_then(Value::as_str)
              .unwrap_or(&name)
              .to_string(),
            name,
            input: block.get("input").cloned().unwrap_or_else(|| json!({})),
          });
        }
      }
      _ => {}
    }
  }
  Ok(AgentAssistantTurn {
    text: (!text_parts.is_empty()).then(|| text_parts.join("")),
    tool_uses,
    usage: parse_anthropic_usage(value),
  })
}

fn parse_anthropic_usage(value: &Value) -> Option<context::ContextTokenUsage> {
  let usage = value.get("usage")?;
  let input_tokens = usage
    .get("input_tokens")
    .and_then(Value::as_u64)
    .and_then(|value| u32::try_from(value).ok())
    .unwrap_or(0);
  let output_tokens = usage
    .get("output_tokens")
    .and_then(Value::as_u64)
    .and_then(|value| u32::try_from(value).ok())
    .unwrap_or(0);
  Some(context::ContextTokenUsage {
    input_tokens,
    output_tokens,
    total_tokens: input_tokens.saturating_add(output_tokens),
  })
}

fn parse_streaming_turn(provider: &str, body: &str) -> Result<AgentAssistantTurn, AgentError> {
  if provider.contains("anthropic") {
    parse_anthropic_streaming_turn(body)
  } else {
    parse_openai_streaming_turn(body)
  }
}

fn parse_openai_streaming_turn(body: &str) -> Result<AgentAssistantTurn, AgentError> {
  let mut text = String::new();
  let mut tools = BTreeMap::<usize, (String, String, String)>::new();
  for value in streaming_data_values(body) {
    let Some(delta) = value
      .get("choices")
      .and_then(Value::as_array)
      .and_then(|choices| choices.first())
      .and_then(|choice| choice.get("delta"))
    else {
      continue;
    };
    if let Some(content) = delta.get("content").and_then(Value::as_str) {
      text.push_str(content);
    }
    if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
      for call in tool_calls {
        let index = call.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
        let entry = tools.entry(index).or_default();
        if let Some(id) = call.get("id").and_then(Value::as_str) {
          entry.0 = id.to_string();
        }
        if let Some(function) = call.get("function") {
          if let Some(name) = function.get("name").and_then(Value::as_str) {
            entry.1 = name.to_string();
          }
          if let Some(arguments) = function.get("arguments").and_then(Value::as_str) {
            entry.2.push_str(arguments);
          }
        }
      }
    }
  }
  let tool_uses = tools
    .into_values()
    .filter(|(_, name, _)| !name.is_empty())
    .map(|(id, name, arguments)| AgentToolUse {
      id: if id.is_empty() { name.clone() } else { id },
      name,
      input: serde_json::from_str(&arguments).unwrap_or_else(|_| json!({})),
    })
    .collect();
  Ok(AgentAssistantTurn {
    text: (!text.is_empty()).then_some(text),
    tool_uses,
    usage: None,
  })
}

fn parse_anthropic_streaming_turn(body: &str) -> Result<AgentAssistantTurn, AgentError> {
  let mut text = String::new();
  let mut blocks = BTreeMap::<usize, (String, String, String)>::new();
  for value in streaming_data_values(body) {
    match value.get("type").and_then(Value::as_str) {
      Some("content_block_start") => {
        let index = value.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
        let block = value.get("content_block").unwrap_or(&Value::Null);
        if block.get("type").and_then(Value::as_str) == Some("tool_use") {
          let name = block.get("name").and_then(Value::as_str).unwrap_or("").to_string();
          let id = block.get("id").and_then(Value::as_str).unwrap_or(&name).to_string();
          blocks.insert(index, (id, name, String::new()));
        }
      }
      Some("content_block_delta") => {
        let index = value.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
        let delta = value.get("delta").unwrap_or(&Value::Null);
        match delta.get("type").and_then(Value::as_str) {
          Some("text_delta") => {
            if let Some(part) = delta.get("text").and_then(Value::as_str) {
              text.push_str(part);
            }
          }
          Some("input_json_delta") => {
            if let Some(part) = delta.get("partial_json").and_then(Value::as_str) {
              blocks.entry(index).or_default().2.push_str(part);
            }
          }
          _ => {}
        }
      }
      _ => {}
    }
  }
  let tool_uses = blocks
    .into_values()
    .filter(|(_, name, _)| !name.is_empty())
    .map(|(id, name, arguments)| AgentToolUse {
      id,
      name,
      input: serde_json::from_str(&arguments).unwrap_or_else(|_| json!({})),
    })
    .collect();
  Ok(AgentAssistantTurn {
    text: (!text.is_empty()).then_some(text),
    tool_uses,
    usage: None,
  })
}

fn streaming_data_values(body: &str) -> Vec<Value> {
  body
    .lines()
    .filter_map(|line| line.trim().strip_prefix("data:"))
    .map(str::trim)
    .filter(|line| !line.is_empty() && *line != "[DONE]")
    .filter_map(|line| serde_json::from_str::<Value>(line).ok())
    .collect()
}

fn emit_event(event_sink: &mut Option<&mut dyn AgentEventSink>, event: AgentEvent) {
  if let Some(sink) = event_sink.as_deref_mut() {
    sink.emit(event);
  }
}

struct ActiveAgentGuard {
  yuanling_id: String,
}

impl ActiveAgentGuard {
  fn enter(yuanling_id: &str) -> Option<Self> {
    let mut active = active_agents().lock().ok()?;
    if active.contains(yuanling_id) {
      return None;
    }
    active.insert(yuanling_id.to_string());
    Some(Self {
      yuanling_id: yuanling_id.to_string(),
    })
  }
}

impl Drop for ActiveAgentGuard {
  fn drop(&mut self) {
    if let Ok(mut active) = active_agents().lock() {
      active.remove(&self.yuanling_id);
    }
  }
}

fn active_agents() -> &'static Mutex<BTreeSet<String>> {
  static ACTIVE: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();
  ACTIVE.get_or_init(|| Mutex::new(BTreeSet::new()))
}

fn env_or(key: &str, default: &str) -> String {
  env::var(key)
    .ok()
    .map(|value| value.trim().to_string())
    .filter(|value| !value.is_empty())
    .unwrap_or_else(|| default.to_string())
}

fn env_or_bool(key: &str, default: bool) -> bool {
  env::var(key)
    .ok()
    .and_then(|value| value.trim().parse::<bool>().ok())
    .unwrap_or(default)
}

fn env_or_usize(key: &str, default: usize) -> usize {
  env::var(key)
    .ok()
    .and_then(|value| value.trim().parse::<usize>().ok())
    .unwrap_or(default)
}

fn env_or_u32(key: &str, default: u32) -> u32 {
  env::var(key)
    .ok()
    .and_then(|value| value.trim().parse::<u32>().ok())
    .unwrap_or(default)
}

fn env_or_list(key: &str) -> Vec<String> {
  env::var(key)
    .ok()
    .map(|raw| {
      raw
        .split(|ch: char| ch == ',' || ch.is_whitespace())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
    })
    .unwrap_or_default()
}

