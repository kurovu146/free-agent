/// Tool icons and message formatting for Telegram output.

pub fn tool_icon(name: &str) -> &str {
    match name {
        "web_search" => "🌐",
        "web_fetch" => "📥",
        "memory_save" | "memory_search" | "memory_list" | "memory_delete" => "🧠",
        "bash" => "⚡",
        "read" => "📖",
        "write" => "✏️",
        "glob" => "🔍",
        "grep" => "🔎",
        "get_datetime" => "🕐",
        _ if name.starts_with("gmail_") => "📧",
        _ if name.starts_with("sheets_") => "📊",
        _ => "🔧",
    }
}

pub fn format_tools_footer(tools: &[String], elapsed_secs: f64) -> String {
    if tools.is_empty() {
        return format!("\n\n---\n⏱ {elapsed_secs:.1}s");
    }

    let formatted: Vec<String> = tools
        .iter()
        .map(|t| format!("{} {t}", tool_icon(t)))
        .collect();

    format!(
        "\n\n---\nTools: {}  |  ⏱ {elapsed_secs:.1}s",
        formatted.join("  ")
    )
}

pub fn format_progress(current_tool: &str) -> String {
    let icon = tool_icon(current_tool);
    format!("⏳ {icon} Đang dùng {current_tool}...")
}

/// Find the largest char-boundary index <= `pos` in `s`.
fn floor_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let mut i = pos;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

pub fn split_message(text: &str, max_len: usize) -> Vec<String> {
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

        // Find a safe char boundary to slice at
        let safe_end = floor_char_boundary(remaining, max_len);
        let search_zone = &remaining[..safe_end];

        let split_at = search_zone
            .rfind('\n')
            .unwrap_or_else(|| search_zone.rfind(' ').unwrap_or(safe_end));

        // Avoid zero-length splits
        let split_at = if split_at == 0 { safe_end } else { split_at };

        chunks.push(remaining[..split_at].to_string());
        remaining = remaining[split_at..].trim_start();
    }

    chunks
}
