use mdread::{doc, render, theme};

/// Flatten a rendered document to plain text. Colour is asserted by the
/// targeted unit tests; snapshotting every style makes snapshots that
/// nobody re-reads and that break on every palette tweak.
fn render_to_text(source: &str, width: u16, theme: &theme::Theme) -> String {
    let document = doc::parse(source);
    render::render(&document, width, theme)
        .lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

macro_rules! fixture_snapshots {
    ($($name:ident => $file:literal),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                let source = include_str!(concat!("fixtures/", $file));
                for width in [60u16, 80, 120] {
                    insta::assert_snapshot!(
                        format!("{}_{}", stringify!($name), width),
                        render_to_text(source, width, &theme::DARK)
                    );
                }
            }
        )*
    };
}

fixture_snapshots! {
    prose => "prose.md",
    lists => "lists.md",
    tables => "tables.md",
    code => "code.md",
    kitchen_sink => "kitchen-sink.md",
}

#[test]
fn themes_do_not_change_the_text_layout() {
    // Only colour should differ between themes. If a theme changes the
    // glyphs, something is wrong with the layout code.
    let source = include_str!("fixtures/kitchen-sink.md");
    let dark = render_to_text(source, 80, &theme::DARK);
    for name in theme::names() {
        let t = theme::by_name(name).unwrap();
        assert_eq!(
            render_to_text(source, 80, t),
            dark,
            "theme {name} altered layout"
        );
    }
}

#[test]
fn no_line_ever_exceeds_the_requested_width() {
    use unicode_width::UnicodeWidthStr;
    let source = include_str!("fixtures/kitchen-sink.md");
    for width in [20u16, 40, 60, 80, 120] {
        for line in render_to_text(source, width, &theme::DARK).lines() {
            assert!(
                line.width() <= width as usize,
                "width {width}: line of {} columns: {line:?}",
                line.width()
            );
        }
    }
}
