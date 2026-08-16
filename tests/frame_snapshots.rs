use mdread::app::action::Action;
use mdread::app::state::App;
use mdread::config::Settings;
use mdread::theme;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::path::PathBuf;

/// Render a whole frame at a given terminal size and dump the buffer as text.
fn frame(width: u16, height: u16, source: &str) -> String {
    render(width, height, source, false)
}

/// Same as `frame`, but with the help overlay toggled on before drawing.
fn frame_with_help(width: u16, height: u16, source: &str) -> String {
    render(width, height, source, true)
}

fn render(width: u16, height: u16, source: &str, show_help: bool) -> String {
    let mut app = App::new(Settings::default(), &theme::DARK);
    app.set_geometry(width, height);
    app.open_source(PathBuf::from("fixture.md"), source);
    if show_help {
        app.apply(Action::Help);
    }

    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| mdread::ui::draw(f, &app)).unwrap();

    let buf = terminal.backend().buffer();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn frame_at_a_wide_terminal() {
    let source = include_str!("fixtures/prose.md");
    insta::assert_snapshot!(frame(120, 24, source));
}

#[test]
fn frame_at_a_standard_terminal() {
    let source = include_str!("fixtures/prose.md");
    insta::assert_snapshot!(frame(80, 24, source));
}

#[test]
fn frame_at_a_narrow_terminal() {
    let source = include_str!("fixtures/prose.md");
    insta::assert_snapshot!(frame(50, 24, source));
}

#[test]
fn frame_with_a_table_stays_inside_its_borders() {
    let source = include_str!("fixtures/tables.md");
    insta::assert_snapshot!(frame(80, 20, source));
}

#[test]
fn a_terminal_too_small_to_draw_does_not_panic() {
    for (w, h) in [(1u16, 1u16), (3, 2), (5, 4), (10, 3)] {
        let _ = frame(w, h, "# Hi\n\nbody\n");
    }
}

#[test]
fn frame_with_the_help_overlay_open() {
    let source = include_str!("fixtures/prose.md");
    insta::assert_snapshot!(frame_with_help(80, 24, source));
}

#[test]
fn the_help_overlay_does_not_panic_on_a_very_small_terminal() {
    for (w, h) in [(20u16, 8u16), (10, 5), (5, 3)] {
        let _ = frame_with_help(w, h, "# Hi\n\nbody\n");
    }
}
