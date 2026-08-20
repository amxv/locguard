---
title: Quickstart
description: Install locguard and start enforcing small source files with no setup.
order: 1
category: Start
summary: Install once, run locguard, and add the full scan to CI.
---

## Install

The npm package installs the native `locguard` binary:

```bash
npm install -g locguard-cli
```

Native archives and shell/PowerShell installers are also attached to GitHub Releases.

## Run it

No initialization or config file is required:

```bash
locguard
```

Inside a Git repository, bare `locguard` checks staged, unstaged, and nonignored untracked source files. If nothing eligible changed:

```text
✓ no source files changed
```

When files changed:

```text
✓ 7 files checked
```

A file above the default 1,000-line limit fails:

```text
FAIL src/runtime.rs  >1000

1 file exceeds the 1000-line limit
```

## Check the whole repository

Use the full scan for CI, pre-push checks, and repository audits:

```bash
locguard scan
```

The scan includes tracked files and nonignored untracked files while skipping high-confidence dependency, vendor, generated, cache, and build-output trees.

## Customize only when needed

`locguard init` is optional. Run it only when the defaults need project-specific customization:

```bash
locguard init
```

It creates `.agents/.locguard.toml`. Most repositories should not need one.
