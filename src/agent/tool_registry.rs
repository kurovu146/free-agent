use serde_json::json;

use crate::provider::{ToolDef, FunctionDef};
use crate::tools;

/// Registry of all available tools with definitions and executor
pub struct ToolRegistry;

impl ToolRegistry {
    /// Get tool definitions to send to LLM
    pub fn definitions() -> Vec<ToolDef> {
        vec![
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
        ]
    }

    /// Execute a tool by name with given arguments
    pub async fn execute(
        tool_name: &str,
        args_json: &str,
        user_id: u64,
        db: &crate::db::Database,
    ) -> String {
        let args: serde_json::Value = serde_json::from_str(args_json).unwrap_or_default();

        match tool_name {
            "web_search" => {
                let query = args["query"].as_str().unwrap_or("");
                tools::web_search(query).await
            }
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
            _ => format!("Unknown tool: {tool_name}"),
        }
    }
}
