use std::time::Duration;

use ureq::Agent;

use crate::config::AiConfig;
use crate::error::{AppError, Result};

/// Synchronous client for the Ollama local LLM API.
pub struct OllamaClient {
    base_url: String,
    model: String,
    agent: Agent,
}

impl OllamaClient {
    /// Create a client from config. Returns `None` if AI is disabled.
    pub fn from_config(config: &AiConfig) -> Option<Self> {
        if !config.enabled {
            return None;
        }
        let agent_config = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(config.timeout_secs)))
            .build();
        Some(Self {
            base_url: config.ollama_url.trim_end_matches('/').to_string(),
            model: config.ollama_model.clone(),
            agent: agent_config.into(),
        })
    }

    /// Check if Ollama is reachable by pinging `/api/tags`.
    pub fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        // Use a short timeout for the ping
        let ping_agent: Agent = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(5)))
            .build()
            .into();
        ping_agent.get(&url).call().is_ok()
    }

    /// Send a prompt to Ollama and return the generated text.
    pub fn generate(&self, prompt: &str) -> Result<String> {
        let url = format!("{}/api/generate", self.base_url);

        let body = serde_json::json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false
        });

        let mut response = self
            .agent
            .post(&url)
            .send_json(&body)
            .map_err(|e| AppError::Validation(format!("Ollama request failed: {e}")))?;

        let json: serde_json::Value = response
            .body_mut()
            .read_json()
            .map_err(|e| AppError::Validation(format!("Failed to parse Ollama response: {e}")))?;

        json.get("response")
            .and_then(|v: &serde_json::Value| v.as_str())
            .map(|s: &str| s.to_string())
            .ok_or_else(|| AppError::Validation("Ollama response missing 'response' field.".into()))
    }
}
