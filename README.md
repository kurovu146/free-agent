# Free Agent

A lightweight Telegram AI bot powered by **free LLM APIs** (Gemini, Groq, Mistral) with round-robin key rotation and automatic fallback.

Built in Rust for minimal resource usage (~5-15MB RAM, ~7MB binary).

## Features

- **Multi-provider**: Gemini 2.5 Flash, Groq Llama 3.3 70B, Mistral Small
- **Round-robin keys**: Multiple API keys per provider, auto-rotated to avoid rate limits
- **Auto-fallback**: If one provider hits rate limit or errors, seamlessly tries the next
- **Agent loop**: LLM calls tools, gets results, calls again — up to N turns per message
- **Tool calling**:
  - Web search (DuckDuckGo) + URL fetch
  - Persistent memory per user (SQLite with FTS5 full-text search)
  - System tools: bash, file read/write, glob, grep (opt-in)
  - Gmail & Google Sheets (opt-in, requires OAuth2)
  - Date/time
- **Skills system**: Markdown files in `skills/` injected into system prompt
- **Streaming UX**: Real-time progress updates — shows which tool is running
- **Tools footer**: Every response shows which tools were used and response time
- **UTF-8 safe**: Proper Unicode handling for message splitting (CJK, emoji, Vietnamese)

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
| `ENABLE_SYSTEM_TOOLS` | No | Enable bash/read/write/glob/grep (default: false) |
| `WORKING_DIR` | No | Working directory for system tools (default: `.`) |
| `BASH_TIMEOUT` | No | Shell command timeout in seconds (default: 120) |
| `GMAIL_CLIENT_ID` | No | Google OAuth2 client ID (for Gmail/Sheets) |
| `GMAIL_CLIENT_SECRET` | No | Google OAuth2 client secret |
| `GMAIL_REFRESH_TOKEN` | No | Google OAuth2 refresh token |
| `RUST_LOG` | No | Log level: `info`, `debug`, `warn` (default: `info`) |

*At least one provider must have keys configured.

## Tools

| Tool | Description | Always Available |
|------|-------------|:---:|
| `web_search` | Search the web via DuckDuckGo | Yes |
| `web_fetch` | Fetch and extract content from URLs | Yes |
| `memory_save` | Save a fact to long-term memory | Yes |
| `memory_search` | Search memory with full-text search | Yes |
| `memory_list` | List all saved facts | Yes |
| `memory_delete` | Delete a saved fact | Yes |
| `get_datetime` | Get current date/time | Yes |
| `bash` | Execute shell commands | System Tools |
| `read` | Read file contents | System Tools |
| `write` | Write/create files | System Tools |
| `glob` | Find files by pattern | System Tools |
| `grep` | Search file contents | System Tools |
| `gmail_search` | Search emails | Gmail OAuth |
| `gmail_read` | Read email content | Gmail OAuth |
| `gmail_send` | Send email | Gmail OAuth |
| `gmail_archive` | Archive emails | Gmail OAuth |
| `gmail_trash` | Move emails to trash | Gmail OAuth |
| `gmail_label` | Add/remove email labels | Gmail OAuth |
| `gmail_list_labels` | List all Gmail labels | Gmail OAuth |
| `sheets_read` | Read spreadsheet data | Gmail OAuth |
| `sheets_write` | Write to spreadsheet | Gmail OAuth |
| `sheets_append` | Append rows to spreadsheet | Gmail OAuth |
| `sheets_list` | List sheet tabs | Gmail OAuth |
| `sheets_create_tab` | Create new sheet tab | Gmail OAuth |

## Skills

Add `.md` files to the `skills/` directory. They are automatically loaded into the system prompt at startup.

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
User (Telegram)
  │
  ▼
Telegram Handler
  ├── Send "⏳ Đang xử lý..." (progress message)
  ├── Build system prompt (base + skills + memory context)
  │
  ▼
Agent Loop (max N turns)
  ├── Call LLM ──► Provider Pool (round-robin + fallback)
  │                  ├── Gemini 2.5 Flash (keys: k1, k2...)
  │                  ├── Groq Llama 3.3 70B (keys: k1, k2...)
  │                  └── Mistral Small (keys: k1...)
  │
  ├── LLM returns tool calls?
  │     ├── Yes → Execute tools → Update progress message → Loop
  │     └── No  → Return text response
  │
  ▼
Edit progress message → Final response + tools footer
```

```
src/
├── main.rs              # Entry point
├── config.rs            # Environment config
├── agent/
│   ├── loop_runner.rs   # Agent loop with progress callback
│   └── tool_registry.rs # Tool definitions + dispatch
├── provider/
│   ├── pool.rs          # Round-robin pool with fallback
│   ├── gemini.rs        # Gemini (OpenAI-compatible)
│   ├── groq.rs          # Groq (OpenAI-compatible)
│   ├── mistral.rs       # Mistral (OpenAI-compatible)
│   └── types.rs         # Shared types (Message, ToolCall, etc.)
├── telegram/
│   ├── handler.rs       # Message handling + streaming UX
│   └── formatter.rs     # Tool icons, footer, message splitting
├── tools/
│   ├── web.rs           # web_search + web_fetch
│   ├── memory.rs        # memory_save/search/list/delete
│   ├── datetime.rs      # get_datetime
│   ├── system.rs        # bash/read/write/glob/grep
│   ├── gmail.rs         # Gmail API tools
│   └── sheets.rs        # Google Sheets API tools
├── db/
│   └── mod.rs           # SQLite: memory (FTS5), sessions, query logs
└── skills/
    └── mod.rs           # Load .md files from skills/ directory
```

## Commands

| Command | Description |
|---------|-------------|
| `/start` | Bot info & status |
| `/help` | Show available commands |
| `/tools` | List available tools |
| `/memory` | List saved facts |
| `/providers` | Show LLM providers |

## Resource Usage

| Metric | Value |
|--------|-------|
| Binary size | ~7 MB (stripped, LTO) |
| RAM usage | ~5-15 MB |
| Dependencies | Minimal (rustls, no OpenSSL) |

## License

MIT
