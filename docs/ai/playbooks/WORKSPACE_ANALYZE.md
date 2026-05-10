# Workspace Analyze Playbook

A guide for LLMs handling cases that the deterministic CLI (`airis gen`) cannot
resolve on its own.

## When to use

Trigger this playbook when one of the following holds:

- The repository uses a framework not covered by `src/conventions.rs` (port,
  health endpoint, isolated dirs are unknown).
- The user wants to override generated services beyond what `manifest.toml`
  exposes (custom command, custom env, profile assignment, etc.).
- An existing `compose.yaml` has structural conflicts the merge logic cannot
  resolve mechanically (e.g. service renamed, network restructured).
- A new repository needs cold-start `manifest.toml` proposed before any code.

If none of the above applies, just call `workspace_gen` — the CLI will do the
right thing on its own.

## How `airis gen` decides what to write

The CLI is the source of truth. It:

1. Resolves workspace patterns from `manifest.toml [packages].workspaces` →
   `pnpm-workspace.yaml` `packages:` → `Cargo.toml [workspace] members`. No
   hardcoded fallback.
2. Discovers projects from those patterns (one per directory with
   `package.json`/`Cargo.toml`/`pyproject.toml`). If the root itself is a
   single project, the root is treated as one app.
3. Generates `compose.yaml` at the project root with production-ready fields:
   `restart: unless-stopped`, `healthcheck`, `ports`, named volumes for every
   `isolated_dir` from conventions.
4. Stamps every generated service with `x-airis-managed: true`.
5. **Merges** with any existing root compose: services without
   `x-airis-managed` are preserved verbatim; services with the marker are
   replaced (or removed if no longer generated).

This means the LLM never needs to write `compose.yaml` directly.

## Standard workflow

1. **Discover.** Call `workspace_discover` to get the raw facts (apps, libs,
   compose files, catalog).
2. **Read existing state.** Use `resources/read` on `manifest.toml`,
   `compose.yaml`, and any framework files (`pnpm-workspace.yaml`,
   `Cargo.toml`) you need.
3. **Decide what's missing or wrong.** Compare the discovered facts to
   conventions:
   - Unknown framework? Propose adding a `[stack.<name>]` definition to
     `manifest.toml` so the CLI knows the port/health path/isolated dirs.
   - Custom port/env on a known framework? Propose an `[[app]]` override.
   - Need a custom service (postgres, redis, mock-server)? Add it to
     `compose.yaml` directly **without** `x-airis-managed`. The CLI will
     leave it alone forever.
4. **Apply manifest changes.** Call `manifest_apply` with `run_gen: true` so
   the CLI regenerates `compose.yaml` afterwards.
5. **Verify.** Read the resulting `compose.yaml` and confirm the merge
   preserved user-authored services and produced sensible defaults for
   generated ones.

## Service ownership rules

| Marker | Owner | Behavior on regeneration |
|--------|-------|--------------------------|
| `x-airis-managed: true` | CLI | Replaced from manifest each run; removed if no longer derived |
| (no marker) | User / LLM | Preserved verbatim, never touched |

When in doubt, **omit the marker**. A service the user controls is safer than
one that disappears the next time `airis gen` runs.

## Anti-patterns

- **Do not** edit a service that already has `x-airis-managed: true`. Either
  remove the marker (taking permanent ownership) or update `manifest.toml`.
- **Do not** add `apps/*`/`libs/*` to `[packages].workspaces` "just in case".
  If the project doesn't actually use those directories, leave it empty —
  the CLI will fall back to the real workspace definition.
- **Do not** write `pnpm-workspace.yaml` unless pnpm itself needs it. The CLI
  reads it as a source of truth, so a fake one will mislead future runs.
