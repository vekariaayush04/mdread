use clap::Parser;
use mdread::app::action::{Action, map_key};
use mdread::app::state::App;
use mdread::cli::Cli;
use mdread::term::{RealTerm, TerminalGuard, install_panic_hook};
use mdread::{config, theme};
use ratatui::crossterm::event::{self, Event, KeyEventKind};
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

    let app = App::new(settings, theme);
    if let Some(path) = cli.file.as_deref() {
        // Fail fast on a missing file rather than opening a blank UI.
        if !path.exists() {
            anyhow::bail!("no such file: {}", path.display());
        }
    }

    install_panic_hook();
    let mut guard = TerminalGuard::enter(RealTerm)?;
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, app, cli.file.clone());
    ratatui::restore();
    guard.leave()?;
    result
}

fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    mut app: App,
    file: Option<std::path::PathBuf>,
) -> anyhow::Result<()> {
    // Establish geometry before the first open so the document is laid out
    // at the real measure rather than at zero width.
    let size = terminal.size()?;
    app.set_geometry(size.width, size.height);
    if let Some(path) = &file {
        app.open_path(path);
    }

    loop {
        let size = terminal.size()?;
        app.set_geometry(size.width, size.height);
        terminal.draw(|f| mdread::ui::draw(f, &app))?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            let action = map_key(key);
            if action != Action::None {
                app.apply(action);
            }
        }
        if app.should_quit {
            return Ok(());
        }
    }
}
