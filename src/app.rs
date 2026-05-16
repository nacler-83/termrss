use crate::config::Config;
use crate::db::{self, Article, Feed};
use crate::feed as feedmod;
use crate::refresh::{self, RefreshMsg};
use crate::search::{self, SearchHit};
use crate::ui;
use anyhow::{Context, Result};
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use rusqlite::Connection;
use std::io::Stdout;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Feeds,
    Articles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    List,
    Article,
}

#[derive(Debug, Clone)]
pub enum Modal {
    None,
    AddFeed { input: String },
    ConfirmDelete { feed_id: i64, feed_title: String },
    Search { input: String, results: Vec<SearchHit>, cursor: usize },
}

pub struct AppState {
    pub feeds: Vec<Feed>,
    pub articles: Vec<Article>,
    pub feed_cursor: usize,
    pub article_cursor: usize,
    pub focus: Focus,
    pub view: View,
    pub modal: Modal,
    pub article_scroll: u16,
    pub status: Option<String>,
    pub current_article_id: Option<i64>,
}

impl AppState {
    pub fn selected_feed_id(&self) -> Option<i64> {
        if self.feed_cursor == 0 {
            None
        } else {
            self.feeds.get(self.feed_cursor - 1).map(|f| f.id)
        }
    }

    pub fn current_article(&self) -> Option<&Article> {
        self.articles
            .iter()
            .find(|a| Some(a.id) == self.current_article_id)
    }

    pub fn current_article_mut(&mut self) -> Option<&mut Article> {
        let id = self.current_article_id?;
        self.articles.iter_mut().find(|a| a.id == id)
    }
}

pub async fn run(cfg: Config, conn: Connection) -> Result<()> {
    let conn = Arc::new(Mutex::new(conn));
    let client = feedmod::build_client()?;

    let feeds = {
        let c = conn.lock().await;
        db::list_feeds(&c)?
    };
    let articles = {
        let c = conn.lock().await;
        db::list_articles(&c, None)?
    };

    let mut state = AppState {
        feeds,
        articles,
        feed_cursor: 0,
        article_cursor: 0,
        focus: Focus::Feeds,
        view: View::List,
        modal: Modal::None,
        article_scroll: 0,
        status: None,
        current_article_id: None,
    };

    let mut terminal = setup_terminal()?;
    let result = event_loop(&mut terminal, &mut state, &cfg, conn.clone(), client).await;
    restore_terminal(&mut terminal)?;
    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("enable raw mode")?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("entering alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend).context("creating terminal")?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().ok();
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .ok();
    terminal.show_cursor().ok();
    Ok(())
}

async fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state: &mut AppState,
    cfg: &Config,
    conn: Arc<Mutex<Connection>>,
    client: reqwest::Client,
) -> Result<()> {
    let (rtx, mut rrx) = mpsc::channel::<RefreshMsg>(64);
    let mut refreshing = true;

    // Initial refresh on startup
    spawn_refresh(conn.clone(), client.clone(), rtx.clone());

    // Periodic refresh
    let refresh_interval = Duration::from_secs(cfg.refresh_interval_minutes.saturating_mul(60).max(60));
    let mut interval = tokio::time::interval(refresh_interval);
    interval.tick().await; // consume immediate tick

    loop {
        terminal.draw(|f| ui::draw(f, state))?;

        tokio::select! {
            _ = interval.tick() => {
                if !refreshing {
                    spawn_refresh(conn.clone(), client.clone(), rtx.clone());
                    refreshing = true;
                }
            }
            Some(msg) = rrx.recv() => {
                handle_refresh_msg(msg, state, &conn, &mut refreshing).await?;
            }
            ev = read_event() => {
                match ev? {
                    Some(Event::Key(k)) if k.kind == KeyEventKind::Press => {
                        if handle_key(k, state, cfg, &conn, &client, &rtx).await? {
                            return Ok(());
                        }
                    }
                    Some(Event::Resize(_, _)) => {}
                    _ => {}
                }
            }
        }
    }
}

async fn read_event() -> Result<Option<Event>> {
    // Poll crossterm events without blocking the runtime.
    tokio::task::spawn_blocking(|| -> Result<Option<Event>> {
        if crossterm::event::poll(Duration::from_millis(200))? {
            Ok(Some(crossterm::event::read()?))
        } else {
            Ok(None)
        }
    })
    .await
    .context("event task join")?
}

fn spawn_refresh(
    conn: Arc<Mutex<Connection>>,
    client: reqwest::Client,
    tx: mpsc::Sender<RefreshMsg>,
) {
    tokio::spawn(async move {
        if let Err(e) = refresh::refresh_all(conn, client, tx).await {
            tracing::error!("refresh failed: {e:?}");
        }
    });
}

async fn handle_refresh_msg(
    msg: RefreshMsg,
    state: &mut AppState,
    conn: &Arc<Mutex<Connection>>,
    refreshing: &mut bool,
) -> Result<()> {
    match msg {
        RefreshMsg::Started { total } => {
            state.status = Some(format!("Refreshing 0/{total}…"));
        }
        RefreshMsg::FeedDone { done, total, .. } => {
            state.status = Some(format!("Refreshing {done}/{total}…"));
        }
        RefreshMsg::FeedError { done, total, error, .. } => {
            tracing::warn!("feed error: {error}");
            state.status = Some(format!("Refreshing {done}/{total} (err)…"));
        }
        RefreshMsg::AllDone { new_total } => {
            state.status = Some(if new_total == 0 {
                "Refresh done (no new)".to_string()
            } else {
                format!("Refresh done (+{new_total})")
            });
            reload_lists(state, conn).await?;
            *refreshing = false;
        }
    }
    Ok(())
}

async fn reload_lists(state: &mut AppState, conn: &Arc<Mutex<Connection>>) -> Result<()> {
    let c = conn.lock().await;
    state.feeds = db::list_feeds(&c)?;
    state.articles = db::list_articles(&c, state.selected_feed_id())?;
    if state.article_cursor >= state.articles.len() {
        state.article_cursor = state.articles.len().saturating_sub(1);
    }
    Ok(())
}

async fn handle_key(
    key: KeyEvent,
    state: &mut AppState,
    cfg: &Config,
    conn: &Arc<Mutex<Connection>>,
    client: &reqwest::Client,
    rtx: &mpsc::Sender<RefreshMsg>,
) -> Result<bool> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
        return Ok(true);
    }

    // Modal dispatch first
    match &state.modal {
        Modal::AddFeed { .. } => return handle_add_feed_key(key, state, conn, client).await,
        Modal::ConfirmDelete { feed_id, .. } => {
            let feed_id = *feed_id;
            return handle_confirm_delete_key(key, feed_id, state, conn).await;
        }
        Modal::Search { .. } => return handle_search_key(key, state, conn).await,
        Modal::None => {}
    }

    if state.view == View::Article {
        return handle_article_key(key, state, conn, client).await;
    }

    // List view
    match key.code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Char('a') => state.modal = Modal::AddFeed { input: String::new() },
        KeyCode::Char('d') => {
            if let Some(id) = state.selected_feed_id() {
                if let Some(f) = state.feeds.iter().find(|f| f.id == id) {
                    state.modal = Modal::ConfirmDelete {
                        feed_id: id,
                        feed_title: f.title.clone(),
                    };
                }
            }
        }
        KeyCode::Char('r') => {
            spawn_refresh(conn.clone(), client.clone(), rtx.clone());
        }
        KeyCode::Char('/') => {
            state.modal = Modal::Search {
                input: String::new(),
                results: vec![],
                cursor: 0,
            };
        }
        KeyCode::Char('M') => {
            if let Some(id) = state.selected_feed_id() {
                let c = conn.lock().await;
                db::mark_feed_read(&c, id)?;
                drop(c);
                reload_lists(state, conn).await?;
            }
        }
        KeyCode::Char('m') if state.focus == Focus::Articles => {
            if let Some(a) = state.articles.get(state.article_cursor) {
                let new_read = !a.is_read;
                let id = a.id;
                let c = conn.lock().await;
                db::set_read(&c, id, new_read)?;
                drop(c);
                reload_lists(state, conn).await?;
            }
        }
        KeyCode::Char('s') if state.focus == Focus::Articles => {
            if let Some(a) = state.articles.get(state.article_cursor) {
                let new_starred = !a.is_starred;
                let id = a.id;
                let c = conn.lock().await;
                db::set_starred(&c, id, new_starred)?;
                drop(c);
                reload_lists(state, conn).await?;
            }
        }
        KeyCode::Char('o') if state.focus == Focus::Articles => {
            if let Some(a) = state.articles.get(state.article_cursor) {
                if let Some(url) = a.url.clone() {
                    open_url(&cfg.browser, &url);
                }
            }
        }
        KeyCode::Up | KeyCode::Char('k') => move_cursor(state, -1),
        KeyCode::Down | KeyCode::Char('j') => move_cursor(state, 1),
        KeyCode::PageUp => move_cursor(state, -10),
        KeyCode::PageDown => move_cursor(state, 10),
        KeyCode::Home => move_cursor_to(state, 0),
        KeyCode::End => move_cursor_to(state, usize::MAX),
        KeyCode::Left | KeyCode::BackTab => {
            state.focus = Focus::Feeds;
        }
        KeyCode::Right | KeyCode::Tab => {
            if !state.articles.is_empty() {
                state.focus = Focus::Articles;
            }
        }
        KeyCode::Enter => match state.focus {
            Focus::Feeds => {
                let c = conn.lock().await;
                state.articles = db::list_articles(&c, state.selected_feed_id())?;
                drop(c);
                state.article_cursor = 0;
                if !state.articles.is_empty() {
                    state.focus = Focus::Articles;
                }
            }
            Focus::Articles => {
                if let Some(a) = state.articles.get(state.article_cursor) {
                    let id = a.id;
                    state.current_article_id = Some(id);
                    state.view = View::Article;
                    state.article_scroll = 0;
                    if !a.is_read {
                        let c = conn.lock().await;
                        db::set_read(&c, id, true)?;
                        drop(c);
                        reload_lists(state, conn).await?;
                    }
                }
            }
        },
        _ => {}
    }
    Ok(false)
}

fn move_cursor(state: &mut AppState, delta: i32) {
    let (cur, max) = match state.focus {
        Focus::Feeds => (state.feed_cursor as i32, state.feeds.len() as i32), // +1 for "All"
        Focus::Articles => (
            state.article_cursor as i32,
            state.articles.len() as i32 - 1,
        ),
    };
    let new = (cur + delta).max(0).min(max.max(0));
    match state.focus {
        Focus::Feeds => {
            // feeds list: cursor 0 = All, 1..=N = each feed
            let upper = state.feeds.len() as i32; // index N is valid (last feed)
            state.feed_cursor = (cur + delta).max(0).min(upper) as usize;
            let _ = new;
        }
        Focus::Articles => {
            if state.articles.is_empty() {
                state.article_cursor = 0;
            } else {
                let upper = state.articles.len() as i32 - 1;
                state.article_cursor = (cur + delta).max(0).min(upper) as usize;
            }
        }
    }
}

fn move_cursor_to(state: &mut AppState, idx: usize) {
    match state.focus {
        Focus::Feeds => {
            let upper = state.feeds.len();
            state.feed_cursor = idx.min(upper);
        }
        Focus::Articles => {
            if state.articles.is_empty() {
                state.article_cursor = 0;
            } else {
                state.article_cursor = idx.min(state.articles.len() - 1);
            }
        }
    }
}

async fn handle_add_feed_key(
    key: KeyEvent,
    state: &mut AppState,
    conn: &Arc<Mutex<Connection>>,
    client: &reqwest::Client,
) -> Result<bool> {
    match key.code {
        KeyCode::Esc => {
            state.modal = Modal::None;
        }
        KeyCode::Enter => {
            let url = if let Modal::AddFeed { input } = &state.modal {
                input.trim().to_string()
            } else {
                return Ok(false);
            };
            if url.is_empty() {
                state.modal = Modal::None;
                return Ok(false);
            }
            state.status = Some(format!("Adding {url}…"));
            match feedmod::fetch(client, &url, None, None).await {
                Ok(fr) => {
                    let title = fr.title.clone().unwrap_or_else(|| url.clone());
                    let c = conn.lock().await;
                    let feed_id = db::add_feed(&c, &url, &title)?;
                    if !fr.not_modified {
                        for e in &fr.entries {
                            db::upsert_article(
                                &c,
                                feed_id,
                                &e.guid,
                                &e.title,
                                e.url.as_deref(),
                                e.author.as_deref(),
                                e.published_at,
                                e.summary_html.as_deref(),
                                e.content_html.as_deref(),
                            )?;
                        }
                    }
                    db::update_feed_cache(&c, feed_id, fr.etag.as_deref(), fr.last_modified.as_deref())?;
                    drop(c);
                    state.status = Some(format!("Added {title}"));
                    reload_lists(state, conn).await?;
                }
                Err(e) => {
                    state.status = Some(format!("Add failed: {e}"));
                }
            }
            state.modal = Modal::None;
        }
        KeyCode::Backspace => {
            if let Modal::AddFeed { input } = &mut state.modal {
                input.pop();
            }
        }
        KeyCode::Char(c) => {
            if let Modal::AddFeed { input } = &mut state.modal {
                input.push(c);
            }
        }
        _ => {}
    }
    Ok(false)
}

async fn handle_confirm_delete_key(
    key: KeyEvent,
    feed_id: i64,
    state: &mut AppState,
    conn: &Arc<Mutex<Connection>>,
) -> Result<bool> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            let c = conn.lock().await;
            db::delete_feed(&c, feed_id)?;
            drop(c);
            state.modal = Modal::None;
            state.feed_cursor = 0;
            state.focus = Focus::Feeds;
            reload_lists(state, conn).await?;
            state.status = Some("Feed deleted".to_string());
        }
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
            state.modal = Modal::None;
        }
        _ => {}
    }
    Ok(false)
}

async fn handle_search_key(
    key: KeyEvent,
    state: &mut AppState,
    conn: &Arc<Mutex<Connection>>,
) -> Result<bool> {
    let mut rerun = false;
    match key.code {
        KeyCode::Esc => {
            state.modal = Modal::None;
            return Ok(false);
        }
        KeyCode::Enter => {
            let target = if let Modal::Search { results, cursor, .. } = &state.modal {
                results.get(*cursor).map(|h| (h.article_id, h.feed_id))
            } else {
                None
            };
            if let Some((article_id, feed_id)) = target {
                let c = conn.lock().await;
                state.articles = db::list_articles(&c, Some(feed_id))?;
                drop(c);
                state.article_cursor = state
                    .articles
                    .iter()
                    .position(|a| a.id == article_id)
                    .unwrap_or(0);
                state.feed_cursor = state
                    .feeds
                    .iter()
                    .position(|f| f.id == feed_id)
                    .map(|i| i + 1)
                    .unwrap_or(0);
                state.current_article_id = Some(article_id);
                state.view = View::Article;
                state.article_scroll = 0;
                state.modal = Modal::None;
                let c = conn.lock().await;
                db::set_read(&c, article_id, true)?;
                drop(c);
                reload_lists(state, conn).await?;
            }
            return Ok(false);
        }
        KeyCode::Up => {
            if let Modal::Search { cursor, .. } = &mut state.modal {
                *cursor = cursor.saturating_sub(1);
            }
        }
        KeyCode::Down => {
            if let Modal::Search { cursor, results, .. } = &mut state.modal {
                if !results.is_empty() {
                    *cursor = (*cursor + 1).min(results.len() - 1);
                }
            }
        }
        KeyCode::Backspace => {
            if let Modal::Search { input, .. } = &mut state.modal {
                input.pop();
                rerun = true;
            }
        }
        KeyCode::Char(c) => {
            if let Modal::Search { input, .. } = &mut state.modal {
                input.push(c);
                rerun = true;
            }
        }
        _ => {}
    }

    if rerun {
        let q = if let Modal::Search { input, .. } = &state.modal {
            input.clone()
        } else {
            String::new()
        };
        let hits = if q.trim().is_empty() {
            vec![]
        } else {
            let c = conn.lock().await;
            let fts_q = build_fts_query(&q);
            search::search(&c, &fts_q, 50).unwrap_or_default()
        };
        if let Modal::Search { results, cursor, .. } = &mut state.modal {
            *results = hits;
            if results.is_empty() {
                *cursor = 0;
            } else if *cursor >= results.len() {
                *cursor = results.len() - 1;
            }
        }
    }

    Ok(false)
}

fn build_fts_query(input: &str) -> String {
    // Treat each term as a prefix match; quote to escape unwelcome chars.
    input
        .split_whitespace()
        .map(|w| {
            let cleaned: String = w
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if cleaned.is_empty() {
                String::new()
            } else {
                format!("{cleaned}*")
            }
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

async fn handle_article_key(
    key: KeyEvent,
    state: &mut AppState,
    conn: &Arc<Mutex<Connection>>,
    client: &reqwest::Client,
) -> Result<bool> {
    match key.code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('h') => {
            state.view = View::List;
            state.current_article_id = None;
            state.article_scroll = 0;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.article_scroll = state.article_scroll.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.article_scroll = state.article_scroll.saturating_add(1);
        }
        KeyCode::PageUp => {
            state.article_scroll = state.article_scroll.saturating_sub(10);
        }
        KeyCode::PageDown => {
            state.article_scroll = state.article_scroll.saturating_add(10);
        }
        KeyCode::Home | KeyCode::Char('g') => state.article_scroll = 0,
        KeyCode::Char('m') => {
            if let Some(a) = state.current_article() {
                let new_read = !a.is_read;
                let id = a.id;
                let c = conn.lock().await;
                db::set_read(&c, id, new_read)?;
                drop(c);
                reload_lists(state, conn).await?;
            }
        }
        KeyCode::Char('s') => {
            if let Some(a) = state.current_article() {
                let new_starred = !a.is_starred;
                let id = a.id;
                let c = conn.lock().await;
                db::set_starred(&c, id, new_starred)?;
                drop(c);
                reload_lists(state, conn).await?;
            }
        }
        KeyCode::Char('o') => {
            if let Some(a) = state.current_article() {
                if let Some(url) = a.url.clone() {
                    let browser = std::env::var("BROWSER").ok();
                    let prog = browser.as_deref().unwrap_or("open");
                    open_url(prog, &url);
                }
            }
        }
        KeyCode::Char('f') => {
            let target = state.current_article().and_then(|a| a.url.clone());
            let article_id = state.current_article_id;
            if let (Some(url), Some(id)) = (target, article_id) {
                state.status = Some("Fetching full article…".to_string());
                match crate::article::fetch_full_article(client, &url).await {
                    Ok(html) => {
                        let c = conn.lock().await;
                        db::update_article_content(&c, id, &html)?;
                        drop(c);
                        if let Some(a) = state.current_article_mut() {
                            a.content_html = Some(html);
                        }
                        state.status = Some("Full article loaded".to_string());
                    }
                    Err(e) => state.status = Some(format!("Fetch failed: {e}")),
                }
            }
        }
        _ => {}
    }
    Ok(false)
}

fn open_url(program: &str, url: &str) {
    let _ = std::process::Command::new(program).arg(url).spawn();
}
