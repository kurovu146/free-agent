mod agent;
mod db;
mod provider;
mod skills;
mod tools;
mod config;
mod telegram;

use config::Config;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Init logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Load config
    let config = Config::from_env();

    tracing::info!("Free Agent v{}", env!("CARGO_PKG_VERSION"));
    tracing::info!(
        "Providers: claude={}, gemini={}, groq={}, mistral={}",
        config.claude_keys.len(),
        config.gemini_keys.len(),
        config.groq_keys.len(),
        config.mistral_keys.len()
    );

    // Start bot
    telegram::run_bot(config).await;
}
