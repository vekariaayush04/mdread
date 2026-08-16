use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const DEFAULT_MAX_MEASURE: u16 = 80;
pub const DEFAULT_THEME: &str = "dark";

/// The on-disk config. Every field optional; an empty file is valid.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    pub theme: Option<String>,
    pub max_measure: Option<u16>,
}

/// Fully resolved settings after applying defaults and precedence.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub theme: String,
    pub max_measure: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: DEFAULT_THEME.to_string(),
            max_measure: DEFAULT_MAX_MEASURE,
        }
    }
}

pub fn parse_config(text: &str) -> Result<ConfigFile, toml::de::Error> {
    toml::from_str(text)
}

/// Precedence, lowest to highest: built-in defaults, config file, CLI flags.
pub fn resolve(
    file: ConfigFile,
    cli_theme: Option<String>,
    cli_max_measure: Option<u16>,
) -> Settings {
    Settings {
        theme: cli_theme
            .or(file.theme)
            .unwrap_or_else(|| DEFAULT_THEME.to_string()),
        max_measure: cli_max_measure
            .or(file.max_measure)
            .unwrap_or(DEFAULT_MAX_MEASURE),
    }
}

pub fn default_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("mdread").join("config.toml"))
}

/// Load config from `explicit`, or the platform default location.
/// A missing file yields defaults; a malformed file is a hard error.
pub fn load(explicit: Option<&Path>) -> anyhow::Result<ConfigFile> {
    let path = match explicit {
        Some(p) => Some(p.to_path_buf()),
        None => default_config_path(),
    };
    let Some(path) = path else {
        return Ok(ConfigFile::default());
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_config(&text)
            .map_err(|e| anyhow::anyhow!("invalid config at {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ConfigFile::default()),
        Err(e) => Err(anyhow::anyhow!("cannot read {}: {e}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_is_valid() {
        assert_eq!(parse_config("").unwrap(), ConfigFile::default());
    }

    #[test]
    fn parses_known_fields() {
        let c = parse_config("theme = \"light\"\nmax_measure = 100\n").unwrap();
        assert_eq!(c.theme.as_deref(), Some("light"));
        assert_eq!(c.max_measure, Some(100));
    }

    #[test]
    fn rejects_unknown_field() {
        // deny_unknown_fields catches typos instead of silently ignoring them.
        assert!(parse_config("thme = \"light\"\n").is_err());
    }

    #[test]
    fn defaults_apply_when_nothing_is_set() {
        let s = resolve(ConfigFile::default(), None, None);
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn config_file_overrides_defaults() {
        let file = ConfigFile {
            theme: Some("light".into()),
            max_measure: Some(70),
        };
        let s = resolve(file, None, None);
        assert_eq!(s.theme, "light");
        assert_eq!(s.max_measure, 70);
    }

    #[test]
    fn cli_overrides_config_file() {
        let file = ConfigFile {
            theme: Some("light".into()),
            max_measure: Some(70),
        };
        let s = resolve(file, Some("dark".into()), Some(120));
        assert_eq!(s.theme, "dark");
        assert_eq!(s.max_measure, 120);
    }

    #[test]
    fn cli_can_override_one_field_without_clobbering_the_other() {
        let file = ConfigFile {
            theme: Some("light".into()),
            max_measure: Some(70),
        };
        let s = resolve(file, None, Some(120));
        assert_eq!(s.theme, "light");
        assert_eq!(s.max_measure, 120);
    }

    #[test]
    fn missing_config_file_is_not_an_error() {
        let c = load(Some(Path::new("/nonexistent/mdread/config.toml"))).unwrap();
        assert_eq!(c, ConfigFile::default());
    }
}
