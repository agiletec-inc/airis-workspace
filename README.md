# airis-monorepo

**The Docker-first monorepo manager for the vibe coding era.**

[![Crates.io](https://img.shields.io/crates/v/airis.svg)](https://crates.io/crates/airis)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/agiletec-inc/airis-monorepo/actions/workflows/ci.yml/badge.svg)](https://github.com/agiletec-inc/airis-monorepo/actions/workflows/ci.yml)

![airis demo](assets/airis-demo.gif)

One manifest file. Compose, packages, and workspace config — all generated. Your AI pair-programmer stays inside the container where it belongs.

---

## Why airis exists

Development tools were designed for humans. Humans read docs, memorize project conventions, and don't forget which package manager to use mid-session. LLMs do.

We wanted Docker-first development for one simple reason: **reproducibility** — every dependency inside a container, same behavior on every machine. But when your AI pair-programmer writes the code, Docker-first breaks in ways it never did with human developers:

1. **The AI forgets the rules.** After context compression or a long session, your coding agent forgets to prefix commands with `docker compose exec`. It runs `pnpm install` on the host. Dependencies leak out of the container. Reproducibility is gone.

2. **It picks the wrong tool.** Your project uses pnpm, but the AI reaches for npm or yarn. Now you have a `package-lock.json` sitting next to your `pnpm-lock.yaml`.

3. **Docker boilerplate is fragile.** Manually wiring compose volumes, keeping `compose.yml` in sync with your workspace structure, making sure `node_modules` and pnpm store never leak onto the host — one mistake and your "Docker-first" setup is Docker-in-name-only.

We fixed these one at a time. Command guards that block `npm`/`yarn` and redirect `pnpm` through Docker. A manifest that generates compose configs, workspace files, and dependency catalogs so the boilerplate can't drift. Named volumes that structurally prevent dependency leakage.

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

---

## How It Works

### Single source of truth

```
manifest.toml  ──  airis gen  ──▶  package.json
                                   pnpm-workspace.yaml
                                   compose.yml
                                   tsconfig.json
                                   per-app package.json
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
airis up     # The One Command: Syncs config, installs deps inside Docker, and starts services
```

### Command guards keep AI inside Docker

When your AI pair-programmer runs a package manager command, airis intercepts it and routes it through Docker:

```bash
$ pnpm install
# ⛔ 'pnpm' must run inside Docker container.
#    Use: airis up

$ airis up
# ✔ Config synced. Dependencies installed. Services running.
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

### Secrets management

Inject secrets from external providers on `airis up`. No more sharing `.env` files over DM.

```toml
[secrets]
provider = "doppler"

[secrets.doppler]
project = "my-project"
config = "dev"
```

airis wraps `docker compose` with the provider CLI, so environment variables are injected automatically. Swap providers by changing `provider` — the rest of your config stays the same.

---

## Quick Start

### Install

From crates.io:

```bash
cargo install airis
```

Pre-built binaries (macOS ARM/Intel, Linux x64/ARM, Windows):

```bash
curl -fsSL https://github.com/agiletec-inc/airis-monorepo/releases/latest/download/install.sh | bash
```

From source:

```bash
cargo install --git https://github.com/agiletec-inc/airis-monorepo
```

### New project

```bash
mkdir my-monorepo && cd my-monorepo
airis init --write     # Analyzes repo and creates manifest.toml
airis up               # Docker-First: Sync config, install deps, and start services
```

### Existing project

![airis init demo](assets/airis-init-demo.gif)

```bash
cd your-monorepo
airis init             # auto-discovers apps, libs, compose files (dry-run)
airis init --write     # writes manifest.toml
airis up               # Start everything (Syncs config + installs deps + up)
```

`airis init` detects your project structure automatically:
- Frameworks: Next.js, Vite, Hono, Rust, Python
- Existing `docker-compose.yml` files
- Apps in `apps/`, libs in `libs/`
- Version catalogs from `package.json`

---

## Features

### Config Generation

`airis gen` generates the following from `manifest.toml`:

| File | Purpose |
|------|---------|
| `package.json` | Root workspace package with catalog versions |
| `pnpm-workspace.yaml` | Workspace member discovery |
| `compose.yml` | Docker Compose for services and workspace |
| `tsconfig.json` / `tsconfig.base.json` | TypeScript project references |
| Per-app `package.json` | App-level dependencies from catalog |
| `.env.example` | Environment variable template |
| `.github/workflows/` | CI/CD pipelines |

### Framework Auto-Detection

`airis init` scans your repo and detects Next.js, Vite, React, Hono, Node.js, Rust, and Python projects. Dependencies, scripts, and Docker configs are generated based on what it finds.

### File Ownership

airis tracks three levels of file ownership:

- **Tool-owned** — fully managed, regenerated on `airis gen` (package.json, compose.yml, tsconfig)
- **Hybrid** — specific fields are managed, your edits are preserved (per-app package.json)
- **User-owned** — never touched (manifest.toml, README.md, source code)

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

### Project Scaffolding

```bash
airis new web my-app       # scaffold a new Next.js app
airis new api my-service   # scaffold a new API service
airis new lib my-lib       # scaffold a shared library
```

### Works With Your Stack

airis wraps whatever commands you define. Zero assumptions about your stack:

- **Build tools**: NX, Turborepo, Bazel, plain scripts
- **Deploy targets**: Vercel, Railway, Fly.io, bare metal
- **Runtimes**: Node.js, Bun, Deno, Rust, Python
- **Secrets**: Doppler, `.env`, Docker Secrets

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

### Secrets example

```toml
[secrets]
provider = "doppler"

[secrets.doppler]
project = "my-project"
config = "dev"
```

When `[secrets]` is configured, `airis up` wraps Docker Compose with the provider CLI to inject environment variables automatically.

---

## CLI Reference

| Category | Commands |
|----------|----------|
| **Lifecycle** | `init`, `up`, `down`, `upgrade` |
| **Docker** | `ps`, `logs`, `exec`, `restart`, `shell`, `network` |
| **Development** | `build`, `test`, `lint`, `format`, `typecheck`, `run` |
| **Analysis** | `doctor`, `diff`, `affected`, `deps`, `validate`, `verify`, `gen` |
| **Scaffolding** | `new` |
| **Deployment** | `bundle`, `bump-version`, `policy` |
| **Guards** | `guards install`, `shim`, `hooks` |
| **Docs** | `docs sync` |

Run `airis --help` or `airis <command> --help` for details.

---

## Project Structure

```
my-monorepo/
  manifest.toml           # single source of truth (edit this)
  package.json            # auto-generated (DO NOT EDIT)
  pnpm-workspace.yaml     # auto-generated (DO NOT EDIT)
  compose.yml             # auto-generated (DO NOT EDIT)
  apps/
    dashboard/
    api/
  libs/
    ui/
    db/
```

---

## Ecosystem

airis is part of a broader toolkit for AI-assisted development. Each component extends your existing tools instead of replacing them.

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
| **[airis](https://github.com/agiletec-inc/airis-monorepo)** | Workspace manager. `manifest.toml` → compose.yml, package.json, CI workflows. Command guards keep AI inside Docker. |
| **[airis-agent](https://github.com/agiletec-inc/airis-agent)** | LLM intelligence layer for editors. |
| **[airis-mcp-gateway](https://github.com/agiletec-inc/airis-mcp-gateway)** | Unified MCP proxy — 60+ tools through 3 meta-endpoints. 90% token reduction so the AI keeps more context for your code. |
| **[mindbase](https://github.com/agiletec-inc/mindbase)** | Cross-session memory. What the AI learned yesterday is still there today. |

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
