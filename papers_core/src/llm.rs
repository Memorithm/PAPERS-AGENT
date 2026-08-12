use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub timeout_secs: u64,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: "ollama".into(),
            model: "gemma4:e2b".into(),
            base_url: "http://localhost:11434".into(),
            api_key: "EMPTY".into(),
            timeout_secs: 300,
            max_tokens: 2048,
            temperature: 0.3,
        }
    }
}

pub struct LlmClient {
    client: Client,
    config: LlmConfig,
}

impl LlmClient {
    pub fn new(config: LlmConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");
        Self { client, config }
    }

    pub fn is_available(&self) -> bool {
        if self.config.provider == "ollama" {
            self.client
                .get(format!("{}/api/tags", self.config.base_url))
                .send()
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        } else {
            true
        }
    }

    pub fn generate(&self, prompt: &str, system: Option<&str>) -> Result<String, String> {
        if self.config.provider == "ollama" {
            self.generate_ollama(prompt, system)
        } else {
            self.generate_openai(prompt, system)
        }
    }

    pub fn generate_json(
        &self,
        prompt: &str,
        system: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        let raw = self.generate(prompt, system)?;
        serde_json::from_str(&raw).or_else(|_| {
            // Try to extract JSON from markdown
            if let Some(start) = raw.find('{') {
                if let Some(end) = raw.rfind('}') {
                    serde_json::from_str(&raw[start..=end])
                        .map_err(|e| format!("JSON parse: {}", e))
                } else {
                    Err("No closing brace".into())
                }
            } else {
                Err("No JSON found".into())
            }
        })
    }

    fn generate_ollama(&self, prompt: &str, system: Option<&str>) -> Result<String, String> {
        let mut body = serde_json::json!({
            "model": self.config.model,
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": self.config.temperature,
                "num_predict": self.config.max_tokens,
            }
        });
        if let Some(sys) = system {
            body["system"] = serde_json::Value::String(sys.to_string());
        }

        let response = self
            .client
            .post(format!("{}/api/generate", self.config.base_url))
            .json(&body)
            .send()
            .map_err(|e| format!("Ollama request failed: {}", e))?;

        let data: serde_json::Value = response.json().map_err(|e| format!("Parse error: {}", e))?;
        let text = data["response"].as_str().unwrap_or("");
        let thinking = data["thinking"].as_str().unwrap_or("");
        Ok(if text.is_empty() { thinking } else { text }.to_string())
    }

    fn generate_openai(&self, prompt: &str, system: Option<&str>) -> Result<String, String> {
        let mut messages = Vec::new();
        if let Some(sys) = system {
            messages.push(serde_json::json!({"role": "system", "content": sys}));
        }
        messages.push(serde_json::json!({"role": "user", "content": prompt}));

        let body = serde_json::json!({
            "model": self.config.model,
            "messages": messages,
            "temperature": self.config.temperature,
            "max_tokens": self.config.max_tokens,
        });

        let response = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&body)
            .send()
            .map_err(|e| format!("OpenAI request failed: {}", e))?;

        let data: serde_json::Value = response.json().map_err(|e| format!("Parse error: {}", e))?;
        Ok(data["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }
}
