---
title: Configuration
description: Customize limits, source recognition, exclusions, exemptions, and path-specific policy.
order: 2
category: Guide
summary: Strong defaults with explicit escape hatches in .agents/.locguard.toml.
---

Configuration is optional. When present, locguard reads exactly one repository-root file:

```text
.agents/.locguard.toml
```

Create a minimal commented version with `locguard init`, or write it directly.

## Basic policy

```toml
limit = 1000
warn_percent = 90

include = []
exclude = []

[exempt]
files = []
```

CLI flags override repository config, and repository config overrides built-in defaults.

## Include additional source files

Locguard recognizes a broad set of code-file extensions automatically, but it intentionally does not guess special filenames such as `Makefile` or `Dockerfile`.

Add them when your project wants them checked:

```toml
include = [
  "Makefile",
  "**/Dockerfile",
  "**/*.foo",
]
```

`include` is additive; normal Rust, TypeScript, Go, Python, and other recognized source files continue to be scanned.

## Exclude project-specific paths

```toml
exclude = [
  "fixtures/generated-tests/**",
  "legacy/snapshots/**",
]
```

Explicit `exclude` rules win over `include` rules.

## Permanently exempt exact legacy files

If an existing repository has a few large files you do not want agents to deal with yet, list their exact repo-relative paths:

```toml
[exempt]
files = [
  "src/legacy/runtime.rs",
  "src/ui/old_screen.tsx",
]
```

Exempt files are completely silent during ordinary checks: no warnings and no failures. New files remain protected by the normal limit without agents needing to know which legacy files were grandfathered.

Exemptions accept exact paths only, never globs. Use `exclude` for categories.

Audit the repository as if exemptions did not exist with:

```bash
locguard scan --no-exempt
```

## Path-specific limits

```toml
[[override]]
files = ["migrations/**/*.sql"]
limit = 1500

[[override]]
files = ["crates/special/**"]
limit = 2000
warn_percent = 95
```

If multiple overrides match, the last matching rule wins. Unspecified values inherit from the global policy.

A CLI override such as `--limit 800` takes precedence over matching config overrides.

## Advanced switches

```toml
respect_ignore = true
default_types = true
default_excludes = true
```

Set `respect_ignore = false` to include ignored files, `default_types = false` to use only explicit includes, or `default_excludes = false` to disable locguard's generated/vendor/build exclusions.

Unknown config keys are errors rather than being silently ignored.
