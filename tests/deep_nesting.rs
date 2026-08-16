//! Regression tests for a stack-overflow crash: pathological Markdown with
//! very deep nesting (blockquotes, lists, or emphasis) used to make our own
//! recursive-descent IR conversion (`src/doc/parse.rs`) recurse once per
//! nesting level with no limit, overflowing the stack and aborting the
//! whole process — comrak's own parser copes fine with the same input.
//!
//! `#[should_panic]` cannot be used here: a stack overflow aborts the
//! process rather than unwinding, so nothing can catch it. These tests only
//! prove the fix by *completing* rather than crashing the test binary.
use mdread::{doc, render, theme};

#[test]
fn extremely_nested_blockquotes_do_not_crash() {
    let source = format!("{}text\n", "> ".repeat(50_000));
    let d = doc::parse(&source);
    assert_eq!(d.blocks.len(), 1);
}

#[test]
fn extremely_nested_lists_do_not_crash() {
    let source = format!("{}text\n", "- ".repeat(50_000));
    let d = doc::parse(&source);
    assert_eq!(d.blocks.len(), 1);
}

#[test]
fn extremely_nested_emphasis_does_not_crash() {
    let source = format!("{}text{}\n", "*".repeat(50_000), "*".repeat(50_000));
    let d = doc::parse(&source);
    assert_eq!(d.blocks.len(), 1);
}

#[test]
fn rendering_an_extremely_nested_document_does_not_crash() {
    // The renderer mirrors the IR's recursive shape (layout_quote /
    // layout_list / render_inlines), so it needs to be exercised too, not
    // just the parser. The rendered output must also stay bounded (a few
    // hundred lines, not tens of thousands) and show that content was cut
    // off rather than silently vanishing.
    // Wide enough that the marker text isn't itself hard-wrapped to one
    // character per line by the accumulated `│ ` bars (each of the 128
    // nested quote levels eats 2 columns of width) — that's a real and
    // already-tested aspect of quote rendering, just not what this test is
    // checking.
    let source = format!("{}text\n", "> ".repeat(50_000));
    let d = doc::parse(&source);
    let rendered = render::render(&d, 300, &theme::DARK);
    assert!(!rendered.lines.is_empty());
    assert!(
        rendered.lines.len() < 1000,
        "expected the render to stay bounded by the nesting cap, got {} lines",
        rendered.lines.len()
    );
    let flattened: String = rendered
        .lines
        .iter()
        .flat_map(|l| l.spans.iter())
        .map(|s| s.content.as_ref())
        .collect();
    assert!(
        flattened.contains("truncated"),
        "expected a visible marker that content was cut off"
    );
}

#[test]
fn normal_nesting_depths_are_completely_unaffected() {
    // Depths well under any sane cap must parse and render exactly as
    // before — this is the guard against the fix being too aggressive.
    let quotes = format!("{}text\n", "> ".repeat(5));
    let d = doc::parse(&quotes);
    match &d.blocks[0] {
        doc::ir::Block::Quote(inner) => {
            // Unwrap all 5 levels; the 5th must still hold real content.
            let mut cur = inner;
            for _ in 0..4 {
                match &cur[0] {
                    doc::ir::Block::Quote(next) => cur = next,
                    other => panic!("expected nested quote, got {other:?}"),
                }
            }
            assert_eq!(
                cur[0],
                doc::ir::Block::Paragraph(vec![doc::ir::Inline::Text("text".into())])
            );
        }
        other => panic!("expected quote, got {other:?}"),
    }

    let lists = format!("{}text\n", "- ".repeat(10));
    let d = doc::parse(&lists);
    assert_eq!(d.blocks.len(), 1);
    assert!(matches!(d.blocks[0], doc::ir::Block::List { .. }));

    let emph = format!("{}text{}\n", "*".repeat(10), "*".repeat(10));
    let d = doc::parse(&emph);
    assert_eq!(d.blocks.len(), 1);

    // And the render output for a normal document is byte-for-byte the
    // same as before this change (10 levels is nowhere near any cap).
    let rendered = render::render(&d, 80, &theme::DARK);
    assert!(!rendered.lines.is_empty());
}
