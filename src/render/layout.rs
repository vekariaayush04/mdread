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
        Block::List {
            ordered,
            start,
            items,
        } => BlockRender {
            lines: layout_list(*ordered, *start, items, width, theme),
            anchor: None,
        },
        Block::Code { lang, text } => BlockRender {
            lines: crate::render::code::layout_code(lang.as_deref(), text, width, theme),
            anchor: None,
        },
        Block::Table(t) => BlockRender {
            lines: crate::render::table::layout_table(t, width, theme),
            anchor: None,
        },
    }
}

fn layout_list(
    ordered: bool,
    start: u64,
    items: &[crate::doc::ir::ListItem],
    width: u16,
    theme: &Theme,
) -> Vec<Line<'static>> {
    // All markers in one list share a width so their content aligns, which
    // matters once the numbers reach two digits.
    let markers: Vec<String> = items
        .iter()
        .enumerate()
        .map(|(i, item)| match (ordered, item.checked) {
            (_, Some(true)) => "[x] ".to_string(),
            (_, Some(false)) => "[ ] ".to_string(),
            (true, None) => format!("{}. ", start + i as u64),
            (false, None) => "• ".to_string(),
        })
        .collect();
    let indent = markers.iter().map(|m| m.width()).max().unwrap_or(0);

    let mut out = Vec::new();
    for (item, marker) in items.iter().zip(&markers) {
        let marker_style = match item.checked {
            Some(true) => Style::default().fg(theme.accent),
            Some(false) => Style::default().fg(theme.muted),
            None => Style::default().fg(theme.accent),
        };
        let inner_width = width.saturating_sub(indent as u16).max(1);
        let body = layout_item_blocks(&item.blocks, inner_width, theme);
        let first = Span::styled(format!("{marker:<indent$}"), marker_style);
        let rest = Span::raw(" ".repeat(indent));
        out.extend(hanging_indent(body, first, rest));
    }
    out
}

/// Lay out the blocks of one list item. A nested list sits directly under
/// its parent's text: a blank line between a bullet and its sub-bullets
/// reads as a break in the list. Every other block separates normally.
fn layout_item_blocks(blocks: &[Block], width: u16, theme: &Theme) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 && !matches!(block, Block::List { .. }) {
            out.push(Line::from(String::new()));
        }
        out.extend(layout_block(block, width, theme).lines);
    }
    out
}

/// Prefix the first line with `first` and every later line with `rest`.
/// Blank lines get no prefix, so no trailing whitespace is emitted.
fn hanging_indent(
    lines: Vec<Line<'static>>,
    first: Span<'static>,
    rest: Span<'static>,
) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            if line.spans.iter().all(|s| s.content.trim().is_empty()) {
                return Line::from(String::new());
            }
            let mut spans = vec![if i == 0 { first.clone() } else { rest.clone() }];
            spans.extend(line.spans);
            Line::from(spans)
        })
        .collect()
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

    #[test]
    fn bullets_use_a_marker_and_hanging_indent() {
        let r = lay("- alpha beta gamma\n", 12);
        assert_eq!(texts(&r.lines), vec!["• alpha beta", "  gamma"]);
    }

    #[test]
    fn ordered_lists_number_from_their_start_value() {
        let r = lay("3. a\n4. b\n", 20);
        assert_eq!(texts(&r.lines), vec!["3. a", "4. b"]);
    }

    #[test]
    fn ordered_markers_wider_than_one_digit_still_align() {
        // Every marker in a list is padded to the widest, so the item text
        // starts in the same column whether the number is one digit or two.
        let r = lay("9. a\n10. b\n", 20);
        assert_eq!(texts(&r.lines), vec!["9.  a", "10. b"]);
    }

    #[test]
    fn task_items_render_checkboxes() {
        let r = lay("- [x] done\n- [ ] todo\n", 20);
        assert_eq!(texts(&r.lines), vec!["[x] done", "[ ] todo"]);
    }

    #[test]
    fn nested_lists_are_indented_under_their_parent() {
        let r = lay("- a\n  - b\n", 20);
        assert_eq!(texts(&r.lines), vec!["• a", "  • b"]);
    }

    #[test]
    fn list_items_never_exceed_the_measure() {
        let r = lay("- alpha beta gamma delta epsilon\n", 14);
        for line in &r.lines {
            assert!(text_of(line).width() <= 14, "overflow: {line:?}");
        }
    }

    #[test]
    fn an_item_with_two_paragraphs_keeps_them_separated_and_indented() {
        let r = lay("- one\n\n  two\n", 20);
        assert_eq!(texts(&r.lines), vec!["• one", "", "  two"]);
    }

    #[test]
    fn a_list_with_no_items_produces_no_lines() {
        let block = Block::List {
            ordered: false,
            start: 1,
            items: vec![],
        };
        assert!(layout_block(&block, 20, &DARK).lines.is_empty());
    }
}
