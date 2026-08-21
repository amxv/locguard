use std::io::{self, IsTerminal, Write};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::cli::{Cli, ColorMode};
use crate::discovery::DiscoveryMode;
use crate::scanner::{FileOutcome, FileScan};

#[derive(Debug, Serialize)]
pub struct Report {
    pub ok: bool,
    pub files_checked: usize,
    pub warnings: Vec<Warning>,
    pub violations: Vec<Violation>,
}

#[derive(Debug, Serialize)]
pub struct Warning {
    pub path: String,
    pub lines: usize,
    pub limit: usize,
}

#[derive(Debug, Serialize)]
pub struct Violation {
    pub path: String,
    pub lines: Option<usize>,
    pub greater_than: usize,
}

impl Report {
    pub fn from_scans(mut scans: Vec<FileScan>, no_warn: bool) -> Self {
        scans.sort_by(|left, right| left.relative.cmp(&right.relative));
        let mut files_checked = 0usize;
        let mut warnings = Vec::new();
        let mut violations = Vec::new();

        for scan in scans {
            match scan.outcome {
                FileOutcome::Skipped => {}
                FileOutcome::Pass { lines } => {
                    files_checked += 1;
                    if !no_warn
                        && let Some(lines) = lines
                        && lines >= scan.limits.warn_at()
                    {
                        warnings.push(Warning {
                            path: scan.relative,
                            lines,
                            limit: scan.limits.limit,
                        });
                    }
                }
                FileOutcome::Violation { exact_lines } => {
                    files_checked += 1;
                    violations.push(Violation {
                        path: scan.relative,
                        lines: exact_lines,
                        greater_than: scan.limits.limit,
                    });
                }
            }
        }

        Self {
            ok: violations.is_empty(),
            files_checked,
            warnings,
            violations,
        }
    }
}

pub fn render(report: &Report, mode: DiscoveryMode, cli: &Cli) -> Result<()> {
    if cli.json {
        let stdout = io::stdout();
        let mut lock = stdout.lock();
        serde_json::to_writer_pretty(&mut lock, report).context("failed to write JSON output")?;
        writeln!(lock).context("failed to finish JSON output")?;
        return Ok(());
    }

    let color = use_color(cli.color);
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let show_warnings = !cli.quiet && !report.warnings.is_empty();
    if !report.violations.is_empty() || show_warnings {
        write_diagnostic_header(&mut out, report, show_warnings)?;
    }

    let common_limit = common_visible_limit(report, show_warnings);
    for violation in &report.violations {
        let count = match (violation.lines, common_limit) {
            (Some(lines), Some(_)) => format!("  {lines}"),
            (Some(lines), None) => format!("  {lines} >{}", violation.greater_than),
            (None, Some(_)) => String::new(),
            (None, None) => format!("  >{}", violation.greater_than),
        };
        writeln!(
            out,
            "{} {}{}",
            style_label("FAIL", "31", color),
            violation.path,
            count
        )
        .context("failed to write failure output")?;
    }

    if show_warnings {
        for warning in &report.warnings {
            let limit = if common_limit.is_some() {
                String::new()
            } else {
                format!(" / {}", warning.limit)
            };
            writeln!(
                out,
                "{} {}  {}{}",
                style_label("WARN", "33", color),
                warning.path,
                warning.lines,
                limit
            )
            .context("failed to write warning output")?;
        }
    }

    if !report.violations.is_empty() {
        return Ok(());
    }

    if cli.quiet {
        return Ok(());
    }

    if mode == DiscoveryMode::Changed && report.files_checked == 0 {
        writeln!(
            out,
            "{} no source files changed",
            style_label("✓", "32", color)
        )
        .context("failed to write success output")?;
        return Ok(());
    }

    let warning_suffix = match report.warnings.len() {
        0 => String::new(),
        1 => ", 1 warning".to_owned(),
        count => format!(", {count} warnings"),
    };
    writeln!(
        out,
        "{} {} {} checked{}",
        style_label("✓", "32", color),
        report.files_checked,
        plural(report.files_checked, "file", "files"),
        warning_suffix
    )
    .context("failed to write success output")?;
    Ok(())
}

fn write_diagnostic_header(
    out: &mut impl Write,
    report: &Report,
    show_warnings: bool,
) -> Result<()> {
    let violation_count = report.violations.len();
    if let Some(limit) = common_visible_limit(report, show_warnings) {
        if violation_count == 0 {
            writeln!(out, "Current LOC limit: {limit} lines.")?;
        } else {
            let noun = plural(violation_count, "file", "files");
            writeln!(
                out,
                "Current LOC limit: {limit} lines, {violation_count} {noun} exceeded this limit."
            )?;
        }
    } else if violation_count == 0 {
        writeln!(out, "Configured LOC limits vary by path.")?;
    } else {
        let noun = plural(violation_count, "file", "files");
        writeln!(
            out,
            "{violation_count} {noun} exceeded configured LOC limits."
        )?;
    }
    Ok(())
}

fn common_visible_limit(report: &Report, show_warnings: bool) -> Option<usize> {
    let mut limits = report
        .violations
        .iter()
        .map(|violation| violation.greater_than)
        .chain(
            show_warnings
                .then_some(report.warnings.iter().map(|warning| warning.limit))
                .into_iter()
                .flatten(),
        );
    let first = limits.next()?;
    limits.all(|limit| limit == first).then_some(first)
}

fn use_color(mode: ColorMode) -> bool {
    match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => io::stdout().is_terminal(),
    }
}

fn style_label(value: &str, ansi: &str, enabled: bool) -> String {
    if enabled {
        format!("\x1b[{ansi}m{value}\x1b[0m")
    } else {
        value.to_owned()
    }
}

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}
