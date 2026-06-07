# Stack

## Repository Shape

- This repository is a Rust CLI project.
- The primary binary is `airis`.
- `manifest.toml` is the thin source of truth that drives convention generation (AI adapters, docs, `tsconfig.json`, scaffolding) and, for containerized repos, Docker environment generation.

## Common Commands

```bash
cargo build
cargo build --release
cargo test                                # all tests (unit + integration)
cargo test --lib                          # unit tests only (in src/)
cargo test --test cli_test                # integration tests only (tests/cli_test.rs)
cargo test <name>                         # filter by test name across both
cargo test --test cli_test <name>         # filter a single integration test
cargo fmt --check
cargo clippy -- -D warnings
```

Unit tests live next to the code under `src/` (`#[cfg(test)] mod tests` blocks). Integration tests live in `tests/cli_test.rs` and exercise the built `airis` binary end-to-end via `assert_cmd`.

`airis verify` is a convenience wrapper, but it **skips `cargo check`/`clippy`/`fmt --check`/`test` when the workspace container is offline** and still prints a green summary. Before pushing, run the `cargo` commands above directly (or `airis up` first) so CI is not the first thing to catch a regression.

## Documentation Sync

`docs/ai/*.md` is the source of truth for `CLAUDE.md`, `AGENTS.md`, and `GEMINI.md` adapters at the repo root. After editing any shared doc:

```bash
airis docs sync           # regenerate all vendor adapter files from docs/ai/*
airis docs sync --force   # overwrite even when [docs.mode = "warn"]
airis docs list           # show which adapter files are managed
```

Never hand-edit the `<!-- BEGIN GENERATED airis gen -->` block in the adapter files — `airis docs sync` rewrites it.

## Git Hooks

Hooks are generated into `.airis/hooks/` by `airis gen` and wired into `.git/hooks/` by:

```bash
airis hooks install        # writes shims to .git/hooks/{pre-commit,pre-push}
airis hooks uninstall      # removes the airis-workspace blocks (keeps other hooks)
```

`.git/hooks/` is not checked in, so a fresh clone needs `airis hooks install` once. The shims delegate to `.airis/hooks/pre-commit` / `.airis/hooks/pre-push` so updates flow through `airis gen` without re-installing.

The `post-commit` hook (which reinstalls the `airis` binary in the background) is installed separately by `airis hooks install`.

## Module Boundary

airis is a convention-unification engine; Docker is one module within it, not the whole tool.

- **Convention core** (applies to every repo, polyglot): `gen`, `docs`, `claude`, `new`, `validate`, `doctor`, `bump-version`, `verify`. These keep AI adapters, docs, `tsconfig.json`, version scheme, and scaffolding consistent.
- **Docker module** (only for containerized repos): `up` / `down` / `exec` / `ps` / `logs` / `restart` / `network` / `run`, plus `compose.yaml` + volume-hygiene generation inside `gen`. The `[docker]` manifest section is optional (`#[serde(default)]`), so non-containerized repos (Edge/Workers, native desktop) use airis without it.

## Important Paths

- `src/manifest/` for manifest schema and validation
- `src/commands/` for CLI behavior
- `src/commands/generate/compose_gen.rs` for the Docker module's `compose.yaml` generation
- `docs/ai/` for shared AI guidance (source of truth for adapter files)
- `tests/cli_test.rs` for end-to-end CLI integration tests
