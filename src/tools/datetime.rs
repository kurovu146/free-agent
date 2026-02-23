use chrono::Utc;

/// Get current date and time in UTC and common timezones
pub async fn get_datetime() -> String {
    let now = Utc::now();
    format!(
        "Current time:\n- UTC: {}\n- Vietnam (GMT+7): {}\n- US Eastern (GMT-5): {}",
        now.format("%Y-%m-%d %H:%M:%S %Z"),
        (now + chrono::Duration::hours(7)).format("%Y-%m-%d %H:%M:%S GMT+7"),
        (now - chrono::Duration::hours(5)).format("%Y-%m-%d %H:%M:%S GMT-5"),
    )
}
