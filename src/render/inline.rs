use crate::doc::ir::Inline;
use crate::render::sanitize::strip_controls;
use crate::theme::Theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Word,
    Space,
    HardBreak,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub text: String,
    pub style: Style,
    pub kind: TokenKind,
}

pub fn tokenize(inlines: &[Inline], theme: &Theme, base: Style) -> Vec<Token> {
    let mut out = Vec::new();
    push_inlines(inlines, theme, base, &mut out);
    out
}

fn push_inlines(inlines: &[Inline], theme: &Theme, style: Style, out: &mut Vec<Token>) {
    for inline in inlines {
        match inline {
            Inline::Text(t) => push_text(t, style, out),
            // One token, so wrapping never breaks inside inline code. Code
            // span text never legitimately contains a control character
            // (CommonMark normalises line endings inside code spans to
            // spaces before this ever sees the text), so no exceptions.
            Inline::Code(t) => out.push(Token {
                text: strip_controls(t, &[]),
                style: Style::default().fg(theme.code_fg).bg(theme.code_bg),
                kind: TokenKind::Word,
            }),
            Inline::Emph(c) => push_inlines(c, theme, style.add_modifier(Modifier::ITALIC), out),
            Inline::Strong(c) => push_inlines(c, theme, style.add_modifier(Modifier::BOLD), out),
            Inline::Strike(c) => {
                push_inlines(c, theme, style.add_modifier(Modifier::CROSSED_OUT), out)
            }
            Inline::Link { content, .. } => push_inlines(
                content,
                theme,
                style.fg(theme.link).add_modifier(Modifier::UNDERLINED),
                out,
            ),
            Inline::Image { alt, .. } => push_text(
                &format!("[image: {}]", strip_controls(alt, &[])),
                Style::default().fg(theme.muted),
                out,
            ),
            Inline::SoftBreak => out.push(Token {
                text: " ".into(),
                style,
                kind: TokenKind::Space,
            }),
            Inline::HardBreak => out.push(Token {
                text: String::new(),
                style,
                kind: TokenKind::HardBreak,
            }),
        }
    }
}

fn push_text(text: &str, style: Style, out: &mut Vec<Token>) {
    let mut buf = String::new();
    for ch in text.chars() {
        if ch.is_whitespace() {
            // Every whitespace control character (tab, newline, CR, NEL,
            // ...) is normalised to a single space here, so it never reaches
            // the `else` branch below and never needs separate stripping.
            if !buf.is_empty() {
                out.push(Token {
                    text: std::mem::take(&mut buf),
                    style,
                    kind: TokenKind::Word,
                });
            }
            out.push(Token {
                text: " ".into(),
                style,
                kind: TokenKind::Space,
            });
        } else if !ch.is_control() {
            // Non-whitespace control characters (ESC, BEL, BS, DEL, C1...)
            // are exactly the ones that can start a terminal escape
            // sequence. Drop them; see `render::sanitize` for the rationale
            // on removal over a visible placeholder.
            buf.push(ch);
        }
    }
    if !buf.is_empty() {
        out.push(Token {
            text: buf,
            style,
            kind: TokenKind::Word,
        });
    }
}

/// Greedy word wrap. Runs of spaces collapse to one; a word wider than the
/// whole measure is hard-split rather than allowed to overflow.
pub fn wrap(tokens: &[Token], width: u16) -> Vec<Line<'static>> {
    let width = (width.max(1)) as usize;
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut cur: Vec<Span<'static>> = Vec::new();
    let mut cur_w = 0usize;
    // A space is only emitted once we know a word follows it on this line.
    let mut pending_space: Option<Style> = None;

    for token in tokens {
        match token.kind {
            TokenKind::HardBreak => {
                lines.push(Line::from(std::mem::take(&mut cur)));
                cur_w = 0;
                pending_space = None;
            }
            TokenKind::Space => {
                if cur_w > 0 {
                    pending_space = Some(token.style);
                }
            }
            TokenKind::Word => {
                let w = token.text.width();
                let sep = usize::from(pending_space.is_some());
                if cur_w > 0 && cur_w + sep + w > width {
                    lines.push(Line::from(std::mem::take(&mut cur)));
                    cur_w = 0;
                    pending_space = None;
                }
                if let Some(style) = pending_space.take() {
                    cur.push(Span::styled(" ".to_string(), style));
                    cur_w += 1;
                }
                if w > width {
                    for chunk in hard_split(&token.text, width) {
                        let cw = chunk.width();
                        if cur_w > 0 && cur_w + cw > width {
                            lines.push(Line::from(std::mem::take(&mut cur)));
                            cur_w = 0;
                        }
                        cur.push(Span::styled(chunk, token.style));
                        cur_w += cw;
                    }
                } else {
                    cur.push(Span::styled(token.text.clone(), token.style));
                    cur_w += w;
                }
            }
        }
    }

    if !cur.is_empty() {
        lines.push(Line::from(cur));
    }
    if lines.is_empty() {
        lines.push(Line::from(String::new()));
    }
    lines
}

/// Split a single over-long word into chunks that each fit the measure,
/// respecting double-width characters.
fn hard_split(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut w = 0usize;
    for ch in text.chars() {
        let cw = ch.width().unwrap_or(0);
        if w + cw > width && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
            w = 0;
        }
        cur.push(ch);
        w += cw;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn render_inlines(
    inlines: &[Inline],
    width: u16,
    theme: &Theme,
    base: Style,
) -> Vec<Line<'static>> {
    wrap(&tokenize(inlines, theme, base), width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::DARK;

    /// Flatten a rendered line back to plain text for assertions.
    fn text_of(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn texts(lines: &[Line<'_>]) -> Vec<String> {
        lines.iter().map(text_of).collect()
    }

    fn render(src_inlines: &[Inline], width: u16) -> Vec<Line<'static>> {
        render_inlines(src_inlines, width, &DARK, Style::default())
    }

    #[test]
    fn wraps_on_word_boundaries() {
        let inlines = vec![Inline::Text("the quick brown fox".into())];
        assert_eq!(texts(&render(&inlines, 10)), vec!["the quick", "brown fox"]);
    }

    #[test]
    fn a_line_exactly_the_width_does_not_wrap() {
        let inlines = vec![Inline::Text("abcde fghij".into())];
        assert_eq!(texts(&render(&inlines, 11)), vec!["abcde fghij"]);
    }

    #[test]
    fn collapses_runs_of_whitespace() {
        let inlines = vec![Inline::Text("a    b".into())];
        assert_eq!(texts(&render(&inlines, 20)), vec!["a b"]);
    }

    #[test]
    fn soft_breaks_become_spaces() {
        let inlines = vec![
            Inline::Text("a".into()),
            Inline::SoftBreak,
            Inline::Text("b".into()),
        ];
        assert_eq!(texts(&render(&inlines, 20)), vec!["a b"]);
    }

    #[test]
    fn hard_breaks_force_a_new_line() {
        let inlines = vec![
            Inline::Text("a".into()),
            Inline::HardBreak,
            Inline::Text("b".into()),
        ];
        assert_eq!(texts(&render(&inlines, 20)), vec!["a", "b"]);
    }

    #[test]
    fn a_word_longer_than_the_width_is_split_rather_than_overflowing() {
        // Long URLs must never blow past the measure and corrupt the layout.
        let inlines = vec![Inline::Text("abcdefghijkl".into())];
        assert_eq!(texts(&render(&inlines, 5)), vec!["abcde", "fghij", "kl"]);
    }

    #[test]
    fn double_width_characters_count_as_two_columns() {
        // Three CJK glyphs are six columns wide, so width 4 fits two.
        let inlines = vec![Inline::Text("漢字語".into())];
        assert_eq!(texts(&render(&inlines, 4)), vec!["漢字", "語"]);
    }

    #[test]
    fn empty_inlines_still_produce_one_line() {
        // Callers index into the returned Vec; never hand back an empty one.
        assert_eq!(render(&[], 20).len(), 1);
    }

    #[test]
    fn inline_code_is_never_split_across_lines() {
        let inlines = vec![
            Inline::Text("run".into()),
            Inline::Text(" ".into()),
            Inline::Code("cargo test".into()),
        ];
        let lines = render(&inlines, 12);
        assert!(
            lines.iter().any(|l| text_of(l).contains("cargo test")),
            "inline code was split: {:?}",
            texts(&lines)
        );
    }

    #[test]
    fn emphasis_and_strong_set_modifiers() {
        let toks = tokenize(
            &[
                Inline::Emph(vec![Inline::Text("i".into())]),
                Inline::Strong(vec![Inline::Text("b".into())]),
            ],
            &DARK,
            Style::default(),
        );
        assert!(toks[0].style.add_modifier.contains(Modifier::ITALIC));
        assert!(toks[1].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn nested_emphasis_accumulates_modifiers() {
        let toks = tokenize(
            &[Inline::Strong(vec![Inline::Emph(vec![Inline::Text(
                "x".into(),
            )])])],
            &DARK,
            Style::default(),
        );
        assert!(toks[0].style.add_modifier.contains(Modifier::BOLD));
        assert!(toks[0].style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn links_are_coloured_and_underlined() {
        let toks = tokenize(
            &[Inline::Link {
                target: "./a.md".into(),
                content: vec![Inline::Text("go".into())],
            }],
            &DARK,
            Style::default(),
        );
        assert_eq!(toks[0].style.fg, Some(DARK.link));
        assert!(toks[0].style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn strikethrough_sets_crossed_out() {
        let toks = tokenize(
            &[Inline::Strike(vec![Inline::Text("x".into())])],
            &DARK,
            Style::default(),
        );
        assert!(toks[0].style.add_modifier.contains(Modifier::CROSSED_OUT));
    }

    #[test]
    fn images_render_as_a_muted_alt_text_placeholder() {
        // P5 replaces this with real graphics; until then the reader must
        // still learn that something is there.
        let lines = render(
            &[Inline::Image {
                target: "/a.png".into(),
                alt: "chart".into(),
            }],
            40,
        );
        assert_eq!(texts(&lines), vec!["[image: chart]"]);
    }

    #[test]
    fn a_zero_width_request_does_not_panic_or_loop() {
        assert!(!render(&[Inline::Text("abc".into())], 0).is_empty());
    }
}
