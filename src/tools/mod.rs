mod web;
mod memory;
pub mod gmail;
mod sheets;
mod datetime;

pub use web::{web_search, web_fetch};
pub use memory::{memory_save, memory_search, memory_list, memory_delete};
pub use gmail::{gmail_search, gmail_read, gmail_send, gmail_archive, gmail_trash, gmail_label, gmail_list_labels};
pub use sheets::{sheets_read, sheets_write, sheets_append, sheets_list, sheets_create_tab};
pub use datetime::get_datetime;
