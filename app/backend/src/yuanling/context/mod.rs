use super::{
  ai::{self, ChatComposeRequest, InputContentBlock, InputMessage},
  skills,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::fmt::{Display, Formatter};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CONTEXT_STATE_VERSION: u32 = 1;
const DEFAULT_MAX_TURNS: usize = 12;
const DEFAULT_MAX_CONTEXT_TOKENS: usize = 12_000;
const DEFAULT_KEEP_SYSTEM_PROMPT: bool = true;
const DEFAULT_STATE_ENABLED: bool = true;
const DEFAULT_CONTEXT_TTL_MINUTES: usize = 30;
const DEFAULT_AUTO_COMPACT_ENABLED: bool = true;
const DEFAULT_AI_COMPACT_ENABLED: bool = true;
const DEFAULT_COMPACT_MAX_OUTPUT_TOKENS: u32 = 1_200;
const DEFAULT_PRESERVE_RECENT_MESSAGES: usize = 4;
const DEFAULT_ROTATE_AFTER_BYTES: u64 = 256 * 1024;
const DEFAULT_MAX_ROTATED_FILES: usize = 3;
const COMPACT_CONTINUATION_PREAMBLE: &str =
  "This Yuanling context is being continued from compacted context. The summary below covers earlier conversation history.\n\n";
const DEFAULT_COMPACT_SYSTEM_PROMPT: &str =
  "Summarize the earlier Yuanling context for a continuing AI agent. Preserve decisions, user intent, open tasks, tool results, files, constraints, and anything needed to resume without asking the user to repeat themselves. Return only the summary.";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextRetentionMode {
  TailTurns,
  TailTokens,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextExpireAction {
  Archive,
  Ignore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextModuleConfig {
  pub enabled: bool,
  pub state_enabled: bool,
  pub context_ttl_minutes: usize,
  pub retention_mode: ContextRetentionMode,
  pub max_turns: usize,
  pub max_tokens: usize,
  pub keep_system_prompt: bool,
  pub storage_dir: String,
  pub auto_compact_enabled: bool,
  pub compact_threshold_tokens: usize,
  pub preserve_recent_messages: usize,
  pub ai_compact_enabled: bool,
  pub compact_max_output_tokens: u32,
  pub compact_system_prompt: String,
  pub input_token_price_per_1m: f64,
  pub output_token_price_per_1m: f64,
  pub rotate_after_bytes: u64,
  pub max_rotated_files: usize,
  pub expire_action: ContextExpireAction,
}

impl Default for ContextModuleConfig {
  fn default() -> Self {
    Self {
      enabled: true,
      state_enabled: DEFAULT_STATE_ENABLED,
      context_ttl_minutes: DEFAULT_CONTEXT_TTL_MINUTES,
      retention_mode: ContextRetentionMode::TailTurns,
      max_turns: DEFAULT_MAX_TURNS,
      max_tokens: DEFAULT_MAX_CONTEXT_TOKENS,
      keep_system_prompt: DEFAULT_KEEP_SYSTEM_PROMPT,
      storage_dir: default_storage_dir(),
      auto_compact_enabled: DEFAULT_AUTO_COMPACT_ENABLED,
      compact_threshold_tokens: DEFAULT_MAX_CONTEXT_TOKENS,
      preserve_recent_messages: DEFAULT_PRESERVE_RECENT_MESSAGES,
      ai_compact_enabled: DEFAULT_AI_COMPACT_ENABLED,
      compact_max_output_tokens: DEFAULT_COMPACT_MAX_OUTPUT_TOKENS,
      compact_system_prompt: DEFAULT_COMPACT_SYSTEM_PROMPT.to_string(),
      input_token_price_per_1m: 0.0,
      output_token_price_per_1m: 0.0,
      rotate_after_bytes: DEFAULT_ROTATE_AFTER_BYTES,
      max_rotated_files: DEFAULT_MAX_ROTATED_FILES,
      expire_action: ContextExpireAction::Archive,
    }
  }
}

impl ContextModuleConfig {
  pub fn view(&self) -> ContextModuleConfigView {
    ContextModuleConfigView {
      enabled: self.enabled,
      state_enabled: self.state_enabled,
      context_ttl_minutes: self.context_ttl_minutes,
      retention_mode: self.retention_mode.clone(),
      max_turns: self.max_turns,
      max_tokens: self.max_tokens,
      keep_system_prompt: self.keep_system_prompt,
      storage_dir: self.storage_dir.clone(),
      auto_compact_enabled: self.auto_compact_enabled,
      compact_threshold_tokens: self.compact_threshold_tokens,
      preserve_recent_messages: self.preserve_recent_messages,
      ai_compact_enabled: self.ai_compact_enabled,
      compact_max_output_tokens: self.compact_max_output_tokens,
      compact_system_prompt: self.compact_system_prompt.clone(),
      input_token_price_per_1m: self.input_token_price_per_1m,
      output_token_price_per_1m: self.output_token_price_per_1m,
      rotate_after_bytes: self.rotate_after_bytes,
      max_rotated_files: self.max_rotated_files,
      expire_action: self.expire_action.clone(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextModuleConfigView {
  pub enabled: bool,
  pub state_enabled: bool,
  pub context_ttl_minutes: usize,
  pub retention_mode: ContextRetentionMode,
  pub max_turns: usize,
  pub max_tokens: usize,
  pub keep_system_prompt: bool,
  pub storage_dir: String,
  pub auto_compact_enabled: bool,
  pub compact_threshold_tokens: usize,
  pub preserve_recent_messages: usize,
  pub ai_compact_enabled: bool,
  pub compact_max_output_tokens: u32,
  pub compact_system_prompt: String,
  pub input_token_price_per_1m: f64,
  pub output_token_price_per_1m: f64,
  pub rotate_after_bytes: u64,
  pub max_rotated_files: usize,
  pub expire_action: ContextExpireAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextRole {
  System,
  User,
  Assistant,
  Tool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContextBlock {
  Text {
    text: String,
  },
  ToolUse {
    id: String,
    name: String,
    input: Value,
  },
  ToolResult {
    tool_use_id: String,
    tool_name: String,
    output: String,
    is_error: bool,
  },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextTokenUsage {
  pub input_tokens: u32,
  pub output_tokens: u32,
  pub total_tokens: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextUsageSummary {
  pub input_tokens: u64,
  pub output_tokens: u64,
  pub total_tokens: u64,
}

impl ContextUsageSummary {
  fn record(&mut self, usage: Option<ContextTokenUsage>) {
    if let Some(usage) = usage {
      self.input_tokens = self.input_tokens.saturating_add(u64::from(usage.input_tokens));
      self.output_tokens = self
        .output_tokens
        .saturating_add(u64::from(usage.output_tokens));
      self.total_tokens = self.total_tokens.saturating_add(u64::from(usage.total_tokens));
    }
  }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ContextCostEstimate {
  pub input_cost: f64,
  pub output_cost: f64,
  pub total_cost: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextMessage {
  pub role: ContextRole,
  pub blocks: Vec<ContextBlock>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub usage: Option<ContextTokenUsage>,
}

impl ContextMessage {
  pub fn text(role: ContextRole, text: impl Into<String>) -> Self {
    Self {
      role,
      blocks: vec![ContextBlock::Text { text: text.into() }],
      usage: None,
    }
  }

  pub fn user_text(text: impl Into<String>) -> Self {
    Self::text(ContextRole::User, text)
  }

  pub fn assistant_text(text: impl Into<String>) -> Self {
    Self::text(ContextRole::Assistant, text)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextSummarySource {
  Ai,
  Deterministic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompaction {
  pub count: u32,
  pub removed_message_count: usize,
  pub summary: String,
  pub summary_source: ContextSummarySource,
  pub compacted_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextPromptEntry {
  pub timestamp_ms: u64,
  pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextLineage {
  pub parent_yuanling_id: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub branch_name: Option<String>,
  pub cloned_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YuanlingContext {
  pub version: u32,
  pub yuanling_id: String,
  pub created_at_ms: u64,
  pub updated_at_ms: u64,
  pub messages: Vec<ContextMessage>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub compaction: Option<ContextCompaction>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub lineage: Option<ContextLineage>,
  pub prompt_history: Vec<ContextPromptEntry>,
  pub usage_summary: ContextUsageSummary,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
}

impl YuanlingContext {
  pub fn new(yuanling_id: impl Into<String>) -> Self {
    let now = current_time_millis();
    Self {
      version: CONTEXT_STATE_VERSION,
      yuanling_id: yuanling_id.into(),
      created_at_ms: now,
      updated_at_ms: now,
      messages: Vec::new(),
      compaction: None,
      lineage: None,
      prompt_history: Vec::new(),
      usage_summary: ContextUsageSummary::default(),
      model: None,
    }
  }

  fn touch(&mut self) {
    self.updated_at_ms = current_time_millis();
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextHealthReport {
  pub healthy: bool,
  pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextCompactionEvent {
  pub removed_message_count: usize,
  pub estimated_tokens_before: usize,
  pub estimated_tokens_after: usize,
  pub summary_source: ContextSummarySource,
  pub health_report: ContextHealthReport,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextBuildResult {
  pub yuanling_id: String,
  pub messages: Vec<ContextMessage>,
  pub estimated_tokens: usize,
  pub usage_summary: ContextUsageSummary,
  pub cost_estimate: ContextCostEstimate,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub compaction: Option<ContextCompactionEvent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextCompactionResult {
  pub compacted: bool,
  pub removed_message_count: usize,
  pub estimated_tokens_before: usize,
  pub estimated_tokens_after: usize,
  pub summary_source: ContextSummarySource,
  pub health_report: ContextHealthReport,
  pub context: YuanlingContext,
}

#[derive(Debug)]
pub enum ContextError {
  Io(std::io::Error),
  Json(serde_json::Error),
  InvalidYuanlingId(String),
  InvalidRecord(String),
  HealthCheckFailed(ContextHealthReport),
}

impl Display for ContextError {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Io(error) => write!(formatter, "{error}"),
      Self::Json(error) => write!(formatter, "{error}"),
      Self::InvalidYuanlingId(yuanling_id) => {
        write!(formatter, "invalid yuanling context id: {yuanling_id}")
      }
      Self::InvalidRecord(message) => write!(formatter, "{message}"),
      Self::HealthCheckFailed(report) => {
        write!(formatter, "context health check failed: {:?}", report.errors)
      }
    }
  }
}

impl std::error::Error for ContextError {}

impl From<std::io::Error> for ContextError {
  fn from(value: std::io::Error) -> Self {
    Self::Io(value)
  }
}

impl From<serde_json::Error> for ContextError {
  fn from(value: serde_json::Error) -> Self {
    Self::Json(value)
  }
}

pub fn resolve_from_env() -> ContextModuleConfig {
  let max_tokens = env_or_usize("YUANLING_CONTEXT_MAX_TOKENS", DEFAULT_MAX_CONTEXT_TOKENS);

  ContextModuleConfig {
    enabled: env_or_bool("YUANLING_CONTEXT_ENABLED", true),
    state_enabled: env_or_bool("YUANLING_CONTEXT_STATE_ENABLED", DEFAULT_STATE_ENABLED),
    context_ttl_minutes: env_or_usize(
      "YUANLING_CONTEXT_TTL_MINUTES",
      DEFAULT_CONTEXT_TTL_MINUTES,
    ),
    retention_mode: resolve_retention_mode(),
    max_turns: env_or_usize("YUANLING_CONTEXT_MAX_TURNS", DEFAULT_MAX_TURNS),
    max_tokens,
    keep_system_prompt: env_or_bool(
      "YUANLING_CONTEXT_KEEP_SYSTEM_PROMPT",
      DEFAULT_KEEP_SYSTEM_PROMPT,
    ),
    storage_dir: env_or("YUANLING_CONTEXT_STORAGE_DIR", &default_storage_dir()),
    auto_compact_enabled: env_or_bool(
      "YUANLING_CONTEXT_AUTO_COMPACT_ENABLED",
      DEFAULT_AUTO_COMPACT_ENABLED,
    ),
    compact_threshold_tokens: env_or_usize(
      "YUANLING_CONTEXT_COMPACT_THRESHOLD_TOKENS",
      max_tokens,
    ),
    preserve_recent_messages: env_or_usize(
      "YUANLING_CONTEXT_PRESERVE_RECENT_MESSAGES",
      DEFAULT_PRESERVE_RECENT_MESSAGES,
    ),
    ai_compact_enabled: env_or_bool(
      "YUANLING_CONTEXT_AI_COMPACT_ENABLED",
      DEFAULT_AI_COMPACT_ENABLED,
    ),
    compact_max_output_tokens: env_or_u32(
      "YUANLING_CONTEXT_COMPACT_MAX_OUTPUT_TOKENS",
      DEFAULT_COMPACT_MAX_OUTPUT_TOKENS,
    ),
    compact_system_prompt: env_or(
      "YUANLING_CONTEXT_COMPACT_SYSTEM_PROMPT",
      DEFAULT_COMPACT_SYSTEM_PROMPT,
    ),
    input_token_price_per_1m: env_or_f64("YUANLING_CONTEXT_INPUT_TOKEN_PRICE_PER_1M", 0.0),
    output_token_price_per_1m: env_or_f64("YUANLING_CONTEXT_OUTPUT_TOKEN_PRICE_PER_1M", 0.0),
    rotate_after_bytes: env_or_u64("YUANLING_CONTEXT_ROTATE_AFTER_BYTES", DEFAULT_ROTATE_AFTER_BYTES),
    max_rotated_files: env_or_usize(
      "YUANLING_CONTEXT_MAX_ROTATED_FILES",
      DEFAULT_MAX_ROTATED_FILES,
    ),
    expire_action: resolve_expire_action(),
  }
}

pub fn load_context(
  yuanling_id: &str,
  config: &ContextModuleConfig,
) -> Result<YuanlingContext, ContextError> {
  validate_yuanling_id(yuanling_id)?;
  let path = context_path(yuanling_id, config)?;
  if !path.exists() {
    return Ok(YuanlingContext::new(yuanling_id));
  }

  let contents = fs::read_to_string(&path)?;
  let context = parse_context_jsonl(yuanling_id, &contents)?;
  if is_context_expired(&context, config) {
    if matches!(config.expire_action, ContextExpireAction::Archive) {
      archive_expired_context_file(&path)?;
    }
    return Ok(YuanlingContext::new(yuanling_id));
  }

  Ok(context)
}

pub fn save_context(
  context: &YuanlingContext,
  config: &ContextModuleConfig,
) -> Result<(), ContextError> {
  validate_yuanling_id(&context.yuanling_id)?;
  fs::create_dir_all(&config.storage_dir)?;
  let path = context_path(&context.yuanling_id, config)?;
  rotate_context_file_if_needed(&path, config)?;
  fs::write(path, render_context_jsonl(context)?)?;
  Ok(())
}

pub fn append_message(
  yuanling_id: &str,
  message: ContextMessage,
  config: &ContextModuleConfig,
) -> Result<YuanlingContext, ContextError> {
  let mut context = load_context(yuanling_id, config)?;
  let path = context_path(yuanling_id, config)?;
  let should_snapshot = !path.exists() || path.metadata().map(|meta| meta.len() == 0).unwrap_or(true);

  context.usage_summary.record(message.usage);
  context.messages.push(message.clone());
  context.touch();

  if should_snapshot || rotate_context_file_if_needed(&path, config)? {
    save_context(&context, config)?;
  } else {
    append_jsonl_record(
      &path,
      json!({
        "type": "message",
        "updated_at_ms": context.updated_at_ms,
        "usage_delta": message.usage,
        "message": message,
      }),
    )?;
  }

  Ok(context)
}

pub fn append_prompt_entry(
  yuanling_id: &str,
  text: impl Into<String>,
  config: &ContextModuleConfig,
) -> Result<YuanlingContext, ContextError> {
  let mut context = load_context(yuanling_id, config)?;
  let path = context_path(yuanling_id, config)?;
  let should_snapshot = !path.exists() || path.metadata().map(|meta| meta.len() == 0).unwrap_or(true);
  let entry = ContextPromptEntry {
    timestamp_ms: current_time_millis(),
    text: text.into(),
  };

  context.prompt_history.push(entry.clone());
  context.touch();

  if should_snapshot || rotate_context_file_if_needed(&path, config)? {
    save_context(&context, config)?;
  } else {
    append_jsonl_record(
      &path,
      json!({
        "type": "prompt_history",
        "updated_at_ms": context.updated_at_ms,
        "entry": entry,
      }),
    )?;
  }

  Ok(context)
}

pub fn clone_context(
  parent_yuanling_id: &str,
  new_yuanling_id: &str,
  branch_name: Option<String>,
  config: &ContextModuleConfig,
) -> Result<YuanlingContext, ContextError> {
  validate_yuanling_id(parent_yuanling_id)?;
  validate_yuanling_id(new_yuanling_id)?;
  let parent = load_context(parent_yuanling_id, config)?;
  let now = current_time_millis();
  let mut cloned = parent.clone();
  cloned.yuanling_id = new_yuanling_id.to_string();
  cloned.created_at_ms = now;
  cloned.updated_at_ms = now;
  cloned.lineage = Some(ContextLineage {
    parent_yuanling_id: parent_yuanling_id.to_string(),
    branch_name: branch_name.filter(|value| !value.trim().is_empty()),
    cloned_at_ms: now,
  });
  save_context(&cloned, config)?;
  Ok(cloned)
}

pub async fn build_context(
  yuanling_id: &str,
  config: &ContextModuleConfig,
) -> Result<ContextBuildResult, ContextError> {
  let context = load_context(yuanling_id, config)?;
  let estimated_tokens_before = estimate_context_tokens(&context);

  if !config.enabled || !config.state_enabled || !should_compact(&context, config) {
    return Ok(build_result(context, config, estimated_tokens_before, None));
  }

  let compaction_result = if config.ai_compact_enabled {
    let ai_config = ai::resolve_from_env();
    compact_context_with_ai(&context, config, &ai_config).await?
  } else {
    compact_context(&context, config)?
  };

  if compaction_result.compacted {
    save_context(&compaction_result.context, config)?;
  }

  let estimated_tokens_after = compaction_result.estimated_tokens_after;
  let event = ContextCompactionEvent {
    removed_message_count: compaction_result.removed_message_count,
    estimated_tokens_before: compaction_result.estimated_tokens_before,
    estimated_tokens_after,
    summary_source: compaction_result.summary_source,
    health_report: compaction_result.health_report.clone(),
  };
  Ok(build_result(
    compaction_result.context,
    config,
    estimated_tokens_after,
    Some(event),
  ))
}

pub fn compact_context(
  context: &YuanlingContext,
  config: &ContextModuleConfig,
) -> Result<ContextCompactionResult, ContextError> {
  compact_context_with_summary(context, config, None)
}

pub async fn compact_context_with_ai(
  context: &YuanlingContext,
  config: &ContextModuleConfig,
  ai_config: &ai::AiModuleConfig,
) -> Result<ContextCompactionResult, ContextError> {
  if !should_compact(context, config) {
    return compact_context(context, config);
  }

  match request_ai_compact_summary(context, config, ai_config).await {
    Some(summary) => match compact_context_with_summary(
      context,
      config,
      Some((summary, ContextSummarySource::Ai)),
    ) {
      Ok(result) => Ok(result),
      Err(_) => compact_context(context, config),
    },
    None => compact_context(context, config),
  }
}

pub fn compact_context_with_summary(
  context: &YuanlingContext,
  config: &ContextModuleConfig,
  summary_override: Option<(String, ContextSummarySource)>,
) -> Result<ContextCompactionResult, ContextError> {
  let estimated_tokens_before = estimate_context_tokens(context);
  let Some(window) = compaction_window(context, config) else {
    return Ok(ContextCompactionResult {
      compacted: false,
      removed_message_count: 0,
      estimated_tokens_before,
      estimated_tokens_after: estimated_tokens_before,
      summary_source: ContextSummarySource::Deterministic,
      health_report: ContextHealthReport {
        healthy: true,
        errors: vec![],
      },
      context: context.clone(),
    });
  };

  let removed_messages = &context.messages[window.remove_start..window.keep_from];
  let preserved_messages = context.messages[window.keep_from..].to_vec();
  let existing_summary = context
    .messages
    .first()
    .and_then(extract_existing_compacted_summary);
  let (summary, summary_source) = match summary_override {
    Some((summary, source)) => (
      merge_summary_text(existing_summary.as_deref(), &summary),
      source,
    ),
    None => (
      merge_compact_summaries(existing_summary.as_deref(), removed_messages),
      ContextSummarySource::Deterministic,
    ),
  };
  let continuation = format_compact_continuation(&summary, !preserved_messages.is_empty());

  let mut compacted_messages = vec![ContextMessage {
    role: ContextRole::System,
    blocks: vec![ContextBlock::Text { text: continuation }],
    usage: None,
  }];
  compacted_messages.extend(preserved_messages);

  let mut compacted_context = context.clone();
  compacted_context.messages = compacted_messages;
  compacted_context.touch();
  compacted_context.compaction = Some(ContextCompaction {
    count: context.compaction.as_ref().map_or(1, |value| value.count + 1),
    removed_message_count: removed_messages.len(),
    summary,
    summary_source,
    compacted_at_ms: current_time_millis(),
  });

  let estimated_tokens_after = estimate_context_tokens(&compacted_context);
  let health_report = run_compaction_health_check(
    context,
    &compacted_context,
    removed_messages.len(),
    estimated_tokens_before,
    estimated_tokens_after,
  )?;

  Ok(ContextCompactionResult {
    compacted: true,
    removed_message_count: removed_messages.len(),
    estimated_tokens_before,
    estimated_tokens_after,
    summary_source,
    health_report,
    context: compacted_context,
  })
}

pub fn should_compact(context: &YuanlingContext, config: &ContextModuleConfig) -> bool {
  if !config.auto_compact_enabled {
    return false;
  }
  compaction_window(context, config).is_some()
}

pub fn estimate_context_tokens(context: &YuanlingContext) -> usize {
  context.messages.iter().map(estimate_message_tokens).sum()
}

pub fn is_context_expired(context: &YuanlingContext, config: &ContextModuleConfig) -> bool {
  if config.context_ttl_minutes == 0 {
    return false;
  }
  let ttl_ms = u64::try_from(config.context_ttl_minutes)
    .unwrap_or(u64::MAX)
    .saturating_mul(60_000);
  current_time_millis().saturating_sub(context.updated_at_ms) > ttl_ms
}

fn build_result(
  context: YuanlingContext,
  config: &ContextModuleConfig,
  estimated_tokens: usize,
  compaction: Option<ContextCompactionEvent>,
) -> ContextBuildResult {
  let mut messages = context.messages;
  if let Some(injection) = build_skills_context_injection() {
    messages.insert(0, ContextMessage::text(ContextRole::System, injection));
  }
  let estimated_tokens = if messages.is_empty() {
    estimated_tokens
  } else {
    messages.iter().map(estimate_message_tokens).sum()
  };

  ContextBuildResult {
    yuanling_id: context.yuanling_id,
    messages,
    estimated_tokens,
    usage_summary: context.usage_summary,
    cost_estimate: estimate_cost(context.usage_summary, config),
    compaction,
  }
}

fn build_skills_context_injection() -> Option<String> {
  let config = skills::resolve_from_env();
  skills::build_context_injection(&config).ok().flatten()
}

fn estimate_cost(
  usage_summary: ContextUsageSummary,
  config: &ContextModuleConfig,
) -> ContextCostEstimate {
  let input_cost =
    usage_summary.input_tokens as f64 * config.input_token_price_per_1m / 1_000_000.0;
  let output_cost =
    usage_summary.output_tokens as f64 * config.output_token_price_per_1m / 1_000_000.0;
  ContextCostEstimate {
    input_cost,
    output_cost,
    total_cost: input_cost + output_cost,
  }
}

fn render_context_jsonl(context: &YuanlingContext) -> Result<String, ContextError> {
  let mut records = vec![json!({
    "type": "context_meta",
    "version": context.version,
    "yuanling_id": context.yuanling_id,
    "created_at_ms": context.created_at_ms,
    "updated_at_ms": context.updated_at_ms,
    "lineage": context.lineage,
    "usage_summary": context.usage_summary,
    "model": context.model,
  })];

  if let Some(compaction) = &context.compaction {
    records.push(json!({
      "type": "compaction",
      "compaction": compaction,
    }));
  }

  records.extend(context.prompt_history.iter().map(|entry| {
    json!({
      "type": "prompt_history",
      "entry": entry,
    })
  }));

  records.extend(context.messages.iter().map(|message| {
    json!({
      "type": "message",
      "message": message,
    })
  }));

  let mut rendered = records
    .into_iter()
    .map(|record| serde_json::to_string(&record))
    .collect::<Result<Vec<_>, _>>()?
    .join("\n");
  rendered.push('\n');
  Ok(rendered)
}

fn parse_context_jsonl(yuanling_id: &str, contents: &str) -> Result<YuanlingContext, ContextError> {
  let mut context = YuanlingContext::new(yuanling_id);
  let mut saw_meta = false;
  let mut saw_usage_summary = false;
  context.messages.clear();

  for (index, raw_line) in contents.lines().enumerate() {
    let line = raw_line.trim();
    if line.is_empty() {
      continue;
    }

    let value: Value = serde_json::from_str(line)?;
    let record_type = value
      .get("type")
      .and_then(Value::as_str)
      .ok_or_else(|| ContextError::InvalidRecord(format!("line {} missing type", index + 1)))?;

    match record_type {
      "context_meta" => {
        let parsed_yuanling_id = value
          .get("yuanling_id")
          .and_then(Value::as_str)
          .unwrap_or(yuanling_id)
          .to_string();
        if parsed_yuanling_id != yuanling_id {
          return Err(ContextError::InvalidRecord(format!(
            "line {} context id mismatch",
            index + 1
          )));
        }
        context.version = value
          .get("version")
          .and_then(Value::as_u64)
          .and_then(|value| u32::try_from(value).ok())
          .unwrap_or(CONTEXT_STATE_VERSION);
        context.yuanling_id = parsed_yuanling_id;
        context.created_at_ms = value
          .get("created_at_ms")
          .and_then(Value::as_u64)
          .unwrap_or(context.created_at_ms);
        context.updated_at_ms = value
          .get("updated_at_ms")
          .and_then(Value::as_u64)
          .unwrap_or(context.updated_at_ms);
        context.lineage = value
          .get("lineage")
          .filter(|value| !value.is_null())
          .map(|value| serde_json::from_value(value.clone()))
          .transpose()?;
        context.usage_summary = value
          .get("usage_summary")
          .map(|value| serde_json::from_value(value.clone()))
          .transpose()?
          .unwrap_or_default();
        saw_usage_summary = value.get("usage_summary").is_some();
        context.model = value
          .get("model")
          .and_then(Value::as_str)
          .map(ToOwned::to_owned);
        saw_meta = true;
      }
      "message" => {
        let message = value
          .get("message")
          .ok_or_else(|| {
            ContextError::InvalidRecord(format!("line {} missing message", index + 1))
          })
          .and_then(|value| serde_json::from_value(value.clone()).map_err(ContextError::from))?;
        if let Some(updated_at_ms) = value.get("updated_at_ms").and_then(Value::as_u64) {
          context.updated_at_ms = updated_at_ms;
        }
        if let Some(usage_delta) = value.get("usage_delta").filter(|value| !value.is_null()) {
          let usage_delta = serde_json::from_value(usage_delta.clone())?;
          context.usage_summary.record(Some(usage_delta));
          saw_usage_summary = true;
        }
        context.messages.push(message);
      }
      "compaction" => {
        let compaction = value
          .get("compaction")
          .ok_or_else(|| {
            ContextError::InvalidRecord(format!("line {} missing compaction", index + 1))
          })
          .and_then(|value| serde_json::from_value(value.clone()).map_err(ContextError::from))?;
        context.compaction = Some(compaction);
      }
      "prompt_history" => {
        let entry = value
          .get("entry")
          .or_else(|| value.get("prompt_history"))
          .ok_or_else(|| {
            ContextError::InvalidRecord(format!("line {} missing prompt history", index + 1))
          })
          .and_then(|value| serde_json::from_value(value.clone()).map_err(ContextError::from))?;
        if let Some(updated_at_ms) = value.get("updated_at_ms").and_then(Value::as_u64) {
          context.updated_at_ms = updated_at_ms;
        }
        context.prompt_history.push(entry);
      }
      other => {
        return Err(ContextError::InvalidRecord(format!(
          "line {} unsupported record type: {other}",
          index + 1
        )));
      }
    }
  }

  if !saw_meta {
    context.touch();
  }
  if !saw_usage_summary {
    context.usage_summary = summarize_usage(&context.messages);
  }

  Ok(context)
}

fn summarize_usage(messages: &[ContextMessage]) -> ContextUsageSummary {
  let mut summary = ContextUsageSummary::default();
  for message in messages {
    summary.record(message.usage);
  }
  summary
}

fn append_jsonl_record(path: &Path, record: Value) -> Result<(), ContextError> {
  let mut file = OpenOptions::new().append(true).create(true).open(path)?;
  writeln!(file, "{}", serde_json::to_string(&record)?)?;
  Ok(())
}

fn rotate_context_file_if_needed(
  path: &Path,
  config: &ContextModuleConfig,
) -> Result<bool, ContextError> {
  if config.rotate_after_bytes == 0 || config.max_rotated_files == 0 || !path.exists() {
    return Ok(false);
  }
  if path.metadata()?.len() < config.rotate_after_bytes {
    return Ok(false);
  }

  for index in (1..=config.max_rotated_files).rev() {
    let current = rotated_context_path(path, index);
    if index == config.max_rotated_files {
      if current.exists() {
        fs::remove_file(&current)?;
      }
      continue;
    }

    if current.exists() {
      fs::rename(&current, rotated_context_path(path, index + 1))?;
    }
  }

  fs::rename(path, rotated_context_path(path, 1))?;
  Ok(true)
}

fn rotated_context_path(path: &Path, index: usize) -> PathBuf {
  let parent = path.parent().unwrap_or_else(|| Path::new(""));
  let stem = path.file_stem().and_then(|value| value.to_str()).unwrap_or("context");
  let extension = path.extension().and_then(|value| value.to_str()).unwrap_or("jsonl");
  parent.join(format!("{stem}.{index}.{extension}"))
}

fn archive_expired_context_file(path: &Path) -> Result<(), ContextError> {
  if !path.exists() {
    return Ok(());
  }
  let parent = path.parent().unwrap_or_else(|| Path::new(""));
  let stem = path.file_stem().and_then(|value| value.to_str()).unwrap_or("context");
  let extension = path.extension().and_then(|value| value.to_str()).unwrap_or("jsonl");
  let archive_path = parent.join(format!(
    "{stem}.expired.{}.{}",
    current_time_millis(),
    extension
  ));
  fs::rename(path, archive_path)?;
  Ok(())
}

#[derive(Debug, Clone, Copy)]
struct CompactionWindow {
  remove_start: usize,
  keep_from: usize,
}

fn compaction_window(
  context: &YuanlingContext,
  config: &ContextModuleConfig,
) -> Option<CompactionWindow> {
  if !config.auto_compact_enabled {
    return None;
  }

  let summary_prefix_len = compacted_summary_prefix_len(context);
  let mut keep_from: Option<usize> = None;

  if matches!(config.retention_mode, ContextRetentionMode::TailTurns) {
    keep_from = turn_retention_boundary(context, config, summary_prefix_len);
  }

  if estimate_context_tokens(context) >= config.compact_threshold_tokens {
    let token_keep_from = context
      .messages
      .len()
      .saturating_sub(config.preserve_recent_messages)
      .max(summary_prefix_len);
    keep_from = Some(keep_from.map_or(token_keep_from, |current| current.max(token_keep_from)));
  }

  let keep_from = keep_from?;
  let keep_from = safe_compaction_boundary(context, keep_from, summary_prefix_len);
  if keep_from <= summary_prefix_len || keep_from >= context.messages.len() {
    return None;
  }

  Some(CompactionWindow {
    remove_start: summary_prefix_len,
    keep_from,
  })
}

fn turn_retention_boundary(
  context: &YuanlingContext,
  config: &ContextModuleConfig,
  summary_prefix_len: usize,
) -> Option<usize> {
  if config.max_turns == 0 {
    return None;
  }

  let mut user_turns = 0usize;
  for index in (summary_prefix_len..context.messages.len()).rev() {
    if context.messages[index].role == ContextRole::User {
      user_turns += 1;
      if user_turns == config.max_turns {
        return Some(index);
      }
    }
  }

  None
}

fn run_compaction_health_check(
  original: &YuanlingContext,
  compacted: &YuanlingContext,
  removed_message_count: usize,
  estimated_tokens_before: usize,
  estimated_tokens_after: usize,
) -> Result<ContextHealthReport, ContextError> {
  let mut errors = Vec::new();

  if removed_message_count == 0 {
    errors.push("compaction removed no messages".to_string());
  }

  let first_message = compacted.messages.first();
  if !first_message.is_some_and(|message| message.role == ContextRole::System) {
    errors.push("first compacted message must be a synthetic system summary".to_string());
  }

  if first_message
    .and_then(first_text_block)
    .is_none_or(|text| !text.starts_with(COMPACT_CONTINUATION_PREAMBLE))
  {
    errors.push("synthetic system summary is missing continuation preamble".to_string());
  }

  if original.messages.len() > removed_message_count && compacted.messages.len() <= 1 {
    errors.push("recent messages were not preserved".to_string());
  }

  if estimated_tokens_after >= estimated_tokens_before {
    errors.push("compaction did not reduce estimated tokens".to_string());
  }

  if let Some(error) = first_tool_pair_error(&compacted.messages) {
    errors.push(error);
  }

  let report = ContextHealthReport {
    healthy: errors.is_empty(),
    errors,
  };
  if report.healthy {
    Ok(report)
  } else {
    Err(ContextError::HealthCheckFailed(report))
  }
}

fn first_tool_pair_error(messages: &[ContextMessage]) -> Option<String> {
  for (index, message) in messages.iter().enumerate() {
    if !starts_with_tool_result(message) {
      continue;
    }
    let previous_has_tool_use = index
      .checked_sub(1)
      .and_then(|previous| messages.get(previous))
      .is_some_and(has_tool_use);
    if !previous_has_tool_use {
      return Some(format!("tool result at message {index} has no preceding tool use"));
    }
  }
  None
}

async fn request_ai_compact_summary(
  context: &YuanlingContext,
  config: &ContextModuleConfig,
  ai_config: &ai::AiModuleConfig,
) -> Option<String> {
  let window = compaction_window(context, config)?;
  let removed_messages = &context.messages[window.remove_start..window.keep_from];
  let transcript = format_compact_transcript(removed_messages);
  let request = ChatComposeRequest {
    model: None,
    max_tokens: Some(config.compact_max_output_tokens),
    messages: vec![InputMessage {
      role: "user".to_string(),
      content: vec![InputContentBlock::Text {
        text: format!("Summarize this earlier context transcript:\n\n{transcript}"),
      }],
    }],
    user_input: None,
    system: Some(config.compact_system_prompt.clone()),
    stream: false,
    tools: None,
    tool_choice: None,
    temperature: Some(0.2),
    top_p: None,
    frequency_penalty: None,
    presence_penalty: None,
    stop: None,
    reasoning_effort: None,
  };
  let response = ai::send_chat_request(request, ai_config).await;
  if !response.success {
    return None;
  }
  response.body.as_deref().and_then(extract_ai_summary)
}

fn extract_ai_summary(body: &str) -> Option<String> {
  let value: Value = serde_json::from_str(body).ok()?;
  if let Some(text) = value
    .get("choices")
    .and_then(Value::as_array)
    .and_then(|choices| choices.first())
    .and_then(|choice| choice.get("message"))
    .and_then(|message| message.get("content"))
    .and_then(Value::as_str)
  {
    let text = text.trim();
    return (!text.is_empty()).then(|| text.to_string());
  }

  if let Some(items) = value.get("content").and_then(Value::as_array) {
    let text = items
      .iter()
      .filter_map(|item| item.get("text").and_then(Value::as_str))
      .collect::<Vec<_>>()
      .join("");
    let text = text.trim();
    if !text.is_empty() {
      return Some(text.to_string());
    }
  }

  let text = value.get("output_text").and_then(Value::as_str)?.trim();
  (!text.is_empty()).then(|| text.to_string())
}

fn format_compact_transcript(messages: &[ContextMessage]) -> String {
  messages
    .iter()
    .map(|message| {
      let role = match message.role {
        ContextRole::System => "system",
        ContextRole::User => "user",
        ContextRole::Assistant => "assistant",
        ContextRole::Tool => "tool",
      };
      let content = message
        .blocks
        .iter()
        .map(summarize_block)
        .collect::<Vec<_>>()
        .join(" | ");
      format!("{role}: {content}")
    })
    .collect::<Vec<_>>()
    .join("\n")
}

fn estimate_message_tokens(message: &ContextMessage) -> usize {
  message
    .blocks
    .iter()
    .map(|block| match block {
      ContextBlock::Text { text } => estimate_text_tokens(text),
      ContextBlock::ToolUse { name, input, .. } => {
        estimate_text_tokens(name) + estimate_text_tokens(&input.to_string())
      }
      ContextBlock::ToolResult {
        tool_name, output, ..
      } => estimate_text_tokens(tool_name) + estimate_text_tokens(output),
    })
    .sum()
}

fn estimate_text_tokens(value: &str) -> usize {
  value.chars().count() / 4 + 1
}

fn summarize_messages(messages: &[ContextMessage]) -> String {
  let user_count = messages
    .iter()
    .filter(|message| message.role == ContextRole::User)
    .count();
  let assistant_count = messages
    .iter()
    .filter(|message| message.role == ContextRole::Assistant)
    .count();
  let tool_count = messages
    .iter()
    .filter(|message| message.role == ContextRole::Tool)
    .count();

  let mut lines = vec![
    "<summary>".to_string(),
    "Conversation summary:".to_string(),
    format!(
      "- Scope: {} earlier messages compacted (user={}, assistant={}, tool={}).",
      messages.len(),
      user_count,
      assistant_count,
      tool_count
    ),
  ];

  let recent_user_messages = collect_recent_text(messages, ContextRole::User, 3);
  if !recent_user_messages.is_empty() {
    lines.push("- Recent user requests:".to_string());
    lines.extend(
      recent_user_messages
        .into_iter()
        .map(|message| format!("  - {message}")),
    );
  }

  let key_timeline = messages
    .iter()
    .map(|message| {
      let role = match message.role {
        ContextRole::System => "system",
        ContextRole::User => "user",
        ContextRole::Assistant => "assistant",
        ContextRole::Tool => "tool",
      };
      let content = message
        .blocks
        .iter()
        .map(summarize_block)
        .collect::<Vec<_>>()
        .join(" | ");
      format!("  - {role}: {content}")
    })
    .collect::<Vec<_>>();

  if !key_timeline.is_empty() {
    lines.push("- Key timeline:".to_string());
    lines.extend(key_timeline);
  }

  lines.push("</summary>".to_string());
  lines.join("\n")
}

fn summarize_block(block: &ContextBlock) -> String {
  let raw = match block {
    ContextBlock::Text { text } => text.clone(),
    ContextBlock::ToolUse { name, input, .. } => format!("tool_use {name}({input})"),
    ContextBlock::ToolResult {
      tool_name,
      output,
      is_error,
      ..
    } => format!(
      "tool_result {tool_name}: {}{output}",
      if *is_error { "error " } else { "" }
    ),
  };
  truncate_summary(&raw, 160)
}

fn collect_recent_text(
  messages: &[ContextMessage],
  role: ContextRole,
  limit: usize,
) -> Vec<String> {
  messages
    .iter()
    .rev()
    .filter(|message| message.role == role)
    .filter_map(first_text_block)
    .take(limit)
    .map(|text| truncate_summary(text, 160))
    .collect::<Vec<_>>()
    .into_iter()
    .rev()
    .collect()
}

fn merge_compact_summaries(
  existing_summary: Option<&str>,
  removed_messages: &[ContextMessage],
) -> String {
  merge_summary_text(existing_summary, &summarize_messages(removed_messages))
}

fn merge_summary_text(existing_summary: Option<&str>, new_summary: &str) -> String {
  let Some(existing_summary) = existing_summary else {
    return new_summary.to_string();
  };

  [
    "<summary>",
    "Conversation summary:",
    "- Previously compacted context:",
    existing_summary.trim(),
    "- Newly compacted context:",
    new_summary.trim(),
    "</summary>",
  ]
  .join("\n")
}

fn format_compact_continuation(summary: &str, recent_messages_preserved: bool) -> String {
  let mut continuation = format!(
    "{COMPACT_CONTINUATION_PREAMBLE}{}",
    format_compact_summary(summary)
  );
  if recent_messages_preserved {
    continuation.push_str("\n\nRecent messages are preserved verbatim.");
  }
  continuation
}

fn format_compact_summary(summary: &str) -> String {
  let summary = strip_tag_block(summary, "analysis");
  if let Some(content) = extract_tag_block(&summary, "summary") {
    collapse_blank_lines(&format!("Summary:\n{}", content.trim()))
  } else {
    collapse_blank_lines(&summary)
  }
  .trim()
  .to_string()
}

fn safe_compaction_boundary(
  context: &YuanlingContext,
  raw_keep_from: usize,
  summary_prefix_len: usize,
) -> usize {
  let mut keep_from = raw_keep_from;
  while keep_from > summary_prefix_len && starts_with_tool_result(&context.messages[keep_from]) {
    keep_from = keep_from.saturating_sub(1);
    if has_tool_use(&context.messages[keep_from]) {
      break;
    }
  }
  keep_from
}

fn starts_with_tool_result(message: &ContextMessage) -> bool {
  message
    .blocks
    .first()
    .is_some_and(|block| matches!(block, ContextBlock::ToolResult { .. }))
}

fn has_tool_use(message: &ContextMessage) -> bool {
  message
    .blocks
    .iter()
    .any(|block| matches!(block, ContextBlock::ToolUse { .. }))
}

fn compacted_summary_prefix_len(context: &YuanlingContext) -> usize {
  usize::from(
    context
      .messages
      .first()
      .and_then(extract_existing_compacted_summary)
      .is_some(),
  )
}

fn extract_existing_compacted_summary(message: &ContextMessage) -> Option<String> {
  if message.role != ContextRole::System {
    return None;
  }
  let text = first_text_block(message)?;
  let summary = text.strip_prefix(COMPACT_CONTINUATION_PREAMBLE)?;
  let summary = summary
    .split_once("\n\nRecent messages are preserved verbatim.")
    .map_or(summary, |(value, _)| value);
  Some(summary.trim().to_string())
}

fn first_text_block(message: &ContextMessage) -> Option<&str> {
  message.blocks.iter().find_map(|block| match block {
    ContextBlock::Text { text } if !text.trim().is_empty() => Some(text.as_str()),
    ContextBlock::Text { .. } | ContextBlock::ToolUse { .. } | ContextBlock::ToolResult { .. } => {
      None
    }
  })
}

fn truncate_summary(content: &str, max_chars: usize) -> String {
  if content.chars().count() <= max_chars {
    return content.to_string();
  }
  let mut truncated = content.chars().take(max_chars).collect::<String>();
  truncated.push_str("...");
  truncated
}

fn extract_tag_block(content: &str, tag: &str) -> Option<String> {
  let start = format!("<{tag}>");
  let end = format!("</{tag}>");
  let start_index = content.find(&start)? + start.len();
  let end_index = content[start_index..].find(&end)? + start_index;
  Some(content[start_index..end_index].to_string())
}

fn strip_tag_block(content: &str, tag: &str) -> String {
  let start = format!("<{tag}>");
  let end = format!("</{tag}>");
  let Some(start_index) = content.find(&start) else {
    return content.to_string();
  };
  let Some(end_index_relative) = content[start_index..].find(&end) else {
    return content.to_string();
  };
  let end_index = start_index + end_index_relative + end.len();
  format!("{}{}", &content[..start_index], &content[end_index..])
}

fn collapse_blank_lines(content: &str) -> String {
  let mut result = String::new();
  let mut last_blank = false;
  for line in content.lines() {
    let is_blank = line.trim().is_empty();
    if is_blank && last_blank {
      continue;
    }
    result.push_str(line);
    result.push('\n');
    last_blank = is_blank;
  }
  result
}

fn context_path(yuanling_id: &str, config: &ContextModuleConfig) -> Result<PathBuf, ContextError> {
  validate_yuanling_id(yuanling_id)?;
  Ok(Path::new(&config.storage_dir).join(format!("{yuanling_id}.jsonl")))
}

fn validate_yuanling_id(yuanling_id: &str) -> Result<(), ContextError> {
  let valid = !yuanling_id.trim().is_empty()
    && yuanling_id
      .chars()
      .all(|value| value.is_ascii_alphanumeric() || matches!(value, '-' | '_'));

  if valid {
    Ok(())
  } else {
    Err(ContextError::InvalidYuanlingId(yuanling_id.to_string()))
  }
}

fn default_storage_dir() -> String {
  let data_dir = env_or("BACKEND_DATA_DIR", "./data");
  Path::new(&data_dir)
    .join("yuanling")
    .join("context")
    .join("yuanlings")
    .to_string_lossy()
    .to_string()
}

fn env_or_optional(key: &str) -> Option<String> {
  env::var(key).ok().filter(|value| !value.trim().is_empty())
}

fn env_or(key: &str, default: &str) -> String {
  env_or_optional(key).unwrap_or_else(|| default.to_string())
}

fn env_or_bool(key: &str, default: bool) -> bool {
  env_or_optional(key)
    .and_then(|raw| raw.parse::<bool>().ok())
    .unwrap_or(default)
}

fn env_or_usize(key: &str, default: usize) -> usize {
  env_or_optional(key)
    .and_then(|raw| raw.parse::<usize>().ok())
    .unwrap_or(default)
}

fn env_or_u32(key: &str, default: u32) -> u32 {
  env_or_optional(key)
    .and_then(|raw| raw.parse::<u32>().ok())
    .unwrap_or(default)
}

fn env_or_u64(key: &str, default: u64) -> u64 {
  env_or_optional(key)
    .and_then(|raw| raw.parse::<u64>().ok())
    .unwrap_or(default)
}

fn env_or_f64(key: &str, default: f64) -> f64 {
  env_or_optional(key)
    .and_then(|raw| raw.parse::<f64>().ok())
    .unwrap_or(default)
}

fn resolve_retention_mode() -> ContextRetentionMode {
  match env_or_optional("YUANLING_CONTEXT_RETENTION_MODE")
    .as_deref()
    .unwrap_or("tail_turns")
    .trim()
    .to_ascii_lowercase()
    .as_str()
  {
    "tail_tokens" => ContextRetentionMode::TailTokens,
    _ => ContextRetentionMode::TailTurns,
  }
}

fn resolve_expire_action() -> ContextExpireAction {
  match env_or_optional("YUANLING_CONTEXT_EXPIRE_ACTION")
    .as_deref()
    .unwrap_or("archive")
    .trim()
    .to_ascii_lowercase()
    .as_str()
  {
    "ignore" => ContextExpireAction::Ignore,
    _ => ContextExpireAction::Archive,
  }
}

fn current_time_millis() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
    .unwrap_or(0)
}

