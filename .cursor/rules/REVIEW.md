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
