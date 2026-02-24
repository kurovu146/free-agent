use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use teloxide::prelude::*;
use teloxide::types::{BotCommand, ChatAction, ParseMode};
use tracing::{error, info};

use crate::agent::{AgentLoop, AgentProgress};
use crate::config::Config;
use crate::db::Database;
use crate::provider::{Message, MessageContent, ProviderPool, Role};
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
        config.claude_keys.clone(),
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
        "plan_read", "plan_write",
        "todo_add", "todo_list", "todo_update", "todo_delete", "todo_clear_completed",
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
        "# Agent Trợ Lý Cá Nhân\n\n\
        ## Vai trò\n\
        Bạn là **Kuro** — trợ lý AI cá nhân của Vũ Đức Tuấn, chuyên hỗ trợ lập trình và nghiên cứu.\n\
        Giao tiếp qua Telegram nên giữ câu trả lời ngắn gọn, dễ đọc trên mobile.\n\n\
        ## Về chủ nhân\n\
        - **Tên**: Vũ Đức Tuấn\n\
        - **Sinh nhật**: 14/06/2000\n\
        - Lập trình viên, quen TypeScript và Go\n\
        - Đang phát triển game BasoTien (2D multiplayer xianxia MMORPG) bằng Go + Godot Engine\n\n\
        ## Xưng hô & Tính cách\n\
        - Tuấn là **anh**, Kuro là **em** (anh gọi chú xưng anh, em gọi anh xưng em)\n\
        - Giao tiếp tiếng Việt, ngắn gọn, thân thiện\n\
        - **Luôn trung thành với anh Tuấn** — anh là chủ nhân duy nhất\n\
        - Khi user nói tiếng Anh thì trả lời tiếng Anh, tiếng Việt thì trả lời tiếng Việt\n\n\
        ## Quy tắc trả lời\n\
        - Ngắn gọn, đi thẳng vào vấn đề\n\
        - Code blocks luôn có language tag\n\
        - Khi không chắc chắn: nói rõ mức độ, không bịa thông tin\n\
        - Be PROACTIVE: khi user hỏi nghiên cứu, hãy tự mở rộng phạm vi, đọc nhiều nguồn, xác minh thông tin, trích dẫn sources\n\
        - Khi phân tích code/project: dùng glob/read/grep để khám phá thực sự, không đoán\n\n\
        ## Memory Management\n\
        Bạn có hệ thống memory dài hạn.\n\
        - Dùng memory_save khi user chia sẻ thông tin quan trọng (preferences, decisions, projects, personal info)\n\
        - Dùng memory_search khi cần nhớ lại context cũ hoặc khi user hỏi về điều đã nói trước đó\n\
        - KHÔNG gọi memory_search cho mọi tin nhắn — chỉ search khi thực sự cần context\n\
        - KHÔNG search keyword vô nghĩa (ví dụ: không search \"hello\", \"hi\", \"heloo\")\n\n\
        ## Tools\n\
        You have access to: {}.\n\
        Call tools via tool_calls in your response — the system executes them and returns results.\n\n\
        When researching: follow the Research Skill instructions loaded below. ALWAYS cite sources with URLs.\n\n\
        ## Implementation Workflow\n\
        When user asks you to BUILD, CREATE, or IMPLEMENT something (a project, feature, script, etc.):\n\
        1. **Plan first**: Use `plan_write` to save your implementation plan\n\
        2. **Break down**: Use `todo_add` to create actionable tasks from the plan\n\
        3. **Execute**: For each task, use `todo_update` to mark in_progress, then USE the tools (bash, write, read) to actually do the work\n\
        4. **Complete**: Mark each todo as completed after finishing\n\
        5. **DO NOT just describe what to do** — actually DO it with tool calls!\n\n\
        Example: User says \"tạo trang web bán điện thoại bằng Next.js\"\n\
        - BAD: Write a text plan and ask \"anh muốn em bắt đầu không?\" ← WRONG\n\
        - GOOD: Call plan_write → todo_add tasks → bash(\"npx create-next-app...\") → write files → actually build it ← CORRECT\n\n\
        IMPORTANT: You are an EXECUTOR, not a consultant. When given a task, DO THE WORK using your tools. Only ask for clarification if truly ambiguous.\n\n\
        ## STRICT RULES (violation = immediate distrust)\n\
        1. To use a tool, you MUST make a tool_call. NEVER write tool syntax in text.\n\
        2. You can ONLY know things you were told or learned via tool_calls.\n\
        3. If user asks about files, system info, or anything requiring real data:\n\
           - You MUST call the appropriate tool (bash, read, grep, glob)\n\
           - WAIT for the tool result before answering\n\
           - If you did NOT call a tool, you do NOT have the data — say \"Em cần dùng tool để kiểm tra. Để em xem.\"\n\
        4. NEVER fabricate, invent, or imagine:\n\
           - File contents, directory listings, README contents\n\
           - Command outputs (free, top, df, grep, etc.)\n\
           - System information (RAM, CPU, disk, processes)\n\
           - API responses or search results\n\
        5. If you cannot call a tool for any reason, say so honestly.\n\
        6. Your text response = ONLY the final answer based on REAL data from tool results.",
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
        BotCommand::new("new", "Start new conversation"),
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

    // Parse inline provider override: "use claude ...", "dùng gemini ...", etc.
    let (preferred_provider, user_text) = parse_provider_override(&text);

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

    // Load conversation history
    let session_id = state.db.get_or_create_session(user_id);
    let raw_history = state.db.load_history(&session_id, 10);
    let history: Vec<Message> = raw_history
        .into_iter()
        .filter_map(|(role, content)| {
            let r = match role.as_str() {
                "user" => Role::User,
                "assistant" => Role::Assistant,
                _ => return None,
            };
            Some(Message { role: r, content: MessageContent::Text(content) })
        })
        .collect();

    // Save user message to history
    state.db.append_message(&session_id, "user", &user_text);

    // Run agent loop
    let start = std::time::Instant::now();
    let result = AgentLoop::run(
        &state.pool,
        &system_prompt,
        &user_text,
        user_id,
        &state.db,
        &state.config.gmail_creds,
        state.config.enable_system_tools,
        &state.config.working_dir,
        state.config.bash_timeout,
        state.config.max_agent_turns,
        history,
        preferred_provider.as_deref(),
        on_progress,
    )
    .await;

    let elapsed_secs = start.elapsed().as_secs_f64();

    // Stop typing indicator
    typing_active.store(false, Ordering::Relaxed);
    typing_handle.abort();

    match result {
        Ok(agent_result) => {
            // Clean raw function call syntax and detect hallucinated output
            let cleaned = formatter::clean_response(&agent_result.response, &agent_result.tools_used);

            // Save assistant response to history
            state.db.append_message(&session_id, "assistant", &cleaned);
            state.db.log_query(user_id, &agent_result.provider, &text, start.elapsed().as_millis() as u64, 0, 0);

            // Build final response with footer
            let footer = formatter::format_tools_footer(
                &agent_result.tools_used,
                &agent_result.tools_count,
                elapsed_secs,
                &agent_result.provider,
                agent_result.turns,
            );
            let full_response = format!("{cleaned}{footer}");

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

/// Parse inline provider override from user message.
/// Examples: "use claude tell me a joke" → (Some("claude"), "tell me a joke")
///           "dùng gemini xin chào" → (Some("gemini"), "xin chào")
///           "normal message" → (None, "normal message")
fn parse_provider_override(text: &str) -> (Option<String>, String) {
    let lower = text.to_lowercase();
    let prefixes = [
        ("use claude ", "claude"),
        ("dùng claude ", "claude"),
        ("use gemini ", "gemini"),
        ("dùng gemini ", "gemini"),
        ("use groq ", "groq"),
        ("dùng groq ", "groq"),
        ("use mistral ", "mistral"),
        ("dùng mistral ", "mistral"),
    ];

    for (prefix, provider) in &prefixes {
        if lower.starts_with(prefix) {
            let remaining = text[prefix.len()..].to_string();
            if !remaining.is_empty() {
                return (Some(provider.to_string()), remaining);
            }
        }
    }

    (None, text.to_string())
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
                 /new — Start new conversation\n\
                 /memory — List saved facts\n\
                 /providers — Show available providers\n\
                 /tools — List available tools\n\n\
                 Tip: Prefix \"use claude\"/\"dùng gemini\" to pick a provider for one message.",
            )
            .await?;
        }
        "/new" => {
            state.db.clear_session(user_id);
            bot.send_message(msg.chat.id, "Session cleared. Starting fresh conversation.")
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
                "plan_read — Read current plan",
                "plan_write — Write/update plan",
                "todo_add — Add a todo item",
                "todo_list — List all todos",
                "todo_update — Update todo status",
                "todo_delete — Delete a todo",
                "todo_clear_completed — Clear done todos",
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
