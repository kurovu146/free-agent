use reqwest::Client;
use serde_json::json;
use tracing::debug;

use super::types::*;

pub struct ClaudeProvider {
    client: Client,
    model: String,
}

impl ClaudeProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            model: "claude-sonnet-4-20250514".into(),
        }
    }

    pub fn with_model(model: &str) -> Self {
        Self {
            client: Client::new(),
            model: model.into(),
        }
    }

    pub async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        api_key: &str,
    ) -> Result<LlmResponse, ProviderError> {
        let (system_prompt, api_messages) = build_claude_messages(messages);

        let mut body = json!({
            "model": self.model,
            "max_tokens": 8192,
            "messages": api_messages,
        });

        if !system_prompt.is_empty() {
            body["system"] = json!(system_prompt);
        }

        if !tools.is_empty() {
            body["tools"] = build_claude_tools(tools);
        }

        debug!("Claude API request: model={}", self.model);

        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::RequestError(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ProviderError::RateLimited);
        }
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ProviderError::AuthError("Invalid API key".into()));
        }
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::RequestError(format!("HTTP {status}: {text}")));
        }

        parse_claude_response(resp).await
    }
}

/// Convert our generic Message format to Claude API format.
/// Claude separates system prompt from messages, and uses content blocks.
fn build_claude_messages(messages: &[Message]) -> (String, Vec<serde_json::Value>) {
    let mut system_prompt = String::new();
    let mut api_messages: Vec<serde_json::Value> = Vec::new();

    for msg in messages {
        match (&msg.role, &msg.content) {
            // System messages → extract as system prompt
            (Role::System, MessageContent::Text(text)) => {
                if !system_prompt.is_empty() {
                    system_prompt.push_str("\n\n");
                }
                system_prompt.push_str(text);
            }
            // User messages → simple text
            (Role::User, MessageContent::Text(text)) => {
                api_messages.push(json!({
                    "role": "user",
                    "content": text,
                }));
            }
            // Assistant text-only → simple text
            (Role::Assistant, MessageContent::Text(text)) => {
                api_messages.push(json!({
                    "role": "assistant",
                    "content": text,
                }));
            }
            // Assistant with tool calls → content blocks
            (Role::Assistant, MessageContent::AssistantWithToolCalls { text, tool_calls }) => {
                let mut content_blocks: Vec<serde_json::Value> = Vec::new();

                if let Some(t) = text {
                    if !t.is_empty() {
                        content_blocks.push(json!({
                            "type": "text",
                            "text": t,
                        }));
                    }
                }

                for tc in tool_calls {
                    // Parse arguments JSON string into Value
                    let input: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                        .unwrap_or(json!({}));

                    content_blocks.push(json!({
                        "type": "tool_use",
                        "id": tc.id,
                        "name": tc.function.name,
                        "input": input,
                    }));
                }

                api_messages.push(json!({
                    "role": "assistant",
                    "content": content_blocks,
                }));
            }
            // Tool results → user message with tool_result content block
            (Role::Tool, MessageContent::ToolResult { tool_call_id, content, .. }) => {
                // Claude expects tool results as user messages with tool_result content blocks.
                // If the previous message is already a user message with tool_result blocks,
                // append to it. Otherwise create a new user message.
                let result_block = json!({
                    "type": "tool_result",
                    "tool_use_id": tool_call_id,
                    "content": content,
                });

                // Check if we can merge with previous user message containing tool_results
                let should_merge = api_messages.last().map_or(false, |last| {
                    last["role"] == "user"
                        && last["content"].is_array()
                        && last["content"]
                            .as_array()
                            .map_or(false, |arr| {
                                arr.first()
                                    .map_or(false, |b| b["type"] == "tool_result")
                            })
                });

                if should_merge {
                    if let Some(last) = api_messages.last_mut() {
                        if let Some(arr) = last["content"].as_array_mut() {
                            arr.push(result_block);
                        }
                    }
                } else {
                    api_messages.push(json!({
                        "role": "user",
                        "content": [result_block],
                    }));
                }
            }
            _ => {}
        }
    }

    (system_prompt, api_messages)
}

/// Convert our generic ToolDef format to Claude tool format.
/// Claude uses { name, description, input_schema } instead of OpenAI's { type: "function", function: { ... } }
fn build_claude_tools(tools: &[ToolDef]) -> serde_json::Value {
    let claude_tools: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| {
            json!({
                "name": t.function.name,
                "description": t.function.description,
                "input_schema": t.function.parameters,
            })
        })
        .collect();

    json!(claude_tools)
}

/// Parse Claude API response into our generic LlmResponse.
async fn parse_claude_response(resp: reqwest::Response) -> Result<LlmResponse, ProviderError> {
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ProviderError::ParseError(e.to_string()))?;

    let mut text_parts: Vec<String> = Vec::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    if let Some(content) = body["content"].as_array() {
        for block in content {
            match block["type"].as_str() {
                Some("text") => {
                    if let Some(t) = block["text"].as_str() {
                        text_parts.push(t.to_string());
                    }
                }
                Some("tool_use") => {
                    let id = block["id"].as_str().unwrap_or("").to_string();
                    let name = block["name"].as_str().unwrap_or("").to_string();
                    let input = &block["input"];

                    tool_calls.push(ToolCall {
                        id,
                        function: ToolCallFunction {
                            name,
                            arguments: serde_json::to_string(input).unwrap_or_default(),
                        },
                    });
                }
                _ => {}
            }
        }
    }

    let content = if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join("\n"))
    };

    let usage = Usage {
        prompt_tokens: body["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
        completion_tokens: body["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
    };

    Ok(LlmResponse {
        content,
        tool_calls,
        usage,
    })
}
