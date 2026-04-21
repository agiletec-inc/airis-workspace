# WORKSPACE Philosophy

Airis workspaces are designed to be **host-hygienic** and **transparent**. Whether you are working on a single-service app or a complex monorepo, Airis ensures that your tools run in the right environment.

## Two Modes of Operation

### 1. Transparent Proxy (Standalone Mode)
If you have a `compose.yml` (or any of its 4 variations), Airis automatically activates.
- **Trigger**: Presence of `compose.yml`.
- **Behavior**: Smart-shims (pnpm, python, etc.) redirect commands to the container.
- **Source of Truth**: The existing `compose.yml`.

### 2. Managed Orchestration (Manifest Mode)
If you have a `manifest.toml`, Airis acts as a **Config Compiler**.
- **Intent**: `manifest.toml` declares your workspace structure, dependencies, and policies.
- **Artifacts**: `airis gen` generates `compose.yaml`, `tsconfig.json`, and other configs.
- **Source of Truth**: `manifest.toml`.

## Core Principles

1. **Host Hygiene**: No `node_modules` or `target` folders on the host. Everything stays in Docker volumes.
2. **Muscle Memory Preservation**: You run `pnpm install`, and Airis handles the redirection. No new command patterns to learn for your daily workflow.
3. **Environment Parity**: Everyone on the team runs the exact same container, enforced by the shims.

## Summary

- **`manifest.toml`**: The **Intent**. Where you define how the workspace should look.
- **`compose.yaml`**: The **Execution**. Where the containers and volumes are defined (either manually or generated).
- **`Smart-Shims`**: The **Bridge**. The transparent proxy that connects your shell to the Docker environment.
