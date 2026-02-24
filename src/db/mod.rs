use rusqlite::{Connection, params};
use std::sync::Mutex;
use tracing::info;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;

        // Memory facts table
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memory_facts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                fact TEXT NOT NULL,
                category TEXT NOT NULL DEFAULT 'general',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                access_count INTEGER NOT NULL DEFAULT 0,
                last_accessed_at TEXT
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS memory_facts_fts USING fts5(
                fact,
                content='memory_facts',
                content_rowid='id'
            );

            CREATE TRIGGER IF NOT EXISTS memory_facts_ai AFTER INSERT ON memory_facts BEGIN
                INSERT INTO memory_facts_fts(rowid, fact) VALUES (new.id, new.fact);
            END;

            CREATE TRIGGER IF NOT EXISTS memory_facts_ad AFTER DELETE ON memory_facts BEGIN
                INSERT INTO memory_facts_fts(memory_facts_fts, rowid, fact) VALUES('delete', old.id, old.fact);
            END;
            "
        )?;

        // Conversation sessions
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                user_id INTEGER NOT NULL,
                title TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_active_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS session_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL REFERENCES sessions(id),
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS query_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                provider TEXT,
                prompt_preview TEXT,
                response_time_ms INTEGER,
                tokens_in INTEGER DEFAULT 0,
                tokens_out INTEGER DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            "
        )?;

        info!("Database initialized: {path}");
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    // --- Memory ---

    pub fn save_fact(&self, user_id: u64, fact: &str, category: &str) -> Result<i64, String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO memory_facts (user_id, fact, category) VALUES (?1, ?2, ?3)",
            params![user_id as i64, fact, category],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    pub fn search_facts(&self, user_id: u64, keyword: &str) -> Result<Vec<(i64, String, String)>, String> {
        let conn = self.conn.lock().unwrap();

        // Try FTS5 first, fall back to LIKE
        let results: Vec<(i64, String, String)> = conn
            .prepare(
                "SELECT mf.id, mf.fact, mf.category FROM memory_facts mf
                 JOIN memory_facts_fts fts ON mf.id = fts.rowid
                 WHERE fts.fact MATCH ?1 AND mf.user_id = ?2
                 ORDER BY rank LIMIT 20"
            )
            .and_then(|mut stmt| {
                let rows = stmt.query_map(params![keyword, user_id as i64], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?;
                rows.collect()
            })
            .unwrap_or_else(|_| {
                // Fallback to LIKE
                conn.prepare(
                    "SELECT id, fact, category FROM memory_facts
                     WHERE user_id = ?1 AND fact LIKE '%' || ?2 || '%'
                     ORDER BY created_at DESC LIMIT 20"
                )
                .and_then(|mut stmt| {
                    let rows = stmt.query_map(params![user_id as i64, keyword], |row| {
                        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                    })?;
                    rows.collect()
                })
                .unwrap_or_default()
            });

        // Update access count
        for (id, _, _) in &results {
            let _ = conn.execute(
                "UPDATE memory_facts SET access_count = access_count + 1, last_accessed_at = datetime('now') WHERE id = ?1",
                params![id],
            );
        }

        Ok(results)
    }

    pub fn list_facts(&self, user_id: u64, category: Option<&str>) -> Result<Vec<(i64, String, String)>, String> {
        let conn = self.conn.lock().unwrap();
        let (sql, p): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = match category {
            Some(cat) => (
                "SELECT id, fact, category FROM memory_facts WHERE user_id = ?1 AND category = ?2 ORDER BY created_at DESC LIMIT 30",
                vec![Box::new(user_id as i64), Box::new(cat.to_string())],
            ),
            None => (
                "SELECT id, fact, category FROM memory_facts WHERE user_id = ?1 ORDER BY created_at DESC LIMIT 30",
                vec![Box::new(user_id as i64)],
            ),
        };

        let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|b| b.as_ref()).collect();
        let rows = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    // --- Memory context for system prompt ---

    pub fn build_memory_context(&self, user_id: u64) -> String {
        let facts = self.list_facts(user_id, None).unwrap_or_default();
        if facts.is_empty() {
            return String::new();
        }

        let mut grouped: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for (_, fact, category) in &facts {
            grouped
                .entry(category.clone())
                .or_default()
                .push(fact.clone());
        }

        let mut ctx = String::from("\n--- MEMORY ---\n");
        for (cat, items) in &grouped {
            ctx.push_str(&format!("\n[{cat}]\n"));
            for item in items {
                ctx.push_str(&format!("- {item}\n"));
            }
        }
        ctx.push_str("\n--- END MEMORY ---\n");
        ctx
    }

    pub fn delete_fact(&self, user_id: u64, fact_id: i64) -> Result<bool, String> {
        let conn = self.conn.lock().unwrap();
        let affected = conn
            .execute(
                "DELETE FROM memory_facts WHERE id = ?1 AND user_id = ?2",
                params![fact_id, user_id as i64],
            )
            .map_err(|e| e.to_string())?;
        Ok(affected > 0)
    }

    // --- Conversation history ---

    /// Get or create the active session for a user. Returns session_id.
    pub fn get_or_create_session(&self, user_id: u64) -> String {
        let conn = self.conn.lock().unwrap();
        // Try to find existing session (most recent)
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM sessions WHERE user_id = ?1 ORDER BY last_active_at DESC LIMIT 1",
                params![user_id as i64],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing {
            // Update last_active_at
            let _ = conn.execute(
                "UPDATE sessions SET last_active_at = datetime('now') WHERE id = ?1",
                params![&id],
            );
            return id;
        }

        // Create new session
        let id = format!("{}-{}", user_id, chrono::Utc::now().timestamp());
        let _ = conn.execute(
            "INSERT INTO sessions (id, user_id) VALUES (?1, ?2)",
            params![&id, user_id as i64],
        );
        id
    }

    /// Load recent conversation history for a session (last N user+assistant message pairs).
    pub fn load_history(&self, session_id: &str, max_pairs: usize) -> Vec<(String, String)> {
        let conn = self.conn.lock().unwrap();
        let limit = (max_pairs * 2) as i64;
        // Get last N messages ordered by id
        let mut stmt = match conn.prepare(
            "SELECT role, content FROM session_messages WHERE session_id = ?1 ORDER BY id DESC LIMIT ?2"
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows: Vec<(String, String)> = stmt
            .query_map(params![session_id, limit], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .map(|r| r.collect::<Result<Vec<_>, _>>().unwrap_or_default())
            .unwrap_or_default();

        // Reverse to chronological order
        let mut result = rows;
        result.reverse();
        result
    }

    /// Append a message to the session history.
    pub fn append_message(&self, session_id: &str, role: &str, content: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT INTO session_messages (session_id, role, content) VALUES (?1, ?2, ?3)",
            params![session_id, role, content],
        );
    }

    // --- Query logs ---

    pub fn log_query(
        &self,
        user_id: u64,
        provider: &str,
        prompt_preview: &str,
        response_time_ms: u64,
        tokens_in: u32,
        tokens_out: u32,
    ) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT INTO query_logs (user_id, provider, prompt_preview, response_time_ms, tokens_in, tokens_out)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                user_id as i64,
                provider,
                &prompt_preview[..prompt_preview.len().min(100)],
                response_time_ms as i64,
                tokens_in as i64,
                tokens_out as i64
            ],
        );
    }
}
