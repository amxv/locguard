# locguard

A fast, zero-config CLI that keeps source files below a configurable line limit.

`locguard` defaults to 1,000 physical lines per source file. It ignores Git-ignored files, dependencies, generated code, vendored code, and build output, then scans only the source files that matter.

```bash
locguard
# ✓ 7 files checked

locguard scan
# ✓ 1,443 files checked
```

## Install

The npm package installs the native `locguard` binary:

```bash
npm install -g locguard-cli
```

Native archives and shell/PowerShell installers are also published with GitHub Releases.

## Usage

Bare `locguard` is optimized for local and agent check loops:

```bash
locguard
```

Inside Git it checks staged, unstaged, and nonignored untracked source files. Use `scan` for the authoritative full-tree check, such as in CI:

```bash
locguard scan
```

Scope a check explicitly when useful:

```bash
locguard --file src/main.rs --file src/server.rs
locguard --dir src --dir crates/api
```

A file over the limit fails with exit code 1:

```text
FAIL src/runtime.rs  >1000

1 file exceeds the 1000-line limit
```

Warnings begin at 90% of the effective limit by default.

## Configuration

No setup is required. Run `locguard init` only if you want to customize the defaults. It creates `.agents/.locguard.toml`:

```toml
limit = 1000
warn_percent = 90

include = []
exclude = []

[exempt]
files = []
```

Use `include` for project-specific source files such as `Makefile`, `exclude` for project-specific categories, and `[exempt].files` for exact legacy files that should be permanently grandfathered without teaching every agent about them.

CLI flags override repository configuration:

```bash
locguard --limit 800
locguard --include '**/*.foo'
locguard scan --no-exempt
locguard scan --json
```

## Why it is fast

`locguard` is a threshold checker, not a code-analysis engine. It does not parse ASTs or decode source text just to count lines.

- Git supplies changed/full candidate paths and ignore semantics.
- Source-type and generated/vendor filters run before files are opened.
- Violating files stop being read as soon as line `limit + 1` is proven.
- File scans run with modest parallelism and reusable buffers.
- Newlines are counted directly from bytes.

Physical lines include comments and blank lines because the invariant is about keeping files small, modular, merge-friendly, and easy for coding agents to navigate.

## Development

```bash
just check-fast
just check
just docs-check
just docs-build
```

See the [documentation](https://locguard.ashray.xyz) for configuration and the complete command reference.

## License

Apache-2.0
