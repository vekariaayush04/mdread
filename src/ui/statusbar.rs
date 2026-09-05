use crate::app::mode::Mode;
use crate::app::state::App;
use crate::render::sanitize::strip_controls;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// A `less`-style position indicator: All / Top / Bot / a percentage.
/// A bare "100%" when everything already fits reads as a bug, so it is
/// distinguished from genuinely being at the end of a long document.
pub fn scroll_indicator(scroll: usize, max_scroll: usize, has_doc: bool) -> String {
    if !has_doc {
        return String::new();
    }
    if max_scroll == 0 {
        return "All".to_string();
    }
    match scroll {
        0 => "Top".to_string(),
        s if s >= max_scroll => "Bot".to_string(),
        s => format!("{}%", s * 100 / max_scroll),
    }
}

/// The `/` prompt, drawn in the status row. The trailing bar is the cursor:
/// the real terminal cursor is hidden for the whole session, so the prompt
/// has to draw its own.
///
/// The query is user input, so it goes through the same `strip_controls`
/// sanitiser as document text before it becomes a `Span`.
fn prompt_line(query: &str, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled("/", Style::default().fg(theme.accent)),
        Span::styled(strip_controls(query, &[]), Style::default().fg(theme.text)),
        Span::styled("▏", Style::default().fg(theme.accent)),
    ])
}

pub fn status_line(app: &App) -> Line<'static> {
    // While the prompt is open it owns the whole row: there is nowhere else
    // to put a one-line editor.
    if let Mode::SearchPrompt { query } = &app.mode {
        return prompt_line(query, app.theme);
    }

    // A filename can legally contain control characters (it is filesystem
    // data, not something this reader chose), so it gets the same treatment
    // as document body text before it becomes a `Span`.
    let path = app
        .doc
        .as_ref()
        .map(|d| strip_controls(&d.path.display().to_string(), &[]))
        .unwrap_or_else(|| "no document".to_string());
    let position = scroll_indicator(
        app.doc.as_ref().map(|d| d.scroll).unwrap_or(0),
        app.max_scroll(),
        app.doc.is_some(),
    );

    let muted = Style::default().fg(app.theme.muted);
    let accent = Style::default().fg(app.theme.accent);
    let mut spans = vec![Span::styled(path, accent), Span::styled("  ", muted)];
    if !position.is_empty() {
        spans.push(Span::styled(position, muted));
        spans.push(Span::styled("  ", muted));
    }
    if let Some(search) = &app.search {
        spans.push(Span::styled(search.indicator(), muted));
        spans.push(Span::styled("  ", muted));
    }
    spans.push(Span::styled("q quit  ? help", muted));
    Line::from(spans)
}

pub fn draw_status(f: &mut Frame, area: Rect, app: &App) {
    f.render_widget(Paragraph::new(status_line(app)), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;
    use crate::theme;
    use std::path::PathBuf;

    #[test]
    fn a_document_that_fits_entirely_reads_all() {
        assert_eq!(scroll_indicator(0, 0, true), "All");
    }

    #[test]
    fn the_start_and_end_read_top_and_bot() {
        assert_eq!(scroll_indicator(0, 100, true), "Top");
        assert_eq!(scroll_indicator(100, 100, true), "Bot");
    }

    #[test]
    fn the_middle_reads_as_a_percentage() {
        assert_eq!(scroll_indicator(50, 100, true), "50%");
        assert_eq!(scroll_indicator(25, 100, true), "25%");
    }

    #[test]
    fn no_document_has_no_indicator() {
        assert_eq!(scroll_indicator(0, 0, false), "");
    }

    #[test]
    fn the_status_line_names_the_open_file() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 30);
        app.open_source(PathBuf::from("docs/intro.md"), "x\n");
        let text: String = status_line(&app)
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(text.contains("docs/intro.md"), "got: {text}");
        assert!(text.contains("q quit"), "keys hint missing: {text}");
    }

    #[test]
    fn the_status_line_is_usable_with_no_document() {
        let app = App::new(Settings::default(), &theme::DARK);
        let text: String = status_line(&app)
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(!text.is_empty());
    }

    use crate::app::action::Action;
    use crate::app::mode::Mode;

    fn text_of(app: &App) -> String {
        status_line(app)
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect()
    }

    fn search_for(app: &mut App, query: &str) {
        app.apply(Action::SearchStart);
        for c in query.chars() {
            app.apply(Action::SearchInput(c));
        }
        app.apply(Action::SearchCommit);
    }

    #[test]
    fn the_prompt_replaces_the_status_row_while_typing() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 30);
        app.open_source(PathBuf::from("a.md"), "x\n");
        app.mode = Mode::SearchPrompt {
            query: "need".to_string(),
        };
        let text = text_of(&app);
        assert!(text.starts_with("/need"), "got: {text}");
        assert!(!text.contains("q quit"), "the prompt owns the whole row");
        assert!(!text.contains("a.md"), "the prompt owns the whole row");
    }

    #[test]
    fn the_prompt_sanitises_the_query_before_drawing_it() {
        // The query is user input like any other text this reader draws.
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 30);
        app.mode = Mode::SearchPrompt {
            query: "a\u{1b}[31mb".to_string(),
        };
        let text = text_of(&app);
        assert!(
            !text.contains('\u{1b}'),
            "escape reached the buffer: {text:?}"
        );
        assert!(text.contains("a[31mb"), "got: {text}");
    }

    #[test]
    fn an_active_search_shows_its_position() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 30);
        app.open_source(PathBuf::from("a.md"), "needle\n\nneedle\n");
        search_for(&mut app, "needle");
        let text = text_of(&app);
        assert!(text.contains("[1/2]"), "got: {text}");
        assert!(text.contains("a.md"), "got: {text}");
        assert!(text.contains("q quit"), "got: {text}");
    }

    #[test]
    fn a_search_that_matched_nothing_says_so() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 30);
        app.open_source(PathBuf::from("a.md"), "alpha\n");
        search_for(&mut app, "zzz");
        assert!(text_of(&app).contains("Pattern not found"));
    }

    #[test]
    fn no_search_means_no_indicator() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 30);
        app.open_source(PathBuf::from("a.md"), "alpha\n");
        let text = text_of(&app);
        assert!(!text.contains('['), "got: {text}");
        assert!(!text.contains("Pattern"), "got: {text}");
    }
}
