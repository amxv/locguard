---
title: CI and agent loops
description: Use the changed-file fast path locally and the authoritative full scan in automation.
order: 4
category: Guide
summary: Fast local feedback without weakening the repository-wide CI invariant.
---

## Local and agent checks

Use bare locguard in frequent edit loops:

```bash
locguard
```

Inside Git this asks one stable, read-only status operation for staged, unstaged, and untracked paths, then opens only eligible source files. Existing unchanged files do not need to be rescanned.

This makes the common agent loop cheap even in large repositories.

## CI

Use the authoritative full scan:

```bash
locguard scan
```

A minimal GitHub Actions step can be as simple as:

```yaml
- name: Enforce source file line limits
  run: locguard scan
```

Install locguard using your preferred release channel before the step.

## Existing repositories

If a repository has a small amount of known legacy debt, create optional config and permanently exempt only those exact paths:

```toml
[exempt]
files = [
  "src/legacy/runtime.rs",
]
```

Agents can then run the same ordinary `locguard` command as every other project. The legacy exception stays invisible while any new oversized source file fails normally.

## Machine-readable results

```bash
locguard scan --json
```

Example violation:

```json
{
  "ok": false,
  "files_checked": 14,
  "warnings": [],
  "violations": [
    {
      "path": "src/runtime.rs",
      "lines": null,
      "greater_than": 1000
    }
  ]
}
```

A null line count is intentional: default mode stopped reading as soon as the violation was proven.
