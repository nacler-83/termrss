use crate::app::{AppState, Modal};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(area.height.saturating_sub(h) / 2),
            Constraint::Length(h),
            Constraint::Min(0),
        ])
        .split(area);
    let h_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(area.width.saturating_sub(w) / 2),
            Constraint::Length(w),
            Constraint::Min(0),
        ])
        .split(v[1]);
    h_layout[1]
}

pub fn draw_add_feed(f: &mut Frame, area: Rect, state: &AppState) {
    let Modal::AddFeed { input } = &state.modal else {
        return;
    };
    let r = centered(area, 60.min(area.width.saturating_sub(4)), 5);
    f.render_widget(Clear, r);
    let block = Block::default()
        .title(" Add feed (Enter to confirm, Esc to cancel) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let p = Paragraph::new(Line::from(vec![
        Span::raw("URL: "),
        Span::styled(input.clone(), Style::default().add_modifier(Modifier::BOLD)),
        Span::styled("_", Style::default().fg(Color::Cyan)),
    ]))
    .block(block);
    f.render_widget(p, r);
}

pub fn draw_confirm_delete(f: &mut Frame, area: Rect, state: &AppState) {
    let Modal::ConfirmDelete { feed_title, .. } = &state.modal else {
        return;
    };
    let r = centered(area, 60.min(area.width.saturating_sub(4)), 5);
    f.render_widget(Clear, r);
    let block = Block::default()
        .title(" Delete feed? ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    let p = Paragraph::new(vec![
        Line::from(format!("Delete '{}' and all its articles?", feed_title)),
        Line::from(Span::styled(
            "y = yes, n / Esc = cancel",
            Style::default().fg(Color::Gray),
        )),
    ])
    .block(block);
    f.render_widget(p, r);
}

pub fn draw_search(f: &mut Frame, area: Rect, state: &AppState) {
    let Modal::Search {
        input,
        results,
        cursor,
    } = &state.modal
    else {
        return;
    };
    let w = (area.width.saturating_sub(6)).min(80);
    let h = (area.height.saturating_sub(4)).min(20).max(8);
    let r = centered(area, w, h);
    f.render_widget(Clear, r);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(r);

    let block = Block::default()
        .title(" Search (Enter to open, Esc to cancel) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let input_p = Paragraph::new(Line::from(vec![
        Span::raw("> "),
        Span::styled(input.clone(), Style::default().add_modifier(Modifier::BOLD)),
        Span::styled("_", Style::default().fg(Color::Cyan)),
    ]))
    .block(block);
    f.render_widget(input_p, chunks[0]);

    let items: Vec<ListItem> = results
        .iter()
        .map(|h| {
            ListItem::new(vec![
                Line::from(Span::styled(
                    h.title.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    h.snippet.clone(),
                    Style::default().fg(Color::Gray),
                )),
            ])
        })
        .collect();
    let results_block = Block::default()
        .title(" Results ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let list = List::new(items)
        .block(results_block)
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
        .highlight_symbol("▸ ");
    let mut ls = ListState::default();
    if !results.is_empty() {
        ls.select(Some((*cursor).min(results.len() - 1)));
    }
    f.render_stateful_widget(list, chunks[1], &mut ls);
}
