# Changelog

All notable changes to mdread are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.1] - 2026-09-13

### Added

- Static Linux builds for `x86_64` and `aarch64` (musl), so the prebuilt
  binaries run on Alpine and on distributions with an older glibc.
- A Homebrew tap: `brew install vekariaayush04/tap/mdread`.

## [0.2.0] - 2026-09-06

### Added

- In-document search. `/` opens a prompt, Enter runs it, `n` and `N` step
  through the hits with wrap-around, and `Esc` clears. Every match is
  highlighted and the current one is drawn in a stronger colour; the status
  bar shows `[k/n]` or `Pattern not found`. Matching is case-insensitive over
  the rendered text, so a hit is always something you can see. The search
  survives reload and resize.
- Reading from stdin. `cat file.md | mdread` works as a pager; the document
  is labelled `(stdin)` and cannot be reloaded.
- Mouse-wheel scrolling, three lines per notch. Clicks and drags do nothing.
- Prebuilt binaries for Linux, macOS, and Windows on every release, with
  one-line shell and PowerShell installers.

### Changed

- Running `mdread` with no file and no piped input now prints an error and
  exits with status 2 instead of opening an empty window.
- `Esc` outside the help overlay clears the active search.

## [0.1.0] - 2026-08-16

### Added

- First release: a terminal Markdown reader with syntax-highlighted code
  blocks, GFM tables, task lists, nested lists, block quotes, links, and
  three colour themes. Text is centred at a configurable measure and
  re-wraps on resize without losing your place. `r` reloads from disk.

[Unreleased]: https://github.com/vekariaayush04/mdread/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/vekariaayush04/mdread/releases/tag/v0.2.1
[0.2.0]: https://github.com/vekariaayush04/mdread/releases/tag/v0.2.0
[0.1.0]: https://crates.io/crates/mdread/0.1.0
