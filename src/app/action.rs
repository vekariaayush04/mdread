use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
    None,
}

pub fn map_key(key: KeyEvent) -> Action {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match (key.code, ctrl) {
        (KeyCode::Char('c'), true) => Action::Quit,
        // Any other Ctrl-modified key is reserved for later phases.
        (_, true) => Action::None,
        (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => Action::Quit,
        (KeyCode::Char('j'), _) | (KeyCode::Down, _) => Action::ScrollLines(1),
        (KeyCode::Char('k'), _) | (KeyCode::Up, _) => Action::ScrollLines(-1),
        (KeyCode::Char('d'), _) => Action::ScrollHalfPage(1),
        (KeyCode::Char('u'), _) => Action::ScrollHalfPage(-1),
        (KeyCode::Char(' '), _) | (KeyCode::PageDown, _) => Action::ScrollPage(1),
        (KeyCode::PageUp, _) => Action::ScrollPage(-1),
        (KeyCode::Char('g'), _) | (KeyCode::Home, _) => Action::Top,
        (KeyCode::Char('G'), _) | (KeyCode::End, _) => Action::Bottom,
        (KeyCode::Char('r'), _) => Action::Reload,
        (KeyCode::Char('?'), _) => Action::Help,
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
}
