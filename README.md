# airis-workspace

**Environment Source-of-Truth, Config Compiler, and Hygiene Enforcer.**

Airis ensures a host-hygienic, Docker-first development environment through automated orchestration and global shims.

## Key Features

- **Docker-First Enforcement**: Commands like `pnpm`, `npm`, and `cargo` are automatically routed to Docker containers when inside an Airis workspace.
- **Thin Manifest**: Declare intent in `manifest.toml`, and Airis handles the rest (Compose generation, TSConfig paths, etc.).
- **Smart Shims**: Global shims in `~/.airis/bin` that detect workspaces and proxy commands intelligently.
- **Hygiene First**: Prevents accidental host-side dependency installation.

## Quick Start

1. **Install airis CLI**:
   ```bash
   cargo install --path .
   ```

2. **Setup Global Shims**:
   ```bash
   airis guards install --global
   ```
   *Follow the instructions to add `~/.airis/bin` to your `PATH`.*

3. **Initialize a Project**:
   Create a `manifest.toml` in your project root.

4. **Start Environment**:
   ```bash
   airis up
   ```
   Now, any command like `pnpm install` or `pnpm dev` will run safely inside Docker.

## Common Commands

- `airis up`: Start the Docker environment.
- `airis run <task>`: Run a specific task defined in manifest or conventions.
- `airis shell`: Enter the workspace container shell.
- `airis workspace uninstall`: Safely remove airis artifacts from the current repo.

## Why Airis?

Airis is not a build tool; it's an **environment orchestrator**. It solves the "it works on my machine" problem by ensuring that everyone on the team uses the exact same environment, enforced by the CLI itself.

---

License: MIT
