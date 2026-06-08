# Hooks Policy

This file defines portable hook intent shared across AI vendors.

## Shared Intent

- respect workload runtime boundaries (Docker workloads run in-container; Workers/edge/native run host-native — see `~/.claude/rules/runtime-workflow.md`)
- prevent obviously dangerous commands
- keep hook behavior explainable and auditable
- allow vendor-specific hook implementations when event models differ

## Ownership

| Layer | Owner | What |
|-------|-------|------|
| Spec / rules | airis-workspace (CLI) | CLAUDE.md, rules/*.md → ~/.claude/ |
| Runtime hooks | airis-mcp-gateway (plugin) | hooks.json, *.sh (PreToolUse, Stop) |
| Skills / commands | airis-mcp-gateway (plugin) | skills/, commands/ |

When debugging:
- Rule content wrong → check `~/.airis/claude/` source files (CLI domain)
- Hook not firing → check plugin's `hooks/` directory (plugin domain)

## Boundaries

- This policy is not the place for vendor-specific JSON or settings formats.
- Claude-specific, Codex-specific, or Gemini-specific hook wiring may extend this policy, but should not contradict it.
