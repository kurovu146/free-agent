use serde::{Deserialize, Serialize};

/// A message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    ToolResult {
        tool_call_id: String,
        name: String,
        content: String,
    },
    AssistantWithToolCalls {
        text: Option<String>,
        tool_calls: Vec<ToolCall>,
    },
}

impl MessageContent {
    pub fn as_text(&self) -> &str {
        match self {
            MessageContent::Text(s) => s,
            MessageContent::ToolResult { content, .. } => content,
            MessageContent::AssistantWithToolCalls { text, .. } => {
                text.as_deref().unwrap_or("")
            }
        }
    }
}

/// Tool definition sent to the LLM
#[derive(Debug, Clone, Serialize)]
pub struct ToolDef {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDef,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// A tool call returned by the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

/// LLM response (either text content or tool calls)
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Usage,
}

#[derive(Debug, Clone, Default)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Rate limited")]
    RateLimited,
    #[error("Auth error: {0}")]
    AuthError(String),
    #[error("Request error: {0}")]
    RequestError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("No available keys")]
    NoKeys,
}
