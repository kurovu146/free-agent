# Free Agent

Bot AI Telegram nhẹ, chạy bằng **LLM API miễn phí** (Gemini, Groq, Mistral) với cơ chế xoay vòng key và tự động chuyển provider khi lỗi.

Viết bằng Rust, tối ưu tài nguyên (~5-15MB RAM, binary ~7MB).

## Tính năng

- **Đa provider**: Gemini 2.5 Flash, Groq Llama 3.3 70B, Mistral Small, Claude (tùy chọn)
- **Xoay vòng key thông minh**: Nhiều API key mỗi provider, luân phiên tự động; thử tất cả key trước khi chuyển provider
- **Tự động fallback**: Nếu một provider lỗi hoặc rate limit, chuyển sang provider tiếp theo
- **Agent loop**: LLM gọi tool, nhận kết quả, gọi tiếp — tối đa N lượt mỗi tin nhắn
- **Lịch sử hội thoại**: Lưu 10 cặp tin nhắn gần nhất mỗi phiên hội thoại (SQLite)
- **Công cụ (19+ tools)**:
  - Tìm kiếm web (DuckDuckGo) + đọc nội dung URL
  - Bộ nhớ dài hạn mỗi user (SQLite với FTS5 tìm kiếm toàn văn)
  - Plan & Todo: lập kế hoạch và quản lý task liên tục
  - System tools: bash, đọc/ghi file, glob, grep (cần bật)
  - Gmail & Google Sheets (cần bật, yêu cầu OAuth2)
  - Ngày giờ hiện tại
- **Skills system**: File markdown trong `skills/` tự động inject vào system prompt
- **Streaming UX**: Cập nhật tiến trình thời gian thực — hiển thị tool đang chạy
- **Footer tools**: Mỗi phản hồi hiển thị tool đã dùng, số lần gọi, số turns, và thời gian xử lý
- **Chống ảo giác**: Phát hiện và cảnh báo khi model bịa kết quả tool
- **An toàn UTF-8**: Xử lý Unicode đúng khi chia nhỏ tin nhắn (CJK, emoji, tiếng Việt)

## Bắt đầu nhanh

### 1. Lấy API Key (tất cả miễn phí)

| Provider | Lấy key ở đâu | Free Tier |
|----------|--------------|-----------|
| [Google Gemini](https://aistudio.google.com/apikey) | AI Studio | 10 req/phút, 250 req/ngày |
| [Groq](https://console.groq.com/keys) | Console | 30 req/phút, 1K req/ngày |
| [Mistral](https://console.mistral.ai/api-keys) | Console | 2 req/phút, 1B token/tháng |

### 2. Tạo Telegram Bot

Nhắn [@BotFather](https://t.me/BotFather), tạo bot mới, lấy token.

### 3. Cấu hình

```bash
cp .env.example .env
# Chỉnh .env với các key của bạn
```

### 4. Build & Chạy

```bash
cargo build --release
./target/release/free-agent
```

## Cấu hình

| Biến môi trường | Bắt buộc | Mô tả |
|---|---|---|
| `TELEGRAM_BOT_TOKEN` | Có | Token bot Telegram từ BotFather |
| `TELEGRAM_ALLOWED_USERS` | Không | Danh sách user ID (cách nhau bởi dấu phẩy, bỏ trống = cho phép tất cả) |
| `GEMINI_API_KEYS` | Không* | Các Gemini API key (cách nhau bởi dấu phẩy) |
| `GROQ_API_KEYS` | Không* | Các Groq API key (cách nhau bởi dấu phẩy) |
| `MISTRAL_API_KEYS` | Không* | Các Mistral API key (cách nhau bởi dấu phẩy) |
| `CLAUDE_API_KEYS` | Không* | Các Anthropic API key (cách nhau bởi dấu phẩy) |
| `DEFAULT_PROVIDER` | Không | `gemini` (mặc định), `groq`, `mistral`, hoặc `claude` |
| `MAX_AGENT_TURNS` | Không | Số lượt tối đa gọi tool mỗi tin nhắn (mặc định: 10) |
| `ENABLE_SYSTEM_TOOLS` | Không | Bật bash/read/write/glob/grep (mặc định: false) |
| `WORKING_DIR` | Không | Thư mục làm việc cho system tools (mặc định: `.`) |
| `BASH_TIMEOUT` | Không | Timeout lệnh shell tính bằng giây (mặc định: 120) |
| `GMAIL_CLIENT_ID` | Không | Google OAuth2 client ID (dùng Gmail/Sheets) |
| `GMAIL_CLIENT_SECRET` | Không | Google OAuth2 client secret |
| `GMAIL_REFRESH_TOKEN` | Không | Google OAuth2 refresh token |
| `RUST_LOG` | Không | Mức log: `info`, `debug`, `warn` (mặc định: `info`) |

*Phải có ít nhất một provider được cấu hình key.

## Công cụ (Tools)

| Tool | Mô tả | Điều kiện |
|------|-------|:---------:|
| `web_search` | Tìm kiếm web qua DuckDuckGo | Luôn có |
| `web_fetch` | Đọc nội dung từ URL | Luôn có |
| `memory_save` | Lưu thông tin vào bộ nhớ dài hạn | Luôn có |
| `memory_search` | Tìm kiếm bộ nhớ toàn văn | Luôn có |
| `memory_list` | Liệt kê tất cả thông tin đã lưu | Luôn có |
| `memory_delete` | Xóa thông tin đã lưu | Luôn có |
| `get_datetime` | Lấy ngày giờ hiện tại | Luôn có |
| `plan_read` | Đọc plan hiện tại | Luôn có |
| `plan_write` | Viết/cập nhật plan | Luôn có |
| `todo_add` | Thêm todo item mới | Luôn có |
| `todo_list` | Liệt kê tất cả todos | Luôn có |
| `todo_update` | Cập nhật trạng thái todo (pending/in_progress/completed) | Luôn có |
| `todo_delete` | Xóa todo item | Luôn có |
| `todo_clear_completed` | Xóa tất cả todo đã hoàn thành | Luôn có |
| `bash` | Thực thi lệnh shell | System Tools |
| `read` | Đọc nội dung file | System Tools |
| `write` | Ghi/tạo file | System Tools |
| `glob` | Tìm file theo pattern | System Tools |
| `grep` | Tìm kiếm nội dung file | System Tools |
| `gmail_search` | Tìm kiếm email | Gmail OAuth |
| `gmail_read` | Đọc nội dung email | Gmail OAuth |
| `gmail_send` | Gửi email | Gmail OAuth |
| `gmail_archive` | Archive email | Gmail OAuth |
| `gmail_trash` | Chuyển email vào thùng rác | Gmail OAuth |
| `gmail_label` | Thêm/xóa nhãn email | Gmail OAuth |
| `gmail_list_labels` | Liệt kê tất cả nhãn Gmail | Gmail OAuth |
| `sheets_read` | Đọc dữ liệu spreadsheet | Gmail OAuth |
| `sheets_write` | Ghi vào spreadsheet | Gmail OAuth |
| `sheets_append` | Thêm hàng vào spreadsheet | Gmail OAuth |
| `sheets_list` | Liệt kê các tab sheet | Gmail OAuth |
| `sheets_create_tab` | Tạo tab sheet mới | Gmail OAuth |

## Skills

Thêm file `.md` vào thư mục `skills/`. Chúng được tự động tải vào system prompt khi khởi động.

Ví dụ `skills/coding.md`:
```markdown
# Trợ lý lập trình

## Hướng dẫn
- Luôn thêm language tag cho code block
- Giải thích code ngắn gọn
- Đề xuất cải tiến khi review code
```

## Kiến trúc

```
User (Telegram)
  │
  ▼
Telegram Handler
  ├── Gửi "⏳ Đang xử lý..." (tin nhắn tiến trình)
  ├── Tải lịch sử hội thoại (10 cặp gần nhất từ SQLite)
  ├── Build system prompt (base + skills + memory context)
  │
  ▼
Agent Loop (tối đa N lượt)
  ├── Gọi LLM ──► Provider Pool (xoay vòng + fallback)
  │                  ├── Gemini 2.5 Flash (keys: k1, k2, k3, k4...)
  │                  ├── Groq Llama 3.3 70B (keys: k1, k2...)
  │                  ├── Mistral Small (keys: k1...)
  │                  └── Claude Sonnet (tùy chọn, keys: k1...)
  │
  ├── LLM trả về tool calls?
  │     ├── Có → Thực thi tools → Cập nhật tin nhắn tiến trình → Lặp lại
  │     └── Không → Trả về phản hồi văn bản
  │
  ▼
Lưu phản hồi vào lịch sử session
  │
  ▼
Cập nhật tin nhắn tiến trình → Phản hồi cuối + footer tools
```

```
src/
├── main.rs              # Entry point
├── config.rs            # Cấu hình từ biến môi trường
├── agent/
│   ├── loop_runner.rs   # Agent loop với injection lịch sử + progress callback
│   └── tool_registry.rs # Định nghĩa tool + dispatch
├── provider/
│   ├── pool.rs          # Round-robin pool với retry từng key + fallback
│   ├── gemini.rs        # Gemini 2.5 Flash (OpenAI-compatible)
│   ├── groq.rs          # Groq Llama 3.3 70B (OpenAI-compatible)
│   ├── mistral.rs       # Mistral Small (OpenAI-compatible)
│   ├── claude.rs        # Claude Sonnet (Anthropic API)
│   └── types.rs         # Các kiểu dùng chung (Message, ToolCall, v.v.)
├── telegram/
│   ├── handler.rs       # Xử lý tin nhắn + lịch sử session + streaming UX
│   └── formatter.rs     # Icon tool, footer, chia nhỏ tin nhắn
├── tools/
│   ├── web.rs           # web_search + web_fetch
│   ├── memory.rs        # memory_save/search/list/delete
│   ├── planning.rs      # plan_read/write + todo_add/list/update/delete
│   ├── datetime.rs      # get_datetime
│   ├── system.rs        # bash/read/write/glob/grep
│   ├── gmail.rs         # Các tool Gmail API
│   └── sheets.rs        # Các tool Google Sheets API
├── db/
│   └── mod.rs           # SQLite: memory (FTS5), sessions, plans, todos, query logs
└── skills/
    └── mod.rs           # Tải file .md từ thư mục skills/
```

## Lệnh bot

| Lệnh | Mô tả |
|------|-------|
| `/start` | Thông tin & trạng thái bot |
| `/help` | Hiển thị các lệnh khả dụng |
| `/new` | Bắt đầu hội thoại mới (xóa lịch sử) |
| `/tools` | Liệt kê các tool khả dụng |
| `/memory` | Liệt kê thông tin đã lưu |
| `/providers` | Hiển thị các LLM provider |

**Chọn provider**: Thêm `dùng claude`, `use gemini`, v.v. trước tin nhắn để chọn provider cho 1 tin nhắn.

## Tài nguyên sử dụng

| Chỉ số | Giá trị |
|--------|---------|
| Kích thước binary | ~7 MB (stripped, LTO) |
| RAM | ~5-15 MB |
| Dependencies | Tối giản (rustls, không cần OpenSSL) |

## Giấy phép

MIT
