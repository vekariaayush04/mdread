use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

/// One row of the help overlay: the human-readable key label(s) and what
/// they do. The label groups every raw key that `map_key` sends to the same
/// action (e.g. `j` and `Down` both scroll one line), so the overlay reads
/// as one row per action rather than one row per key.
pub struct KeyHelp {
    pub keys: &'static str,
    pub description: &'static str,
}

/// The keymap shown in the help overlay, in display order. A test in this
/// module feeds each label's underlying keys through `map_key` and asserts
/// the action matches, so this list cannot silently drift from the real
/// bindings in `action.rs`.
pub fn keymap() -> Vec<KeyHelp> {
    vec![
        KeyHelp {
            keys: "j / ↓",
            description: "down a line",
        },
        KeyHelp {
            keys: "k / ↑",
            description: "up a line",
        },
        KeyHelp {
            keys: "d",
            description: "half page down",
        },
        KeyHelp {
            keys: "u",
            description: "half page up",
        },
        KeyHelp {
            keys: "Space / PgDn",
            description: "page down",
        },
        KeyHelp {
            keys: "PgUp",
            description: "page up",
        },
        KeyHelp {
            keys: "g / Home",
            description: "top",
        },
        KeyHelp {
            keys: "G / End",
            description: "bottom",
        },
        KeyHelp {
            keys: "/",
            description: "search",
        },
        KeyHelp {
            keys: "n / N",
            description: "next / previous match",
        },
        KeyHelp {
            keys: "Esc",
            description: "close this, or clear search",
        },
        KeyHelp {
            keys: "r",
            description: "reload from disk",
        },
        KeyHelp {
            keys: "?",
            description: "this help",
        },
        KeyHelp {
            keys: "q / Ctrl-C",
            description: "quit",
        },
    ]
}

/// The overlay's content as styled lines: one row per binding, then a line
/// explaining how to close it. No heading line — the block's border title
/// already says "Help", and the viewer never repeats its own title in its
/// body either. Pure — no ratatui `Frame` involved — so it is covered by
/// ordinary unit tests, with the drawing itself left to a frame snapshot.
///
/// Every line carries one column of leading padding so its text never sits
/// flush against the left border; `draw_help` reserves a matching column on
/// the right. Full-width content abutting a border reads as part of the
/// frame rather than as content — the same reason the viewer pads its text.
pub fn help_lines(theme: &Theme) -> Vec<Line<'static>> {
    let rows = keymap();
    let key_width = rows
        .iter()
        .map(|r| r.keys.chars().count())
        .max()
        .unwrap_or(0);

    let mut lines = Vec::new();
    for row in &rows {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<width$}", row.keys, width = key_width),
                Style::default().fg(theme.accent),
            ),
            Span::styled("  ", Style::default().fg(theme.text)),
            Span::styled(row.description, Style::default().fg(theme.text)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::styled(
        "Esc or ? to close",
        Style::default().fg(theme.muted),
    ));

    lines
        .into_iter()
        .map(|line| {
            let mut spans = vec![Span::raw(" ")];
            spans.extend(line.spans);
            Line::from(spans)
        })
        .collect()
}

/// Fit a box `content_width` x `content_height` (excluding borders), centred
/// inside `area`, clamped so it never exceeds the available space. Mirrors
/// `viewer::centred`, but for both dimensions and with room for a border.
fn overlay_rect(area: Rect, content_width: u16, content_height: u16) -> Rect {
    let width = content_width.saturating_add(2).min(area.width);
    let height = content_height.saturating_add(2).min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}

/// Draw the help overlay, centred over `area`. Degrades sanely when the
/// terminal is too small to fit it: the box clamps to whatever space is
/// available and simply omits content rather than drawing outside its rect
/// or panicking.
pub fn draw_help(f: &mut Frame, area: Rect, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let lines = help_lines(theme);
    // `lines` already carries one column of left padding; add the matching
    // column on the right so the longest row doesn't sit flush against it.
    let content_width = lines
        .iter()
        .map(|l| l.width() as u16)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let content_height = lines.len() as u16;

    let rect = overlay_rect(area, content_width, content_height);
    if rect.width == 0 || rect.height == 0 {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(Span::styled(" Help ", Style::default().fg(theme.accent)));
    let inner = block.inner(rect);

    f.render_widget(Clear, rect);
    f.render_widget(block, rect);
    if inner.width > 0 && inner.height > 0 {
        f.render_widget(Paragraph::new(lines), inner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::action::{Action, map_key};
    use crate::theme;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// Every row in the overlay must describe a binding `map_key` actually
    /// implements. This is what keeps the help text from drifting away from
    /// reality as `action.rs` changes.
    #[test]
    fn keymap_matches_actual_bindings() {
        assert_eq!(map_key(key(KeyCode::Char('j'))), Action::ScrollLines(1));
        assert_eq!(map_key(key(KeyCode::Down)), Action::ScrollLines(1));
        assert_eq!(map_key(key(KeyCode::Char('k'))), Action::ScrollLines(-1));
        assert_eq!(map_key(key(KeyCode::Up)), Action::ScrollLines(-1));
        assert_eq!(map_key(key(KeyCode::Char('d'))), Action::ScrollHalfPage(1));
        assert_eq!(map_key(key(KeyCode::Char('u'))), Action::ScrollHalfPage(-1));
        assert_eq!(map_key(key(KeyCode::Char(' '))), Action::ScrollPage(1));
        assert_eq!(map_key(key(KeyCode::PageDown)), Action::ScrollPage(1));
        assert_eq!(map_key(key(KeyCode::PageUp)), Action::ScrollPage(-1));
        assert_eq!(map_key(key(KeyCode::Char('g'))), Action::Top);
        assert_eq!(map_key(key(KeyCode::Home)), Action::Top);
        assert_eq!(map_key(key(KeyCode::Char('G'))), Action::Bottom);
        assert_eq!(map_key(key(KeyCode::End)), Action::Bottom);
        assert_eq!(map_key(key(KeyCode::Char('r'))), Action::Reload);
        assert_eq!(map_key(key(KeyCode::Char('?'))), Action::Help);
        assert_eq!(map_key(key(KeyCode::Char('q'))), Action::Quit);
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::Quit
        );
        assert_eq!(map_key(key(KeyCode::Char('/'))), Action::SearchStart);
        assert_eq!(map_key(key(KeyCode::Char('n'))), Action::NextMatch);
        assert_eq!(map_key(key(KeyCode::Char('N'))), Action::PrevMatch);
        assert_eq!(map_key(key(KeyCode::Esc)), Action::Dismiss);

        // One row per action shown, in the order listed above.
        assert_eq!(keymap().len(), 14);
    }

    #[test]
    fn help_lines_include_every_binding_and_the_close_hint() {
        let lines = help_lines(&theme::DARK);
        let joined: String = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        for row in keymap() {
            assert!(joined.contains(row.keys), "missing key label: {}", row.keys);
            assert!(
                joined.contains(row.description),
                "missing description: {}",
                row.description
            );
        }
        assert!(joined.to_lowercase().contains("esc"));
    }

    #[test]
    fn help_lines_have_no_redundant_heading_and_are_left_padded() {
        let lines = help_lines(&theme::DARK);
        // The border title already reads "Help"; the body must not repeat it.
        let first_line: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_ne!(first_line.trim(), "Help");

        // Every line — including the blank spacer — starts with one column
        // of padding, so text never sits flush against the left border.
        for line in &lines {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(text.starts_with(' '), "line is not left-padded: {text:?}");
        }
    }

    #[test]
    fn overlay_rect_is_centred_and_fits_inside_the_area() {
        let area = Rect::new(0, 0, 100, 40);
        let r = overlay_rect(area, 30, 10);
        assert_eq!(r.width, 32);
        assert_eq!(r.height, 12);
        assert!(r.x + r.width <= area.width);
        assert!(r.y + r.height <= area.height);
    }

    #[test]
    fn overlay_rect_clamps_to_a_small_area_without_overflowing() {
        let area = Rect::new(0, 0, 10, 5);
        let r = overlay_rect(area, 80, 40);
        assert!(r.width <= area.width);
        assert!(r.height <= area.height);
        assert!(r.x + r.width <= area.x + area.width);
        assert!(r.y + r.height <= area.y + area.height);
    }

    #[test]
    fn drawing_the_overlay_on_tiny_terminals_does_not_panic() {
        for (w, h) in [(20u16, 8u16), (10, 5), (5, 3), (1, 1), (0, 0)] {
            let mut terminal = Terminal::new(TestBackend::new(w.max(1), h.max(1))).unwrap();
            terminal
                .draw(|f| draw_help(f, f.area(), &theme::DARK))
                .unwrap();
        }
    }
}
