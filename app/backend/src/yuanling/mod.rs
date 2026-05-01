use axum::{routing::get, Json, Router};
use serde_json::json;

pub mod ai;
pub mod agent;
pub mod context;
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
    let ai_config = ai::resolve_from_env().as_view();
    let context_config = context::resolve_from_env().view();
  Json(json!({
    "module": "yuanling",
    "enabled": true,
    "ai": ai_config,
    "context": context_config,
    "submodules": {
      "ai": ai_config.enabled,
      "context": context_config.enabled,
      "tools": true,
      "skills": true,
      "mcp": true,
      "memory": true,
      "agent": true,
    }
  }))
}

pub fn router() -> Router {
  Router::new()
    .route("/yuanling/status", get(status))
    .merge(ai::router())
}
