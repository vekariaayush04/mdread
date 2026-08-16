use crate::app::action::Action;
use crate::config::Settings;
use crate::theme::Theme;
use std::path::PathBuf;

/// A document open in the viewer. Task 16 replaces `total_lines` with a
/// full `RenderedDoc`; the scroll arithmetic here is unchanged by that.
#[derive(Debug, Clone)]
pub struct OpenDoc {
    pub path: PathBuf,
    pub total_lines: usize,
    pub scroll: usize,
}

pub struct App {
    pub settings: Settings,
    pub theme: &'static Theme,
    pub doc: Option<OpenDoc>,
    pub error: Option<String>,
    pub should_quit: bool,
    pub viewport_height: u16,
}

impl App {
    pub fn new(settings: Settings, theme: &'static Theme) -> Self {
        Self {
            settings,
            theme,
            doc: None,
            error: None,
            should_quit: false,
            viewport_height: 0,
        }
    }

    /// Highest valid scroll offset: the last position that still fills the
    /// viewport. Zero when the document is shorter than the viewport.
    pub fn max_scroll(&self) -> usize {
        let Some(doc) = &self.doc else { return 0 };
        doc.total_lines
            .saturating_sub(self.viewport_height as usize)
    }

    fn scroll_by(&mut self, delta: i32) {
        let max = self.max_scroll();
        let Some(doc) = &mut self.doc else { return };
        let next = doc.scroll as i64 + delta as i64;
        doc.scroll = next.clamp(0, max as i64) as usize;
    }

    pub fn apply(&mut self, action: Action) {
        let height = self.viewport_height as i32;
        match action {
            Action::Quit => self.should_quit = true,
            Action::ScrollLines(n) => self.scroll_by(n),
            Action::ScrollHalfPage(n) => self.scroll_by(n * (height / 2).max(1)),
            // One line of overlap so the reader keeps their place across a page.
            Action::ScrollPage(n) => self.scroll_by(n * (height - 1).max(1)),
            Action::Top => self.scroll_by(i32::MIN / 2),
            Action::Bottom => self.scroll_by(i32::MAX / 2),
            // Reload and Help become real in Tasks 19 and 18 respectively.
            Action::Reload | Action::Help | Action::None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme;

    fn app_with(total_lines: usize, viewport_height: u16) -> App {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.viewport_height = viewport_height;
        app.doc = Some(OpenDoc {
            path: PathBuf::from("a.md"),
            total_lines,
            scroll: 0,
        });
        app
    }

    #[test]
    fn quit_sets_the_flag() {
        let mut app = app_with(100, 10);
        app.apply(Action::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn max_scroll_stops_with_a_full_screen_of_content() {
        // 100 lines in a 10-line viewport: last valid top line is 90.
        let app = app_with(100, 10);
        assert_eq!(app.max_scroll(), 90);
    }

    #[test]
    fn max_scroll_is_zero_when_content_fits() {
        let app = app_with(5, 10);
        assert_eq!(app.max_scroll(), 0);
    }

    #[test]
    fn scrolling_down_advances_and_clamps_at_the_bottom() {
        let mut app = app_with(100, 10);
        app.apply(Action::ScrollLines(1));
        assert_eq!(app.doc.as_ref().unwrap().scroll, 1);
        app.apply(Action::Bottom);
        assert_eq!(app.doc.as_ref().unwrap().scroll, 90);
        app.apply(Action::ScrollLines(1));
        assert_eq!(app.doc.as_ref().unwrap().scroll, 90);
    }

    #[test]
    fn scrolling_up_clamps_at_zero_without_underflowing() {
        let mut app = app_with(100, 10);
        app.apply(Action::ScrollLines(-5));
        assert_eq!(app.doc.as_ref().unwrap().scroll, 0);
    }

    #[test]
    fn half_page_moves_by_half_the_viewport() {
        let mut app = app_with(100, 10);
        app.apply(Action::ScrollHalfPage(1));
        assert_eq!(app.doc.as_ref().unwrap().scroll, 5);
    }

    #[test]
    fn full_page_keeps_one_line_of_overlap() {
        // Overlap gives the reader an anchor across the jump.
        let mut app = app_with(100, 10);
        app.apply(Action::ScrollPage(1));
        assert_eq!(app.doc.as_ref().unwrap().scroll, 9);
    }

    #[test]
    fn top_returns_to_the_beginning() {
        let mut app = app_with(100, 10);
        app.apply(Action::Bottom);
        app.apply(Action::Top);
        assert_eq!(app.doc.as_ref().unwrap().scroll, 0);
    }

    #[test]
    fn scroll_actions_are_harmless_with_no_document_open() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.viewport_height = 10;
        app.apply(Action::ScrollLines(1));
        app.apply(Action::Bottom);
        assert_eq!(app.max_scroll(), 0);
    }

    #[test]
    fn a_zero_height_viewport_does_not_panic() {
        // Terminals report height 0 transiently during some resizes.
        let mut app = app_with(100, 0);
        app.apply(Action::ScrollHalfPage(1));
        app.apply(Action::ScrollPage(1));
    }
}
