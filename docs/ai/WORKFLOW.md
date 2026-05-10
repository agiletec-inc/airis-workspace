# Workflow

## Default Flow

1. **Research**: Read the relevant shared docs and inspect the current repository structure. Airis uses conventions (apps/*, libs/*) to automatically detect projects.
2. **Infrastructure (manifest.toml)**: If you need to change ports, volumes, or add explicit overrides, modify `manifest.toml`. Run `airis gen` to sync.
3. **Dependencies (package.json)**: If you need to add libraries, edit the project's `package.json` directly. Airis preserves your edits.
4. **Execution**: Always use `airis up` to start the environment. It ensures configuration is synced and dependencies are installed inside Docker.
5. **Verification**: Run `airis verify` before finishing a task to ensure environment integrity and quality gates.

## Design Bias

- **Convention over Configuration**: Prefer repository structure over redundant manifest declarations.
- **Environment Focus**: Treat Airis as an environment orchestrator, not a task runner or package manager.
- **Hygiene**: Never introduce host-side dependencies. Keep AI agents inside the container.

## Operational Notes

These are easy to discover the hard way. Read once, save a debugging session.

- **Direct push to `main` is rejected** by a repository rule. All changes land via pull request — branch off, push the branch, open a PR, and merge from there.
- **The pre-commit hook auto-bumps the version on every commit.** `airis bump-version --auto` rewrites both `Cargo.toml` and `Cargo.lock` in lockstep; do not manually sync the lockfile, the hook handles it. If you ever see Cargo.toml/Cargo.lock drift after a commit, the bump binary is stale — run `cargo install --path .` to refresh it.
- **The post-commit hook reinstalls the `airis` binary in the background** (`cargo install --path . --quiet 2>/dev/null &`). After committing changes to the CLI itself, the next commit will use the rebuilt binary; manual reinstall is rarely needed.
- **`airis verify` skips runtime checks when the workspace container is offline.** It will report "All quality checks passed" even if `cargo clippy`, `cargo fmt --check`, and `cargo test` were never run. Before pushing, either bring the container up (`airis up`) or run `cargo fmt --check && cargo clippy -- -D warnings && cargo test` directly so CI does not surface the first real failure.
