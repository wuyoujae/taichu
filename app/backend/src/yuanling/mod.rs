use axum::{routing::get, Json, Router};
use serde_json::json;

pub mod ai;
pub mod agent;
pub mod memory;
pub mod mcp;
pub mod skills;
pub mod tools;

pub struct YuanlingModule {
  pub enabled: bool,
}

impl YuanlingModule {
  pub fn new(enabled: bool) -> Self {
    Self { enabled }
  }
}

pub async fn status() -> Json<serde_json::Value> {
  Json(json!({
    "module": "yuanling",
    "enabled": true,
    "submodules": {
      "ai": true,
      "tools": true,
      "skills": true,
      "mcp": true,
      "memory": true,
      "agent": true,
    }
  }))
}

pub fn router() -> Router {
  Router::new().route("/yuanling/status", get(status))
}

