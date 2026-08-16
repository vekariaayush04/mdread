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

    install_panic_hook();
    let mut guard = TerminalGuard::enter(RealTerm)?;
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, app);
    ratatui::restore();
    guard.leave()?;
    result
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, mut app: App) -> anyhow::Result<()> {
    loop {
        let size = terminal.size()?;
        app.viewport_height = size.height.saturating_sub(3);
        terminal.draw(|f| {
            // Task 18 replaces this with mdread::ui::draw(f, &app).
            f.render_widget(ratatui::widgets::Paragraph::new("mdread"), f.area());
        })?;

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
