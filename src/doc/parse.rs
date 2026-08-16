use crate::doc::ir::*;
use comrak::nodes::{AstNode, ListType, NodeValue, TableAlignment};
use comrak::{Arena, Options, parse_document};

/// Extensions we enable. Kept in one place so parsing is identical
/// everywhere, including in tests.
pub fn options() -> Options<'static> {
    let mut o = Options::default();
    o.extension.table = true;
    o.extension.strikethrough = true;
    o.extension.tasklist = true;
    o.extension.autolink = true;
    o.extension.footnotes = true;
    o
}

/// Maximum recursion depth allowed when converting comrak's AST into our own
/// IR — applied independently to block containers (quotes, lists) and to
/// inline containers (emphasis, strong, strikethrough, links; see
/// `bounded_inlines`).
///
/// comrak's own parser copes fine with arbitrarily deep nesting (its
/// delimiter-stack and arena-based algorithms aren't recursive-per-level),
/// but the conversion below recurses once per level of nesting, and so does
/// every downstream consumer that walks the resulting IR (the renderer in
/// `src/render/layout.rs`, `inline_text` in `src/doc/outline.rs`). Capping
/// depth here, at the single point where the IR is built, is enough to
/// bound all of them: a document like `"> ".repeat(10_000)` used to blow the
/// stack (and stack overflow aborts the process — it cannot be caught) here
/// and in every walker after it.
///
/// Real Markdown essentially never nests past ~10 levels, so 128 leaves
/// over 10x headroom for any legitimate document while staying nowhere near
/// a thread's stack limit even under a worst case combining block and
/// inline recursion together (256 total levels through both this
/// conversion and the renderer's mirroring recursion) on a constrained 2-8
/// MiB stack. This is the same range (100-1000) CommonMark reference
/// implementations commonly use for the equivalent guard.
const MAX_NESTING_DEPTH: usize = 128;

/// What a truncated container is replaced with once `MAX_NESTING_DEPTH` is
/// reached. An *empty* container would technically be "truncated" too, but
/// every layout function in `src/render/layout.rs` renders an empty
/// container as zero lines, and that zero propagates: a quote whose only
/// child renders to nothing itself renders to nothing, all the way up the
/// chain. A 50,000-deep blockquote would then render as a genuinely blank
/// screen, which reads as broken/hung rather than "there was more here that
/// got cut off". A one-line marker breaks that cascade at the cap and keeps
/// the truncation visible instead of silent.
const TRUNCATED_MARKER: &str = "[truncated: exceeds maximum nesting depth]";

fn truncated_paragraph() -> Block {
    Block::Paragraph(vec![Inline::Text(TRUNCATED_MARKER.to_string())])
}

pub fn parse_blocks(source: &str) -> Vec<Block> {
    let arena = Arena::new();
    let root = parse_document(&arena, source, &options());
    collect_blocks(root, 0)
}

fn collect_blocks<'a>(node: &'a AstNode<'a>, depth: usize) -> Vec<Block> {
    node.children()
        .filter_map(|n| block_from(n, depth))
        .collect()
}

/// Recurse into a block container's children unless the depth cap has been
/// reached, in which case a visible marker takes the place of the (dropped)
/// over-deep content — see `TRUNCATED_MARKER`.
fn bounded_blocks<'a>(node: &'a AstNode<'a>, depth: usize) -> Vec<Block> {
    if depth >= MAX_NESTING_DEPTH {
        vec![truncated_paragraph()]
    } else {
        collect_blocks(node, depth + 1)
    }
}

/// Same choice as `bounded_blocks`, for a list's items.
fn bounded_items<'a>(node: &'a AstNode<'a>, depth: usize) -> Vec<ListItem> {
    if depth >= MAX_NESTING_DEPTH {
        vec![ListItem {
            checked: None,
            blocks: vec![truncated_paragraph()],
        }]
    } else {
        collect_items(node, depth + 1)
    }
}

fn block_from<'a>(node: &'a AstNode<'a>, depth: usize) -> Option<Block> {
    let value = node.data.borrow().value.clone();
    match value {
        NodeValue::Heading(h) => Some(Block::Heading {
            level: h.level,
            content: collect_inlines(node, 0),
            slug: String::new(),
        }),
        NodeValue::Paragraph => Some(Block::Paragraph(collect_inlines(node, 0))),
        NodeValue::ThematicBreak => Some(Block::Rule),
        NodeValue::BlockQuote => Some(Block::Quote(bounded_blocks(node, depth))),
        NodeValue::List(l) => Some(Block::List {
            ordered: l.list_type == ListType::Ordered,
            start: l.start as u64,
            items: bounded_items(node, depth),
        }),
        NodeValue::CodeBlock(c) => Some(Block::Code {
            // The info string may carry extra words ("rust,ignore"); only the
            // first token is the language, and an empty info means none.
            lang: c
                .info
                .split([',', ' '])
                .next()
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            text: c.literal,
        }),
        NodeValue::Table(t) => Some(Block::Table(table_from(node, &t.alignments))),
        // Anything we do not model — HTML blocks, footnote definitions — is dropped.
        _ => None,
    }
}

fn table_from<'a>(node: &'a AstNode<'a>, alignments: &[TableAlignment]) -> Table {
    let align = alignments
        .iter()
        .map(|a| match a {
            TableAlignment::Left => Align::Left,
            TableAlignment::Center => Align::Center,
            TableAlignment::Right => Align::Right,
            TableAlignment::None => Align::None,
        })
        .collect();

    let mut head = Vec::new();
    let mut rows = Vec::new();
    for row in node.children() {
        let is_header = matches!(row.data.borrow().value, NodeValue::TableRow(true));
        let cells: Vec<Vec<Inline>> = row.children().map(|c| collect_inlines(c, 0)).collect();
        if is_header {
            head = cells;
        } else {
            rows.push(cells);
        }
    }
    Table { align, head, rows }
}

fn collect_items<'a>(node: &'a AstNode<'a>, depth: usize) -> Vec<ListItem> {
    node.children()
        .filter_map(|child| match child.data.borrow().value.clone() {
            NodeValue::Item(_) => Some(ListItem {
                checked: None,
                blocks: collect_blocks(child, depth),
            }),
            // A present symbol means the box is ticked.
            NodeValue::TaskItem(t) => Some(ListItem {
                checked: Some(t.symbol.is_some()),
                blocks: collect_blocks(child, depth),
            }),
            _ => None,
        })
        .collect()
}

fn collect_inlines<'a>(node: &'a AstNode<'a>, depth: usize) -> Vec<Inline> {
    node.children()
        .filter_map(|n| inline_from(n, depth))
        .collect()
}

/// Recurse into an inline container's children unless the depth cap has
/// been reached (see `MAX_NESTING_DEPTH`). Past the cap the container's own
/// content is replaced with `TRUNCATED_MARKER` rather than descended into
/// further, for the same reason blocks are — otherwise an over-deep `Emph`
/// wraps nothing, and that nothing propagates outward through every
/// enclosing span the same way an empty quote does.
fn bounded_inlines<'a>(node: &'a AstNode<'a>, depth: usize) -> Vec<Inline> {
    if depth >= MAX_NESTING_DEPTH {
        vec![Inline::Text(TRUNCATED_MARKER.to_string())]
    } else {
        collect_inlines(node, depth + 1)
    }
}

fn inline_from<'a>(node: &'a AstNode<'a>, depth: usize) -> Option<Inline> {
    let value = node.data.borrow().value.clone();
    match value {
        // comrak 0.54 stores text as Cow<'static, str>, not String.
        NodeValue::Text(t) => Some(Inline::Text(t.into_owned())),
        NodeValue::Code(c) => Some(Inline::Code(c.literal)),
        NodeValue::Emph => Some(Inline::Emph(bounded_inlines(node, depth))),
        NodeValue::Strong => Some(Inline::Strong(bounded_inlines(node, depth))),
        NodeValue::Strikethrough => Some(Inline::Strike(bounded_inlines(node, depth))),
        NodeValue::Link(l) => Some(Inline::Link {
            target: l.url,
            content: bounded_inlines(node, depth),
        }),
        NodeValue::Image(l) => Some(Inline::Image {
            target: l.url,
            alt: plain_text(node),
        }),
        NodeValue::SoftBreak => Some(Inline::SoftBreak),
        NodeValue::LineBreak => Some(Inline::HardBreak),
        // Drop the tags but keep the text between them.
        NodeValue::HtmlInline(_) => None,
        _ => None,
    }
}

/// Concatenate all text descendants, discarding formatting. Used for image
/// alt text and heading titles in the outline.
///
/// Alt text can itself contain arbitrarily nested formatting
/// (`![**...**](url)`), so this walk is depth-capped exactly like
/// `bounded_inlines` — it walks comrak's AST directly rather than our IR,
/// but the same unbounded-recursion risk applies.
fn plain_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut out = String::new();
    fn walk<'a>(node: &'a AstNode<'a>, out: &mut String, depth: usize) {
        if depth >= MAX_NESTING_DEPTH {
            return;
        }
        match &node.data.borrow().value {
            NodeValue::Text(t) => out.push_str(t),
            NodeValue::Code(c) => out.push_str(&c.literal),
            NodeValue::SoftBreak | NodeValue::LineBreak => out.push(' '),
            _ => {}
        }
        for c in node.children() {
            walk(c, out, depth + 1);
        }
    }
    for c in node.children() {
        walk(c, &mut out, 0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_headings_at_every_level() {
        let blocks = parse_blocks("# One\n\n### Three\n");
        assert_eq!(blocks.len(), 2);
        match &blocks[0] {
            Block::Heading {
                level,
                content,
                slug,
            } => {
                assert_eq!(*level, 1);
                assert_eq!(content, &vec![Inline::Text("One".into())]);
                assert_eq!(slug, "", "slugs are assigned by outline::build, not here");
            }
            other => panic!("expected heading, got {other:?}"),
        }
        match &blocks[1] {
            Block::Heading { level, .. } => assert_eq!(*level, 3),
            other => panic!("expected heading, got {other:?}"),
        }
    }

    #[test]
    fn parses_a_paragraph() {
        let blocks = parse_blocks("hello world\n");
        assert_eq!(
            blocks,
            vec![Block::Paragraph(vec![Inline::Text("hello world".into())])]
        );
    }

    #[test]
    fn parses_a_thematic_break() {
        assert_eq!(parse_blocks("---\n"), vec![Block::Rule]);
    }

    #[test]
    fn empty_input_yields_no_blocks() {
        assert_eq!(parse_blocks(""), vec![]);
    }

    #[test]
    fn blocks_keep_document_order() {
        let blocks = parse_blocks("# H\n\npara\n\n---\n");
        assert!(matches!(blocks[0], Block::Heading { .. }));
        assert!(matches!(blocks[1], Block::Paragraph(_)));
        assert_eq!(blocks[2], Block::Rule);
    }

    #[test]
    fn html_blocks_are_dropped() {
        // Rendering raw HTML is an explicit non-goal.
        let blocks = parse_blocks("<div>hi</div>\n");
        assert_eq!(blocks, vec![]);
    }

    /// Helper: parse a single paragraph and return its inlines.
    fn inlines_of(src: &str) -> Vec<Inline> {
        match parse_blocks(src).into_iter().next() {
            Some(Block::Paragraph(i)) => i,
            other => panic!("expected a paragraph, got {other:?}"),
        }
    }

    #[test]
    fn parses_emphasis_and_strong() {
        assert_eq!(
            inlines_of("*a* **b**\n"),
            vec![
                Inline::Emph(vec![Inline::Text("a".into())]),
                Inline::Text(" ".into()),
                Inline::Strong(vec![Inline::Text("b".into())]),
            ]
        );
    }

    #[test]
    fn parses_strikethrough() {
        assert_eq!(
            inlines_of("~~gone~~\n"),
            vec![Inline::Strike(vec![Inline::Text("gone".into())])]
        );
    }

    #[test]
    fn parses_inline_code() {
        assert_eq!(inlines_of("`x = 1`\n"), vec![Inline::Code("x = 1".into())]);
    }

    #[test]
    fn parses_links_with_nested_emphasis() {
        assert_eq!(
            inlines_of("[*go*](./a.md)\n"),
            vec![Inline::Link {
                target: "./a.md".into(),
                content: vec![Inline::Emph(vec![Inline::Text("go".into())])],
            }]
        );
    }

    #[test]
    fn parses_images_flattening_alt_text() {
        assert_eq!(
            inlines_of("![a *b*](/p.png)\n"),
            vec![Inline::Image {
                target: "/p.png".into(),
                alt: "a b".into(),
            }]
        );
    }

    #[test]
    fn distinguishes_soft_and_hard_breaks() {
        assert_eq!(
            inlines_of("a\nb\n"),
            vec![
                Inline::Text("a".into()),
                Inline::SoftBreak,
                Inline::Text("b".into()),
            ]
        );
        assert_eq!(
            inlines_of("a  \nb\n"),
            vec![
                Inline::Text("a".into()),
                Inline::HardBreak,
                Inline::Text("b".into()),
            ]
        );
    }

    #[test]
    fn inline_html_is_dropped_but_its_text_survives() {
        // "<b>raw</b>" must render as "raw", never as literal angle brackets.
        assert_eq!(
            inlines_of("x <b>raw</b>\n"),
            vec![Inline::Text("x ".into()), Inline::Text("raw".into()),]
        );
    }

    #[test]
    fn parses_a_bullet_list() {
        let blocks = parse_blocks("- a\n- b\n");
        match &blocks[0] {
            Block::List {
                ordered,
                start,
                items,
            } => {
                assert!(!ordered);
                assert_eq!(*start, 1);
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].checked, None);
                assert_eq!(
                    items[0].blocks,
                    vec![Block::Paragraph(vec![Inline::Text("a".into())])]
                );
            }
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn parses_an_ordered_list_honouring_its_start_number() {
        let blocks = parse_blocks("3. a\n4. b\n");
        match &blocks[0] {
            Block::List {
                ordered,
                start,
                items,
            } => {
                assert!(ordered);
                assert_eq!(*start, 3);
                assert_eq!(items.len(), 2);
            }
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn parses_task_items_with_their_checked_state() {
        // comrak reports checked items as TaskItem(symbol: Some(_)) and
        // unchecked ones as TaskItem(symbol: None) — NOT as plain Items.
        let blocks = parse_blocks("- [x] done\n- [ ] todo\n");
        match &blocks[0] {
            Block::List { items, .. } => {
                assert_eq!(items[0].checked, Some(true));
                assert_eq!(items[1].checked, Some(false));
            }
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn parses_nested_lists() {
        let blocks = parse_blocks("- a\n  - b\n");
        match &blocks[0] {
            Block::List { items, .. } => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].blocks.len(), 2);
                assert!(matches!(items[0].blocks[1], Block::List { .. }));
            }
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn parses_a_block_quote() {
        let blocks = parse_blocks("> hello\n");
        assert_eq!(
            blocks,
            vec![Block::Quote(vec![Block::Paragraph(vec![Inline::Text(
                "hello".into()
            )])])]
        );
    }

    #[test]
    fn parses_nested_block_quotes() {
        let blocks = parse_blocks("> > deep\n");
        match &blocks[0] {
            Block::Quote(inner) => assert!(matches!(inner[0], Block::Quote(_))),
            other => panic!("expected quote, got {other:?}"),
        }
    }

    #[test]
    fn parses_a_list_inside_a_block_quote() {
        let blocks = parse_blocks("> - a\n");
        match &blocks[0] {
            Block::Quote(inner) => assert!(matches!(inner[0], Block::List { .. })),
            other => panic!("expected quote, got {other:?}"),
        }
    }

    #[test]
    fn parses_a_fenced_code_block_with_a_language() {
        let blocks = parse_blocks("```rust\nfn a() {}\n```\n");
        assert_eq!(
            blocks,
            vec![Block::Code {
                lang: Some("rust".into()),
                text: "fn a() {}\n".into(),
            }]
        );
    }

    #[test]
    fn a_fence_with_no_language_has_none() {
        let blocks = parse_blocks("```\nplain\n```\n");
        assert_eq!(
            blocks,
            vec![Block::Code {
                lang: None,
                text: "plain\n".into()
            }]
        );
    }

    #[test]
    fn an_indented_code_block_has_no_language() {
        // comrak reports info == "" here. Passing "" to syntect finds no
        // syntax, so it must become None rather than Some("").
        let blocks = parse_blocks("    indented\n");
        assert_eq!(
            blocks,
            vec![Block::Code {
                lang: None,
                text: "indented\n".into()
            }]
        );
    }

    #[test]
    fn an_info_string_with_extra_words_keeps_only_the_language() {
        let blocks = parse_blocks("```rust,ignore\nx\n```\n");
        match &blocks[0] {
            Block::Code { lang, .. } => assert_eq!(lang.as_deref(), Some("rust")),
            other => panic!("expected code, got {other:?}"),
        }
    }

    #[test]
    fn parses_a_table_with_a_header_and_rows() {
        let blocks = parse_blocks("| a | b |\n|---|---|\n| 1 | 2 |\n");
        match &blocks[0] {
            Block::Table(t) => {
                assert_eq!(t.head.len(), 2);
                assert_eq!(t.head[0], vec![Inline::Text("a".into())]);
                assert_eq!(t.rows.len(), 1);
                assert_eq!(t.rows[0].len(), 2);
                assert_eq!(t.rows[0][1], vec![Inline::Text("2".into())]);
            }
            other => panic!("expected table, got {other:?}"),
        }
    }

    #[test]
    fn parses_table_column_alignments() {
        let blocks = parse_blocks("| a | b | c | d |\n|:--|:-:|--:|---|\n| 1 | 2 | 3 | 4 |\n");
        match &blocks[0] {
            Block::Table(t) => assert_eq!(
                t.align,
                vec![Align::Left, Align::Center, Align::Right, Align::None]
            ),
            other => panic!("expected table, got {other:?}"),
        }
    }

    #[test]
    fn a_table_with_no_body_rows_still_parses() {
        let blocks = parse_blocks("| a |\n|---|\n");
        match &blocks[0] {
            Block::Table(t) => {
                assert_eq!(t.head.len(), 1);
                assert!(t.rows.is_empty());
            }
            other => panic!("expected table, got {other:?}"),
        }
    }

    #[test]
    fn autolinks_become_links() {
        assert_eq!(
            inlines_of("https://example.com\n"),
            vec![Inline::Link {
                target: "https://example.com".into(),
                content: vec![Inline::Text("https://example.com".into())],
            }]
        );
    }
}
