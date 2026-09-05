use crate::app::mode::Mode;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    ScrollLines(i32),
    ScrollHalfPage(i32),
    ScrollPage(i32),
    Top,
    Bottom,
    Reload,
    Help,
    Dismiss,
    /// Open the `/` prompt.
    SearchStart,
    /// A character typed into the open prompt.
    SearchInput(char),
    SearchBackspace,
    /// Enter: run the typed query.
    SearchCommit,
    NextMatch,
    PrevMatch,
    None,
}

pub fn map_key(key: KeyEvent) -> Action {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match (key.code, ctrl) {
        (KeyCode::Char('c'), true) => Action::Quit,
        // Any other Ctrl-modified key is reserved for later phases.
        (_, true) => Action::None,
        (KeyCode::Char('q'), _) => Action::Quit,
        // Esc must NOT quit. The design gives it exactly one job — back out of
        // the current mode. The help overlay is the first mode that job
        // applies to: Esc dismisses it. Binding Esc to Quit would mean that
        // dismissing a future mode (e.g. a search box) could end the session.
        (KeyCode::Esc, _) => Action::Dismiss,
        (KeyCode::Char('j'), _) | (KeyCode::Down, _) => Action::ScrollLines(1),
        (KeyCode::Char('k'), _) | (KeyCode::Up, _) => Action::ScrollLines(-1),
        (KeyCode::Char('d'), _) => Action::ScrollHalfPage(1),
        (KeyCode::Char('u'), _) => Action::ScrollHalfPage(-1),
        (KeyCode::Char(' '), _) | (KeyCode::PageDown, _) => Action::ScrollPage(1),
        (KeyCode::PageUp, _) => Action::ScrollPage(-1),
        (KeyCode::Char('g'), _) | (KeyCode::Home, _) => Action::Top,
        (KeyCode::Char('G'), _) | (KeyCode::End, _) => Action::Bottom,
        (KeyCode::Char('r'), _) => Action::Reload,
        (KeyCode::Char('/'), _) => Action::SearchStart,
        (KeyCode::Char('n'), _) => Action::NextMatch,
        (KeyCode::Char('N'), _) => Action::PrevMatch,
        (KeyCode::Char('?'), _) => Action::Help,
        _ => Action::None,
    }
}

/// Keys while the `/` prompt is open.
///
/// This cannot share `map_key`'s table: in the prompt every printable
/// character is text, so `q` is a letter rather than a quit and `/` is a
/// slash rather than a second prompt. Ctrl-C stays bound so nothing the
/// user can type traps them here.
pub fn map_prompt_key(key: KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') => Action::Quit,
            _ => Action::None,
        };
    }
    match key.code {
        KeyCode::Char(c) => Action::SearchInput(c),
        KeyCode::Backspace => Action::SearchBackspace,
        KeyCode::Enter => Action::SearchCommit,
        KeyCode::Esc => Action::Dismiss,
        _ => Action::None,
    }
}

/// How many lines one wheel notch moves. Three is the conventional step and
/// matches what every other terminal pager does.
const WHEEL_LINES: i32 = 3;

/// The wheel, and only the wheel: clicks and drags do nothing, so mouse
/// capture buys exactly one feature and no surprises. Ignored in every mode
/// but `Reading`, matching how the scroll keys are handled there.
pub fn map_mouse(event: MouseEvent, mode: &Mode) -> Action {
    if !mode.is_reading() {
        return Action::None;
    }
    match event.kind {
        MouseEventKind::ScrollDown => Action::ScrollLines(WHEEL_LINES),
        MouseEventKind::ScrollUp => Action::ScrollLines(-WHEEL_LINES),
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn q_and_ctrl_c_quit() {
        assert_eq!(map_key(key('q')), Action::Quit);
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::Quit
        );
    }

    #[test]
    fn vim_and_arrow_keys_scroll_by_one_line() {
        assert_eq!(map_key(key('j')), Action::ScrollLines(1));
        assert_eq!(map_key(key('k')), Action::ScrollLines(-1));
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            Action::ScrollLines(1)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            Action::ScrollLines(-1)
        );
    }

    #[test]
    fn half_and_full_page_scrolling() {
        assert_eq!(map_key(key('d')), Action::ScrollHalfPage(1));
        assert_eq!(map_key(key('u')), Action::ScrollHalfPage(-1));
        assert_eq!(map_key(key(' ')), Action::ScrollPage(1));
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
            Action::ScrollPage(1)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)),
            Action::ScrollPage(-1)
        );
    }

    #[test]
    fn g_goes_to_top_and_shift_g_to_bottom() {
        assert_eq!(map_key(key('g')), Action::Top);
        assert_eq!(map_key(key('G')), Action::Bottom);
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)),
            Action::Top
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
            Action::Bottom
        );
    }

    #[test]
    fn reload_and_help_are_bound() {
        assert_eq!(map_key(key('r')), Action::Reload);
        assert_eq!(map_key(key('?')), Action::Help);
    }

    #[test]
    fn esc_dismisses_rather_than_quits() {
        // Esc is reserved for dismissing a mode, never for quitting. If this
        // ever maps to Quit, closing an overlay would end the session instead.
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Action::Dismiss
        );
    }

    #[test]
    fn unbound_keys_produce_none() {
        assert_eq!(map_key(key('z')), Action::None);
    }

    #[test]
    fn ctrl_modified_letters_do_not_trigger_plain_bindings() {
        // Ctrl-D must not be read as the plain `d` half-page binding; later
        // phases bind Ctrl-D separately.
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)),
            Action::None
        );
    }

    #[test]
    fn slash_opens_search_and_n_steps_through_matches() {
        assert_eq!(map_key(key('/')), Action::SearchStart);
        assert_eq!(map_key(key('n')), Action::NextMatch);
        assert_eq!(map_key(key('N')), Action::PrevMatch);
    }

    #[test]
    fn prompt_keys_type_delete_commit_and_cancel() {
        assert_eq!(map_prompt_key(key('q')), Action::SearchInput('q'));
        assert_eq!(map_prompt_key(key('/')), Action::SearchInput('/'));
        assert_eq!(map_prompt_key(key('N')), Action::SearchInput('N'));
        assert_eq!(
            map_prompt_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
            Action::SearchBackspace
        );
        assert_eq!(
            map_prompt_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Action::SearchCommit
        );
        assert_eq!(
            map_prompt_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Action::Dismiss
        );
    }

    #[test]
    fn ctrl_c_still_quits_from_the_prompt() {
        // Nothing the user can type should trap them in the prompt.
        assert_eq!(
            map_prompt_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::Quit
        );
    }

    #[test]
    fn other_ctrl_and_unbound_keys_do_nothing_in_the_prompt() {
        assert_eq!(
            map_prompt_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)),
            Action::None
        );
        assert_eq!(
            map_prompt_key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)),
            Action::None
        );
    }

    use crate::app::mode::Mode;
    use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    fn wheel(kind: MouseEventKind) -> MouseEvent {
        MouseEvent {
            kind,
            column: 10,
            row: 5,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn the_wheel_scrolls_three_lines_at_a_time() {
        assert_eq!(
            map_mouse(wheel(MouseEventKind::ScrollDown), &Mode::Reading),
            Action::ScrollLines(3)
        );
        assert_eq!(
            map_mouse(wheel(MouseEventKind::ScrollUp), &Mode::Reading),
            Action::ScrollLines(-3)
        );
    }

    #[test]
    fn clicks_drags_and_sideways_scrolling_do_nothing() {
        for kind in [
            MouseEventKind::Down(MouseButton::Left),
            MouseEventKind::Up(MouseButton::Left),
            MouseEventKind::Drag(MouseButton::Left),
            MouseEventKind::Moved,
            MouseEventKind::ScrollLeft,
            MouseEventKind::ScrollRight,
        ] {
            assert_eq!(map_mouse(wheel(kind), &Mode::Reading), Action::None);
        }
    }

    #[test]
    fn the_wheel_is_ignored_in_every_mode_but_reading() {
        // Same rule the scroll keys follow: overlays and the prompt freeze
        // the document underneath.
        for mode in [
            Mode::Help,
            Mode::SearchPrompt {
                query: String::new(),
            },
        ] {
            assert_eq!(
                map_mouse(wheel(MouseEventKind::ScrollDown), &mode),
                Action::None
            );
            assert_eq!(
                map_mouse(wheel(MouseEventKind::ScrollUp), &mode),
                Action::None
            );
        }
    }
}
