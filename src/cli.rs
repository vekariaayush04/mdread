use clap::Parser;
use std::path::PathBuf;

/// A terminal Markdown reader.
#[derive(Parser, Debug, PartialEq)]
#[command(name = "mdread", version, about, long_about = None)]
pub struct Cli {
    /// Markdown file to open.
    pub file: Option<PathBuf>,

    /// Colour theme: dark, light, or high-contrast.
    #[arg(short = 't', long)]
    pub theme: Option<String>,

    /// Maximum text width in columns.
    #[arg(long)]
    pub max_measure: Option<u16>,

    /// Path to a config file, overriding the default location.
    #[arg(long)]
    pub config: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_file_argument() {
        let cli = Cli::try_parse_from(["mdread", "README.md"]).unwrap();
        assert_eq!(cli.file, Some(PathBuf::from("README.md")));
        assert_eq!(cli.theme, None);
    }

    #[test]
    fn parses_all_flags() {
        let cli = Cli::try_parse_from([
            "mdread",
            "docs/a.md",
            "--theme",
            "light",
            "--max-measure",
            "100",
            "--config",
            "/tmp/c.toml",
        ])
        .unwrap();
        assert_eq!(cli.file, Some(PathBuf::from("docs/a.md")));
        assert_eq!(cli.theme.as_deref(), Some("light"));
        assert_eq!(cli.max_measure, Some(100));
        assert_eq!(cli.config, Some(PathBuf::from("/tmp/c.toml")));
    }

    #[test]
    fn file_argument_is_optional() {
        let cli = Cli::try_parse_from(["mdread"]).unwrap();
        assert_eq!(cli.file, None);
    }

    #[test]
    fn rejects_unknown_flag() {
        assert!(Cli::try_parse_from(["mdread", "--nope"]).is_err());
    }
}
