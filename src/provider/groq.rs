use reqwest::Client;
use serde_json::json;

use super::gemini::{build_oai_messages, parse_oai_response};
use super::types::*;

pub struct GroqProvider {
    client: Client,
    model: String,
}

impl GroqProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            model: "llama-3.3-70b-versatile".into(),
        }
    }
}

impl GroqProvider {
    pub async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        api_key: &str,
    ) -> Result<LlmResponse, ProviderError> {
        let mut body = json!({
            "model": self.model,
            "messages": build_oai_messages(messages),
        });

        if !tools.is_empty() {
            body["tools"] = serde_json::to_value(tools)
                .map_err(|e| ProviderError::ParseError(e.to_string()))?;
            body["tool_choice"] = json!("auto");
        }

        let resp = self
            .client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::RequestError(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ProviderError::RateLimited);
        }
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ProviderError::AuthError(format!("HTTP {status}")));
        }
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::RequestError(format!("HTTP {status}: {text}")));
        }

        parse_oai_response(resp).await
    }
}
