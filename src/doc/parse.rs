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
    node.children()
        .filter_map(|c| match c.data.borrow().value.clone() {
            // comrak 0.54 stores text as Cow<'static, str>, not String.
            NodeValue::Text(t) => Some(Inline::Text(t.into_owned())),
            _ => None,
        })
        .collect()
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
}
