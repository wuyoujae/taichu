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
      "agent": true,
    }
  }))
}

#[cfg(test)]
mod integration_tests {
  use super::{context, tools};
  use serde_json::json;
  use std::fs;
  use std::path::PathBuf;
  use std::time::{SystemTime, UNIX_EPOCH};

  fn smoke_storage_dir() -> PathBuf {
    let millis = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("clock should work")
      .as_millis();
    let path = std::env::temp_dir().join(format!("taichu-yuanling-smoke-{millis}"));
    fs::create_dir_all(&path).expect("smoke storage should be created");
    path
  }

  #[test]
  #[ignore = "requires YUANLING_SKILLS_ROOTS to point at a temporary skill root"]
  fn skills_context_and_tool_chain_smoke() {
    let configured_roots = std::env::var("YUANLING_SKILLS_ROOTS")
      .expect("YUANLING_SKILLS_ROOTS must point at a skill root");
    let first_root = std::env::split_paths(&configured_roots)
      .next()
      .expect("at least one skill root is required");
    assert!(
      first_root.join("writer").join("SKILL.md").is_file(),
      "smoke test expects writer/SKILL.md in the configured root"
    );

    let mut context_config = context::ContextModuleConfig::default();
    context_config.storage_dir = smoke_storage_dir().to_string_lossy().to_string();
    context_config.auto_compact_enabled = false;
    context::append_message(
      "skills-smoke",
      context::ContextMessage::text(context::ContextRole::User, "Use the writer skill."),
      &context_config,
    )
    .expect("message should append");

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime should build");
    let built = runtime
      .block_on(context::build_context("skills-smoke", &context_config))
      .expect("context should build");
    let first_text = built
      .messages
      .first()
      .and_then(|message| message.blocks.first())
      .and_then(|block| match block {
        context::ContextBlock::Text { text } => Some(text.as_str()),
        _ => None,
      })
      .expect("first context message should be text");
    assert!(first_text.contains("Available Yuanling skills"));
    assert!(first_text.contains("writer"));

    let registry = tools::ToolRegistry::builtin();
    let tools_config = tools::resolve_from_env();
    let cwd = std::env::current_dir().expect("cwd should resolve");
    let mut executor = tools::BuiltinToolExecutor::global(cwd).expect("executor should build");
    let output = registry
      .execute_with_permissions(
        "Skill",
        &json!({"skill": "writer", "args": "draft"}),
        &tools_config,
        &mut executor,
        None,
      )
      .expect("Skill tool should load the configured skill");

    assert_eq!(output.tool_name, "Skill");
    assert!(output.output["prompt"]
      .as_str()
      .unwrap_or_default()
      .contains("Use this smoke skill carefully."));
    assert_eq!(output.output["args"], "draft");
  }
}

pub fn router() -> Router {
  Router::new()
    .route("/yuanling/status", get(status))
    .merge(ai::router())
}
