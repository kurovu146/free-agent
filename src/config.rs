use std::env;

use crate::tools::gmail::GmailCreds;

#[derive(Debug, Clone)]
pub struct Config {
    pub telegram_bot_token: String,
    pub allowed_users: Vec<u64>,

    // Provider keys (multiple per provider for round-robin)
    pub gemini_keys: Vec<String>,
    pub groq_keys: Vec<String>,
    pub mistral_keys: Vec<String>,

    // Defaults
    pub default_provider: String,
    pub max_agent_turns: usize,
    pub max_queue_depth: usize,

    // Google OAuth (Gmail + Sheets)
    pub gmail_creds: GmailCreds,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        Self {
            telegram_bot_token: env::var("TELEGRAM_BOT_TOKEN")
                .expect("TELEGRAM_BOT_TOKEN is required"),
            allowed_users: env::var("TELEGRAM_ALLOWED_USERS")
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.trim().parse().ok())
                .collect(),
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
        }
    }
}

fn parse_keys(env_var: &str) -> Vec<String> {
    env::var(env_var)
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
