#[path = "../src/spiritkind/mod.rs"]
mod spiritkind;

use spiritkind::{
  list_directory, load_registry, register_member, register_triad, save_registry, set_member_status,
  skills_for, system_prompt_for, tools_for, RegisterSpiritkindMemberRequest, RegisterTriadRequest,
  SpiritkindModuleConfig, SpiritkindRole, SpiritkindStatus, AEGIS_YUANLING_ID, VERIN_YUANLING_ID,
};
use uuid::Uuid;

fn config() -> SpiritkindModuleConfig {
  SpiritkindModuleConfig {
    enabled: true,
    storage_dir: std::env::temp_dir()
      .join(format!("taichu-spiritkind-test-{}", Uuid::new_v4()))
      .display()
      .to_string(),
  }
}

#[test]
fn default_registry_has_verin_and_aegis() {
  let config = config();
  let registry = load_registry(&config).expect("default registry should load");

  assert!(registry.members.iter().any(|member| member.yuanling_id == VERIN_YUANLING_ID));
  assert!(registry.members.iter().any(|member| member.yuanling_id == AEGIS_YUANLING_ID));
}

#[test]
fn register_member_persists_and_can_be_queried() {
  let config = config();
  let member = register_member(
    RegisterSpiritkindMemberRequest {
      role: SpiritkindRole::Artifex,
      display_name: "编程司工".to_string(),
      description: "负责代码实现".to_string(),
      system_prompt: "你是编程司工。".to_string(),
      tools: Some(vec!["read_file".to_string(), "write_file".to_string()]),
      skills: Some(vec!["rust".to_string()]),
      team_id: None,
      yuanling_id: None,
    },
    &config,
  ).expect("member should register");

  let loaded = load_registry(&config).expect("registry should reload");
  assert!(loaded.members.iter().any(|item| item.yuanling_id == member.yuanling_id));
  assert_eq!(system_prompt_for(&member.yuanling_id, &config).expect("prompt lookup"), Some("你是编程司工。".to_string()));
  assert_eq!(tools_for(&member.yuanling_id, &config).expect("tools lookup"), vec!["read_file", "write_file"]);
  assert_eq!(skills_for(&member.yuanling_id, &config).expect("skills lookup"), vec!["rust"]);
}

#[test]
fn register_triad_creates_team_and_three_members() {
  let config = config();
  let team = register_triad(
    RegisterTriadRequest {
      name: "编程三府".to_string(),
      domain: "coding".to_string(),
      description: "负责软件开发任务".to_string(),
      taiyi_prompt: Some("决策编程方案".to_string()),
      artifex_prompt: None,
      lexon_prompt: None,
      tools: Some(vec!["read_file".to_string(), "edit_file".to_string()]),
      skills: Some(vec!["backend".to_string()]),
    },
    &config,
  ).expect("triad should register");

  let registry = load_registry(&config).expect("registry should reload");
  assert_eq!(team.member_yuanling_ids.len(), 3);
  assert!(registry.teams.iter().any(|item| item.team_id == team.team_id));
  for member_id in &team.member_yuanling_ids {
    let member = registry.members.iter().find(|member| member.yuanling_id == *member_id).expect("member should exist");
    assert_eq!(member.team_id.as_deref(), Some(team.team_id.as_str()));
  }
}

#[test]
fn directory_groups_leadership_and_triad_members() {
  let config = config();
  register_triad(
    RegisterTriadRequest {
      name: "邮件三府".to_string(),
      domain: "mail".to_string(),
      description: "负责邮件管理".to_string(),
      taiyi_prompt: None,
      artifex_prompt: None,
      lexon_prompt: None,
      tools: None,
      skills: None,
    },
    &config,
  ).expect("triad should register");

  let directory = list_directory(&config).expect("directory should build");
  assert_eq!(directory.leadership.len(), 2);
  assert_eq!(directory.teams.len(), 1);
  assert_eq!(directory.teams[0].members.len(), 3);
}

#[test]
fn disabling_member_keeps_it_in_directory_with_status() {
  let config = config();
  let updated = set_member_status(VERIN_YUANLING_ID, SpiritkindStatus::Disabled, &config)
    .expect("member status should update");

  assert_eq!(updated.status, SpiritkindStatus::Disabled);
  let directory = list_directory(&config).expect("directory should build");
  let verin = directory.leadership.iter().find(|member| member.yuanling_id == VERIN_YUANLING_ID).expect("verin should remain visible");
  assert_eq!(verin.status, SpiritkindStatus::Disabled);
}

#[test]
fn save_registry_creates_storage_directory() {
  let config = config();
  let registry = load_registry(&config).expect("default registry should load");
  save_registry(&registry, &config).expect("registry should save");

  assert!(std::path::Path::new(&config.storage_dir).join("registry.json").exists());
}

#[test]
fn duplicate_or_invalid_yuanling_id_is_rejected() {
  let config = config();
  let duplicate = register_member(
    RegisterSpiritkindMemberRequest {
      role: SpiritkindRole::Artifex,
      display_name: "重复成员".to_string(),
      description: "重复 id".to_string(),
      system_prompt: "重复".to_string(),
      tools: None,
      skills: None,
      team_id: None,
      yuanling_id: Some(VERIN_YUANLING_ID.to_string()),
    },
    &config,
  );
  assert!(duplicate.is_err());

  let invalid = register_member(
    RegisterSpiritkindMemberRequest {
      role: SpiritkindRole::Artifex,
      display_name: "非法成员".to_string(),
      description: "非法 id".to_string(),
      system_prompt: "非法".to_string(),
      tools: None,
      skills: None,
      team_id: None,
      yuanling_id: Some("not-a-uuid".to_string()),
    },
    &config,
  );
  assert!(invalid.is_err());
}
