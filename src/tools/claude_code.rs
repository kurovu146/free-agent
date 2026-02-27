use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;

use chrono::Utc;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Info about a Claude Code session (conversation continuity via --resume).
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: Option<String>,
    pub working_dir: String,
    pub created_at: String,
    pub last_activity: String,
}

/// Manages Claude Code sessions using `--print` (non-interactive) mode.
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

/// Start a new Claude Code session (register name + working dir).
pub async fn cc_start(mgr: &ClaudeCodeManager, name: &str, working_dir: &str) -> String {
    {
        let sessions = mgr.sessions.read().await;
        if sessions.contains_key(name) {
            return format!("Session '{name}' already exists. Use cc_stop first or pick another name.");
        }
    }

    if !std::path::Path::new(working_dir).is_dir() {
        return format!("Directory does not exist: {working_dir}");
    }

    let now = Utc::now().to_rfc3339();
    let info = SessionInfo {
        session_id: None,
        working_dir: working_dir.to_string(),
        created_at: now.clone(),
        last_activity: now,
    };

    mgr.sessions.write().await.insert(name.to_string(), info);

    format!("Session '{name}' created for {working_dir}. Use cc_send to send messages.")
}

/// Send a message to Claude Code using --print mode.
/// If the session has a previous session_id, uses --resume for continuity.
pub async fn cc_send(mgr: &ClaudeCodeManager, name: &str, message: &str, timeout: Option<u64>) -> String {
    let timeout_secs = timeout.unwrap_or(mgr.default_timeout);

    let (working_dir, session_id) = {
        let sessions = mgr.sessions.read().await;
        match sessions.get(name) {
            Some(info) => (info.working_dir.clone(), info.session_id.clone()),
            None => return format!("Session '{name}' not found. Use cc_start first."),
        }
    };

    let mut args = vec![
        "--print".to_string(),
        "--output-format".to_string(), "json".to_string(),
        "--dangerously-skip-permissions".to_string(),
    ];

    // Resume previous conversation if we have a session_id
    if let Some(ref sid) = session_id {
        args.push("--resume".to_string());
        args.push(sid.clone());
    }

    args.push(message.to_string());

    debug!("cc_send: {} {:?} in {}", mgr.claude_path, args, working_dir);

    let child = Command::new(&mgr.claude_path)
        .args(&args)
        .current_dir(&working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut child = match child {
        Ok(c) => c,
        Err(e) => return format!("Failed to start claude: {e}"),
    };

    // Wait with timeout
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        async {
            let mut stdout_buf = Vec::new();
            let mut stderr_buf = Vec::new();

            if let Some(ref mut stdout) = child.stdout {
                let _ = stdout.read_to_end(&mut stdout_buf).await;
            }
            if let Some(ref mut stderr) = child.stderr {
                let _ = stderr.read_to_end(&mut stderr_buf).await;
            }

            let status = child.wait().await;
            (stdout_buf, stderr_buf, status)
        }
    ).await;

    let (stdout_bytes, stderr_bytes) = match result {
        Ok((stdout, stderr, status)) => {
            if let Ok(s) = &status {
                if !s.success() {
                    let stderr_str = String::from_utf8_lossy(&stderr);
                    warn!("claude exited with {s}: {stderr_str}");
                }
            }
            (stdout, stderr)
        }
        Err(_) => {
            // Timeout — kill the process
            let _ = child.kill().await;
            warn!("cc_send timed out after {timeout_secs}s for session '{name}'");
            return format!("[TIMEOUT after {timeout_secs}s — Claude Code did not respond in time]");
        }
    };

    let stdout_str = String::from_utf8_lossy(&stdout_bytes).to_string();
    let stderr_str = String::from_utf8_lossy(&stderr_bytes).to_string();

    // Parse JSON output to extract result and session_id
    let (response_text, new_session_id) = parse_cc_json_output(&stdout_str);

    // Update session info
    {
        let mut sessions = mgr.sessions.write().await;
        if let Some(info) = sessions.get_mut(name) {
            info.last_activity = Utc::now().to_rfc3339();
            if let Some(sid) = new_session_id {
                info.session_id = Some(sid);
            }
        }
    }

    if response_text.is_empty() && !stderr_str.is_empty() {
        format!("[Claude Code error]\n{stderr_str}")
    } else if response_text.is_empty() {
        format!("[No output from Claude Code]\nstdout: {stdout_str}\nstderr: {stderr_str}")
    } else {
        response_text
    }
}

/// Read session info (no pane to read in --print mode, show metadata).
pub async fn cc_read(mgr: &ClaudeCodeManager, name: &str) -> String {
    let sessions = mgr.sessions.read().await;
    match sessions.get(name) {
        Some(info) => {
            let sid = info.session_id.as_deref().unwrap_or("(none yet)");
            format!(
                "Session '{name}':\n  dir: {}\n  session_id: {sid}\n  created: {}\n  last_activity: {}",
                info.working_dir, info.created_at, info.last_activity
            )
        }
        None => format!("Session '{name}' not found."),
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
        let sid = info.session_id.as_deref().unwrap_or("new");
        lines.push(format!(
            "- {name} dir={} session={sid} last_activity={}",
            info.working_dir, info.last_activity
        ));
    }

    lines.join("\n")
}

/// Remove a session from tracking.
pub async fn cc_stop(mgr: &ClaudeCodeManager, name: &str) -> String {
    let mut sessions = mgr.sessions.write().await;
    if sessions.remove(name).is_some() {
        format!("Session '{name}' removed.")
    } else {
        format!("Session '{name}' not found.")
    }
}

/// Interrupt is not applicable in --print mode (process runs to completion).
/// This is kept for API compatibility — it returns a helpful message.
pub async fn cc_interrupt(_mgr: &ClaudeCodeManager, _name: &str) -> String {
    "cc_interrupt is not needed in --print mode. Each cc_send runs to completion or times out.".to_string()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse Claude Code --output-format json response.
/// Returns (response_text, Option<session_id>).
fn parse_cc_json_output(output: &str) -> (String, Option<String>) {
    // The JSON output format returns a single JSON object with result and session_id
    let trimmed = output.trim();

    // Try parsing as a single JSON object
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
        let session_id = val["session_id"].as_str().map(|s| s.to_string());
        let result = val["result"].as_str().unwrap_or("").to_string();

        if !result.is_empty() {
            return (result, session_id);
        }

        // Sometimes the response is in "content" or nested differently
        if let Some(content) = val["content"].as_str() {
            return (content.to_string(), session_id);
        }

        // Fall through to return the whole JSON as text
        return (trimmed.to_string(), session_id);
    }

    // If not valid JSON, return raw output (--print text mode fallback)
    (trimmed.to_string(), None)
}
