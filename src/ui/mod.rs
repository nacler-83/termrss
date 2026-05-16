pub mod feeds_pane;
pub mod articles_pane;
pub mod article_view;
pub mod modals;
pub mod statusbar;

use crate::app::{AppState, Focus, Modal, View};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};

pub fn draw(f: &mut Frame, state: &mut AppState) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    match state.view {
        View::List => {
            let panes = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(28), Constraint::Min(20)])
                .split(chunks[0]);
            feeds_pane::draw(f, panes[0], state);
            articles_pane::draw(f, panes[1], state);
        }
        View::Article => {
            article_view::draw(f, chunks[0], state);
        }
    }

    statusbar::draw(f, chunks[1], state);

    match &state.modal {
        Modal::None => {}
        Modal::AddFeed { .. } => modals::draw_add_feed(f, area, state),
        Modal::ConfirmDelete { .. } => modals::draw_confirm_delete(f, area, state),
        Modal::Search { .. } => modals::draw_search(f, area, state),
    }

    let _ = Focus::Feeds;
}
