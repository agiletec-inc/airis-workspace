<!-- BEGIN GENERATED airis gen -->
## Shared Rules (Auto-generated)
Primary project instructions. Read these first.

### Source: docs/ai/PROJECT_RULES.md

# Project Rules

## Architectural Boundaries

Airis-workspace is a **polyglot monorepo convention-unification engine**. From a thin `manifest.toml` it keeps a heterogeneous set of repositories consistent: AI adapter files, shared docs, `tsconfig.json`, version scheme, and project scaffolding. It is not a build orchestrator (like Nx/Turborepo) or a package manager replacement. Docker development-environment generation (`compose.yaml`, volume hygiene) is **one module**, serving the subset of repositories that are containerized — not the whole tool.

## Design Principles

- **Principle 1: Convention first, manifest second, inference last.**
  The existence and basic properties of projects are derived from repository structure (e.g., `apps/*`, `libs/*`). `manifest.toml` is used for overrides.
- **Principle 2: Manifest declares intent and exceptions, not everything.**
  The manifest should be "thin". Avoid redundant declarations. Use it to specify frameworks, custom ports, or explicit dependencies that cannot be inferred.
- **Principle 3: Generated files are outputs, not the primary source of truth.**
  `package.json` (partial), `compose.yaml`, and `tsconfig.json` are artifacts generated to maintain environmental integrity. 
- **Principle 4: Discovery and scanning never overwrite explicit intent.**
  Import scanning and automatic detection are advisory mechanisms. They may suggest missing dependencies or configurations but must never overwrite explicit definitions in `manifest.toml` or manual edits in `package.json` (for dependencies/scripts).

## Source Of Truth
...

## Non-Negotiables

- For **runtime application** configuration (DB-backed settings, tenant boundaries, feature flags), follow `docs/ai/architecture-invariants.md` alongside this file—`manifest.toml` remains the SoT for workspace tooling, not for per-app DB config.
- For containerized repositories, preserve the integrity of the Docker module (safe `compose.yaml` merge that never destroys user-authored services, volume hygiene). Do not weaken it — but do not impose Docker-first defaults on repositories that don't use it (e.g. Edge/Workers, native desktop apps).
- Keep `airis gen` and the `workspace_init`/`manifest_apply` MCP tools safe by default. Avoid destructive overwrites unless the feature explicitly supports backups or opt-in replacement.
- Prefer minimal, reviewable diffs. When changing generation or enforcement logic, document the intended invariant.

## Editing Guidance

- Keep `manifest.toml` schema changes backward-compatible when possible.
- Vendor-specific AI files such as `AGENTS.md`, `CLAUDE.md`, and `GEMINI.md` should remain thin adapters that point to shared docs.
- Reusable task guidance belongs in playbooks or skills, not in the adapter files.


### Source: docs/ai/WORKFLOW.md

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


### Source: docs/ai/REVIEW.md

# Review

## Primary Checks

- Does the change keep the convention-unification engine coherent — AI adapters, docs, `tsconfig.json`, and scaffolding consistent across repos?
- Does it keep `manifest.toml` authoritative as the thin source of truth, and the Docker module safe for containerized repos?
- Are generated files or adapters clearly marked and reproducible?
- Are vendor-specific differences isolated instead of duplicated into shared docs?

## Verification Expectations

- Run the smallest relevant Rust tests for touched commands and manifest behavior.
- If CLI output or serialization changes, verify the command path directly.
- Call out gaps when a full integration check is too expensive for the current change.


### Source: docs/ai/STACK.md

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


<!-- END GENERATED -->

# GEMINI.md

<!-- Generated by `airis docs sync`. -->

Primary project instructions:
@./docs/ai/PROJECT_RULES.md
@./docs/ai/WORKFLOW.md
@./docs/ai/REVIEW.md
@./docs/ai/STACK.md

Reusable playbooks:
@./docs/ai/playbooks

Hook policy:
@./docs/ai/hooks/HOOKS_POLICY.md

Testing policy:
- **Mock policy: forbidden** — Never mock external services (DB, APIs). Use real instances or local emulators.

`manifest.toml` is the machine-readable source of truth for convention generation (AI adapters, docs, tsconfig, scaffolding) and, for containerized repos, Docker environment generation.