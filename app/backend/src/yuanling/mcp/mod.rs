use super::tools::{
  RuntimeToolDefinition, ToolError, ToolExecutionOutput, ToolExecutor, ToolPermissionMode,
};
use axum::{
  extract::Path as AxumPath,
  routing::{get, post, put},
  Json, Router,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;

pub const MCP_PROTOCOL_VERSION: &str = "2025-03-26";
const DEFAULT_INITIALIZE_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_LIST_TOOLS_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_TOOL_CALL_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_RESOURCE_TIMEOUT_MS: u64 = 30_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcId {
  Number(u64),
  String(String),
  Null,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcRequest<T = Value> {
  pub jsonrpc: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub id: Option<JsonRpcId>,
  pub method: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub params: Option<T>,
}

impl<T> JsonRpcRequest<T> {
  pub fn new(id: JsonRpcId, method: impl Into<String>, params: Option<T>) -> Self {
    Self {
      jsonrpc: "2.0".to_string(),
      id: Some(id),
      method: method.into(),
      params,
    }
  }

  pub fn notification(method: impl Into<String>, params: Option<T>) -> Self {
    Self {
      jsonrpc: "2.0".to_string(),
      id: None,
      method: method.into(),
      params,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcError {
  pub code: i64,
  pub message: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub data: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcResponse<T = Value> {
  pub jsonrpc: String,
  #[serde(default)]
  pub id: Option<JsonRpcId>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub result: Option<T>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInitializeParams {
  pub protocol_version: String,
  pub capabilities: Value,
  pub client_info: McpInitializeClientInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInitializeClientInfo {
  pub name: String,
  pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInitializeResult {
  pub protocol_version: String,
  pub capabilities: Value,
  pub server_info: McpInitializeServerInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInitializeServerInfo {
  pub name: String,
  pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpTool {
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub description: Option<String>,
  #[serde(rename = "inputSchema", skip_serializing_if = "Option::is_none")]
  pub input_schema: Option<Value>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub annotations: Option<Value>,
  #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
  pub meta: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpListToolsResult {
  pub tools: Vec<McpTool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolCallParams {
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub arguments: Option<Value>,
  #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
  pub meta: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolCallContent {
  #[serde(rename = "type")]
  pub kind: String,
  #[serde(flatten)]
  pub data: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolCallResult {
  #[serde(default)]
  pub content: Vec<McpToolCallContent>,
  #[serde(default)]
  pub structured_content: Option<Value>,
  #[serde(default)]
  pub is_error: Option<bool>,
  #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
  pub meta: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpResource {
  pub uri: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub description: Option<String>,
  #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
  pub mime_type: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub annotations: Option<Value>,
  #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
  pub meta: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpListResourcesResult {
  pub resources: Vec<McpResource>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpResourceContents {
  pub uri: String,
  #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
  pub mime_type: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub text: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub blob: Option<String>,
  #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
  pub meta: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpReadResourceResult {
  pub contents: Vec<McpResourceContents>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpServerConfig {
  Stdio {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    tool_call_timeout_ms: Option<u64>,
  },
  Http {
    url: String,
    #[serde(default)]
    headers: BTreeMap<String, String>,
  },
  Sse {
    url: String,
    #[serde(default)]
    headers: BTreeMap<String, String>,
  },
  Ws {
    url: String,
    #[serde(default)]
    headers: BTreeMap<String, String>,
  },
}

impl McpServerConfig {
  pub fn transport(&self) -> McpTransportKind {
    match self {
      Self::Stdio { .. } => McpTransportKind::Stdio,
      Self::Http { .. } => McpTransportKind::Http,
      Self::Sse { .. } => McpTransportKind::Sse,
      Self::Ws { .. } => McpTransportKind::Ws,
    }
  }

  fn tool_call_timeout_ms(&self, default: u64) -> u64 {
    match self {
      Self::Stdio {
        tool_call_timeout_ms,
        ..
      } => tool_call_timeout_ms.unwrap_or(default),
      Self::Http { .. } | Self::Sse { .. } | Self::Ws { .. } => default,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTransportKind {
  Stdio,
  Http,
  Sse,
  Ws,
}

impl McpTransportKind {
  pub fn label(self) -> &'static str {
    match self {
      Self::Stdio => "stdio",
      Self::Http => "http",
      Self::Sse => "sse",
      Self::Ws => "ws",
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpModuleConfig {
  pub enabled: bool,
  pub servers: BTreeMap<String, McpServerConfig>,
  pub initialize_timeout_ms: u64,
  pub list_tools_timeout_ms: u64,
  pub default_tool_call_timeout_ms: u64,
  pub resource_timeout_ms: u64,
}

impl McpModuleConfig {
  pub fn as_view(&self) -> McpModuleConfigView {
    let preflight = preflight_config(self);
    let stdio_count = self
      .servers
      .values()
      .filter(|config| config.transport() == McpTransportKind::Stdio)
      .count();
    let unsupported_count = self.servers.len().saturating_sub(stdio_count);

    McpModuleConfigView {
      enabled: self.enabled,
      configured_count: self.servers.len(),
      stdio_count,
      unsupported_count,
      initialize_timeout_ms: self.initialize_timeout_ms,
      list_tools_timeout_ms: self.list_tools_timeout_ms,
      default_tool_call_timeout_ms: self.default_tool_call_timeout_ms,
      resource_timeout_ms: self.resource_timeout_ms,
      degraded: preflight.degraded,
      servers: preflight.servers,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpModuleConfigView {
  pub enabled: bool,
  pub configured_count: usize,
  pub stdio_count: usize,
  pub unsupported_count: usize,
  pub initialize_timeout_ms: u64,
  pub list_tools_timeout_ms: u64,
  pub default_tool_call_timeout_ms: u64,
  pub resource_timeout_ms: u64,
  pub degraded: bool,
  pub servers: Vec<McpServerStatusView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerAdminView {
  pub name: String,
  pub config: McpServerConfig,
  pub status: McpServerStatus,
  pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct McpStoredConfig {
  version: u32,
  servers: BTreeMap<String, McpServerConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpServerUpsertRequest {
  pub name: String,
  pub config: McpServerConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpReadResourceRequest {
  pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerStatusView {
  pub name: String,
  pub transport: McpTransportKind,
  pub status: McpServerStatus,
  pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpServerStatus {
  Configured,
  Resolved,
  CommandNotFound,
  UnsupportedTransport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpPreflightReport {
  pub degraded: bool,
  pub servers: Vec<McpServerStatusView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManagedMcpTool {
  pub server_name: String,
  pub raw_name: String,
  pub qualified_name: String,
  pub tool: McpTool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpDiscoveryFailure {
  pub server_name: String,
  pub error: String,
  pub recoverable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnsupportedMcpServer {
  pub server_name: String,
  pub transport: McpTransportKind,
  pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolDiscoveryReport {
  pub tools: Vec<ManagedMcpTool>,
  pub failed_servers: Vec<McpDiscoveryFailure>,
  pub unsupported_servers: Vec<UnsupportedMcpServer>,
  pub degraded: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum McpError {
  Config(String),
  Io(String),
  Transport {
    server_name: String,
    method: &'static str,
    message: String,
  },
  JsonRpc {
    server_name: String,
    method: &'static str,
    code: i64,
    message: String,
  },
  InvalidResponse {
    server_name: String,
    method: &'static str,
    details: String,
  },
  Timeout {
    server_name: String,
    method: &'static str,
    timeout_ms: u64,
  },
  UnknownServer(String),
  UnknownTool(String),
  UnsupportedTransport {
    server_name: String,
    transport: McpTransportKind,
  },
}

impl Display for McpError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Config(message) => write!(f, "MCP config error: {message}"),
      Self::Io(message) => write!(f, "MCP I/O error: {message}"),
      Self::Transport {
        server_name,
        method,
        message,
      } => write!(
        f,
        "MCP server `{server_name}` transport failed during {method}: {message}"
      ),
      Self::JsonRpc {
        server_name,
        method,
        code,
        message,
      } => write!(
        f,
        "MCP server `{server_name}` returned JSON-RPC error for {method}: {message} ({code})"
      ),
      Self::InvalidResponse {
        server_name,
        method,
        details,
      } => write!(
        f,
        "MCP server `{server_name}` returned invalid response for {method}: {details}"
      ),
      Self::Timeout {
        server_name,
        method,
        timeout_ms,
      } => write!(
        f,
        "MCP server `{server_name}` timed out after {timeout_ms} ms during {method}"
      ),
      Self::UnknownServer(server_name) => write!(f, "unknown MCP server `{server_name}`"),
      Self::UnknownTool(tool_name) => write!(f, "unknown MCP tool `{tool_name}`"),
      Self::UnsupportedTransport {
        server_name,
        transport,
      } => write!(
        f,
        "MCP server `{server_name}` uses unsupported transport `{}`",
        transport.label()
      ),
    }
  }
}

impl std::error::Error for McpError {}

impl From<std::io::Error> for McpError {
  fn from(value: std::io::Error) -> Self {
    Self::Io(value.to_string())
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolRoute {
  server_name: String,
  raw_name: String,
}

#[derive(Debug)]
struct ManagedMcpServer {
  config: McpServerConfig,
  process: Option<McpStdioProcess>,
  initialized: bool,
  server_info: Option<McpInitializeServerInfo>,
}

impl ManagedMcpServer {
  fn new(config: McpServerConfig) -> Self {
    Self {
      config,
      process: None,
      initialized: false,
      server_info: None,
    }
  }
}

#[derive(Debug)]
pub struct McpServerManager {
  servers: BTreeMap<String, ManagedMcpServer>,
  unsupported_servers: Vec<UnsupportedMcpServer>,
  tool_index: BTreeMap<String, ToolRoute>,
  initialize_timeout_ms: u64,
  list_tools_timeout_ms: u64,
  default_tool_call_timeout_ms: u64,
  resource_timeout_ms: u64,
  next_request_id: u64,
}

impl McpServerManager {
  pub fn from_config(config: &McpModuleConfig) -> Self {
    let mut servers = BTreeMap::new();
    let mut unsupported_servers = Vec::new();

    if config.enabled {
      for (name, server_config) in &config.servers {
        if server_config.transport() == McpTransportKind::Stdio {
          servers.insert(name.clone(), ManagedMcpServer::new(server_config.clone()));
        } else {
          unsupported_servers.push(UnsupportedMcpServer {
            server_name: name.clone(),
            transport: server_config.transport(),
            reason: "only stdio MCP transport is executable in Yuanling v1".to_string(),
          });
        }
      }
    }

    Self {
      servers,
      unsupported_servers,
      tool_index: BTreeMap::new(),
      initialize_timeout_ms: config.initialize_timeout_ms,
      list_tools_timeout_ms: config.list_tools_timeout_ms,
      default_tool_call_timeout_ms: config.default_tool_call_timeout_ms,
      resource_timeout_ms: config.resource_timeout_ms,
      next_request_id: 1,
    }
  }

  pub fn server_names(&self) -> Vec<String> {
    self.servers.keys().cloned().collect()
  }

  pub fn unsupported_servers(&self) -> &[UnsupportedMcpServer] {
    &self.unsupported_servers
  }

  pub async fn discover_tools(&mut self) -> Result<Vec<ManagedMcpTool>, McpError> {
    let mut tools = Vec::new();
    for server_name in self.server_names() {
      let server_tools = self.discover_tools_for_server(&server_name).await?;
      self.clear_routes_for_server(&server_name);
      for tool in server_tools {
        self.tool_index.insert(
          tool.qualified_name.clone(),
          ToolRoute {
            server_name: tool.server_name.clone(),
            raw_name: tool.raw_name.clone(),
          },
        );
        tools.push(tool);
      }
    }
    Ok(tools)
  }

  pub async fn discover_tools_best_effort(&mut self) -> McpToolDiscoveryReport {
    let mut tools = Vec::new();
    let mut failed_servers = Vec::new();

    for server_name in self.server_names() {
      match self.discover_tools_for_server(&server_name).await {
        Ok(server_tools) => {
          self.clear_routes_for_server(&server_name);
          for tool in server_tools {
            self.tool_index.insert(
              tool.qualified_name.clone(),
              ToolRoute {
                server_name: tool.server_name.clone(),
                raw_name: tool.raw_name.clone(),
              },
            );
            tools.push(tool);
          }
        }
        Err(error) => {
          self.clear_routes_for_server(&server_name);
          failed_servers.push(McpDiscoveryFailure {
            server_name,
            error: error.to_string(),
            recoverable: matches!(error, McpError::Timeout { .. } | McpError::Transport { .. }),
          });
        }
      }
    }

    let degraded = !failed_servers.is_empty() || !self.unsupported_servers.is_empty();
    McpToolDiscoveryReport {
      tools,
      failed_servers,
      unsupported_servers: self.unsupported_servers.clone(),
      degraded,
    }
  }

  pub async fn call_tool(
    &mut self,
    qualified_tool_name: &str,
    arguments: Option<Value>,
  ) -> Result<McpToolCallResult, McpError> {
    if !self.tool_index.contains_key(qualified_tool_name) {
      let _ = self.discover_tools().await?;
    }
    let route = self
      .tool_index
      .get(qualified_tool_name)
      .cloned()
      .ok_or_else(|| McpError::UnknownTool(qualified_tool_name.to_string()))?;
    self.ensure_server_ready(&route.server_name).await?;
    let timeout_ms = self.tool_call_timeout_ms(&route.server_name)?;
    let id = self.take_request_id();
    let response = {
      let process = self.process_mut(&route.server_name, "tools/call")?;
      run_with_timeout(
        &route.server_name,
        "tools/call",
        timeout_ms,
        process.call_tool(id.clone(), route.raw_name, arguments),
      )
      .await?
    };
    expect_response(&route.server_name, "tools/call", id, response)
  }

  pub async fn list_resources(
    &mut self,
    server_name: &str,
  ) -> Result<McpListResourcesResult, McpError> {
    self.ensure_server_ready(server_name).await?;
    let id = self.take_request_id();
    let timeout_ms = self.resource_timeout_ms;
    let response = {
      let process = self.process_mut(server_name, "resources/list")?;
      run_with_timeout(
        server_name,
        "resources/list",
        timeout_ms,
        process.list_resources(id.clone()),
      )
      .await?
    };
    expect_response(server_name, "resources/list", id, response)
  }

  pub async fn read_resource(
    &mut self,
    server_name: &str,
    uri: &str,
  ) -> Result<McpReadResourceResult, McpError> {
    self.ensure_server_ready(server_name).await?;
    let id = self.take_request_id();
    let timeout_ms = self.resource_timeout_ms;
    let response = {
      let process = self.process_mut(server_name, "resources/read")?;
      run_with_timeout(
        server_name,
        "resources/read",
        timeout_ms,
        process.read_resource(id.clone(), uri.to_string()),
      )
      .await?
    };
    expect_response(server_name, "resources/read", id, response)
  }

  pub async fn shutdown(&mut self) -> Result<(), McpError> {
    for server in self.servers.values_mut() {
      if let Some(mut process) = server.process.take() {
        process.terminate().await?;
      }
      server.initialized = false;
    }
    Ok(())
  }

  async fn discover_tools_for_server(
    &mut self,
    server_name: &str,
  ) -> Result<Vec<ManagedMcpTool>, McpError> {
    self.ensure_server_ready(server_name).await?;
    let id = self.take_request_id();
    let timeout_ms = self.list_tools_timeout_ms;
    let response = {
      let process = self.process_mut(server_name, "tools/list")?;
      run_with_timeout(
        server_name,
        "tools/list",
        timeout_ms,
        process.list_tools(id.clone()),
      )
      .await?
    };
    let result = expect_response(server_name, "tools/list", id, response)?;

    Ok(result
      .tools
      .into_iter()
      .map(|tool| ManagedMcpTool {
        qualified_name: mcp_tool_name(server_name, &tool.name),
        raw_name: tool.name.clone(),
        server_name: server_name.to_string(),
        tool,
      })
      .collect())
  }

  async fn ensure_server_ready(&mut self, server_name: &str) -> Result<(), McpError> {
    if self
      .servers
      .get(server_name)
      .ok_or_else(|| McpError::UnknownServer(server_name.to_string()))?
      .initialized
    {
      return Ok(());
    }

    let config = self
      .servers
      .get(server_name)
      .ok_or_else(|| McpError::UnknownServer(server_name.to_string()))?
      .config
      .clone();
    let mut process = McpStdioProcess::spawn(server_name, &config).await?;
    let id = self.take_request_id();
    let response = run_with_timeout(
      server_name,
      "initialize",
      self.initialize_timeout_ms,
      process.initialize(id.clone()),
    )
    .await?;
    let initialized = expect_response(server_name, "initialize", id, response)?;

    let server = self
      .servers
      .get_mut(server_name)
      .ok_or_else(|| McpError::UnknownServer(server_name.to_string()))?;
    server.server_info = Some(initialized.server_info);
    server.initialized = true;
    server.process = Some(process);
    Ok(())
  }

  fn process_mut(
    &mut self,
    server_name: &str,
    method: &'static str,
  ) -> Result<&mut McpStdioProcess, McpError> {
    self
      .servers
      .get_mut(server_name)
      .ok_or_else(|| McpError::UnknownServer(server_name.to_string()))?
      .process
      .as_mut()
      .ok_or_else(|| McpError::InvalidResponse {
        server_name: server_name.to_string(),
        method,
        details: "server process missing after initialization".to_string(),
      })
  }

  fn tool_call_timeout_ms(&self, server_name: &str) -> Result<u64, McpError> {
    Ok(
      self
        .servers
        .get(server_name)
        .ok_or_else(|| McpError::UnknownServer(server_name.to_string()))?
        .config
        .tool_call_timeout_ms(self.default_tool_call_timeout_ms),
    )
  }

  fn clear_routes_for_server(&mut self, server_name: &str) {
    self
      .tool_index
      .retain(|_, route| route.server_name != server_name);
  }

  fn take_request_id(&mut self) -> JsonRpcId {
    let id = self.next_request_id;
    self.next_request_id = self.next_request_id.saturating_add(1);
    JsonRpcId::Number(id)
  }
}

#[derive(Debug)]
struct McpStdioProcess {
  child: Child,
  stdin: ChildStdin,
  stdout: BufReader<ChildStdout>,
}

impl McpStdioProcess {
  async fn spawn(server_name: &str, config: &McpServerConfig) -> Result<Self, McpError> {
    let McpServerConfig::Stdio {
      command,
      args,
      env,
      ..
    } = config
    else {
      return Err(McpError::UnsupportedTransport {
        server_name: server_name.to_string(),
        transport: config.transport(),
      });
    };

    let mut command_builder = Command::new(command);
    command_builder
      .args(args)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::null());
    for (key, value) in env {
      command_builder.env(key, value);
    }

    let mut child = command_builder
      .spawn()
      .map_err(|error| McpError::Transport {
        server_name: server_name.to_string(),
        method: "spawn",
        message: error.to_string(),
      })?;
    let stdin = child.stdin.take().ok_or_else(|| McpError::InvalidResponse {
      server_name: server_name.to_string(),
      method: "spawn",
      details: "failed to capture child stdin".to_string(),
    })?;
    let stdout = child.stdout.take().ok_or_else(|| McpError::InvalidResponse {
      server_name: server_name.to_string(),
      method: "spawn",
      details: "failed to capture child stdout".to_string(),
    })?;

    Ok(Self {
      child,
      stdin,
      stdout: BufReader::new(stdout),
    })
  }

  async fn initialize(
    &mut self,
    id: JsonRpcId,
  ) -> Result<JsonRpcResponse<McpInitializeResult>, McpError> {
    self
      .request(
        id,
        "initialize",
        Some(json!({
          "protocolVersion": MCP_PROTOCOL_VERSION,
          "capabilities": {},
          "clientInfo": {"name": "taichu-yuanling", "version": "0.1.0"}
        })),
      )
      .await
  }

  async fn list_tools(
    &mut self,
    id: JsonRpcId,
  ) -> Result<JsonRpcResponse<McpListToolsResult>, McpError> {
    self.request(id, "tools/list", Some(json!({}))).await
  }

  async fn call_tool(
    &mut self,
    id: JsonRpcId,
    name: String,
    arguments: Option<Value>,
  ) -> Result<JsonRpcResponse<McpToolCallResult>, McpError> {
    self
      .request(
        id,
        "tools/call",
        Some(json!({
          "name": name,
          "arguments": arguments.unwrap_or_else(|| json!({}))
        })),
      )
      .await
  }

  async fn list_resources(
    &mut self,
    id: JsonRpcId,
  ) -> Result<JsonRpcResponse<McpListResourcesResult>, McpError> {
    self.request(id, "resources/list", Some(json!({}))).await
  }

  async fn read_resource(
    &mut self,
    id: JsonRpcId,
    uri: String,
  ) -> Result<JsonRpcResponse<McpReadResourceResult>, McpError> {
    self
      .request(id, "resources/read", Some(json!({"uri": uri})))
      .await
  }

  async fn request<T: DeserializeOwned>(
    &mut self,
    id: JsonRpcId,
    method: &'static str,
    params: Option<Value>,
  ) -> Result<JsonRpcResponse<T>, McpError> {
    let request = JsonRpcRequest::new(id, method, params);
    let payload = serde_json::to_vec(&request).map_err(|error| McpError::InvalidResponse {
      server_name: "local".to_string(),
      method,
      details: error.to_string(),
    })?;
    self.stdin.write_all(&encode_frame(&payload)).await?;
    self.stdin.flush().await?;
    let response_payload = read_frame(&mut self.stdout).await?;
    serde_json::from_slice(&response_payload).map_err(|error| McpError::InvalidResponse {
      server_name: "local".to_string(),
      method,
      details: error.to_string(),
    })
  }

  async fn terminate(&mut self) -> Result<(), McpError> {
    if self.child.try_wait()?.is_none() {
      let _ = self.child.kill().await;
    }
    let _ = self.child.wait().await;
    Ok(())
  }
}

#[derive(Debug, Clone)]
pub struct McpToolExecutor {
  config: McpModuleConfig,
}

impl McpToolExecutor {
  pub fn new(config: McpModuleConfig) -> Self {
    Self { config }
  }
}

impl ToolExecutor for McpToolExecutor {
  fn execute(
    &mut self,
    tool_name: &str,
    input: &Value,
  ) -> Result<ToolExecutionOutput, ToolError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .map_err(|error| ToolError::ExecutionFailed(format!("failed to build MCP runtime: {error}")))?;
    let config = self.config.clone();
    let tool_name = tool_name.to_string();
    let call_tool_name = tool_name.clone();
    let input = input.clone();
    let output = runtime
      .block_on(async move {
        let mut manager = McpServerManager::from_config(&config);
        let result = manager.call_tool(&call_tool_name, Some(input)).await;
        let shutdown = manager.shutdown().await;
        match (result, shutdown) {
          (Ok(result), Ok(())) => Ok(serde_json::to_value(result).unwrap_or_else(|_| json!({}))),
          (Err(error), _) => Err(error),
          (Ok(_), Err(error)) => Err(error),
        }
      })
      .map_err(|error: McpError| ToolError::ExecutionFailed(error.to_string()))?;

    Ok(ToolExecutionOutput {
      tool_name,
      output,
      permission_outcome: None,
    })
  }
}

pub fn resolve_from_env() -> McpModuleConfig {
  let mut servers = load_mcp_servers_from_storage().unwrap_or_default();
  let env_servers = env_or_optional("YUANLING_MCP_SERVERS_JSON")
    .and_then(|raw| parse_servers_json(&raw).ok())
    .unwrap_or_default();
  servers.extend(env_servers);

  McpModuleConfig {
    enabled: env_or_bool("YUANLING_MCP_ENABLED", true),
    servers,
    initialize_timeout_ms: env_or_u64(
      "YUANLING_MCP_INITIALIZE_TIMEOUT_MS",
      DEFAULT_INITIALIZE_TIMEOUT_MS,
    ),
    list_tools_timeout_ms: env_or_u64(
      "YUANLING_MCP_LIST_TOOLS_TIMEOUT_MS",
      DEFAULT_LIST_TOOLS_TIMEOUT_MS,
    ),
    default_tool_call_timeout_ms: env_or_u64(
      "YUANLING_MCP_TOOL_CALL_TIMEOUT_MS",
      DEFAULT_TOOL_CALL_TIMEOUT_MS,
    ),
    resource_timeout_ms: env_or_u64("YUANLING_MCP_RESOURCE_TIMEOUT_MS", DEFAULT_RESOURCE_TIMEOUT_MS),
  }
}

pub fn mcp_storage_dir() -> PathBuf {
  if let Some(path) = env_or_optional("YUANLING_MCP_CONFIG_STORAGE_DIR") {
    return PathBuf::from(path);
  }
  env_or_optional("BACKEND_DATA_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from("./data"))
    .join("yuanling")
    .join("mcp")
}

fn mcp_storage_file() -> PathBuf {
  mcp_storage_dir().join("servers.json")
}

pub fn load_mcp_servers_from_storage() -> Result<BTreeMap<String, McpServerConfig>, McpError> {
  let path = mcp_storage_file();
  if !path.is_file() {
    return Ok(BTreeMap::new());
  }
  let contents = fs::read_to_string(&path).map_err(|error| McpError::Io(error.to_string()))?;
  let stored: McpStoredConfig =
    serde_json::from_str(&contents).map_err(|error| McpError::Config(error.to_string()))?;
  Ok(stored.servers)
}

pub fn save_mcp_servers_to_storage(
  servers: &BTreeMap<String, McpServerConfig>,
) -> Result<(), McpError> {
  let path = mcp_storage_file();
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).map_err(|error| McpError::Io(error.to_string()))?;
  }
  let stored = McpStoredConfig {
    version: 1,
    servers: servers.clone(),
  };
  let contents =
    serde_json::to_string_pretty(&stored).map_err(|error| McpError::Config(error.to_string()))?;
  fs::write(path, contents.as_bytes()).map_err(|error| McpError::Io(error.to_string()))
}

pub fn upsert_mcp_server(
  request: McpServerUpsertRequest,
) -> Result<McpServerAdminView, McpError> {
  let name = normalize_mcp_server_id(&request.name)?;
  let mut servers = load_mcp_servers_from_storage()?;
  servers.insert(name.clone(), request.config);
  save_mcp_servers_to_storage(&servers)?;
  list_mcp_server_admin_views()?
    .into_iter()
    .find(|server| server.name == name)
    .ok_or_else(|| McpError::Config(format!("MCP server `{name}` was not saved")))
}

pub fn delete_mcp_server(name: &str) -> Result<String, McpError> {
  let name = normalize_mcp_server_id(name)?;
  let mut servers = load_mcp_servers_from_storage()?;
  servers.remove(&name);
  save_mcp_servers_to_storage(&servers)?;
  Ok(name)
}

pub fn list_mcp_server_admin_views() -> Result<Vec<McpServerAdminView>, McpError> {
  let config = resolve_from_env();
  let statuses = preflight_config(&config)
    .servers
    .into_iter()
    .map(|server| (server.name.clone(), server))
    .collect::<BTreeMap<_, _>>();
  Ok(config
    .servers
    .into_iter()
    .map(|(name, server_config)| {
      let status = statuses.get(&name);
      McpServerAdminView {
        name,
        config: server_config,
        status: status
          .map(|server| server.status)
          .unwrap_or(McpServerStatus::Configured),
        detail: status.and_then(|server| server.detail.clone()),
      }
    })
    .collect())
}

fn normalize_mcp_server_id(value: &str) -> Result<String, McpError> {
  let normalized = value.trim();
  if normalized.is_empty() {
    return Err(McpError::Config("MCP server name is required".to_string()));
  }
  if normalized
    .chars()
    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
  {
    Ok(normalized.to_string())
  } else {
    Err(McpError::Config(
      "MCP server name may only contain letters, numbers, underscores, and hyphens".to_string(),
    ))
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
  pub success: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub data: Option<T>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub message: Option<String>,
}

impl<T> ApiResponse<T> {
  fn ok(data: T) -> Self {
    Self {
      success: true,
      data: Some(data),
      message: None,
    }
  }

  fn error(message: String) -> Self {
    Self {
      success: false,
      data: None,
      message: Some(message),
    }
  }
}

async fn config() -> Json<ApiResponse<McpModuleConfigView>> {
  Json(ApiResponse::ok(resolve_from_env().as_view()))
}

async fn list_servers() -> Json<ApiResponse<Vec<McpServerAdminView>>> {
  Json(match list_mcp_server_admin_views() {
    Ok(servers) => ApiResponse::ok(servers),
    Err(error) => ApiResponse::error(error.to_string()),
  })
}

async fn create_server(
  Json(request): Json<McpServerUpsertRequest>,
) -> Json<ApiResponse<McpServerAdminView>> {
  Json(match upsert_mcp_server(request) {
    Ok(server) => ApiResponse::ok(server),
    Err(error) => ApiResponse::error(error.to_string()),
  })
}

async fn update_server(
  AxumPath(name): AxumPath<String>,
  Json(mut request): Json<McpServerUpsertRequest>,
) -> Json<ApiResponse<McpServerAdminView>> {
  request.name = name;
  Json(match upsert_mcp_server(request) {
    Ok(server) => ApiResponse::ok(server),
    Err(error) => ApiResponse::error(error.to_string()),
  })
}

async fn remove_server(AxumPath(name): AxumPath<String>) -> Json<ApiResponse<String>> {
  Json(match delete_mcp_server(&name) {
    Ok(name) => ApiResponse::ok(name),
    Err(error) => ApiResponse::error(error.to_string()),
  })
}

async fn discover() -> Json<ApiResponse<McpToolDiscoveryReport>> {
  let config = resolve_from_env();
  let mut manager = McpServerManager::from_config(&config);
  let report = manager.discover_tools_best_effort().await;
  let _ = manager.shutdown().await;
  Json(ApiResponse::ok(report))
}

async fn list_server_resources(
  AxumPath(name): AxumPath<String>,
) -> Json<ApiResponse<McpListResourcesResult>> {
  let config = resolve_from_env();
  let mut manager = McpServerManager::from_config(&config);
  let result = manager.list_resources(&name).await;
  let _ = manager.shutdown().await;
  Json(match result {
    Ok(resources) => ApiResponse::ok(resources),
    Err(error) => ApiResponse::error(error.to_string()),
  })
}

async fn read_server_resource(
  AxumPath(name): AxumPath<String>,
  Json(request): Json<McpReadResourceRequest>,
) -> Json<ApiResponse<McpReadResourceResult>> {
  let config = resolve_from_env();
  let mut manager = McpServerManager::from_config(&config);
  let result = manager.read_resource(&name, &request.uri).await;
  let _ = manager.shutdown().await;
  Json(match result {
    Ok(resource) => ApiResponse::ok(resource),
    Err(error) => ApiResponse::error(error.to_string()),
  })
}

pub fn router() -> Router {
  Router::new()
    .route("/yuanling/mcp/config", get(config))
    .route("/yuanling/mcp/servers", get(list_servers).post(create_server))
    .route("/yuanling/mcp/servers/{name}", put(update_server).delete(remove_server))
    .route("/yuanling/mcp/servers/{name}/resources", get(list_server_resources))
    .route("/yuanling/mcp/servers/{name}/resources/read", post(read_server_resource))
    .route("/yuanling/mcp/discover", post(discover))
}

pub fn parse_servers_json(raw: &str) -> Result<BTreeMap<String, McpServerConfig>, McpError> {
  serde_json::from_str(raw).map_err(|error| McpError::Config(error.to_string()))
}

pub fn runtime_tool_definitions(tools: &[ManagedMcpTool]) -> Vec<RuntimeToolDefinition> {
  tools
    .iter()
    .map(|tool| RuntimeToolDefinition {
      name: tool.qualified_name.clone(),
      description: tool.tool.description.clone(),
      input_schema: tool
        .tool
        .input_schema
        .clone()
        .unwrap_or_else(|| json!({"type": "object", "additionalProperties": true})),
      required_permission: ToolPermissionMode::DangerFullAccess,
    })
    .collect()
}

pub async fn discover_runtime_tools(
  config: &McpModuleConfig,
) -> Result<Vec<RuntimeToolDefinition>, McpError> {
  let mut manager = McpServerManager::from_config(config);
  let tools = manager.discover_tools().await;
  let shutdown = manager.shutdown().await;
  match (tools, shutdown) {
    (Ok(tools), Ok(())) => Ok(runtime_tool_definitions(&tools)),
    (Err(error), _) => Err(error),
    (Ok(_), Err(error)) => Err(error),
  }
}

pub fn default_mcps() -> Vec<McpServerStatusView> {
  resolve_from_env().as_view().servers
}

pub fn active_mcp_count() -> usize {
  resolve_from_env().servers.len()
}

pub fn normalize_name_for_mcp(name: &str) -> String {
  let mut normalized = String::with_capacity(name.len());
  for ch in name.chars() {
    match ch {
      'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' => normalized.push(ch),
      _ => normalized.push('_'),
    }
  }
  normalized
}

pub fn mcp_tool_prefix(server_name: &str) -> String {
  format!("mcp__{}__", normalize_name_for_mcp(server_name))
}

pub fn mcp_tool_name(server_name: &str, tool_name: &str) -> String {
  format!("{}{}", mcp_tool_prefix(server_name), normalize_name_for_mcp(tool_name))
}

pub fn preflight_config(config: &McpModuleConfig) -> McpPreflightReport {
  let mut servers = Vec::new();
  for (name, server_config) in &config.servers {
    let (status, detail) = match server_config {
      McpServerConfig::Stdio { command, .. } => {
        if resolve_command(command).is_some() {
          (McpServerStatus::Resolved, None)
        } else {
          (
            McpServerStatus::CommandNotFound,
            Some(format!("command `{command}` was not found on PATH or filesystem")),
          )
        }
      }
      McpServerConfig::Http { .. }
      | McpServerConfig::Sse { .. }
      | McpServerConfig::Ws { .. } => (
        McpServerStatus::UnsupportedTransport,
        Some("remote MCP transports are configured but not executable in Yuanling v1".to_string()),
      ),
    };
    servers.push(McpServerStatusView {
      name: name.clone(),
      transport: server_config.transport(),
      status,
      detail,
    });
  }
  let degraded = servers.iter().any(|server| {
    matches!(
      server.status,
      McpServerStatus::CommandNotFound | McpServerStatus::UnsupportedTransport
    )
  });
  McpPreflightReport { degraded, servers }
}

fn expect_response<T>(
  server_name: &str,
  method: &'static str,
  id: JsonRpcId,
  response: JsonRpcResponse<T>,
) -> Result<T, McpError> {
  if response.id != Some(id) {
    return Err(McpError::InvalidResponse {
      server_name: server_name.to_string(),
      method,
      details: "mismatched JSON-RPC response id".to_string(),
    });
  }
  if let Some(error) = response.error {
    return Err(McpError::JsonRpc {
      server_name: server_name.to_string(),
      method,
      code: error.code,
      message: error.message,
    });
  }
  response.result.ok_or_else(|| McpError::InvalidResponse {
    server_name: server_name.to_string(),
    method,
    details: "missing JSON-RPC result".to_string(),
  })
}

async fn run_with_timeout<T>(
  server_name: &str,
  method: &'static str,
  timeout_ms: u64,
  future: impl std::future::Future<Output = Result<T, McpError>>,
) -> Result<T, McpError> {
  match timeout(Duration::from_millis(timeout_ms.max(1)), future).await {
    Ok(result) => result,
    Err(_) => Err(McpError::Timeout {
      server_name: server_name.to_string(),
      method,
      timeout_ms,
    }),
  }
}

fn encode_frame(payload: &[u8]) -> Vec<u8> {
  let mut output = format!("Content-Length: {}\r\n\r\n", payload.len()).into_bytes();
  output.extend_from_slice(payload);
  output
}

async fn read_frame(reader: &mut BufReader<ChildStdout>) -> Result<Vec<u8>, McpError> {
  let mut header = Vec::new();
  let mut byte = [0_u8; 1];
  loop {
    let read = reader.read(&mut byte).await?;
    if read == 0 {
      return Err(McpError::Io("unexpected EOF while reading MCP frame header".to_string()));
    }
    header.push(byte[0]);
    if header.ends_with(b"\r\n\r\n") {
      break;
    }
    if header.len() > 8192 {
      return Err(McpError::Io("MCP frame header is too large".to_string()));
    }
  }

  let header_text = String::from_utf8_lossy(&header);
  let mut content_length = None;
  for line in header_text.split("\r\n") {
    let Some((key, value)) = line.split_once(':') else {
      continue;
    };
    if key.eq_ignore_ascii_case("content-length") {
      content_length = value.trim().parse::<usize>().ok();
    }
  }
  let Some(content_length) = content_length else {
    return Err(McpError::Io("MCP frame is missing Content-Length".to_string()));
  };

  let mut payload = vec![0_u8; content_length];
  reader.read_exact(&mut payload).await?;
  Ok(payload)
}

fn resolve_command(command: &str) -> Option<PathBuf> {
  let path = Path::new(command);
  if path.components().count() > 1 || path.is_absolute() {
    return path.is_file().then(|| path.to_path_buf());
  }
  let path_env = env::var_os("PATH")?;
  env::split_paths(&path_env)
    .map(|dir| dir.join(command))
    .find(|candidate| candidate.is_file())
}

fn env_or_optional(key: &str) -> Option<String> {
  env::var(key).ok().filter(|value| !value.trim().is_empty())
}

fn env_or_bool(key: &str, default: bool) -> bool {
  env_or_optional(key)
    .and_then(|raw| raw.parse::<bool>().ok())
    .unwrap_or(default)
}

fn env_or_u64(key: &str, default: u64) -> u64 {
  env_or_optional(key)
    .and_then(|raw| raw.parse::<u64>().ok())
    .unwrap_or(default)
}
