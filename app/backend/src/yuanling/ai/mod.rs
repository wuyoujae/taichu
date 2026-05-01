pub type TopP = f64;
pub type Temperature = f64;

#[derive(Clone)]
pub struct AiModelConfig {
  pub provider: String,
  pub model_id: String,
  pub prompt_template: String,
  pub top_p: TopP,
  pub temperature: Temperature,
  pub timeout_ms: u64,
}

impl Default for AiModelConfig {
  fn default() -> Self {
    Self {
      provider: "none".to_string(),
      model_id: "local-default".to_string(),
      prompt_template: "You are YUANLING, a practical assistant.".to_string(),
      top_p: 0.95,
      temperature: 0.7,
      timeout_ms: 60000,
    }
  }
}

#[derive(Clone)]
pub struct AiModuleConfig {
  pub enabled: bool,
  pub model: AiModelConfig,
}

pub fn default_config() -> AiModuleConfig {
  AiModuleConfig {
    enabled: true,
    model: AiModelConfig::default(),
  }
}

