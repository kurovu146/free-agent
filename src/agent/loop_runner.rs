use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, info, warn};

use crate::db::Database;
use crate::provider::{Message, MessageContent, ProviderPool, Role};
use crate::tools::gmail::GmailCreds;
use crate::tools::claude_code::ClaudeCodeManager;

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
    /// Deduplicated list of tools used.
    pub tools_used: Vec<String>,
    /// Counts for each tool (parallel to tools_used).
    pub tools_count: Vec<usize>,
    pub provider: String,
    pub turns: usize,
}

pub struct AgentLoop;

impl AgentLoop {
    /// Run the agent loop: send messages to LLM, execute tool calls, repeat.
    /// Calls `on_progress` between turns so the caller can update the UI.
    pub async fn run<F>(
        pool: &ProviderPool,
        system_prompt: &str,
        user_content: MessageContent,
        user_id: u64,
        db: &Database,
        gmail_creds: &GmailCreds,
        system_tools_enabled: bool,
        working_dir: &str,
        bash_timeout: u64,
        max_turns: usize,
        history: Vec<Message>,
        preferred_provider: Option<&str>,
        cc_manager: Option<&ClaudeCodeManager>,
        cancel_flag: &Arc<AtomicBool>,
        on_progress: F,
    ) -> Result<AgentResult, String>
    where
        F: Fn(AgentProgress),
    {
        let tools = ToolRegistry::definitions(gmail_creds.is_configured(), system_tools_enabled, cc_manager.is_some());
        let mut tools_used: Vec<String> = Vec::new();
        let mut last_provider = String::new();

        // Build messages: system + history + current user message
        let mut messages = vec![Message {
            role: Role::System,
            content: MessageContent::Text(system_prompt.to_string()),
        }];
        messages.extend(history);
        messages.push(Message {
            role: Role::User,
            content: user_content,
        });

        for turn in 0..max_turns {
            // Check cancel flag before each turn
            if cancel_flag.load(Ordering::Relaxed) {
                info!("Agent cancelled by user at turn {}", turn + 1);
                let last_text = messages
                    .iter()
                    .rev()
                    .find(|m| m.role == Role::Assistant)
                    .map(|m| m.content.as_text().to_string())
                    .unwrap_or_default();
                let (deduped, counts) = dedup_with_counts(&tools_used);
                return Ok(AgentResult {
                    response: if last_text.is_empty() {
                        "Query đã bị dừng.".into()
                    } else {
                        format!("{last_text}\n\n_— Query đã bị dừng._")
                    },
                    tools_used: deduped,
                    tools_count: counts,
                    provider: last_provider.clone(),
                    turns: turn,
                });
            }

            debug!("Agent turn {}/{}", turn + 1, max_turns);
            on_progress(AgentProgress::Thinking);

            let (response, provider_name) = match preferred_provider {
                Some(name) => pool.chat_with_provider(&messages, &tools, name).await,
                None => pool.chat(&messages, &tools).await,
            }
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
                let (deduped, counts) = dedup_with_counts(&tools_used);
                return Ok(AgentResult {
                    response: content,
                    tools_used: deduped,
                    tools_count: counts,
                    provider: last_provider,
                    turns: turn + 1,
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
                    cc_manager,
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

        let (deduped, counts) = dedup_with_counts(&tools_used);
        Ok(AgentResult {
            response: last_assistant,
            tools_used: deduped,
            tools_count: counts,
            provider: last_provider,
            turns: max_turns,
        })
    }
}

/// Deduplicate a list of tool names while counting occurrences.
fn dedup_with_counts(tools: &[String]) -> (Vec<String>, Vec<usize>) {
    use std::collections::BTreeMap;
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for t in tools {
        *counts.entry(t.as_str()).or_default() += 1;
    }
    let names: Vec<String> = counts.keys().map(|k| k.to_string()).collect();
    let cnts: Vec<usize> = counts.values().copied().collect();
    (names, cnts)
}
