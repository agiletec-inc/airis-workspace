# Stack

## Repository Shape

- This repository is a Rust CLI project.
- The primary binary is `airis`.
- `manifest.toml` drives workspace discovery, generation, orchestration, guards, and related automation.

## Common Commands

```bash
cargo build
cargo build --release
cargo test
cargo test <name>
cargo fmt --check
cargo clippy -- -D warnings
```

`airis verify` is a convenience wrapper, but it **skips `cargo check`/`clippy`/`fmt --check`/`test` when the workspace container is offline** and still prints a green summary. Before pushing, run the `cargo` commands above directly (or `airis up` first) so CI is not the first thing to catch a regression.

## Important Paths

- `src/manifest/` for manifest schema and validation
- `src/commands/` for CLI behavior
- `docs/ai/` for shared AI guidance
- `hooks/` for native git hooks
