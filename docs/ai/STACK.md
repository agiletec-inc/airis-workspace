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

## Important Paths

- `src/manifest/` for manifest schema and validation
- `src/commands/` for CLI behavior
- `docs/ai/` for shared AI guidance
- `hooks/` for native git hooks
