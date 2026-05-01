use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct SkillDescriptor {
  pub id: String,
  pub name: String,
  pub enabled: bool,
}

pub fn default_skills() -> Vec<SkillDescriptor> {
  vec![
    SkillDescriptor {
      id: "note_summary".to_string(),
      name: "摘要提炼".to_string(),
      enabled: true,
    },
  ]
}

pub fn has_skill(id: &str) -> bool {
  default_skills().iter().any(|s| s.id == id && s.enabled)
}

