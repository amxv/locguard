---
title: Changelog
description: User-facing locguard release history.
order: 99
category: Reference
summary: New commands, policy behavior, performance changes, and compatibility notes.
---

## 0.1.2 - 2026-08-21

- Make human diagnostics more token-efficient by printing the active LOC limit and number of failing files once at the top instead of repeating the limit on every WARN/FAIL line.
- Keep warning line counts visible while omitting redundant `/ limit` suffixes when all visible diagnostics share the same effective limit.
- Preserve per-line thresholds only for the uncommon mixed path-specific-limit case, where omitting them would make the output ambiguous.

## 0.1.1 - 2026-08-21

- Skip binary-like and UTF-16-style source fixtures instead of failing the entire repository when a recognized extension contains NUL bytes.
- Use already-required file metadata to prove files below the warning threshold are safe without reading their contents, while still opening them to preserve unreadable-file errors.
- Keep the automatic eight-worker cap after benchmarking it against four workers across VS Code, Kubernetes, Rust, and Linux source trees.

## 0.1.0 - 2026-08-21

- Initial locguard CLI with zero-config 1,000-line enforcement and 90% warnings.
- Fast Git changed-file checks plus authoritative full-tree scans.
- Broad source-extension recognition with generated, vendored, dependency, cache, and build-output defaults.
- Optional `.agents/.locguard.toml` configuration, exact legacy exemptions, glob includes/excludes, and path-specific limits.
- Repeatable `--file` / `--dir` scopes, JSON output, exact-count mode, and stable exit codes.
