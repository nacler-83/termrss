use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_refresh_minutes")]
    pub refresh_interval_minutes: u64,
    #[serde(default = "default_max_articles")]
    pub max_articles_per_feed: usize,
    #[serde(default = "default_browser")]
    pub browser: String,
    #[serde(default)]
    pub theme: String,
    #[serde(skip)]
    data_dir: PathBuf,
}

fn default_refresh_minutes() -> u64 {
    30
}
fn default_max_articles() -> usize {
    500
}
fn default_browser() -> String {
    "open".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            refresh_interval_minutes: default_refresh_minutes(),
            max_articles_per_feed: default_max_articles(),
            browser: default_browser(),
            theme: String::new(),
            data_dir: PathBuf::new(),
        }
    }
}

impl Config {
    pub fn load_or_init() -> Result<Self> {
        let dirs = ProjectDirs::from("", "", "termrss")
            .context("could not determine project directories")?;
        let config_dir = dirs.config_dir();
        let data_dir = dirs.data_dir();
        std::fs::create_dir_all(config_dir).context("creating config dir")?;
        std::fs::create_dir_all(data_dir).context("creating data dir")?;

        let config_path = config_dir.join("config.toml");
        let mut cfg: Config = if config_path.exists() {
            let text = std::fs::read_to_string(&config_path).context("reading config.toml")?;
            toml::from_str(&text).context("parsing config.toml")?
        } else {
            let cfg = Config::default();
            let text = toml::to_string_pretty(&cfg).context("serializing default config")?;
            std::fs::write(&config_path, text).context("writing default config.toml")?;
            cfg
        };
        cfg.data_dir = data_dir.to_path_buf();
        Ok(cfg)
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("termrss.db")
    }
}
