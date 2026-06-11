# Workflow

## Default Flow

1. **Research**: Read the relevant shared docs and inspect the current repository structure. Airis uses conventions (apps/*, libs/*) to automatically detect projects.
2. **Infrastructure (manifest.toml)**: If you need to change ports, volumes, or add explicit overrides, modify `manifest.toml`. Run `airis gen` to sync.
3. **Dependencies (package.json)**: If you need to add libraries, edit the project's `package.json` directly. Airis preserves your edits.
4. **Execution**: Always use `airis up` to start the environment. It ensures configuration is synced and dependencies are installed inside Docker.
5. **Verification**: Run `airis verify` before finishing a task to ensure environment integrity and quality gates.

## Design Bias

- **Convention over Configuration**: Prefer repository structure over redundant manifest declarations.
- **Convention Focus**: Treat Airis as a convention-unification engine across repos (AI adapters, docs, tsconfig, scaffolding), not a task runner or package manager.
- **Hygiene (containerized repos)**: Keep host-side dependencies out and run inside the container. This does not apply to repos where host execution is canonical (Edge/Workers, native desktop apps).

## Operational Notes

These are easy to discover the hard way. Read once, save a debugging session.

- **Direct push to `main` is rejected** by a repository rule. All changes land via pull request — branch off, push the branch, open a PR, and merge from there.
- **`airis verify` skips runtime checks when the workspace container is offline.** It will report "All quality checks passed" even if `cargo clippy`, `cargo fmt --check`, and `cargo test` were never run. Before pushing, either bring the container up (`airis up`) or run `cargo fmt --check && cargo clippy -- -D warnings && cargo test` directly so CI does not surface the first real failure.
- **`docs/ai/*.md` is the source of truth for the AI adapter files.** When you edit `PROJECT_RULES.md`, `WORKFLOW.md`, `REVIEW.md`, or `STACK.md`, run `airis docs sync` to refresh `CLAUDE.md`, `AGENTS.md`, and `GEMINI.md` in one pass. Never hand-edit the generated block in the adapter files.
