# Project Rules

## Source Of Truth

- `manifest.toml` is the machine-readable source of truth for workspace structure, Docker-first orchestration, command guards, and generated files.
- Generated files must stay derivable from `manifest.toml`; do not treat generated output as the place to change behavior.
- AI-facing guidance lives in `docs/ai/`. Do not copy large instruction blocks into vendor-specific adapter files.

## Non-Negotiables

- For **runtime application** configuration (DB-backed settings, tenant boundaries, feature flags), follow `docs/ai/architecture-invariants.md` alongside this file—`manifest.toml` remains the SoT for workspace tooling, not for per-app DB config.
- Preserve the Docker-first value proposition of airis. Changes must not weaken command guards or make host-side workflows the default path.
- Keep `airis init` and related generation flows safe by default. Avoid destructive overwrites unless the feature explicitly supports backups or opt-in replacement.
- Prefer minimal, reviewable diffs. When changing generation or enforcement logic, document the intended invariant.

## Editing Guidance

- Keep `manifest.toml` schema changes backward-compatible when possible.
- Vendor-specific AI files such as `AGENTS.md`, `CLAUDE.md`, and `GEMINI.md` should remain thin adapters that point to shared docs.
- Reusable task guidance belongs in playbooks or skills, not in the adapter files.
