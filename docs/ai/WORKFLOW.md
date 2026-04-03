# Workflow

## Default Flow

1. Read the relevant shared docs and inspect the current implementation.
2. Confirm how the change affects `manifest.toml`, generated files, Docker-first guards, and CLI UX.
3. Implement the smallest coherent change.
4. Run targeted verification first, then broader checks if the change touches shared infrastructure.
5. Summarize behavior changes, verification, and remaining risks.

## Design Bias

- Prefer shared abstractions only when they preserve tool-specific strengths.
- Separate machine-readable config from human-readable agent guidance.
- When supporting multiple AI vendors, isolate differences in adapter files or vendor-specific wiring.
