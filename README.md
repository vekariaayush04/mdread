# mdread

A terminal Markdown reader that renders a document beautifully and — in later
releases — lets you browse a whole tree of documents.

## Install

```
cargo install mdread
```

Or build from source:

```
git clone https://github.com/vekariaayush04/mdread
cd mdread
cargo install --path .
```

## Usage

```
mdread README.md
```

Options:

| Flag | Meaning |
|---|---|
| `-t`, `--theme <NAME>` | `dark`, `light`, or `high-contrast` |
| `--max-measure <COLS>` | Widest the text may become (default 80) |
| `--config <PATH>` | Use this config file instead of the default |

## Keymap

```
scroll     j/k  down/up        d/u half page    space page-down    g/G top/bottom
misc       r reload   ? help   q quit
```

Planned for later phases:

```
panes      t tree   o outline   Tab cycle focus   Esc dismiss
open       Enter — tree: open file · outline: jump to heading
links      f  link-hint mode
history    Backspace back    Alt-Right forward
search     /  in-doc    n/N next/prev match    Esc clear
find       Ctrl-P fuzzy file picker    Ctrl-F grep across tree
watch      w toggle watch
```

## Configuration

TOML at the platform config directory — `~/.config/mdread/config.toml` on
Linux. Every field is optional; mdread works with no config file at all.

```toml
# Colour theme: "dark", "light", or "high-contrast".
theme = "dark"

# The widest the document text may become, in columns. Text is centred
# within the pane at this measure.
max_measure = 80
```

Precedence, lowest to highest: built-in defaults, the config file, CLI flags.

## Status

Rendering covers headings, prose with emphasis, links, inline code, lists
(bullet, ordered, and task), block quotes, thematic breaks, GFM tables, and
syntax-highlighted fenced code blocks.

Planned: a file tree, link following with history, an outline pane, in-document
and cross-tree search, live reload, inline images, and diagram rendering.

## Licence

MIT. See [LICENSE](./LICENSE).
