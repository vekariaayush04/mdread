use crate::app::action::Action;
use crate::config::Settings;
use crate::doc;
use crate::doc::ir::Document;
use crate::render::{RenderedDoc, render};
use crate::theme::Theme;
use std::path::{Path, PathBuf};

/// Content narrower than this is unreadable; we stop shrinking here and
/// let the text clip rather than wrap every word onto its own line.
pub const MIN_CONTENT_WIDTH: u16 = 20;

pub struct OpenDoc {
    pub path: PathBuf,
    pub document: Document,
    pub rendered: RenderedDoc,
    pub scroll: usize,
    pub content_width: u16,
}

pub struct App {
    pub settings: Settings,
    pub theme: &'static Theme,
    pub doc: Option<OpenDoc>,
    pub error: Option<String>,
    pub should_quit: bool,
    pub viewport_height: u16,
    /// Last terminal width seen, used to recompute the measure.
    pub last_term_width: u16,
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
            last_term_width: 0,
        }
    }

    /// Highest valid scroll offset: the last position that still fills the
    /// viewport. Zero when the document is shorter than the viewport.
    pub fn max_scroll(&self) -> usize {
        let Some(doc) = &self.doc else { return 0 };
        doc.rendered
            .len()
            .saturating_sub(self.viewport_height as usize)
    }

    /// Parse and render `source` as the open document. Pure: no filesystem.
    pub fn open_source(&mut self, path: PathBuf, source: &str) {
        let content_width = self.content_width();
        let document = doc::parse(source);
        let rendered = render(&document, content_width, self.theme);
        self.error = None;
        self.doc = Some(OpenDoc {
            path,
            document,
            rendered,
            scroll: 0,
            content_width,
        });
    }

    /// Read and open `path`. A read failure is reported in the UI, never fatal.
    pub fn open_path(&mut self, path: &Path) {
        match std::fs::read_to_string(path) {
            Ok(source) => self.open_source(path.to_path_buf(), &source),
            Err(e) => {
                self.doc = None;
                self.error = Some(format!("cannot read {}: {e}", path.display()));
            }
        }
    }

    /// Reload the currently open document from disk.
    pub fn reload(&mut self) {
        if let Some(path) = self.doc.as_ref().map(|d| d.path.clone()) {
            let scroll = self.doc.as_ref().map(|d| d.scroll).unwrap_or(0);
            self.open_path(&path);
            if let Some(d) = self.doc.as_mut() {
                d.scroll = scroll.min(d.rendered.len().saturating_sub(1));
            }
        }
    }

    /// The measure text is laid out at, given the terminal width and config.
    fn content_width(&self) -> u16 {
        self.last_term_width
            .saturating_sub(2) // the viewer's left and right borders
            .min(self.settings.max_measure)
            .max(MIN_CONTENT_WIDTH)
    }

    /// Record the terminal size and re-render if the measure changed.
    pub fn set_geometry(&mut self, term_width: u16, term_height: u16) {
        self.last_term_width = term_width;
        // Two border rows plus the one-row status bar.
        self.viewport_height = term_height.saturating_sub(3);

        let target = self.content_width();
        let needs_reflow = self.doc.as_ref().is_some_and(|d| d.content_width != target);
        if needs_reflow {
            self.reflow(target);
        }
    }

    /// Re-render at a new measure, keeping the reader on the same block.
    fn reflow(&mut self, content_width: u16) {
        // Capture these before taking the mutable borrow of `self.doc`;
        // calling self.max_scroll() below would be a second borrow of self.
        let theme = self.theme;
        let viewport = self.viewport_height as usize;

        let Some(doc) = self.doc.as_mut() else { return };
        let block = doc.rendered.block_at_line(doc.scroll);
        doc.rendered = render(&doc.document, content_width, theme);
        doc.content_width = content_width;

        let max = doc.rendered.len().saturating_sub(viewport);
        doc.scroll = doc.rendered.line_of_block(block).min(max);
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
            Action::Reload => self.reload(),
            Action::Help | Action::None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme;

    /// Build an app whose document renders to exactly `total_lines` lines.
    fn app_with(total_lines: usize, viewport_height: u16) -> App {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.viewport_height = viewport_height;
        // n paragraphs separated by blank lines render to 2n-1 lines, which
        // is always at least `total_lines`; we then trim to the exact length.
        let src: String = (0..total_lines).map(|i| format!("p{i}\n\n")).collect();
        app.open_source(PathBuf::from("a.md"), &src);
        app.doc
            .as_mut()
            .unwrap()
            .rendered
            .lines
            .truncate(total_lines);
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

    #[test]
    fn opening_a_source_renders_it_and_resets_scroll() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 30);
        app.open_source(PathBuf::from("a.md"), "# Title\n\nbody\n");
        let d = app.doc.as_ref().unwrap();
        assert_eq!(d.scroll, 0);
        assert_eq!(d.document.outline.len(), 1);
        assert!(!d.rendered.is_empty());
        assert!(app.error.is_none());
    }

    #[test]
    fn opening_a_new_document_clears_a_previous_error() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 30);
        app.error = Some("stale".into());
        app.open_source(PathBuf::from("a.md"), "x\n");
        assert!(app.error.is_none());
    }

    #[test]
    fn content_width_is_capped_by_the_max_measure() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        // Default max_measure is 80; a 200-column terminal must not use it all.
        app.set_geometry(200, 30);
        app.open_source(PathBuf::from("a.md"), "x\n");
        assert_eq!(app.doc.as_ref().unwrap().content_width, 80);
    }

    #[test]
    fn content_width_follows_a_narrow_terminal() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(50, 30);
        app.open_source(PathBuf::from("a.md"), "x\n");
        assert_eq!(app.doc.as_ref().unwrap().content_width, 48);
    }

    #[test]
    fn content_width_never_drops_below_the_minimum() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(4, 30);
        app.open_source(PathBuf::from("a.md"), "x\n");
        assert_eq!(app.doc.as_ref().unwrap().content_width, MIN_CONTENT_WIDTH);
    }

    #[test]
    fn viewport_height_excludes_borders_and_the_status_bar() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 30);
        assert_eq!(app.viewport_height, 27);
    }

    #[test]
    fn a_tiny_terminal_does_not_panic_or_underflow() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        for (w, h) in [(0, 0), (1, 1), (2, 3), (3, 2)] {
            app.set_geometry(w, h);
            app.open_source(PathBuf::from("a.md"), "hello world\n");
            app.apply(Action::ScrollPage(1));
        }
    }

    #[test]
    fn resizing_keeps_the_reader_on_the_same_block() {
        // The paragraph fits one line at the wide measure and wraps to two at
        // the narrow one, so every block after it shifts down by a line. The
        // document must also outrun the viewport, or scroll clamps to 0 and
        // the test proves nothing.
        let src = "# One\n\nalpha bravo charlie delta echo foxtrot golf hotel india\n\n\
                   # Two\n\nzzz\n\nmore one\n\nmore two\n\nmore three\n";
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 10);
        app.open_source(PathBuf::from("a.md"), src);

        // Scroll to the "# Two" heading.
        let target_block = 2;
        let line = app
            .doc
            .as_ref()
            .unwrap()
            .rendered
            .line_of_block(target_block);
        app.doc.as_mut().unwrap().scroll = line;

        // Shrink the terminal; the paragraph above now wraps onto more lines.
        app.set_geometry(40, 10);

        let d = app.doc.as_ref().unwrap();
        assert_eq!(
            d.rendered.block_at_line(d.scroll),
            target_block,
            "resize should land the reader back on the same block"
        );
    }

    #[test]
    fn resizing_clamps_scroll_when_the_document_gets_shorter() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(40, 30);
        app.open_source(PathBuf::from("a.md"), "aaa bbb ccc ddd eee fff ggg hhh\n");
        app.apply(Action::Bottom);
        app.set_geometry(200, 30);
        let d = app.doc.as_ref().unwrap();
        assert!(d.scroll <= app.max_scroll());
    }

    #[test]
    fn geometry_changes_before_any_document_is_open_are_harmless() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 30);
        app.set_geometry(50, 20);
        assert!(app.doc.is_none());
    }

    #[test]
    fn a_missing_file_records_an_error_instead_of_failing() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 30);
        app.open_path(Path::new("/nonexistent/definitely-not-here.md"));
        assert!(app.doc.is_none());
        let msg = app.error.as_deref().unwrap();
        assert!(
            msg.contains("definitely-not-here.md"),
            "unhelpful error: {msg}"
        );
        assert!(!app.should_quit, "a bad file must not end the session");
    }
}
