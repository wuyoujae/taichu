use axum::{routing::get, Json, Router};
use serde_json::json;

pub mod ai;
pub mod agent;
pub mod contact;
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
  let agent_config = agent::resolve_from_env().as_view();
  let ai_config = ai::resolve_from_env().as_view();
  let contact_config = contact::resolve_from_env().as_view();
  let context_config = context::resolve_from_env().view();
  let tools_registry = tools::ToolRegistry::builtin();
  let tools_config = tools::resolve_from_env().as_view(&tools_registry);
  let mcp_config = mcp::resolve_from_env();
  let mcp_view = mcp_config.as_view();
  let skills_config_raw = skills::resolve_from_env();
  let skills_registry = skills::SkillRegistry::discover(&skills_config_raw)
    .unwrap_or_else(|_| skills::SkillRegistry::discover(&skills::SkillsModuleConfig {
      enabled: false,
      roots: Vec::new(),
      allowed_skills: None,
      max_prompt_chars: 0,
      max_search_results: 0,
      auto_inject_enabled: false,
      auto_inject_max_items: 0,
    }).expect("disabled skills registry should build"));
  let skills_config = skills_config_raw.as_view(&skills_registry);
  Json(json!({
    "module": "yuanling",
    "enabled": true,
    "agent": agent_config,
    "ai": ai_config,
    "contact": contact_config,
    "context": context_config,
    "tools": tools_config,
    "mcp": mcp_view,
    "skills": skills_config,
    "submodules": {
      "ai": ai_config.enabled,
      "contact": contact_config.enabled,
      "context": context_config.enabled,
      "tools": tools_config.enabled,
      "skills": skills_config.enabled,
      "mcp": mcp_config.enabled,
      "memory": true,
      "agent": agent_config.enabled,
    }
  }))
}


pub fn router() -> Router {
  Router::new()
    .route("/yuanling/status", get(status))
    .merge(ai::router())
    .merge(skills::router())
    .merge(mcp::router())
}
