use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct McpConnectorConfig {
  pub id: String,
  pub endpoint: String,
  pub enabled: bool,
}

pub fn default_mcps() -> Vec<McpConnectorConfig> {
  vec![
    McpConnectorConfig {
      id: "local_filesystem".to_string(),
      endpoint: "mcp://local/filesystem".to_string(),
      enabled: true,
    },
  ]
}

pub fn active_mcp_count() -> usize {
  default_mcps().into_iter().filter(|c| c.enabled).count()
}

