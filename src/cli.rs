use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

const AFTER_HELP: &str = r#"CONFIGURATION
  locguard requires no configuration and works with sensible defaults.

  Run `locguard init` only to customize behavior, such as limits,
  includes/excludes, or permanently exempting existing files.

  Config: .agents/.locguard.toml

BEHAVIOR
  Bare `locguard` checks changed source files in Git repositories.
  `locguard scan` checks the whole source tree.
  `--file` and `--dir` are repeatable explicit scopes.
  Permanent legacy exceptions live in `[exempt].files` in the optional config."#;

#[derive(Debug, Parser)]
#[command(
    version,
    about = env!("CARGO_PKG_DESCRIPTION"),
    after_help = AFTER_HELP,
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Check this exact file. Repeat to check multiple files.
    #[arg(short = 'f', long = "file", value_name = "PATH", global = true)]
    pub files: Vec<PathBuf>,

    /// Scan recognized source files under this directory. Repeatable.
    #[arg(short = 'd', long = "dir", value_name = "PATH", global = true)]
    pub dirs: Vec<PathBuf>,

    /// Override the maximum physical lines per source file.
    #[arg(long, value_name = "N", value_parser = parse_positive_usize, global = true)]
    pub limit: Option<usize>,

    /// Override the warning threshold as a percentage of the effective limit.
    #[arg(long, value_name = "N", value_parser = parse_warn_percent, global = true)]
    pub warn_percent: Option<u8>,

    /// Suppress warning diagnostics.
    #[arg(long, global = true)]
    pub no_warn: bool,

    /// Add a source path/glob beyond built-in recognition. Repeatable.
    #[arg(long, value_name = "GLOB", global = true)]
    pub include: Vec<String>,

    /// Exclude a path/glob. Repeatable.
    #[arg(long, value_name = "GLOB", global = true)]
    pub exclude: Vec<String>,

    /// Scan only these source globs instead of built-in source types. Repeatable.
    #[arg(long, value_name = "GLOB", conflicts_with = "include", global = true)]
    pub only: Vec<String>,

    /// Disable built-in generated/vendor/build exclusions.
    #[arg(long, global = true)]
    pub no_default_excludes: bool,

    /// Include paths ignored by Git or ignore files.
    #[arg(long, global = true)]
    pub no_ignore: bool,

    /// Apply normal policy to files listed in `[exempt].files`.
    #[arg(long, global = true)]
    pub no_exempt: bool,

    /// Continue through violating files to report exact line counts.
    #[arg(long, global = true)]
    pub exact: bool,

    /// Suppress success and warning output. Failures still print.
    #[arg(long, conflicts_with = "json", global = true)]
    pub quiet: bool,

    /// Emit stable machine-readable JSON.
    #[arg(long, conflicts_with = "quiet", global = true)]
    pub json: bool,

    /// Use this config file instead of `.agents/.locguard.toml`.
    #[arg(long, value_name = "PATH", conflicts_with = "no_config", global = true)]
    pub config: Option<PathBuf>,

    /// Ignore repository locguard configuration.
    #[arg(long, conflicts_with = "config", global = true)]
    pub no_config: bool,

    /// Override automatic scanner worker count.
    #[arg(short = 'j', long = "threads", value_name = "N", value_parser = parse_positive_usize, global = true)]
    pub threads: Option<usize>,

    /// Control colored human output.
    #[arg(long, value_enum, default_value_t = ColorMode::Auto, global = true)]
    pub color: ColorMode,
}

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum Command {
    /// Check the complete eligible source tree.
    Scan,
    /// Create optional `.agents/.locguard.toml` customization config.
    Init,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

impl Cli {
    pub fn has_explicit_scope(&self) -> bool {
        !self.files.is_empty() || !self.dirs.is_empty()
    }

    pub fn is_scan(&self) -> bool {
        matches!(self.command, Some(Command::Scan))
    }

    pub fn is_init(&self) -> bool {
        matches!(self.command, Some(Command::Init))
    }
}

fn parse_positive_usize(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("'{value}' is not a positive integer"))?;
    if parsed == 0 {
        return Err("value must be at least 1".to_owned());
    }
    Ok(parsed)
}

fn parse_warn_percent(value: &str) -> Result<u8, String> {
    let parsed = value
        .parse::<u8>()
        .map_err(|_| format!("'{value}' is not an integer between 1 and 100"))?;
    if !(1..=100).contains(&parsed) {
        return Err("value must be between 1 and 100".to_owned());
    }
    Ok(parsed)
}
