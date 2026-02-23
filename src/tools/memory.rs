use crate::db::Database;

pub async fn memory_save(db: &Database, user_id: u64, fact: &str, category: &str) -> String {
    if fact.is_empty() {
        return "Error: fact cannot be empty".into();
    }
    match db.save_fact(user_id, fact, category) {
        Ok(id) => format!("Saved (ID: {id}): \"{fact}\" [{category}]"),
        Err(e) => format!("Error saving: {e}"),
    }
}

pub async fn memory_search(db: &Database, user_id: u64, keyword: &str) -> String {
    if keyword.is_empty() {
        return "Error: keyword cannot be empty".into();
    }
    match db.search_facts(user_id, keyword) {
        Ok(results) if results.is_empty() => "No facts found.".into(),
        Ok(results) => {
            let lines: Vec<String> = results
                .iter()
                .map(|(id, fact, cat)| format!("[{id}] [{cat}] {fact}"))
                .collect();
            lines.join("\n")
        }
        Err(e) => format!("Error searching: {e}"),
    }
}

pub async fn memory_delete(db: &Database, user_id: u64, fact_id: i64) -> String {
    match db.delete_fact(user_id, fact_id) {
        Ok(true) => format!("Deleted memory ID: {fact_id}"),
        Ok(false) => format!("Memory ID {fact_id} not found or not yours"),
        Err(e) => format!("Error deleting: {e}"),
    }
}

pub async fn memory_list(db: &Database, user_id: u64, category: Option<&str>) -> String {
    match db.list_facts(user_id, category) {
        Ok(results) if results.is_empty() => "No facts saved yet.".into(),
        Ok(results) => {
            let lines: Vec<String> = results
                .iter()
                .map(|(id, fact, cat)| format!("[{id}] [{cat}] {fact}"))
                .collect();
            lines.join("\n")
        }
        Err(e) => format!("Error listing: {e}"),
    }
}
