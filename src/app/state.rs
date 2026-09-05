use crate::app::action::Action;
use crate::app::mode::Mode;
use crate::app::search::Search;
use crate::config::Settings;
use crate::doc;
use crate::doc::ir::Document;
use crate::render::sanitize::strip_controls;
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
    /// Whether `r` can re-read this document. A document piped in on stdin
    /// has no file behind it, so there is nothing to re-read.
    pub reloadable: bool,
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
    /// Which input mode the reader is in. While it is anything but
    /// `Reading`, the document must not move behind the overlay or prompt,
    /// so `apply` short-circuits everything except mode switches, the
    /// prompt's own editing keys, and quitting.
    pub mode: Mode,
    /// The committed search, if any. `None` means no highlights, no
    /// indicator, and `n`/`N` do nothing.
    pub search: Option<Search>,
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
            mode: Mode::default(),
            search: None,
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
        self.open_with(path, source, true);
    }

    /// Open a document piped in on stdin. It gets a sentinel path — the
    /// label the status bar and the frame title show — and no reload.
    pub fn open_stdin(&mut self, source: &str) {
        self.open_with(PathBuf::from("(stdin)"), source, false);
    }

    fn open_with(&mut self, path: PathBuf, source: &str, reloadable: bool) {
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
            reloadable,
        });
    }

    /// Read and open `path`. A read failure is reported in the UI, never fatal.
    pub fn open_path(&mut self, path: &Path) {
        match std::fs::read_to_string(path) {
            Ok(source) => self.open_source(path.to_path_buf(), &source),
            Err(e) => {
                self.doc = None;
                // `path` is filesystem data and can legally contain control
                // characters; this message is eventually rendered as a
                // `Span`, so it gets the same sanitisation as document text.
                let shown = strip_controls(&path.display().to_string(), &[]);
                self.error = Some(format!("cannot read {shown}: {e}"));
            }
        }
    }

    /// Reload the currently open document from disk. A no-op for a document
    /// that came in on stdin: there is no file to go back to.
    pub fn reload(&mut self) {
        if !self.doc.as_ref().is_some_and(|d| d.reloadable) {
            return;
        }
        if let Some(path) = self.doc.as_ref().map(|d| d.path.clone()) {
            let scroll = self.doc.as_ref().map(|d| d.scroll).unwrap_or(0);
            self.open_path(&path);
            if let Some(d) = self.doc.as_mut() {
                d.scroll = scroll.min(d.rendered.len().saturating_sub(1));
            }
            self.rerun_search();
        }
    }

    /// The measure text is laid out at, given the terminal width and config.
    ///
    /// Four columns are reserved: two for the viewer's borders, and one of
    /// padding on each side. Without that padding, full-width content — a
    /// horizontal rule, a quote bar — renders flush against the frame and
    /// reads as part of it rather than as content.
    fn content_width(&self) -> u16 {
        self.last_term_width
            .saturating_sub(4)
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
            // The line numbers every match refers to have just changed.
            self.rerun_search();
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
        // Quit and the overlay's own toggle/dismiss work from anywhere,
        // including while the overlay is open.
        match action {
            Action::Quit => {
                self.should_quit = true;
                return;
            }
            Action::Help => {
                self.mode = if self.mode.is_help() {
                    Mode::Reading
                } else {
                    Mode::Help
                };
                return;
            }
            Action::Dismiss => {
                // Help and the prompt take precedence; in Reading, Esc has
                // no overlay to close, so it clears the active search.
                if self.mode.is_reading() {
                    self.search = None;
                }
                self.mode = Mode::Reading;
                return;
            }
            Action::SearchStart => {
                if self.mode.is_reading() {
                    self.mode = Mode::SearchPrompt {
                        query: String::new(),
                    };
                }
                return;
            }
            Action::SearchInput(c) => {
                if let Mode::SearchPrompt { query } = &mut self.mode {
                    query.push(c);
                }
                return;
            }
            Action::SearchBackspace => {
                if let Mode::SearchPrompt { query } = &mut self.mode {
                    query.pop();
                }
                return;
            }
            Action::SearchCommit => {
                self.commit_search();
                return;
            }
            _ => {}
        }

        // Everything else — scrolling, reload — is suspended in every mode
        // but Reading, so the document underneath cannot move.
        if !self.mode.is_reading() {
            return;
        }

        let height = self.viewport_height as i32;
        match action {
            Action::ScrollLines(n) => self.scroll_by(n),
            Action::ScrollHalfPage(n) => self.scroll_by(n * (height / 2).max(1)),
            // One line of overlap so the reader keeps their place across a page.
            Action::ScrollPage(n) => self.scroll_by(n * (height - 1).max(1)),
            Action::Top => self.scroll_by(i32::MIN / 2),
            Action::Bottom => self.scroll_by(i32::MAX / 2),
            Action::Reload => self.reload(),
            Action::NextMatch => self.step_match(1),
            Action::PrevMatch => self.step_match(-1),
            Action::Quit
            | Action::Help
            | Action::Dismiss
            | Action::SearchStart
            | Action::SearchInput(_)
            | Action::SearchBackspace
            | Action::SearchCommit
            | Action::None => {}
        }
    }

    /// Commit the prompt's query: run it, land on the first match at or
    /// after where the reader already is, and leave the prompt.
    fn commit_search(&mut self) {
        let Mode::SearchPrompt { query } = &self.mode else {
            return;
        };
        let query = query.clone();
        self.mode = Mode::Reading;
        if query.is_empty() {
            // Enter on an empty prompt cancels rather than matching every
            // position in the document.
            self.search = None;
            return;
        }
        self.search = Some(Search::new(query));
        self.rerun_search();
        self.reveal_current_match();
    }

    fn step_match(&mut self, delta: i32) {
        if let Some(search) = self.search.as_mut() {
            search.step(delta);
        }
        self.reveal_current_match();
    }

    /// Re-run the active search against the current rendering. The query
    /// survives; the current match is re-derived from the scroll offset.
    /// Called on commit, on reload, and after a resize re-wraps the text.
    fn rerun_search(&mut self) {
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let Some(search) = self.search.as_mut() else {
            return;
        };
        search.rerun(&doc.rendered.lines, doc.scroll);
    }

    /// Bring the current match into view, scrolling as little as possible.
    /// A match already on screen does not move the document — jumping when
    /// the reader can already see the hit is disorienting.
    fn reveal_current_match(&mut self) {
        let Some(m) = self.search.as_ref().and_then(Search::current_match) else {
            return;
        };
        let max = self.max_scroll();
        let height = self.viewport_height as usize;
        let Some(doc) = self.doc.as_mut() else {
            return;
        };
        if m.line < doc.scroll {
            doc.scroll = m.line.min(max);
        } else if height > 0 && m.line >= doc.scroll + height {
            doc.scroll = (m.line + 1 - height).min(max);
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
        // 50 columns less two borders and one column of padding per side.
        assert_eq!(app.doc.as_ref().unwrap().content_width, 46);
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
    fn help_toggles_on_and_off() {
        let mut app = app_with(100, 10);
        assert!(app.mode.is_reading());
        app.apply(Action::Help);
        assert!(app.mode.is_help());
        app.apply(Action::Help);
        assert!(app.mode.is_reading());
    }

    #[test]
    fn dismiss_closes_the_help_overlay() {
        let mut app = app_with(100, 10);
        app.apply(Action::Help);
        assert!(app.mode.is_help());
        app.apply(Action::Dismiss);
        assert!(app.mode.is_reading());
    }

    #[test]
    fn dismiss_is_harmless_when_help_is_already_closed() {
        let mut app = app_with(100, 10);
        app.apply(Action::Dismiss);
        assert!(app.mode.is_reading());
        assert!(!app.should_quit);
    }

    #[test]
    fn scrolling_does_nothing_while_help_is_open() {
        let mut app = app_with(100, 10);
        app.apply(Action::Bottom);
        let scroll_before = app.doc.as_ref().unwrap().scroll;
        app.apply(Action::Help);

        for action in [
            Action::ScrollLines(-5),
            Action::ScrollHalfPage(-1),
            Action::ScrollPage(-1),
            Action::Top,
            Action::Bottom,
        ] {
            app.apply(action);
            assert_eq!(
                app.doc.as_ref().unwrap().scroll,
                scroll_before,
                "{action:?} moved the document while help was open"
            );
        }
    }

    #[test]
    fn reload_does_nothing_while_help_is_open() {
        let mut app = app_with(100, 10);
        app.apply(Action::Help);
        // "a.md" doesn't exist on disk; if Reload were actually attempted it
        // would fail and clear `doc` in favour of an error. It must instead
        // be a complete no-op while the overlay is open.
        app.apply(Action::Reload);
        assert!(app.mode.is_help());
        assert!(app.doc.is_some());
        assert!(app.error.is_none());
    }

    #[test]
    fn every_mode_returns_to_reading_on_dismiss() {
        for mode in [
            Mode::Help,
            Mode::SearchPrompt {
                query: "half typed".to_string(),
            },
        ] {
            let mut app = app_with(100, 10);
            app.mode = mode;
            app.apply(Action::Dismiss);
            assert!(app.mode.is_reading());
        }
    }

    #[test]
    fn the_document_does_not_move_while_the_prompt_is_open() {
        // The spec is explicit: the document does not move while typing.
        let mut app = app_with(100, 10);
        app.apply(Action::ScrollLines(4));
        app.mode = Mode::SearchPrompt {
            query: String::new(),
        };
        for action in [
            Action::ScrollLines(5),
            Action::ScrollHalfPage(1),
            Action::ScrollPage(1),
            Action::Top,
            Action::Bottom,
        ] {
            app.apply(action);
            assert_eq!(
                app.doc.as_ref().unwrap().scroll,
                4,
                "{action:?} moved the document while the prompt was open"
            );
        }
    }

    #[test]
    fn quit_still_works_while_help_is_open() {
        let mut app = app_with(100, 10);
        app.apply(Action::Help);
        app.apply(Action::Quit);
        assert!(app.should_quit);
    }

    /// Type `query` into a fresh prompt and commit it, exactly as the key
    /// handler would.
    fn search_for(app: &mut App, query: &str) {
        app.apply(Action::SearchStart);
        for c in query.chars() {
            app.apply(Action::SearchInput(c));
        }
        app.apply(Action::SearchCommit);
    }

    #[test]
    fn slash_opens_the_prompt_and_typing_builds_the_query() {
        let mut app = app_with(100, 10);
        app.apply(Action::SearchStart);
        app.apply(Action::SearchInput('p'));
        app.apply(Action::SearchInput('7'));
        assert_eq!(
            app.mode,
            Mode::SearchPrompt {
                query: "p7".to_string()
            }
        );
        assert!(app.search.is_none(), "nothing runs until Enter");
    }

    #[test]
    fn backspace_deletes_the_last_character() {
        let mut app = app_with(100, 10);
        app.apply(Action::SearchStart);
        app.apply(Action::SearchInput('a'));
        app.apply(Action::SearchInput('b'));
        app.apply(Action::SearchBackspace);
        app.apply(Action::SearchBackspace);
        app.apply(Action::SearchBackspace); // one too many
        assert_eq!(
            app.mode,
            Mode::SearchPrompt {
                query: String::new()
            }
        );
    }

    #[test]
    fn esc_in_the_prompt_leaves_the_previous_search_untouched() {
        let mut app = app_with(100, 10);
        search_for(&mut app, "p3");
        let before = app.search.clone();
        app.apply(Action::SearchStart);
        app.apply(Action::SearchInput('z'));
        app.apply(Action::Dismiss);
        assert!(app.mode.is_reading());
        assert_eq!(
            app.search, before,
            "cancelling must restore the prior state"
        );
    }

    #[test]
    fn committing_finds_matches_and_starts_at_the_first_one() {
        // app_with lays paragraph `pN` on line 2N.
        let mut app = app_with(100, 10);
        search_for(&mut app, "p7");
        let search = app.search.as_ref().unwrap();
        assert_eq!(search.query, "p7");
        assert_eq!(search.matches.len(), 1);
        assert_eq!(search.current_match().unwrap().line, 14);
        assert!(app.mode.is_reading(), "Enter leaves the prompt");
    }

    #[test]
    fn committing_scrolls_a_match_below_the_viewport_into_view() {
        // "p7" is on line 14; a 10-line viewport at scroll 0 ends at line 9.
        let mut app = app_with(100, 10);
        search_for(&mut app, "p7");
        assert_eq!(app.doc.as_ref().unwrap().scroll, 5);
    }

    #[test]
    fn committing_does_not_move_a_match_that_is_already_visible() {
        // "p2" is on line 4, inside the first screen: no jump.
        let mut app = app_with(100, 10);
        search_for(&mut app, "p2");
        assert_eq!(app.doc.as_ref().unwrap().scroll, 0);
    }

    #[test]
    fn committing_an_empty_query_clears_the_search_rather_than_matching_all() {
        let mut app = app_with(100, 10);
        search_for(&mut app, "p7");
        assert!(app.search.is_some());
        search_for(&mut app, "");
        assert!(app.search.is_none());
    }

    #[test]
    fn a_query_with_no_matches_says_so_and_does_not_move() {
        let mut app = app_with(100, 10);
        app.apply(Action::ScrollLines(3));
        search_for(&mut app, "zzz");
        assert_eq!(app.doc.as_ref().unwrap().scroll, 3);
        assert_eq!(
            app.search.as_ref().unwrap().indicator(),
            "Pattern not found"
        );
    }

    #[test]
    fn n_and_shift_n_wrap_around_the_document() {
        let mut app = app_with(100, 10);
        search_for(&mut app, "p1"); // p1, p10..p19: 11 matches
        let total = app.search.as_ref().unwrap().matches.len();
        assert!(total > 2, "expected several matches, got {total}");

        app.apply(Action::PrevMatch);
        assert_eq!(app.search.as_ref().unwrap().current, Some(total - 1));
        app.apply(Action::NextMatch);
        assert_eq!(app.search.as_ref().unwrap().current, Some(0));
    }

    #[test]
    fn stepping_scrolls_only_when_the_target_is_off_screen() {
        let mut app = app_with(100, 10);
        search_for(&mut app, "p0"); // line 0 only
        assert_eq!(app.doc.as_ref().unwrap().scroll, 0);
        app.apply(Action::NextMatch); // wraps to itself, already visible
        assert_eq!(app.doc.as_ref().unwrap().scroll, 0);
    }

    #[test]
    fn esc_while_reading_clears_the_active_search() {
        let mut app = app_with(100, 10);
        search_for(&mut app, "p7");
        app.apply(Action::Dismiss);
        assert!(app.search.is_none());
        assert!(app.mode.is_reading());
    }

    #[test]
    fn search_keys_are_ignored_while_help_is_open() {
        let mut app = app_with(100, 10);
        app.apply(Action::Help);
        app.apply(Action::SearchStart);
        assert!(app.mode.is_help(), "help takes precedence over the prompt");
        app.apply(Action::NextMatch);
        app.apply(Action::PrevMatch);
        assert!(app.search.is_none());
        assert_eq!(app.doc.as_ref().unwrap().scroll, 0);
    }

    #[test]
    fn resizing_re_runs_the_search_against_the_re_wrapped_lines() {
        let src = "# One\n\nalpha bravo charlie delta echo foxtrot golf hotel india\n\n\
                   needle\n\nmore one\n\nmore two\n\nmore three\n";
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 12);
        app.open_source(PathBuf::from("a.md"), src);
        search_for(&mut app, "needle");
        let wide = app.search.as_ref().unwrap().current_match().unwrap().line;

        // The paragraph above wraps at the narrow measure, pushing the
        // needle down; the query survives and the match line follows it.
        app.set_geometry(40, 12);
        let search = app.search.as_ref().unwrap();
        assert_eq!(search.query, "needle");
        assert_eq!(search.matches.len(), 1);
        assert!(
            search.current_match().unwrap().line > wide,
            "re-wrapping should have moved the match down"
        );
    }

    #[test]
    fn reloading_re_runs_the_search_and_keeps_the_query() {
        let path = std::env::temp_dir().join("mdread_p3_reload_search.md");
        std::fs::write(&path, "alpha\n\nneedle\n\nomega\n").unwrap();

        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 12);
        app.open_path(&path);
        search_for(&mut app, "needle");
        assert_eq!(app.search.as_ref().unwrap().matches.len(), 1);

        std::fs::write(&path, "needle\n\nneedle\n").unwrap();
        app.apply(Action::Reload);
        let search = app.search.as_ref().unwrap();
        assert_eq!(search.query, "needle");
        assert_eq!(search.matches.len(), 2);

        let _ = std::fs::remove_file(&path);
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

    #[test]
    fn a_stdin_document_is_labelled_and_cannot_be_reloaded() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 30);
        app.open_stdin("# Piped\n\nbody\n");

        let d = app.doc.as_ref().unwrap();
        assert_eq!(d.path, PathBuf::from("(stdin)"));
        assert!(!d.reloadable);
        assert!(!d.rendered.is_empty());
        assert!(app.error.is_none());
    }

    #[test]
    fn reload_is_a_no_op_for_a_stdin_document() {
        // "(stdin)" is not a file; if reload tried to read it the document
        // would be replaced by an error.
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 30);
        app.open_stdin("# Piped\n\nbody\n");
        app.apply(Action::Reload);
        assert!(app.doc.is_some());
        assert!(app.error.is_none());
    }

    #[test]
    fn a_file_document_is_reloadable() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 30);
        app.open_source(PathBuf::from("a.md"), "x\n");
        assert!(app.doc.as_ref().unwrap().reloadable);
    }
}
