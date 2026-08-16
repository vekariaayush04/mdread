use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

pub trait TermOps {
    fn enter(&mut self) -> io::Result<()>;
    fn leave(&mut self) -> io::Result<()>;
}

/// The real terminal: raw mode plus the alternate screen.
pub struct RealTerm;

impl TermOps for RealTerm {
    fn enter(&mut self) -> io::Result<()> {
        ratatui::crossterm::terminal::enable_raw_mode()?;
        ratatui::crossterm::execute!(
            io::stdout(),
            ratatui::crossterm::terminal::EnterAlternateScreen,
            ratatui::crossterm::cursor::Hide
        )
    }

    fn leave(&mut self) -> io::Result<()> {
        ratatui::crossterm::execute!(
            io::stdout(),
            ratatui::crossterm::cursor::Show,
            ratatui::crossterm::terminal::LeaveAlternateScreen
        )?;
        ratatui::crossterm::terminal::disable_raw_mode()
    }
}

/// Owns the terminal's modified state and guarantees restoration exactly once,
/// whether we exit normally, return an error, or unwind through a panic.
pub struct TerminalGuard<T: TermOps> {
    ops: T,
    active: bool,
}

impl<T: TermOps> TerminalGuard<T> {
    pub fn enter(mut ops: T) -> io::Result<Self> {
        ops.enter()?;
        Ok(Self { ops, active: true })
    }

    /// Restore the terminal. Safe to call more than once; later calls are no-ops.
    pub fn leave(&mut self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        self.ops.leave()
    }
}

impl<T: TermOps> Drop for TerminalGuard<T> {
    fn drop(&mut self) {
        // Never panic in Drop: doing so while already unwinding aborts.
        let _ = self.leave();
    }
}

static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Restore the terminal before the default panic handler prints, so a crash
/// never leaves the user in raw mode with a hidden cursor.
pub fn install_panic_hook() {
    if HOOK_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = RealTerm.leave();
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct Recorder {
        log: Rc<RefCell<Vec<&'static str>>>,
        fail_leave: bool,
    }

    impl TermOps for Recorder {
        fn enter(&mut self) -> io::Result<()> {
            self.log.borrow_mut().push("enter");
            Ok(())
        }
        fn leave(&mut self) -> io::Result<()> {
            self.log.borrow_mut().push("leave");
            if self.fail_leave {
                return Err(io::Error::other("boom"));
            }
            Ok(())
        }
    }

    #[test]
    fn entering_calls_enter_once() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let ops = Recorder {
            log: log.clone(),
            fail_leave: false,
        };
        let _guard = TerminalGuard::enter(ops).unwrap();
        assert_eq!(*log.borrow(), vec!["enter"]);
    }

    #[test]
    fn drop_restores_the_terminal() {
        let log = Rc::new(RefCell::new(Vec::new()));
        {
            let ops = Recorder {
                log: log.clone(),
                fail_leave: false,
            };
            let _guard = TerminalGuard::enter(ops).unwrap();
        }
        assert_eq!(*log.borrow(), vec!["enter", "leave"]);
    }

    #[test]
    fn explicit_leave_then_drop_restores_only_once() {
        // Double-restoring can re-enter raw mode on some terminals.
        let log = Rc::new(RefCell::new(Vec::new()));
        {
            let ops = Recorder {
                log: log.clone(),
                fail_leave: false,
            };
            let mut guard = TerminalGuard::enter(ops).unwrap();
            guard.leave().unwrap();
        }
        assert_eq!(*log.borrow(), vec!["enter", "leave"]);
    }

    #[test]
    fn leave_is_idempotent_when_called_twice_explicitly() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let ops = Recorder {
            log: log.clone(),
            fail_leave: false,
        };
        let mut guard = TerminalGuard::enter(ops).unwrap();
        guard.leave().unwrap();
        guard.leave().unwrap();
        assert_eq!(*log.borrow(), vec!["enter", "leave"]);
    }

    #[test]
    fn drop_swallows_leave_errors() {
        // Panicking inside Drop while already unwinding aborts the process.
        let log = Rc::new(RefCell::new(Vec::new()));
        {
            let ops = Recorder {
                log: log.clone(),
                fail_leave: true,
            };
            let _guard = TerminalGuard::enter(ops).unwrap();
        }
        assert_eq!(*log.borrow(), vec!["enter", "leave"]);
    }

    #[test]
    fn installing_the_panic_hook_is_idempotent() {
        install_panic_hook();
        install_panic_hook();
    }
}
