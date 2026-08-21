use std::fs::{self, File};
use std::io::{ErrorKind, Read};
use std::path::PathBuf;

use anyhow::{Context, Result};
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;

use crate::policy::Limits;

const BUFFER_SIZE: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct ScanTask {
    pub path: PathBuf,
    pub relative: String,
    pub limits: Limits,
}

#[derive(Debug)]
pub struct FileScan {
    pub relative: String,
    pub limits: Limits,
    pub outcome: FileOutcome,
}

#[derive(Debug)]
pub enum FileOutcome {
    Pass { lines: Option<usize> },
    Violation { exact_lines: Option<usize> },
    Skipped,
}

pub fn scan_files(tasks: &[ScanTask], threads: usize, exact: bool) -> Result<Vec<FileScan>> {
    let pool = ThreadPoolBuilder::new()
        .num_threads(threads.max(1))
        .build()
        .context("failed to initialize scanner worker pool")?;

    let results = pool.install(|| {
        tasks
            .par_iter()
            .map_init(
                || vec![0_u8; BUFFER_SIZE],
                |buffer, task| scan_one(task, buffer, exact),
            )
            .collect::<Vec<_>>()
    });

    results.into_iter().collect()
}

fn scan_one(task: &ScanTask, buffer: &mut [u8], exact: bool) -> Result<FileScan> {
    let metadata = match fs::symlink_metadata(&task.path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(file_scan(task, FileOutcome::Skipped));
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect '{}'", task.path.display()));
        }
    };

    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Ok(file_scan(task, FileOutcome::Skipped));
    }

    if metadata.len() == 0 {
        return Ok(file_scan(task, FileOutcome::Pass { lines: None }));
    }

    let mut file = File::open(&task.path)
        .with_context(|| format!("failed to open source file '{}'", task.path.display()))?;

    // We already needed metadata to avoid following symlinks, so file size is
    // free information here. A physical line needs at least one byte. If the
    // entire file is smaller than the warning threshold, it cannot warn or
    // fail and there is no reason to issue a read syscall.
    let warn_at = u64::try_from(task.limits.warn_at()).unwrap_or(u64::MAX);
    if metadata.len() < warn_at {
        return Ok(file_scan(task, FileOutcome::Pass { lines: None }));
    }

    let outcome = if exact {
        count_exact(&mut file, buffer, task.limits.limit, &task.path)?
    } else {
        count_to_limit(&mut file, buffer, task.limits.limit, &task.path)?
    };
    Ok(file_scan(task, outcome))
}

fn count_to_limit<R: Read>(
    file: &mut R,
    buffer: &mut [u8],
    limit: usize,
    path: &std::path::Path,
) -> Result<FileOutcome> {
    let mut newline_count = 0usize;
    let mut bytes_seen = false;
    let mut last_was_newline = false;
    let mut first_chunk = true;

    loop {
        let read = file
            .read(buffer)
            .with_context(|| format!("failed while reading '{}'", path.display()))?;
        if read == 0 {
            let lines = newline_count + usize::from(bytes_seen && !last_was_newline);
            return Ok(FileOutcome::Pass { lines: Some(lines) });
        }

        let chunk = &buffer[..read];
        if first_chunk {
            if chunk.contains(&0) {
                return Ok(FileOutcome::Skipped);
            }
            first_chunk = false;
        }

        if newline_count == limit {
            return Ok(FileOutcome::Violation { exact_lines: None });
        }

        bytes_seen = true;
        let chunk_newlines = bytecount::count(chunk, b'\n');
        let new_total = newline_count.saturating_add(chunk_newlines);
        if new_total > limit {
            return Ok(FileOutcome::Violation { exact_lines: None });
        }

        last_was_newline = chunk.last() == Some(&b'\n');
        if new_total == limit && !last_was_newline {
            return Ok(FileOutcome::Violation { exact_lines: None });
        }
        newline_count = new_total;
    }
}

fn count_exact<R: Read>(
    file: &mut R,
    buffer: &mut [u8],
    limit: usize,
    path: &std::path::Path,
) -> Result<FileOutcome> {
    let mut newline_count = 0usize;
    let mut bytes_seen = false;
    let mut last_was_newline = false;
    let mut first_chunk = true;

    loop {
        let read = file
            .read(buffer)
            .with_context(|| format!("failed while reading '{}'", path.display()))?;
        if read == 0 {
            break;
        }
        let chunk = &buffer[..read];
        if first_chunk {
            if chunk.contains(&0) {
                return Ok(FileOutcome::Skipped);
            }
            first_chunk = false;
        }
        bytes_seen = true;
        newline_count = newline_count.saturating_add(bytecount::count(chunk, b'\n'));
        last_was_newline = chunk.last() == Some(&b'\n');
    }

    let lines = newline_count + usize::from(bytes_seen && !last_was_newline);
    if lines > limit {
        Ok(FileOutcome::Violation {
            exact_lines: Some(lines),
        })
    } else {
        Ok(FileOutcome::Pass { lines: Some(lines) })
    }
}

fn file_scan(task: &ScanTask, outcome: FileOutcome) -> FileScan {
    FileScan {
        relative: task.relative.clone(),
        limits: task.limits,
        outcome,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::path::Path;

    use super::*;

    fn count(input: &[u8], limit: usize) -> FileOutcome {
        let mut cursor = Cursor::new(input);
        let mut buffer = vec![0; 8];
        count_to_limit(&mut cursor, &mut buffer, limit, Path::new("test.rs")).unwrap()
    }

    #[test]
    fn physical_line_semantics_match_editor_lines() {
        assert!(matches!(
            count(b"", 1),
            FileOutcome::Pass { lines: Some(0) }
        ));
        assert!(matches!(
            count(b"hello", 1),
            FileOutcome::Pass { lines: Some(1) }
        ));
        assert!(matches!(
            count(b"hello\n", 1),
            FileOutcome::Pass { lines: Some(1) }
        ));
        assert!(matches!(
            count(b"hello\nworld", 2),
            FileOutcome::Pass { lines: Some(2) }
        ));
        assert!(matches!(
            count(b"hello\nworld\n", 2),
            FileOutcome::Pass { lines: Some(2) }
        ));
    }

    #[test]
    fn detects_line_limit_without_requiring_eof() {
        assert!(matches!(
            count(b"one\ntwo\nthree", 2),
            FileOutcome::Violation { exact_lines: None }
        ));
        assert!(matches!(
            count(b"one\ntwo\nthree\n", 2),
            FileOutcome::Violation { exact_lines: None }
        ));
    }

    #[test]
    fn skips_binary_like_source_bytes() {
        let utf16le = b"a\0\n\0b\0\n\0";
        assert!(matches!(count(utf16le, 2), FileOutcome::Skipped));
    }
}
