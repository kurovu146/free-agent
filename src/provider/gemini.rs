use reqwest::Client;
use serde_json::json;

use super::types::*;

pub struct GeminiProvider {
    client: Client,
    model: String,
}

impl GeminiProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            model: "gemini-2.5-flash".into(),
        }
    }
}

impl GeminiProvider {
    pub async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        api_key: &str,
    ) -> Result<LlmResponse, ProviderError> {
        // Gemini OpenAI-compatible endpoint
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
        );

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
            .post(&url)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::RequestError(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ProviderError::RateLimited);
        }
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::AuthError(format!("HTTP {status}")));
        }
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::RequestError(format!("HTTP {status}: {text}")));
        }

        parse_oai_response(resp).await
    }
}

// --- Shared OpenAI-compatible helpers ---

pub fn build_oai_messages(messages: &[Message]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .map(|m| {
            let role = match m.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool => "tool",
            };
            match &m.content {
                MessageContent::Text(text) => json!({
                    "role": role,
                    "content": text,
                }),
                MessageContent::ToolResult { tool_call_id, content } => json!({
                    "role": "tool",
                    "tool_call_id": tool_call_id,
                    "content": content,
                }),
            }
        })
        .collect()
}

pub async fn parse_oai_response(resp: reqwest::Response) -> Result<LlmResponse, ProviderError> {
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ProviderError::ParseError(e.to_string()))?;

    let choice = body["choices"]
        .get(0)
        .ok_or_else(|| ProviderError::ParseError("No choices in response".into()))?;

    let message = &choice["message"];

    let content = message["content"].as_str().map(|s| s.to_string());

    let tool_calls: Vec<ToolCall> = if let Some(tcs) = message["tool_calls"].as_array() {
        tcs.iter()
            .filter_map(|tc| serde_json::from_value(tc.clone()).ok())
            .collect()
    } else {
        vec![]
    };

    let usage = Usage {
        prompt_tokens: body["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
        completion_tokens: body["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
    };

    Ok(LlmResponse {
        content,
        tool_calls,
        usage,
    })
}
