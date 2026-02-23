use tracing::{debug, info, warn};

use crate::db::Database;
use crate::provider::{Message, MessageContent, ProviderPool, Role};
use crate::tools::gmail::GmailCreds;

use super::tool_registry::ToolRegistry;

/// Progress updates sent during agent execution.
pub enum AgentProgress {
    /// A tool is about to be executed.
    ToolUse(String),
    /// LLM is being called (new turn starting).
    Thinking,
}

/// Result of an agent loop execution.
pub struct AgentResult {
    pub response: String,
    pub tools_used: Vec<String>,
    pub provider: String,
}

pub struct AgentLoop;

impl AgentLoop {
    /// Run the agent loop: send messages to LLM, execute tool calls, repeat.
    /// Calls `on_progress` between turns so the caller can update the UI.
    pub async fn run<F>(
        pool: &ProviderPool,
        system_prompt: &str,
        user_message: &str,
        user_id: u64,
        db: &Database,
        gmail_creds: &GmailCreds,
        system_tools_enabled: bool,
        working_dir: &str,
        bash_timeout: u64,
        max_turns: usize,
        on_progress: F,
    ) -> Result<AgentResult, String>
    where
        F: Fn(AgentProgress),
    {
        let tools = ToolRegistry::definitions(gmail_creds.is_configured(), system_tools_enabled);
        let mut tools_used: Vec<String> = Vec::new();
        let mut last_provider = String::new();

        let mut messages = vec![
            Message {
                role: Role::System,
                content: MessageContent::Text(system_prompt.to_string()),
            },
            Message {
                role: Role::User,
                content: MessageContent::Text(user_message.to_string()),
            },
        ];

        for turn in 0..max_turns {
            debug!("Agent turn {}/{}", turn + 1, max_turns);
            on_progress(AgentProgress::Thinking);

            let (response, provider_name) = pool
                .chat(&messages, &tools)
                .await
                .map_err(|e| format!("LLM error: {e}"))?;

            last_provider = provider_name;

            // If no tool calls, return the text content
            if response.tool_calls.is_empty() {
                let content = response.content.unwrap_or_default();
                info!(
                    "Agent completed in {} turns via {} ({} + {} tokens)",
                    turn + 1,
                    last_provider,
                    response.usage.prompt_tokens,
                    response.usage.completion_tokens
                );
                // Deduplicate tools
                tools_used.sort();
                tools_used.dedup();
                return Ok(AgentResult {
                    response: content,
                    tools_used,
                    provider: last_provider,
                });
            }

            // Add assistant message with tool calls to history
            messages.push(Message {
                role: Role::Assistant,
                content: MessageContent::AssistantWithToolCalls {
                    text: response.content.clone(),
                    tool_calls: response.tool_calls.clone(),
                },
            });

            // Execute each tool call and add results
            for tc in &response.tool_calls {
                let tool_name = &tc.function.name;
                debug!("Executing tool: {tool_name}({})", tc.function.arguments);

                // Track tool usage + notify caller
                tools_used.push(tool_name.clone());
                on_progress(AgentProgress::ToolUse(tool_name.clone()));

                let result = ToolRegistry::execute(
                    tool_name,
                    &tc.function.arguments,
                    user_id,
                    db,
                    gmail_creds,
                    working_dir,
                    bash_timeout,
                )
                .await;

                messages.push(Message {
                    role: Role::Tool,
                    content: MessageContent::ToolResult {
                        tool_call_id: tc.id.clone(),
                        name: tool_name.clone(),
                        content: result,
                    },
                });
            }
        }

        warn!("Agent hit max turns ({max_turns})");
        let last_assistant = messages
            .iter()
            .rev()
            .find(|m| m.role == Role::Assistant)
            .map(|m| m.content.as_text().to_string())
            .unwrap_or_else(|| "Reached max processing limit. Please try again.".into());

        tools_used.sort();
        tools_used.dedup();
        Ok(AgentResult {
            response: last_assistant,
            tools_used,
            provider: last_provider,
        })
    }
}
