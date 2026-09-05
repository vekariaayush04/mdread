//! Painting a search match over already-styled text.
//!
//! A rendered line is a sequence of spans, each with the style the renderer
//! chose for it — bold, a code background, a link colour. A match is a
//! column range that can begin and end anywhere, including inside a span.
//! Rather than rebuild the line, this splits spans at the range boundaries
//! and `patch`es the highlight on top of whatever style was already there,
//! so a half-bold word stays half-bold under the highlight.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;

/// One stretch of a line to paint over: `[start_col, end_col)` in display
/// columns from the start of the line's visible text, and the style to lay
/// on top of what is already there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Highlight {
    pub start_col: usize,
    pub end_col: usize,
    pub style: Style,
}

fn overlay_at(ranges: &[Highlight], col: usize) -> Option<Style> {
    ranges
        .iter()
        .find(|r| col >= r.start_col && col < r.end_col)
        .map(|r| r.style)
}

fn fragment(text: String, base: Style, overlay: Option<Style>) -> Span<'static> {
    match overlay {
        Some(style) => Span::styled(text, base.patch(style)),
        None => Span::styled(text, base),
    }
}

/// Rebuild `line` with `ranges` painted over it. Columns are display
/// columns, measured the same way the wrapper measures them, so the
/// highlight lands on exactly the cells the reader sees.
pub fn highlight_ranges(line: &Line<'static>, ranges: &[Highlight]) -> Line<'static> {
    if ranges.is_empty() {
        return line.clone();
    }

    let mut spans: Vec<Span<'static>> = Vec::with_capacity(line.spans.len() + ranges.len() * 2);
    let mut col = 0usize;
    for span in &line.spans {
        let mut buf = String::new();
        let mut buf_overlay: Option<Style> = None;
        for ch in span.content.chars() {
            let overlay = overlay_at(ranges, col);
            if !buf.is_empty() && overlay != buf_overlay {
                spans.push(fragment(std::mem::take(&mut buf), span.style, buf_overlay));
            }
            buf_overlay = overlay;
            buf.push(ch);
            col += ch.width().unwrap_or(0);
        }
        if !buf.is_empty() {
            spans.push(fragment(buf, span.style, buf_overlay));
        }
    }

    let mut out = line.clone();
    out.spans = spans;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Modifier};

    fn hl(start_col: usize, end_col: usize) -> Highlight {
        Highlight {
            start_col,
            end_col,
            style: Style::default().bg(Color::Yellow),
        }
    }

    fn texts(line: &Line<'_>) -> Vec<String> {
        line.spans.iter().map(|s| s.content.to_string()).collect()
    }

    fn joined(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn no_ranges_returns_the_line_unchanged() {
        let line = Line::from("hello world".to_string());
        let out = highlight_ranges(&line, &[]);
        assert_eq!(texts(&out), texts(&line));
    }

    #[test]
    fn a_range_inside_one_span_splits_it_into_three() {
        let line = Line::from("hello world".to_string());
        let out = highlight_ranges(&line, &[hl(6, 11)]);
        assert_eq!(texts(&out), vec!["hello ", "world"]);
        assert_eq!(out.spans[1].style.bg, Some(Color::Yellow));
        assert_eq!(out.spans[0].style.bg, None);
    }

    #[test]
    fn a_range_in_the_middle_leaves_a_tail_span() {
        let line = Line::from("abcdef".to_string());
        let out = highlight_ranges(&line, &[hl(2, 4)]);
        assert_eq!(texts(&out), vec!["ab", "cd", "ef"]);
        assert_eq!(out.spans[1].style.bg, Some(Color::Yellow));
    }

    #[test]
    fn the_underlying_style_survives_the_overlay() {
        let bold = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
        let line = Line::from(vec![Span::styled("bold text", bold)]);
        let out = highlight_ranges(&line, &[hl(0, 4)]);
        assert_eq!(out.spans[0].style.fg, Some(Color::Red));
        assert!(out.spans[0].style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(out.spans[0].style.bg, Some(Color::Yellow));
        // ...and the untouched remainder is still plain bold red.
        assert_eq!(out.spans[1].style.bg, None);
    }

    #[test]
    fn a_range_spanning_two_spans_highlights_both_halves() {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let line = Line::from(vec![Span::raw("he"), Span::styled("llo!", bold)]);
        let out = highlight_ranges(&line, &[hl(0, 5)]);
        assert_eq!(texts(&out), vec!["he", "llo", "!"]);
        assert_eq!(out.spans[0].style.bg, Some(Color::Yellow));
        assert_eq!(out.spans[1].style.bg, Some(Color::Yellow));
        assert!(out.spans[1].style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(out.spans[2].style.bg, None);
    }

    #[test]
    fn two_ranges_on_one_line_keep_their_own_styles() {
        let line = Line::from("ab cd ef".to_string());
        let ranges = [
            Highlight {
                start_col: 0,
                end_col: 2,
                style: Style::default().bg(Color::Yellow),
            },
            Highlight {
                start_col: 6,
                end_col: 8,
                style: Style::default().bg(Color::Blue),
            },
        ];
        let out = highlight_ranges(&line, &ranges);
        assert_eq!(texts(&out), vec!["ab", " cd ", "ef"]);
        assert_eq!(out.spans[0].style.bg, Some(Color::Yellow));
        assert_eq!(out.spans[2].style.bg, Some(Color::Blue));
    }

    #[test]
    fn adjacent_ranges_with_different_styles_do_not_merge() {
        let line = Line::from("abcd".to_string());
        let ranges = [
            Highlight {
                start_col: 0,
                end_col: 2,
                style: Style::default().bg(Color::Yellow),
            },
            Highlight {
                start_col: 2,
                end_col: 4,
                style: Style::default().bg(Color::Blue),
            },
        ];
        let out = highlight_ranges(&line, &ranges);
        assert_eq!(texts(&out), vec!["ab", "cd"]);
    }

    #[test]
    fn wide_characters_are_measured_in_columns() {
        // 漢字 is four columns; the range 4..6 is exactly "ab".
        let line = Line::from("漢字ab".to_string());
        let out = highlight_ranges(&line, &[hl(4, 6)]);
        assert_eq!(texts(&out), vec!["漢字", "ab"]);
        assert_eq!(out.spans[1].style.bg, Some(Color::Yellow));
    }

    #[test]
    fn a_range_past_the_end_of_the_line_is_harmless() {
        let line = Line::from("short".to_string());
        let out = highlight_ranges(&line, &[hl(100, 120)]);
        assert_eq!(joined(&out), "short");
    }

    #[test]
    fn the_visible_text_is_never_changed() {
        let line = Line::from(vec![
            Span::raw("alpha "),
            Span::raw("beta "),
            Span::raw("gamma"),
        ]);
        for range in [hl(0, 1), hl(3, 9), hl(5, 16)] {
            assert_eq!(
                joined(&highlight_ranges(&line, &[range])),
                "alpha beta gamma"
            );
        }
    }
}
