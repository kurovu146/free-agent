use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::{info, warn};

use super::claude::ClaudeProvider;
use super::gemini::GeminiProvider;
use super::groq::GroqProvider;
use super::mistral::MistralProvider;
use super::types::*;

struct KeyPool {
    keys: Vec<String>,
    index: AtomicUsize,
}

impl KeyPool {
    fn new(keys: Vec<String>) -> Self {
        Self {
            keys,
            index: AtomicUsize::new(0),
        }
    }

    fn next_key(&self) -> Option<&str> {
        if self.keys.is_empty() {
            return None;
        }
        let idx = self.index.fetch_add(1, Ordering::Relaxed) % self.keys.len();
        Some(&self.keys[idx])
    }

    fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    fn len(&self) -> usize {
        self.keys.len()
    }
}

/// Enum-based provider dispatch (no dyn trait needed)
enum Provider {
    Claude(ClaudeProvider),
    Gemini(GeminiProvider),
    Groq(GroqProvider),
    Mistral(MistralProvider),
}

impl Provider {
    fn name(&self) -> &str {
        match self {
            Provider::Claude(_) => "claude",
            Provider::Gemini(_) => "gemini",
            Provider::Groq(_) => "groq",
            Provider::Mistral(_) => "mistral",
        }
    }

    async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        api_key: &str,
    ) -> Result<LlmResponse, ProviderError> {
        match self {
            Provider::Claude(p) => p.chat(messages, tools, api_key).await,
            Provider::Gemini(p) => p.chat(messages, tools, api_key).await,
            Provider::Groq(p) => p.chat(messages, tools, api_key).await,
            Provider::Mistral(p) => p.chat(messages, tools, api_key).await,
        }
    }
}

struct ProviderEntry {
    provider: Provider,
    keys: KeyPool,
}

/// Round-robin provider pool with automatic fallback
pub struct ProviderPool {
    providers: Vec<ProviderEntry>,
    default_idx: usize,
}

impl ProviderPool {
    pub fn new(
        claude_keys: Vec<String>,
        gemini_keys: Vec<String>,
        groq_keys: Vec<String>,
        mistral_keys: Vec<String>,
        default_provider: &str,
    ) -> Self {
        let mut providers = Vec::new();

        if !claude_keys.is_empty() {
            providers.push(ProviderEntry {
                provider: Provider::Claude(ClaudeProvider::new()),
                keys: KeyPool::new(claude_keys),
            });
        }
        if !gemini_keys.is_empty() {
            providers.push(ProviderEntry {
                provider: Provider::Gemini(GeminiProvider::new()),
                keys: KeyPool::new(gemini_keys),
            });
        }
        if !groq_keys.is_empty() {
            providers.push(ProviderEntry {
                provider: Provider::Groq(GroqProvider::new()),
                keys: KeyPool::new(groq_keys),
            });
        }
        if !mistral_keys.is_empty() {
            providers.push(ProviderEntry {
                provider: Provider::Mistral(MistralProvider::new()),
                keys: KeyPool::new(mistral_keys),
            });
        }

        let default_idx = providers
            .iter()
            .position(|p| p.provider.name() == default_provider)
            .unwrap_or(0);

        info!(
            "Provider pool: {} providers, default={}",
            providers.len(),
            providers.get(default_idx).map(|p| p.provider.name()).unwrap_or("none")
        );

        Self {
            providers,
            default_idx,
        }
    }

    /// Send a chat request, trying providers in order with fallback
    pub async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
    ) -> Result<(LlmResponse, String), ProviderError> {
        if self.providers.is_empty() {
            return Err(ProviderError::NoKeys);
        }

        let order = self.provider_order();

        for idx in order {
            let entry = &self.providers[idx];
            let provider_name = entry.provider.name().to_string();
            let num_keys = entry.keys.len();

            // Try all keys for this provider before moving to next provider
            for _attempt in 0..num_keys {
                let key = match entry.keys.next_key() {
                    Some(k) => k,
                    None => break,
                };

                info!("Trying provider: {provider_name} (key: {}...)", &key[..key.len().min(10)]);
                match entry.provider.chat(messages, tools, key).await {
                    Ok(response) => {
                        info!("Provider {provider_name} succeeded");
                        return Ok((response, provider_name));
                    }
                    Err(ProviderError::RateLimited) => {
                        warn!("{provider_name} RATE LIMITED (key: {}...), trying next key", &key[..key.len().min(10)]);
                        continue; // try next key of same provider
                    }
                    Err(ProviderError::AuthError(e)) => {
                        warn!("{provider_name} AUTH ERROR (key: {}...): {e}", &key[..key.len().min(10)]);
                        break; // auth error = skip this provider entirely
                    }
                    Err(e) => {
                        warn!("{provider_name} FAILED (key: {}...): {e}", &key[..key.len().min(10)]);
                        break; // other errors = skip this provider
                    }
                }
            }
        }

        Err(ProviderError::RequestError("All providers failed".into()))
    }

    fn provider_order(&self) -> Vec<usize> {
        let mut order = vec![self.default_idx];
        for i in 0..self.providers.len() {
            if i != self.default_idx {
                order.push(i);
            }
        }
        order
    }

    /// Send a chat request to a specific provider by name.
    /// Falls back to default pool behavior if the named provider is unavailable.
    pub async fn chat_with_provider(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        provider_name: &str,
    ) -> Result<(LlmResponse, String), ProviderError> {
        // Try the requested provider first
        if let Some(entry) = self.providers.iter().find(|p| p.provider.name() == provider_name) {
            if let Some(key) = entry.keys.next_key() {
                match entry.provider.chat(messages, tools, key).await {
                    Ok(response) => return Ok((response, provider_name.to_string())),
                    Err(e) => {
                        warn!("{provider_name} failed: {e}, falling back to pool");
                    }
                }
            }
        }

        // Fallback to round-robin
        self.chat(messages, tools).await
    }

    pub fn available_providers(&self) -> Vec<&str> {
        self.providers
            .iter()
            .filter(|p| !p.keys.is_empty())
            .map(|p| p.provider.name())
            .collect()
    }
}
