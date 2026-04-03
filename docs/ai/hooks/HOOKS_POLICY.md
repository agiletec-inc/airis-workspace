# Hooks Policy

This file defines portable hook intent shared across AI vendors.

## Shared Intent

- enforce Docker-first command safety
- prevent obviously dangerous commands
- keep hook behavior explainable and auditable
- allow vendor-specific hook implementations when event models differ

## Boundaries

- This policy is not the place for vendor-specific JSON or settings formats.
- Claude-specific, Codex-specific, or Gemini-specific hook wiring may extend this policy, but should not contradict it.
