# mdread

A terminal Markdown reader. Point it at a file and read.

## Features

- Syntax-highlighted code blocks
- GFM tables with real column layout
- Search with `/`, stdin as a pager, mouse-wheel scrolling

```rust
fn main() {
    println!("hello from a highlighted code block");
}
```

| Flag | Meaning |
|---|---|
| `-t` | Pick a theme |
| `--max-measure` | Widest the text gets |

## Reading

Text is set at a comfortable measure and re-wraps when the terminal is
resized, without losing your place. Headings, emphasis, **strong text**,
~~strikethrough~~, and [links](https://example.com) all render the way
you would expect.

> Block quotes keep their bar and wrap inside it, and a highlighted
> search hit stays readable on top of any of these styles.

## Search

Type `/` and a word, press Enter, and every occurrence lights up. `n` and
`N` move between them and wrap around the document. The status bar shows
which hit you are on, so a long document never feels like a haystack.

Piping works too: `git log --stat | mdread` reads the document from stdin
and shows it exactly like a file, minus reload.

## Themes

Three built-in themes, `dark`, `light`, and `high-contrast`, chosen with
`--theme` or from a tiny TOML config. Every field is optional and mdread
runs fine with no config file at all.
