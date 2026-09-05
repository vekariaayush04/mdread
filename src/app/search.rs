//! In-document search over *rendered* lines.
//!
//! Matching deliberately runs on what is on screen — the concatenated text
//! of a rendered line's spans, after wrapping and after `src/render/` has
//! already sanitised it — so a match is always something the reader can
//! see, and so the column offsets agree with the cells the highlighter has
//! to paint. Nothing here touches `src/render/`; it only consumes its
//! output.

use ratatui::text::Line;
use unicode_width::UnicodeWidthStr;

/// One match: which rendered line, and the half-open column range it covers
/// in display columns from the start of that line's visible text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    pub line: usize,
    pub start_col: usize,
    pub end_col: usize,
}

/// The visible text of a rendered line: every span's content, concatenated.
pub fn line_text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

/// Lowercase `s`, keeping a map from every *byte* of the lowercased string
/// back to the byte offset of the source character it came from.
///
/// `str::to_lowercase` is not length-preserving — `İ` (U+0130) lowercases
/// to two characters — so an offset found in a naively lowercased haystack
/// does not index the original string. The map fixes that. The final entry
/// is `s.len()`, so an offset one past the end always resolves.
fn fold(s: &str) -> (String, Vec<usize>) {
    let mut lowered = String::with_capacity(s.len());
    let mut origin = Vec::with_capacity(s.len() + 1);
    for (i, ch) in s.char_indices() {
        for lc in ch.to_lowercase() {
            for _ in 0..lc.len_utf8() {
                origin.push(i);
            }
            lowered.push(lc);
        }
    }
    origin.push(s.len());
    (lowered, origin)
}

/// Every case-insensitive occurrence of `query` in `lines`, in document
/// order. Matches never overlap: the scan resumes at the end of the
/// previous hit, so "aaaa" contains two "aa"s, the way `less` counts them.
pub fn find_matches(lines: &[Line<'_>], query: &str) -> Vec<Match> {
    let (needle, _) = fold(query);
    if needle.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let text = line_text(line);
        let (hay, origin) = fold(&text);
        let mut from = 0;
        while let Some(rel) = hay[from..].find(&needle) {
            let lo = from + rel;
            let hi = lo + needle.len();
            let start = origin[lo];
            let mut end = origin[hi];
            if end <= start {
                // The hit ended inside a case expansion (query "i" against
                // "İ"). Cover the whole source character rather than report
                // an empty range the highlighter could not paint.
                end = start + text[start..].chars().next().map_or(0, char::len_utf8);
            }
            out.push(Match {
                line: index,
                start_col: text[..start].width(),
                end_col: text[..end].width(),
            });
            from = hi;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Modifier, Style};
    use ratatui::text::Span;

    fn lines(texts: &[&str]) -> Vec<Line<'static>> {
        texts.iter().map(|t| Line::from(t.to_string())).collect()
    }

    #[test]
    fn line_text_concatenates_every_span() {
        let line = Line::from(vec![
            Span::raw("he"),
            Span::styled("llo", Style::default().add_modifier(Modifier::BOLD)),
        ]);
        assert_eq!(line_text(&line), "hello");
    }

    #[test]
    fn finds_a_match_and_reports_its_columns() {
        let m = find_matches(&lines(&["the quick brown fox"]), "quick");
        assert_eq!(
            m,
            vec![Match {
                line: 0,
                start_col: 4,
                end_col: 9
            }]
        );
    }

    #[test]
    fn matching_ignores_case_in_both_directions() {
        let m = find_matches(&lines(&["Hello WORLD"]), "hello world");
        assert_eq!(m.len(), 1);
        let m = find_matches(&lines(&["hello world"]), "HELLO");
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn reports_the_rendered_line_index() {
        let m = find_matches(&lines(&["nope", "nope", "here"]), "here");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].line, 2);
    }

    #[test]
    fn finds_several_matches_on_one_line_in_order() {
        let m = find_matches(&lines(&["ab ab ab"]), "ab");
        assert_eq!(m.len(), 3);
        assert_eq!(m[0].start_col, 0);
        assert_eq!(m[1].start_col, 3);
        assert_eq!(m[2].start_col, 6);
    }

    #[test]
    fn matches_never_overlap() {
        // "aaaa" contains three overlapping "aa"s, but a reader counting
        // hits — and `less` and `vim` — sees two.
        let m = find_matches(&lines(&["aaaa"]), "aa");
        assert_eq!(m.len(), 2);
        assert_eq!((m[0].start_col, m[0].end_col), (0, 2));
        assert_eq!((m[1].start_col, m[1].end_col), (2, 4));
    }

    #[test]
    fn matches_across_several_styled_spans() {
        // A word that is half bold must still match: the wrapper decides
        // where spans break, and the reader sees one word.
        let line = Line::from(vec![
            Span::raw("he"),
            Span::styled("llo", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" there"),
        ]);
        let m = find_matches(&[line], "hello");
        assert_eq!(
            m,
            vec![Match {
                line: 0,
                start_col: 0,
                end_col: 5
            }]
        );
    }

    #[test]
    fn columns_are_display_columns_not_byte_or_char_counts() {
        // Two double-width characters occupy four columns, so the match
        // starts at column 4 even though it is character 2 and byte 6.
        let m = find_matches(&lines(&["漢字xy"]), "xy");
        assert_eq!(
            m,
            vec![Match {
                line: 0,
                start_col: 4,
                end_col: 6
            }]
        );
    }

    #[test]
    fn an_empty_query_matches_nothing() {
        assert!(find_matches(&lines(&["anything"]), "").is_empty());
    }

    #[test]
    fn a_query_that_is_not_there_matches_nothing() {
        assert!(find_matches(&lines(&["alpha", "beta"]), "gamma").is_empty());
    }

    #[test]
    fn a_multi_character_case_expansion_does_not_panic_or_invert_a_range() {
        // U+0130 lowercases to two chars, so a naive `to_lowercase()` would
        // put offsets out of step with the original text.
        for query in ["i", "istanbul", "İ"] {
            for m in find_matches(&lines(&["İstanbul"]), query) {
                assert!(m.start_col < m.end_col, "empty range for {query:?}");
            }
        }
    }

    #[test]
    fn no_match_column_runs_past_the_end_of_its_line() {
        let source = ["short", "a longer line with words", "漢字 mixed ascii"];
        for m in find_matches(&lines(&source), "a") {
            let width = line_text(&lines(&source)[m.line]).width();
            assert!(m.end_col <= width, "{m:?} runs past a {width}-column line");
        }
    }
}
