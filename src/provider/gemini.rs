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
            model: "gemma-4-31b-it".into(),
        }
    }

    fn is_gemma(&self) -> bool {
        self.model.starts_with("gemma")
    }
}

impl GeminiProvider {
    pub async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        api_key: &str,
    ) -> Result<LlmResponse, ProviderError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            self.model
        );

        let is_gemma = self.is_gemma();
        let (contents, system_instruction) = build_google_contents(messages, is_gemma);

        let mut body = json!({ "contents": contents });

        if !is_gemma {
            if let Some(sys) = system_instruction {
                body["systemInstruction"] = sys;
            }
        }

        // Gemma models don't support function calling; skip tools for them.
        if !tools.is_empty() && !is_gemma {
            let decls: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.function.name,
                        "description": t.function.description,
                        "parameters": t.function.parameters,
                    })
                })
                .collect();
            body["tools"] = json!([{ "functionDeclarations": decls }]);
            body["toolConfig"] = json!({ "functionCallingConfig": { "mode": "AUTO" } });
        }

        let resp = self
            .client
            .post(&url)
            .header("x-goog-api-key", api_key)
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

        parse_google_response(resp).await
    }
}

fn build_google_contents(
    messages: &[Message],
    is_gemma: bool,
) -> (Vec<serde_json::Value>, Option<serde_json::Value>) {
    let system_texts: Vec<String> = messages
        .iter()
        .filter(|m| m.role == Role::System)
        .map(|m| m.content.as_text().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let system_instruction = if !system_texts.is_empty() && !is_gemma {
        Some(json!({ "parts": [{ "text": system_texts.join("\n\n") }] }))
    } else {
        None
    };

    // Gemma has no systemInstruction field — fold the system text into the first user turn.
    let gemma_system_prefix = if is_gemma && !system_texts.is_empty() {
        Some(format!("{}\n\n", system_texts.join("\n\n")))
    } else {
        None
    };
    let mut gemma_prefix_applied = false;

    let mut contents: Vec<serde_json::Value> = Vec::new();

    for m in messages {
        if m.role == Role::System {
            continue;
        }

        match &m.content {
            MessageContent::Text(text) => {
                let role = match m.role {
                    Role::User | Role::Tool => "user",
                    Role::Assistant => "model",
                    Role::System => unreachable!(),
                };
                let mut final_text = text.clone();
                if role == "user" && !gemma_prefix_applied {
                    if let Some(prefix) = &gemma_system_prefix {
                        final_text = format!("{prefix}{text}");
                        gemma_prefix_applied = true;
                    }
                }
                contents.push(json!({
                    "role": role,
                    "parts": [{ "text": final_text }]
                }));
            }
            MessageContent::UserWithImage { text, images } => {
                let mut parts: Vec<serde_json::Value> = Vec::new();
                for img in images {
                    parts.push(json!({
                        "inlineData": {
                            "mimeType": img.media_type,
                            "data": img.base64_data,
                        }
                    }));
                }
                let mut final_text = text.clone();
                if !gemma_prefix_applied {
                    if let Some(prefix) = &gemma_system_prefix {
                        final_text = format!("{prefix}{text}");
                        gemma_prefix_applied = true;
                    }
                }
                if !final_text.is_empty() {
                    parts.push(json!({ "text": final_text }));
                }
                contents.push(json!({ "role": "user", "parts": parts }));
            }
            MessageContent::ToolResult { name, content, .. } => {
                let response_value: serde_json::Value = serde_json::from_str(content)
                    .unwrap_or_else(|_| json!({ "result": content }));
                contents.push(json!({
                    "role": "user",
                    "parts": [{
                        "functionResponse": {
                            "name": name,
                            "response": response_value,
                        }
                    }]
                }));
            }
            MessageContent::AssistantWithToolCalls { text, tool_calls } => {
                let mut parts: Vec<serde_json::Value> = Vec::new();
                if let Some(t) = text {
                    if !t.is_empty() {
                        parts.push(json!({ "text": t }));
                    }
                }
                for tc in tool_calls {
                    let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                        .unwrap_or_else(|_| json!({}));
                    parts.push(json!({
                        "functionCall": {
                            "name": tc.function.name,
                            "args": args,
                        }
                    }));
                }
                contents.push(json!({ "role": "model", "parts": parts }));
            }
        }
    }

    (contents, system_instruction)
}

async fn parse_google_response(resp: reqwest::Response) -> Result<LlmResponse, ProviderError> {
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ProviderError::ParseError(e.to_string()))?;

    let candidate = body["candidates"]
        .get(0)
        .ok_or_else(|| ProviderError::ParseError("No candidates in response".into()))?;

    let parts = candidate["content"]["parts"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let mut text_parts: Vec<String> = Vec::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    for (idx, part) in parts.iter().enumerate() {
        if let Some(text) = part["text"].as_str() {
            if !text.is_empty() {
                text_parts.push(text.to_string());
            }
        } else if let Some(fc) = part.get("functionCall") {
            let name = fc["name"].as_str().unwrap_or("").to_string();
            let args = fc.get("args").cloned().unwrap_or(json!({}));
            let arguments = serde_json::to_string(&args)
                .map_err(|e| ProviderError::ParseError(e.to_string()))?;
            tool_calls.push(ToolCall {
                id: format!("call_{idx}"),
                function: ToolCallFunction { name, arguments },
            });
        }
    }

    let content = if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join(""))
    };

    let usage = Usage {
        prompt_tokens: body["usageMetadata"]["promptTokenCount"]
            .as_u64()
            .unwrap_or(0) as u32,
        completion_tokens: body["usageMetadata"]["candidatesTokenCount"]
            .as_u64()
            .unwrap_or(0) as u32,
    };

    Ok(LlmResponse {
        content,
        tool_calls,
        usage,
    })
}

// --- Shared OpenAI-compatible helpers (used by groq, mistral) ---

pub fn build_oai_messages(messages: &[Message]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .map(|m| {
            match &m.content {
                MessageContent::Text(text) => {
                    let role = match m.role {
                        Role::System => "system",
                        Role::User => "user",
                        Role::Assistant => "assistant",
                        Role::Tool => "tool",
                    };
                    json!({
                        "role": role,
                        "content": text,
                    })
                }
                MessageContent::UserWithImage { text, images } => {
                    let mut parts: Vec<serde_json::Value> = Vec::new();
                    for img in images {
                        parts.push(json!({
                            "type": "image_url",
                            "image_url": {
                                "url": format!("data:{};base64,{}", img.media_type, img.base64_data)
                            }
                        }));
                    }
                    if !text.is_empty() {
                        parts.push(json!({ "type": "text", "text": text }));
                    }
                    json!({ "role": "user", "content": parts })
                }
                MessageContent::ToolResult { tool_call_id, name, content } => json!({
                    "role": "tool",
                    "tool_call_id": tool_call_id,
                    "name": name,
                    "content": content,
                }),
                MessageContent::AssistantWithToolCalls { text, tool_calls } => {
                    let mut msg = json!({
                        "role": "assistant",
                        "tool_calls": tool_calls.iter().map(|tc| json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.function.name,
                                "arguments": tc.function.arguments,
                            }
                        })).collect::<Vec<_>>(),
                    });
                    if let Some(t) = text {
                        msg["content"] = json!(t);
                    }
                    msg
                }
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
