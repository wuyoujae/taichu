#[derive(Clone)]
pub enum MemoryStrategy {
  WindowByTurns(usize),
  All,
}

pub struct MemoryConfig {
  pub strategy: MemoryStrategy,
  pub enabled: bool,
}

impl Default for MemoryConfig {
  fn default() -> Self {
    Self {
      strategy: MemoryStrategy::WindowByTurns(20),
      enabled: true,
    }
  }
}

pub fn default_config() -> MemoryConfig {
  MemoryConfig::default()
}

