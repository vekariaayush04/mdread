pub mod code;
pub mod inline;
pub mod layout;

use ratatui::text::Line;

/// The result of laying out one block. `anchor` carries a heading's slug so
/// the document renderer can index it; everything else is plain lines.
#[derive(Debug, Clone, Default)]
pub struct BlockRender {
    pub lines: Vec<Line<'static>>,
    pub anchor: Option<String>,
}
