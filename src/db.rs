use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::Path;

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path).with_context(|| format!("opening db at {:?}", path))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(conn)
}

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS feeds (
            id INTEGER PRIMARY KEY,
            url TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL,
            etag TEXT,
            last_modified TEXT,
            last_refreshed_at INTEGER,
            added_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS articles (
            id INTEGER PRIMARY KEY,
            feed_id INTEGER NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
            guid TEXT NOT NULL,
            url TEXT,
            title TEXT NOT NULL,
            author TEXT,
            published_at INTEGER,
            summary_html TEXT,
            content_html TEXT,
            fetched_at INTEGER NOT NULL,
            is_read INTEGER NOT NULL DEFAULT 0,
            is_starred INTEGER NOT NULL DEFAULT 0,
            UNIQUE(feed_id, guid)
        );

        CREATE INDEX IF NOT EXISTS idx_articles_feed_published
            ON articles(feed_id, published_at DESC);
        CREATE INDEX IF NOT EXISTS idx_articles_unread
            ON articles(is_read, published_at DESC);

        CREATE VIRTUAL TABLE IF NOT EXISTS articles_fts USING fts5(
            title, summary_html, content_html,
            content='articles',
            content_rowid='id',
            tokenize='porter unicode61'
        );

        CREATE TRIGGER IF NOT EXISTS articles_ai AFTER INSERT ON articles BEGIN
            INSERT INTO articles_fts(rowid, title, summary_html, content_html)
            VALUES (new.id, new.title, new.summary_html, new.content_html);
        END;
        CREATE TRIGGER IF NOT EXISTS articles_ad AFTER DELETE ON articles BEGIN
            INSERT INTO articles_fts(articles_fts, rowid, title, summary_html, content_html)
            VALUES('delete', old.id, old.title, old.summary_html, old.content_html);
        END;
        CREATE TRIGGER IF NOT EXISTS articles_au AFTER UPDATE ON articles BEGIN
            INSERT INTO articles_fts(articles_fts, rowid, title, summary_html, content_html)
            VALUES('delete', old.id, old.title, old.summary_html, old.content_html);
            INSERT INTO articles_fts(rowid, title, summary_html, content_html)
            VALUES (new.id, new.title, new.summary_html, new.content_html);
        END;

        CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        "#,
    )
    .context("running migrations")?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Feed {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub unread_count: i64,
}

#[derive(Debug, Clone)]
pub struct Article {
    pub id: i64,
    pub feed_id: i64,
    pub title: String,
    pub author: Option<String>,
    pub url: Option<String>,
    pub published_at: Option<i64>,
    pub summary_html: Option<String>,
    pub content_html: Option<String>,
    pub is_read: bool,
    pub is_starred: bool,
}

pub fn list_feeds(conn: &Connection) -> Result<Vec<Feed>> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.url, f.title, f.etag, f.last_modified,
                COALESCE((SELECT COUNT(*) FROM articles a WHERE a.feed_id = f.id AND a.is_read = 0), 0)
         FROM feeds f
         ORDER BY f.title COLLATE NOCASE",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Feed {
                id: r.get(0)?,
                url: r.get(1)?,
                title: r.get(2)?,
                etag: r.get(3)?,
                last_modified: r.get(4)?,
                unread_count: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn add_feed(conn: &Connection, url: &str, title: &str) -> Result<i64> {
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO feeds (url, title, added_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(url) DO UPDATE SET title = excluded.title",
        params![url, title, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_feed(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM feeds WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn list_articles(conn: &Connection, feed_id: Option<i64>) -> Result<Vec<Article>> {
    let (sql, params_vec): (&str, Vec<rusqlite::types::Value>) = match feed_id {
        Some(fid) => (
            "SELECT id, feed_id, title, author, url, published_at,
                    summary_html, content_html, is_read, is_starred
             FROM articles WHERE feed_id = ?1
             ORDER BY COALESCE(published_at, fetched_at) DESC LIMIT 1000",
            vec![fid.into()],
        ),
        None => (
            "SELECT id, feed_id, title, author, url, published_at,
                    summary_html, content_html, is_read, is_starred
             FROM articles
             ORDER BY COALESCE(published_at, fetched_at) DESC LIMIT 1000",
            vec![],
        ),
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params_vec.iter()), |r| {
            Ok(Article {
                id: r.get(0)?,
                feed_id: r.get(1)?,
                title: r.get(2)?,
                author: r.get(3)?,
                url: r.get(4)?,
                published_at: r.get(5)?,
                summary_html: r.get(6)?,
                content_html: r.get(7)?,
                is_read: r.get::<_, i64>(8)? != 0,
                is_starred: r.get::<_, i64>(9)? != 0,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn set_read(conn: &Connection, article_id: i64, read: bool) -> Result<()> {
    conn.execute(
        "UPDATE articles SET is_read = ?1 WHERE id = ?2",
        params![read as i64, article_id],
    )?;
    Ok(())
}

pub fn set_starred(conn: &Connection, article_id: i64, starred: bool) -> Result<()> {
    conn.execute(
        "UPDATE articles SET is_starred = ?1 WHERE id = ?2",
        params![starred as i64, article_id],
    )?;
    Ok(())
}

pub fn mark_feed_read(conn: &Connection, feed_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE articles SET is_read = 1 WHERE feed_id = ?1 AND is_read = 0",
        params![feed_id],
    )?;
    Ok(())
}

pub fn upsert_article(
    conn: &Connection,
    feed_id: i64,
    guid: &str,
    title: &str,
    url: Option<&str>,
    author: Option<&str>,
    published_at: Option<i64>,
    summary_html: Option<&str>,
    content_html: Option<&str>,
) -> Result<bool> {
    let now = chrono::Utc::now().timestamp();
    let changes = conn.execute(
        "INSERT INTO articles
            (feed_id, guid, url, title, author, published_at, summary_html, content_html, fetched_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(feed_id, guid) DO NOTHING",
        params![
            feed_id,
            guid,
            url,
            title,
            author,
            published_at,
            summary_html,
            content_html,
            now,
        ],
    )?;
    Ok(changes > 0)
}

pub fn update_feed_cache(
    conn: &Connection,
    feed_id: i64,
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "UPDATE feeds SET etag = ?1, last_modified = ?2, last_refreshed_at = ?3 WHERE id = ?4",
        params![etag, last_modified, now, feed_id],
    )?;
    Ok(())
}

pub fn update_article_content(conn: &Connection, article_id: i64, content_html: &str) -> Result<()> {
    conn.execute(
        "UPDATE articles SET content_html = ?1 WHERE id = ?2",
        params![content_html, article_id],
    )?;
    Ok(())
}
