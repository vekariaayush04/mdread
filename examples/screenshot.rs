//! Renders a demo document through the real UI and prints the frame as text.
//!
//! This is what generates the screenshot in the README. Regenerate with:
//!
//! ```sh
//! cargo run --example screenshot
//! ```
//!
//! Keeping it as an example rather than pasting a screenshot by hand means the
//! README cannot drift away from what the renderer actually produces.

use mdread::app::state::App;
use mdread::config::Settings;
use mdread::theme;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::path::PathBuf;

const DEMO: &str = r#"# mdread

A terminal Markdown reader. Point it at a file and read.

## Features

- Syntax-highlighted code blocks
- GFM tables with real column layout
- Task lists, quotes, and nested lists

```rust
fn main() {
    println!("hello from a highlighted code block");
}
```

| Flag | Meaning |
|:-----|:--------|
| `-t` | Pick a theme |
| `--max-measure` | Widest the text gets |

> Text is centred at a readable measure, so prose never
> stretches across a wide terminal.
"#;

fn main() {
    let width = 78;
    let height = 26;

    let mut app = App::new(Settings::default(), &theme::DARK);
    app.set_geometry(width, height);
    app.open_source(PathBuf::from("demo.md"), DEMO);

    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| mdread::ui::draw(f, &app)).unwrap();

    let buf = terminal.backend().buffer();
    for y in 0..buf.area.height {
        let row: String = (0..buf.area.width)
            .map(|x| buf[(x, y)].symbol())
            .collect::<Vec<_>>()
            .join("");
        // Trailing spaces are invisible but bloat the README diff.
        println!("{}", row.trim_end());
    }
}
