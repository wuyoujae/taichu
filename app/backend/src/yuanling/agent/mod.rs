use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentRequest {
  pub user_input: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentResponse {
  pub reply: String,
}

#[derive(Default)]
pub struct AgentState {
  pub activated: bool,
}

pub fn activate() -> AgentState {
  AgentState { activated: true }
}

pub async fn run_once(_request: AgentRequest, _state: &mut AgentState) -> AgentResponse {
  AgentResponse {
    reply: "yuanling foundation not fully implemented yet".to_string(),
  }
}

