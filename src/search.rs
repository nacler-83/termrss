use anyhow::Result;
use rusqlite::{Connection, params};

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub article_id: i64,
    pub feed_id: i64,
    pub title: String,
    pub snippet: String,
}

pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    let mut stmt = conn.prepare(
        "SELECT a.id, a.feed_id, a.title,
                snippet(articles_fts, 1, '[', ']', '…', 12) AS snip
         FROM articles_fts f
         JOIN articles a ON a.id = f.rowid
         WHERE articles_fts MATCH ?1
         ORDER BY rank LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![query, limit as i64], |r| {
            Ok(SearchHit {
                article_id: r.get(0)?,
                feed_id: r.get(1)?,
                title: r.get(2)?,
                snippet: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}
