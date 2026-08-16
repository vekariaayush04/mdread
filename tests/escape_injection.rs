//! Defence-in-depth against terminal escape-sequence injection: a hostile
//! Markdown file can contain raw ANSI/OSC escape sequences, BEL, backspace,
//! DEL, and other C0/C1 control characters. These are not exploitable
//! today — ratatui drops them at its cell-buffer boundary before anything
//! reaches the real terminal — but that protection is incidental behaviour
//! of a dependency, not something this crate controls. `src/render`
//! sanitises document text as it becomes `Span` content so a `RenderedDoc`
//! can never carry a control character through to whatever draws it.
use mdread::{doc, render, theme};

/// Flatten a rendered document to one string, spans concatenated in order.
fn flatten(source: &str, width: u16) -> String {
    let document = doc::parse(source);
    render::render(&document, width, &theme::DARK)
        .lines
        .iter()
        .flat_map(|l| l.spans.iter())
        .map(|s| s.content.as_ref())
        .collect()
}

#[test]
fn an_ansi_colour_escape_in_a_paragraph_is_neutralised() {
    let source = "\u{1b}[31mRED\u{1b}[0m text\n";
    let flat = flatten(source, 80);
    assert!(!flat.contains('\u{1b}'), "escape survived: {flat:?}");
    assert!(flat.contains("RED"), "printable payload lost: {flat:?}");
}

#[test]
fn an_osc_title_rewrite_sequence_is_neutralised() {
    let source = "\u{1b}]0;pwned\u{7} some text\n";
    let flat = flatten(source, 80);
    assert!(!flat.contains('\u{1b}'), "ESC survived: {flat:?}");
    assert!(!flat.contains('\u{7}'), "BEL survived: {flat:?}");
    assert!(flat.contains("some text"), "payload lost: {flat:?}");
}

#[test]
fn bel_backspace_and_del_are_neutralised() {
    let source = "a\u{7}b\u{8}c\u{7f}d\n";
    let flat = flatten(source, 80);
    for c in ['\u{7}', '\u{8}', '\u{7f}'] {
        assert!(!flat.contains(c), "control char {:?} survived: {flat:?}", c);
    }
    assert!(flat.contains('a') && flat.contains('d'));
}

#[test]
fn an_escape_sequence_inside_a_fenced_code_block_is_neutralised() {
    // Syntax highlighting is a different code path (syntect, via
    // `render::code`) than plain paragraph text, so it needs its own check.
    let source = "```\nfn main() { \u{1b}[31m\"pwned\"\u{1b}[0m }\n```\n";
    let flat = flatten(source, 80);
    assert!(!flat.contains('\u{1b}'), "escape survived: {flat:?}");
    assert!(flat.contains("pwned"), "payload lost: {flat:?}");

    // Same, but with a recognised language so it goes through syntect's
    // highlighter rather than the plain-text fallback.
    let source = "```rust\nfn main() { \u{1b}[31m\"pwned\"\u{1b}[0m }\n```\n";
    let flat = flatten(source, 80);
    assert!(
        !flat.contains('\u{1b}'),
        "escape survived syntax highlighting: {flat:?}"
    );
    assert!(flat.contains("pwned"), "payload lost: {flat:?}");
}

#[test]
fn an_escape_sequence_inside_a_table_cell_is_neutralised() {
    let source = "| a | b |\n|---|---|\n| \u{1b}[31mred\u{1b}[0m | plain |\n";
    let flat = flatten(source, 80);
    assert!(!flat.contains('\u{1b}'), "escape survived: {flat:?}");
    assert!(flat.contains("red"));
    assert!(flat.contains("plain"));
}

#[test]
fn escape_sequences_in_headings_links_and_images_are_neutralised() {
    let source = "# \u{1b}[31mHeading\u{1b}[0m\n\n\
                  [\u{1b}[31mlink text\u{1b}[0m](./a.md)\n\n\
                  ![\u{1b}[31malt text\u{1b}[0m](./a.png)\n";
    let flat = flatten(source, 80);
    assert!(!flat.contains('\u{1b}'), "escape survived: {flat:?}");
    assert!(flat.contains("Heading"));
    assert!(flat.contains("link text"));
    assert!(flat.contains("alt text"));
}

#[test]
fn normal_text_with_emoji_cjk_and_accents_is_byte_for_byte_unchanged() {
    // The test that stops the fix from being overzealous: nothing here is a
    // control character, so none of it should be touched.
    let payload = "Café Münster 漢字語 🎉🚀 plain prose, with punctuation!";
    let source = format!("{payload}\n");
    let flat = flatten(&source, 200);
    assert_eq!(flat.trim_end(), payload);
}

#[test]
fn no_rendered_line_contains_a_control_character_our_own_layout_did_not_emit() {
    // Property-style check across every hostile fixture used above, plus a
    // combined document that hits every block kind at once. Our own layout
    // never puts a control character *inside* a span's text — `\n` only
    // ever appears as a boundary between `Line`s, never embedded in one —
    // so every span's content must be entirely free of `char::is_control()`.
    let fixtures = [
        "\u{1b}[31mRED\u{1b}[0m\n",
        "\u{1b}]0;pwned\u{7}\n",
        "a\u{7}b\u{8}c\u{7f}d\n",
        "```\n\u{1b}[31mcode\u{1b}[0m\n```\n",
        "```rust\nfn f() { \u{1b}[31m1\u{1b}[0m }\n```\n",
        "| \u{1b}[31mh\u{1b}[0m |\n|---|\n| \u{1b}[31mx\u{1b}[0m |\n",
        "# \u{1b}[31mH\u{1b}[0m\n\n\
         > \u{1b}[31mquoted\u{1b}[0m\n\n\
         - \u{1b}[31mitem\u{1b}[0m\n\n\
         [\u{1b}[31ml\u{1b}[0m](x)\n\n\
         ![\u{1b}[31ma\u{1b}[0m](x)\n\n\
         `\u{1b}[31minline\u{1b}[0m`\n",
    ];

    for src in fixtures {
        let document = doc::parse(src);
        for width in [10u16, 40, 80] {
            let rendered = render::render(&document, width, &theme::DARK);
            for line in &rendered.lines {
                for span in &line.spans {
                    for ch in span.content.chars() {
                        assert!(
                            !ch.is_control(),
                            "control char {:?} leaked into a span for fixture {:?} at width {width}: {:?}",
                            ch,
                            src,
                            span.content
                        );
                    }
                }
            }
        }
    }
}
