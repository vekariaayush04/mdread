use clap::Parser;
use mdread::app::action::{Action, map_key, map_prompt_key};
use mdread::app::mode::Mode;
use mdread::app::state::App;
use mdread::cli::Cli;
use mdread::term::{RealTerm, TerminalGuard, install_panic_hook};
use mdread::{config, theme};
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use std::io::{IsTerminal, Read};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

const EXIT_USAGE: u8 = 2;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Startup failures print to stderr and never enter raw mode.
            eprintln!("mdread: {e}");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let file = config::load(cli.config.as_deref())?;
    let settings = config::resolve(file, cli.theme.clone(), cli.max_measure);

    let theme = theme::by_name(&settings.theme).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown theme {:?}; available: {}",
            settings.theme,
            theme::names().join(", ")
        )
    })?;

    let source = resolve_source(cli.file.clone())?;
    let app = App::new(settings, theme);

    install_panic_hook();
    let mut guard = TerminalGuard::enter(RealTerm)?;
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, app, source);
    ratatui::restore();
    guard.leave()?;
    result
}

/// Where the document comes from.
#[derive(Debug)]
enum Source {
    File(PathBuf),
    Stdin(String),
}

/// Decide what to open, and read stdin if that is the answer.
///
/// This must run *before* raw mode is entered. crossterm reads key events
/// from `/dev/tty` on Unix rather than from fd 0, so draining stdin first
/// leaves the keyboard working; draining it afterwards would not.
fn resolve_source(file: Option<PathBuf>) -> anyhow::Result<Source> {
    if let Some(path) = file {
        // Fail fast on a missing file rather than opening a blank UI. A
        // file argument wins even when something is also piped in.
        if !path.exists() {
            anyhow::bail!("no such file: {}", path.display());
        }
        return Ok(Source::File(path));
    }

    if std::io::stdin().is_terminal() {
        anyhow::bail!("no file given and stdin is a terminal");
    }

    let mut buf = Vec::new();
    std::io::stdin().read_to_end(&mut buf)?;
    let text = String::from_utf8(buf).map_err(|_| anyhow::anyhow!("stdin is not valid UTF-8"))?;
    Ok(Source::Stdin(text))
}

fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    mut app: App,
    source: Source,
) -> anyhow::Result<()> {
    // Establish geometry before the first open so the document is laid out
    // at the real measure rather than at zero width.
    let size = terminal.size()?;
    app.set_geometry(size.width, size.height);
    match &source {
        Source::File(path) => app.open_path(path),
        Source::Stdin(text) => app.open_stdin(text),
    }

    loop {
        let size = terminal.size()?;
        app.set_geometry(size.width, size.height);
        terminal.draw(|f| mdread::ui::draw(f, &app))?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            // The prompt reads every printable character as text, so it
            // needs its own key table.
            let action = match &app.mode {
                Mode::SearchPrompt { .. } => map_prompt_key(key),
                _ => map_key(key),
            };
            if action != Action::None {
                app.apply(action);
            }
        }
        if app.should_quit {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_argument_resolves_to_that_file() {
        // Tests run with the package root as the working directory.
        match resolve_source(Some(PathBuf::from("Cargo.toml"))).unwrap() {
            Source::File(p) => assert_eq!(p, PathBuf::from("Cargo.toml")),
            Source::Stdin(_) => panic!("a file argument must win over stdin"),
        }
    }

    #[test]
    fn a_missing_file_fails_before_the_terminal_is_touched() {
        let err = resolve_source(Some(PathBuf::from("/nonexistent/nope.md"))).unwrap_err();
        assert_eq!(err.to_string(), "no such file: /nonexistent/nope.md");
    }
}
