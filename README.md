# termrss

A terminal RSS / Atom reader for macOS. Two-pane TUI built with Rust + Ratatui.

## Install

Requires Rust (1.75+). Tested with Rust 1.95 from Homebrew.

```sh
brew install rust    # if you don't have it
cargo build --release
./target/release/termrss
```

Or just `cargo run --release`.

## Usage

Launch with `termrss`. The app stores data under standard macOS app dirs:

- Config: `~/Library/Application Support/termrss/config.toml` (auto-created on first run)
- Database: `~/Library/Application Support/termrss/termrss.db`
- Logs: `~/Library/Caches/termrss/termrss.log`

### Adding your first feed

Press `a`, paste a feed URL (e.g. `https://news.ycombinator.com/rss`), press Enter.

### Keybindings

**List view (feeds + articles):**

| Key | Action |
|---|---|
| ↑ / ↓ | Move selection |
| ← / → / Tab | Switch pane |
| Enter | Open feed / open article |
| `a` | Add feed |
| `d` | Delete focused feed |
| `r` | Refresh all feeds |
| `m` | Toggle read/unread on selected article |
| `M` | Mark entire feed as read |
| `s` | Star/unstar article |
| `o` | Open article URL in browser |
| `/` | Search across all articles |
| PgUp / PgDn | Page move |
| `q` | Quit |

**Article view:**

| Key | Action |
|---|---|
| ↑ / ↓ | Scroll |
| PgUp / PgDn | Page scroll |
| `f` | Fetch full article via readability |
| `o` | Open in browser |
| `m` | Toggle read |
| `s` | Toggle star |
| Esc / Backspace | Back to list |
| `q` | Quit |

**Search modal:**

Type to filter (uses SQLite FTS5 with prefix matching). ↑/↓ move, Enter opens the selected hit, Esc cancels.

## Configuration

`config.toml`:

```toml
refresh_interval_minutes = 30
max_articles_per_feed = 500
browser = "open"
theme = ""
```

`refresh_interval_minutes` controls background polling. The app also refreshes on startup and on `r`.

## Architecture

- `src/main.rs` — entry, logging, runtime
- `src/config.rs` — TOML config
- `src/db.rs` — SQLite schema, migrations, queries
- `src/feed.rs` — RSS/Atom fetch + parse (ETag-aware via `feed-rs`)
- `src/article.rs` — HTML → terminal text, optional readability fetch
- `src/refresh.rs` — background refresh task, mpsc updates to UI
- `src/search.rs` — FTS5 search
- `src/app.rs` — state, event loop, key handling
- `src/ui/` — ratatui widgets for each screen

## Logs / debugging

`RUST_LOG=debug termrss` writes verbose logs to `~/Library/Caches/termrss/termrss.log`.
