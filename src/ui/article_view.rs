use crate::app::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub fn draw(f: &mut Frame, area: Rect, state: &mut AppState) {
    let Some(a) = state.current_article() else {
        return;
    };

    let mut header_lines: Vec<Line> = Vec::new();
    header_lines.push(Line::from(Span::styled(
        a.title.clone(),
        Style::default().add_modifier(Modifier::BOLD),
    )));
    let mut meta_spans: Vec<Span> = Vec::new();
    if let Some(author) = &a.author {
        meta_spans.push(Span::styled(
            format!("by {author}"),
            Style::default().fg(Color::Gray),
        ));
    }
    if let Some(ts) = a.published_at {
        use chrono::{Local, TimeZone};
        if let Some(d) = Local.timestamp_opt(ts, 0).single() {
            if !meta_spans.is_empty() {
                meta_spans.push(Span::raw("  "));
            }
            meta_spans.push(Span::styled(
                d.format("%Y-%m-%d %H:%M").to_string(),
                Style::default().fg(Color::Gray),
            ));
        }
    }
    if let Some(url) = &a.url {
        if !meta_spans.is_empty() {
            meta_spans.push(Span::raw("  "));
        }
        meta_spans.push(Span::styled(
            url.clone(),
            Style::default().fg(Color::DarkGray),
        ));
    }
    if !meta_spans.is_empty() {
        header_lines.push(Line::from(meta_spans));
    }
    header_lines.push(Line::from(""));

    let body_html = a
        .content_html
        .as_deref()
        .or(a.summary_html.as_deref())
        .unwrap_or("");
    let body_text = crate::article::html_to_text(body_html, area.width.saturating_sub(4) as usize);

    let mut all_lines: Vec<Line> = header_lines;
    for line in body_text.lines() {
        all_lines.push(Line::from(line.to_string()));
    }

    let block = Block::default()
        .title(" Article ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let para = Paragraph::new(all_lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((state.article_scroll, 0));

    f.render_widget(para, area);
}
