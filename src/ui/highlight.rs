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
use unicode_width::UnicodeWidthStr;

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

/// Rebuild `line` with `ranges` painted over it. Columns are measured
/// exactly the way `crate::app::search::find_matches` measures them: as
/// `UnicodeWidthStr::width` of the prefix of the line's *concatenated*
/// visible text up to a character's byte offset, not a per-character width
/// sum. The two disagree on VS16 and ZWJ sequences (`"❤️"` is one grapheme
/// of width 2, not two chars summing to 1+1), so matching the matcher's
/// method — rather than merely using the same crate — is what keeps a
/// highlight landing on the columns `find_matches` reported. A character
/// whose own presence does not grow that prefix width (a combining mark, a
/// variation selector, a ZWJ) is folded into the same span as the
/// character before it, so one grapheme is never split across two spans
/// with different styles.
pub fn highlight_ranges(line: &Line<'static>, ranges: &[Highlight]) -> Line<'static> {
    if ranges.is_empty() {
        return line.clone();
    }

    // The full visible text of the line, concatenated exactly the way
    // `search::line_text` builds it, plus which source span each character
    // came from (spans always break at span boundaries, whatever the
    // overlay does).
    let mut full = String::new();
    let mut chars: Vec<(char, usize, usize)> = Vec::new();
    for (span_idx, span) in line.spans.iter().enumerate() {
        for ch in span.content.chars() {
            chars.push((ch, full.len(), span_idx));
            full.push(ch);
        }
    }

    let mut spans: Vec<Span<'static>> = Vec::with_capacity(line.spans.len() + ranges.len() * 2);
    let mut buf = String::new();
    let mut buf_span_idx: Option<usize> = None;
    let mut buf_overlay: Option<Style> = None;
    let mut prev_overlay: Option<Style> = None;

    for (ch, byte_offset, span_idx) in chars {
        let col = full[..byte_offset].width();
        let next_col = full[..byte_offset + ch.len_utf8()].width();
        let overlay = if next_col == col {
            // This character did not widen the line, so it rides along
            // with whatever the previous character's overlay was.
            prev_overlay
        } else {
            overlay_at(ranges, col)
        };

        if !buf.is_empty() && (Some(span_idx) != buf_span_idx || overlay != buf_overlay) {
            let style = line.spans[buf_span_idx.expect("buf non-empty implies a span")].style;
            spans.push(fragment(std::mem::take(&mut buf), style, buf_overlay));
        }
        buf_span_idx = Some(span_idx);
        buf_overlay = overlay;
        buf.push(ch);
        prev_overlay = overlay;
    }
    if !buf.is_empty() {
        let style = line.spans[buf_span_idx.expect("buf non-empty implies a span")].style;
        spans.push(fragment(buf, style, buf_overlay));
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
    fn emoji_with_variation_selector_does_not_shift_later_columns() {
        // "❤️" (heart + VS16) is one grapheme with display width 2; the
        // matcher measures it with `UnicodeWidthStr::width`, not a per-char
        // sum, so the highlighter must agree on where column 3 falls.
        let line = Line::from("\u{2764}\u{fe0f} foo".to_string());
        let out = highlight_ranges(&line, &[hl(3, 6)]);
        assert_eq!(texts(&out), vec!["\u{2764}\u{fe0f} ", "foo"]);
        assert_eq!(out.spans[1].style.bg, Some(Color::Yellow));
    }

    #[test]
    fn zwj_sequence_is_measured_as_one_grapheme() {
        // A ZWJ family emoji is three code points glued into one grapheme
        // of display width 2, not the naive per-char sum of 6.
        let line = Line::from("\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467} x".to_string());
        let out = highlight_ranges(&line, &[hl(3, 4)]);
        assert_eq!(
            texts(&out),
            vec!["\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467} ", "x"]
        );
    }

    #[test]
    fn a_combining_mark_stays_with_its_base_char() {
        let line = Line::from("e\u{301}a".to_string());
        let out = highlight_ranges(&line, &[hl(0, 1)]);
        assert_eq!(texts(&out), vec!["e\u{301}", "a"]);
        assert_eq!(out.spans[0].style.bg, Some(Color::Yellow));
        assert_eq!(out.spans[1].style.bg, None);
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
