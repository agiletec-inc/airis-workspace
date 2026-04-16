# Contributing to AIris

Thank you for your interest in contributing to AIris, a Docker-first monorepo workspace manager built in Rust.

Repository: https://github.com/agiletec-inc/airis-workspace

## Getting Started

### Prerequisites

- Rust toolchain 1.85+ (edition 2024)
- Git

### Build

```bash
git clone https://github.com/agiletec-inc/airis-workspace.git
cd airis-workspace
cargo build
```

### Run Tests

```bash
cargo test
```

### Install Locally

```bash
cargo install --path .
```

## Development Workflow

1. Create a feature branch from `main`.
2. Make your changes.
3. Run `cargo test` and `cargo clippy` to verify correctness and lint.
4. Commit using conventional commit format:
   ```
   <type>(<scope>): <subject>
   ```
   Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`
5. Open a pull request against `main`.

## Code Style

- Rust edition 2024 (`edition = "2024"` in Cargo.toml).
- Run `cargo fmt` before committing.
- Run `cargo clippy` for lint checks.
- Write code comments in English.
- Write error messages in English.

## Architecture Overview

- `src/main.rs` -- CLI entry point using `clap` derive macros.
- `src/manifest.rs` -- manifest.toml schema and helpers.
- `src/templates/mod.rs` -- Handlebars template engine for file generation.
- `src/commands/` -- Command implementations (init, generate, run, doctor, etc.).

Key pattern: `manifest.toml` is the single source of truth. All workspace files (`package.json`, `pnpm-workspace.yaml`, `docker-compose.yml`, `Cargo.toml`) are generated from it and should never be edited manually.

## Testing

- Place unit tests in `#[cfg(test)]` modules within the source files.
- Use the `tempfile` crate for filesystem tests.
- For tests that change the current working directory, use a `Mutex` to prevent race conditions:
  ```rust
  use std::sync::Mutex;
  static DIR_LOCK: Mutex<()> = Mutex::new(());

  #[test]
  fn test_with_directory_change() {
      let _guard = DIR_LOCK.lock().unwrap();
      let original_dir = std::env::current_dir().unwrap();
      let dir = tempfile::tempdir().unwrap();
      std::env::set_current_dir(&dir).unwrap();

      let result = std::panic::catch_unwind(|| {
          // Test logic here
      });

      std::env::set_current_dir(original_dir).unwrap();
      result.unwrap();
  }
  ```
- Integration tests go in the `tests/` directory.

## Reporting Issues

### Bug Reports

Include the following:

- Operating system and version
- Rust version (`rustc --version`)
- Steps to reproduce the issue
- Expected vs. actual behavior

### Feature Requests

Describe the use case and the problem you are trying to solve.

## License

This project is licensed under the MIT License.
