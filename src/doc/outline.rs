use crate::doc::ir::{Block, Document, Inline, OutlineEntry};
use std::collections::HashMap;

/// GitHub's heading-anchor algorithm: lowercase, drop punctuation other than
/// hyphens and underscores, turn whitespace runs into single hyphens.
pub fn slugify(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_sep = false;
    for ch in text.chars() {
        if ch.is_whitespace() || ch == '-' {
            // Only emit a separator once we know real content follows.
            pending_sep = !out.is_empty();
        } else if ch.is_alphanumeric() || ch == '_' {
            if pending_sep {
                out.push('-');
                pending_sep = false;
            }
            out.extend(ch.to_lowercase());
        }
        // Everything else — punctuation, symbols — is dropped outright.
    }
    out
}

/// Flatten inlines to plain text, for display, slugging, and measuring
/// table cells (Task 15).
pub(crate) fn inline_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for i in inlines {
        match i {
            Inline::Text(t) => out.push_str(t),
            Inline::Code(t) => out.push_str(t),
            Inline::Emph(c) | Inline::Strong(c) | Inline::Strike(c) => {
                out.push_str(&inline_text(c))
            }
            Inline::Link { content, .. } => out.push_str(&inline_text(content)),
            Inline::Image { alt, .. } => out.push_str(alt),
            Inline::SoftBreak | Inline::HardBreak => out.push(' '),
        }
    }
    out
}

/// Assign a unique slug to every top-level heading and return the outline.
pub fn build(blocks: &mut [Block]) -> Vec<OutlineEntry> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut outline = Vec::new();

    for (block_index, block) in blocks.iter_mut().enumerate() {
        let Block::Heading {
            level,
            content,
            slug,
        } = block
        else {
            continue;
        };
        let text = inline_text(content);
        let base = slugify(&text);
        // First occurrence keeps the bare slug; later ones get -1, -2, ...
        let count = seen.entry(base.clone()).or_insert(0);
        let unique = if *count == 0 {
            base.clone()
        } else {
            format!("{base}-{count}")
        };
        *count += 1;

        *slug = unique.clone();
        outline.push(OutlineEntry {
            level: *level,
            text,
            slug: unique,
            block_index,
        });
    }
    outline
}

/// Parse a Markdown source into a fully-formed document. This is the only
/// parsing entry point callers should use.
pub fn parse(source: &str) -> Document {
    let mut blocks = crate::doc::parse::parse_blocks(source);
    let outline = build(&mut blocks);
    Document { blocks, outline }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercases_and_hyphenates() {
        assert_eq!(slugify("Getting Started"), "getting-started");
    }

    #[test]
    fn strips_punctuation_but_keeps_hyphens_and_underscores() {
        assert_eq!(slugify("What's new, really?"), "whats-new-really");
        assert_eq!(slugify("well-known_thing"), "well-known_thing");
    }

    #[test]
    fn collapses_runs_of_whitespace_into_one_hyphen() {
        assert_eq!(slugify("a   b"), "a-b");
    }

    #[test]
    fn trims_leading_and_trailing_separators() {
        assert_eq!(slugify("  spaced  "), "spaced");
        assert_eq!(slugify("!!bang!!"), "bang");
    }

    #[test]
    fn keeps_non_ascii_letters() {
        assert_eq!(slugify("Café Münster"), "café-münster");
    }

    #[test]
    fn an_empty_or_all_punctuation_heading_yields_an_empty_slug() {
        assert_eq!(slugify("***"), "");
    }

    #[test]
    fn duplicate_headings_get_numeric_suffixes() {
        // This matches GitHub: the first keeps the bare slug, later ones
        // get -1, -2, ... Anchors in real docs depend on this exact rule.
        let mut blocks = parse_for_test("# Setup\n\n# Setup\n\n# Setup\n");
        let outline = build(&mut blocks);
        let slugs: Vec<&str> = outline.iter().map(|e| e.slug.as_str()).collect();
        assert_eq!(slugs, vec!["setup", "setup-1", "setup-2"]);
    }

    #[test]
    fn build_writes_slugs_back_into_the_blocks() {
        let mut blocks = parse_for_test("# Hello There\n");
        build(&mut blocks);
        match &blocks[0] {
            Block::Heading { slug, .. } => assert_eq!(slug, "hello-there"),
            other => panic!("expected heading, got {other:?}"),
        }
    }

    #[test]
    fn outline_records_level_text_and_block_index() {
        let mut blocks = parse_for_test("intro\n\n# A\n\n## B\n");
        let outline = build(&mut blocks);
        assert_eq!(outline.len(), 2);
        assert_eq!(outline[0].level, 1);
        assert_eq!(outline[0].text, "A");
        assert_eq!(
            outline[0].block_index, 1,
            "index into blocks, not into headings"
        );
        assert_eq!(outline[1].level, 2);
        assert_eq!(outline[1].block_index, 2);
    }

    #[test]
    fn heading_text_is_flattened_from_formatting() {
        let mut blocks = parse_for_test("# A `b` *c*\n");
        let outline = build(&mut blocks);
        assert_eq!(outline[0].text, "A b c");
        assert_eq!(outline[0].slug, "a-b-c");
    }

    #[test]
    fn parse_returns_blocks_and_outline_together() {
        let doc = parse("# Title\n\nbody\n");
        assert_eq!(doc.blocks.len(), 2);
        assert_eq!(doc.outline.len(), 1);
        assert_eq!(doc.outline[0].slug, "title");
    }

    #[test]
    fn headings_nested_in_quotes_are_not_in_the_outline() {
        // The outline is a navigation aid for top-level structure.
        let doc = parse("> # Quoted\n");
        assert!(doc.outline.is_empty());
    }

    fn parse_for_test(src: &str) -> Vec<Block> {
        crate::doc::parse::parse_blocks(src)
    }
}
