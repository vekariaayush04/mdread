use ratatui::style::Color;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub name: &'static str,
    pub text: Color,
    pub muted: Color,
    pub heading: Color,
    pub link: Color,
    pub code_fg: Color,
    pub code_bg: Color,
    pub quote_bar: Color,
    pub rule: Color,
    pub border: Color,
    pub accent: Color,
    /// Name of a syntect bundled theme. Must be one of the seven that
    /// `ThemeSet::load_defaults()` actually provides.
    pub syntect_theme: &'static str,
}

pub const DARK: Theme = Theme {
    name: "dark",
    text: Color::Rgb(0xd8, 0xde, 0xe9),
    muted: Color::Rgb(0x7f, 0x8c, 0x98),
    heading: Color::Rgb(0x88, 0xc0, 0xd0),
    link: Color::Rgb(0x81, 0xa1, 0xc1),
    code_fg: Color::Rgb(0xd8, 0xde, 0xe9),
    code_bg: Color::Rgb(0x2e, 0x34, 0x40),
    quote_bar: Color::Rgb(0x5e, 0x81, 0xac),
    rule: Color::Rgb(0x4c, 0x56, 0x6a),
    border: Color::Rgb(0x4c, 0x56, 0x6a),
    accent: Color::Rgb(0xa3, 0xbe, 0x8c),
    syntect_theme: "base16-ocean.dark",
};

pub const LIGHT: Theme = Theme {
    name: "light",
    text: Color::Rgb(0x2e, 0x34, 0x40),
    muted: Color::Rgb(0x6b, 0x74, 0x80),
    heading: Color::Rgb(0x00, 0x5c, 0x7a),
    link: Color::Rgb(0x00, 0x50, 0xb3),
    code_fg: Color::Rgb(0x2e, 0x34, 0x40),
    code_bg: Color::Rgb(0xec, 0xef, 0xf4),
    quote_bar: Color::Rgb(0x81, 0xa1, 0xc1),
    rule: Color::Rgb(0xc4, 0xcb, 0xd6),
    border: Color::Rgb(0xc4, 0xcb, 0xd6),
    accent: Color::Rgb(0x2d, 0x6a, 0x4f),
    syntect_theme: "InspiredGitHub",
};

/// Uses the 16-colour ANSI palette rather than RGB so it honours the
/// user's terminal palette, which is what high-contrast users configure.
pub const HIGH_CONTRAST: Theme = Theme {
    name: "high-contrast",
    text: Color::White,
    muted: Color::Gray,
    heading: Color::Yellow,
    link: Color::Cyan,
    code_fg: Color::White,
    code_bg: Color::Black,
    quote_bar: Color::Yellow,
    rule: Color::White,
    border: Color::White,
    accent: Color::Green,
    syntect_theme: "base16-eighties.dark",
};

const ALL: &[&Theme] = &[&DARK, &LIGHT, &HIGH_CONTRAST];
const NAMES: &[&str] = &["dark", "light", "high-contrast"];

pub fn by_name(name: &str) -> Option<&'static Theme> {
    let lower = name.to_ascii_lowercase();
    ALL.iter().copied().find(|t| t.name == lower)
}

pub fn names() -> &'static [&'static str] {
    NAMES
}

#[cfg(test)]
mod tests {
    use super::*;
    use syntect::highlighting::ThemeSet;

    #[test]
    fn lookup_is_case_insensitive() {
        assert_eq!(by_name("DARK").unwrap().name, "dark");
        assert_eq!(by_name("Light").unwrap().name, "light");
    }

    #[test]
    fn lookup_rejects_unknown_names() {
        assert!(by_name("dracula").is_none());
    }

    #[test]
    fn names_lists_every_theme() {
        assert_eq!(names(), &["dark", "light", "high-contrast"]);
        for n in names() {
            assert!(by_name(n).is_some(), "{n} listed but not resolvable");
        }
    }

    #[test]
    fn every_syntect_theme_reference_actually_exists() {
        // syntect ships exactly seven themes and none of them are high-contrast.
        // If a theme names one that is missing, code blocks panic at runtime.
        let ts = ThemeSet::load_defaults();
        for n in names() {
            let t = by_name(n).unwrap();
            assert!(
                ts.themes.contains_key(t.syntect_theme),
                "theme {n} references missing syntect theme {}",
                t.syntect_theme
            );
        }
    }

    #[test]
    fn high_contrast_uses_pure_black_and_white_text() {
        assert_eq!(HIGH_CONTRAST.text, Color::White);
    }
}
