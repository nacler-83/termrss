use crate::app::{AppState, Focus};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

pub fn draw(f: &mut Frame, area: Rect, state: &mut AppState) {
    let focused = matches!(state.focus, Focus::Feeds);
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let total_unread: i64 = state.feeds.iter().map(|f| f.unread_count).sum();
    let mut items: Vec<ListItem> = Vec::with_capacity(state.feeds.len() + 1);
    items.push(ListItem::new(Line::from(vec![
        Span::styled("All", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(format!(" ({total_unread})")),
    ])));

    for feed in &state.feeds {
        let mut spans = vec![Span::raw(truncate(&feed.title, 22))];
        if feed.unread_count > 0 {
            spans.push(Span::styled(
                format!(" ({})", feed.unread_count),
                Style::default().fg(Color::Yellow),
            ));
        }
        items.push(ListItem::new(Line::from(spans)));
    }

    let block = Block::default()
        .title(" Feeds ")
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
    ls.select(Some(state.feed_cursor));
    f.render_stateful_widget(list, area, &mut ls);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
