use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow, bail};
use ignore::{DirEntry, WalkBuilder, WalkState};

use crate::cli::Cli;
use crate::config::Config;
use crate::paths::{absolute_from_cwd, git_bytes_to_os, is_within, lexical_normalize};
use crate::policy::{Policy, Scope, has_builtin_excluded_component};

#[derive(Debug, Clone)]
pub struct Repository {
    pub root: PathBuf,
    pub is_git: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryMode {
    Changed,
    Full,
    Explicit,
}

#[derive(Debug)]
pub struct Discovery {
    pub mode: DiscoveryMode,
    pub candidates: Vec<PathBuf>,
}

pub fn repository_from(cwd: &Path) -> Result<Repository> {
    let cwd = fs::canonicalize(cwd)
        .with_context(|| format!("failed to resolve current directory '{}'", cwd.display()))?;
    match git_repo_root(&cwd)? {
        Some(root) => Ok(Repository {
            root: fs::canonicalize(&root).unwrap_or(root),
            is_git: true,
        }),
        None => Ok(Repository {
            root: cwd,
            is_git: false,
        }),
    }
}

pub fn build_scope(cli: &Cli, root: &Path, cwd: &Path) -> Result<Scope> {
    let mut scope = Scope::default();

    for value in &cli.files {
        let path = absolute_from_cwd(value, cwd);
        validate_inside_root(&path, root)?;
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect explicit file '{}'", path.display()))?;
        if metadata.file_type().is_symlink() {
            bail!(
                "explicit file is a symlink; locguard does not follow symlinks: {}",
                path.display()
            );
        }
        if !metadata.is_file() {
            bail!(
                "explicit --file path is not a regular file: {}",
                path.display()
            );
        }
        scope.files.insert(path);
    }

    for value in &cli.dirs {
        let path = absolute_from_cwd(value, cwd);
        validate_inside_root(&path, root)?;
        let metadata = fs::symlink_metadata(&path).with_context(|| {
            format!("failed to inspect explicit directory '{}'", path.display())
        })?;
        if metadata.file_type().is_symlink() {
            bail!(
                "explicit directory is a symlink; locguard does not follow symlinks: {}",
                path.display()
            );
        }
        if !metadata.is_dir() {
            bail!("explicit --dir path is not a directory: {}", path.display());
        }
        if !scope.dirs.contains(&path) {
            scope.dirs.push(path);
        }
    }

    Ok(scope)
}

pub fn discover(
    repo: &Repository,
    cli: &Cli,
    config: &Config,
    policy: &Policy,
    scope: &Scope,
    threads: usize,
) -> Result<Discovery> {
    let respect_ignore = config.respect_ignore(cli);
    let mode = if cli.has_explicit_scope() {
        DiscoveryMode::Explicit
    } else if cli.is_scan() || !repo.is_git {
        DiscoveryMode::Full
    } else {
        DiscoveryMode::Changed
    };

    let candidates = match mode {
        DiscoveryMode::Changed => discover_changed(repo, respect_ignore)?,
        DiscoveryMode::Full => discover_full(repo, respect_ignore, policy, threads)?,
        DiscoveryMode::Explicit => discover_explicit(repo, respect_ignore, policy, scope, threads)?,
    };

    Ok(Discovery { mode, candidates })
}

fn discover_changed(repo: &Repository, respect_ignore: bool) -> Result<Vec<PathBuf>> {
    if !repo.is_git {
        return Ok(Vec::new());
    }

    let output = git_output(
        &repo.root,
        &[
            "--no-optional-locks",
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--no-renames",
            "--ignore-submodules=all",
        ],
    )?;
    let mut paths = BTreeSet::new();
    for record in output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        if record.len() < 4 || record[2] != b' ' {
            bail!("unexpected Git status record while discovering changed files");
        }
        let relative = PathBuf::from(git_bytes_to_os(&record[3..])?);
        paths.insert(lexical_normalize(&repo.root.join(relative)));
    }

    if !respect_ignore {
        for path in git_path_list(
            &repo.root,
            &[
                "ls-files",
                "-z",
                "--others",
                "--ignored",
                "--exclude-standard",
            ],
        )? {
            paths.insert(path);
        }
    }

    Ok(paths.into_iter().collect())
}

fn discover_full(
    repo: &Repository,
    respect_ignore: bool,
    policy: &Policy,
    threads: usize,
) -> Result<Vec<PathBuf>> {
    if repo.is_git && respect_ignore {
        git_path_list(
            &repo.root,
            &[
                "ls-files",
                "-z",
                "--cached",
                "--others",
                "--exclude-standard",
            ],
        )
    } else {
        walk_roots(
            std::slice::from_ref(&repo.root),
            respect_ignore,
            policy.can_prune_builtin_dirs(),
            threads,
        )
    }
}

fn discover_explicit(
    repo: &Repository,
    respect_ignore: bool,
    policy: &Policy,
    scope: &Scope,
    threads: usize,
) -> Result<Vec<PathBuf>> {
    if repo.is_git && respect_ignore {
        return Ok(git_path_list(
            &repo.root,
            &[
                "ls-files",
                "-z",
                "--cached",
                "--others",
                "--exclude-standard",
            ],
        )?
        .into_iter()
        .filter(|path| scope.contains(path))
        .collect());
    }

    let mut roots = scope.dirs.clone();
    roots.extend(scope.files.iter().cloned());
    walk_roots(
        &roots,
        respect_ignore,
        policy.can_prune_builtin_dirs(),
        threads,
    )
}

fn walk_roots(
    roots: &[PathBuf],
    respect_ignore: bool,
    prune_builtins: bool,
    threads: usize,
) -> Result<Vec<PathBuf>> {
    let paths = Arc::new(Mutex::new(BTreeSet::new()));
    let errors = Arc::new(Mutex::new(Vec::new()));

    for walk_root in roots {
        let mut builder = WalkBuilder::new(walk_root);
        builder
            .hidden(false)
            .follow_links(false)
            .git_ignore(respect_ignore)
            .git_global(respect_ignore)
            .git_exclude(respect_ignore)
            .ignore(respect_ignore)
            .parents(respect_ignore)
            .threads(threads.max(1));

        let root_for_filter = walk_root.clone();
        if prune_builtins {
            builder.filter_entry(move |entry| should_descend(entry, &root_for_filter));
        }

        let paths_ref = Arc::clone(&paths);
        let errors_ref = Arc::clone(&errors);
        builder.build_parallel().run(|| {
            let paths_ref = Arc::clone(&paths_ref);
            let errors_ref = Arc::clone(&errors_ref);
            Box::new(move |entry| {
                match entry {
                    Ok(entry) if entry.file_type().is_some_and(|kind| kind.is_file()) => {
                        paths_ref
                            .lock()
                            .expect("path collector mutex poisoned")
                            .insert(lexical_normalize(entry.path()));
                    }
                    Ok(_) => {}
                    Err(error) => errors_ref
                        .lock()
                        .expect("error collector mutex poisoned")
                        .push(error.to_string()),
                }
                WalkState::Continue
            })
        });
    }

    let errors = errors.lock().expect("error collector mutex poisoned");
    if let Some(error) = errors.first() {
        bail!("filesystem traversal failed: {error}");
    }
    drop(errors);

    let result = paths
        .lock()
        .expect("path collector mutex poisoned")
        .iter()
        .cloned()
        .collect();
    Ok(result)
}

fn should_descend(entry: &DirEntry, walk_root: &Path) -> bool {
    if entry.path() == walk_root || !entry.file_type().is_some_and(|kind| kind.is_dir()) {
        return true;
    }
    !has_builtin_excluded_component(Path::new(entry.file_name()))
}

fn git_repo_root(cwd: &Path) -> Result<Option<PathBuf>> {
    let output = match Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("failed to run Git"),
    };
    if !output.status.success() {
        return Ok(None);
    }
    let root = String::from_utf8(output.stdout).context("Git repository root is not UTF-8")?;
    Ok(Some(PathBuf::from(root.trim())))
}

fn git_path_list(root: &Path, args: &[&str]) -> Result<Vec<PathBuf>> {
    let output = git_output(root, args)?;
    output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .map(|record| {
            let relative = PathBuf::from(git_bytes_to_os(record)?);
            Ok(lexical_normalize(&root.join(relative)))
        })
        .collect()
}

fn git_output(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .with_context(|| format!("failed to run Git while scanning '{}'", root.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Git command failed: {}", stderr.trim()));
    }
    Ok(output.stdout)
}

fn validate_inside_root(path: &Path, root: &Path) -> Result<()> {
    if !is_within(path, root) {
        bail!(
            "explicit path '{}' is outside scan root '{}'",
            path.display(),
            root.display()
        );
    }
    Ok(())
}
