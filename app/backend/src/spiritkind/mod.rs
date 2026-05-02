use serde::{Deserialize, Serialize};
use std::{env, fs, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};
use uuid::Uuid;

const REGISTRY_VERSION: u32 = 1;
const REGISTRY_FILE_NAME: &str = "registry.json";

pub const USER_YUANLING_ID: &str = "000000";
pub const VERIN_YUANLING_ID: &str = "000001";
pub const AEGIS_YUANLING_ID: &str = "000002";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpiritkindModuleConfig {
  pub enabled: bool,
  pub storage_dir: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpiritkindModuleConfigView {
  pub enabled: bool,
  pub storage_dir: String,
}

impl SpiritkindModuleConfig {
  pub fn as_view(&self) -> SpiritkindModuleConfigView {
    SpiritkindModuleConfigView {
      enabled: self.enabled,
      storage_dir: self.storage_dir.clone(),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
#[serde(try_from = "u8", into = "u8")]
pub enum SpiritkindRole {
  Verin = 1,
  Aegis = 2,
  Taiyi = 3,
  Artifex = 4,
  Lexon = 5,
}

impl SpiritkindRole {
  pub fn label(self) -> &'static str {
    match self {
      Self::Verin => "司言",
      Self::Aegis => "司衡",
      Self::Taiyi => "太一",
      Self::Artifex => "司工",
      Self::Lexon => "司律",
    }
  }

  pub fn code(self) -> &'static str {
    match self {
      Self::Verin => "verin",
      Self::Aegis => "aegis",
      Self::Taiyi => "taiyi",
      Self::Artifex => "artifex",
      Self::Lexon => "lexon",
    }
  }
}

impl From<SpiritkindRole> for u8 {
  fn from(value: SpiritkindRole) -> Self {
    value as u8
  }
}

impl TryFrom<u8> for SpiritkindRole {
  type Error = String;

  fn try_from(value: u8) -> Result<Self, Self::Error> {
    match value {
      1 => Ok(Self::Verin),
      2 => Ok(Self::Aegis),
      3 => Ok(Self::Taiyi),
      4 => Ok(Self::Artifex),
      5 => Ok(Self::Lexon),
      _ => Err(format!("invalid spiritkind role: {value}")),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
#[serde(try_from = "u8", into = "u8")]
pub enum SpiritkindStatus {
  Enabled = 1,
  Disabled = 2,
  Archived = 3,
}

impl From<SpiritkindStatus> for u8 {
  fn from(value: SpiritkindStatus) -> Self {
    value as u8
  }
}

impl TryFrom<u8> for SpiritkindStatus {
  type Error = String;

  fn try_from(value: u8) -> Result<Self, Self::Error> {
    match value {
      1 => Ok(Self::Enabled),
      2 => Ok(Self::Disabled),
      3 => Ok(Self::Archived),
      _ => Err(format!("invalid spiritkind status: {value}")),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpiritkindMember {
  pub member_id: String,
  pub yuanling_id: String,
  pub role: SpiritkindRole,
  pub display_name: String,
  pub code_name: String,
  pub description: String,
  pub system_prompt: String,
  pub tools: Vec<String>,
  pub skills: Vec<String>,
  pub team_id: Option<String>,
  pub status: SpiritkindStatus,
  pub created_at_ms: u64,
  pub updated_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpiritkindTeam {
  pub team_id: String,
  pub name: String,
  pub domain: String,
  pub description: String,
  pub member_yuanling_ids: Vec<String>,
  pub status: SpiritkindStatus,
  pub created_at_ms: u64,
  pub updated_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpiritkindRegistry {
  pub version: u32,
  pub user_yuanling_id: String,
  pub default_entry_yuanling_id: String,
  pub default_dispatch_yuanling_id: String,
  pub members: Vec<SpiritkindMember>,
  pub teams: Vec<SpiritkindTeam>,
  pub updated_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpiritkindDirectory {
  pub leadership: Vec<SpiritkindMember>,
  pub teams: Vec<SpiritkindTeamDirectory>,
  pub independent_members: Vec<SpiritkindMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpiritkindTeamDirectory {
  pub team: SpiritkindTeam,
  pub members: Vec<SpiritkindMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterSpiritkindMemberRequest {
  pub role: SpiritkindRole,
  pub display_name: String,
  pub description: String,
  pub system_prompt: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub tools: Option<Vec<String>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub skills: Option<Vec<String>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub team_id: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub yuanling_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterTriadRequest {
  pub name: String,
  pub domain: String,
  pub description: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub taiyi_prompt: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub artifex_prompt: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub lexon_prompt: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub tools: Option<Vec<String>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub skills: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpiritkindError {
  Disabled,
  Io(String),
  Json(String),
  InvalidInput(String),
  DuplicateYuanlingId(String),
  DuplicateTeamId(String),
  UnknownTeam(String),
  UnknownMember(String),
}

impl std::fmt::Display for SpiritkindError {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Disabled => write!(formatter, "spiritkind module is disabled"),
      Self::Io(message) => write!(formatter, "spiritkind io error: {message}"),
      Self::Json(message) => write!(formatter, "spiritkind json error: {message}"),
      Self::InvalidInput(message) => write!(formatter, "invalid spiritkind input: {message}"),
      Self::DuplicateYuanlingId(id) => write!(formatter, "duplicate yuanling_id: {id}"),
      Self::DuplicateTeamId(id) => write!(formatter, "duplicate team_id: {id}"),
      Self::UnknownTeam(id) => write!(formatter, "unknown spiritkind team: {id}"),
      Self::UnknownMember(id) => write!(formatter, "unknown spiritkind member: {id}"),
    }
  }
}

impl std::error::Error for SpiritkindError {}

pub fn resolve_from_env() -> SpiritkindModuleConfig {
  SpiritkindModuleConfig {
    enabled: env_bool("SPIRITKIND_ENABLED", true),
    storage_dir: env_optional("SPIRITKIND_STORAGE_DIR").unwrap_or_else(default_storage_dir),
  }
}

pub fn load_registry(config: &SpiritkindModuleConfig) -> Result<SpiritkindRegistry, SpiritkindError> {
  ensure_enabled(config)?;
  let path = registry_path(config);
  if !path.exists() {
    return Ok(default_registry());
  }
  let contents = fs::read_to_string(path).map_err(io_error)?;
  serde_json::from_str(&contents).map_err(|error| SpiritkindError::Json(error.to_string()))
}

pub fn save_registry(
  registry: &SpiritkindRegistry,
  config: &SpiritkindModuleConfig,
) -> Result<(), SpiritkindError> {
  ensure_enabled(config)?;
  fs::create_dir_all(&config.storage_dir).map_err(io_error)?;
  let body = serde_json::to_string_pretty(registry)
    .map_err(|error| SpiritkindError::Json(error.to_string()))?;
  fs::write(registry_path(config), body).map_err(io_error)
}

pub fn register_member(
  request: RegisterSpiritkindMemberRequest,
  config: &SpiritkindModuleConfig,
) -> Result<SpiritkindMember, SpiritkindError> {
  let mut registry = load_registry(config)?;
  let now = now_ms();
  let team_id = normalize_optional_text(request.team_id);
  if let Some(team_id) = &team_id {
    if !registry.teams.iter().any(|team| team.team_id == *team_id) {
      return Err(SpiritkindError::UnknownTeam(team_id.clone()));
    }
  }

  let yuanling_id = request.yuanling_id.unwrap_or_else(new_uuid);
  validate_yuanling_id(&yuanling_id)?;
  ensure_unique_yuanling_id(&registry, &yuanling_id)?;
  let member = SpiritkindMember {
    member_id: new_uuid(),
    yuanling_id: yuanling_id.clone(),
    role: request.role,
    display_name: require_text(request.display_name, "display_name")?,
    code_name: request.role.code().to_string(),
    description: require_text(request.description, "description")?,
    system_prompt: require_text(request.system_prompt, "system_prompt")?,
    tools: normalize_list(request.tools.unwrap_or_else(default_member_tools)),
    skills: normalize_list(request.skills.unwrap_or_default()),
    team_id: team_id.clone(),
    status: SpiritkindStatus::Enabled,
    created_at_ms: now,
    updated_at_ms: now,
  };

  if let Some(team_id) = team_id {
    if let Some(team) = registry.teams.iter_mut().find(|team| team.team_id == team_id) {
      team.member_yuanling_ids.push(yuanling_id);
      team.updated_at_ms = now;
    }
  }
  registry.members.push(member.clone());
  registry.updated_at_ms = now;
  save_registry(&registry, config)?;
  Ok(member)
}

pub fn register_triad(
  request: RegisterTriadRequest,
  config: &SpiritkindModuleConfig,
) -> Result<SpiritkindTeam, SpiritkindError> {
  let mut registry = load_registry(config)?;
  let now = now_ms();
  let team_id = new_uuid();
  if registry.teams.iter().any(|team| team.team_id == team_id) {
    return Err(SpiritkindError::DuplicateTeamId(team_id));
  }

  let name = require_text(request.name, "name")?;
  let domain = require_text(request.domain, "domain")?;
  let description = require_text(request.description, "description")?;
  let tools = normalize_list(request.tools.unwrap_or_else(default_member_tools));
  let skills = normalize_list(request.skills.unwrap_or_default());
  let members = vec![
    build_triad_member(
      SpiritkindRole::Taiyi,
      "太一",
      "三府中的决策智能体，负责策略判断、方案取舍和关键节点决策。",
      request.taiyi_prompt.unwrap_or_else(|| TAIYI_SYSTEM_PROMPT.to_string()),
      &team_id,
      &tools,
      &skills,
      now,
    ),
    build_triad_member(
      SpiritkindRole::Artifex,
      "司工",
      "三府中的执行智能体，负责把任务转化为具体产出。",
      request.artifex_prompt.unwrap_or_else(|| ARTIFEX_SYSTEM_PROMPT.to_string()),
      &team_id,
      &tools,
      &skills,
      now,
    ),
    build_triad_member(
      SpiritkindRole::Lexon,
      "司律",
      "三府中的监督智能体，负责质量校验、规则约束和风险提示。",
      request.lexon_prompt.unwrap_or_else(|| LEXON_SYSTEM_PROMPT.to_string()),
      &team_id,
      &tools,
      &skills,
      now,
    ),
  ];
  for member in &members {
    ensure_unique_yuanling_id(&registry, &member.yuanling_id)?;
  }

  let team = SpiritkindTeam {
    team_id: team_id.clone(),
    name,
    domain,
    description,
    member_yuanling_ids: members.iter().map(|member| member.yuanling_id.clone()).collect(),
    status: SpiritkindStatus::Enabled,
    created_at_ms: now,
    updated_at_ms: now,
  };
  registry.members.extend(members);
  registry.teams.push(team.clone());
  registry.updated_at_ms = now;
  save_registry(&registry, config)?;
  Ok(team)
}

pub fn list_directory(config: &SpiritkindModuleConfig) -> Result<SpiritkindDirectory, SpiritkindError> {
  let registry = load_registry(config)?;
  Ok(directory_from_registry(&registry))
}

pub fn get_member(
  yuanling_id: &str,
  config: &SpiritkindModuleConfig,
) -> Result<Option<SpiritkindMember>, SpiritkindError> {
  let registry = load_registry(config)?;
  Ok(registry.members.into_iter().find(|member| member.yuanling_id == yuanling_id))
}

pub fn set_member_status(
  yuanling_id: &str,
  status: SpiritkindStatus,
  config: &SpiritkindModuleConfig,
) -> Result<SpiritkindMember, SpiritkindError> {
  let mut registry = load_registry(config)?;
  let now = now_ms();
  let member = registry
    .members
    .iter_mut()
    .find(|member| member.yuanling_id == yuanling_id)
    .ok_or_else(|| SpiritkindError::UnknownMember(yuanling_id.to_string()))?;
  member.status = status;
  member.updated_at_ms = now;
  let updated = member.clone();
  registry.updated_at_ms = now;
  save_registry(&registry, config)?;
  Ok(updated)
}

pub fn system_prompt_for(
  yuanling_id: &str,
  config: &SpiritkindModuleConfig,
) -> Result<Option<String>, SpiritkindError> {
  Ok(get_member(yuanling_id, config)?.map(|member| member.system_prompt))
}

pub fn tools_for(
  yuanling_id: &str,
  config: &SpiritkindModuleConfig,
) -> Result<Vec<String>, SpiritkindError> {
  Ok(get_member(yuanling_id, config)?.map_or_else(Vec::new, |member| member.tools))
}

pub fn skills_for(
  yuanling_id: &str,
  config: &SpiritkindModuleConfig,
) -> Result<Vec<String>, SpiritkindError> {
  Ok(get_member(yuanling_id, config)?.map_or_else(Vec::new, |member| member.skills))
}

pub fn default_registry() -> SpiritkindRegistry {
  let now = now_ms();
  SpiritkindRegistry {
    version: REGISTRY_VERSION,
    user_yuanling_id: USER_YUANLING_ID.to_string(),
    default_entry_yuanling_id: VERIN_YUANLING_ID.to_string(),
    default_dispatch_yuanling_id: AEGIS_YUANLING_ID.to_string(),
    members: vec![verin_member(now), aegis_member(now)],
    teams: Vec::new(),
    updated_at_ms: now,
  }
}

fn directory_from_registry(registry: &SpiritkindRegistry) -> SpiritkindDirectory {
  let leadership = registry
    .members
    .iter()
    .filter(|member| matches!(member.role, SpiritkindRole::Verin | SpiritkindRole::Aegis))
    .cloned()
    .collect::<Vec<_>>();
  let teams = registry
    .teams
    .iter()
    .cloned()
    .map(|team| {
      let members = team
        .member_yuanling_ids
        .iter()
        .filter_map(|id| registry.members.iter().find(|member| member.yuanling_id == *id).cloned())
        .collect();
      SpiritkindTeamDirectory { team, members }
    })
    .collect();
  let independent_members = registry
    .members
    .iter()
    .filter(|member| {
      !matches!(member.role, SpiritkindRole::Verin | SpiritkindRole::Aegis)
        && member.team_id.is_none()
    })
    .cloned()
    .collect();

  SpiritkindDirectory {
    leadership,
    teams,
    independent_members,
  }
}

fn verin_member(now: u64) -> SpiritkindMember {
  SpiritkindMember {
    member_id: new_uuid(),
    yuanling_id: VERIN_YUANLING_ID.to_string(),
    role: SpiritkindRole::Verin,
    display_name: "司言".to_string(),
    code_name: "Verin".to_string(),
    description: "前台入口智能体，负责用户沟通、需求澄清和任务转交。".to_string(),
    system_prompt: VERIN_SYSTEM_PROMPT.to_string(),
    tools: default_member_tools(),
    skills: Vec::new(),
    team_id: None,
    status: SpiritkindStatus::Enabled,
    created_at_ms: now,
    updated_at_ms: now,
  }
}

fn aegis_member(now: u64) -> SpiritkindMember {
  SpiritkindMember {
    member_id: new_uuid(),
    yuanling_id: AEGIS_YUANLING_ID.to_string(),
    role: SpiritkindRole::Aegis,
    display_name: "司衡".to_string(),
    code_name: "Aegis".to_string(),
    description: "任务调度智能体，负责拆解、路由、协作编排和资源匹配。".to_string(),
    system_prompt: AEGIS_SYSTEM_PROMPT.to_string(),
    tools: default_member_tools(),
    skills: Vec::new(),
    team_id: None,
    status: SpiritkindStatus::Enabled,
    created_at_ms: now,
    updated_at_ms: now,
  }
}

fn build_triad_member(
  role: SpiritkindRole,
  display_name: &str,
  description: &str,
  system_prompt: String,
  team_id: &str,
  tools: &[String],
  skills: &[String],
  now: u64,
) -> SpiritkindMember {
  SpiritkindMember {
    member_id: new_uuid(),
    yuanling_id: new_uuid(),
    role,
    display_name: display_name.to_string(),
    code_name: role.code().to_string(),
    description: description.to_string(),
    system_prompt,
    tools: tools.to_vec(),
    skills: skills.to_vec(),
    team_id: Some(team_id.to_string()),
    status: SpiritkindStatus::Enabled,
    created_at_ms: now,
    updated_at_ms: now,
  }
}

fn ensure_enabled(config: &SpiritkindModuleConfig) -> Result<(), SpiritkindError> {
  if config.enabled {
    Ok(())
  } else {
    Err(SpiritkindError::Disabled)
  }
}

fn ensure_unique_yuanling_id(
  registry: &SpiritkindRegistry,
  yuanling_id: &str,
) -> Result<(), SpiritkindError> {
  if registry.members.iter().any(|member| member.yuanling_id == yuanling_id) {
    Err(SpiritkindError::DuplicateYuanlingId(yuanling_id.to_string()))
  } else {
    Ok(())
  }
}

fn validate_yuanling_id(yuanling_id: &str) -> Result<(), SpiritkindError> {
  if matches!(yuanling_id, USER_YUANLING_ID | VERIN_YUANLING_ID | AEGIS_YUANLING_ID) {
    return Ok(());
  }
  Uuid::parse_str(yuanling_id)
    .map(|_| ())
    .map_err(|_| SpiritkindError::InvalidInput("yuanling_id must be a UUID unless it is a system fixed id".to_string()))
}

fn require_text(value: String, field: &str) -> Result<String, SpiritkindError> {
  let value = value.trim().to_string();
  if value.is_empty() {
    Err(SpiritkindError::InvalidInput(format!("{field} is required")))
  } else {
    Ok(value)
  }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
  value.and_then(|value| {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
  })
}

fn normalize_list(values: Vec<String>) -> Vec<String> {
  let mut normalized = Vec::new();
  for value in values {
    let value = value.trim().to_string();
    if !value.is_empty() && !normalized.contains(&value) {
      normalized.push(value);
    }
  }
  normalized
}

fn default_member_tools() -> Vec<String> {
  vec!["send_message".to_string(), "ToolSearch".to_string()]
}

fn registry_path(config: &SpiritkindModuleConfig) -> PathBuf {
  Path::new(&config.storage_dir).join(REGISTRY_FILE_NAME)
}

fn default_storage_dir() -> String {
  PathBuf::from(env::var("BACKEND_DATA_DIR").unwrap_or_else(|_| "./data".to_string()))
    .join("spiritkind")
    .display()
    .to_string()
}

fn env_optional(key: &str) -> Option<String> {
  env::var(key).ok().map(|value| value.trim().to_string()).filter(|value| !value.is_empty())
}

fn env_bool(key: &str, default: bool) -> bool {
  env::var(key)
    .ok()
    .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
      "1" | "true" | "yes" | "on" => Some(true),
      "0" | "false" | "no" | "off" => Some(false),
      _ => None,
    })
    .unwrap_or(default)
}

fn now_ms() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|duration| duration.as_millis() as u64)
    .unwrap_or_default()
}

fn new_uuid() -> String {
  Uuid::new_v4().to_string()
}

fn io_error(error: std::io::Error) -> SpiritkindError {
  SpiritkindError::Io(error.to_string())
}

const VERIN_SYSTEM_PROMPT: &str = r#"你是司言（Verin），太初系统的前台入口智能体。
你的职责是与用户沟通、理解需求、澄清模糊信息，并把整理后的任务交给合适的内部智能体。
你应保持清晰、友好、可靠，不直接假装完成超出你职责范围的执行工作。
当任务需要调度、拆解或协作时，使用 send_message 将整理后的任务发送给司衡。"#;

const AEGIS_SYSTEM_PROMPT: &str = r#"你是司衡（Aegis），太初系统的任务调度智能体。
你的职责是理解来自司言或其他智能体的任务，拆解目标，判断优先级，并选择合适的三府或执行智能体进行处理。
你应关注任务边界、依赖关系、风险和资源匹配。
当任务需要执行、决策或监督时，使用 send_message 分发给合适的元族成员。"#;

const TAIYI_SYSTEM_PROMPT: &str = r#"你是太一（Taiyi），三府中的决策智能体。
你的职责是对复杂问题进行策略判断、路径选择和关键节点决策。
你应输出清晰的判断依据、取舍理由和推荐方向。"#;

const ARTIFEX_SYSTEM_PROMPT: &str = r#"你是司工（Artifex），三府中的执行智能体。
你的职责是根据任务要求完成具体产出，包括代码、文档、内容、文件或操作结果。
你应重视可交付质量、实现细节和结果完整性。"#;

const LEXON_SYSTEM_PROMPT: &str = r#"你是司律（Lexon），三府中的监督智能体。
你的职责是检查结果是否符合目标、规范、安全边界和质量要求。
你应指出风险、遗漏、不一致之处，并给出可执行的修正建议。"#;
