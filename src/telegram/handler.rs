use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{ChatAction, ParseMode};
use tracing::{error, info};

use crate::agent::AgentLoop;
use crate::config::Config;
use crate::db::Database;
use crate::provider::ProviderPool;
use crate::skills;

struct AppState {
    pool: ProviderPool,
    db: Database,
    config: Config,
    skills_content: String,
    base_prompt: String,
}

pub async fn run_bot(config: Config) {
    let bot = Bot::new(&config.telegram_bot_token);

    let pool = ProviderPool::new(
        config.gemini_keys.clone(),
        config.groq_keys.clone(),
        config.mistral_keys.clone(),
        &config.default_provider,
    );

    let db = Database::open("free-agent.db").expect("Failed to open database");

    let skills_content = skills::load_skills("skills");

    let base_prompt = r#"You are a helpful AI assistant running as a Telegram bot.
You have access to tools: web_search, memory_save, memory_search, memory_list.
Use tools when needed to help the user. Be concise — responses will be sent via Telegram.
Always respond in the same language the user uses."#
        .to_string();

    let state = Arc::new(AppState {
        pool,
        db,
        config: config.clone(),
        skills_content,
        base_prompt,
    });

    info!(
        "Bot started. Providers: {:?}, Allowed users: {:?}",
        state.pool.available_providers(),
        config.allowed_users
    );

    let handler = Update::filter_message().endpoint(handle_message);

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .build()
        .dispatch()
        .await;
}

async fn handle_message(
    msg: teloxide::types::Message,
    bot: Bot,
    state: Arc<AppState>,
) -> ResponseResult<()> {
    let user_id = msg.from.as_ref().map(|u| u.id.0).unwrap_or(0);

    // Auth check
    if !state.config.allowed_users.is_empty()
        && !state.config.allowed_users.contains(&user_id)
    {
        bot.send_message(msg.chat.id, "Unauthorized.").await?;
        return Ok(());
    }

    // Get text content
    let text = match msg.text() {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return Ok(()),
    };

    // Handle commands
    if text.starts_with('/') {
        return handle_command(&msg, &bot, &state, &text, user_id).await;
    }

    // Typing indicator
    let _ = bot.send_chat_action(msg.chat.id, ChatAction::Typing).await;

    // Build system prompt with memory
    let memory_ctx = state.db.build_memory_context(user_id);
    let system_prompt = skills::build_system_prompt(
        &state.base_prompt,
        &state.skills_content,
        &memory_ctx,
    );

    // Run agent loop
    let start = std::time::Instant::now();
    let result = AgentLoop::run(
        &state.pool,
        &system_prompt,
        &text,
        user_id,
        &state.db,
        state.config.max_agent_turns,
    )
    .await;

    let elapsed_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(response) => {
            state.db.log_query(user_id, "agent", &text, elapsed_ms, 0, 0);

            for chunk in split_message(&response, 4096) {
                // Try markdown first, fallback to plain text
                let md_result = bot
                    .send_message(msg.chat.id, &chunk)
                    .parse_mode(ParseMode::MarkdownV2)
                    .await;

                if md_result.is_err() {
                    let _ = bot.send_message(msg.chat.id, &chunk).await;
                }
            }
        }
        Err(err) => {
            error!("Agent error: {err}");
            bot.send_message(msg.chat.id, format!("Error: {err}")).await?;
        }
    }

    Ok(())
}

async fn handle_command(
    msg: &teloxide::types::Message,
    bot: &Bot,
    state: &AppState,
    text: &str,
    user_id: u64,
) -> ResponseResult<()> {
    match text.split_whitespace().next().unwrap_or("") {
        "/start" => {
            bot.send_message(
                msg.chat.id,
                format!(
                    "Free Agent Bot\n\nProviders: {}\nTools: web_search, memory\n\n/help for commands",
                    state.pool.available_providers().join(", ")
                ),
            )
            .await?;
        }
        "/help" => {
            bot.send_message(
                msg.chat.id,
                "/start — Bot info\n\
                 /help — Show commands\n\
                 /memory — List saved facts\n\
                 /providers — Show available providers",
            )
            .await?;
        }
        "/memory" => {
            let facts = state.db.list_facts(user_id, None).unwrap_or_default();
            if facts.is_empty() {
                bot.send_message(msg.chat.id, "No facts saved yet.").await?;
            } else {
                let output: String = facts
                    .iter()
                    .map(|(id, fact, cat)| format!("[{id}] [{cat}] {fact}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                for chunk in split_message(&output, 4096) {
                    bot.send_message(msg.chat.id, &chunk).await?;
                }
            }
        }
        "/providers" => {
            bot.send_message(
                msg.chat.id,
                format!("Available: {}", state.pool.available_providers().join(", ")),
            )
            .await?;
        }
        _ => {
            bot.send_message(msg.chat.id, "Unknown command. /help")
                .await?;
        }
    }
    Ok(())
}

fn split_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= max_len {
            chunks.push(remaining.to_string());
            break;
        }

        let split_at = remaining[..max_len]
            .rfind('\n')
            .unwrap_or_else(|| remaining[..max_len].rfind(' ').unwrap_or(max_len));

        chunks.push(remaining[..split_at].to_string());
        remaining = remaining[split_at..].trim_start();
    }

    chunks
}
