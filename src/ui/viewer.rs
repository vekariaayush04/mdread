use crate::app::state::App;
use crate::ui::highlight::{Highlight, highlight_ranges};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};

/// Centre a `content_width`-wide column inside `area`, clipping if it does
/// not fit. Prose stretched across a very wide terminal is unreadable.
pub fn centred(area: Rect, content_width: u16) -> Rect {
    let width = content_width.min(area.width);
    let left = (area.width - width) / 2;
    Rect::new(area.x + left, area.y, width, area.height)
}

/// The slice of the document currently under the viewport, or a hint or
/// error message when there is nothing to show.
pub fn visible_lines(app: &App) -> Vec<Line<'static>> {
    if let Some(message) = &app.error {
        return vec![
            Line::from(String::new()),
            Line::styled(message.clone(), Style::default().fg(app.theme.muted)),
        ];
    }
    let Some(doc) = &app.doc else {
        return vec![
            Line::from(String::new()),
            Line::styled(
                "No document open. Pass a file: mdread README.md".to_string(),
                Style::default().fg(app.theme.muted),
            ),
        ];
    };
    let start = doc.scroll.min(doc.rendered.len());
    let end = (start + app.viewport_height as usize).min(doc.rendered.len());
    let window = &doc.rendered.lines[start..end];

    let Some(search) = &app.search else {
        return window.to_vec();
    };
    let current = search.current_match();

    window
        .iter()
        .enumerate()
        .map(|(offset, line)| {
            let index = start + offset;
            let ranges: Vec<Highlight> = search
                .matches
                .iter()
                .filter(|m| m.line == index)
                .map(|m| Highlight {
                    start_col: m.start_col,
                    end_col: m.end_col,
                    style: if Some(*m) == current {
                        app.theme.search_current
                    } else {
                        app.theme.search_match
                    },
                })
                .collect();
            if ranges.is_empty() {
                line.clone()
            } else {
                highlight_ranges(line, &ranges)
            }
        })
        .collect()
}

pub fn draw_viewer(f: &mut Frame, area: Rect, app: &App) {
    let title = app
        .doc
        .as_ref()
        .map(|d| d.path.display().to_string())
        .unwrap_or_else(|| "mdread".to_string());

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border))
        .title(title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let width = app
        .doc
        .as_ref()
        .map(|d| d.content_width)
        .unwrap_or(inner.width);
    f.render_widget(Paragraph::new(visible_lines(app)), centred(inner, width));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;
    use crate::theme;
    use std::path::PathBuf;

    fn text_of(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn content_narrower_than_the_area_is_centred() {
        let r = centred(Rect::new(0, 0, 100, 10), 80);
        assert_eq!(r.x, 10);
        assert_eq!(r.width, 80);
    }

    #[test]
    fn content_as_wide_as_the_area_is_flush() {
        let r = centred(Rect::new(0, 0, 80, 10), 80);
        assert_eq!(r.x, 0);
        assert_eq!(r.width, 80);
    }

    #[test]
    fn content_wider_than_the_area_is_clipped_not_overflowed() {
        let r = centred(Rect::new(0, 0, 40, 10), 80);
        assert_eq!(r.x, 0);
        assert_eq!(r.width, 40);
    }

    #[test]
    fn centring_respects_a_non_zero_origin() {
        let r = centred(Rect::new(5, 2, 100, 10), 80);
        assert_eq!(r.x, 15);
        assert_eq!(r.y, 2);
    }

    #[test]
    fn visible_lines_returns_the_viewport_window() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 8); // viewport height 5
        app.open_source(PathBuf::from("a.md"), "a\n\nb\n\nc\n\nd\n\ne\n\nf\n");
        app.doc.as_mut().unwrap().scroll = 2;
        let lines = visible_lines(&app);
        assert_eq!(lines.len(), 5);
        assert_eq!(text_of(&lines[0]), "b");
    }

    #[test]
    fn visible_lines_near_the_end_does_not_run_past_the_document() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 30);
        app.open_source(PathBuf::from("a.md"), "a\n\nb\n");
        app.doc.as_mut().unwrap().scroll = 2;
        assert_eq!(visible_lines(&app).len(), 1);
    }

    #[test]
    fn visible_lines_with_no_document_shows_a_hint_rather_than_nothing() {
        let app = App::new(Settings::default(), &theme::DARK);
        let lines = visible_lines(&app);
        assert!(!lines.is_empty());
    }

    #[test]
    fn visible_lines_shows_the_error_when_one_is_set() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 30);
        app.error = Some("cannot read missing.md".into());
        let joined: String = visible_lines(&app).iter().map(text_of).collect();
        assert!(joined.contains("missing.md"));
    }

    use crate::app::action::Action;

    fn search_for(app: &mut App, query: &str) {
        app.apply(Action::SearchStart);
        for c in query.chars() {
            app.apply(Action::SearchInput(c));
        }
        app.apply(Action::SearchCommit);
    }

    #[test]
    fn visible_lines_highlight_every_match_on_screen() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 12);
        app.open_source(PathBuf::from("a.md"), "alpha needle beta\n\nneedle again\n");
        search_for(&mut app, "needle");

        let lines = visible_lines(&app);
        let painted: Vec<&str> = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .filter(|s| s.style.bg.is_some())
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(painted, vec!["needle", "needle"]);
    }

    #[test]
    fn the_current_match_is_styled_differently_from_the_others() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 12);
        app.open_source(PathBuf::from("a.md"), "needle one\n\nneedle two\n");
        search_for(&mut app, "needle");

        let lines = visible_lines(&app);
        let backgrounds: Vec<_> = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .filter(|s| s.content.as_ref() == "needle")
            .map(|s| s.style.bg)
            .collect();
        assert_eq!(backgrounds.len(), 2);
        assert_eq!(backgrounds[0], theme::DARK.search_current.bg);
        assert_eq!(backgrounds[1], theme::DARK.search_match.bg);
        assert_ne!(backgrounds[0], backgrounds[1]);
    }

    #[test]
    fn visible_lines_are_untouched_when_no_search_is_active() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 12);
        app.open_source(PathBuf::from("a.md"), "alpha needle beta\n");
        let lines = visible_lines(&app);
        assert!(
            lines
                .iter()
                .flat_map(|l| l.spans.iter())
                .all(|s| s.style.bg.is_none())
        );
    }

    #[test]
    fn a_match_scrolled_off_screen_is_not_drawn() {
        let mut app = App::new(Settings::default(), &theme::DARK);
        app.set_geometry(100, 8); // viewport height 5
        app.open_source(
            PathBuf::from("a.md"),
            "needle\n\nb\n\nc\n\nd\n\ne\n\nf\n\nneedle\n",
        );
        search_for(&mut app, "needle");
        app.apply(Action::Bottom);

        let painted = visible_lines(&app)
            .iter()
            .flat_map(|l| l.spans.iter())
            .filter(|s| s.style.bg.is_some())
            .count();
        assert_eq!(painted, 1, "only the match inside the viewport is painted");
    }
}
