use crate::doc::ir::Block;
use crate::render::BlockRender;
use crate::render::inline::render_inlines;
use crate::theme::Theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

/// Lay out a sequence of blocks, separated by a single blank line.
pub fn layout_blocks(blocks: &[Block], width: u16, theme: &Theme) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            out.push(Line::from(String::new()));
        }
        out.extend(layout_block(block, width, theme).lines);
    }
    out
}

pub fn layout_block(block: &Block, width: u16, theme: &Theme) -> BlockRender {
    let width = width.max(1);
    match block {
        Block::Heading {
            level,
            content,
            slug,
        } => layout_heading(*level, content, slug, width, theme),
        Block::Paragraph(inlines) => BlockRender {
            lines: render_inlines(inlines, width, theme, Style::default().fg(theme.text)),
            anchor: None,
        },
        Block::Rule => BlockRender {
            lines: vec![Line::from(Span::styled(
                "─".repeat(width as usize),
                Style::default().fg(theme.rule),
            ))],
            anchor: None,
        },
        Block::Quote(inner) => BlockRender {
            lines: layout_quote(inner, width, theme),
            anchor: None,
        },
        // Tasks 13, 14, and 15 fill these in.
        Block::List { .. } | Block::Code { .. } | Block::Table(_) => BlockRender::default(),
    }
}

fn layout_heading(
    level: u8,
    content: &[crate::doc::ir::Inline],
    slug: &str,
    width: u16,
    theme: &Theme,
) -> BlockRender {
    let colour = if level <= 2 {
        theme.heading
    } else {
        theme.muted
    };
    let style = Style::default().fg(colour).add_modifier(Modifier::BOLD);
    let mut lines = render_inlines(content, width, theme, style);

    // Only H1 gets an underline, and only as wide as the title itself.
    if level == 1 {
        let title_width = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.width()).sum::<usize>())
            .max()
            .unwrap_or(0)
            .min(width as usize);
        lines.push(Line::from(Span::styled(
            "─".repeat(title_width),
            Style::default().fg(theme.rule),
        )));
    }

    BlockRender {
        lines,
        anchor: (!slug.is_empty()).then(|| slug.to_string()),
    }
}

fn layout_quote(inner: &[Block], width: u16, theme: &Theme) -> Vec<Line<'static>> {
    const BAR: &str = "│ ";
    let inner_width = width.saturating_sub(BAR.width() as u16).max(1);
    let lines = layout_blocks(inner, inner_width, theme);
    let bar_style = Style::default().fg(theme.quote_bar);

    lines
        .into_iter()
        .map(|line| {
            // A blank line inside a quote keeps the bar but drops the padding,
            // so no trailing whitespace is emitted.
            let is_blank = line.spans.iter().all(|s| s.content.trim().is_empty());
            let prefix = if is_blank { "│" } else { BAR };
            let mut spans = vec![Span::styled(prefix.to_string(), bar_style)];
            if !is_blank {
                spans.extend(line.spans);
            }
            Line::from(spans)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc;
    use crate::theme::DARK;

    fn text_of(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn texts(lines: &[Line<'_>]) -> Vec<String> {
        lines.iter().map(text_of).collect()
    }

    /// Parse one block of markdown and lay it out.
    fn lay(src: &str, width: u16) -> BlockRender {
        let d = doc::parse(src);
        layout_block(&d.blocks[0], width, &DARK)
    }

    #[test]
    fn h1_is_underlined_with_a_rule_the_width_of_its_text() {
        let r = lay("# Getting Started\n", 40);
        assert_eq!(texts(&r.lines), vec!["Getting Started", "───────────────"]);
    }

    #[test]
    fn the_h1_rule_never_exceeds_the_measure() {
        let r = lay("# aaaaaaaaaaaaaaaaaaaaaaaa\n", 10);
        for line in &r.lines {
            assert!(text_of(line).width() <= 10, "line overflowed: {line:?}");
        }
    }

    #[test]
    fn h2_and_below_have_no_rule() {
        assert_eq!(texts(&lay("## Sub\n", 40).lines), vec!["Sub"]);
        assert_eq!(texts(&lay("### Deep\n", 40).lines), vec!["Deep"]);
    }

    #[test]
    fn headings_are_bold() {
        let r = lay("## Sub\n", 40);
        assert!(
            r.lines[0].spans[0]
                .style
                .add_modifier
                .contains(Modifier::BOLD)
        );
    }

    #[test]
    fn headings_expose_their_slug_as_an_anchor() {
        assert_eq!(
            lay("# Getting Started\n", 40).anchor.as_deref(),
            Some("getting-started")
        );
    }

    #[test]
    fn non_headings_have_no_anchor() {
        assert_eq!(lay("text\n", 40).anchor, None);
    }

    #[test]
    fn paragraphs_wrap_to_the_measure() {
        let r = lay("the quick brown fox jumps\n", 10);
        for line in &r.lines {
            assert!(text_of(line).width() <= 10);
        }
        assert!(r.lines.len() > 1);
    }

    #[test]
    fn a_rule_fills_the_measure() {
        let r = lay("---\n", 8);
        assert_eq!(texts(&r.lines), vec!["────────"]);
    }

    #[test]
    fn quotes_get_a_bar_on_every_line() {
        let r = lay("> alpha beta gamma delta\n", 12);
        for line in &r.lines {
            assert!(text_of(line).starts_with("│ "), "missing bar: {line:?}");
            assert!(text_of(line).width() <= 12);
        }
        assert!(r.lines.len() > 1, "expected the quote to wrap");
    }

    #[test]
    fn nested_quotes_stack_their_bars() {
        let r = lay("> > deep\n", 20);
        assert_eq!(texts(&r.lines), vec!["│ │ deep"]);
    }

    #[test]
    fn a_quote_containing_two_paragraphs_has_a_barred_blank_line_between_them() {
        let r = lay("> one\n>\n> two\n", 20);
        assert_eq!(texts(&r.lines), vec!["│ one", "│", "│ two"]);
    }

    #[test]
    fn layout_blocks_separates_blocks_with_one_blank_line() {
        let d = doc::parse("a\n\nb\n");
        assert_eq!(
            texts(&layout_blocks(&d.blocks, 20, &DARK)),
            vec!["a", "", "b"]
        );
    }

    #[test]
    fn layout_blocks_on_an_empty_document_yields_nothing() {
        assert!(layout_blocks(&[], 20, &DARK).is_empty());
    }
}
