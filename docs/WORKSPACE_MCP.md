# airis-workspace MCP Server

**Status**: Implemented (as the `airis mcp` stdio subcommand)
**Last updated**: 2026-04-20

---

## Vision

Expose workspace-management capabilities as MCP tools so that LLM agents (Claude
Code, Cursor, any MCP-aware client) can drive repo scaffolding, manifest
authoring, and verification with the same primitives humans use on the CLI —
without a second language runtime or an intermediate Python wrapper.

The stdio MCP server ships inside the Rust `airis` binary. There is **no
separate `airis-workspace-mcp` package** and **no `airis-agent` Python service**
in this design. Earlier drafts proposed both; both were dropped after the
functionality they were reaching for was absorbed elsewhere.

---

## Architecture

```
┌─────────────────────────────────────────────────┐
│  Human on CLI            LLM via MCP            │
│  ───────────────────     ─────────────────────  │
│  $ airis gen             call "workspace_gen"   │
│  $ airis verify          call "workspace_verify"│
│                          call "workspace_init"  │
└─────────────────────────────────────────────────┘
                    ↓               ↓
┌─────────────────────────────────────────────────┐
│  airis (single Rust binary)                     │
│  ───────────────────────────────────────────    │
│  • CLI entry (clap)                             │
│  • `airis mcp` stdio JSON-RPC server            │
│  • Shared handlers: discover, generate, doctor, │
│    verify, validate, migrate                    │
└─────────────────────────────────────────────────┘
```

The MCP handler for side-effectful tools (`workspace_gen/verify/doctor/...`)
spawns the current binary as a subprocess so the CLI's stdout noise cannot
contaminate the stdio protocol on the MCP server's own stdout.

---

## Responsibility split

| Concern | Owner |
|--------|-------|
| `manifest.toml` → deterministic files (package.json, pnpm-workspace.yaml, CI, compose.yaml) | Rust CLI (`airis gen`) |
| Discovery of legacy repo layout + proposing a fresh `manifest.toml` | MCP tool `workspace_init` (invoked by LLM) |
| Format-preserving edits to `manifest.toml` (catalog merges, comment retention) | LLM text-editing; there is no TOML re-serializer in the CLI path |
| Command-guard enforcement, hook wiring | Rust CLI (`airis guards`, `airis claude setup`) |
| Cross-server routing, confidence checks, repo-index, suggest | `airis-mcp-gateway` (separate repository; already integrates these) |

The `airis-mcp-gateway` handles "thinking-layer" meta-tools (`airis-confidence`,
`airis-suggest`, `airis-repo-index`, `airis-exec`, `airis-find`). This server
only handles workspace mutations and reads — it is deliberately single-purpose.

---

## Tools

Returned by `tools/list` (see `src/commands/mcp/mod.rs`):

| Tool | Purpose |
|------|---------|
| `workspace_init` | Scan the repo, propose a `manifest.toml` covering apps/libs/compose/catalog. Read-only. |
| `workspace_discover` | Raw discovery facts as JSON (inputs used by `workspace_init`). Read-only. |
| `workspace_cleanup` | List legacy compose files and orphan backups that should be removed. Read-only. |
| `workspace_gen` | Regenerate workspace files from the current `manifest.toml`. Write. |
| `workspace_validate_all` | Run manifest / ports / networks / env / deps validation. Read-only. |
| `workspace_doctor` | Diagnose environment drift and suggest fixes. Read-only. |
| `workspace_verify` | Run `[rule.verify]` commands inside the Docker workspace. Read-only. |
| `workspace_status` | `docker compose ps` output. Read-only. |
| `manifest_validate` | Validate a proposed manifest string without writing. Read-only. |
| `manifest_apply` | Persist a manifest string and (optionally) run `airis gen`. Write. |
| `migration_execute` | Run a batch of file migration steps proposed by `workspace_init`. Write. |

---

## Running the server

```bash
airis mcp
```

This reads JSON-RPC line-framed requests on stdin and writes responses on
stdout. Logging should go to stderr.

### Gateway registration

Register this server from `airis-mcp-gateway/mcp-config.json`:

```json
{
  "mcpServers": {
    "airis-workspace": {
      "command": "airis",
      "args": ["mcp"],
      "detect": ["manifest.toml"]
    }
  }
}
```

The gateway's Dynamic MCP mode keeps `airis-workspace` cold and starts it on
demand when an agent calls `airis-exec airis-workspace:workspace_init` (or any
other tool from this server).

---

## Non-goals

- **No Python wrapper.** Earlier drafts proposed `airis-workspace-mcp` as a
  `uvx`-installed Python MCP server that shells out to `airis`. That extra hop
  buys nothing — the Rust binary already speaks MCP directly.
- **No `airis-agent` service.** The "think, then act" split it represented is
  handled today by the gateway (confidence/suggest/repo-index) and the LLM
  itself editing files. Keeping a third service would only re-introduce
  responsibility overlap.
- **No manifest re-serialization in Rust.** Comment and formatting preservation
  under TOML re-serialization is a known lossy operation; leaving manifest
  edits to the LLM's text editor avoids it entirely.

---

## See also

- `docs/airis-init-architecture.md` — how the (now removed) `airis init` CLI
  flow was replaced by the `workspace_init` MCP tool.
- `src/commands/mcp/mod.rs` — implementation of this server.
