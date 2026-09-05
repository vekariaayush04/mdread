use mdread::app::action::Action;
use mdread::app::state::App;
use mdread::config::Settings;
use mdread::theme;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::path::PathBuf;

fn draw_app(app: &App, width: u16, height: u16) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| mdread::ui::draw(f, app)).unwrap();
    terminal
}

/// The buffer's characters, one row per line.
fn symbols(terminal: &Terminal<TestBackend>) -> String {
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

/// Which cells the search repainted: `#` for the current match, `~` for the
/// others, `.` for everything else. Styles are invisible in a character
/// dump, so highlighting needs its own snapshot.
fn highlight_mask(terminal: &Terminal<TestBackend>) -> String {
    let buf = terminal.backend().buffer();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| {
                    let bg = Some(buf[(x, y)].bg);
                    if bg == theme::DARK.search_current.bg {
                        '#'
                    } else if bg == theme::DARK.search_match.bg {
                        '~'
                    } else {
                        '.'
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn app_for(width: u16, height: u16, source: &str) -> App {
    let mut app = App::new(Settings::default(), &theme::DARK);
    app.set_geometry(width, height);
    app.open_source(PathBuf::from("fixture.md"), source);
    app
}

fn type_query(app: &mut App, query: &str) {
    app.apply(Action::SearchStart);
    for c in query.chars() {
        app.apply(Action::SearchInput(c));
    }
}

/// Render a whole frame at a given terminal size and dump the buffer as text.
fn frame(width: u16, height: u16, source: &str) -> String {
    let app = app_for(width, height, source);
    symbols(&draw_app(&app, width, height))
}

/// Same as `frame`, but with the help overlay toggled on before drawing.
fn frame_with_help(width: u16, height: u16, source: &str) -> String {
    let mut app = app_for(width, height, source);
    app.apply(Action::Help);
    symbols(&draw_app(&app, width, height))
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

#[test]
fn search_highlights_every_match_and_marks_the_current_one() {
    let source = include_str!("fixtures/prose.md");
    let mut app = app_for(80, 24, source);
    type_query(&mut app, "paragraph");
    app.apply(Action::SearchCommit);
    let terminal = draw_app(&app, 80, 24);

    insta::assert_snapshot!("search_frame", symbols(&terminal));
    insta::assert_snapshot!("search_mask", highlight_mask(&terminal));
}

#[test]
fn search_highlights_a_match_that_crosses_styled_spans() {
    // "sis, strong" runs from the italic "emphasis", through plain text,
    // into the bold "strong text": three spans, one highlight.
    let source = include_str!("fixtures/prose.md");
    let mut app = app_for(80, 24, source);
    type_query(&mut app, "sis, strong");
    app.apply(Action::SearchCommit);
    insta::assert_snapshot!(highlight_mask(&draw_app(&app, 80, 24)));
}

#[test]
fn the_search_prompt_owns_the_status_row() {
    let source = include_str!("fixtures/prose.md");
    let mut app = app_for(80, 24, source);
    type_query(&mut app, "para");
    insta::assert_snapshot!(symbols(&draw_app(&app, 80, 24)));
}

#[test]
fn a_query_that_matches_nothing_says_so_in_the_status_row() {
    let source = include_str!("fixtures/prose.md");
    let mut app = app_for(80, 24, source);
    type_query(&mut app, "zzzznotpresent");
    app.apply(Action::SearchCommit);
    insta::assert_snapshot!(symbols(&draw_app(&app, 80, 24)));
}

#[test]
fn searching_on_a_tiny_terminal_does_not_panic() {
    for (w, h) in [(1u16, 1u16), (3, 2), (5, 4), (10, 3)] {
        let mut app = app_for(w, h, "# Hi\n\nbody with a needle\n");
        type_query(&mut app, "needle");
        app.apply(Action::SearchCommit);
        let _ = symbols(&draw_app(&app, w, h));
    }
}
