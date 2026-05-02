use axum::{
  extract::{Path as AxumPath, Query},
  routing::{get, post, put},
  Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_MAX_PROMPT_CHARS: usize = 40_000;
const DEFAULT_MAX_SEARCH_RESULTS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSource {
  DataDir,
  UserHome,
  Configured,
  Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillOrigin {
  SkillsDir,
  LegacyCommandsDir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillStatus {
  Active,
  Disabled,
  Deleted,
}

impl SkillStatus {
  pub fn code(self) -> u8 {
    match self {
      Self::Active => 1,
      Self::Disabled => 2,
      Self::Deleted => 3,
    }
  }

  pub fn label(self) -> &'static str {
    match self {
      Self::Active => "active",
      Self::Disabled => "disabled",
      Self::Deleted => "deleted",
    }
  }

  fn from_frontmatter(value: Option<String>) -> Self {
    let normalized = value.unwrap_or_default().trim().to_ascii_lowercase();
    match normalized.as_str() {
      "disabled" | "disable" | "2" => Self::Disabled,
      "deleted" | "delete" | "3" => Self::Deleted,
      _ => Self::Active,
    }
  }

  fn enabled(self) -> bool {
    matches!(self, Self::Active)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRoot {
  pub source: SkillSource,
  pub path: PathBuf,
  pub origin: SkillOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillDescriptor {
  pub id: String,
  pub name: String,
  pub description: Option<String>,
  pub source: SkillSource,
  pub origin: SkillOrigin,
  pub path: PathBuf,
  pub status: u8,
  pub status_label: String,
  pub enabled: bool,
  pub shadowed_by: Option<SkillSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillInjection {
  pub id: String,
  pub name: String,
  pub description: Option<String>,
  pub status: u8,
  pub status_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillLoadResult {
  pub id: String,
  pub name: String,
  pub description: Option<String>,
  pub path: PathBuf,
  pub args: Option<String>,
  pub prompt: String,
  pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSearchMatch {
  pub id: String,
  pub name: String,
  pub description: Option<String>,
  pub source: SkillSource,
  pub score: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSearchOutput {
  pub query: String,
  pub normalized_query: String,
  pub matches: Vec<SkillSearchMatch>,
  pub total_skills: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledSkill {
  pub id: String,
  pub name: String,
  pub source_path: PathBuf,
  pub installed_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillError {
  Disabled,
  UnknownSkill(String),
  DuplicateSkill(String),
  InvalidInput(String),
  Io(String),
}

impl Display for SkillError {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Disabled => write!(formatter, "skills module is disabled"),
      Self::UnknownSkill(name) => write!(formatter, "unknown skill `{name}`"),
      Self::DuplicateSkill(name) => write!(formatter, "duplicate runtime skill `{name}`"),
      Self::InvalidInput(message) => write!(formatter, "invalid skill input: {message}"),
      Self::Io(message) => write!(formatter, "{message}"),
    }
  }
}

impl std::error::Error for SkillError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSkillDefinition {
  pub id: String,
  pub name: String,
  pub description: Option<String>,
  pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsModuleConfig {
  pub enabled: bool,
  pub roots: Vec<SkillRoot>,
  pub allowed_skills: Option<BTreeSet<String>>,
  pub max_prompt_chars: usize,
  pub max_search_results: usize,
  pub auto_inject_enabled: bool,
  pub auto_inject_max_items: usize,
}

impl SkillsModuleConfig {
  pub fn as_view(&self, registry: &SkillRegistry) -> SkillsModuleConfigView {
    SkillsModuleConfigView {
      enabled: self.enabled,
      roots: self.roots.clone(),
      allowed_skills: self.allowed_skills.clone(),
      max_prompt_chars: self.max_prompt_chars,
      max_search_results: self.max_search_results,
      auto_inject_enabled: self.auto_inject_enabled,
      auto_inject_max_items: self.auto_inject_max_items,
      registered_count: registry.skills.len(),
      exposed_count: registry
        .injections(self.allowed_skills.as_ref())
        .map(|items| items.len())
        .unwrap_or(0),
      skills: registry
        .all_descriptors(self.allowed_skills.as_ref())
        .unwrap_or_default(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsModuleConfigView {
  pub enabled: bool,
  pub roots: Vec<SkillRoot>,
  pub allowed_skills: Option<BTreeSet<String>>,
  pub max_prompt_chars: usize,
  pub max_search_results: usize,
  pub auto_inject_enabled: bool,
  pub auto_inject_max_items: usize,
  pub registered_count: usize,
  pub exposed_count: usize,
  pub skills: Vec<SkillDescriptor>,
}

#[derive(Debug, Clone)]
pub struct SkillRegistry {
  skills: Vec<SkillRecord>,
}

#[derive(Debug, Clone)]
enum SkillContent {
  File(PathBuf),
  Inline(String),
}

#[derive(Debug, Clone)]
struct SkillRecord {
  descriptor: SkillDescriptor,
  content: SkillContent,
}

impl SkillRegistry {
  pub fn discover(config: &SkillsModuleConfig) -> Result<Self, SkillError> {
    if !config.enabled {
      return Ok(Self { skills: Vec::new() });
    }

    let mut records = Vec::new();
    let mut active = BTreeMap::<String, SkillSource>::new();

    for root in &config.roots {
      if !root.path.is_dir() {
        continue;
      }
      let mut root_records = load_records_from_root(root)?;
      root_records.sort_by(|left, right| left.descriptor.id.cmp(&right.descriptor.id));
      for mut record in root_records {
        let key = normalize_skill_name(&record.descriptor.id);
        if let Some(source) = active.get(&key) {
          record.descriptor.enabled = false;
          record.descriptor.shadowed_by = Some(*source);
        } else {
          active.insert(key, record.descriptor.source);
        }
        records.push(record);
      }
    }

    Ok(Self { skills: records })
  }

  pub fn with_runtime_skills(
    mut self,
    runtime_skills: Vec<RuntimeSkillDefinition>,
  ) -> Result<Self, SkillError> {
    let mut seen = self
      .skills
      .iter()
      .map(|record| normalize_skill_name(&record.descriptor.id))
      .collect::<BTreeSet<_>>();

    for skill in runtime_skills {
      let id = normalize_skill_id(&skill.id);
      let key = normalize_skill_name(&id);
      if !seen.insert(key) {
        return Err(SkillError::DuplicateSkill(id));
      }
      self.skills.push(SkillRecord {
        descriptor: SkillDescriptor {
          id,
          name: skill.name,
          description: skill.description,
          source: SkillSource::Runtime,
          origin: SkillOrigin::SkillsDir,
          path: PathBuf::new(),
          status: SkillStatus::Active.code(),
          status_label: SkillStatus::Active.label().to_string(),
          enabled: true,
          shadowed_by: None,
        },
        content: SkillContent::Inline(skill.prompt),
      });
    }

    Ok(self)
  }

  pub fn descriptors(
    &self,
    allowed_skills: Option<&BTreeSet<String>>,
  ) -> Result<Vec<SkillDescriptor>, SkillError> {
    Ok(self
      .skills
      .iter()
      .filter(|record| record.descriptor.enabled)
      .filter(|record| is_skill_allowed(&record.descriptor.id, allowed_skills))
      .map(|record| record.descriptor.clone())
      .collect())
  }

  pub fn all_descriptors(
    &self,
    allowed_skills: Option<&BTreeSet<String>>,
  ) -> Result<Vec<SkillDescriptor>, SkillError> {
    Ok(self
      .skills
      .iter()
      .filter(|record| is_skill_allowed(&record.descriptor.id, allowed_skills))
      .map(|record| record.descriptor.clone())
      .collect())
  }

  pub fn injections(
    &self,
    allowed_skills: Option<&BTreeSet<String>>,
  ) -> Result<Vec<SkillInjection>, SkillError> {
    Ok(self
      .descriptors(allowed_skills)?
      .into_iter()
      .map(|skill| SkillInjection {
        id: skill.id,
        name: skill.name,
        description: skill.description,
        status: skill.status,
        status_label: skill.status_label,
      })
      .collect())
  }

  pub fn search(&self, query: &str, max_results: usize) -> SkillSearchOutput {
    let query = query.trim().to_string();
    let normalized_query = normalize_skill_name(&query);
    let mut matches = self
      .skills
      .iter()
      .filter(|record| record.descriptor.enabled)
      .filter_map(|record| {
        let score = score_skill_match(&record.descriptor, &normalized_query);
        (score > 0 || normalized_query.is_empty()).then(|| SkillSearchMatch {
          id: record.descriptor.id.clone(),
          name: record.descriptor.name.clone(),
          description: record.descriptor.description.clone(),
          source: record.descriptor.source,
          score,
        })
      })
      .collect::<Vec<_>>();

    matches.sort_by(|left, right| {
      right
        .score
        .cmp(&left.score)
        .then_with(|| left.id.cmp(&right.id))
    });
    matches.truncate(max_results.max(1));

    SkillSearchOutput {
      query,
      normalized_query,
      total_skills: self.skills.iter().filter(|record| record.descriptor.enabled).count(),
      matches,
    }
  }

  pub fn load(
    &self,
    skill: &str,
    args: Option<String>,
    config: &SkillsModuleConfig,
  ) -> Result<SkillLoadResult, SkillError> {
    if !config.enabled {
      return Err(SkillError::Disabled);
    }
    let requested = normalize_skill_name(skill);
    let Some(record) = self.skills.iter().find(|record| {
      record.descriptor.enabled
        && (normalize_skill_name(&record.descriptor.id) == requested
          || normalize_skill_name(&record.descriptor.name) == requested)
        && is_skill_allowed(&record.descriptor.id, config.allowed_skills.as_ref())
    }) else {
      return Err(SkillError::UnknownSkill(skill.to_string()));
    };

    let contents = match &record.content {
      SkillContent::File(path) => fs::read_to_string(path).map_err(io_to_skill_error)?,
      SkillContent::Inline(prompt) => prompt.clone(),
    };
    let prompt = strip_frontmatter(&contents);
    let (prompt, truncated) = truncate_chars(&prompt, config.max_prompt_chars);

    Ok(SkillLoadResult {
      id: record.descriptor.id.clone(),
      name: record.descriptor.name.clone(),
      description: record.descriptor.description.clone(),
      path: record.descriptor.path.clone(),
      args,
      prompt,
      truncated,
    })
  }
}

pub fn resolve_from_env() -> SkillsModuleConfig {
  let roots = discover_default_roots();
  let allowed_skills = env_or_optional("YUANLING_SKILLS_ALLOWED")
    .and_then(|value| normalize_allowed_skills(&[value]).ok())
    .flatten();

  SkillsModuleConfig {
    enabled: env_or_bool("YUANLING_SKILLS_ENABLED", true),
    roots,
    allowed_skills,
    max_prompt_chars: env_or_usize("YUANLING_SKILLS_MAX_PROMPT_CHARS", DEFAULT_MAX_PROMPT_CHARS),
    max_search_results: env_or_usize(
      "YUANLING_SKILLS_MAX_SEARCH_RESULTS",
      DEFAULT_MAX_SEARCH_RESULTS,
    ),
    auto_inject_enabled: env_or_bool("YUANLING_SKILLS_AUTO_INJECT_ENABLED", true),
    auto_inject_max_items: env_or_usize("YUANLING_SKILLS_AUTO_INJECT_MAX_ITEMS", 20),
  }
}

pub fn build_context_injection(config: &SkillsModuleConfig) -> Result<Option<String>, SkillError> {
  if !config.enabled || !config.auto_inject_enabled {
    return Ok(None);
  }
  let registry = SkillRegistry::discover(config)?;
  let mut injections = registry.injections(config.allowed_skills.as_ref())?;
  injections.truncate(config.auto_inject_max_items.max(1));
  if injections.is_empty() {
    return Ok(None);
  }

  let mut lines = vec![
    "Available Yuanling skills are listed below. These are reusable local instructions, not executable tools. Use the Skill tool to load the full SKILL.md instructions only when a skill is relevant.".to_string(),
    String::new(),
  ];
  for skill in injections {
    let description = skill
      .description
      .unwrap_or_else(|| "No description provided.".to_string());
    lines.push(format!("- `{}` / {}: {}", skill.id, skill.name, description));
  }
  Ok(Some(lines.join("\n")))
}

pub fn default_skills() -> Vec<SkillDescriptor> {
  let config = resolve_from_env();
  SkillRegistry::discover(&config)
    .and_then(|registry| registry.descriptors(config.allowed_skills.as_ref()))
    .unwrap_or_default()
}

pub fn has_skill(id: &str) -> bool {
  let config = resolve_from_env();
  SkillRegistry::discover(&config)
    .and_then(|registry| registry.load(id, None, &config))
    .is_ok()
}

pub fn install_skill(source: &Path, config: &SkillsModuleConfig) -> Result<InstalledSkill, SkillError> {
  if !config.enabled {
    return Err(SkillError::Disabled);
  }
  let target_root = config
    .roots
    .iter()
    .find(|root| root.source == SkillSource::DataDir && root.origin == SkillOrigin::SkillsDir)
    .map(|root| root.path.clone())
    .unwrap_or_else(default_data_skill_root);
  fs::create_dir_all(&target_root).map_err(io_to_skill_error)?;

  let source_path = source.canonicalize().map_err(io_to_skill_error)?;
  let skill_path = if source_path.is_dir() {
    source_path.join("SKILL.md")
  } else {
    source_path.clone()
  };
  if !skill_path.is_file() {
    return Err(SkillError::InvalidInput(
      "skill source must be a SKILL.md file or a directory containing SKILL.md".to_string(),
    ));
  }

  let contents = fs::read_to_string(&skill_path).map_err(io_to_skill_error)?;
  let metadata = parse_skill_frontmatter(&contents);
  let fallback = source_path
    .file_stem()
    .or_else(|| source_path.file_name())
    .and_then(|value| value.to_str())
    .unwrap_or("skill");
  let id = normalize_skill_id(metadata.name.as_deref().unwrap_or(fallback));
  let installed_dir = target_root.join(&id);
  fs::create_dir_all(&installed_dir).map_err(io_to_skill_error)?;
  let installed_path = installed_dir.join("SKILL.md");
  fs::write(&installed_path, contents.as_bytes()).map_err(io_to_skill_error)?;

  Ok(InstalledSkill {
    id: id.clone(),
    name: metadata.name.unwrap_or(id),
    source_path,
    installed_path,
  })
}

pub fn set_skill_status(
  skill_id: &str,
  status: SkillStatus,
  config: &SkillsModuleConfig,
) -> Result<SkillDescriptor, SkillError> {
  if !config.enabled {
    return Err(SkillError::Disabled);
  }
  let registry = SkillRegistry::discover(config)?;
  let requested = normalize_skill_id(skill_id);
  let descriptor = registry
    .all_descriptors(None)?
    .into_iter()
    .find(|skill| skill.id == requested)
    .ok_or_else(|| SkillError::UnknownSkill(skill_id.to_string()))?;
  if descriptor.source == SkillSource::Runtime || descriptor.path.as_os_str().is_empty() {
    return Err(SkillError::InvalidInput(
      "runtime skills cannot be updated through file status".to_string(),
    ));
  }
  let contents = fs::read_to_string(&descriptor.path).map_err(io_to_skill_error)?;
  let updated = upsert_skill_status_frontmatter(&contents, status.label(), &descriptor);
  fs::write(&descriptor.path, updated.as_bytes()).map_err(io_to_skill_error)?;

  let refreshed = SkillRegistry::discover(config)?;
  refreshed
    .all_descriptors(None)?
    .into_iter()
    .find(|skill| skill.id == requested)
    .ok_or_else(|| SkillError::UnknownSkill(skill_id.to_string()))
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

#[derive(Debug, Deserialize)]
struct SkillSearchQuery {
  q: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SkillInstallRequest {
  source_path: String,
}

#[derive(Debug, Deserialize)]
struct SkillStatusRequest {
  status: u8,
}

fn skill_status_from_code(code: u8) -> Result<SkillStatus, SkillError> {
  match code {
    1 => Ok(SkillStatus::Active),
    2 => Ok(SkillStatus::Disabled),
    3 => Ok(SkillStatus::Deleted),
    _ => Err(SkillError::InvalidInput(
      "skill status must be 1(active), 2(disabled), or 3(deleted)".to_string(),
    )),
  }
}

fn discover_view() -> Result<SkillsModuleConfigView, SkillError> {
  let config = resolve_from_env();
  let registry = SkillRegistry::discover(&config)?;
  Ok(config.as_view(&registry))
}

async fn config() -> Json<ApiResponse<SkillsModuleConfigView>> {
  Json(match discover_view() {
    Ok(view) => ApiResponse::ok(view),
    Err(error) => ApiResponse::error(error.to_string()),
  })
}

async fn list() -> Json<ApiResponse<Vec<SkillDescriptor>>> {
  Json(match discover_view() {
    Ok(view) => ApiResponse::ok(view.skills),
    Err(error) => ApiResponse::error(error.to_string()),
  })
}

async fn search(Query(query): Query<SkillSearchQuery>) -> Json<ApiResponse<SkillSearchOutput>> {
  let config = resolve_from_env();
  let registry = match SkillRegistry::discover(&config) {
    Ok(registry) => registry,
    Err(error) => return Json(ApiResponse::error(error.to_string())),
  };
  let output = registry.search(
    query.q.as_deref().unwrap_or_default(),
    config.max_search_results,
  );
  Json(ApiResponse::ok(output))
}

async fn load(AxumPath(id): AxumPath<String>) -> Json<ApiResponse<SkillLoadResult>> {
  let config = resolve_from_env();
  let registry = match SkillRegistry::discover(&config) {
    Ok(registry) => registry,
    Err(error) => return Json(ApiResponse::error(error.to_string())),
  };
  Json(match registry.load(&id, None, &config) {
    Ok(skill) => ApiResponse::ok(skill),
    Err(error) => ApiResponse::error(error.to_string()),
  })
}

async fn install(Json(request): Json<SkillInstallRequest>) -> Json<ApiResponse<InstalledSkill>> {
  let config = resolve_from_env();
  Json(match install_skill(Path::new(&request.source_path), &config) {
    Ok(skill) => ApiResponse::ok(skill),
    Err(error) => ApiResponse::error(error.to_string()),
  })
}

async fn update_status(
  AxumPath(id): AxumPath<String>,
  Json(request): Json<SkillStatusRequest>,
) -> Json<ApiResponse<SkillDescriptor>> {
  let config = resolve_from_env();
  let status = match skill_status_from_code(request.status) {
    Ok(status) => status,
    Err(error) => return Json(ApiResponse::error(error.to_string())),
  };
  Json(match set_skill_status(&id, status, &config) {
    Ok(skill) => ApiResponse::ok(skill),
    Err(error) => ApiResponse::error(error.to_string()),
  })
}

pub fn router() -> Router {
  Router::new()
    .route("/yuanling/skills/config", get(config))
    .route("/yuanling/skills", get(list))
    .route("/yuanling/skills/search", get(search))
    .route("/yuanling/skills/install", post(install))
    .route("/yuanling/skills/{id}", get(load))
    .route("/yuanling/skills/{id}/status", put(update_status))
}

fn discover_default_roots() -> Vec<SkillRoot> {
  let mut roots = Vec::new();
  push_unique_root(
    &mut roots,
    SkillSource::DataDir,
    default_data_skill_root(),
    SkillOrigin::SkillsDir,
  );

  if let Some(raw_roots) = env_or_optional("YUANLING_SKILLS_ROOTS") {
    for raw in raw_roots.split(path_list_separator()).filter(|item| !item.trim().is_empty()) {
      push_unique_root(
        &mut roots,
        SkillSource::Configured,
        PathBuf::from(raw.trim()),
        SkillOrigin::SkillsDir,
      );
    }
  }

  if env_or_bool("YUANLING_SKILLS_INCLUDE_USER_HOME", true) {
    if let Some(home) = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE")) {
      let home = PathBuf::from(home);
      push_unique_root(
        &mut roots,
        SkillSource::UserHome,
        home.join(".taichu").join("skills"),
        SkillOrigin::SkillsDir,
      );
    }
  }

  roots
}

fn load_records_from_root(root: &SkillRoot) -> Result<Vec<SkillRecord>, SkillError> {
  let mut records = Vec::new();
  for entry in fs::read_dir(&root.path).map_err(io_to_skill_error)? {
    let entry = entry.map_err(io_to_skill_error)?;
    let path = entry.path();
    match root.origin {
      SkillOrigin::SkillsDir => {
        if !path.is_dir() {
          continue;
        }
        let skill_path = path.join("SKILL.md");
        if skill_path.is_file() {
          records.push(load_record_from_file(root, &skill_path, &path)?);
        }
      }
      SkillOrigin::LegacyCommandsDir => {
        if path.is_dir() {
          let skill_path = path.join("SKILL.md");
          if skill_path.is_file() {
            records.push(load_record_from_file(root, &skill_path, &path)?);
          }
        } else if path
          .extension()
          .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("md"))
        {
          records.push(load_record_from_file(root, &path, &path)?);
        }
      }
    }
  }
  Ok(records)
}

fn load_record_from_file(
  root: &SkillRoot,
  skill_path: &Path,
  fallback_path: &Path,
) -> Result<SkillRecord, SkillError> {
  let contents = fs::read_to_string(skill_path).map_err(io_to_skill_error)?;
  let metadata = parse_skill_frontmatter(&contents);
  let fallback = fallback_path
    .file_stem()
    .or_else(|| fallback_path.file_name())
    .and_then(|value| value.to_str())
    .unwrap_or("skill");
  let name = metadata.name.unwrap_or_else(|| fallback.to_string());
  let id = normalize_skill_id(&name);
  let status = SkillStatus::from_frontmatter(metadata.status);

  Ok(SkillRecord {
    descriptor: SkillDescriptor {
      id,
      name,
      description: metadata.description,
      source: root.source,
      origin: root.origin,
      path: skill_path.to_path_buf(),
      status: status.code(),
      status_label: status.label().to_string(),
      enabled: status.enabled(),
      shadowed_by: None,
    },
    content: SkillContent::File(skill_path.to_path_buf()),
  })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SkillFrontmatter {
  name: Option<String>,
  description: Option<String>,
  status: Option<String>,
}

fn parse_skill_frontmatter(contents: &str) -> SkillFrontmatter {
  let mut lines = contents.lines();
  if lines.next().map(str::trim) != Some("---") {
    return SkillFrontmatter::default();
  }

  let mut metadata = SkillFrontmatter::default();
  for line in lines {
    let trimmed = line.trim();
    if trimmed == "---" {
      break;
    }
    if let Some(value) = trimmed.strip_prefix("name:") {
      let value = unquote_frontmatter_value(value.trim());
      if !value.is_empty() {
        metadata.name = Some(value);
      }
      continue;
    }
    if let Some(value) = trimmed.strip_prefix("description:") {
      let value = unquote_frontmatter_value(value.trim());
      if !value.is_empty() {
        metadata.description = Some(value);
      }
      continue;
    }
    if let Some(value) = trimmed.strip_prefix("status:") {
      let value = unquote_frontmatter_value(value.trim());
      if !value.is_empty() {
        metadata.status = Some(value);
      }
    }
  }

  metadata
}

fn strip_frontmatter(contents: &str) -> String {
  let mut lines = contents.lines();
  if lines.next().map(str::trim) != Some("---") {
    return contents.to_string();
  }
  for line in lines.by_ref() {
    if line.trim() == "---" {
      return lines.collect::<Vec<_>>().join("\n").trim().to_string();
    }
  }
  contents.to_string()
}

fn upsert_skill_status_frontmatter(
  contents: &str,
  status_label: &str,
  descriptor: &SkillDescriptor,
) -> String {
  let lines = contents.lines().collect::<Vec<_>>();
  if lines.first().map(|line| line.trim()) != Some("---") {
    let description = descriptor.description.clone().unwrap_or_default();
    return format!(
      "---\nname: {}\ndescription: {}\nstatus: {}\n---\n\n{}",
      descriptor.name, description, status_label, contents
    );
  }

  let mut output = Vec::new();
  output.push("---".to_string());
  let mut status_written = false;
  let mut index = 1;
  while index < lines.len() {
    let line = lines[index];
    if line.trim() == "---" {
      if !status_written {
        output.push(format!("status: {status_label}"));
      }
      output.push("---".to_string());
      index += 1;
      break;
    }
    if line.trim_start().starts_with("status:") {
      output.push(format!("status: {status_label}"));
      status_written = true;
    } else {
      output.push(line.to_string());
    }
    index += 1;
  }
  output.extend(lines[index..].iter().map(|line| (*line).to_string()));
  output.join("\n")
}

fn unquote_frontmatter_value(value: &str) -> String {
  value
    .trim()
    .trim_matches('"')
    .trim_matches('\'')
    .trim()
    .to_string()
}

fn normalize_allowed_skills(values: &[String]) -> Result<Option<BTreeSet<String>>, SkillError> {
  let mut allowed = BTreeSet::new();
  for value in values {
    for token in value
      .split(|ch: char| ch == ',' || ch.is_whitespace())
      .filter(|token| !token.trim().is_empty())
    {
      allowed.insert(normalize_skill_id(token));
    }
  }
  Ok((!allowed.is_empty()).then_some(allowed))
}

fn is_skill_allowed(id: &str, allowed_skills: Option<&BTreeSet<String>>) -> bool {
  allowed_skills.is_none_or(|allowed| allowed.contains(&normalize_skill_id(id)))
}

fn score_skill_match(skill: &SkillDescriptor, normalized_query: &str) -> u32 {
  if normalized_query.is_empty() {
    return 1;
  }
  let id = normalize_skill_name(&skill.id);
  let name = normalize_skill_name(&skill.name);
  let description = skill
    .description
    .as_deref()
    .map(normalize_skill_name)
    .unwrap_or_default();

  if id == normalized_query || name == normalized_query {
    100
  } else if id.contains(normalized_query) || name.contains(normalized_query) {
    80
  } else if description.contains(normalized_query) {
    50
  } else {
    0
  }
}

fn normalize_skill_id(value: &str) -> String {
  normalize_skill_name(value)
    .replace(' ', "-")
    .replace('_', "-")
}

fn normalize_skill_name(value: &str) -> String {
  value.trim().to_ascii_lowercase().replace('_', "-")
}

fn truncate_chars(value: &str, max_chars: usize) -> (String, bool) {
  let mut output = String::new();
  for (index, ch) in value.chars().enumerate() {
    if index >= max_chars {
      return (output, true);
    }
    output.push(ch);
  }
  (output, false)
}

fn default_data_skill_root() -> PathBuf {
  env_or_optional("BACKEND_DATA_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from("./data"))
    .join("yuanling")
    .join("skills")
}

fn push_unique_root(
  roots: &mut Vec<SkillRoot>,
  source: SkillSource,
  path: PathBuf,
  origin: SkillOrigin,
) {
  if roots.iter().any(|root| root.path == path && root.origin == origin) {
    return;
  }
  roots.push(SkillRoot {
    source,
    path,
    origin,
  });
}

fn path_list_separator() -> char {
  if cfg!(windows) {
    ';'
  } else {
    ':'
  }
}

fn env_or_optional(key: &str) -> Option<String> {
  env::var(key).ok().filter(|value| !value.trim().is_empty())
}

fn env_or_bool(key: &str, default: bool) -> bool {
  env_or_optional(key)
    .and_then(|raw| raw.parse::<bool>().ok())
    .unwrap_or(default)
}

fn env_or_usize(key: &str, default: usize) -> usize {
  env_or_optional(key)
    .and_then(|raw| raw.parse::<usize>().ok())
    .unwrap_or(default)
}

fn io_to_skill_error(error: std::io::Error) -> SkillError {
  SkillError::Io(error.to_string())
}
