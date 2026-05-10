# Workspace Initialization Architecture

## Overview

Workspace bootstrapping used to live in the Rust CLI as `airis init`. That
command ran discovery, serialized a fresh `manifest.toml`, then generated
downstream files. It was removed because the discovery → manifest step is
better done by an LLM (format-preserving edits, catalog consolidation,
judgement about framework detection edge cases), while the manifest → files
step is a clean deterministic pipeline that belongs in the Rust CLI.

Today the flow is split across two surfaces:

```
┌─────────────────────────────────────────────────────────┐
│                                                         │
│   LLM side (Claude Code via MCP)                        │
│   ────────────────────────────                          │
│   workspace_discover   — raw repo facts                 │
│   workspace_init       — propose manifest.toml          │
│   manifest_validate    — sanity-check proposal          │
│   manifest_apply       — persist manifest.toml          │
│   migration_execute    — move legacy files              │
│                                                         │
│                 ↓                                       │
│                                                         │
│   Rust CLI side                                         │
│   ──────────────                                        │
│   airis gen            — manifest.toml → files          │
│   airis validate all   — check generated artifacts      │
│   airis up             — boot the Docker workspace      │
│   airis verify         — run quality gates              │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

The LLM side is exposed through `airis mcp` (stdio MCP server). The Rust side
is the existing CLI, now with no discovery-to-manifest entry point.

---

## Why the split

**Problem**: TOML re-serialization loses comments and subtle formatting.

```toml
# === Apps Configuration ===   ← comment disappears after re-serialize
[apps.dashboard]
path = "apps/dashboard"        ← indentation mutates
type = "nextjs"
```

**Resolution**: only the LLM edits `manifest.toml`. The Rust CLI reads it and
writes downstream files. This keeps the CLI fast and predictable, while giving
users the flexibility of natural-language-driven manifest authoring.

---

## Discovery (shared module)

Both `workspace_init` and `workspace_discover` use `src/commands/discover/`
to gather facts:

- `apps/*/package.json` → apps + framework detection
- `libs/*/package.json` → libraries
- `apps/*/Cargo.toml` → Rust
- `apps/*/pyproject.toml` → Python
- root `package.json` devDependencies → candidate catalog entries
- existing `docker-compose*.yml` / `compose*.yaml` → legacy compose location

The same `DiscoveryResult` struct backs both the MCP response and the proposed
manifest string that `workspace_init` returns.

---

## Generation (`airis gen`)

`src/commands/generate/` owns the deterministic manifest → files pipeline:

1. **package.json** (Hybrid ownership — merged)
2. **pnpm-workspace.yaml** (Tool-owned — overwritten)
3. **.github/workflows/ci.yml** (Tool-owned — overwritten)
4. **.github/workflows/release.yml** (Tool-owned — overwritten)

`compose.yaml`, `Dockerfile`, and `.env.example` are User-owned since 4.0.1 and
are never overwritten.

---

## Verification (`airis verify`)

Runs `[rule.verify]` commands defined in `manifest.toml` and stack-derived
app-specific checks — inside the Docker workspace when the container is up,
with a warning when it is not. Also exposed as `workspace_verify` over MCP.

---

## Typical bootstrap from Claude Code

```text
user: scaffold an airis workspace here
llm: calls workspace_discover → sees apps/, libs/, legacy docker-compose.yml
llm: calls workspace_init → receives proposed manifest.toml
llm: calls manifest_validate with the proposed string
llm: calls manifest_apply(manifest, run_gen=true)  # writes manifest.toml + runs airis gen
llm: calls workspace_verify                         # confirms environment
```

No `airis init` CLI call appears anywhere — it was never needed once the MCP
surface existed.
