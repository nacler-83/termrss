mod app;
mod config;
mod db;
mod feed;
mod article;
mod refresh;
mod search;
mod ui;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    init_logging()?;
    let cfg = config::Config::load_or_init()?;
    let conn = db::open(&cfg.db_path())?;
    db::migrate(&conn)?;

    app::run(cfg, conn).await
}

fn init_logging() -> Result<()> {
    let log_dir = directories::ProjectDirs::from("", "", "termrss")
        .map(|p| p.cache_dir().to_path_buf())
        .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&log_dir).ok();
    let file_appender = tracing_appender::rolling::never(&log_dir, "termrss.log");
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(file_appender)
        .with_ansi(false)
        .init();
    Ok(())
}
