use serde_json::json;

use crate::provider::{ToolDef, FunctionDef};
use crate::tools;
use crate::tools::gmail::GmailCreds;

/// Registry of all available tools with definitions and executor
pub struct ToolRegistry;

impl ToolRegistry {
    /// Get tool definitions to send to LLM
    pub fn definitions(gmail_configured: bool, system_tools_enabled: bool) -> Vec<ToolDef> {
        let mut defs = vec![
            // --- Web ---
            tool_def("web_search",
                "Search the web for information. Returns search results with titles, URLs, and snippets.",
                json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "The search query" }
                    },
                    "required": ["query"]
                }),
            ),
            tool_def("web_fetch",
                "Fetch a URL and extract readable text content from the page.",
                json!({
                    "type": "object",
                    "properties": {
                        "url": { "type": "string", "description": "The URL to fetch" }
                    },
                    "required": ["url"]
                }),
            ),
            // --- Memory ---
            tool_def("memory_save",
                "Save an important fact to long-term memory for future conversations.",
                json!({
                    "type": "object",
                    "properties": {
                        "fact": { "type": "string", "description": "The fact to remember" },
                        "category": {
                            "type": "string",
                            "enum": ["preference", "decision", "personal", "technical", "project", "workflow", "general"],
                            "description": "Category of the fact"
                        }
                    },
                    "required": ["fact"]
                }),
            ),
            tool_def("memory_search",
                "Search long-term memory for previously saved facts.",
                json!({
                    "type": "object",
                    "properties": {
                        "keyword": { "type": "string", "description": "Keyword to search for" }
                    },
                    "required": ["keyword"]
                }),
            ),
            tool_def("memory_list",
                "List all saved facts from long-term memory.",
                json!({
                    "type": "object",
                    "properties": {
                        "category": { "type": "string", "description": "Optional category filter" }
                    }
                }),
            ),
            tool_def("memory_delete",
                "Delete a specific memory by its ID.",
                json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer", "description": "The memory fact ID to delete" }
                    },
                    "required": ["id"]
                }),
            ),
            // --- Datetime ---
            tool_def("get_datetime",
                "Get current date and time in UTC and common timezones (Vietnam, US Eastern).",
                json!({ "type": "object", "properties": {} }),
            ),
        ];

        // System tools (Bash, Read, Write, Glob, Grep)
        if system_tools_enabled {
            defs.extend(vec![
                tool_def("bash",
                    "Execute a bash command and return stdout/stderr. Use for git, npm, docker, compilation, and other terminal operations. Commands run in the configured working directory.",
                    json!({
                        "type": "object",
                        "properties": {
                            "command": { "type": "string", "description": "The bash command to execute" },
                            "timeout": { "type": "integer", "description": "Timeout in seconds (default: 120, max: 600)" }
                        },
                        "required": ["command"]
                    }),
                ),
                tool_def("read",
                    "Read the contents of a file. Returns numbered lines. For large files, use offset and limit to read specific sections.",
                    json!({
                        "type": "object",
                        "properties": {
                            "file_path": { "type": "string", "description": "Path to the file to read" },
                            "offset": { "type": "integer", "description": "Line number to start from (0-indexed, default: 0)" },
                            "limit": { "type": "integer", "description": "Number of lines to read (default: 2000)" }
                        },
                        "required": ["file_path"]
                    }),
                ),
                tool_def("write",
                    "Write content to a file. Creates the file if it doesn't exist, overwrites if it does. Creates parent directories automatically.",
                    json!({
                        "type": "object",
                        "properties": {
                            "file_path": { "type": "string", "description": "Path to the file to write" },
                            "content": { "type": "string", "description": "The content to write" }
                        },
                        "required": ["file_path", "content"]
                    }),
                ),
                tool_def("glob",
                    "Find files matching a pattern. Returns up to 50 matching file paths sorted.",
                    json!({
                        "type": "object",
                        "properties": {
                            "pattern": { "type": "string", "description": "File name pattern (e.g. '*.rs', '*.ts', 'Cargo.toml')" },
                            "path": { "type": "string", "description": "Directory to search in (default: working directory)" }
                        },
                        "required": ["pattern"]
                    }),
                ),
                tool_def("grep",
                    "Search file contents using regex. Uses ripgrep if available, falls back to grep. Returns matching lines with file paths and line numbers.",
                    json!({
                        "type": "object",
                        "properties": {
                            "pattern": { "type": "string", "description": "Regex pattern to search for" },
                            "path": { "type": "string", "description": "File or directory to search in (default: working directory)" },
                            "glob": { "type": "string", "description": "File pattern filter (e.g. '*.rs', '*.ts')" },
                            "case_insensitive": { "type": "boolean", "description": "Case insensitive search (default: false)" },
                            "context": { "type": "integer", "description": "Number of context lines before and after each match" }
                        },
                        "required": ["pattern"]
                    }),
                ),
            ]);
        }

        // Gmail tools (only if configured)
        if gmail_configured {
            defs.extend(vec![
                tool_def("gmail_search",
                    "Search emails using Gmail query syntax. Returns email summaries (id, subject, from, date, snippet).",
                    json!({
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Gmail search query" },
                            "maxResults": { "type": "integer", "description": "Max results to return (default 10)" }
                        },
                        "required": ["query"]
                    }),
                ),
                tool_def("gmail_read",
                    "Read the full content of a specific email by its message ID.",
                    json!({
                        "type": "object",
                        "properties": {
                            "messageId": { "type": "string", "description": "The Gmail message ID" }
                        },
                        "required": ["messageId"]
                    }),
                ),
                tool_def("gmail_send",
                    "Send a new email. IMPORTANT: Always confirm with the user before sending.",
                    json!({
                        "type": "object",
                        "properties": {
                            "to": { "type": "string", "description": "Recipient email address" },
                            "subject": { "type": "string", "description": "Email subject" },
                            "body": { "type": "string", "description": "Email body text" }
                        },
                        "required": ["to", "subject", "body"]
                    }),
                ),
                tool_def("gmail_archive",
                    "Archive emails by removing the INBOX label.",
                    json!({
                        "type": "object",
                        "properties": {
                            "messageIds": { "type": "array", "items": { "type": "string" }, "description": "Array of message IDs" }
                        },
                        "required": ["messageIds"]
                    }),
                ),
                tool_def("gmail_trash",
                    "Move emails to trash (permanently deleted after 30 days).",
                    json!({
                        "type": "object",
                        "properties": {
                            "messageIds": { "type": "array", "items": { "type": "string" }, "description": "Array of message IDs" }
                        },
                        "required": ["messageIds"]
                    }),
                ),
                tool_def("gmail_label",
                    "Add or remove labels from emails.",
                    json!({
                        "type": "object",
                        "properties": {
                            "messageIds": { "type": "array", "items": { "type": "string" }, "description": "Array of message IDs" },
                            "addLabelIds": { "type": "array", "items": { "type": "string" }, "description": "Labels to add" },
                            "removeLabelIds": { "type": "array", "items": { "type": "string" }, "description": "Labels to remove" }
                        },
                        "required": ["messageIds"]
                    }),
                ),
                tool_def("gmail_list_labels",
                    "List all Gmail labels (system and custom).",
                    json!({ "type": "object", "properties": {} }),
                ),
                // --- Google Sheets ---
                tool_def("sheets_read",
                    "Read data from Google Sheets. Pass spreadsheet URL or ID.",
                    json!({
                        "type": "object",
                        "properties": {
                            "spreadsheetId": { "type": "string", "description": "Spreadsheet URL or ID" },
                            "range": { "type": "string", "description": "Range in A1 notation (e.g. Sheet1!A1:E10)" }
                        },
                        "required": ["spreadsheetId"]
                    }),
                ),
                tool_def("sheets_write",
                    "Write data to Google Sheets. Overwrites cells in the specified range.",
                    json!({
                        "type": "object",
                        "properties": {
                            "spreadsheetId": { "type": "string", "description": "Spreadsheet URL or ID" },
                            "range": { "type": "string", "description": "Range in A1 notation" },
                            "values": { "type": "array", "items": { "type": "array", "items": { "type": "string" } }, "description": "2D array of values" }
                        },
                        "required": ["spreadsheetId", "range", "values"]
                    }),
                ),
                tool_def("sheets_append",
                    "Append rows to the end of a Google Sheet.",
                    json!({
                        "type": "object",
                        "properties": {
                            "spreadsheetId": { "type": "string", "description": "Spreadsheet URL or ID" },
                            "range": { "type": "string", "description": "Range in A1 notation" },
                            "values": { "type": "array", "items": { "type": "array", "items": { "type": "string" } }, "description": "2D array of values" }
                        },
                        "required": ["spreadsheetId", "range", "values"]
                    }),
                ),
                tool_def("sheets_list",
                    "List all sheets (tabs) in a spreadsheet.",
                    json!({
                        "type": "object",
                        "properties": {
                            "spreadsheetId": { "type": "string", "description": "Spreadsheet URL or ID" }
                        },
                        "required": ["spreadsheetId"]
                    }),
                ),
                tool_def("sheets_create_tab",
                    "Create a new sheet tab in a spreadsheet.",
                    json!({
                        "type": "object",
                        "properties": {
                            "spreadsheetId": { "type": "string", "description": "Spreadsheet URL or ID" },
                            "title": { "type": "string", "description": "Name for the new tab" }
                        },
                        "required": ["spreadsheetId", "title"]
                    }),
                ),
            ]);
        }

        defs
    }

    /// Execute a tool by name with given arguments
    pub async fn execute(
        tool_name: &str,
        args_json: &str,
        user_id: u64,
        db: &crate::db::Database,
        gmail_creds: &GmailCreds,
        working_dir: &str,
        bash_timeout: u64,
    ) -> String {
        let args: serde_json::Value = serde_json::from_str(args_json).unwrap_or_default();

        match tool_name {
            // --- Web ---
            "web_search" => {
                let query = args["query"].as_str().unwrap_or("");
                tools::web_search(query).await
            }
            "web_fetch" => {
                let url = args["url"].as_str().unwrap_or("");
                tools::web_fetch(url).await
            }
            // --- Memory ---
            "memory_save" => {
                let fact = args["fact"].as_str().unwrap_or("");
                let category = args["category"].as_str().unwrap_or("general");
                tools::memory_save(db, user_id, fact, category).await
            }
            "memory_search" => {
                let keyword = args["keyword"].as_str().unwrap_or("");
                tools::memory_search(db, user_id, keyword).await
            }
            "memory_list" => {
                let category = args["category"].as_str();
                tools::memory_list(db, user_id, category).await
            }
            "memory_delete" => {
                let id = args["id"].as_i64().unwrap_or(0);
                tools::memory_delete(db, user_id, id).await
            }
            // --- Datetime ---
            "get_datetime" => tools::get_datetime().await,
            // --- System tools ---
            "bash" => {
                let command = args["command"].as_str().unwrap_or("");
                let timeout = args["timeout"].as_u64().unwrap_or(bash_timeout).min(600);
                tools::bash_exec(command, working_dir, timeout).await
            }
            "read" => {
                let file_path = args["file_path"].as_str().unwrap_or("");
                let offset = args["offset"].as_u64().map(|v| v as usize);
                let limit = args["limit"].as_u64().map(|v| v as usize);
                tools::file_read(file_path, offset, limit).await
            }
            "write" => {
                let file_path = args["file_path"].as_str().unwrap_or("");
                let content = args["content"].as_str().unwrap_or("");
                tools::file_write(file_path, content).await
            }
            "glob" => {
                let pattern = args["pattern"].as_str().unwrap_or("");
                let path = args["path"].as_str().or(Some(working_dir));
                tools::glob_search(pattern, path).await
            }
            "grep" => {
                let pattern = args["pattern"].as_str().unwrap_or("");
                let path = args["path"].as_str().or(Some(working_dir));
                let glob_filter = args["glob"].as_str();
                let case_insensitive = args["case_insensitive"].as_bool().unwrap_or(false);
                let context = args["context"].as_u64().map(|v| v as u32);
                tools::grep_search(pattern, path, glob_filter, case_insensitive, context).await
            }
            // --- Gmail ---
            "gmail_search" => {
                let query = args["query"].as_str().unwrap_or("");
                let max = args["maxResults"].as_u64().unwrap_or(10) as u32;
                tools::gmail_search(query, max, gmail_creds).await
            }
            "gmail_read" => {
                let id = args["messageId"].as_str().unwrap_or("");
                tools::gmail_read(id, gmail_creds).await
            }
            "gmail_send" => {
                let to = args["to"].as_str().unwrap_or("");
                let subject = args["subject"].as_str().unwrap_or("");
                let body = args["body"].as_str().unwrap_or("");
                tools::gmail_send(to, subject, body, gmail_creds).await
            }
            "gmail_archive" => {
                let ids = parse_string_array(&args["messageIds"]);
                tools::gmail_archive(&ids, gmail_creds).await
            }
            "gmail_trash" => {
                let ids = parse_string_array(&args["messageIds"]);
                tools::gmail_trash(&ids, gmail_creds).await
            }
            "gmail_label" => {
                let ids = parse_string_array(&args["messageIds"]);
                let add = parse_string_array(&args["addLabelIds"]);
                let remove = parse_string_array(&args["removeLabelIds"]);
                let add_refs: Vec<&str> = add.iter().map(|s| s.as_str()).collect();
                let remove_refs: Vec<&str> = remove.iter().map(|s| s.as_str()).collect();
                tools::gmail_label(&ids, &add_refs, &remove_refs, gmail_creds).await
            }
            "gmail_list_labels" => tools::gmail_list_labels(gmail_creds).await,
            // --- Sheets ---
            "sheets_read" => {
                let sid = args["spreadsheetId"].as_str().unwrap_or("");
                let range = args["range"].as_str();
                tools::sheets_read(sid, range, gmail_creds).await
            }
            "sheets_write" => {
                let sid = args["spreadsheetId"].as_str().unwrap_or("");
                let range = args["range"].as_str().unwrap_or("");
                let values = parse_2d_array(&args["values"]);
                tools::sheets_write(sid, range, values, gmail_creds).await
            }
            "sheets_append" => {
                let sid = args["spreadsheetId"].as_str().unwrap_or("");
                let range = args["range"].as_str().unwrap_or("");
                let values = parse_2d_array(&args["values"]);
                tools::sheets_append(sid, range, values, gmail_creds).await
            }
            "sheets_list" => {
                let sid = args["spreadsheetId"].as_str().unwrap_or("");
                tools::sheets_list(sid, gmail_creds).await
            }
            "sheets_create_tab" => {
                let sid = args["spreadsheetId"].as_str().unwrap_or("");
                let title = args["title"].as_str().unwrap_or("");
                tools::sheets_create_tab(sid, title, gmail_creds).await
            }
            _ => format!("Unknown tool: {tool_name}"),
        }
    }
}

fn tool_def(name: &str, description: &str, parameters: serde_json::Value) -> ToolDef {
    ToolDef {
        tool_type: "function".into(),
        function: FunctionDef {
            name: name.into(),
            description: description.into(),
            parameters,
        },
    }
}

fn parse_string_array(val: &serde_json::Value) -> Vec<String> {
    val.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_2d_array(val: &serde_json::Value) -> Vec<Vec<String>> {
    val.as_array()
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    row.as_array()
                        .map(|cells| {
                            cells
                                .iter()
                                .map(|c| c.as_str().unwrap_or("").to_string())
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default()
}
