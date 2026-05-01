use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::env;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const CONTACT_VERSION: u32 = 1;
const DEFAULT_ENABLED: bool = true;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactModuleConfig {
  pub enabled: bool,
  pub storage_dir: String,
}

impl ContactModuleConfig {
  pub fn as_view(&self) -> ContactModuleConfigView {
    ContactModuleConfigView {
      enabled: self.enabled,
      storage_dir: self.storage_dir.clone(),
      statuses: vec![
        ContactStatusView::from(ContactStatus::Idle),
        ContactStatusView::from(ContactStatus::Busy),
        ContactStatusView::from(ContactStatus::Disabled),
      ],
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactModuleConfigView {
  pub enabled: bool,
  pub storage_dir: String,
  pub statuses: Vec<ContactStatusView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContactStatus {
  Idle,
  Busy,
  Disabled,
}

impl ContactStatus {
  pub fn code(self) -> u8 {
    match self {
      Self::Idle => 1,
      Self::Busy => 2,
      Self::Disabled => 3,
    }
  }

  pub fn label(self) -> &'static str {
    match self {
      Self::Idle => "idle",
      Self::Busy => "busy",
      Self::Disabled => "disabled",
    }
  }

  fn from_code(code: u8) -> Result<Self, ContactError> {
    match code {
      1 => Ok(Self::Idle),
      2 => Ok(Self::Busy),
      3 => Ok(Self::Disabled),
      _ => Err(ContactError::InvalidInput(format!("invalid contact status: {code}"))),
    }
  }
}

impl Serialize for ContactStatus {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_u8(self.code())
  }
}

impl<'de> Deserialize<'de> for ContactStatus {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct ContactStatusVisitor;

    impl Visitor<'_> for ContactStatusVisitor {
      type Value = ContactStatus;

      fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("a contact status code")
      }

      fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
      where
        E: de::Error,
      {
        let code = u8::try_from(value).map_err(|_| E::custom("contact status out of range"))?;
        ContactStatus::from_code(code).map_err(E::custom)
      }
    }

    deserializer.deserialize_u64(ContactStatusVisitor)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactStatusView {
  pub code: u8,
  pub label: String,
}

impl From<ContactStatus> for ContactStatusView {
  fn from(status: ContactStatus) -> Self {
    Self {
      code: status.code(),
      label: status.label().to_string(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactMessage {
  pub message_id: String,
  pub from_yuanling_id: String,
  pub to_yuanling_id: String,
  pub content: String,
  pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct YuanlingContact {
  pub version: u32,
  pub yuanling_id: String,
  pub status: ContactStatus,
  pub created_at_ms: u64,
  pub updated_at_ms: u64,
  pub pending_messages: Vec<ContactMessage>,
  pub inflight_messages: Vec<ContactMessage>,
}

impl YuanlingContact {
  pub fn new(yuanling_id: impl Into<String>) -> Self {
    let now = now_ms();
    Self {
      version: CONTACT_VERSION,
      yuanling_id: yuanling_id.into(),
      status: ContactStatus::Idle,
      created_at_ms: now,
      updated_at_ms: now,
      pending_messages: Vec::new(),
      inflight_messages: Vec::new(),
    }
  }

  fn touch(&mut self) {
    self.updated_at_ms = now_ms();
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactSendResult {
  pub status: String,
  pub message_id: String,
  pub from_yuanling_id: String,
  pub to_yuanling_id: String,
  pub receiver_status: u8,
  pub receiver_status_label: String,
  pub pending_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactTakeResult {
  pub status: String,
  pub receiver_status: u8,
  pub receiver_status_label: String,
  pub messages: Vec<ContactMessage>,
  pub pending_count: usize,
  pub inflight_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContactError {
  Disabled,
  InvalidInput(String),
  InvalidYuanlingId(String),
  Io(String),
  Serde(String),
}

impl Display for ContactError {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Disabled => write!(formatter, "contact module is disabled"),
      Self::InvalidInput(message) => write!(formatter, "{message}"),
      Self::InvalidYuanlingId(yuanling_id) => {
        write!(formatter, "invalid yuanling contact id: {yuanling_id}")
      }
      Self::Io(message) => write!(formatter, "{message}"),
      Self::Serde(message) => write!(formatter, "{message}"),
    }
  }
}

impl std::error::Error for ContactError {}

impl From<std::io::Error> for ContactError {
  fn from(error: std::io::Error) -> Self {
    Self::Io(error.to_string())
  }
}

impl From<serde_json::Error> for ContactError {
  fn from(error: serde_json::Error) -> Self {
    Self::Serde(error.to_string())
  }
}

pub fn resolve_from_env() -> ContactModuleConfig {
  ContactModuleConfig {
    enabled: env_or_bool("YUANLING_CONTACT_ENABLED", DEFAULT_ENABLED),
    storage_dir: env_or_optional("YUANLING_CONTACT_STORAGE_DIR")
      .unwrap_or_else(default_storage_dir),
  }
}

pub fn load_contact(
  yuanling_id: &str,
  config: &ContactModuleConfig,
) -> Result<YuanlingContact, ContactError> {
  ensure_enabled(config)?;
  validate_yuanling_id(yuanling_id)?;
  let path = contact_path(yuanling_id, config)?;
  if !path.exists() {
    return Ok(YuanlingContact::new(yuanling_id));
  }
  let contents = fs::read_to_string(path)?;
  let contact = serde_json::from_str::<YuanlingContact>(&contents)?;
  if contact.yuanling_id != yuanling_id {
    return Err(ContactError::InvalidInput(
      "contact file yuanling_id does not match requested id".to_string(),
    ));
  }
  Ok(contact)
}

pub fn save_contact(
  contact: &YuanlingContact,
  config: &ContactModuleConfig,
) -> Result<(), ContactError> {
  ensure_enabled(config)?;
  validate_yuanling_id(&contact.yuanling_id)?;
  fs::create_dir_all(&config.storage_dir)?;
  let path = contact_path(&contact.yuanling_id, config)?;
  let payload = serde_json::to_string_pretty(contact)?;
  fs::write(path, payload)?;
  Ok(())
}

pub fn set_contact_status(
  yuanling_id: &str,
  status: ContactStatus,
  config: &ContactModuleConfig,
) -> Result<YuanlingContact, ContactError> {
  let mut contact = load_contact(yuanling_id, config)?;
  contact.status = status;
  contact.touch();
  save_contact(&contact, config)?;
  Ok(contact)
}

pub fn send_message(
  from_yuanling_id: &str,
  to_yuanling_id: &str,
  content: &str,
  config: &ContactModuleConfig,
) -> Result<ContactSendResult, ContactError> {
  ensure_enabled(config)?;
  validate_yuanling_id(from_yuanling_id)?;
  validate_yuanling_id(to_yuanling_id)?;
  let content = content.trim();
  if content.is_empty() {
    return Err(ContactError::InvalidInput("content is required".to_string()));
  }

  let mut receiver = load_contact(to_yuanling_id, config)?;
  if receiver.status == ContactStatus::Disabled {
    return Err(ContactError::InvalidInput(format!(
      "yuanling `{to_yuanling_id}` contact is disabled"
    )));
  }

  let message = ContactMessage {
    message_id: Uuid::new_v4().to_string(),
    from_yuanling_id: from_yuanling_id.to_string(),
    to_yuanling_id: to_yuanling_id.to_string(),
    content: content.to_string(),
    created_at_ms: now_ms(),
  };
  let message_id = message.message_id.clone();
  receiver.pending_messages.push(message);
  receiver.touch();
  save_contact(&receiver, config)?;

  Ok(ContactSendResult {
    status: "queued".to_string(),
    message_id,
    from_yuanling_id: from_yuanling_id.to_string(),
    to_yuanling_id: to_yuanling_id.to_string(),
    receiver_status: receiver.status.code(),
    receiver_status_label: receiver.status.label().to_string(),
    pending_count: receiver.pending_messages.len(),
  })
}

pub fn take_ready_messages(
  yuanling_id: &str,
  config: &ContactModuleConfig,
) -> Result<ContactTakeResult, ContactError> {
  let mut contact = load_contact(yuanling_id, config)?;
  match contact.status {
    ContactStatus::Disabled => {
      return Ok(take_result("disabled", &contact, Vec::new()));
    }
    ContactStatus::Busy => {
      return Ok(take_result("blocked", &contact, Vec::new()));
    }
    ContactStatus::Idle => {}
  }

  if contact.pending_messages.is_empty() {
    return Ok(take_result("empty", &contact, Vec::new()));
  }

  let messages = std::mem::take(&mut contact.pending_messages);
  contact.inflight_messages = messages.clone();
  contact.status = ContactStatus::Busy;
  contact.touch();
  save_contact(&contact, config)?;
  Ok(take_result("ready", &contact, messages))
}

pub fn finish_contact_processing(
  yuanling_id: &str,
  config: &ContactModuleConfig,
) -> Result<YuanlingContact, ContactError> {
  let mut contact = load_contact(yuanling_id, config)?;
  contact.inflight_messages.clear();
  contact.status = ContactStatus::Idle;
  contact.touch();
  save_contact(&contact, config)?;
  Ok(contact)
}

fn take_result(
  status: &str,
  contact: &YuanlingContact,
  messages: Vec<ContactMessage>,
) -> ContactTakeResult {
  ContactTakeResult {
    status: status.to_string(),
    receiver_status: contact.status.code(),
    receiver_status_label: contact.status.label().to_string(),
    messages,
    pending_count: contact.pending_messages.len(),
    inflight_count: contact.inflight_messages.len(),
  }
}

fn ensure_enabled(config: &ContactModuleConfig) -> Result<(), ContactError> {
  if config.enabled {
    Ok(())
  } else {
    Err(ContactError::Disabled)
  }
}

fn contact_path(yuanling_id: &str, config: &ContactModuleConfig) -> Result<PathBuf, ContactError> {
  validate_yuanling_id(yuanling_id)?;
  Ok(Path::new(&config.storage_dir).join(format!("{yuanling_id}.json")))
}

fn validate_yuanling_id(yuanling_id: &str) -> Result<(), ContactError> {
  let valid = !yuanling_id.trim().is_empty()
    && yuanling_id != "."
    && yuanling_id != ".."
    && yuanling_id
      .chars()
      .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.');
  if valid {
    Ok(())
  } else {
    Err(ContactError::InvalidYuanlingId(yuanling_id.to_string()))
  }
}

fn default_storage_dir() -> String {
  PathBuf::from(env::var("BACKEND_DATA_DIR").unwrap_or_else(|_| "./data".to_string()))
    .join("yuanling")
    .join("contact")
    .to_string_lossy()
    .to_string()
}

fn now_ms() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_millis()
    .try_into()
    .unwrap_or(u64::MAX)
}

fn env_or_optional(key: &str) -> Option<String> {
  env::var(key)
    .ok()
    .map(|value| value.trim().to_string())
    .filter(|value| !value.is_empty())
}

fn env_or_bool(key: &str, default: bool) -> bool {
  env_or_optional(key)
    .and_then(|raw| raw.parse::<bool>().ok())
    .unwrap_or(default)
}

#[cfg(test)]
mod tests {
  use super::{
    finish_contact_processing, load_contact, save_contact, send_message, set_contact_status,
    take_ready_messages, ContactError, ContactModuleConfig, ContactStatus, YuanlingContact,
  };
  use std::fs;
  use std::path::PathBuf;
  use std::time::{SystemTime, UNIX_EPOCH};

  fn config(name: &str) -> ContactModuleConfig {
    let millis = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("clock should work")
      .as_millis();
    let storage_dir = std::env::temp_dir()
      .join(format!("taichu-contact-{name}-{millis}"))
      .to_string_lossy()
      .to_string();
    ContactModuleConfig {
      enabled: true,
      storage_dir,
    }
  }

  #[test]
  fn load_contact_creates_idle_contact_when_file_is_missing() {
    let config = config("missing");
    let contact = load_contact("receiver", &config).expect("contact should load");

    assert_eq!(contact.yuanling_id, "receiver");
    assert_eq!(contact.status, ContactStatus::Idle);
    assert!(contact.pending_messages.is_empty());
    assert!(contact.inflight_messages.is_empty());
  }

  #[test]
  fn send_message_queues_pending_message() {
    let config = config("send");
    let sent = send_message("sender", "receiver", "hello", &config).expect("message should send");
    let contact = load_contact("receiver", &config).expect("receiver should load");

    assert_eq!(sent.status, "queued");
    assert_eq!(contact.pending_messages.len(), 1);
    assert_eq!(contact.pending_messages[0].message_id, sent.message_id);
    assert_eq!(contact.pending_messages[0].from_yuanling_id, "sender");
    assert_eq!(contact.pending_messages[0].content, "hello");
  }

  #[test]
  fn disabled_target_rejects_new_messages() {
    let config = config("disabled");
    set_contact_status("receiver", ContactStatus::Disabled, &config)
      .expect("status should update");

    let result = send_message("sender", "receiver", "hello", &config);

    assert!(matches!(result, Err(ContactError::InvalidInput(_))));
  }

  #[test]
  fn busy_target_accepts_pending_but_take_is_blocked() {
    let config = config("busy");
    set_contact_status("receiver", ContactStatus::Busy, &config).expect("status should update");
    send_message("sender", "receiver", "hello", &config).expect("message should queue");

    let result = take_ready_messages("receiver", &config).expect("take should return blocked");
    let contact = load_contact("receiver", &config).expect("receiver should load");

    assert_eq!(result.status, "blocked");
    assert_eq!(contact.pending_messages.len(), 1);
    assert!(contact.inflight_messages.is_empty());
  }

  #[test]
  fn idle_target_moves_pending_to_inflight_and_becomes_busy() {
    let config = config("take");
    send_message("sender", "receiver", "first", &config).expect("first should queue");
    send_message("sender", "receiver", "second", &config).expect("second should queue");

    let result = take_ready_messages("receiver", &config).expect("messages should be ready");
    let contact = load_contact("receiver", &config).expect("receiver should load");

    assert_eq!(result.status, "ready");
    assert_eq!(result.messages.len(), 2);
    assert_eq!(contact.status, ContactStatus::Busy);
    assert!(contact.pending_messages.is_empty());
    assert_eq!(contact.inflight_messages.len(), 2);
  }

  #[test]
  fn finish_contact_processing_clears_inflight_and_restores_idle() {
    let config = config("finish");
    send_message("sender", "receiver", "hello", &config).expect("message should queue");
    take_ready_messages("receiver", &config).expect("messages should be ready");

    let contact = finish_contact_processing("receiver", &config).expect("finish should work");

    assert_eq!(contact.status, ContactStatus::Idle);
    assert!(contact.inflight_messages.is_empty());
  }

  #[test]
  fn storage_path_rejects_path_traversal_yuanling_ids() {
    let config = config("invalid");
    let result = load_contact("../bad", &config);

    assert!(matches!(result, Err(ContactError::InvalidYuanlingId(_))));
  }

  #[test]
  fn save_and_load_contact_snapshot() {
    let config = config("snapshot");
    let mut contact = YuanlingContact::new("receiver");
    contact.status = ContactStatus::Busy;
    save_contact(&contact, &config).expect("contact should save");

    let restored = load_contact("receiver", &config).expect("contact should load");
    let path = PathBuf::from(&config.storage_dir).join("receiver.json");

    assert_eq!(restored.status, ContactStatus::Busy);
    assert!(path.exists());
    assert!(fs::read_to_string(path).expect("snapshot should read").contains("\"status\": 2"));
  }
}
