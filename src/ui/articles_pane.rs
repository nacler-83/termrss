use crate::app::{AppState, Focus};
use chrono::{Local, TimeZone};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

pub fn draw(f: &mut Frame, area: Rect, state: &mut AppState) {
    let focused = matches!(state.focus, Focus::Articles);
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = match state.selected_feed_id() {
        Some(id) => state
            .feeds
            .iter()
            .find(|f| f.id == id)
            .map(|f| format!(" {} ", f.title))
            .unwrap_or_else(|| " Articles ".to_string()),
        None => " All articles ".to_string(),
    };

    let items: Vec<ListItem> = state
        .articles
        .iter()
        .map(|a| {
            let mut spans: Vec<Span> = Vec::new();
            spans.push(Span::raw(if a.is_starred { "★ " } else { "  " }));
            let date = a
                .published_at
                .and_then(|ts| Local.timestamp_opt(ts, 0).single())
                .map(|d| d.format("%m-%d ").to_string())
                .unwrap_or_else(|| "      ".to_string());
            spans.push(Span::styled(date, Style::default().fg(Color::DarkGray)));
            let title_style = if a.is_read {
                Style::default().fg(Color::Gray)
            } else {
                Style::default().add_modifier(Modifier::BOLD)
            };
            spans.push(Span::styled(a.title.clone(), title_style));
            ListItem::new(Line::from(spans))
        })
        .collect();

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let highlight = if focused {
        Style::default().bg(Color::Blue).fg(Color::White)
    } else {
        Style::default().add_modifier(Modifier::REVERSED)
    };

    let list = List::new(items)
        .block(block)
        .highlight_style(highlight)
        .highlight_symbol("▸ ");

    let mut ls = ListState::default();
    if !state.articles.is_empty() {
        ls.select(Some(state.article_cursor.min(state.articles.len() - 1)));
    }
    f.render_stateful_widget(list, area, &mut ls);
}
