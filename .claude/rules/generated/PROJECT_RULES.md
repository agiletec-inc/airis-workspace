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
