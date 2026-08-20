use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::config::CONFIG_RELATIVE_PATH;

const INITIAL_CONFIG: &str = r#"# Maximum physical lines per source file.
limit = 1000

# Warn when a file reaches this percentage of its limit.
warn_percent = 90

# Additional source files or patterns to scan.
include = []

# Additional paths or patterns to skip.
exclude = []

# Exact repo-relative files permanently exempt from the limit.
[exempt]
files = []
"#;

pub fn create(root: &Path) -> Result<()> {
    let path = root.join(CONFIG_RELATIVE_PATH);
    if path.exists() {
        bail!("config already exists: {}", path.display());
    }
    let parent = path.parent().expect("config path has a parent");
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create '{}'", parent.display()))?;
    fs::write(&path, INITIAL_CONFIG)
        .with_context(|| format!("failed to write '{}'", path.display()))?;
    println!("created {CONFIG_RELATIVE_PATH}");
    Ok(())
}
