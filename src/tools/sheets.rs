use reqwest::Client;
use serde_json::json;

use super::gmail::GmailCreds; // Reuse same OAuth creds

const SHEETS_API: &str = "https://sheets.googleapis.com/v4/spreadsheets";

/// Get a fresh access token (reuses gmail's OAuth)
async fn get_access_token(creds: &GmailCreds) -> Result<String, String> {
    let client = Client::new();
    let resp = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", creds.client_id.as_str()),
            ("client_secret", creds.client_secret.as_str()),
            ("refresh_token", creds.refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|e| format!("Token error: {e}"))?;

    let body: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;
    body["access_token"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("No access_token: {body}"))
}

/// Extract spreadsheet ID from URL or return as-is
fn extract_spreadsheet_id(input: &str) -> &str {
    if input.contains("/spreadsheets/d/") {
        input
            .split("/spreadsheets/d/")
            .nth(1)
            .and_then(|s| s.split('/').next())
            .unwrap_or(input)
    } else {
        input
    }
}

fn sheets_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_else(|_| Client::new())
}

pub async fn sheets_read(spreadsheet_id: &str, range: Option<&str>, creds: &GmailCreds) -> String {
    let token = match get_access_token(creds).await {
        Ok(t) => t,
        Err(e) => return e,
    };

    let sid = extract_spreadsheet_id(spreadsheet_id);
    let client = sheets_client();

    let url = match range {
        Some(r) => format!("{SHEETS_API}/{sid}/values/{}", urlencoding::encode(r)),
        None => format!("{SHEETS_API}/{sid}/values/Sheet1"),
    };

    match client.get(&url).bearer_auth(&token).send().await {
        Ok(resp) => {
            let body: serde_json::Value = match resp.json().await {
                Ok(b) => b,
                Err(e) => return format!("Parse error: {e}"),
            };

            if let Some(err) = body["error"]["message"].as_str() {
                return format!("Error: {err}");
            }

            let values = body["values"].as_array();
            match values {
                Some(rows) => {
                    let formatted: Vec<String> = rows
                        .iter()
                        .enumerate()
                        .map(|(i, row)| {
                            let cells: Vec<&str> = row
                                .as_array()
                                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                                .unwrap_or_default();
                            format!("Row {}: {}", i + 1, cells.join(" | "))
                        })
                        .collect();
                    if formatted.is_empty() {
                        "Sheet is empty.".into()
                    } else {
                        formatted.join("\n")
                    }
                }
                None => "No data found.".into(),
            }
        }
        Err(e) => format!("Error: {e}"),
    }
}

pub async fn sheets_write(
    spreadsheet_id: &str,
    range: &str,
    values: Vec<Vec<String>>,
    creds: &GmailCreds,
) -> String {
    let token = match get_access_token(creds).await {
        Ok(t) => t,
        Err(e) => return e,
    };

    let sid = extract_spreadsheet_id(spreadsheet_id);
    let client = sheets_client();
    let url = format!(
        "{SHEETS_API}/{sid}/values/{}?valueInputOption=USER_ENTERED",
        urlencoding::encode(range)
    );

    let body = json!({
        "range": range,
        "values": values,
    });

    match client.put(&url).bearer_auth(&token).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            let result: serde_json::Value = resp.json().await.unwrap_or_default();
            let updated = result["updatedCells"].as_u64().unwrap_or(0);
            format!("Updated {updated} cells in {range}")
        }
        Ok(resp) => {
            let text = resp.text().await.unwrap_or_default();
            format!("Write failed: {text}")
        }
        Err(e) => format!("Error: {e}"),
    }
}

pub async fn sheets_append(
    spreadsheet_id: &str,
    range: &str,
    values: Vec<Vec<String>>,
    creds: &GmailCreds,
) -> String {
    let token = match get_access_token(creds).await {
        Ok(t) => t,
        Err(e) => return e,
    };

    let sid = extract_spreadsheet_id(spreadsheet_id);
    let client = sheets_client();
    let url = format!(
        "{SHEETS_API}/{sid}/values/{}:append?valueInputOption=USER_ENTERED&insertDataOption=INSERT_ROWS",
        urlencoding::encode(range)
    );

    let body = json!({
        "values": values,
    });

    match client.post(&url).bearer_auth(&token).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            let result: serde_json::Value = resp.json().await.unwrap_or_default();
            let updated = result["updates"]["updatedRows"].as_u64().unwrap_or(0);
            format!("Appended {updated} rows")
        }
        Ok(resp) => {
            let text = resp.text().await.unwrap_or_default();
            format!("Append failed: {text}")
        }
        Err(e) => format!("Error: {e}"),
    }
}

pub async fn sheets_list(spreadsheet_id: &str, creds: &GmailCreds) -> String {
    let token = match get_access_token(creds).await {
        Ok(t) => t,
        Err(e) => return e,
    };

    let sid = extract_spreadsheet_id(spreadsheet_id);
    let client = sheets_client();
    let url = format!("{SHEETS_API}/{sid}?fields=sheets.properties");

    match client.get(&url).bearer_auth(&token).send().await {
        Ok(resp) => {
            let body: serde_json::Value = match resp.json().await {
                Ok(b) => b,
                Err(e) => return format!("Parse error: {e}"),
            };

            let sheets = body["sheets"].as_array();
            match sheets {
                Some(arr) => arr
                    .iter()
                    .filter_map(|s| {
                        let props = &s["properties"];
                        let title = props["title"].as_str()?;
                        let id = props["sheetId"].as_u64()?;
                        let rows = props["gridProperties"]["rowCount"].as_u64().unwrap_or(0);
                        let cols = props["gridProperties"]["columnCount"].as_u64().unwrap_or(0);
                        Some(format!("ID: {id} | {title} ({rows} rows x {cols} cols)"))
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                None => "No sheets found.".into(),
            }
        }
        Err(e) => format!("Error: {e}"),
    }
}

pub async fn sheets_create_tab(spreadsheet_id: &str, title: &str, creds: &GmailCreds) -> String {
    let token = match get_access_token(creds).await {
        Ok(t) => t,
        Err(e) => return e,
    };

    let sid = extract_spreadsheet_id(spreadsheet_id);
    let client = sheets_client();
    let url = format!("{SHEETS_API}/{sid}:batchUpdate");

    let body = json!({
        "requests": [{
            "addSheet": {
                "properties": {
                    "title": title
                }
            }
        }]
    });

    match client.post(&url).bearer_auth(&token).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => format!("Created sheet tab: {title}"),
        Ok(resp) => {
            let text = resp.text().await.unwrap_or_default();
            format!("Create failed: {text}")
        }
        Err(e) => format!("Error: {e}"),
    }
}
