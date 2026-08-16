use crate::app::state::App;
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

pub fn status_line(app: &App) -> Line<'static> {
    let path = app
        .doc
        .as_ref()
        .map(|d| d.path.display().to_string())
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
}
