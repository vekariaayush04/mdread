use crate::doc::ir::*;
use comrak::nodes::{AstNode, NodeValue};
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

pub fn parse_blocks(source: &str) -> Vec<Block> {
    let arena = Arena::new();
    let root = parse_document(&arena, source, &options());
    collect_blocks(root)
}

fn collect_blocks<'a>(node: &'a AstNode<'a>) -> Vec<Block> {
    node.children().filter_map(block_from).collect()
}

fn block_from<'a>(node: &'a AstNode<'a>) -> Option<Block> {
    let value = node.data.borrow().value.clone();
    match value {
        NodeValue::Heading(h) => Some(Block::Heading {
            level: h.level,
            content: collect_inlines(node),
            slug: String::new(),
        }),
        NodeValue::Paragraph => Some(Block::Paragraph(collect_inlines(node))),
        NodeValue::ThematicBreak => Some(Block::Rule),
        // Anything we do not model — HTML blocks, footnote definitions — is dropped.
        _ => None,
    }
}

fn collect_inlines<'a>(node: &'a AstNode<'a>) -> Vec<Inline> {
    node.children().filter_map(inline_from).collect()
}

fn inline_from<'a>(node: &'a AstNode<'a>) -> Option<Inline> {
    let value = node.data.borrow().value.clone();
    match value {
        // comrak 0.54 stores text as Cow<'static, str>, not String.
        NodeValue::Text(t) => Some(Inline::Text(t.into_owned())),
        NodeValue::Code(c) => Some(Inline::Code(c.literal)),
        NodeValue::Emph => Some(Inline::Emph(collect_inlines(node))),
        NodeValue::Strong => Some(Inline::Strong(collect_inlines(node))),
        NodeValue::Strikethrough => Some(Inline::Strike(collect_inlines(node))),
        NodeValue::Link(l) => Some(Inline::Link {
            target: l.url,
            content: collect_inlines(node),
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
fn plain_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut out = String::new();
    fn walk<'a>(node: &'a AstNode<'a>, out: &mut String) {
        match &node.data.borrow().value {
            NodeValue::Text(t) => out.push_str(t),
            NodeValue::Code(c) => out.push_str(&c.literal),
            NodeValue::SoftBreak | NodeValue::LineBreak => out.push(' '),
            _ => {}
        }
        for c in node.children() {
            walk(c, out);
        }
    }
    for c in node.children() {
        walk(c, &mut out);
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
