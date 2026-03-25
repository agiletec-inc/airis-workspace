# airis-monorepo

**The Docker-first monorepo manager for the vibe coding era.**

[![Crates.io](https://img.shields.io/crates/v/airis.svg)](https://crates.io/crates/airis)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/agiletec-inc/airis-monorepo/actions/workflows/ci.yml/badge.svg)](https://github.com/agiletec-inc/airis-monorepo/actions/workflows/ci.yml)

![airis demo](assets/airis-demo.gif)

One manifest file. Every config generated. Your AI pair-programmer stays inside the container where it belongs.

---

## Why airis exists

Development tools were designed for humans. Humans read docs, memorize project conventions, and don't forget which package manager to use mid-session. LLMs do.

We wanted Docker-first development for one simple reason: **reproducibility** — every dependency inside a container, same behavior on every machine. But when your AI pair-programmer writes the code, Docker-first breaks in ways it never did with human developers:

1. **The AI forgets the rules.** After context compression or a long session, your coding agent forgets to prefix commands with `docker compose exec`. It runs `pnpm install` on the host. Dependencies leak out of the container. Reproducibility is gone.

2. **It picks the wrong tool.** Your project uses pnpm, but the AI reaches for npm or yarn. Now you have a `package-lock.json` sitting next to your `pnpm-lock.yaml`.

3. **Docker boilerplate is fragile.** Manually wiring `turbo prune` into multi-stage Dockerfiles, keeping `compose.yml` volumes in sync, making sure `node_modules` and pnpm store never leak onto the host — one mistake and your "Docker-first" setup is Docker-in-name-only.

We fixed these one at a time. Command guards that block `npm`/`yarn` and redirect `pnpm` through Docker. A manifest that generates Dockerfiles, compose configs, and CI workflows so the boilerplate can't drift. Named volumes that structurally prevent dependency leakage.

The result is airis — not a replacement for Turborepo or NX (we use Turborepo ourselves and `turbo prune` internally), but **the layer that keeps Docker-first actually working** when your AI pair-programmer has a short memory.

```
┌─────────────────────────────────────────────┐
│  Your Build Tool (Turborepo / NX / Bazel)   │  ← task orchestration, caching
├─────────────────────────────────────────────┤
│  airis                                      │  ← config generation, Docker wiring,
│                                             │     filesystem boundaries, CI/CD
├─────────────────────────────────────────────┤
│  Docker / Compose                           │  ← containers, volumes, networking
└─────────────────────────────────────────────┘
```

## The AIRIS Stack

The gap between human-era tooling and AI-native development isn't just one tool wide. When you code with AI, every layer — workspace config, tool access, memory — needs structure the AI can rely on even after context compression. AIRIS is a suite that fills these gaps. Each component extends what already exists instead of replacing it.

```
┌─────────────────────────────────────────────────────┐
│                  Your Editor                        │
│            (Claude Code / Cursor / …)               │
├──────────┬──────────┬──────────┬────────────────────┤
│  airis   │  airis   │  airis   │    mindbase        │
│          │  agent   │  mcp     │                    │
│ Workspace│  LLM     │ gateway  │  Cross-session     │
│ Manager  │  Layer   │          │  Memory            │
└──────────┴──────────┴──────────┴────────────────────┘
```

| Component | What it does |
|-----------|-------------|
| **[airis](https://github.com/agiletec-inc/airis-monorepo)** | Workspace manager. `manifest.toml` → Dockerfile, compose.yml, CI workflows. Command guards keep AI inside Docker. |
| **[airis-agent](https://github.com/agiletec-inc/airis-agent)** | LLM intelligence layer for editors. |
| **[airis-mcp-gateway](https://github.com/agiletec-inc/airis-mcp-gateway)** | Unified MCP proxy — 60+ tools through 3 meta-endpoints. 90% token reduction so the AI keeps more context for your code. |
| **[mindbase](https://github.com/agiletec-inc/mindbase)** | Cross-session memory. What the AI learned yesterday is still there today. |

Every component follows the same principle: **extend your existing tools, don't replace them.** airis wraps Turborepo. airis-mcp-gateway proxies your MCP servers. mindbase plugs into any editor. Nothing asks you to throw away what already works.

---

## How It Works

### Single source of truth

```
manifest.toml  ──  airis gen  ──▶  package.json
                                   pnpm-workspace.yaml
                                   Dockerfile
                                   docker-compose.yml
                                   .github/workflows/
```

Edit `manifest.toml`. Run `airis gen`. Everything else is derived.

### Generate everything from one file

```toml
# manifest.toml
[workspace]
name = "my-project"
package_manager = "pnpm@10.22.0"
image = "node:22-alpine"

[packages.catalog]
react = "latest"       # resolved from npm registry
next = "lts"           # resolved to LTS version
typescript = "^5.0"    # used as-is

[guards]
deny = ["npm", "yarn", "pnpm", "bun"]
```

```bash
airis gen    # generates package.json, compose.yml, Dockerfile, CI workflows
airis up     # builds containers, installs deps inside Docker, starts services
```

### Command guards keep AI inside Docker

When your AI pair-programmer runs a package manager command, airis intercepts it and routes it through Docker:

```bash
$ pnpm install
# ⛔ 'pnpm' must run inside Docker container.
#    Use: airis up

$ airis up
# ✔ Containers built. Dependencies installed. Services running.
```

AI agents can also be auto-remapped transparently:

```toml
[remap]
"docker compose up" = "airis up"
"npm install" = "airis install"
```

### Version catalog

Centralized dependency versions across your entire monorepo. Resolved from the npm registry automatically.

```toml
[packages.catalog]
react = "latest"        # → ^19.1.0
next = "lts"            # → ^15.3.2
zod = "^3.22"           # → ^3.22 (as-is)
```

Every workspace member gets the same versions. No divergence between teammates.

---

## Quick Start

### New project

```bash
mkdir my-monorepo && cd my-monorepo
airis init --write     # creates manifest.toml
airis gen              # generates all config files
airis up               # start Docker services
```

### Existing project

![airis init demo](assets/airis-init-demo.gif)

```bash
cd your-monorepo
airis init             # auto-discovers apps, libs, compose files (dry-run)
airis init --write     # writes manifest.toml
airis gen              # generates workspace files
airis up               # start everything
```

`airis init` detects your project structure automatically:
- Frameworks: Next.js, Vite, Hono, Rust, Python
- Existing `docker-compose.yml` files
- Apps in `apps/`, libs in `libs/`
- Version catalogs from `package.json`

---

## Install

### From crates.io

```bash
cargo install airis
```

### Pre-built binaries

macOS (ARM/Intel), Linux (x64/ARM), and Windows:

```bash
curl -fsSL https://github.com/agiletec-inc/airis-monorepo/releases/latest/download/install.sh | bash
```

### From source

```bash
cargo install --git https://github.com/agiletec-inc/airis-monorepo
```

---

## Features

### Framework Auto-Detection

`airis init` scans your repo and detects Next.js, Vite, React, Hono, Node.js, Rust, and Python projects. Dependencies, scripts, and Docker configs are generated based on what it finds.

### File Ownership

airis tracks three levels of file ownership:

- **Tool-owned** — fully managed, regenerated on `airis gen` (package.json, compose.yml, Dockerfile)
- **Hybrid** — specific fields are managed, your edits are preserved (per-app package.json)
- **User-owned** — never touched (manifest.toml, tsconfig.json, README.md)

Automatic backups in `.airis/backups/` before any modification.

### Health Checks

![airis doctor demo](assets/airis-doctor-demo.gif)

```bash
airis doctor           # diagnose drift between manifest and generated files
airis doctor --fix     # auto-repair issues
```

### Affected-Only Builds

```bash
airis build --affected --docker    # build only changed projects
```

### Command Remapping

AI runs `docker compose up` — airis translates it to `airis up` (which includes `--build` for dependency installation). Transparent and automatic.

### Works With Your Stack

airis wraps whatever commands you define. Zero assumptions about your stack:

- **Build tools**: NX, Turborepo, Bazel, plain scripts
- **Deploy targets**: Vercel, Railway, Fly.io, bare metal
- **Runtimes**: Node.js, Bun, Deno, Rust, Python
- **Env management**: Doppler, `.env`, Docker Secrets

---

## Configuration

airis is configured through `manifest.toml`. See the **[full reference](docs/CONFIG.md)**.

### Minimal example

```toml
version = 1
mode = "docker-first"

[workspace]
name = "my-project"
package_manager = "pnpm@10.22.0"
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
up = "docker compose up -d --build --remove-orphans"
down = "docker compose down --remove-orphans"
ps = "docker compose ps"
```

---

## Project Structure

```
my-monorepo/
  manifest.toml           # single source of truth (edit this)
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

## Documentation

- [manifest.toml Reference](docs/CONFIG.md)
- [Manifest Specification](docs/manifest.md)
- [Commands Guide](docs/commands.md)
- [Init Architecture](docs/airis-init-architecture.md)
- [Deployment Guide](docs/DEPLOYMENT.md)
- [Changelog](CHANGELOG.md)

## Contributing

Contributions welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT — see [LICENSE](LICENSE).

---

[@agiletec-inc](https://github.com/agiletec-inc)
