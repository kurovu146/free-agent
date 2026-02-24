/// Tool icons and message formatting for Telegram output.

/// Strip raw function/tool call syntax that some LLMs leak into text responses.
/// Catches patterns like: <function=name>...</function>, <tool_call>...</tool_call>,
/// ```tool_code ... ```, and similar hallucinated tool invocations.
///
/// If `tools_used` is provided, also detects hallucinated command output
/// (model pretending to run commands without actually calling bash tool).
pub fn clean_response(text: &str, tools_used: &[String]) -> String {
    let mut result = text.to_string();

    // Remove <function=...>{...}</function> patterns (and trailing ...)
    while let Some(start) = result.find("<function=") {
        if let Some(end) = result[start..].find("</function>") {
            let remove_end = start + end + "</function>".len();
            // Also remove trailing "..." or "…" after </function>
            let mut actual_end = remove_end;
            let remaining = &result[actual_end..];
            if remaining.starts_with("...") {
                actual_end += 3;
            } else if remaining.starts_with("…") {
                actual_end += "…".len();
            }
            result = format!("{}{}", &result[..start], result[actual_end..].trim_start());
        } else {
            // No closing tag — remove from <function= to end of line
            if let Some(eol) = result[start..].find('\n') {
                result = format!("{}{}", &result[..start], &result[start + eol..]);
            } else {
                result = result[..start].to_string();
            }
        }
    }

    // Remove <tool_call>...</tool_call> patterns
    while let Some(start) = result.find("<tool_call>") {
        if let Some(end) = result[start..].find("</tool_call>") {
            let remove_end = start + end + "</tool_call>".len();
            result = format!("{}{}", &result[..start], result[remove_end..].trim_start());
        } else {
            if let Some(eol) = result[start..].find('\n') {
                result = format!("{}{}", &result[..start], &result[start + eol..]);
            } else {
                result = result[..start].to_string();
            }
        }
    }

    // Remove ```tool_code ... ``` blocks
    while let Some(start) = result.find("```tool_code") {
        if let Some(end) = result[start + 12..].find("```") {
            let remove_end = start + 12 + end + 3;
            result = format!("{}{}", &result[..start], result[remove_end..].trim_start());
        } else {
            break;
        }
    }

    // Detect hallucinated command output: model shows "$ command\noutput" or
    // ```bash/```sh blocks with fake shell commands, but never actually called bash tool.
    let bash_was_used = tools_used.iter().any(|t| t == "bash");
    if !bash_was_used && looks_like_fake_command_output(&result) {
        result = strip_fake_command_blocks(&result);
        if result.trim().is_empty() {
            result = "Em không thể trả lời chính xác câu hỏi này mà không chạy lệnh thật. \
                      Anh hỏi lại để em dùng tool bash nhé."
                .to_string();
        }
    }

    // Detect when model claims to have performed actions but used NO tools at all.
    // This catches fabricated grep/read/bash results written as prose.
    if tools_used.is_empty() && looks_like_fabricated_action(&result) {
        result.push_str("\n\n⚠️ _Cảnh báo: câu trả lời trên có thể không chính xác vì em chưa thực sự gọi tool nào để kiểm tra._");
    }

    // Clean up excessive whitespace from removals
    while result.contains("\n\n\n") {
        result = result.replace("\n\n\n", "\n\n");
    }

    result.trim().to_string()
}

/// Check if text contains patterns that look like fabricated command output.
fn looks_like_fake_command_output(text: &str) -> bool {
    let indicators = [
        // Shell prompt patterns with system commands
        "$ free ",
        "$ top ",
        "$ df ",
        "$ cat /proc/",
        "$ uname ",
        "$ lscpu",
        "$ lsblk",
        "$ ps aux",
        "$ htop",
        "$ vmstat",
        "$ iostat",
        "$ uptime",
        "$ whoami",
        "$ hostname",
        "$ ifconfig",
        "$ ip addr",
        "$ netstat",
    ];

    // Check for shell prompt patterns
    for indicator in &indicators {
        if text.contains(indicator) {
            return true;
        }
    }

    // Check for ```bash or ```sh blocks containing $ prompts
    let lower = text.to_lowercase();
    if (lower.contains("```bash") || lower.contains("```sh") || lower.contains("```shell"))
        && text.contains("$ ")
    {
        return true;
    }

    false
}

/// Check if model claims to have performed actions (read files, run commands, searched)
/// without actually calling any tools. Detects fabricated prose-style results.
fn looks_like_fabricated_action(text: &str) -> bool {
    let lower = text.to_lowercase();

    // Phrases that indicate the model claims to have performed an action
    let action_claims = [
        "em tìm thấy",
        "em đã tìm",
        "em đã kiểm tra",
        "em đã chạy",
        "em đã đọc",
        "em đã xem",
        "kết quả cho thấy",
        "kết quả:",
        "kết quả grep",
        "kết quả tìm kiếm",
        "folder chứa",
        "thư mục chứa",
        "file readme",
        "nội dung file",
        "dự án này có",
        "cấu trúc thư mục",
        "cấu trúc dự án",
    ];

    // Also check for patterns that look like fabricated file/dir listings
    let listing_patterns = [
        "├──",
        "└──",
        "│  ",
    ];

    let has_action_claim = action_claims.iter().any(|p| lower.contains(p));
    let has_listing = listing_patterns.iter().any(|p| text.contains(p));

    // Threshold: at least one action claim, or a tree listing
    has_action_claim || has_listing
}

/// Remove code blocks that contain fake command output.
fn strip_fake_command_blocks(text: &str) -> String {
    let mut result = String::new();
    let block_start_tags = ["```bash", "```sh", "```shell", "```console", "```"];

    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let lower = line.trim().to_lowercase();

        // Detect start of a code block that might be fake
        {
            let is_code_block_start = block_start_tags.iter().any(|tag| lower.starts_with(tag));

            if is_code_block_start {
                // Look ahead: does this block contain $ prompt patterns?
                let mut has_shell_prompt = false;
                let mut block_end = i + 1;
                for j in (i + 1)..lines.len() {
                    if lines[j].trim() == "```" {
                        block_end = j;
                        break;
                    }
                    if lines[j].trim_start().starts_with("$ ") {
                        has_shell_prompt = true;
                    }
                }

                if has_shell_prompt {
                    // Skip this entire fake block
                    i = block_end + 1;
                    continue;
                }
            }

            // Also detect inline fake output: lines starting with "$ command"
            // followed by lines that look like output (not starting with $)
            if line.trim_start().starts_with("$ ") {
                // Skip this line and any following non-$ lines (fake output)
                i += 1;
                while i < lines.len()
                    && !lines[i].trim_start().starts_with("$ ")
                    && !lines[i].trim().is_empty()
                    && !lines[i].trim().starts_with("```")
                {
                    i += 1;
                }
                continue;
            }
        }

        result.push_str(line);
        result.push('\n');
        i += 1;
    }

    result
}

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

pub fn format_tools_footer(
    tools: &[String],
    tools_count: &[usize],
    elapsed_secs: f64,
    provider: &str,
    turns: usize,
) -> String {
    let mut parts = Vec::new();

    if !tools.is_empty() {
        let formatted: Vec<String> = tools
            .iter()
            .zip(tools_count.iter())
            .map(|(t, &count)| {
                if count > 1 {
                    format!("{} {}x{}", tool_icon(t), t, count)
                } else {
                    format!("{} {}", tool_icon(t), t)
                }
            })
            .collect();
        parts.push(format!("Tools: {}", formatted.join("  ")));
    }

    parts.push(format!("⏱ {elapsed_secs:.1}s"));

    if turns > 1 {
        parts.push(format!("{provider} ({turns} turns)"));
    } else {
        parts.push(provider.to_string());
    }

    format!("\n\n---\n{}", parts.join("  |  "))
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
