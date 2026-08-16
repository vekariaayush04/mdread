pub mod code;
pub mod inline;
pub mod layout;
pub mod table;

use crate::doc::ir::Document;
use crate::theme::Theme;
use ratatui::text::Line;
use std::collections::HashMap;

// NOTE: do not add `use unicode_width::UnicodeWidthStr;` here — only the test
// module needs it, and an unused import fails `clippy -D warnings`. The test
// module imports it itself.

/// The result of laying out one block. `anchor` carries a heading's slug so
/// the document renderer can index it; everything else is plain lines.
#[derive(Debug, Clone, Default)]
pub struct BlockRender {
    pub lines: Vec<Line<'static>>,
    pub anchor: Option<String>,
}

/// A whole document laid out at one specific width.
#[derive(Debug, Clone, Default)]
pub struct RenderedDoc {
    pub lines: Vec<Line<'static>>,
    /// Heading slug to the line it starts on.
    pub anchors: HashMap<String, usize>,
    /// First line of each source block, in document order.
    pub block_starts: Vec<usize>,
}

impl RenderedDoc {
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Which block owns `line`. Clamps past the end to the final block.
    pub fn block_at_line(&self, line: usize) -> usize {
        match self.block_starts.binary_search(&line) {
            Ok(i) => i,
            Err(0) => 0,
            Err(i) => i - 1,
        }
    }

    pub fn line_of_block(&self, block: usize) -> usize {
        self.block_starts.get(block).copied().unwrap_or(0)
    }
}

pub fn render(doc: &Document, width: u16, theme: &Theme) -> RenderedDoc {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut anchors = HashMap::new();
    let mut block_starts = Vec::with_capacity(doc.blocks.len());

    for (i, block) in doc.blocks.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(String::new()));
        }
        let start = lines.len();
        block_starts.push(start);

        let rendered = layout::layout_block(block, width, theme);
        if let Some(anchor) = rendered.anchor {
            anchors.insert(anchor, start);
        }
        lines.extend(rendered.lines);
    }

    RenderedDoc {
        lines,
        anchors,
        block_starts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc;
    use crate::theme::DARK;
    use unicode_width::UnicodeWidthStr;

    fn text_of(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn blocks_are_separated_by_one_blank_line() {
        let d = doc::parse("alpha\n\nbeta\n");
        let r = render(&d, 40, &DARK);
        let texts: Vec<String> = r.lines.iter().map(text_of).collect();
        assert_eq!(texts, vec!["alpha", "", "beta"]);
    }

    #[test]
    fn block_starts_points_at_each_blocks_first_line() {
        let d = doc::parse("alpha\n\nbeta\n");
        let r = render(&d, 40, &DARK);
        assert_eq!(r.block_starts, vec![0, 2]);
    }

    #[test]
    fn block_starts_accounts_for_multi_line_blocks() {
        // An H1 renders as a title plus its underline: two lines.
        let d = doc::parse("# Title\n\nbody\n");
        let r = render(&d, 40, &DARK);
        assert_eq!(r.block_starts, vec![0, 3]);
    }

    #[test]
    fn anchors_map_each_heading_slug_to_its_line() {
        let d = doc::parse("# One\n\nbody\n\n## Two\n");
        let r = render(&d, 40, &DARK);
        assert_eq!(r.anchors.get("one"), Some(&0));
        assert_eq!(r.anchors.get("two"), Some(&5));
    }

    #[test]
    fn duplicate_headings_get_distinct_anchors() {
        let d = doc::parse("# Setup\n\n# Setup\n");
        let r = render(&d, 40, &DARK);
        assert!(r.anchors.contains_key("setup"));
        assert!(r.anchors.contains_key("setup-1"));
        assert_ne!(r.anchors["setup"], r.anchors["setup-1"]);
    }

    #[test]
    fn an_empty_document_renders_to_nothing() {
        let r = render(&doc::parse(""), 40, &DARK);
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        assert!(r.block_starts.is_empty());
    }

    #[test]
    fn block_at_line_finds_the_containing_block() {
        let d = doc::parse("# Title\n\nbody\n");
        let r = render(&d, 40, &DARK);
        assert_eq!(r.block_at_line(0), 0);
        assert_eq!(
            r.block_at_line(1),
            0,
            "the H1 underline belongs to the heading"
        );
        assert_eq!(r.block_at_line(3), 1);
        assert_eq!(
            r.block_at_line(999),
            1,
            "past the end clamps to the last block"
        );
    }

    #[test]
    fn line_of_block_is_the_inverse_of_block_at_line() {
        let d = doc::parse("# Title\n\nbody\n\n## Next\n");
        let r = render(&d, 40, &DARK);
        for b in 0..r.block_starts.len() {
            assert_eq!(r.block_at_line(r.line_of_block(b)), b);
        }
    }

    #[test]
    fn no_rendered_line_exceeds_the_requested_width() {
        let src = "# Heading\n\nsome fairly long prose here\n\n- a list item that is long\n\n```rust\nfn a() {}\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";
        let d = doc::parse(src);
        for width in [20u16, 40, 80] {
            for line in &render(&d, width, &DARK).lines {
                let w: usize = line.spans.iter().map(|s| s.content.width()).sum();
                assert!(w <= width as usize, "width {width} overflowed: {line:?}");
            }
        }
    }
}
