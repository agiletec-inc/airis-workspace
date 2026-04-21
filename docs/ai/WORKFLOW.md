# Workflow

## Default Flow

1. **Research**: Read the relevant shared docs and inspect the current repository structure. Airis uses conventions (apps/*, libs/*) to automatically detect projects.
2. **Infrastructure (manifest.toml)**: If you need to change ports, volumes, or add explicit overrides, modify `manifest.toml`. Run `airis gen` to sync.
3. **Dependencies (package.json)**: If you need to add libraries, edit the project's `package.json` directly. Airis preserves your edits.
4. **Execution**: Always use `airis up` to start the environment. It ensures configuration is synced and dependencies are installed inside Docker.
5. **Verification**: Run `airis verify` before finishing a task to ensure environment integrity and quality gates.

## Design Bias

- **Convention over Configuration**: Prefer repository structure over redundant manifest declarations.
- **Environment Focus**: Treat Airis as an environment orchestrator, not a task runner or package manager.
- **Hygiene**: Never introduce host-side dependencies. Keep AI agents inside the container.
