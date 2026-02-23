# Free Agent

A lightweight Telegram AI bot powered by **free LLM APIs** (Gemini, Groq, Mistral) with round-robin key rotation and automatic fallback.

Built in Rust for minimal resource usage (~5-15MB RAM).

## Features

- **Multi-provider**: Gemini 2.5 Flash, Groq Llama 3.3 70B, Mistral Small
- **Round-robin keys**: Multiple API keys per provider, auto-rotated to avoid rate limits
- **Auto-fallback**: If one provider hits rate limit, seamlessly tries the next
- **Tool calling**: Web search (DuckDuckGo), persistent memory (SQLite)
- **Skills system**: Markdown files in `skills/` injected into system prompt
- **Memory**: Per-user long-term memory with FTS5 search

## Quick Start

### 1. Get API Keys (all free)

| Provider | Get Key | Free Tier |
|----------|---------|-----------|
| [Google Gemini](https://aistudio.google.com/apikey) | AI Studio | 10 RPM, 250 req/day |
| [Groq](https://console.groq.com/keys) | Console | 30 RPM, 1K req/day |
| [Mistral](https://console.mistral.ai/api-keys) | Console | 2 RPM, 1B tokens/month |

### 2. Create a Telegram Bot

Message [@BotFather](https://t.me/BotFather), create a new bot, get the token.

### 3. Configure

```bash
cp .env.example .env
# Edit .env with your keys
```

### 4. Build & Run

```bash
cargo build --release
./target/release/free-agent
```

Or with cargo directly:
```bash
cargo run
```

## Configuration

| Env Variable | Required | Description |
|---|---|---|
| `TELEGRAM_BOT_TOKEN` | Yes | Telegram bot token from BotFather |
| `TELEGRAM_ALLOWED_USERS` | No | Comma-separated user IDs (empty = allow all) |
| `GEMINI_API_KEYS` | No* | Comma-separated Gemini API keys |
| `GROQ_API_KEYS` | No* | Comma-separated Groq API keys |
| `MISTRAL_API_KEYS` | No* | Comma-separated Mistral API keys |
| `DEFAULT_PROVIDER` | No | `gemini` (default), `groq`, or `mistral` |
| `MAX_AGENT_TURNS` | No | Max tool-call loops per message (default: 10) |

*At least one provider must have keys configured.

## Skills

Add `.md` files to the `skills/` directory. They are automatically loaded into the system prompt.

Example `skills/coding.md`:
```markdown
# Coding Assistant

## Guidelines
- Always include language tags on code blocks
- Explain code concisely
- Suggest improvements when reviewing
```

## Architecture

```
User (Telegram) → Bot → Agent Loop
                          ├── Provider Pool (round-robin + fallback)
                          │     ├── Gemini (keys: k1, k2, k3...)
                          │     ├── Groq (keys: k1, k2...)
                          │     └── Mistral (keys: k1...)
                          └── Tool Executor
                                ├── web_search (DuckDuckGo)
                                ├── memory_save
                                ├── memory_search
                                └── memory_list
```

## Commands

| Command | Description |
|---------|-------------|
| `/start` | Bot info |
| `/help` | Show commands |
| `/memory` | List saved facts |
| `/providers` | Show available providers |

## License

MIT
