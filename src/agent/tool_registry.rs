use serde_json::json;

use crate::provider::{ToolDef, FunctionDef};
use crate::tools;
use crate::tools::gmail::GmailCreds;

/// Registry of all available tools with definitions and executor
pub struct ToolRegistry;

impl ToolRegistry {
    /// Get tool definitions to send to LLM (conditionally includes Gmail/Sheets if configured)
    pub fn definitions(gmail_configured: bool) -> Vec<ToolDef> {
        let mut defs = vec![
            // --- Web ---
            ToolDef {
                tool_type: "function".into(),
                function: FunctionDef {
                    name: "web_search".into(),
                    description: "Search the web for information. Returns search results with titles, URLs, and snippets.".into(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "The search query"
                            }
                        },
                        "required": ["query"]
                    }),
                },
            },
            ToolDef {
                tool_type: "function".into(),
                function: FunctionDef {
                    name: "web_fetch".into(),
                    description: "Fetch a URL and extract readable text content from the page.".into(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "url": {
                                "type": "string",
                                "description": "The URL to fetch"
                            }
                        },
                        "required": ["url"]
                    }),
                },
            },
            // --- Memory ---
            ToolDef {
                tool_type: "function".into(),
                function: FunctionDef {
                    name: "memory_save".into(),
                    description: "Save an important fact to long-term memory for future conversations.".into(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "fact": {
                                "type": "string",
                                "description": "The fact to remember"
                            },
                            "category": {
                                "type": "string",
                                "enum": ["preference", "decision", "personal", "technical", "project", "workflow", "general"],
                                "description": "Category of the fact"
                            }
                        },
                        "required": ["fact"]
                    }),
                },
            },
            ToolDef {
                tool_type: "function".into(),
                function: FunctionDef {
                    name: "memory_search".into(),
                    description: "Search long-term memory for previously saved facts.".into(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "keyword": {
                                "type": "string",
                                "description": "Keyword to search for"
                            }
                        },
                        "required": ["keyword"]
                    }),
                },
            },
            ToolDef {
                tool_type: "function".into(),
                function: FunctionDef {
                    name: "memory_list".into(),
                    description: "List all saved facts from long-term memory.".into(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "category": {
                                "type": "string",
                                "description": "Optional category filter"
                            }
                        }
                    }),
                },
            },
            ToolDef {
                tool_type: "function".into(),
                function: FunctionDef {
                    name: "memory_delete".into(),
                    description: "Delete a specific memory by its ID.".into(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "id": {
                                "type": "integer",
                                "description": "The memory fact ID to delete"
                            }
                        },
                        "required": ["id"]
                    }),
                },
            },
            // --- Datetime ---
            ToolDef {
                tool_type: "function".into(),
                function: FunctionDef {
                    name: "get_datetime".into(),
                    description: "Get current date and time in UTC and common timezones (Vietnam, US Eastern).".into(),
                    parameters: json!({
                        "type": "object",
                        "properties": {}
                    }),
                },
            },
        ];

        // Gmail tools (only if configured)
        if gmail_configured {
            defs.extend(vec![
                ToolDef {
                    tool_type: "function".into(),
                    function: FunctionDef {
                        name: "gmail_search".into(),
                        description: "Search emails using Gmail query syntax. Returns email summaries (id, subject, from, date, snippet). Use operators like: is:unread, from:user@example.com, subject:keyword, newer_than:2d, has:attachment, label:name.".into(),
                        parameters: json!({
                            "type": "object",
                            "properties": {
                                "query": {
                                    "type": "string",
                                    "description": "Gmail search query"
                                },
                                "maxResults": {
                                    "type": "integer",
                                    "description": "Max results to return (default 10)"
                                }
                            },
                            "required": ["query"]
                        }),
                    },
                },
                ToolDef {
                    tool_type: "function".into(),
                    function: FunctionDef {
                        name: "gmail_read".into(),
                        description: "Read the full content of a specific email by its message ID.".into(),
                        parameters: json!({
                            "type": "object",
                            "properties": {
                                "messageId": {
                                    "type": "string",
                                    "description": "The Gmail message ID"
                                }
                            },
                            "required": ["messageId"]
                        }),
                    },
                },
                ToolDef {
                    tool_type: "function".into(),
                    function: FunctionDef {
                        name: "gmail_send".into(),
                        description: "Send a new email. IMPORTANT: Always confirm with the user before sending.".into(),
                        parameters: json!({
                            "type": "object",
                            "properties": {
                                "to": { "type": "string", "description": "Recipient email address" },
                                "subject": { "type": "string", "description": "Email subject" },
                                "body": { "type": "string", "description": "Email body text" }
                            },
                            "required": ["to", "subject", "body"]
                        }),
                    },
                },
                ToolDef {
                    tool_type: "function".into(),
                    function: FunctionDef {
                        name: "gmail_archive".into(),
                        description: "Archive emails by removing the INBOX label. Accepts one or more message IDs.".into(),
                        parameters: json!({
                            "type": "object",
                            "properties": {
                                "messageIds": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "description": "Array of message IDs to archive"
                                }
                            },
                            "required": ["messageIds"]
                        }),
                    },
                },
                ToolDef {
                    tool_type: "function".into(),
                    function: FunctionDef {
                        name: "gmail_trash".into(),
                        description: "Move emails to trash. They will be permanently deleted after 30 days.".into(),
                        parameters: json!({
                            "type": "object",
                            "properties": {
                                "messageIds": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "description": "Array of message IDs to trash"
                                }
                            },
                            "required": ["messageIds"]
                        }),
                    },
                },
                ToolDef {
                    tool_type: "function".into(),
                    function: FunctionDef {
                        name: "gmail_label".into(),
                        description: "Add or remove labels from emails. Use gmail_list_labels first to get valid label IDs.".into(),
                        parameters: json!({
                            "type": "object",
                            "properties": {
                                "messageIds": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "description": "Array of message IDs"
                                },
                                "addLabelIds": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "description": "Labels to add"
                                },
                                "removeLabelIds": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "description": "Labels to remove"
                                }
                            },
                            "required": ["messageIds"]
                        }),
                    },
                },
                ToolDef {
                    tool_type: "function".into(),
                    function: FunctionDef {
                        name: "gmail_list_labels".into(),
                        description: "List all Gmail labels (both system and custom). Useful to get label IDs.".into(),
                        parameters: json!({
                            "type": "object",
                            "properties": {}
                        }),
                    },
                },
                // --- Google Sheets ---
                ToolDef {
                    tool_type: "function".into(),
                    function: FunctionDef {
                        name: "sheets_read".into(),
                        description: "Read data from Google Sheets. Pass spreadsheet URL or ID, and optional range in A1 notation.".into(),
                        parameters: json!({
                            "type": "object",
                            "properties": {
                                "spreadsheetId": {
                                    "type": "string",
                                    "description": "Spreadsheet URL or ID"
                                },
                                "range": {
                                    "type": "string",
                                    "description": "Range in A1 notation (e.g. Sheet1!A1:E10). If omitted, reads entire first sheet."
                                }
                            },
                            "required": ["spreadsheetId"]
                        }),
                    },
                },
                ToolDef {
                    tool_type: "function".into(),
                    function: FunctionDef {
                        name: "sheets_write".into(),
                        description: "Write data to Google Sheets. Overwrites cells in the specified range.".into(),
                        parameters: json!({
                            "type": "object",
                            "properties": {
                                "spreadsheetId": { "type": "string", "description": "Spreadsheet URL or ID" },
                                "range": { "type": "string", "description": "Range in A1 notation" },
                                "values": {
                                    "type": "array",
                                    "items": {
                                        "type": "array",
                                        "items": { "type": "string" }
                                    },
                                    "description": "2D array of values (rows x cols)"
                                }
                            },
                            "required": ["spreadsheetId", "range", "values"]
                        }),
                    },
                },
                ToolDef {
                    tool_type: "function".into(),
                    function: FunctionDef {
                        name: "sheets_append".into(),
                        description: "Append rows to the end of a Google Sheet.".into(),
                        parameters: json!({
                            "type": "object",
                            "properties": {
                                "spreadsheetId": { "type": "string", "description": "Spreadsheet URL or ID" },
                                "range": { "type": "string", "description": "Range in A1 notation" },
                                "values": {
                                    "type": "array",
                                    "items": {
                                        "type": "array",
                                        "items": { "type": "string" }
                                    },
                                    "description": "2D array of values to append"
                                }
                            },
                            "required": ["spreadsheetId", "range", "values"]
                        }),
                    },
                },
                ToolDef {
                    tool_type: "function".into(),
                    function: FunctionDef {
                        name: "sheets_list".into(),
                        description: "List all sheets (tabs) in a spreadsheet. Returns sheet names, IDs, row/col counts.".into(),
                        parameters: json!({
                            "type": "object",
                            "properties": {
                                "spreadsheetId": { "type": "string", "description": "Spreadsheet URL or ID" }
                            },
                            "required": ["spreadsheetId"]
                        }),
                    },
                },
                ToolDef {
                    tool_type: "function".into(),
                    function: FunctionDef {
                        name: "sheets_create_tab".into(),
                        description: "Create a new sheet tab in a spreadsheet.".into(),
                        parameters: json!({
                            "type": "object",
                            "properties": {
                                "spreadsheetId": { "type": "string", "description": "Spreadsheet URL or ID" },
                                "title": { "type": "string", "description": "Name for the new tab" }
                            },
                            "required": ["spreadsheetId", "title"]
                        }),
                    },
                },
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
                let add: Vec<String> = parse_string_array(&args["addLabelIds"]);
                let remove: Vec<String> = parse_string_array(&args["removeLabelIds"]);
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
