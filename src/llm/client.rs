use crate::config::AnthropicConfig;
use crate::error::{BotError, Result};
use secrecy::ExposeSecret;

pub struct AnthropicClient {
    http: reqwest::Client,
    api_key: String,
    model: String,
    max_tokens: u32,
}

impl AnthropicClient {
    pub fn new(config: &AnthropicConfig) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: config.api_key.expose_secret().to_string(),
            model: config.model.clone(),
            max_tokens: config.max_tokens,
        }
    }

    pub async fn complete(&self, prompt: &str) -> Result<String> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            "messages": [
                {"role": "user", "content": prompt}
            ]
        });

        let resp = self
            .http
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(BotError::AnthropicApi {
                status,
                message: body,
            });
        }

        let data: serde_json::Value = resp.json().await?;
        let text = data["content"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|block| block["text"].as_str())
            .unwrap_or("")
            .to_string();

        Ok(text)
    }
}
