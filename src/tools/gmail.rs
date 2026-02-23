use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

const GMAIL_API: &str = "https://gmail.googleapis.com/gmail/v1/users/me";

/// Get a fresh access token using refresh token
async fn get_access_token(client_id: &str, client_secret: &str, refresh_token: &str) -> Result<String, String> {
    let client = Client::new();
    let resp = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|e| format!("Token refresh error: {e}"))?;

    let body: serde_json::Value = resp.json().await.map_err(|e| format!("Token parse error: {e}"))?;
    body["access_token"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("No access_token in response: {body}"))
}

fn gmail_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_else(|_| Client::new())
}

#[derive(Debug, Deserialize)]
struct GmailMessage {
    id: String,
    payload: Option<Payload>,
    snippet: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Payload {
    headers: Option<Vec<Header>>,
    body: Option<Body>,
    parts: Option<Vec<Part>>,
}

#[derive(Debug, Deserialize)]
struct Header {
    name: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct Body {
    data: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Part {
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
    body: Option<Body>,
    parts: Option<Vec<Part>>,
}

fn get_header(headers: &[Header], name: &str) -> String {
    headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(name))
        .map(|h| h.value.clone())
        .unwrap_or_default()
}

fn decode_base64url(data: &str) -> String {
    // base64url → standard base64
    let b64 = data.replace('-', "+").replace('_', "/");
    // Pad if needed
    let padded = match b64.len() % 4 {
        2 => format!("{b64}=="),
        3 => format!("{b64}="),
        _ => b64,
    };
    let bytes = match base64_decode(&padded) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };
    String::from_utf8_lossy(&bytes).to_string()
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    // Simple base64 decoder
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = Vec::new();
    let mut buf: u32 = 0;
    let mut bits = 0;

    for &byte in input.as_bytes() {
        if byte == b'=' || byte == b'\n' || byte == b'\r' {
            continue;
        }
        let val = TABLE.iter().position(|&b| b == byte).ok_or("invalid base64")? as u32;
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(output)
}

fn extract_body_text(payload: &Payload) -> String {
    // Try plain text first
    if let Some(parts) = &payload.parts {
        for part in parts {
            if part.mime_type.as_deref() == Some("text/plain") {
                if let Some(body) = &part.body {
                    if let Some(data) = &body.data {
                        return decode_base64url(data);
                    }
                }
            }
            // Nested parts
            if let Some(sub_parts) = &part.parts {
                for sp in sub_parts {
                    if sp.mime_type.as_deref() == Some("text/plain") {
                        if let Some(body) = &sp.body {
                            if let Some(data) = &body.data {
                                return decode_base64url(data);
                            }
                        }
                    }
                }
            }
        }
    }
    // Fallback: direct body
    if let Some(body) = &payload.body {
        if let Some(data) = &body.data {
            return decode_base64url(data);
        }
    }
    String::new()
}

pub async fn gmail_search(query: &str, max_results: u32, creds: &GmailCreds) -> String {
    let token = match get_access_token(&creds.client_id, &creds.client_secret, &creds.refresh_token).await {
        Ok(t) => t,
        Err(e) => return e,
    };

    let client = gmail_client();
    let url = format!("{GMAIL_API}/messages?q={}&maxResults={max_results}", urlencoding::encode(query));

    let resp = match client.get(&url).bearer_auth(&token).send().await {
        Ok(r) => r,
        Err(e) => return format!("Gmail API error: {e}"),
    };

    let body: serde_json::Value = match resp.json().await {
        Ok(b) => b,
        Err(e) => return format!("Parse error: {e}"),
    };

    let messages = body["messages"].as_array();
    if messages.is_none() || messages.unwrap().is_empty() {
        return "No emails found.".into();
    }

    let msg_ids: Vec<&str> = messages
        .unwrap()
        .iter()
        .filter_map(|m| m["id"].as_str())
        .collect();

    // Fetch metadata for each message
    let mut results = Vec::new();
    for id in msg_ids.iter().take(10) {
        let detail_url = format!("{GMAIL_API}/messages/{id}?format=metadata&metadataHeaders=Subject&metadataHeaders=From&metadataHeaders=Date");
        if let Ok(resp) = client.get(&detail_url).bearer_auth(&token).send().await {
            if let Ok(detail) = resp.json::<serde_json::Value>().await {
                let headers = detail["payload"]["headers"].as_array();
                let (mut subject, mut from, mut date) = (String::new(), String::new(), String::new());
                if let Some(hdrs) = headers {
                    for h in hdrs {
                        match h["name"].as_str().unwrap_or("") {
                            "Subject" => subject = h["value"].as_str().unwrap_or("").to_string(),
                            "From" => from = h["value"].as_str().unwrap_or("").to_string(),
                            "Date" => date = h["value"].as_str().unwrap_or("").to_string(),
                            _ => {}
                        }
                    }
                }
                let snippet = detail["snippet"].as_str().unwrap_or("");
                results.push(format!("ID: {id}\nFrom: {from}\nDate: {date}\nSubject: {subject}\nSnippet: {snippet}"));
            }
        }
    }

    if results.is_empty() {
        "No emails found.".into()
    } else {
        results.join("\n---\n")
    }
}

pub async fn gmail_read(message_id: &str, creds: &GmailCreds) -> String {
    let token = match get_access_token(&creds.client_id, &creds.client_secret, &creds.refresh_token).await {
        Ok(t) => t,
        Err(e) => return e,
    };

    let client = gmail_client();
    let url = format!("{GMAIL_API}/messages/{message_id}?format=full");

    let resp = match client.get(&url).bearer_auth(&token).send().await {
        Ok(r) => r,
        Err(e) => return format!("Gmail API error: {e}"),
    };

    let msg: GmailMessage = match resp.json().await {
        Ok(m) => m,
        Err(e) => return format!("Parse error: {e}"),
    };

    let payload = match msg.payload {
        Some(p) => p,
        None => return format!("ID: {}\nSnippet: {}", msg.id, msg.snippet.unwrap_or_default()),
    };

    let headers = payload.headers.as_deref().unwrap_or(&[]);
    let subject = get_header(headers, "Subject");
    let from = get_header(headers, "From");
    let to = get_header(headers, "To");
    let date = get_header(headers, "Date");
    let body = extract_body_text(&payload);

    let body_preview = if body.len() > 4000 {
        format!("{}\n\n[... truncated, {} chars total]", &body[..4000], body.len())
    } else {
        body
    };

    format!("Subject: {subject}\nFrom: {from}\nTo: {to}\nDate: {date}\n\n{body_preview}")
}

pub async fn gmail_send(to: &str, subject: &str, body: &str, creds: &GmailCreds) -> String {
    let token = match get_access_token(&creds.client_id, &creds.client_secret, &creds.refresh_token).await {
        Ok(t) => t,
        Err(e) => return e,
    };

    // Build RFC 2822 message
    let raw = format!("To: {to}\r\nSubject: {subject}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}");
    let encoded = base64url_encode(raw.as_bytes());

    let client = gmail_client();
    let url = format!("{GMAIL_API}/messages/send");

    match client
        .post(&url)
        .bearer_auth(&token)
        .json(&json!({ "raw": encoded }))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => format!("Email sent to {to}"),
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            format!("Send failed ({status}): {text}")
        }
        Err(e) => format!("Send error: {e}"),
    }
}

pub async fn gmail_archive(message_ids: &[String], creds: &GmailCreds) -> String {
    modify_labels(message_ids, &[], &["INBOX"], creds).await
}

pub async fn gmail_trash(message_ids: &[String], creds: &GmailCreds) -> String {
    let token = match get_access_token(&creds.client_id, &creds.client_secret, &creds.refresh_token).await {
        Ok(t) => t,
        Err(e) => return e,
    };

    let client = gmail_client();
    let mut results = Vec::new();
    for id in message_ids {
        let url = format!("{GMAIL_API}/messages/{id}/trash");
        match client.post(&url).bearer_auth(&token).send().await {
            Ok(r) if r.status().is_success() => results.push(format!("Trashed: {id}")),
            Ok(r) => results.push(format!("Failed {id}: {}", r.status())),
            Err(e) => results.push(format!("Error {id}: {e}")),
        }
    }
    results.join("\n")
}

pub async fn gmail_label(message_ids: &[String], add: &[&str], remove: &[&str], creds: &GmailCreds) -> String {
    modify_labels(message_ids, add, remove, creds).await
}

async fn modify_labels(message_ids: &[String], add: &[&str], remove: &[&str], creds: &GmailCreds) -> String {
    let token = match get_access_token(&creds.client_id, &creds.client_secret, &creds.refresh_token).await {
        Ok(t) => t,
        Err(e) => return e,
    };

    let client = gmail_client();
    let mut results = Vec::new();
    for id in message_ids {
        let url = format!("{GMAIL_API}/messages/{id}/modify");
        let body = json!({
            "addLabelIds": add,
            "removeLabelIds": remove,
        });
        match client.post(&url).bearer_auth(&token).json(&body).send().await {
            Ok(r) if r.status().is_success() => results.push(format!("Modified: {id}")),
            Ok(r) => results.push(format!("Failed {id}: {}", r.status())),
            Err(e) => results.push(format!("Error {id}: {e}")),
        }
    }
    results.join("\n")
}

pub async fn gmail_list_labels(creds: &GmailCreds) -> String {
    let token = match get_access_token(&creds.client_id, &creds.client_secret, &creds.refresh_token).await {
        Ok(t) => t,
        Err(e) => return e,
    };

    let client = gmail_client();
    let url = format!("{GMAIL_API}/labels");

    match client.get(&url).bearer_auth(&token).send().await {
        Ok(resp) => {
            let body: serde_json::Value = match resp.json().await {
                Ok(b) => b,
                Err(e) => return format!("Parse error: {e}"),
            };
            let labels = body["labels"].as_array();
            match labels {
                Some(arr) => arr
                    .iter()
                    .filter_map(|l| {
                        let id = l["id"].as_str()?;
                        let name = l["name"].as_str()?;
                        Some(format!("{id}: {name}"))
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                None => "No labels found.".into(),
            }
        }
        Err(e) => format!("Error: {e}"),
    }
}

fn base64url_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut output = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        output.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        output.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            output.push(TABLE[(triple & 0x3F) as usize] as char);
        }
    }
    output
}

/// Gmail credentials container
#[derive(Debug, Clone)]
pub struct GmailCreds {
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: String,
}

impl GmailCreds {
    pub fn is_configured(&self) -> bool {
        !self.client_id.is_empty() && !self.client_secret.is_empty() && !self.refresh_token.is_empty()
    }
}
