/// Which input mode the reader is in.
///
/// Flat, not a stack: `Esc` always returns to `Reading`, so the user can
/// never get lost several layers deep in modes they did not know they had
/// entered. Anything that is not `Reading` suspends the document — the same
/// contract the help overlay has had since P0.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Reading,
    Help,
    /// The `/` prompt. The buffer lives inside the variant so it cannot
    /// outlive the mode, and so a cancelled search leaves nothing behind
    /// to clean up.
    SearchPrompt {
        query: String,
    },
}

impl Mode {
    pub fn is_reading(&self) -> bool {
        matches!(self, Self::Reading)
    }

    pub fn is_help(&self) -> bool {
        matches!(self, Self::Help)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_is_the_default_mode() {
        assert_eq!(Mode::default(), Mode::Reading);
        assert!(Mode::default().is_reading());
    }

    #[test]
    fn each_mode_reports_only_itself() {
        assert!(Mode::Reading.is_reading());
        assert!(!Mode::Reading.is_help());
        assert!(Mode::Help.is_help());
        assert!(!Mode::Help.is_reading());

        let prompt = Mode::SearchPrompt {
            query: "abc".to_string(),
        };
        assert!(!prompt.is_reading());
        assert!(!prompt.is_help());
    }

    #[test]
    fn the_prompt_carries_its_own_buffer() {
        let mut mode = Mode::SearchPrompt {
            query: String::new(),
        };
        if let Mode::SearchPrompt { query } = &mut mode {
            query.push('h');
            query.push('i');
        }
        assert_eq!(
            mode,
            Mode::SearchPrompt {
                query: "hi".to_string()
            }
        );
    }
}
