use std::env;

use crate::tools::gmail::GmailCreds;

#[derive(Debug, Clone)]
pub struct Config {
    pub telegram_bot_token: String,
    pub allowed_users: Vec<u64>,

    // Provider keys (multiple per provider for round-robin)
    pub claude_keys: Vec<String>,
    pub gemini_keys: Vec<String>,
    pub groq_keys: Vec<String>,
    pub mistral_keys: Vec<String>,

    // Defaults
    pub default_provider: String,
    pub max_agent_turns: usize,
    #[allow(dead_code)]
    pub max_queue_depth: usize,

    // Google OAuth (Gmail + Sheets)
    pub gmail_creds: GmailCreds,

    // System tools
    pub enable_system_tools: bool,
    pub working_dir: String,
    pub bash_timeout: u64,

    // Claude Code (tmux-based control)
    pub enable_claude_code: bool,
    pub claude_code_path: String,
    pub cc_timeout: u64,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv_override().ok();

        Self {
            telegram_bot_token: env::var("TELEGRAM_BOT_TOKEN")
                .expect("TELEGRAM_BOT_TOKEN is required"),
            allowed_users: env::var("TELEGRAM_ALLOWED_USERS")
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.trim().parse().ok())
                .collect(),
            claude_keys: parse_keys("CLAUDE_API_KEYS"),
            gemini_keys: parse_keys("GEMINI_API_KEYS"),
            groq_keys: parse_keys("GROQ_API_KEYS"),
            mistral_keys: parse_keys("MISTRAL_API_KEYS"),
            default_provider: env::var("DEFAULT_PROVIDER").unwrap_or_else(|_| "gemini".into()),
            max_agent_turns: env::var("MAX_AGENT_TURNS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            max_queue_depth: env::var("MAX_QUEUE_DEPTH")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
            gmail_creds: GmailCreds {
                client_id: env::var("GMAIL_CLIENT_ID").unwrap_or_default(),
                client_secret: env::var("GMAIL_CLIENT_SECRET").unwrap_or_default(),
                refresh_token: env::var("GMAIL_REFRESH_TOKEN").unwrap_or_default(),
            },
            enable_system_tools: env::var("ENABLE_SYSTEM_TOOLS")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            working_dir: expand_tilde(&env::var("WORKING_DIR").unwrap_or_else(|_| ".".into())),
            bash_timeout: env::var("BASH_TIMEOUT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(120),
            enable_claude_code: env::var("ENABLE_CLAUDE_CODE")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            claude_code_path: env::var("CLAUDE_CODE_PATH")
                .unwrap_or_else(|_| "claude".into()),
            cc_timeout: env::var("CC_TIMEOUT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
        }
    }
}

/// Expand `~` to home directory. Works on both macOS and Linux.
fn expand_tilde(path: &str) -> String {
    if path == "~" || path.starts_with("~/") {
        if let Some(home) = env::var("HOME").ok() {
            return path.replacen('~', &home, 1);
        }
    }
    path.to_string()
}

fn parse_keys(env_var: &str) -> Vec<String> {
    env::var(env_var)
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
