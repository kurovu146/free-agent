use tracing::{debug, info, warn};

use crate::db::Database;
use crate::provider::{Message, MessageContent, ProviderPool, Role};
use crate::tools::gmail::GmailCreds;

use super::tool_registry::ToolRegistry;

pub struct AgentLoop;

impl AgentLoop {
    /// Run the agent loop: send messages to LLM, execute tool calls, repeat
    pub async fn run(
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
    ) -> Result<String, String> {
        let tools = ToolRegistry::definitions(gmail_creds.is_configured(), system_tools_enabled);

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

            let (response, provider_name) = pool
                .chat(&messages, &tools)
                .await
                .map_err(|e| format!("LLM error: {e}"))?;

            // If no tool calls, return the text content
            if response.tool_calls.is_empty() {
                let content = response.content.unwrap_or_default();
                info!(
                    "Agent completed in {} turns via {} ({} + {} tokens)",
                    turn + 1,
                    provider_name,
                    response.usage.prompt_tokens,
                    response.usage.completion_tokens
                );
                return Ok(content);
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
                debug!("Executing tool: {}({})", tc.function.name, tc.function.arguments);

                let result = ToolRegistry::execute(
                    &tc.function.name,
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
                        name: tc.function.name.clone(),
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

        Ok(last_assistant)
    }
}
