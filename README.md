# airis

[![Crates.io](https://img.shields.io/crates/v/airis.svg)](https://crates.io/crates/airis)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/agiletec-inc/airis-monorepo/actions/workflows/ci.yml/badge.svg)](https://github.com/agiletec-inc/airis-monorepo/actions/workflows/ci.yml)

![airis demo](assets/airis-demo.gif)

**Stop AI coding agents from polluting your host environment.**

Claude Code ran `pnpm install` on your host? Never again.

airis is a Docker-first monorepo workspace manager that guards your host environment from LLM-triggered package manager commands. It auto-generates `package.json`, `pnpm-workspace.yaml`, `docker-compose.yml`, and CI workflows from a single `manifest.toml`.

> **airis is not a replacement for NX or Turborepo.** It's a guard rail layer that works alongside your existing build tools. Use them together for maximum safety.

---

## The Problem

You're coding with Claude Code or Cursor. Things are going great. Then:

```bash
# Claude says: "I'll install the dependencies for you"
$ pnpm install
# 200 packages installed to your HOST machine
# node_modules now polluting your local environment
```

Or worse -- Claude edits `package.json` directly, your versions diverge from your teammates, and CI fails.

**This happens constantly.** AI coding agents don't understand Docker-first workflows. They just run commands.

## The Solution

```bash
$ pnpm install
# ERROR: 'pnpm' must run inside Docker workspace
#    Use: airis install

$ airis install
# Runs pnpm install inside your Docker container
```

When an AI agent tries to run `pnpm install`, it gets blocked with a helpful error. Your host stays clean. If it breaks `package.json`, just regenerate:

```bash
$ airis generate files
# Regenerated package.json from manifest.toml
```

Since `manifest.toml` is the single source of truth, all derived files can be regenerated instantly.

---

## Install

### From crates.io (recommended)

```bash
cargo install airis
```

### From GitHub Releases

Pre-built binaries are available for macOS (ARM/Intel), Linux (x64/ARM), and Windows:

```bash
# macOS / Linux
curl -fsSL https://github.com/agiletec-inc/airis-monorepo/releases/latest/download/install.sh | bash
```

### From source

```bash
cargo install --git https://github.com/agiletec-inc/airis-monorepo
```

---

## Quick Start

### New project

```bash
mkdir my-monorepo && cd my-monorepo
airis init --write        # Creates manifest.toml
airis generate files      # Generates all config files
airis up                  # Start Docker services
```

### Existing project

```bash
cd your-monorepo
airis init                # Auto-discovers apps, libs, compose files (dry-run)
airis init --write        # Executes migration
airis generate files      # Generates workspace files
airis up                  # Start everything
```

What happens:
1. Discovers apps in `apps/`, libs in `libs/`
2. Detects frameworks (Next.js, Vite, Hono, Rust, Python)
3. Finds docker-compose.yml files
4. Generates `manifest.toml` as single source of truth
5. Never overwrites existing `manifest.toml`

---

## How It Works

### Single Source of Truth

```
manifest.toml (you edit this)
    |
    v  airis generate files
package.json           <-- auto-generated
pnpm-workspace.yaml    <-- auto-generated
docker-compose.yml     <-- auto-generated
.github/workflows/     <-- auto-generated
```

### Command Guards

Block dangerous commands on your host:

```toml
# manifest.toml
[guards]
deny = ["npm", "yarn", "pnpm", "bun"]
forbid = ["docker compose down -v"]
```

```bash
$ npm install
# BLOCKED: 'npm' is not allowed on host
#    Use: airis install
```

### Command Remapping

Auto-translate AI agent commands to safe alternatives:

```toml
[remap]
"npm install" = "airis install"
"pnpm install" = "airis install"
"docker compose up" = "airis up"
```

When an AI agent runs `npm install`, it automatically becomes `airis install` (which runs inside Docker).

### Version Catalog

Centralized dependency management across your monorepo:

```toml
[packages.catalog]
react = "latest"        # resolves to ^19.x.x
next = "lts"            # resolves to LTS version
typescript = "^5.0"     # used as-is
```

Real versions are resolved from the npm registry and written to `pnpm-workspace.yaml`. Everyone gets the same versions.

---

## Commands

### Workspace Management

```bash
airis init                # Discover & create manifest (dry-run)
airis init --write        # Execute migration
airis generate files      # Regenerate from manifest
airis doctor              # Check workspace health
airis doctor --fix        # Auto-repair issues
airis guards install      # Install command guards globally
```

### Development

```bash
airis up                  # Start Docker services
airis down                # Stop services
airis shell               # Enter container shell
airis dev                 # Start dev servers
airis build               # Build projects
airis test                # Run tests
airis lint                # Run linting
airis clean               # Remove build artifacts
```

### Build & Deploy

```bash
airis build --affected --docker     # Build changed projects only
airis bundle apps/api               # Generate deployment package
airis policy check                  # Pre-deploy validation
```

### Diagnostics

```bash
airis validate            # Validate workspace config
airis diff                # Preview manifest vs generated diff
airis deps tree           # Visualize dependency graph
airis manifest json       # Output workspace config as JSON (for automation)
```

---

## Use with NX / Turborepo / Bazel

airis is a guard rail layer, not a build orchestrator. Use it alongside your existing tools:

| Tool | What it does | How to combine |
|------|-------------|----------------|
| **NX** | Build orchestration, dependency graph | airis guards + NX builds |
| **Turborepo** | Fast task execution, caching | airis guards + Turbo caching |
| **Bazel** | Hermetic builds at scale | airis guards + Bazel builds |

Example with Turborepo:

```toml
# manifest.toml
[commands]
build = "docker compose exec workspace pnpm turbo run build"
test = "docker compose exec workspace pnpm turbo run test"

[guards]
deny = ["npm", "yarn", "pnpm"]
```

Turborepo handles caching and orchestration. airis ensures everything runs inside Docker.

---

## Configuration

airis is configured through a single `manifest.toml` file. See the full reference:

- **[manifest.toml Reference](docs/CONFIG.md)** -- complete field-by-field documentation

### Minimal example

```toml
version = 1
mode = "docker-first"

[workspace]
name = "my-project"
package_manager = "pnpm@10.22.0"
service = "workspace"
image = "node:22-alpine"

[packages]
workspaces = ["apps/*", "libs/*"]

[packages.catalog]
react = "latest"
next = "latest"
typescript = "latest"

[guards]
deny = ["npm", "yarn", "pnpm", "bun"]

[commands]
up = "docker compose up -d --build"
down = "docker compose down"
shell = "docker compose exec workspace sh"
build = "docker compose exec workspace pnpm build"
test = "docker compose exec workspace pnpm test"

[remap]
"pnpm install" = "airis install"
"npm install" = "airis install"
```

---

## Project Structure

```
my-monorepo/
  manifest.toml           # SINGLE SOURCE OF TRUTH (edit this)
  package.json            # auto-generated (DO NOT EDIT)
  pnpm-workspace.yaml     # auto-generated (DO NOT EDIT)
  docker-compose.yml      # auto-generated (DO NOT EDIT)
  apps/
    dashboard/
    api/
  libs/
    ui/
    db/
```

---

## Part of the AIRIS Ecosystem

| Component | Purpose |
|-----------|---------|
| **airis** (this repo) | Docker-first monorepo guard rails |
| **[airis-agent](https://github.com/agiletec-inc/airis-agent)** | LLM intelligence layer for editors |
| **[airis-mcp-gateway](https://github.com/agiletec-inc/airis-mcp-gateway)** | Unified MCP proxy (90% token reduction) |
| **[mindbase](https://github.com/agiletec-inc/mindbase)** | Cross-session memory |

---

## Documentation

- [manifest.toml Reference](docs/CONFIG.md)
- [Commands Guide](docs/commands.md)
- [Init Architecture](docs/airis-init-architecture.md)
- [Contributing](CONTRIBUTING.md)
- [Changelog](CHANGELOG.md)

---

## Contributing

Contributions welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

Priority areas:
- Guard system improvements
- Multi-compose orchestration
- New framework detection (Bun, Deno, etc.)

---

## License

MIT License -- see [LICENSE](LICENSE).

---

## Author

[@agiletec-inc](https://github.com/agiletec-inc)

Born from frustration with AI coding agents breaking Docker-first workflows repeatedly.
