#[path = "../src/yuanling/ai/mod.rs"]
mod ai;
#[path = "../src/yuanling/contact/mod.rs"]
mod contact;
#[path = "../src/yuanling/mcp/mod.rs"]
mod mcp;
#[path = "../src/yuanling/skills/mod.rs"]
mod skills;
#[path = "../src/yuanling/tools/mod.rs"]
mod tools;

use serde_json::json;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use tools::{
  clear_tool_access_rule, definitions_for_yuanling, load_tool_state, save_tool_state,
  set_tool_access_rule, tool_access_for_yuanling, StaticToolExecutor, ToolAccessMode,
  ToolAccessRule, ToolFilesystemScope, ToolPermissionPolicy, ToolRegistry, ToolsModuleConfig,
};
use uuid::Uuid;

fn config() -> ToolsModuleConfig {
  ToolsModuleConfig {
    enabled: true,
    allowed_tools: None,
    max_search_results: 10,
    permission_policy: ToolPermissionPolicy::default(),
    filesystem_scope: ToolFilesystemScope::Global,
    workspace_root: None,
    state_dir: std::env::temp_dir()
      .join(format!("taichu-tools-state-test-{}", Uuid::new_v4()))
      .display()
      .to_string(),
  }
}

fn rule(tool_name: &str, mode: ToolAccessMode, allow: &[&str], deny: &[&str]) -> ToolAccessRule {
  ToolAccessRule {
    tool_name: tool_name.to_string(),
    mode,
    allow_yuanling_ids: allow.iter().map(|value| value.to_string()).collect(),
    deny_yuanling_ids: deny.iter().map(|value| value.to_string()).collect(),
    updated_at_ms: 0,
  }
}

#[test]
fn missing_state_file_defaults_to_enabled_all() {
  let config = config();
  let state = load_tool_state(&config).expect("state should load");
  let decision = tool_access_for_yuanling("read_file", "agent-a", &config)
    .expect("decision should build");

  assert!(state.rules.is_empty());
  assert!(decision.allowed);
  assert_eq!(decision.mode, ToolAccessMode::EnabledAll);
}

#[test]
fn disabled_all_blocks_every_yuanling() {
  let config = config();
  set_tool_access_rule(
    "read_file",
    rule("read_file", ToolAccessMode::DisabledAll, &[], &[]),
    &config,
  ).expect("rule should save");

  let decision = tool_access_for_yuanling("read_file", "agent-a", &config)
    .expect("decision should build");
  assert!(!decision.allowed);
}

#[test]
fn enabled_all_allows_every_yuanling() {
  let config = config();
  set_tool_access_rule(
    "read_file",
    rule("read_file", ToolAccessMode::EnabledAll, &[], &[]),
    &config,
  ).expect("rule should save");

  assert!(tool_access_for_yuanling("read_file", "agent-a", &config).unwrap().allowed);
  assert!(tool_access_for_yuanling("read_file", "agent-b", &config).unwrap().allowed);
}

#[test]
fn allow_only_allows_only_selected_yuanlings() {
  let config = config();
  set_tool_access_rule(
    "read_file",
    rule("read_file", ToolAccessMode::AllowOnly, &["agent-a"], &[]),
    &config,
  ).expect("rule should save");

  assert!(tool_access_for_yuanling("read_file", "agent-a", &config).unwrap().allowed);
  assert!(!tool_access_for_yuanling("read_file", "agent-b", &config).unwrap().allowed);
}

#[test]
fn deny_only_blocks_selected_yuanlings() {
  let config = config();
  set_tool_access_rule(
    "read_file",
    rule("read_file", ToolAccessMode::DenyOnly, &[], &["agent-a"]),
    &config,
  ).expect("rule should save");

  assert!(!tool_access_for_yuanling("read_file", "agent-a", &config).unwrap().allowed);
  assert!(tool_access_for_yuanling("read_file", "agent-b", &config).unwrap().allowed);
}

#[test]
fn deny_list_wins_over_allow_list() {
  let config = config();
  set_tool_access_rule(
    "read_file",
    rule("read_file", ToolAccessMode::AllowOnly, &["agent-a"], &["agent-a"]),
    &config,
  ).expect("rule should save");

  assert!(!tool_access_for_yuanling("read_file", "agent-a", &config).unwrap().allowed);
}

#[test]
fn clear_rule_restores_default_access() {
  let config = config();
  set_tool_access_rule(
    "read_file",
    rule("read_file", ToolAccessMode::DisabledAll, &[], &[]),
    &config,
  ).expect("rule should save");
  clear_tool_access_rule("read_file", &config).expect("rule should clear");

  assert!(tool_access_for_yuanling("read_file", "agent-a", &config).unwrap().allowed);
}

#[test]
fn definitions_for_yuanling_intersects_spiritkind_and_state() {
  let config = config();
  set_tool_access_rule(
    "write_file",
    rule("write_file", ToolAccessMode::DisabledAll, &[], &[]),
    &config,
  ).expect("rule should save");
  let registry = ToolRegistry::builtin();
  let spiritkind_tools = BTreeSet::from(["read_file".to_string(), "write_file".to_string()]);
  let definitions = definitions_for_yuanling(
    &registry,
    "agent-a",
    &config,
    Some(&spiritkind_tools),
  ).expect("definitions should build");
  let names = definitions.into_iter().map(|definition| definition.name).collect::<BTreeSet<_>>();

  assert!(names.contains("read_file"));
  assert!(!names.contains("write_file"));
  assert!(!names.contains("bash"));
}

#[test]
fn execute_for_yuanling_state_block_does_not_prompt_or_execute() {
  let config = config();
  set_tool_access_rule(
    "read_file",
    rule("read_file", ToolAccessMode::DisabledAll, &[], &[]),
    &config,
  ).expect("rule should save");
  let count = Arc::new(Mutex::new(0usize));
  let counter = count.clone();
  let mut executor = StaticToolExecutor::new().register("read_file", move |_| {
    *counter.lock().expect("counter lock") += 1;
    Ok(json!({"ok": true}))
  });

  let result = ToolRegistry::builtin().execute_for_yuanling(
    "agent-a",
    "read_file",
    &json!({"path":"README.md"}),
    &config,
    None,
    &mut executor,
    None,
  );

  assert!(result.is_err());
  assert_eq!(*count.lock().expect("counter lock"), 0);
}

#[test]
fn execute_with_permissions_keeps_legacy_behavior_without_state_check() {
  let config = config();
  set_tool_access_rule(
    "read_file",
    rule("read_file", ToolAccessMode::DisabledAll, &[], &[]),
    &config,
  ).expect("rule should save");
  let mut executor = StaticToolExecutor::new().register("read_file", |_| Ok(json!({"ok": true})));

  let result = ToolRegistry::builtin().execute_with_permissions(
    "read_file",
    &json!({"path":"README.md"}),
    &config,
    &mut executor,
    None,
  ).expect("legacy execution should ignore state rules");

  assert_eq!(result.output, json!({"ok": true}));
}

#[test]
fn save_tool_state_creates_storage_file() {
  let config = config();
  let mut state = load_tool_state(&config).expect("state should load");
  state.rules.push(rule("read_file", ToolAccessMode::EnabledAll, &[], &[]));
  save_tool_state(&state, &config).expect("state should save");

  assert!(std::path::Path::new(&config.state_dir).join("tool_state.json").exists());
}
