mod web;
mod memory;
pub mod gmail;
mod sheets;
mod datetime;
mod system;
mod planning;

pub use web::{web_search, web_fetch};
pub use memory::{memory_save, memory_search, memory_list, memory_delete};
pub use gmail::{gmail_search, gmail_read, gmail_send, gmail_archive, gmail_trash, gmail_label, gmail_list_labels};
pub use sheets::{sheets_read, sheets_write, sheets_append, sheets_list, sheets_create_tab};
pub use datetime::get_datetime;
pub use system::{bash_exec, file_read, file_write, glob_search, grep_search};
pub use planning::{plan_read, plan_write, todo_add, todo_list, todo_update, todo_delete, todo_clear_completed};
