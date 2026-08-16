//! Defence-in-depth against terminal escape-sequence injection.
//!
//! A Markdown file is attacker-controlled text: nothing stops it from
//! containing a raw ANSI escape (`\x1b[31m`), an OSC title-rewrite
//! (`\x1b]0;pwned\x07`), a bell, a backspace, or any other C0/C1 control
//! character. ratatui happens to drop those at its cell-buffer boundary
//! today, so nothing reaches the real terminal — but that is incidental
//! behaviour of a dependency, not a guarantee this crate controls. If
//! ratatui's behaviour ever changes, or a future code path writes text to
//! the terminal without going through the buffer, that protection
//! disappears. The correct place to defend is our own boundary: where
//! document text is turned into `Span` content.

/// Remove every C0 control character (U+0000-U+001F), DEL (U+007F), and C1
/// control character (U+0080-U+009F) from `s`, except any characters listed
/// in `keep` — for callers where a particular control is structurally
/// load-bearing (a fenced code block still needs `\n` to split into source
/// lines, and `\t` ahead of tab expansion).
///
/// Characters are deleted outright, not replaced with a visible placeholder
/// (`?`, U+FFFD, etc). Two reasons:
///
/// - `unicode-width` already scores every control character as zero columns
///   (`char::width()` returns `None` for them, and every width computation
///   in this codebase — including `render::table`'s column sizing, which
///   measures the *raw, unsanitised* document text before this function
///   ever runs) treats that as 0. Deleting the character keeps that
///   zero-width contract intact end to end, so nothing computed against the
///   original text drifts out of sync with what the sanitised text actually
///   renders as. A visible placeholder would need every such
///   pre-measurement site rewritten to sanitise first, purely so a stray
///   control byte could be shown as one more character that tells the
///   reader nothing about what it actually was.
/// - A control character reaching this function is not meaningful content —
///   it is either a mistake in how the file was produced or an attempt to
///   manipulate the terminal. Silently dropping it is less confusing for a
///   reader than replacing it with a glyph that looks like it was meant to
///   be there.
pub fn strip_controls(s: &str, keep: &[char]) -> String {
    s.chars()
        .filter(|c| !c.is_control() || keep.contains(c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_c0_controls() {
        assert_eq!(strip_controls("a\u{1b}b", &[]), "ab");
        assert_eq!(strip_controls("\u{7}bell", &[]), "bell");
        assert_eq!(strip_controls("back\u{8}space", &[]), "backspace");
    }

    #[test]
    fn removes_del_and_c1_controls() {
        assert_eq!(strip_controls("a\u{7f}b", &[]), "ab");
        assert_eq!(strip_controls("a\u{9b}b", &[]), "ab"); // C1 CSI
    }

    #[test]
    fn removes_a_full_osc_title_rewrite_sequence() {
        // \x1b ] 0 ; pwned \x07 — only the ESC and BEL are controls; the
        // rest is ordinary printable text that must survive.
        assert_eq!(strip_controls("\u{1b}]0;pwned\u{7}", &[]), "]0;pwned");
    }

    #[test]
    fn keep_list_preserves_named_controls() {
        assert_eq!(strip_controls("a\nb\tc", &['\n', '\t']), "a\nb\tc");
        assert_eq!(strip_controls("a\u{1b}b\nc", &['\n']), "ab\nc");
    }

    #[test]
    fn ordinary_text_is_untouched() {
        let s = "Café 漢字 🎉 plain text";
        assert_eq!(strip_controls(s, &[]), s);
    }
}
