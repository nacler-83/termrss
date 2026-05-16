use anyhow::{Context, Result};
use feed_rs::parser;
use reqwest::Client;
use reqwest::header::{ETAG, HeaderMap, HeaderValue, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};

#[derive(Debug, Clone)]
pub struct FetchResult {
    pub not_modified: bool,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub title: Option<String>,
    pub entries: Vec<ParsedEntry>,
}

#[derive(Debug, Clone)]
pub struct ParsedEntry {
    pub guid: String,
    pub title: String,
    pub url: Option<String>,
    pub author: Option<String>,
    pub published_at: Option<i64>,
    pub summary_html: Option<String>,
    pub content_html: Option<String>,
}

pub fn build_client() -> Result<Client> {
    Client::builder()
        .user_agent(concat!("termrss/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .context("building HTTP client")
}

pub async fn fetch(
    client: &Client,
    url: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> Result<FetchResult> {
    let mut headers = HeaderMap::new();
    if let Some(e) = etag {
        if let Ok(v) = HeaderValue::from_str(e) {
            headers.insert(IF_NONE_MATCH, v);
        }
    }
    if let Some(lm) = last_modified {
        if let Ok(v) = HeaderValue::from_str(lm) {
            headers.insert(IF_MODIFIED_SINCE, v);
        }
    }

    let resp = client
        .get(url)
        .headers(headers)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;

    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(FetchResult {
            not_modified: true,
            etag: etag.map(|s| s.to_string()),
            last_modified: last_modified.map(|s| s.to_string()),
            title: None,
            entries: vec![],
        });
    }

    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("HTTP {status} fetching {url}");
    }

    let new_etag = resp
        .headers()
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let new_last_modified = resp
        .headers()
        .get(LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let bytes = resp.bytes().await.context("reading response body")?;
    let feed = parser::parse(bytes.as_ref()).context("parsing feed")?;

    let title = feed.title.as_ref().map(|t| t.content.clone());
    let entries = feed
        .entries
        .into_iter()
        .map(|e| {
            let url = e.links.first().map(|l| l.href.clone());
            let summary_html = e.summary.as_ref().map(|s| s.content.clone());
            let content_html = e
                .content
                .as_ref()
                .and_then(|c| c.body.clone())
                .or_else(|| summary_html.clone());
            let title = e
                .title
                .as_ref()
                .map(|t| t.content.clone())
                .unwrap_or_else(|| "(untitled)".to_string());
            let author = e.authors.first().map(|a| a.name.clone());
            let published_at = e.published.or(e.updated).map(|d| d.timestamp());
            let guid = if !e.id.is_empty() {
                e.id.clone()
            } else if let Some(u) = &url {
                u.clone()
            } else {
                format!("{}|{}", title, published_at.unwrap_or(0))
            };
            ParsedEntry {
                guid,
                title,
                url,
                author,
                published_at,
                summary_html,
                content_html,
            }
        })
        .collect();

    Ok(FetchResult {
        not_modified: false,
        etag: new_etag,
        last_modified: new_last_modified,
        title,
        entries,
    })
}
