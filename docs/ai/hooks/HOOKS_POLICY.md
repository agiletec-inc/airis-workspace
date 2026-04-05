# Hooks Policy

This file defines portable hook intent shared across AI vendors.

## Shared Intent

- enforce Docker-first command safety
- prevent obviously dangerous commands
- keep hook behavior explainable and auditable
- allow vendor-specific hook implementations when event models differ

## Ownership

| Layer | Owner | What |
|-------|-------|------|
| Spec / rules | airis-monorepo (CLI) | CLAUDE.md, rules/*.md → ~/.claude/ |
| Runtime hooks | airis-mcp-gateway (plugin) | hooks.json, *.sh (PreToolUse, Stop) |
| PATH guards | airis-monorepo (CLI) | ~/.airis/bin/ command wrappers |
| Skills / commands | airis-mcp-gateway (plugin) | skills/, commands/ |

When debugging:
- Rule content wrong → check `~/.airis/claude/` source files (CLI domain)
- Hook not firing → check plugin's `hooks/` directory (plugin domain)
- Command blocked on host → check `~/.airis/bin/` guards (CLI domain)

## Boundaries

- This policy is not the place for vendor-specific JSON or settings formats.
- Claude-specific, Codex-specific, or Gemini-specific hook wiring may extend this policy, but should not contradict it.
