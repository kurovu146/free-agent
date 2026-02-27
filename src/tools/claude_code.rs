use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Info about a running Claude Code tmux session.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub working_dir: String,
    pub created_at: String,
    pub last_activity: String,
}

/// Manages Claude Code tmux sessions.
#[derive(Clone)]
pub struct ClaudeCodeManager {
    sessions: Arc<RwLock<HashMap<String, SessionInfo>>>,
    claude_path: String,
    default_timeout: u64,
}

impl ClaudeCodeManager {
    pub fn new(claude_path: &str, default_timeout: u64) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            claude_path: claude_path.to_string(),
            default_timeout,
        }
    }
}

// ---------------------------------------------------------------------------
// Public tool functions
// ---------------------------------------------------------------------------

/// Start a new Claude Code session in a tmux window.
pub async fn cc_start(mgr: &ClaudeCodeManager, name: &str, working_dir: &str) -> String {
    let session_name = format!("cc-{name}");

    // Check if session already exists
    {
        let sessions = mgr.sessions.read().await;
        if sessions.contains_key(name) {
            return format!("Session '{name}' already exists. Use cc_stop first or pick another name.");
        }
    }

    // Verify working_dir exists
    if !std::path::Path::new(working_dir).is_dir() {
        return format!("Directory does not exist: {working_dir}");
    }

    // Create tmux session running Claude Code CLI
    let create_result = tmux_cmd(&[
        "new-session", "-d",
        "-s", &session_name,
        "-c", working_dir,
        &mgr.claude_path,
    ]).await;

    if let Err(e) = create_result {
        return format!("Failed to create tmux session: {e}");
    }

    // Wait for Claude Code to initialize
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let now = Utc::now().to_rfc3339();
    let info = SessionInfo {
        working_dir: working_dir.to_string(),
        created_at: now.clone(),
        last_activity: now,
    };

    mgr.sessions.write().await.insert(name.to_string(), info);

    // Read initial output
    let output = capture_pane(&session_name).await.unwrap_or_default();
    let clean = strip_ansi(&output);

    format!("Session '{name}' started in {working_dir}\n\nInitial output:\n{clean}")
}

/// Send a message to a Claude Code session and wait for completion.
pub async fn cc_send(mgr: &ClaudeCodeManager, name: &str, message: &str, timeout: Option<u64>) -> String {
    let session_name = format!("cc-{name}");
    let timeout_secs = timeout.unwrap_or(mgr.default_timeout);

    // Verify session exists
    {
        let sessions = mgr.sessions.read().await;
        if !sessions.contains_key(name) {
            return format!("Session '{name}' not found. Use cc_start first.");
        }
    }

    // Capture baseline before sending
    let baseline = capture_pane(&session_name).await.unwrap_or_default();

    // Send message via tmux send-keys (literal mode to avoid key interpretation)
    if let Err(e) = tmux_cmd(&["send-keys", "-t", &session_name, "-l", message]).await {
        return format!("Failed to send message: {e}");
    }
    // Press Enter
    if let Err(e) = tmux_cmd(&["send-keys", "-t", &session_name, "Enter"]).await {
        return format!("Failed to send Enter: {e}");
    }

    // Wait for completion
    let result = wait_for_completion(&session_name, &baseline, timeout_secs).await;

    // Update last_activity
    {
        let mut sessions = mgr.sessions.write().await;
        if let Some(info) = sessions.get_mut(name) {
            info.last_activity = Utc::now().to_rfc3339();
        }
    }

    result
}

/// Read current pane content of a session.
pub async fn cc_read(mgr: &ClaudeCodeManager, name: &str) -> String {
    let session_name = format!("cc-{name}");

    {
        let sessions = mgr.sessions.read().await;
        if !sessions.contains_key(name) {
            return format!("Session '{name}' not found.");
        }
    }

    match capture_pane(&session_name).await {
        Ok(output) => {
            let clean = strip_ansi(&output);
            if clean.trim().is_empty() {
                "(pane is empty)".to_string()
            } else {
                clean
            }
        }
        Err(e) => format!("Failed to read session: {e}"),
    }
}

/// List all tracked sessions.
pub async fn cc_list(mgr: &ClaudeCodeManager) -> String {
    let sessions = mgr.sessions.read().await;

    if sessions.is_empty() {
        return "No active Claude Code sessions.".to_string();
    }

    let mut lines = Vec::new();
    for (name, info) in sessions.iter() {
        let session_name = format!("cc-{name}");
        let alive = check_session_alive(&session_name).await;
        let status = if alive { "running" } else { "dead" };
        lines.push(format!(
            "- {name} [{status}] dir={} created={} last_activity={}",
            info.working_dir, info.created_at, info.last_activity
        ));
    }

    lines.join("\n")
}

/// Stop and kill a session.
pub async fn cc_stop(mgr: &ClaudeCodeManager, name: &str) -> String {
    let session_name = format!("cc-{name}");

    {
        let sessions = mgr.sessions.read().await;
        if !sessions.contains_key(name) {
            return format!("Session '{name}' not found.");
        }
    }

    let _ = tmux_cmd(&["kill-session", "-t", &session_name]).await;
    mgr.sessions.write().await.remove(name);

    format!("Session '{name}' stopped.")
}

/// Send Ctrl+C interrupt to a session.
pub async fn cc_interrupt(mgr: &ClaudeCodeManager, name: &str) -> String {
    let session_name = format!("cc-{name}");

    {
        let sessions = mgr.sessions.read().await;
        if !sessions.contains_key(name) {
            return format!("Session '{name}' not found.");
        }
    }

    match tmux_cmd(&["send-keys", "-t", &session_name, "C-c"]).await {
        Ok(_) => format!("Sent Ctrl+C to session '{name}'."),
        Err(e) => format!("Failed to interrupt: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Run a tmux command and return stdout.
async fn tmux_cmd(args: &[&str]) -> Result<String, String> {
    debug!("tmux {}", args.join(" "));
    let output = Command::new("tmux")
        .args(args)
        .output()
        .await
        .map_err(|e| format!("tmux exec error: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("tmux error: {stderr}"))
    }
}

/// Capture the full pane content of a tmux session.
async fn capture_pane(session_name: &str) -> Result<String, String> {
    tmux_cmd(&["capture-pane", "-t", session_name, "-p", "-S", "-", "-E", "-"])
        .await
}

/// Check if a tmux session is alive.
async fn check_session_alive(session_name: &str) -> bool {
    tmux_cmd(&["has-session", "-t", session_name]).await.is_ok()
}

/// Strip ANSI escape codes (CSI sequences, OSC, carriage returns).
fn strip_ansi(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\x1b' => {
                // ESC sequence
                match chars.peek() {
                    Some('[') => {
                        // CSI sequence: ESC [ ... final_byte
                        chars.next();
                        while let Some(&ch) = chars.peek() {
                            if ch.is_ascii_alphabetic() || ch == '@' || ch == '~' {
                                chars.next();
                                break;
                            }
                            chars.next();
                        }
                    }
                    Some(']') => {
                        // OSC sequence: ESC ] ... ST (BEL or ESC \)
                        chars.next();
                        while let Some(&ch) = chars.peek() {
                            if ch == '\x07' {
                                chars.next();
                                break;
                            }
                            if ch == '\x1b' {
                                chars.next();
                                if chars.peek() == Some(&'\\') {
                                    chars.next();
                                }
                                break;
                            }
                            chars.next();
                        }
                    }
                    Some('(') | Some(')') => {
                        // Character set designation — skip 2 chars
                        chars.next();
                        chars.next();
                    }
                    _ => {
                        // Unknown ESC — skip one char
                        chars.next();
                    }
                }
            }
            '\r' => {
                // Skip carriage return
            }
            _ => {
                result.push(c);
            }
        }
    }

    result
}

/// Check if the last non-empty line looks like an interactive prompt.
fn looks_like_prompt(content: &str) -> bool {
    let last_line = content
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty());

    match last_line {
        Some(line) => {
            let trimmed = line.trim();
            // Must be short (prompt lines are typically < 200 chars)
            if trimmed.len() > 200 {
                return false;
            }
            // Common prompt indicators
            trimmed.ends_with('>')
                || trimmed.ends_with('❯')
                || trimmed.ends_with("$ ")
                || trimmed == "$"
                || trimmed == ">"
                || trimmed == "❯"
        }
        None => false,
    }
}

/// Extract the new content that appeared after the baseline.
fn extract_response(baseline: &str, current: &str) -> String {
    let baseline_clean = strip_ansi(baseline);
    let current_clean = strip_ansi(current);

    let baseline_lines: Vec<&str> = baseline_clean.lines().collect();
    let current_lines: Vec<&str> = current_clean.lines().collect();

    // Find where the new content starts by looking for the divergence point
    let common = baseline_lines
        .iter()
        .zip(current_lines.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let new_lines: Vec<&str> = current_lines[common..].to_vec();
    new_lines.join("\n")
}

/// Poll until Claude Code output stabilizes and a prompt appears.
async fn wait_for_completion(session_name: &str, baseline: &str, timeout_secs: u64) -> String {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(timeout_secs);

    // Initial wait for Claude Code to start processing
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let mut prev_content = String::new();
    let mut stable_count = 0u32;

    loop {
        if start.elapsed() > timeout {
            let current = capture_pane(session_name).await.unwrap_or_default();
            let response = extract_response(baseline, &current);
            let clean = strip_ansi(&response);
            warn!("cc_send timed out after {timeout_secs}s for {session_name}");
            return format!("{clean}\n\n[TIMEOUT after {timeout_secs}s — output may be incomplete]");
        }

        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

        let current = match capture_pane(session_name).await {
            Ok(c) => c,
            Err(_) => continue,
        };

        let clean = strip_ansi(&current);

        if clean == prev_content {
            stable_count += 1;
        } else {
            stable_count = 0;
            prev_content = clean.clone();
        }

        // Output hasn't changed for 2 consecutive polls AND last line is a prompt
        if stable_count >= 2 && looks_like_prompt(&clean) {
            let response = extract_response(baseline, &current);
            return strip_ansi(&response);
        }
    }
}
