---
title: Command reference
description: Every locguard command, scope flag, policy override, and output mode.
order: 3
category: Reference
summary: The complete compact CLI surface.
---

## Core commands

```bash
locguard
locguard scan
locguard init
```

`locguard` checks current changes inside Git and performs a full current-tree scan outside Git.

`locguard scan` checks the complete eligible source tree.

`locguard init` creates optional `.agents/.locguard.toml` customization config. It is never required for normal use.

## Explicit scope

```text
-f, --file <PATH>   exact file; repeatable
-d, --dir <PATH>    directory scope; repeatable
```

Examples:

```bash
locguard --file src/main.rs --file src/server.rs
locguard --dir src --dir crates/api
```

`--file` is explicit intent and can check files such as `Makefile` even though they are not recognized automatically. `--dir` keeps normal source-type recognition inside the requested directory.

Explicit scope checks the whole requested scope rather than only changed paths.

## Policy overrides

```text
--limit <N>              override maximum physical lines
--warn-percent <N>       override warning percentage
--no-warn                suppress warnings
--include <GLOB>         add source pattern; repeatable
--exclude <GLOB>         exclude pattern; repeatable
--only <GLOB>            use only supplied source patterns; repeatable
--no-default-excludes    disable built-in generated/vendor/build exclusions
--no-ignore              include ignored files
--no-exempt              apply policy to configured exemptions
```

## Output and performance

```text
--exact                   report exact counts for offenders
--quiet                   suppress success/warning human output
--json                    stable machine-readable output
-j, --threads <N>         override automatic worker count
--color auto|always|never color policy for human output
```

By default locguard stops reading a violating file as soon as the limit is proven. Human output prints the effective limit and failure count once at the top instead of repeating `>1000` on every failure line; JSON still uses `"lines": null`. `--exact` deliberately reads offenders to EOF.

## Config selection

```text
--config <PATH>  use an explicit config file
--no-config      ignore repository locguard config
```

## Exit codes

```text
0  policy passed; warnings are allowed
1  one or more files violate policy
2  usage, config, filesystem, or tool execution error
```
