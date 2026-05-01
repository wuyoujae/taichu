use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
  pub id: String,
  pub name: String,
  pub enabled: bool,
  pub description: String,
}

pub fn default_tools() -> Vec<ToolDescriptor> {
  vec![
    ToolDescriptor {
      id: "http_request".to_string(),
      name: "HTTP 请求".to_string(),
      enabled: true,
      description: "与外部服务通信的基础工具。".to_string(),
    },
  ]
}

pub fn register_tool(tool_id: &str) -> bool {
  matches!(tool_id, "http_request")
}

