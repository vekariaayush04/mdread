pub mod statusbar;
pub mod viewer;

use crate::app::state::App;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

pub fn draw(f: &mut Frame, app: &App) {
    let areas = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(f.area());
    viewer::draw_viewer(f, areas[0], app);
    statusbar::draw_status(f, areas[1], app);
}
