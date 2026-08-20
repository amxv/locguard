use std::env;

use anyhow::{Context, Result, bail};

use crate::cli::Cli;
use crate::config::Config;
use crate::discovery::{build_scope, discover, repository_from};
use crate::init;
use crate::output::{Report, render};
use crate::paths::slash_relative;
use crate::policy::Policy;
use crate::scanner::{ScanTask, scan_files};

pub fn execute(cli: Cli) -> Result<u8> {
    let cwd = env::current_dir().context("failed to read current directory")?;
    let cwd = std::fs::canonicalize(&cwd)
        .with_context(|| format!("failed to resolve current directory '{}'", cwd.display()))?;
    let repo = repository_from(&cwd)?;

    if cli.is_init() {
        validate_init_args(&cli)?;
        init::create(&repo.root)?;
        return Ok(0);
    }

    let config = Config::load(&cli, &repo.root, &cwd)?;
    let policy = Policy::new(&config, &cli, &repo.root)?;
    let scope = build_scope(&cli, &repo.root, &cwd)?;
    let threads = cli.threads.unwrap_or_else(default_threads);
    let discovery = discover(&repo, &cli, &config, &policy, &scope, threads)?;

    let mut tasks = Vec::new();
    for path in discovery.candidates {
        let relative = slash_relative(&path, &repo.root)?;
        if !policy.should_scan(&path, &relative, &scope, cli.no_exempt) {
            continue;
        }
        tasks.push(ScanTask {
            limits: policy.limits_for(&relative),
            path,
            relative,
        });
    }
    tasks.sort_by(|left, right| left.relative.cmp(&right.relative));
    tasks.dedup_by(|left, right| left.path == right.path);

    let scans = scan_files(&tasks, threads, cli.exact)?;
    let report = Report::from_scans(scans, cli.no_warn);
    render(&report, discovery.mode, &cli)?;
    Ok(u8::from(!report.ok))
}

fn default_threads() -> usize {
    std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .clamp(1, 8)
}

fn validate_init_args(cli: &Cli) -> Result<()> {
    let has_scan_options = cli.has_explicit_scope()
        || cli.limit.is_some()
        || cli.warn_percent.is_some()
        || cli.no_warn
        || !cli.include.is_empty()
        || !cli.exclude.is_empty()
        || !cli.only.is_empty()
        || cli.no_default_excludes
        || cli.no_ignore
        || cli.no_exempt
        || cli.exact
        || cli.quiet
        || cli.json
        || cli.config.is_some()
        || cli.no_config
        || cli.threads.is_some();
    if has_scan_options {
        bail!("`locguard init` does not accept scan/config override flags");
    }
    Ok(())
}
