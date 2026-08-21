# Contributor and maintainer notes

## Prerequisites

- Rust from `rust-toolchain.toml` (currently 1.98.0)
- Bun for documentation work
- `dist` 0.32.0 when changing or inspecting release configuration

## Development

```bash
cargo fmt --all -- --check
cargo check --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked
./target/debug/locguard --help
./target/debug/locguard scan
```

The full Rust check is format, strict Clippy, then tests. End-to-end CLI behavior lives in `tests/cli.rs` and uses isolated temporary Git repositories.

## Docs

```bash
cd docs
bun install --frozen-lockfile
bun run test:vercel
bun run check
bun run build
bun run dev
```

Run docs checks/builds serially. Vercel uses Root Directory `docs`.

## Release infrastructure

`dist-workspace.toml` is maintained source and `.github/workflows/release.yml` is generated:

```bash
dist plan
dist generate
```

Releases produce macOS/Linux/Windows archives, shell and PowerShell installers, the `locguard-cli` npm package, checksums, and GitHub artifact attestations. crates.io publication is intentionally disabled.

The generated npm publish job expects the GitHub Actions secret `NPM_TOKEN`.

For release requests, follow `.agents/skills/release/SKILL.md`.
