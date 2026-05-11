<!-- BEGIN GENERATED airis gen -->
## Shared Rules (Auto-generated)
Primary project instructions. Read these first.

### Source: docs/ai/PROJECT_RULES.md

# Project Rules

## Architectural Boundaries

Airis-workspace is an **Environment Source-of-Truth, Config Compiler, and Hygiene Enforcer**. It is not a build orchestrator (like Nx/Turborepo) or a package manager replacement. Its primary value is ensuring a host-hygienic Docker development environment through automated orchestration.

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
- Preserve the Docker-first value proposition of airis. Changes must not weaken command guards or make host-side workflows the default path.
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


### Source: docs/ai/REVIEW.md

# Review

## Primary Checks

- Does the change preserve or strengthen Docker-first enforcement?
- Does it keep `manifest.toml` authoritative for workspace and guard behavior?
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


<!-- END GENERATED -->

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

This file is generated by `airis docs sync`. Keep this file short; the shared docs are the source of truth.

Read these files first:
- `docs/ai/PROJECT_RULES.md`
- `docs/ai/WORKFLOW.md`
- `docs/ai/REVIEW.md`
- `docs/ai/STACK.md`

Repository rules:
- `manifest.toml` remains the source of truth for Docker-first orchestration, command guards, and generated workspace config.
- Follow the shared docs for architecture, workflow, and review expectations.
- Keep verification proportional to the change.
- Project playbooks and reusable task guidance live under `docs/ai/playbooks`.
- Hook intent and shared guard policy live in `docs/ai/hooks/HOOKS_POLICY.md`; Claude-specific hook wiring may extend it but should not contradict it.

Testing policy:
- **Mock policy: forbidden** — Never mock external services (DB, APIs). Use real instances or local emulators.

## Architecture

The `airis gen` pipeline: `manifest.toml` → `Manifest::load()` (schema.rs) → discovery (commands/discover/) → generation (commands/generate/compose_gen.rs + tsconfig_gen.rs + ai_gen.rs) → write output files.

Key modules:
- `src/manifest/schema.rs` — `Manifest` struct (full toml schema). All sections are `#[serde(default)]`; the manifest is intentionally thin.
- `src/conventions.rs` — framework lookup table (ports, health paths, isolated dirs, scripts) keyed by framework name. Manifest fields override these; never hardcode at call sites.
- `src/ownership.rs` — `Ownership::Tool | User` per file path. Tool-owned files get backup+overwrite; user-owned files are never touched by gen.
- `src/workspace.rs` — resolves workspace glob patterns from `manifest.toml [packages].workspaces` → `pnpm-workspace.yaml` → `Cargo.toml [workspace]`, in that priority order.
- `src/commands/discover/` — scans `apps/*/` and `libs/*/` for projects; detects framework from `package.json` scripts/deps. Used by gen and migrate.
- `src/commands/generate/compose_gen.rs` — merges airis-managed services into existing `compose.yaml`, preserving services without `x-airis-managed: true`.
- `src/commands/guards/` — global shims (`~/.airis/bin/`) that intercept guarded commands (pnpm, npm, python, …) and redirect to Docker when inside a workspace.
- `src/dag.rs` — dependency graph from manifest + pnpm-lock.yaml, used by `airis affected` and `airis build --affected`.
- `src/docker_build/` — Dockerfile generation and BuildKit cache integration.

## Running Tests

```bash
cargo test                          # all tests
cargo test <name>                   # filter by test name
cargo test -p airis-workspace       # explicit package
cargo fmt --check && cargo clippy -- -D warnings && cargo test  # full pre-push check
```

Do not rely on `airis verify` alone — it silently skips cargo checks when the workspace container is offline.
