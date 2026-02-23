use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use teloxide::prelude::*;
use teloxide::types::{BotCommand, ChatAction, ParseMode};
use tracing::{error, info};

use crate::agent::{AgentLoop, AgentProgress};
use crate::config::Config;
use crate::db::Database;
use crate::provider::ProviderPool;
use crate::skills;

use super::formatter;

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

    // Build tool list dynamically based on config
    let gmail_ok = config.gmail_creds.is_configured();
    let sys_ok = config.enable_system_tools;
    let mut tool_list = vec![
        "web_search", "web_fetch", "memory_save", "memory_search",
        "memory_list", "memory_delete", "get_datetime",
    ];
    if sys_ok {
        tool_list.extend(&["bash", "read", "write", "glob", "grep"]);
    }
    if gmail_ok {
        tool_list.extend(&[
            "gmail_search", "gmail_read", "gmail_send", "gmail_archive",
            "gmail_trash", "gmail_label", "gmail_list_labels",
            "sheets_read", "sheets_write", "sheets_append",
            "sheets_list", "sheets_create_tab",
        ]);
    }

    let base_prompt = format!(
        "You are a friendly, helpful AI assistant running as a Telegram bot.\n\
        Your name is KuroFree.\n\n\
        ## Communication style\n\
        - Be warm, friendly, and approachable\n\
        - Use casual, natural language — avoid being robotic or overly formal\n\
        - When speaking Vietnamese, use anh/em pronouns (anh is the user, em is you). NEVER use mình/bạn/tôi\n\
        - Keep responses concise for Telegram readability\n\
        - Use bullet points and formatting when helpful\n\
        - Always respond in the same language the user uses\n\n\
        ## Memory Management — CRITICAL\n\
        You have a long-term memory system that persists across conversations.\n\
        ALWAYS call memory_search at the START of each conversation to recall known facts about the user.\n\
        You MUST call memory_save IMMEDIATELY when the user shares ANY of the following:\n\
        - Their name, nickname, or how they want to be called\n\
        - Personal preferences, interests, or habits\n\
        - Technical details: projects, tools, tech stack\n\
        - Decisions, plans, or goals\n\
        - Any fact they explicitly ask you to remember\n\
        - Important context about their work or life\n\
        Do NOT wait — save the fact as soon as you see it in the message.\n\n\
        ## Tools available\n\
        You have access to: {}.\n\
        Use tools proactively when they can help answer the user's question better.",
        tool_list.join(", ")
    );

    let state = Arc::new(AppState {
        pool,
        db,
        config: config.clone(),
        skills_content,
        base_prompt,
    });

    info!(
        "Bot started. Providers: {:?}, Tools: {}, SystemTools: {}, Gmail: {}, Allowed users: {:?}",
        state.pool.available_providers(),
        tool_list.len(),
        if sys_ok { "enabled" } else { "disabled" },
        if gmail_ok { "enabled" } else { "disabled" },
        config.allowed_users
    );

    // Register bot commands menu in Telegram
    let commands = vec![
        BotCommand::new("start", "Bot info & status"),
        BotCommand::new("help", "Show available commands"),
        BotCommand::new("tools", "List available tools"),
        BotCommand::new("memory", "View saved memories"),
        BotCommand::new("providers", "Show LLM providers"),
    ];
    if let Err(e) = bot.set_my_commands(commands).await {
        error!("Failed to set bot commands: {e}");
    } else {
        info!("Bot commands menu registered");
    }

    let handler = Update::filter_message().endpoint(handle_message);

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .build()
        .dispatch()
        .await;
}

/// Edit a Telegram message, trying Markdown first then falling back to plain text.
async fn safe_edit(bot: &Bot, chat_id: ChatId, msg_id: i32, text: &str) {
    // Try Markdown first (legacy mode — simpler than MarkdownV2)
    #[allow(deprecated)]
    let md_result = bot
        .edit_message_text(chat_id, teloxide::types::MessageId(msg_id), text)
        .parse_mode(ParseMode::Markdown)
        .await;
    if md_result.is_err() {
        let _ = bot
            .edit_message_text(chat_id, teloxide::types::MessageId(msg_id), text)
            .await;
    }
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

    // Send initial progress message
    let _ = bot.send_chat_action(msg.chat.id, ChatAction::Typing).await;
    let progress_msg = bot
        .send_message(msg.chat.id, "⏳ Đang xử lý...")
        .await?;
    let progress_msg_id = progress_msg.id.0;

    // Typing indicator loop
    let bot_typing = bot.clone();
    let chat_id = msg.chat.id;
    let typing_active = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let typing_flag = typing_active.clone();
    let typing_handle = tokio::spawn(async move {
        while typing_flag.load(Ordering::Relaxed) {
            let _ = bot_typing.send_chat_action(chat_id, ChatAction::Typing).await;
            tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        }
    });

    // Progress callback: edit the progress message when tools are used
    let bot_progress = bot.clone();
    let progress_chat_id = msg.chat.id;
    let last_edit = Arc::new(AtomicI64::new(0));

    let on_progress = move |progress: AgentProgress| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let prev = last_edit.load(Ordering::Relaxed);

        // Throttle: at least 1.5s between edits
        if now - prev < 1500 {
            return;
        }

        let display_text = match &progress {
            AgentProgress::ToolUse(name) => formatter::format_progress(name),
            AgentProgress::Thinking => "⏳ Đang suy nghĩ...".to_string(),
        };

        last_edit.store(now, Ordering::Relaxed);
        let bot_inner = bot_progress.clone();
        let _ = tokio::task::spawn(async move {
            safe_edit(&bot_inner, progress_chat_id, progress_msg_id, &display_text).await;
        });
    };

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
        &state.config.gmail_creds,
        state.config.enable_system_tools,
        &state.config.working_dir,
        state.config.bash_timeout,
        state.config.max_agent_turns,
        on_progress,
    )
    .await;

    let elapsed_secs = start.elapsed().as_secs_f64();

    // Stop typing indicator
    typing_active.store(false, Ordering::Relaxed);
    typing_handle.abort();

    match result {
        Ok(agent_result) => {
            state.db.log_query(user_id, &agent_result.provider, &text, start.elapsed().as_millis() as u64, 0, 0);

            // Build final response with footer
            let footer = formatter::format_tools_footer(&agent_result.tools_used, elapsed_secs);
            let full_response = format!("{}{footer}", agent_result.response);

            let chunks = formatter::split_message(&full_response, 4096);

            // Edit the first chunk into the progress message
            if let Some(first) = chunks.first() {
                safe_edit(&bot, msg.chat.id, progress_msg_id, first).await;
            }

            // Send remaining chunks as new messages
            for chunk in chunks.iter().skip(1) {
                #[allow(deprecated)]
                let md_result = bot
                    .send_message(msg.chat.id, chunk)
                    .parse_mode(ParseMode::Markdown)
                    .await;
                if md_result.is_err() {
                    let _ = bot.send_message(msg.chat.id, chunk).await;
                }
            }
        }
        Err(err) => {
            error!("Agent error: {err}");
            safe_edit(&bot, msg.chat.id, progress_msg_id, &format!("❌ Error: {err}")).await;
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
            let gmail_status = if state.config.gmail_creds.is_configured() {
                "enabled" } else { "disabled" };
            let sys_status = if state.config.enable_system_tools {
                "enabled" } else { "disabled" };
            bot.send_message(
                msg.chat.id,
                format!(
                    "KuroFree Bot\n\n\
                    Providers: {}\n\
                    Gmail/Sheets: {gmail_status}\n\
                    System tools (bash/read/write): {sys_status}\n\n\
                    /help for commands",
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
                 /providers — Show available providers\n\
                 /tools — List available tools",
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
                for chunk in formatter::split_message(&output, 4096) {
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
        "/tools" => {
            let gmail_ok = state.config.gmail_creds.is_configured();
            let sys_ok = state.config.enable_system_tools;
            let mut tools = vec![
                "web_search — Search the web",
                "web_fetch — Fetch URL content",
                "memory_save — Save a fact",
                "memory_search — Search memory",
                "memory_list — List all facts",
                "memory_delete — Delete a fact",
                "get_datetime — Current date/time",
            ];
            if sys_ok {
                tools.extend(&[
                    "bash — Execute shell commands",
                    "read — Read file contents",
                    "write — Write/create files",
                    "glob — Find files by pattern",
                    "grep — Search file contents",
                ]);
            }
            if gmail_ok {
                tools.extend(&[
                    "gmail_search — Search emails",
                    "gmail_read — Read email",
                    "gmail_send — Send email",
                    "gmail_archive — Archive emails",
                    "gmail_trash — Trash emails",
                    "gmail_label — Add/remove labels",
                    "gmail_list_labels — List labels",
                    "sheets_read — Read spreadsheet",
                    "sheets_write — Write to spreadsheet",
                    "sheets_append — Append rows",
                    "sheets_list — List sheet tabs",
                    "sheets_create_tab — Create new tab",
                ]);
            }
            bot.send_message(msg.chat.id, tools.join("\n")).await?;
        }
        _ => {
            bot.send_message(msg.chat.id, "Unknown command. /help")
                .await?;
        }
    }
    Ok(())
}
