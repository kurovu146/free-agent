use crate::db::Database;

// --- Plan tools ---

pub async fn plan_read(db: &Database, user_id: u64) -> String {
    let content = db.get_plan(user_id);
    if content.is_empty() {
        "No plan set. Use plan_write to create one.".into()
    } else {
        content
    }
}

pub async fn plan_write(db: &Database, user_id: u64, content: &str) -> String {
    match db.set_plan(user_id, content) {
        Ok(()) => "Plan updated successfully.".into(),
        Err(e) => format!("Error saving plan: {e}"),
    }
}

// --- Todo tools ---

pub async fn todo_add(db: &Database, user_id: u64, content: &str) -> String {
    match db.add_todo(user_id, content) {
        Ok(id) => format!("Todo #{id} added: {content}"),
        Err(e) => format!("Error adding todo: {e}"),
    }
}

pub async fn todo_list(db: &Database, user_id: u64) -> String {
    match db.list_todos(user_id) {
        Ok(todos) if todos.is_empty() => "No todos. Use todo_add to create one.".into(),
        Ok(todos) => {
            let lines: Vec<String> = todos
                .iter()
                .map(|(id, content, status)| {
                    let icon = match status.as_str() {
                        "completed" => "[x]",
                        "in_progress" => "[~]",
                        _ => "[ ]",
                    };
                    format!("#{id} {icon} {content}")
                })
                .collect();
            lines.join("\n")
        }
        Err(e) => format!("Error listing todos: {e}"),
    }
}

pub async fn todo_update(db: &Database, user_id: u64, todo_id: i64, status: &str) -> String {
    let valid = ["pending", "in_progress", "completed"];
    if !valid.contains(&status) {
        return format!("Invalid status '{status}'. Use: pending, in_progress, completed");
    }
    match db.update_todo_status(user_id, todo_id, status) {
        Ok(true) => format!("Todo #{todo_id} updated to {status}"),
        Ok(false) => format!("Todo #{todo_id} not found"),
        Err(e) => format!("Error updating todo: {e}"),
    }
}

pub async fn todo_delete(db: &Database, user_id: u64, todo_id: i64) -> String {
    match db.delete_todo(user_id, todo_id) {
        Ok(true) => format!("Todo #{todo_id} deleted"),
        Ok(false) => format!("Todo #{todo_id} not found"),
        Err(e) => format!("Error deleting todo: {e}"),
    }
}

pub async fn todo_clear_completed(db: &Database, user_id: u64) -> String {
    match db.clear_completed_todos(user_id) {
        Ok(count) => format!("Cleared {count} completed todos"),
        Err(e) => format!("Error clearing todos: {e}"),
    }
}
