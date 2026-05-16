use anyhow::{Context, Result};

pub fn html_to_text(html: &str, width: usize) -> String {
    let w = width.max(20);
    html2text::from_read(html.as_bytes(), w).unwrap_or_else(|_| html.to_string())
}

pub async fn fetch_full_article(client: &reqwest::Client, url: &str) -> Result<String> {
    let parsed_url = url::Url::parse(url).context("parsing article URL")?;
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {} fetching {url}", resp.status());
    }
    let body = resp.text().await.context("reading article body")?;
    let mut cursor = std::io::Cursor::new(body.into_bytes());
    let product = readability::extractor::extract(&mut cursor, &parsed_url)
        .context("running readability extractor")?;
    Ok(product.content)
}
