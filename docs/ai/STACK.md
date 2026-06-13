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

Run the `cargo` commands above directly — that is the host-native default for this Rust CLI. `airis workspace gen` only writes `compose.yaml` / `tsconfig.json` / AI-rule files and does not run tests, so it is no substitute for running the checks above before pushing.

## Documentation Sync

`docs/ai/*.md` is the source of truth for `CLAUDE.md`, `AGENTS.md`, and `GEMINI.md` adapters at the repo root. After editing any shared doc:

```bash
airis workspace docs sync           # regenerate all vendor adapter files from docs/ai/*
airis workspace docs sync --force   # overwrite even when [docs.mode = "warn"]
airis workspace docs list           # show which adapter files are managed
```

Never hand-edit the `<!-- BEGIN GENERATED airis gen -->` block in the adapter files — `airis workspace docs sync` rewrites it.

## Module Boundary

airis is a convention-unification engine; Docker is one module within it, not the whole tool.

- **Convention core** (applies to every repo, polyglot): `gen`, `docs`, `claude`, `new`, `validate`, `doctor`, `bump-version`, `verify`. These keep AI adapters, docs, `tsconfig.json`, version scheme, and scaffolding consistent.
- **Docker module** (only for containerized repos): `compose.yaml` + volume-hygiene generation inside `gen`. airis writes the compose file but does not run containers for you — execution stays host-native by default (`~/.claude/rules/runtime-workflow.md`). The `[docker]` manifest section is optional (`#[serde(default)]`), so non-containerized repos (host-native CLI/Edge/Workers, native desktop) use airis without it.

## Important Paths

- `src/manifest/` for manifest schema and validation
- `src/commands/` for CLI behavior
- `src/commands/generate/compose_gen.rs` for the Docker module's `compose.yaml` generation
- `docs/ai/` for shared AI guidance (source of truth for adapter files)
- `tests/cli_test.rs` for end-to-end CLI integration tests
