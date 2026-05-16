use crate::app::{AppState, Modal, View};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub fn draw(f: &mut Frame, area: Rect, state: &AppState) {
    let hints: &str = if matches!(state.modal, Modal::Search { .. }) {
        "Type to search  •  ↑/↓ select  •  Enter open  •  Esc cancel"
    } else if matches!(state.modal, Modal::AddFeed { .. }) {
        "Type URL  •  Enter add  •  Esc cancel"
    } else if matches!(state.modal, Modal::ConfirmDelete { .. }) {
        "y confirm  •  n / Esc cancel"
    } else {
        match state.view {
            View::List => {
                "↑/↓ move  •  ←/→ panes  •  Enter open  •  a add  •  d del  •  r refresh  •  m read  •  M all read  •  s star  •  o browser  •  / search  •  q quit"
            }
            View::Article => {
                "↑/↓ scroll  •  m read  •  s star  •  f full page  •  o browser  •  Esc back  •  q quit"
            }
        }
    };

    let status = state.status.clone().unwrap_or_default();
    let left = Span::styled(hints, Style::default().fg(Color::DarkGray));
    let right = Span::styled(status, Style::default().fg(Color::Yellow));

    let line = if right.content.is_empty() {
        Line::from(left)
    } else {
        Line::from(vec![left, Span::raw("  "), right])
    };
    f.render_widget(Paragraph::new(line), area);
}
