use crate::render::sanitize::strip_controls;
use crate::theme::Theme;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const TAB_WIDTH: usize = 4;

fn syntax_set() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    // The "newlines" variant is required: highlight_line expects a trailing \n.
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme_set() -> &'static ThemeSet {
    static SET: OnceLock<ThemeSet> = OnceLock::new();
    SET.get_or_init(ThemeSet::load_defaults)
}

fn expand_tabs(s: &str) -> String {
    s.replace('\t', &" ".repeat(TAB_WIDTH))
}

fn syntect_style(style: syntect::highlighting::Style, theme: &Theme) -> Style {
    let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
    let mut out = Style::default().fg(fg).bg(theme.code_bg);
    if style.font_style.contains(FontStyle::BOLD) {
        out = out.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        out = out.add_modifier(Modifier::ITALIC);
    }
    out
}

/// One entry per source line, each a run of styled fragments.
fn highlight(lang: Option<&str>, text: &str, theme: &Theme) -> Vec<Vec<(Style, String)>> {
    // A fenced code block is document text like any other, and an obvious
    // place to hide an escape sequence (syntax highlighting is a different
    // code path than `render::inline`, so it needs its own sanitisation).
    // `\n` is kept because `.lines()` below depends on it; `\t` is kept
    // because `expand_tabs` below converts it to spaces, so it must survive
    // this pass to be expanded rather than silently vanish.
    let text = strip_controls(text, &['\n', '\t']);
    let text = text.as_str();
    let plain_style = Style::default().fg(theme.code_fg).bg(theme.code_bg);
    let plain = |text: &str| -> Vec<Vec<(Style, String)>> {
        text.lines()
            .map(|l| vec![(plain_style, expand_tabs(l))])
            .collect()
    };

    let set = syntax_set();
    let Some(syntax) = lang.and_then(|l| set.find_syntax_by_token(l)) else {
        return plain(text);
    };
    let Some(sy_theme) = theme_set().themes.get(theme.syntect_theme) else {
        return plain(text);
    };

    let mut hl = HighlightLines::new(syntax, sy_theme);
    text.lines()
        .map(|line| {
            let with_newline = format!("{line}\n");
            match hl.highlight_line(&with_newline, set) {
                Ok(ranges) => ranges
                    .into_iter()
                    .map(|(st, frag)| {
                        (
                            syntect_style(st, theme),
                            expand_tabs(frag.trim_end_matches('\n')),
                        )
                    })
                    .filter(|(_, frag)| !frag.is_empty())
                    .collect(),
                // A highlighter error must degrade, never abort the render.
                Err(_) => vec![(plain_style, expand_tabs(line))],
            }
        })
        .collect()
}

/// Break one highlighted source line into visual rows no wider than `inner`.
/// Code is hard-split, never word-wrapped — breaking on spaces would be a lie
/// about the source.
fn split_to_rows(fragments: &[(Style, String)], inner: usize) -> Vec<Vec<Span<'static>>> {
    let mut rows: Vec<Vec<Span<'static>>> = Vec::new();
    let mut row: Vec<Span<'static>> = Vec::new();
    let mut row_w = 0usize;
    let mut buf = String::new();

    for (style, frag) in fragments {
        for ch in frag.chars() {
            let cw = ch.width().unwrap_or(0);
            if row_w + cw > inner {
                if !buf.is_empty() {
                    row.push(Span::styled(std::mem::take(&mut buf), *style));
                }
                rows.push(std::mem::take(&mut row));
                row_w = 0;
            }
            buf.push(ch);
            row_w += cw;
        }
        if !buf.is_empty() {
            row.push(Span::styled(std::mem::take(&mut buf), *style));
        }
    }
    if !row.is_empty() {
        rows.push(row);
    }
    if rows.is_empty() {
        rows.push(Vec::new());
    }
    rows
}

pub fn layout_code(
    lang: Option<&str>,
    text: &str,
    width: u16,
    theme: &Theme,
) -> Vec<Line<'static>> {
    // "│ " + content + " │" costs four columns.
    let width = width.max(5) as usize;
    let inner = width - 4;
    let border = Style::default().fg(theme.border);
    let pad = Style::default().bg(theme.code_bg);

    let mut out = Vec::new();
    out.push(Line::from(Span::styled(
        format!("╭{}╮", "─".repeat(width - 2)),
        border,
    )));

    for fragments in highlight(lang, text, theme) {
        for row in split_to_rows(&fragments, inner) {
            let used: usize = row.iter().map(|s| s.content.width()).sum();
            let mut spans = vec![
                Span::styled("│".to_string(), border),
                Span::styled(" ".to_string(), pad),
            ];
            spans.extend(row);
            spans.push(Span::styled(" ".repeat(inner - used + 1), pad));
            spans.push(Span::styled("│".to_string(), border));
            out.push(Line::from(spans));
        }
    }

    // An empty block still gets one blank content row so the box has a body.
    if out.len() == 1 {
        out.push(Line::from(vec![
            Span::styled("│".to_string(), border),
            Span::styled(" ".repeat(width - 2), pad),
            Span::styled("│".to_string(), border),
        ]));
    }

    out.push(Line::from(Span::styled(
        format!("╰{}╯", "─".repeat(width - 2)),
        border,
    )));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::DARK;

    fn text_of(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn texts(lines: &[Line<'_>]) -> Vec<String> {
        lines.iter().map(text_of).collect()
    }

    #[test]
    fn draws_a_rounded_box_around_the_code() {
        let lines = layout_code(None, "abc\n", 11, &DARK);
        assert_eq!(
            texts(&lines),
            vec!["╭─────────╮", "│ abc     │", "╰─────────╯"]
        );
    }

    #[test]
    fn every_line_is_exactly_the_requested_width() {
        let lines = layout_code(Some("rust"), "fn a() {}\nlet x = 1;\n", 30, &DARK);
        for line in &lines {
            assert_eq!(text_of(line).width(), 30, "ragged line: {line:?}");
        }
    }

    #[test]
    fn an_unknown_language_falls_back_to_plain_text_without_panicking() {
        let lines = layout_code(Some("not-a-language"), "hello\n", 20, &DARK);
        assert!(text_of(&lines[1]).contains("hello"));
    }

    #[test]
    fn no_language_renders_plain_text() {
        let lines = layout_code(None, "hello\n", 20, &DARK);
        assert!(text_of(&lines[1]).contains("hello"));
    }

    #[test]
    fn a_known_language_produces_more_than_one_colour() {
        // Proves highlighting actually ran rather than silently falling back.
        let lines = layout_code(Some("rust"), "fn main() {}\n", 40, &DARK);
        let colours: std::collections::HashSet<Option<Color>> =
            lines[1].spans.iter().map(|s| s.style.fg).collect();
        assert!(colours.len() > 1, "expected highlighting, got {colours:?}");
    }

    #[test]
    fn long_code_lines_wrap_inside_the_box_instead_of_overflowing() {
        let lines = layout_code(None, "abcdefghijklmnop\n", 12, &DARK);
        for line in &lines {
            assert_eq!(text_of(line).width(), 12);
        }
        // top + two content lines + bottom
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn tabs_are_expanded_so_the_box_stays_square() {
        let lines = layout_code(None, "\tx\n", 20, &DARK);
        assert_eq!(text_of(&lines[1]), "│     x            │");
        for line in &lines {
            assert_eq!(text_of(line).width(), 20);
        }
    }

    #[test]
    fn an_empty_code_block_still_renders_a_box() {
        let lines = layout_code(None, "", 10, &DARK);
        assert_eq!(lines.len(), 3);
        for line in &lines {
            assert_eq!(text_of(line).width(), 10);
        }
    }

    #[test]
    fn a_very_narrow_measure_does_not_panic() {
        for w in 0..6u16 {
            let lines = layout_code(None, "xyz\n", w, &DARK);
            assert!(!lines.is_empty());
        }
    }

    #[test]
    fn code_uses_the_theme_background() {
        let lines = layout_code(None, "x\n", 20, &DARK);
        let has_bg = lines[1]
            .spans
            .iter()
            .any(|s| s.style.bg == Some(DARK.code_bg));
        assert!(
            has_bg,
            "code content should sit on the theme's code background"
        );
    }
}
