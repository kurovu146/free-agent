use reqwest::Client;

/// Simple web search using DuckDuckGo lite (no API key needed)
pub async fn web_search(query: &str) -> String {
    if query.is_empty() {
        return "Error: empty query".into();
    }

    let client = Client::new();

    // Use DuckDuckGo HTML API (no key required)
    let url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        urlencoding::encode(query)
    );

    match client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (compatible; FreeAgent/1.0)")
        .send()
        .await
    {
        Ok(resp) => match resp.text().await {
            Ok(html) => parse_ddg_html(&html),
            Err(e) => format!("Error reading response: {e}"),
        },
        Err(e) => format!("Search error: {e}"),
    }
}

/// Parse DuckDuckGo HTML results into text
fn parse_ddg_html(html: &str) -> String {
    let mut results = Vec::new();
    let mut count = 0;

    // Simple HTML parsing — extract result blocks
    for part in html.split("class=\"result__a\"") {
        if count == 0 {
            count += 1;
            continue; // Skip first split part
        }
        if count > 5 {
            break;
        }

        // Extract href
        let href = part
            .split("href=\"")
            .nth(0)
            .and_then(|s| s.split('"').nth(0))
            .unwrap_or("");

        // Extract title text (between > and </a>)
        let title = part
            .split('>')
            .nth(0)
            .and_then(|rest| rest.split("</a>").nth(0))
            .map(|s| strip_html_tags(s))
            .unwrap_or_default();

        // Extract snippet
        let snippet = if let Some(snip_start) = part.find("class=\"result__snippet\"") {
            let after = &part[snip_start..];
            after
                .split('>')
                .nth(1)
                .and_then(|s| s.split("</").nth(0))
                .map(|s| strip_html_tags(s))
                .unwrap_or_default()
        } else {
            String::new()
        };

        if !title.is_empty() || !snippet.is_empty() {
            results.push(format!(
                "{}. {}\n   {}\n   {}",
                count,
                if title.is_empty() { "(no title)" } else { &title },
                snippet,
                href,
            ));
        }
        count += 1;
    }

    if results.is_empty() {
        "No results found.".into()
    } else {
        results.join("\n\n")
    }
}

fn strip_html_tags(s: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .trim()
        .to_string()
}
