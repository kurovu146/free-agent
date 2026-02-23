use reqwest::Client;
use serde_json::json;

use super::gemini::{build_oai_messages, parse_oai_response};
use super::types::*;

pub struct MistralProvider {
    client: Client,
    model: String,
}

impl MistralProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            model: "mistral-small-latest".into(),
        }
    }
}

impl MistralProvider {
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
        }

        let resp = self
            .client
            .post("https://api.mistral.ai/v1/chat/completions")
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
