# Workflow

## Default Flow

1. **Research**: Read the relevant shared docs and inspect the current repository structure. Airis uses conventions (apps/*, libs/*) to automatically detect projects.
2. **Infrastructure (manifest.toml)**: If you need to change ports, volumes, or add explicit overrides, modify `manifest.toml`. Run `airis workspace gen` to sync.
3. **Dependencies (package.json)**: If you need to add libraries, edit the project's `package.json` directly. Airis preserves your edits.
4. **Execution (host-native default)**: Build and run with the native toolchain directly — for this Rust CLI that is `cargo build` / `cargo run`. Docker is only for local stateful infra or deploy-parity checks, not for everyday execution. `airis workspace gen` only writes `compose.yaml` / `tsconfig.json` / AI-rule files; it does not run anything. See `~/.claude/rules/runtime-workflow.md` for the org-wide matrix.
5. **Verification**: Before finishing, run the native checks directly — `cargo fmt --check && cargo clippy -- -D warnings && cargo test`.

## Design Bias

- **Convention over Configuration**: Prefer repository structure over redundant manifest declarations.
- **Convention Focus**: Treat Airis as a convention-unification engine across repos (AI adapters, docs, tsconfig, scaffolding), not a task runner or package manager.
- **Host-native default**: Build, run, and test with the native toolchain on the host. Use Docker only for local stateful infra (DBs) or deploy-parity verification; GPU/k3s workloads run on the cluster. Per-repo overrides win. SSoT: `~/.claude/rules/runtime-workflow.md`.

## Operational Notes

These are easy to discover the hard way. Read once, save a debugging session.

- **Direct push to `main` is rejected** by a repository rule. All changes land via pull request — branch off, push the branch, open a PR, and merge from there.
- **`airis workspace gen` does not run tests.** It only writes `compose.yaml` / `tsconfig.json` / AI-rule files. Run `cargo fmt --check && cargo clippy -- -D warnings && cargo test` directly before pushing so CI is not the first thing to surface a regression.
- **`docs/ai/*.md` is the source of truth for the AI adapter files.** When you edit `PROJECT_RULES.md`, `WORKFLOW.md`, `REVIEW.md`, or `STACK.md`, run `airis workspace docs sync` to refresh `CLAUDE.md`, `AGENTS.md`, and `GEMINI.md` in one pass. Never hand-edit the generated block in the adapter files.
