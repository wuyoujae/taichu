use super::{ai::ToolDefinition, contact, mcp, skills};
use regex::Regex;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::{Display, Formatter};
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_MAX_SEARCH_RESULTS: usize = 10;
const DEFAULT_COMMAND_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_WEB_TIMEOUT_MS: u64 = 20_000;
const MAX_COMMAND_OUTPUT_BYTES: usize = 256 * 1024;
const DEFAULT_GREP_LIMIT: usize = 100;
const DEFAULT_GLOB_LIMIT: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPermissionMode {
  ReadOnly,
  WorkspaceWrite,
  DangerFullAccess,
}

impl ToolPermissionMode {
  pub fn code(self) -> u8 {
    match self {
      Self::ReadOnly => 1,
      Self::WorkspaceWrite => 2,
      Self::DangerFullAccess => 3,
    }
  }

  pub fn label(self) -> &'static str {
    match self {
      Self::ReadOnly => "read_only",
      Self::WorkspaceWrite => "workspace_write",
      Self::DangerFullAccess => "danger_full_access",
    }
  }

  pub fn risk(self) -> &'static str {
    match self {
      Self::ReadOnly => "This tool can read filesystem, network, or runtime data.",
      Self::WorkspaceWrite => "This tool can modify files or runtime state in its filesystem scope.",
      Self::DangerFullAccess => {
        "This tool can access external services or perform high-risk operations."
      }
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSource {
  Builtin,
  Runtime,
  Plugin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolFilesystemScope {
  Global,
  Workspace,
}

impl ToolFilesystemScope {
  fn from_env(value: Option<String>) -> Self {
    let normalized = value.unwrap_or_default().trim().to_ascii_lowercase();
    match normalized.as_str() {
      "workspace" => Self::Workspace,
      _ => Self::Global,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
  pub name: String,
  pub description: Option<String>,
  pub input_schema: Value,
  pub required_permission: ToolPermissionMode,
  pub source: ToolSource,
  pub enabled: bool,
}

impl ToolSpec {
  fn definition(&self) -> ToolDefinition {
    ToolDefinition {
      name: self.name.clone(),
      description: self.description.clone(),
      input_schema: self.input_schema.clone(),
    }
  }

  fn view(&self) -> ToolDefinitionView {
    ToolDefinitionView {
      name: self.name.clone(),
      description: self.description.clone(),
      input_schema: self.input_schema.clone(),
      required_permission: self.required_permission.code(),
      permission_label: self.required_permission.label().to_string(),
      source: self.source,
      enabled: self.enabled,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeToolDefinition {
  pub name: String,
  pub description: Option<String>,
  pub input_schema: Value,
  pub required_permission: ToolPermissionMode,
}

impl RuntimeToolDefinition {
  fn into_spec(self) -> ToolSpec {
    ToolSpec {
      name: self.name,
      description: self.description,
      input_schema: self.input_schema,
      required_permission: self.required_permission,
      source: ToolSource::Runtime,
      enabled: true,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginToolDefinition {
  pub name: String,
  pub description: Option<String>,
  pub input_schema: Value,
  pub required_permission: ToolPermissionMode,
}

impl PluginToolDefinition {
  fn into_spec(self) -> ToolSpec {
    ToolSpec {
      name: self.name,
      description: self.description,
      input_schema: self.input_schema,
      required_permission: self.required_permission,
      source: ToolSource::Plugin,
      enabled: true,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinitionView {
  pub name: String,
  pub description: Option<String>,
  pub input_schema: Value,
  pub required_permission: u8,
  pub permission_label: String,
  pub source: ToolSource,
  pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSearchMatch {
  pub name: String,
  pub description: Option<String>,
  pub source: ToolSource,
  pub score: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSearchOutput {
  pub query: String,
  pub normalized_query: String,
  pub matches: Vec<ToolSearchMatch>,
  pub total_tools: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolExecutionOutput {
  pub tool_name: String,
  pub output: Value,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub permission_outcome: Option<ToolPermissionOutcome>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolPermissionRequest {
  pub tool_name: String,
  pub input: Value,
  pub required_permission: u8,
  pub permission_label: String,
  pub source: ToolSource,
  pub description: Option<String>,
  pub risk: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "decision")]
pub enum ToolPermissionDecision {
  Allow,
  Deny {
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
  },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPermissionOutcome {
  AutoAllowed,
  UserAllowed,
}

pub trait ToolPermissionPrompter {
  fn confirm(&mut self, request: &ToolPermissionRequest) -> ToolPermissionDecision;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolPermissionPolicy {
  pub auto_allow_read: bool,
  pub confirm_workspace_write: bool,
  pub confirm_danger_full_access: bool,
}

impl Default for ToolPermissionPolicy {
  fn default() -> Self {
    Self {
      auto_allow_read: true,
      confirm_workspace_write: true,
      confirm_danger_full_access: true,
    }
  }
}

impl ToolPermissionPolicy {
  pub fn requires_confirmation(&self, permission: ToolPermissionMode) -> bool {
    match permission {
      ToolPermissionMode::ReadOnly => !self.auto_allow_read,
      ToolPermissionMode::WorkspaceWrite => self.confirm_workspace_write,
      ToolPermissionMode::DangerFullAccess => self.confirm_danger_full_access,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolError {
  ModuleDisabled,
  Disabled(String),
  NotAllowed(String),
  UnknownTool(String),
  DuplicateTool(String),
  MissingHandler(String),
  PermissionRequired(String),
  PermissionDenied(String),
  InvalidInput(String),
  ExecutionFailed(String),
}

impl Display for ToolError {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::ModuleDisabled => write!(formatter, "tools module is disabled"),
      Self::Disabled(name) => write!(formatter, "tool `{name}` is disabled"),
      Self::NotAllowed(name) => write!(formatter, "tool `{name}` is not allowed"),
      Self::UnknownTool(name) => write!(formatter, "unknown tool `{name}`"),
      Self::DuplicateTool(name) => write!(formatter, "duplicate tool `{name}`"),
      Self::MissingHandler(name) => write!(formatter, "tool `{name}` has no registered handler"),
      Self::PermissionRequired(name) => {
        write!(formatter, "tool `{name}` requires permission confirmation")
      }
      Self::PermissionDenied(message) => write!(formatter, "{message}"),
      Self::InvalidInput(message) => write!(formatter, "invalid tool input: {message}"),
      Self::ExecutionFailed(message) => write!(formatter, "{message}"),
    }
  }
}

impl std::error::Error for ToolError {}

pub trait ToolExecutor {
  fn execute(
    &mut self,
    tool_name: &str,
    input: &Value,
  ) -> Result<ToolExecutionOutput, ToolError>;
}

type StaticToolHandler = Box<dyn FnMut(&Value) -> Result<Value, ToolError>>;

#[derive(Default)]
pub struct StaticToolExecutor {
  handlers: BTreeMap<String, StaticToolHandler>,
}

impl StaticToolExecutor {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn register(
    mut self,
    tool_name: impl Into<String>,
    handler: impl FnMut(&Value) -> Result<Value, ToolError> + 'static,
  ) -> Self {
    self.handlers.insert(tool_name.into(), Box::new(handler));
    self
  }
}

impl ToolExecutor for StaticToolExecutor {
  fn execute(
    &mut self,
    tool_name: &str,
    input: &Value,
  ) -> Result<ToolExecutionOutput, ToolError> {
    let handler = self
      .handlers
      .get_mut(tool_name)
      .ok_or_else(|| ToolError::MissingHandler(tool_name.to_string()))?;
    Ok(ToolExecutionOutput {
      tool_name: tool_name.to_string(),
      output: handler(input)?,
      permission_outcome: None,
    })
  }
}

#[derive(Debug)]
pub struct BuiltinToolExecutor {
  root: PathBuf,
  filesystem_scope: ToolFilesystemScope,
  http: reqwest::blocking::Client,
}

impl BuiltinToolExecutor {
  pub fn new(workspace_root: impl AsRef<Path>) -> Result<Self, ToolError> {
    Self::with_scope(workspace_root, ToolFilesystemScope::Workspace)
  }

  pub fn global(root: impl AsRef<Path>) -> Result<Self, ToolError> {
    Self::with_scope(root, ToolFilesystemScope::Global)
  }

  pub fn with_scope(
    root: impl AsRef<Path>,
    filesystem_scope: ToolFilesystemScope,
  ) -> Result<Self, ToolError> {
    let root = root
      .as_ref()
      .canonicalize()
      .map_err(|error| ToolError::InvalidInput(format!("invalid tool root: {error}")))?;
    let http = reqwest::blocking::Client::builder()
      .timeout(Duration::from_millis(DEFAULT_WEB_TIMEOUT_MS))
      .user_agent("taichu-yuanling-tools/0.1")
      .build()
      .map_err(|error| ToolError::ExecutionFailed(format!("failed to build HTTP client: {error}")))?;

    Ok(Self {
      root,
      filesystem_scope,
      http,
    })
  }

  pub fn from_current_dir() -> Result<Self, ToolError> {
    let cwd = env::current_dir()
      .map_err(|error| ToolError::InvalidInput(format!("failed to resolve current dir: {error}")))?;
    Self::global(cwd)
  }

  pub fn from_config(config: &ToolsModuleConfig) -> Result<Self, ToolError> {
    let filesystem_scope = config.filesystem_scope;
    match &config.workspace_root {
      Some(root) => Self::with_scope(root, filesystem_scope),
      None => {
        let cwd = env::current_dir().map_err(|error| {
          ToolError::InvalidInput(format!("failed to resolve current dir: {error}"))
        })?;
        Self::with_scope(cwd, filesystem_scope)
      }
    }
  }
}

impl ToolExecutor for BuiltinToolExecutor {
  fn execute(
    &mut self,
    tool_name: &str,
    input: &Value,
  ) -> Result<ToolExecutionOutput, ToolError> {
    let output = match tool_name {
      "bash" => self.run_bash(parse_tool_input(input)?)?,
      "read_file" => self.run_read_file(parse_tool_input(input)?)?,
      "write_file" => self.run_write_file(parse_tool_input(input)?)?,
      "edit_file" => self.run_edit_file(parse_tool_input(input)?)?,
      "glob_search" => self.run_glob_search(parse_tool_input(input)?)?,
      "grep_search" => self.run_grep_search(parse_tool_input(input)?)?,
      "WebFetch" => self.run_web_fetch(parse_tool_input(input)?)?,
      "WebSearch" => self.run_web_search(parse_tool_input(input)?)?,
      "Skill" => run_skill(parse_tool_input(input)?)?,
      "send_message" => run_send_message(parse_tool_input(input)?)?,
      "MCP" => run_mcp_tool(parse_tool_input(input)?)?,
      "ListMcpResources" => run_list_mcp_resources(parse_tool_input(input)?)?,
      "ReadMcpResource" => run_read_mcp_resource(parse_tool_input(input)?)?,
      "McpAuth" => run_mcp_auth(parse_tool_input(input)?)?,
      "Sleep" => run_sleep(parse_tool_input(input)?)?,
      "REPL" => self.run_repl(parse_tool_input(input)?)?,
      "PowerShell" => self.run_powershell(parse_tool_input(input)?)?,
      _ => return Err(ToolError::MissingHandler(tool_name.to_string())),
    };

    Ok(ToolExecutionOutput {
      tool_name: tool_name.to_string(),
      output,
      permission_outcome: None,
    })
  }
}

#[derive(Debug, Deserialize)]
struct BashInput {
  command: String,
  timeout_ms: Option<u64>,
  timeout: Option<u64>,
  cwd: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReadFileInput {
  path: String,
  offset: Option<usize>,
  limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct WriteFileInput {
  path: String,
  content: String,
}

#[derive(Debug, Deserialize)]
struct EditFileInput {
  path: String,
  old_string: String,
  new_string: String,
  replace_all: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct GlobSearchInput {
  pattern: String,
  path: Option<String>,
  limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct GrepSearchInput {
  pattern: String,
  path: Option<String>,
  include: Option<String>,
  limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct WebFetchInput {
  url: String,
  max_chars: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct WebSearchInput {
  query: String,
  max_results: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct SkillInput {
  skill: String,
  args: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SendMessageInput {
  from_yuanling_id: String,
  to_yuanling_id: String,
  content: String,
}

#[derive(Debug, Deserialize)]
struct McpToolInput {
  server: String,
  tool: String,
  arguments: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ListMcpResourcesInput {
  server: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReadMcpResourceInput {
  server: Option<String>,
  uri: String,
}

#[derive(Debug, Deserialize)]
struct McpAuthInput {
  server: String,
}

#[derive(Debug, Deserialize)]
struct SleepInput {
  duration_ms: Option<u64>,
  seconds: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ReplInput {
  language: Option<String>,
  code: String,
  timeout_ms: Option<u64>,
  command: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PowerShellInput {
  command: String,
  timeout_ms: Option<u64>,
  timeout: Option<u64>,
  cwd: Option<String>,
}

impl BuiltinToolExecutor {
  fn run_bash(&self, input: BashInput) -> Result<Value, ToolError> {
    if input.command.trim().is_empty() {
      return Err(ToolError::InvalidInput("command is required".to_string()));
    }
    let cwd = self.resolve_optional_dir(input.cwd.as_deref())?;
    run_command(
      "bash",
      &["-lc", input.command.as_str()],
      None,
      &cwd,
      command_timeout(input.timeout_ms, input.timeout),
    )
  }

  fn run_read_file(&self, input: ReadFileInput) -> Result<Value, ToolError> {
    let path = self.resolve_existing_file(&input.path)?;
    let bytes = fs::read(&path).map_err(io_to_tool_error)?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let lines = text.lines().collect::<Vec<_>>();
    let offset = input.offset.unwrap_or(0);
    let selected = lines
      .iter()
      .skip(offset)
      .take(input.limit.unwrap_or(usize::MAX))
      .copied()
      .collect::<Vec<_>>();
    let content = selected.join("\n");
    let truncated = offset > 0 || offset.saturating_add(selected.len()) < lines.len();

    Ok(json!({
      "path": self.display_workspace_path(&path),
      "content": content,
      "bytes": bytes.len(),
      "total_lines": lines.len(),
      "offset": offset,
      "returned_lines": selected.len(),
      "truncated": truncated
    }))
  }

  fn run_write_file(&self, input: WriteFileInput) -> Result<Value, ToolError> {
    let path = self.resolve_writable_file(&input.path)?;
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent).map_err(io_to_tool_error)?;
    }
    fs::write(&path, input.content.as_bytes()).map_err(io_to_tool_error)?;
    Ok(json!({
      "path": self.display_workspace_path(&path),
      "bytes_written": input.content.len()
    }))
  }

  fn run_edit_file(&self, input: EditFileInput) -> Result<Value, ToolError> {
    if input.old_string.is_empty() {
      return Err(ToolError::InvalidInput("old_string cannot be empty".to_string()));
    }
    let path = self.resolve_existing_file(&input.path)?;
    let original = fs::read_to_string(&path).map_err(io_to_tool_error)?;
    let matches = original.matches(&input.old_string).count();
    if matches == 0 {
      return Err(ToolError::ExecutionFailed(format!(
        "old_string was not found in `{}`",
        input.path
      )));
    }

    let replace_all = input.replace_all.unwrap_or(false);
    let updated = if replace_all {
      original.replace(&input.old_string, &input.new_string)
    } else {
      original.replacen(&input.old_string, &input.new_string, 1)
    };
    fs::write(&path, updated.as_bytes()).map_err(io_to_tool_error)?;

    Ok(json!({
      "path": self.display_workspace_path(&path),
      "replacements": if replace_all { matches } else { 1 }
    }))
  }

  fn run_glob_search(&self, input: GlobSearchInput) -> Result<Value, ToolError> {
    if input.pattern.trim().is_empty() {
      return Err(ToolError::InvalidInput("pattern is required".to_string()));
    }
    let base = self.resolve_optional_dir(input.path.as_deref())?;
    let limit = input.limit.unwrap_or(DEFAULT_GLOB_LIMIT).max(1);
    let mut files = Vec::new();
    self.collect_matching_files(&base, &input.pattern, None, limit, &mut files)?;
    let matches = files
      .iter()
      .map(|path| self.display_workspace_path(path))
      .collect::<Vec<_>>();

    Ok(json!({
      "pattern": input.pattern,
      "matches": matches,
      "limit": limit
    }))
  }

  fn run_grep_search(&self, input: GrepSearchInput) -> Result<Value, ToolError> {
    if input.pattern.trim().is_empty() {
      return Err(ToolError::InvalidInput("pattern is required".to_string()));
    }
    let regex = Regex::new(&input.pattern)
      .map_err(|error| ToolError::InvalidInput(format!("invalid regex pattern: {error}")))?;
    let base = self.resolve_optional_dir(input.path.as_deref())?;
    let limit = input.limit.unwrap_or(DEFAULT_GREP_LIMIT).max(1);
    let mut files = Vec::new();
    self.collect_matching_files(
      &base,
      input.include.as_deref().unwrap_or("*"),
      None,
      usize::MAX,
      &mut files,
    )?;

    let mut matches = Vec::new();
    for path in files {
      if matches.len() >= limit {
        break;
      }
      let Ok(content) = fs::read_to_string(&path) else {
        continue;
      };
      for (index, line) in content.lines().enumerate() {
        if regex.is_match(line) {
          matches.push(json!({
            "path": self.display_workspace_path(&path),
            "line_number": index + 1,
            "line": line
          }));
          if matches.len() >= limit {
            break;
          }
        }
      }
    }

    Ok(json!({
      "pattern": input.pattern,
      "matches": matches,
      "limit": limit
    }))
  }

  fn run_web_fetch(&self, input: WebFetchInput) -> Result<Value, ToolError> {
    validate_http_url(&input.url)?;
    let response = self
      .http
      .get(&input.url)
      .send()
      .map_err(|error| ToolError::ExecutionFailed(format!("WebFetch failed: {error}")))?;
    let status = response.status().as_u16();
    let content_type = response
      .headers()
      .get(reqwest::header::CONTENT_TYPE)
      .and_then(|value| value.to_str().ok())
      .unwrap_or("")
      .to_string();
    let body = response
      .text()
      .map_err(|error| ToolError::ExecutionFailed(format!("failed to read WebFetch body: {error}")))?;
    let readable = if content_type.contains("html") || body.to_ascii_lowercase().contains("<html") {
      html_to_text(&body)
    } else {
      body
    };
    let max_chars = input.max_chars.unwrap_or(12_000);
    let (text, truncated) = truncate_chars(&readable, max_chars);

    Ok(json!({
      "url": input.url,
      "status": status,
      "content_type": content_type,
      "text": text,
      "truncated": truncated
    }))
  }

  fn run_web_search(&self, input: WebSearchInput) -> Result<Value, ToolError> {
    if input.query.trim().is_empty() {
      return Err(ToolError::InvalidInput("query is required".to_string()));
    }
    let max_results = input.max_results.unwrap_or(5).clamp(1, 10);
    let url = format!(
      "https://html.duckduckgo.com/html/?q={}",
      percent_encode(&input.query)
    );
    let response = self
      .http
      .get(&url)
      .send()
      .map_err(|error| ToolError::ExecutionFailed(format!("WebSearch failed: {error}")))?;
    let body = response
      .text()
      .map_err(|error| ToolError::ExecutionFailed(format!("failed to read WebSearch body: {error}")))?;
    let results = parse_duckduckgo_results(&body, max_results);

    Ok(json!({
      "query": input.query,
      "results": results,
      "source": "duckduckgo_html"
    }))
  }

  fn run_repl(&self, input: ReplInput) -> Result<Value, ToolError> {
    if input.code.trim().is_empty() {
      return Err(ToolError::InvalidInput("code is required".to_string()));
    }
    let timeout = input.timeout_ms.unwrap_or(DEFAULT_COMMAND_TIMEOUT_MS);
    let cwd = self.root.clone();

    if let Some(command) = input.command {
      return run_command(
        "bash",
        &["-lc", command.as_str()],
        Some(input.code.as_bytes()),
        &cwd,
        timeout,
      );
    }

    let language = input.language.unwrap_or_else(|| "python".to_string());
    let (program, args): (&str, Vec<&str>) = match language.to_ascii_lowercase().as_str() {
      "python" | "python3" => ("python3", Vec::new()),
      "node" | "javascript" | "js" => ("node", Vec::new()),
      "bash" | "sh" => ("bash", Vec::new()),
      other => {
        return Err(ToolError::InvalidInput(format!(
          "unsupported REPL language `{other}`"
        )))
      }
    };

    run_command(program, &args, Some(input.code.as_bytes()), &cwd, timeout)
  }

  fn run_powershell(&self, input: PowerShellInput) -> Result<Value, ToolError> {
    if input.command.trim().is_empty() {
      return Err(ToolError::InvalidInput("command is required".to_string()));
    }
    let program = find_program(&["pwsh", "powershell"])
      .ok_or_else(|| ToolError::ExecutionFailed("PowerShell executable was not found".to_string()))?;
    let cwd = self.resolve_optional_dir(input.cwd.as_deref())?;
    run_command(
      &program,
      &["-NoLogo", "-NoProfile", "-Command", input.command.as_str()],
      None,
      &cwd,
      command_timeout(input.timeout_ms, input.timeout),
    )
  }

  fn collect_matching_files(
    &self,
    base: &Path,
    pattern: &str,
    include: Option<&str>,
    limit: usize,
    output: &mut Vec<PathBuf>,
  ) -> Result<(), ToolError> {
    if output.len() >= limit {
      return Ok(());
    }

    for entry in fs::read_dir(base).map_err(io_to_tool_error)? {
      let entry = entry.map_err(io_to_tool_error)?;
      let path = entry.path();
      let metadata = entry.metadata().map_err(io_to_tool_error)?;
      if metadata.is_dir() {
        self.collect_matching_files(&path, pattern, include, limit, output)?;
      } else if metadata.is_file() {
        let relative = self.display_workspace_path(&path);
        let file_name = path.file_name().and_then(|value| value.to_str()).unwrap_or("");
        let pattern_matches = wildcard_match(pattern, &relative) || wildcard_match(pattern, file_name);
        let include_matches = include.is_none_or(|value| {
          wildcard_match(value, &relative) || wildcard_match(value, file_name)
        });
        if pattern_matches && include_matches {
          output.push(path);
          if output.len() >= limit {
            break;
          }
        }
      }
    }

    Ok(())
  }

  fn resolve_optional_dir(&self, path: Option<&str>) -> Result<PathBuf, ToolError> {
    match path {
      Some(value) if !value.trim().is_empty() => {
        let resolved = self.resolve_workspace_path(value)?;
        let canonical = resolved.canonicalize().map_err(io_to_tool_error)?;
        if !canonical.is_dir() {
          return Err(ToolError::InvalidInput(format!("`{value}` is not a directory")));
        }
        self.ensure_inside_workspace(&canonical)?;
        Ok(canonical)
      }
      _ => Ok(self.root.clone()),
    }
  }

  fn resolve_existing_file(&self, path: &str) -> Result<PathBuf, ToolError> {
    let resolved = self.resolve_workspace_path(path)?;
    let canonical = resolved.canonicalize().map_err(io_to_tool_error)?;
    if !canonical.is_file() {
      return Err(ToolError::InvalidInput(format!("`{path}` is not a file")));
    }
    self.ensure_inside_workspace(&canonical)?;
    Ok(canonical)
  }

  fn resolve_writable_file(&self, path: &str) -> Result<PathBuf, ToolError> {
    let resolved = self.resolve_workspace_path(path)?;
    if resolved.exists() {
      let canonical = resolved.canonicalize().map_err(io_to_tool_error)?;
      self.ensure_inside_workspace(&canonical)?;
      return Ok(canonical);
    }

    if let Some(parent) = resolved.parent() {
      if parent.exists() {
        let canonical_parent = parent.canonicalize().map_err(io_to_tool_error)?;
        self.ensure_inside_workspace(&canonical_parent)?;
      }
    }

    Ok(resolved)
  }

  fn resolve_workspace_path(&self, raw_path: &str) -> Result<PathBuf, ToolError> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
      return Err(ToolError::InvalidInput("path is required".to_string()));
    }
    let path = Path::new(trimmed);
    if self.filesystem_scope == ToolFilesystemScope::Global {
      return Ok(if path.is_absolute() {
        path.to_path_buf()
      } else {
        self.root.join(path)
      });
    }

    if path.is_absolute() {
      return Err(ToolError::InvalidInput(
        "absolute paths are not allowed in workspace filesystem scope".to_string(),
      ));
    }
    if path.components().any(|component| {
      matches!(
        component,
        Component::ParentDir | Component::RootDir | Component::Prefix(_)
      )
    }) {
      return Err(ToolError::InvalidInput(
        "path traversal is not allowed in workspace filesystem scope".to_string(),
      ));
    }
    Ok(self.root.join(path))
  }

  fn ensure_inside_workspace(&self, path: &Path) -> Result<(), ToolError> {
    if self.filesystem_scope == ToolFilesystemScope::Global || path.starts_with(&self.root) {
      Ok(())
    } else {
      Err(ToolError::InvalidInput(
        "resolved path is outside the workspace filesystem scope".to_string(),
      ))
    }
  }

  fn display_workspace_path(&self, path: &Path) -> String {
    if self.filesystem_scope == ToolFilesystemScope::Global {
      return path.to_string_lossy().replace('\\', "/");
    }

    path
      .strip_prefix(&self.root)
      .unwrap_or(path)
      .to_string_lossy()
      .replace('\\', "/")
  }
}

#[derive(Debug, Clone)]
pub struct ToolRegistry {
  builtin_tools: Vec<ToolSpec>,
  runtime_tools: Vec<ToolSpec>,
  plugin_tools: Vec<ToolSpec>,
}

impl ToolRegistry {
  pub fn builtin() -> Self {
    Self {
      builtin_tools: builtin_tool_specs(),
      runtime_tools: Vec::new(),
      plugin_tools: Vec::new(),
    }
  }

  pub fn with_runtime_tools(
    mut self,
    runtime_tools: Vec<RuntimeToolDefinition>,
  ) -> Result<Self, ToolError> {
    for tool in runtime_tools {
      self.register_runtime_tool(tool)?;
    }
    Ok(self)
  }

  pub fn with_plugin_tools(
    mut self,
    plugin_tools: Vec<PluginToolDefinition>,
  ) -> Result<Self, ToolError> {
    for tool in plugin_tools {
      let spec = tool.into_spec();
      self.ensure_unique_tool_name(&spec.name)?;
      self.plugin_tools.push(spec);
    }
    Ok(self)
  }

  pub fn register_runtime_tool(
    &mut self,
    tool: RuntimeToolDefinition,
  ) -> Result<(), ToolError> {
    let spec = tool.into_spec();
    self.ensure_unique_tool_name(&spec.name)?;
    self.runtime_tools.push(spec);
    Ok(())
  }

  pub fn definitions(&self, allowed_tools: Option<&BTreeSet<String>>) -> Vec<ToolDefinition> {
    self
      .all_specs()
      .into_iter()
      .filter(|spec| spec.enabled)
      .filter(|spec| allowed_tools.is_none_or(|allowed| allowed.contains(&spec.name)))
      .map(|spec| spec.definition())
      .collect()
  }

  pub fn views(&self, allowed_tools: Option<&BTreeSet<String>>) -> Vec<ToolDefinitionView> {
    self
      .all_specs()
      .into_iter()
      .filter(|spec| allowed_tools.is_none_or(|allowed| allowed.contains(&spec.name)))
      .map(|spec| spec.view())
      .collect()
  }

  pub fn normalize_allowed_tools(
    &self,
    values: &[String],
  ) -> Result<Option<BTreeSet<String>>, ToolError> {
    if values.is_empty() {
      return Ok(None);
    }

    let mut name_map = self
      .all_specs()
      .into_iter()
      .map(|spec| (normalize_tool_name(&spec.name), spec.name.clone()))
      .collect::<BTreeMap<_, _>>();

    for (alias, canonical) in [
      ("read", "read_file"),
      ("write", "write_file"),
      ("edit", "edit_file"),
      ("glob", "glob_search"),
      ("grep", "grep_search"),
    ] {
      name_map.insert(alias.to_string(), canonical.to_string());
    }

    let mut allowed = BTreeSet::new();
    for value in values {
      for token in value
        .split(|ch: char| ch == ',' || ch.is_whitespace())
        .filter(|token| !token.trim().is_empty())
      {
        let normalized = normalize_tool_name(token);
        let Some(canonical) = name_map.get(&normalized) else {
          return Err(ToolError::UnknownTool(token.to_string()));
        };
        allowed.insert(canonical.clone());
      }
    }

    Ok((!allowed.is_empty()).then_some(allowed))
  }

  pub fn search(&self, query: &str, max_results: usize) -> ToolSearchOutput {
    let query = query.trim().to_string();
    let normalized_query = normalize_tool_name(&query);
    let mut matches = self
      .all_specs()
      .into_iter()
      .filter(|spec| spec.enabled)
      .filter_map(|spec| {
        let score = score_tool_match(spec, &normalized_query);
        (score > 0 || normalized_query.is_empty()).then(|| ToolSearchMatch {
          name: spec.name.clone(),
          description: spec.description.clone(),
          source: spec.source,
          score,
        })
      })
      .collect::<Vec<_>>();

    matches.sort_by(|left, right| {
      right
        .score
        .cmp(&left.score)
        .then_with(|| left.name.cmp(&right.name))
    });
    matches.truncate(max_results.max(1));

    ToolSearchOutput {
      query,
      normalized_query,
      matches,
      total_tools: self.all_specs().len(),
    }
  }

  pub fn execute<E: ToolExecutor>(
    &self,
    name: &str,
    input: &Value,
    allowed_tools: Option<&BTreeSet<String>>,
    executor: &mut E,
  ) -> Result<ToolExecutionOutput, ToolError> {
    let canonical = self.canonical_tool_name(name)?;
    let spec = self
      .find_spec(&canonical)
      .ok_or_else(|| ToolError::UnknownTool(name.to_string()))?;

    if !spec.enabled {
      return Err(ToolError::Disabled(canonical));
    }

    if allowed_tools.is_some_and(|allowed| !allowed.contains(&canonical)) {
      return Err(ToolError::NotAllowed(canonical));
    }

    if canonical == "ToolSearch" {
      let query = input.get("query").and_then(Value::as_str).unwrap_or("");
      let max_results = input
        .get("max_results")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(DEFAULT_MAX_SEARCH_RESULTS);
      return Ok(ToolExecutionOutput {
        tool_name: canonical,
        output: json!(self.search(query, max_results)),
        permission_outcome: None,
      });
    }

    executor.execute(&canonical, input)
  }

  pub fn execute_with_permissions<E: ToolExecutor>(
    &self,
    name: &str,
    input: &Value,
    config: &ToolsModuleConfig,
    executor: &mut E,
    mut prompter: Option<&mut dyn ToolPermissionPrompter>,
  ) -> Result<ToolExecutionOutput, ToolError> {
    if !config.enabled {
      return Err(ToolError::ModuleDisabled);
    }

    let canonical = self.canonical_tool_name(name)?;
    let spec = self
      .find_spec(&canonical)
      .ok_or_else(|| ToolError::UnknownTool(name.to_string()))?;

    if config
      .allowed_tools
      .as_ref()
      .is_some_and(|allowed| !allowed.contains(&canonical))
    {
      return Err(ToolError::NotAllowed(canonical));
    }

    if !spec.enabled {
      return Err(ToolError::Disabled(canonical));
    }

    let permission_outcome = if !config
      .permission_policy
      .requires_confirmation(spec.required_permission)
    {
      ToolPermissionOutcome::AutoAllowed
    } else {
      let request = ToolPermissionRequest {
        tool_name: spec.name.clone(),
        input: input.clone(),
        required_permission: spec.required_permission.code(),
        permission_label: spec.required_permission.label().to_string(),
        source: spec.source,
        description: spec.description.clone(),
        risk: spec.required_permission.risk().to_string(),
      };

      let Some(prompter) = prompter.as_mut() else {
        return Err(ToolError::PermissionRequired(spec.name.clone()));
      };

      match prompter.confirm(&request) {
        ToolPermissionDecision::Allow => ToolPermissionOutcome::UserAllowed,
        ToolPermissionDecision::Deny { reason } => {
          let suffix = reason
            .filter(|value| !value.trim().is_empty())
            .map(|value| format!(": {value}"))
            .unwrap_or_default();
          return Err(ToolError::PermissionDenied(format!(
            "tool `{}` permission denied{}",
            spec.name, suffix
          )));
        }
      }
    };

    if canonical == "ToolSearch" {
      let query = input.get("query").and_then(Value::as_str).unwrap_or("");
      let max_results = input
        .get("max_results")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(config.max_search_results);
      return Ok(ToolExecutionOutput {
        tool_name: canonical,
        output: json!(self.search(query, max_results)),
        permission_outcome: Some(permission_outcome),
      });
    }

    let mut output = executor.execute(&canonical, input)?;
    output.permission_outcome = Some(permission_outcome);
    Ok(output)
  }

  fn ensure_unique_tool_name(&self, name: &str) -> Result<(), ToolError> {
    if self.find_spec(name).is_some() {
      Err(ToolError::DuplicateTool(name.to_string()))
    } else {
      Ok(())
    }
  }

  fn canonical_tool_name(&self, name: &str) -> Result<String, ToolError> {
    let normalized = normalize_tool_name(name);
    let aliases = alias_map();
    let normalized = aliases.get(normalized.as_str()).unwrap_or(&normalized);
    self
      .all_specs()
      .into_iter()
      .find(|spec| normalize_tool_name(&spec.name) == *normalized)
      .map(|spec| spec.name.clone())
      .ok_or_else(|| ToolError::UnknownTool(name.to_string()))
  }

  fn find_spec(&self, name: &str) -> Option<&ToolSpec> {
    self
      .builtin_tools
      .iter()
      .chain(self.runtime_tools.iter())
      .chain(self.plugin_tools.iter())
      .find(|spec| spec.name == name)
  }

  fn all_specs(&self) -> Vec<&ToolSpec> {
    self
      .builtin_tools
      .iter()
      .chain(self.runtime_tools.iter())
      .chain(self.plugin_tools.iter())
      .collect()
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsModuleConfig {
  pub enabled: bool,
  pub allowed_tools: Option<BTreeSet<String>>,
  pub max_search_results: usize,
  pub permission_policy: ToolPermissionPolicy,
  pub filesystem_scope: ToolFilesystemScope,
  pub workspace_root: Option<String>,
}

impl ToolsModuleConfig {
  pub fn as_view(&self, registry: &ToolRegistry) -> ToolsModuleConfigView {
    ToolsModuleConfigView {
      enabled: self.enabled,
      allowed_tools: self.allowed_tools.clone(),
      max_search_results: self.max_search_results,
      permission_policy: self.permission_policy.clone(),
      filesystem_scope: self.filesystem_scope,
      workspace_root: self.workspace_root.clone(),
      registered_count: registry.all_specs().len(),
      exposed_count: registry.definitions(self.allowed_tools.as_ref()).len(),
      tools: registry.views(self.allowed_tools.as_ref()),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsModuleConfigView {
  pub enabled: bool,
  pub allowed_tools: Option<BTreeSet<String>>,
  pub max_search_results: usize,
  pub permission_policy: ToolPermissionPolicy,
  pub filesystem_scope: ToolFilesystemScope,
  pub workspace_root: Option<String>,
  pub registered_count: usize,
  pub exposed_count: usize,
  pub tools: Vec<ToolDefinitionView>,
}

pub fn resolve_from_env() -> ToolsModuleConfig {
  let registry = ToolRegistry::builtin();
  let allowed_values = env_or_optional("YUANLING_TOOLS_ALLOWED")
    .map(|raw| vec![raw])
    .unwrap_or_default();
  let allowed_tools = registry.normalize_allowed_tools(&allowed_values).ok().flatten();

  ToolsModuleConfig {
    enabled: env_or_bool("YUANLING_TOOLS_ENABLED", true),
    allowed_tools,
    max_search_results: env_or_usize("YUANLING_TOOLS_MAX_SEARCH_RESULTS", DEFAULT_MAX_SEARCH_RESULTS),
    filesystem_scope: ToolFilesystemScope::from_env(env_or_optional(
      "YUANLING_TOOLS_FILESYSTEM_SCOPE",
    )),
    workspace_root: env_or_optional("YUANLING_TOOLS_WORKSPACE_ROOT"),
    permission_policy: ToolPermissionPolicy {
      auto_allow_read: env_or_bool("YUANLING_TOOLS_AUTO_ALLOW_READ", true),
      confirm_workspace_write: env_or_bool("YUANLING_TOOLS_CONFIRM_WORKSPACE_WRITE", true),
      confirm_danger_full_access: env_or_bool(
        "YUANLING_TOOLS_CONFIRM_DANGER_FULL_ACCESS",
        true,
      ),
    },
  }
}

pub fn default_tools() -> Vec<ToolDefinitionView> {
  ToolRegistry::builtin().views(None)
}

pub async fn registry_with_mcp_tools(
  config: &mcp::McpModuleConfig,
) -> Result<ToolRegistry, ToolError> {
  let runtime_tools = mcp::discover_runtime_tools(config)
    .await
    .map_err(|error| ToolError::ExecutionFailed(error.to_string()))?;
  ToolRegistry::builtin().with_runtime_tools(runtime_tools)
}

pub fn register_tool(tool_id: &str) -> bool {
  ToolRegistry::builtin().canonical_tool_name(tool_id).is_ok()
}

fn builtin_tool_specs() -> Vec<ToolSpec> {
  vec![
    ToolSpec {
      name: "bash".to_string(),
      description: Some("Execute a shell command in the configured user environment.".to_string()),
      input_schema: json!({
        "type": "object",
        "properties": {
          "command": { "type": "string" },
          "timeout_ms": { "type": "integer", "minimum": 1 },
          "timeout": { "type": "integer", "minimum": 1 },
          "cwd": { "type": "string" }
        },
        "required": ["command"],
        "additionalProperties": false
      }),
      required_permission: ToolPermissionMode::DangerFullAccess,
      source: ToolSource::Builtin,
      enabled: true,
    },
    ToolSpec {
      name: "read_file".to_string(),
      description: Some("Read a text file from the configured filesystem scope.".to_string()),
      input_schema: json!({
        "type": "object",
        "properties": {
          "path": { "type": "string" },
          "offset": { "type": "integer", "minimum": 0 },
          "limit": { "type": "integer", "minimum": 1 }
        },
        "required": ["path"],
        "additionalProperties": false
      }),
      required_permission: ToolPermissionMode::ReadOnly,
      source: ToolSource::Builtin,
      enabled: true,
    },
    ToolSpec {
      name: "write_file".to_string(),
      description: Some("Write a text file in the configured filesystem scope.".to_string()),
      input_schema: json!({
        "type": "object",
        "properties": {
          "path": { "type": "string" },
          "content": { "type": "string" }
        },
        "required": ["path", "content"],
        "additionalProperties": false
      }),
      required_permission: ToolPermissionMode::WorkspaceWrite,
      source: ToolSource::Builtin,
      enabled: true,
    },
    ToolSpec {
      name: "edit_file".to_string(),
      description: Some("Replace text in a file from the configured filesystem scope.".to_string()),
      input_schema: json!({
        "type": "object",
        "properties": {
          "path": { "type": "string" },
          "old_string": { "type": "string" },
          "new_string": { "type": "string" },
          "replace_all": { "type": "boolean" }
        },
        "required": ["path", "old_string", "new_string"],
        "additionalProperties": false
      }),
      required_permission: ToolPermissionMode::WorkspaceWrite,
      source: ToolSource::Builtin,
      enabled: true,
    },
    ToolSpec {
      name: "glob_search".to_string(),
      description: Some("Find files by glob pattern in the configured filesystem scope.".to_string()),
      input_schema: json!({
        "type": "object",
        "properties": {
          "pattern": { "type": "string" },
          "path": { "type": "string" }
        },
        "required": ["pattern"],
        "additionalProperties": false
      }),
      required_permission: ToolPermissionMode::ReadOnly,
      source: ToolSource::Builtin,
      enabled: true,
    },
    ToolSpec {
      name: "grep_search".to_string(),
      description: Some("Search text by regular expression in the configured filesystem scope.".to_string()),
      input_schema: json!({
        "type": "object",
        "properties": {
          "pattern": { "type": "string" },
          "path": { "type": "string" },
          "include": { "type": "string" }
        },
        "required": ["pattern"],
        "additionalProperties": false
      }),
      required_permission: ToolPermissionMode::ReadOnly,
      source: ToolSource::Builtin,
      enabled: true,
    },
    ToolSpec {
      name: "WebFetch".to_string(),
      description: Some("Fetch URL content and convert it into readable text.".to_string()),
      input_schema: json!({
        "type": "object",
        "properties": {
          "url": { "type": "string" },
          "max_chars": { "type": "integer", "minimum": 1 }
        },
        "required": ["url"],
        "additionalProperties": false
      }),
      required_permission: ToolPermissionMode::ReadOnly,
      source: ToolSource::Builtin,
      enabled: true,
    },
    ToolSpec {
      name: "WebSearch".to_string(),
      description: Some("Search the web and return cited results.".to_string()),
      input_schema: json!({
        "type": "object",
        "properties": {
          "query": { "type": "string" },
          "max_results": { "type": "integer", "minimum": 1, "maximum": 10 }
        },
        "required": ["query"],
        "additionalProperties": false
      }),
      required_permission: ToolPermissionMode::ReadOnly,
      source: ToolSource::Builtin,
      enabled: true,
    },
    ToolSpec {
      name: "ToolSearch".to_string(),
      description: Some("Search available tools by name, description, or source.".to_string()),
      input_schema: json!({
        "type": "object",
        "properties": {
          "query": { "type": "string" },
          "max_results": { "type": "integer", "minimum": 1 }
        },
        "required": ["query"],
        "additionalProperties": false
      }),
      required_permission: ToolPermissionMode::ReadOnly,
      source: ToolSource::Builtin,
      enabled: true,
    },
    ToolSpec {
      name: "send_message".to_string(),
      description: Some("Send one internal message from one Yuanling to another Yuanling.".to_string()),
      input_schema: json!({
        "type": "object",
        "properties": {
          "from_yuanling_id": { "type": "string" },
          "to_yuanling_id": { "type": "string" },
          "content": { "type": "string" }
        },
        "required": ["from_yuanling_id", "to_yuanling_id", "content"],
        "additionalProperties": false
      }),
      required_permission: ToolPermissionMode::ReadOnly,
      source: ToolSource::Builtin,
      enabled: true,
    },
    ToolSpec {
      name: "Skill".to_string(),
      description: Some("Load a local skill definition and its instructions.".to_string()),
      input_schema: json!({
        "type": "object",
        "properties": {
          "skill": { "type": "string" },
          "args": { "type": "string" }
        },
        "required": ["skill"],
        "additionalProperties": false
      }),
      required_permission: ToolPermissionMode::ReadOnly,
      source: ToolSource::Builtin,
      enabled: true,
    },
    ToolSpec {
      name: "MCP".to_string(),
      description: Some("Call a tool exposed by a configured MCP server.".to_string()),
      input_schema: json!({
        "type": "object",
        "properties": {
          "server": { "type": "string" },
          "tool": { "type": "string" },
          "arguments": { "type": "object" }
        },
        "required": ["server", "tool"],
        "additionalProperties": false
      }),
      required_permission: ToolPermissionMode::DangerFullAccess,
      source: ToolSource::Builtin,
      enabled: true,
    },
    ToolSpec {
      name: "ListMcpResources".to_string(),
      description: Some("List available resources from configured MCP servers.".to_string()),
      input_schema: json!({
        "type": "object",
        "properties": {
          "server": { "type": "string" }
        },
        "additionalProperties": false
      }),
      required_permission: ToolPermissionMode::ReadOnly,
      source: ToolSource::Builtin,
      enabled: true,
    },
    ToolSpec {
      name: "ReadMcpResource".to_string(),
      description: Some("Read a specific resource from a configured MCP server by URI.".to_string()),
      input_schema: json!({
        "type": "object",
        "properties": {
          "server": { "type": "string" },
          "uri": { "type": "string" }
        },
        "required": ["uri"],
        "additionalProperties": false
      }),
      required_permission: ToolPermissionMode::ReadOnly,
      source: ToolSource::Builtin,
      enabled: true,
    },
    ToolSpec {
      name: "McpAuth".to_string(),
      description: Some("Prepare authentication for an MCP server when a future transport requires it.".to_string()),
      input_schema: json!({
        "type": "object",
        "properties": {
          "server": { "type": "string" }
        },
        "required": ["server"],
        "additionalProperties": false
      }),
      required_permission: ToolPermissionMode::DangerFullAccess,
      source: ToolSource::Builtin,
      enabled: true,
    },
    ToolSpec {
      name: "Sleep".to_string(),
      description: Some("Wait for a duration without occupying a shell process.".to_string()),
      input_schema: json!({
        "type": "object",
        "properties": {
          "duration_ms": { "type": "integer", "minimum": 1 },
          "seconds": { "type": "number", "minimum": 0.001 }
        },
        "additionalProperties": false
      }),
      required_permission: ToolPermissionMode::ReadOnly,
      source: ToolSource::Builtin,
      enabled: true,
    },
    ToolSpec {
      name: "REPL".to_string(),
      description: Some("Execute code in a REPL-like subprocess.".to_string()),
      input_schema: json!({
        "type": "object",
        "properties": {
          "language": { "type": "string", "enum": ["python", "python3", "node", "javascript", "js", "bash", "sh"] },
          "code": { "type": "string" },
          "timeout_ms": { "type": "integer", "minimum": 1 },
          "command": { "type": "string" }
        },
        "required": ["code"],
        "additionalProperties": false
      }),
      required_permission: ToolPermissionMode::DangerFullAccess,
      source: ToolSource::Builtin,
      enabled: true,
    },
    ToolSpec {
      name: "PowerShell".to_string(),
      description: Some("Execute a PowerShell command in the configured user environment.".to_string()),
      input_schema: json!({
        "type": "object",
        "properties": {
          "command": { "type": "string" },
          "timeout_ms": { "type": "integer", "minimum": 1 },
          "timeout": { "type": "integer", "minimum": 1 },
          "cwd": { "type": "string" }
        },
        "required": ["command"],
        "additionalProperties": false
      }),
      required_permission: ToolPermissionMode::DangerFullAccess,
      source: ToolSource::Builtin,
      enabled: true,
    },
  ]
}

fn parse_tool_input<T: DeserializeOwned>(input: &Value) -> Result<T, ToolError> {
  serde_json::from_value(input.clone())
    .map_err(|error| ToolError::InvalidInput(error.to_string()))
}

fn io_to_tool_error(error: std::io::Error) -> ToolError {
  ToolError::ExecutionFailed(error.to_string())
}

fn command_timeout(timeout_ms: Option<u64>, timeout_seconds: Option<u64>) -> u64 {
  timeout_ms
    .or_else(|| timeout_seconds.map(|value| value.saturating_mul(1000)))
    .unwrap_or(DEFAULT_COMMAND_TIMEOUT_MS)
    .max(1)
}

fn run_command(
  program: &str,
  args: &[&str],
  stdin: Option<&[u8]>,
  cwd: &Path,
  timeout_ms: u64,
) -> Result<Value, ToolError> {
  let started = Instant::now();
  let mut child = Command::new(program)
    .args(args)
    .current_dir(cwd)
    .stdin(if stdin.is_some() {
      Stdio::piped()
    } else {
      Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|error| ToolError::ExecutionFailed(format!("failed to start `{program}`: {error}")))?;

  if let Some(input) = stdin {
    if let Some(mut child_stdin) = child.stdin.take() {
      child_stdin.write_all(input).map_err(io_to_tool_error)?;
    }
  }

  let stdout = child
    .stdout
    .take()
    .ok_or_else(|| ToolError::ExecutionFailed("failed to capture stdout".to_string()))?;
  let stderr = child
    .stderr
    .take()
    .ok_or_else(|| ToolError::ExecutionFailed("failed to capture stderr".to_string()))?;
  let stdout_reader = thread::spawn(move || read_stream_limited(stdout, MAX_COMMAND_OUTPUT_BYTES));
  let stderr_reader = thread::spawn(move || read_stream_limited(stderr, MAX_COMMAND_OUTPUT_BYTES));

  let timeout = Duration::from_millis(timeout_ms);
  let mut timed_out = false;
  let status = loop {
    if let Some(status) = child.try_wait().map_err(io_to_tool_error)? {
      break status;
    }
    if started.elapsed() >= timeout {
      timed_out = true;
      let _ = child.kill();
      break child.wait().map_err(io_to_tool_error)?;
    }
    thread::sleep(Duration::from_millis(10));
  };

  let stdout = stdout_reader
    .join()
    .map_err(|_| ToolError::ExecutionFailed("stdout reader panicked".to_string()))?;
  let stderr = stderr_reader
    .join()
    .map_err(|_| ToolError::ExecutionFailed("stderr reader panicked".to_string()))?;

  Ok(json!({
    "success": status.success() && !timed_out,
    "exit_code": status.code(),
    "timed_out": timed_out,
    "duration_ms": started.elapsed().as_millis() as u64,
    "stdout": stdout,
    "stderr": stderr
  }))
}

fn read_stream_limited(mut reader: impl Read, max_bytes: usize) -> String {
  let mut stored = Vec::new();
  let mut buffer = [0_u8; 8192];

  loop {
    let Ok(read) = reader.read(&mut buffer) else {
      break;
    };
    if read == 0 {
      break;
    }
    if stored.len() < max_bytes {
      let remaining = max_bytes - stored.len();
      stored.extend_from_slice(&buffer[..read.min(remaining)]);
    }
  }

  String::from_utf8_lossy(&stored).to_string()
}

fn run_sleep(input: SleepInput) -> Result<Value, ToolError> {
  let duration_ms = input
    .duration_ms
    .or_else(|| input.seconds.map(|seconds| (seconds * 1000.0).round() as u64))
    .ok_or_else(|| ToolError::InvalidInput("duration_ms or seconds is required".to_string()))?
    .max(1);
  let started = Instant::now();
  thread::sleep(Duration::from_millis(duration_ms));

  Ok(json!({
    "requested_ms": duration_ms,
    "slept_ms": started.elapsed().as_millis() as u64
  }))
}

fn run_skill(input: SkillInput) -> Result<Value, ToolError> {
  if input.skill.trim().is_empty() {
    return Err(ToolError::InvalidInput("skill is required".to_string()));
  }

  let config = skills::resolve_from_env();
  let registry = skills::SkillRegistry::discover(&config)
    .map_err(|error| ToolError::ExecutionFailed(error.to_string()))?;
  let loaded = registry
    .load(&input.skill, input.args, &config)
    .map_err(|error| ToolError::ExecutionFailed(error.to_string()))?;
  Ok(json!(loaded))
}

fn run_send_message(input: SendMessageInput) -> Result<Value, ToolError> {
  let config = contact::resolve_from_env();
  let result = contact::send_message(
    &input.from_yuanling_id,
    &input.to_yuanling_id,
    &input.content,
    &config,
  )
  .map_err(|error| ToolError::ExecutionFailed(error.to_string()))?;
  Ok(json!(result))
}

fn run_mcp_tool(input: McpToolInput) -> Result<Value, ToolError> {
  if input.server.trim().is_empty() {
    return Err(ToolError::InvalidInput("server is required".to_string()));
  }
  if input.tool.trim().is_empty() {
    return Err(ToolError::InvalidInput("tool is required".to_string()));
  }

  let config = mcp::resolve_from_env();
  if !config.enabled {
    return Err(ToolError::ExecutionFailed("MCP module is disabled".to_string()));
  }
  let qualified_tool = mcp::mcp_tool_name(&input.server, &input.tool);
  let call_tool_name = qualified_tool.clone();
  let arguments = input.arguments.unwrap_or_else(|| json!({}));
  let runtime = build_mcp_runtime()?;
  let result = runtime
    .block_on(async move {
      let mut manager = mcp::McpServerManager::from_config(&config);
      let result = manager
        .call_tool(&call_tool_name, Some(arguments))
        .await
        .map(|result| serde_json::to_value(result).unwrap_or_else(|_| json!({})));
      let shutdown = manager.shutdown().await;
      match (result, shutdown) {
        (Ok(result), Ok(())) => Ok(result),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
      }
    })
    .map_err(|error: mcp::McpError| ToolError::ExecutionFailed(error.to_string()))?;

  Ok(json!({
    "server": input.server,
    "tool": input.tool,
    "qualified_tool": qualified_tool,
    "result": result,
    "status": "success"
  }))
}

fn run_list_mcp_resources(input: ListMcpResourcesInput) -> Result<Value, ToolError> {
  let config = mcp::resolve_from_env();
  if !config.enabled {
    return Err(ToolError::ExecutionFailed("MCP module is disabled".to_string()));
  }
  let server_filter = input.server.filter(|value| !value.trim().is_empty());
  let runtime = build_mcp_runtime()?;
  runtime
    .block_on(async move {
      let mut manager = mcp::McpServerManager::from_config(&config);
      let server_names = if let Some(server) = server_filter {
        vec![server]
      } else {
        manager.server_names()
      };
      let mut resources = Vec::new();
      let mut errors = Vec::new();
      for server in server_names {
        match manager.list_resources(&server).await {
          Ok(result) => resources.push(json!({
            "server": server,
            "resources": result.resources
          })),
          Err(error) => errors.push(json!({
            "server": server,
            "error": error.to_string()
          })),
        }
      }
      let status = if errors.is_empty() { "success" } else { "partial" };
      let output = json!({
        "resources": resources,
        "errors": errors,
        "status": status
      });
      manager.shutdown().await?;
      Ok(output)
    })
    .map_err(|error: mcp::McpError| ToolError::ExecutionFailed(error.to_string()))
}

fn run_read_mcp_resource(input: ReadMcpResourceInput) -> Result<Value, ToolError> {
  if input.uri.trim().is_empty() {
    return Err(ToolError::InvalidInput("uri is required".to_string()));
  }
  let config = mcp::resolve_from_env();
  if !config.enabled {
    return Err(ToolError::ExecutionFailed("MCP module is disabled".to_string()));
  }
  let uri = input.uri;
  let server = input.server.filter(|value| !value.trim().is_empty());
  let runtime = build_mcp_runtime()?;
  runtime
    .block_on(async move {
      let mut manager = mcp::McpServerManager::from_config(&config);
      let server_name = if let Some(server) = server {
        server
      } else {
        manager
          .server_names()
          .into_iter()
          .next()
          .ok_or_else(|| mcp::McpError::UnknownServer("no configured MCP servers".to_string()))?
      };
      let contents = manager.read_resource(&server_name, &uri).await;
      let shutdown = manager.shutdown().await;
      match (contents, shutdown) {
        (Ok(contents), Ok(())) => Ok(json!({
          "server": server_name,
          "uri": uri,
          "contents": contents.contents,
          "status": "success"
        })),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
      }
    })
    .map_err(|error: mcp::McpError| ToolError::ExecutionFailed(error.to_string()))
}

fn run_mcp_auth(input: McpAuthInput) -> Result<Value, ToolError> {
  if input.server.trim().is_empty() {
    return Err(ToolError::InvalidInput("server is required".to_string()));
  }
  Ok(json!({
    "server": input.server,
    "status": "not_required",
    "message": "MCP auth is reserved for future remote transports; stdio MCP servers do not require OAuth in Yuanling v1."
  }))
}

fn build_mcp_runtime() -> Result<tokio::runtime::Runtime, ToolError> {
  tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .map_err(|error| ToolError::ExecutionFailed(format!("failed to build MCP runtime: {error}")))
}

fn validate_http_url(url: &str) -> Result<(), ToolError> {
  if url.starts_with("https://") || url.starts_with("http://") {
    Ok(())
  } else {
    Err(ToolError::InvalidInput(
      "url must start with http:// or https://".to_string(),
    ))
  }
}

fn truncate_chars(value: &str, max_chars: usize) -> (String, bool) {
  let mut output = String::new();
  for (index, ch) in value.chars().enumerate() {
    if index >= max_chars {
      return (output, true);
    }
    output.push(ch);
  }
  (output, false)
}

fn html_to_text(html: &str) -> String {
  let without_scripts = Regex::new(r"(?is)<(script|style)[^>]*>.*?</(script|style)>")
    .map(|regex| regex.replace_all(html, " ").to_string())
    .unwrap_or_else(|_| html.to_string());
  let without_tags = Regex::new(r"(?is)<[^>]+>")
    .map(|regex| regex.replace_all(&without_scripts, " ").to_string())
    .unwrap_or(without_scripts);
  decode_html_entities(&without_tags)
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
}

fn parse_duckduckgo_results(body: &str, max_results: usize) -> Vec<Value> {
  let Ok(regex) = Regex::new(r#"(?is)<a[^>]*class="result__a"[^>]*href="([^"]+)"[^>]*>(.*?)</a>"#)
  else {
    return Vec::new();
  };

  regex
    .captures_iter(body)
    .take(max_results)
    .map(|capture| {
      let url = clean_duckduckgo_url(capture.get(1).map(|value| value.as_str()).unwrap_or(""));
      let title = html_to_text(capture.get(2).map(|value| value.as_str()).unwrap_or(""));
      json!({
        "title": title,
        "url": url,
        "citation": url
      })
    })
    .collect()
}

fn clean_duckduckgo_url(raw: &str) -> String {
  let decoded = decode_html_entities(raw);
  if let Some(index) = decoded.find("uddg=") {
    let rest = &decoded[index + 5..];
    let encoded = rest.split('&').next().unwrap_or(rest);
    return percent_decode(encoded);
  }
  decoded
}

fn decode_html_entities(value: &str) -> String {
  value
    .replace("&amp;", "&")
    .replace("&lt;", "<")
    .replace("&gt;", ">")
    .replace("&quot;", "\"")
    .replace("&#39;", "'")
    .replace("&nbsp;", " ")
}

fn percent_encode(value: &str) -> String {
  let mut output = String::new();
  for byte in value.bytes() {
    match byte {
      b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
        output.push(byte as char)
      }
      b' ' => output.push('+'),
      _ => output.push_str(&format!("%{byte:02X}")),
    }
  }
  output
}

fn percent_decode(value: &str) -> String {
  let bytes = value.as_bytes();
  let mut output = Vec::new();
  let mut index = 0;
  while index < bytes.len() {
    if bytes[index] == b'%' && index + 2 < bytes.len() {
      if let Ok(hex) = u8::from_str_radix(&value[index + 1..index + 3], 16) {
        output.push(hex);
        index += 3;
        continue;
      }
    }
    output.push(if bytes[index] == b'+' { b' ' } else { bytes[index] });
    index += 1;
  }
  String::from_utf8_lossy(&output).to_string()
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
  let pattern = pattern.as_bytes();
  let text = text.as_bytes();
  let mut dp = vec![vec![false; text.len() + 1]; pattern.len() + 1];
  dp[0][0] = true;

  for pattern_index in 1..=pattern.len() {
    if pattern[pattern_index - 1] == b'*' {
      dp[pattern_index][0] = dp[pattern_index - 1][0];
    }
  }

  for pattern_index in 1..=pattern.len() {
    for text_index in 1..=text.len() {
      dp[pattern_index][text_index] = match pattern[pattern_index - 1] {
        b'*' => dp[pattern_index - 1][text_index] || dp[pattern_index][text_index - 1],
        b'?' => dp[pattern_index - 1][text_index - 1],
        byte => byte == text[text_index - 1] && dp[pattern_index - 1][text_index - 1],
      };
    }
  }

  dp[pattern.len()][text.len()]
}

fn find_program(candidates: &[&str]) -> Option<String> {
  let path = env::var_os("PATH")?;
  for dir in env::split_paths(&path) {
    for candidate in candidates {
      let full_path = dir.join(candidate);
      if full_path.is_file() {
        return Some(full_path.to_string_lossy().to_string());
      }
    }
  }
  None
}

fn score_tool_match(spec: &ToolSpec, normalized_query: &str) -> u32 {
  if normalized_query.is_empty() {
    return 1;
  }

  let normalized_name = normalize_tool_name(&spec.name);
  let normalized_description = spec
    .description
    .as_deref()
    .map(normalize_tool_name)
    .unwrap_or_default();
  let normalized_source = normalize_tool_name(&format!("{:?}", spec.source));

  if normalized_name == normalized_query {
    100
  } else if normalized_name.contains(normalized_query) {
    80
  } else if normalized_description.contains(normalized_query) {
    50
  } else if normalized_source.contains(normalized_query) {
    25
  } else {
    0
  }
}

fn normalize_tool_name(name: &str) -> String {
  name
    .trim()
    .to_ascii_lowercase()
    .replace('-', "_")
    .replace(' ', "_")
}

fn alias_map() -> BTreeMap<&'static str, String> {
  [
    ("read", "read_file"),
    ("write", "write_file"),
    ("edit", "edit_file"),
    ("glob", "glob_search"),
    ("grep", "grep_search"),
  ]
  .into_iter()
  .map(|(alias, canonical)| (alias, canonical.to_string()))
  .collect()
}

fn env_or_optional(key: &str) -> Option<String> {
  env::var(key).ok().filter(|value| !value.trim().is_empty())
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

