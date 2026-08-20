use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::cli::Cli;
use crate::paths::normalize_config_path;

pub const DEFAULT_LIMIT: usize = 1000;
pub const DEFAULT_WARN_PERCENT: u8 = 90;
pub const CONFIG_RELATIVE_PATH: &str = ".agents/.locguard.toml";

#[derive(Debug, Clone)]
pub struct Config {
    pub limit: usize,
    pub warn_percent: u8,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub exempt_files: Vec<String>,
    pub respect_ignore: bool,
    pub default_types: bool,
    pub default_excludes: bool,
    pub overrides: Vec<PathOverride>,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct PathOverride {
    pub files: Vec<String>,
    pub limit: Option<usize>,
    pub warn_percent: Option<u8>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    limit: Option<usize>,
    warn_percent: Option<u8>,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    exempt: ExemptConfig,
    respect_ignore: Option<bool>,
    default_types: Option<bool>,
    default_excludes: Option<bool>,
    #[serde(default, rename = "override")]
    overrides: Vec<FileOverride>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExemptConfig {
    #[serde(default)]
    files: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileOverride {
    files: Vec<String>,
    limit: Option<usize>,
    warn_percent: Option<u8>,
}

impl Config {
    pub fn load(cli: &Cli, root: &Path, cwd: &Path) -> Result<Self> {
        let path = if cli.no_config {
            None
        } else if let Some(explicit) = &cli.config {
            Some(if explicit.is_absolute() {
                explicit.clone()
            } else {
                cwd.join(explicit)
            })
        } else {
            let default = root.join(CONFIG_RELATIVE_PATH);
            default.exists().then_some(default)
        };

        let file_config = match &path {
            Some(path) => read_config(path, cli.config.is_some())?,
            None => FileConfig::default(),
        };

        validate_values(file_config.limit, file_config.warn_percent)?;
        for rule in &file_config.overrides {
            if rule.files.is_empty() {
                bail!("config override must contain at least one file pattern");
            }
            validate_values(rule.limit, rule.warn_percent)?;
        }

        let exempt_files = file_config
            .exempt
            .files
            .iter()
            .map(|path| normalize_config_path(path))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            limit: file_config.limit.unwrap_or(DEFAULT_LIMIT),
            warn_percent: file_config.warn_percent.unwrap_or(DEFAULT_WARN_PERCENT),
            include: file_config.include,
            exclude: file_config.exclude,
            exempt_files,
            respect_ignore: file_config.respect_ignore.unwrap_or(true),
            default_types: file_config.default_types.unwrap_or(true),
            default_excludes: file_config.default_excludes.unwrap_or(true),
            overrides: file_config
                .overrides
                .into_iter()
                .map(|rule| PathOverride {
                    files: rule.files,
                    limit: rule.limit,
                    warn_percent: rule.warn_percent,
                })
                .collect(),
            path,
        })
    }

    pub fn respect_ignore(&self, cli: &Cli) -> bool {
        self.respect_ignore && !cli.no_ignore
    }

    pub fn default_excludes(&self, cli: &Cli) -> bool {
        self.default_excludes && !cli.no_default_excludes
    }
}

fn read_config(path: &Path, explicit: bool) -> Result<FileConfig> {
    if !path.exists() {
        if explicit {
            bail!("config file does not exist: {}", path.display());
        }
        return Ok(FileConfig::default());
    }
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read config '{}'", path.display()))?;
    toml::from_str(&source).with_context(|| format!("invalid config '{}'", path.display()))
}

fn validate_values(limit: Option<usize>, warn_percent: Option<u8>) -> Result<()> {
    if matches!(limit, Some(0)) {
        bail!("line limit must be at least 1");
    }
    if let Some(percent) = warn_percent
        && !(1..=100).contains(&percent)
    {
        bail!("warn_percent must be between 1 and 100");
    }
    Ok(())
}
