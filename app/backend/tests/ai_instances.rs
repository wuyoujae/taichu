use std::{env, path::PathBuf, sync::Mutex};
use uuid::Uuid;

#[path = "../src/yuanling/ai/mod.rs"]
mod ai;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn isolated_storage() -> PathBuf {
  env::temp_dir().join(format!("taichu-ai-instances-{}", Uuid::new_v4()))
}

fn use_isolated_storage() -> std::sync::MutexGuard<'static, ()> {
  let guard = ENV_LOCK.lock().expect("env lock should not be poisoned");
  env::set_var("YUANLING_AI_INSTANCES_STORAGE_DIR", isolated_storage());
  guard
}

fn instance_request(name: &str, provider: &str, api_key: Option<&str>) -> ai::AiInstanceRequest {
  ai::AiInstanceRequest {
    name: name.to_string(),
    enabled: Some(true),
    provider: provider.to_string(),
    base_url: "https://api.example.com".to_string(),
    request_path: "/chat/completions".to_string(),
    api_key: api_key.map(str::to_string),
    model: "example-model".to_string(),
    prompt_template: "You are a test assistant.".to_string(),
    timeout_ms: 30_000,
    auth_header: "Authorization".to_string(),
    stream: Some(false),
    max_tokens: Some(2048),
    temperature: Some(0.7),
    top_p: Some(0.9),
    frequency_penalty: None,
    presence_penalty: None,
    stop: None,
    reasoning_effort: None,
  }
}

#[test]
fn missing_instance_file_returns_empty_list() {
  let _guard = use_isolated_storage();

  let instances = ai::list_ai_instances().expect("missing storage should return empty instances");

  assert!(instances.is_empty());
}

#[test]
fn create_instance_persists_and_hides_api_key_in_view() {
  let _guard = use_isolated_storage();

  let view = ai::create_ai_instance(instance_request(
    "DeepSeek Primary",
    "openai-compatible",
    Some("secret-key"),
  ))
  .expect("instance should be created");
  let registry = ai::load_ai_instances().expect("registry should reload");
  let public_json = serde_json::to_string(&view).expect("view should serialize");

  assert_eq!(registry.instances.len(), 1);
  assert_eq!(registry.instances[0].api_key, "secret-key");
  assert_eq!(registry.instances[0].max_tokens, Some(2048));
  assert!(view.has_api_key);
  assert_eq!(view.max_tokens, Some(2048));
  assert!(!public_json.contains("secret-key"));
}

#[test]
fn update_without_api_key_keeps_existing_key() {
  let _guard = use_isolated_storage();

  let created = ai::create_ai_instance(instance_request(
    "Initial",
    "openai-compatible",
    Some("original-secret"),
  ))
  .expect("instance should be created");
  let mut update = instance_request("Renamed", "openai-compatible", None);
  update.model = "updated-model".to_string();

  let updated = ai::update_ai_instance(&created.id, update).expect("instance should update");
  let config = ai::get_ai_instance_config(&created.id).expect("config should load");

  assert_eq!(updated.name, "Renamed");
  assert_eq!(updated.model, "updated-model");
  assert_eq!(config.api_key.as_deref(), Some("original-secret"));
  assert!(config.has_api_key);
}

#[test]
fn delete_instance_removes_it_from_list() {
  let _guard = use_isolated_storage();

  let created = ai::create_ai_instance(instance_request(
    "Temporary",
    "openai-compatible",
    Some("secret"),
  ))
  .expect("instance should be created");

  ai::delete_ai_instance(&created.id).expect("instance should delete");
  let instances = ai::list_ai_instances().expect("instances should reload");

  assert!(instances.iter().all(|instance| instance.id != created.id));
}

#[test]
fn provider_defaults_fill_empty_connection_fields() {
  let _guard = use_isolated_storage();

  let mut openai = instance_request("OpenAI Default", "openai-compatible", None);
  openai.base_url.clear();
  openai.request_path.clear();
  openai.auth_header.clear();
  openai.model.clear();
  let openai_view = ai::create_ai_instance(openai).expect("openai default should create");

  let mut anthropic = instance_request("Anthropic Default", "anthropic-compatible", None);
  anthropic.base_url.clear();
  anthropic.request_path.clear();
  anthropic.auth_header.clear();
  anthropic.model.clear();
  let anthropic_view = ai::create_ai_instance(anthropic).expect("anthropic default should create");

  assert_eq!(openai_view.base_url, "https://api.openai.com/v1");
  assert_eq!(openai_view.request_path, "/chat/completions");
  assert_eq!(openai_view.auth_header, "Authorization");
  assert_eq!(openai_view.model, "gpt-4o-mini");
  assert_eq!(anthropic_view.base_url, "https://api.anthropic.com");
  assert_eq!(anthropic_view.request_path, "/v1/messages");
  assert_eq!(anthropic_view.auth_header, "x-api-key");
  assert_eq!(anthropic_view.model, "claude-3-5-sonnet-20240620");
}
