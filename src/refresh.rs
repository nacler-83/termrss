use crate::db;
use crate::feed;
use anyhow::Result;
use reqwest::Client;
use rusqlite::Connection;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc::Sender;

#[derive(Debug, Clone)]
pub enum RefreshMsg {
    Started { total: usize },
    FeedDone { feed_id: i64, new_count: usize, done: usize, total: usize },
    FeedError { feed_id: i64, error: String, done: usize, total: usize },
    AllDone { new_total: usize },
}

pub type ConnHandle = Arc<Mutex<Connection>>;

pub async fn refresh_all(
    conn: ConnHandle,
    client: Client,
    tx: Sender<RefreshMsg>,
) -> Result<()> {
    let feeds = {
        let c = conn.lock().await;
        db::list_feeds(&c)?
    };
    let total = feeds.len();
    let _ = tx.send(RefreshMsg::Started { total }).await;
    let mut new_total = 0;
    for (i, f) in feeds.into_iter().enumerate() {
        let result = feed::fetch(&client, &f.url, f.etag.as_deref(), f.last_modified.as_deref()).await;
        match result {
            Ok(fr) => {
                let mut new_count = 0;
                {
                    let c = conn.lock().await;
                    if !fr.not_modified {
                        for e in &fr.entries {
                            if db::upsert_article(
                                &c,
                                f.id,
                                &e.guid,
                                &e.title,
                                e.url.as_deref(),
                                e.author.as_deref(),
                                e.published_at,
                                e.summary_html.as_deref(),
                                e.content_html.as_deref(),
                            )? {
                                new_count += 1;
                            }
                        }
                    }
                    db::update_feed_cache(&c, f.id, fr.etag.as_deref(), fr.last_modified.as_deref())?;
                }
                new_total += new_count;
                let _ = tx
                    .send(RefreshMsg::FeedDone {
                        feed_id: f.id,
                        new_count,
                        done: i + 1,
                        total,
                    })
                    .await;
            }
            Err(e) => {
                let _ = tx
                    .send(RefreshMsg::FeedError {
                        feed_id: f.id,
                        error: e.to_string(),
                        done: i + 1,
                        total,
                    })
                    .await;
            }
        }
    }
    let _ = tx.send(RefreshMsg::AllDone { new_total }).await;
    Ok(())
}
