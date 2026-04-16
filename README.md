# airis-workspace

**AI doesn't break your build system. It breaks your environment.**

[![Crates.io](https://img.shields.io/crates/v/airis.svg)](https://crates.io/crates/airis)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/agiletec-inc/airis-workspace/actions/workflows/ci.yml/badge.svg)](https://github.com/agiletec-inc/airis-workspace/actions/workflows/ci.yml)

![airis demo](assets/airis-demo.gif)

AI pair-programmers are powerful, but they have terrible environment hygiene. They run `npm install` on the host, break bind mounts, leak credentials, and destroy reproducibility. 

**airis** is the deterministic environment compiler for Docker-first monorepos. It doesn't replace your build tools—it makes them safe to use in AI-assisted workflows.

---

## Why airis exists

Development tools were designed for humans. Humans read docs, memorize project conventions, and don't forget which package manager to use mid-session. LLMs do.

Nx and Turborepo optimize builds. **airis makes the environment deterministic.**

We wanted Docker-first development for one simple reason: **reproducibility** — every dependency inside a container, same behavior on every machine. But when your AI pair-programmer writes the code, Docker-first breaks in ways it never did with human developers:

1. **The AI forgets the rules.** It runs commands on the host. Dependencies leak. Reproducibility is gone.
2. **It destroys the filesystem.** Bind mounts and `node_modules` drift apart.
3. **Docker-first is hard to keep correct.** One manual change to `compose.yml` and the "truth" is gone.

airis enforces Docker-first hygiene structurally. Adopting `manifest.toml` as your Source of Truth guarantees that AI agents operate in a locked-down, reproducible environment.

The result is airis — not a replacement for Turborepo or NX (we use Turborepo ourselves and `turbo prune` internally), but **the layer that keeps Docker-first actually working** when your AI pair-programmer has a short memory.

```
┌─────────────────────────────────────────────┐
│  Your Build Tool (Turborepo / NX / Bazel)   │  ← Task orchestration, caching
├─────────────────────────────────────────────┤
│  AIRIS Workspace                            │  ← Environment SoT, Config Compiler,
│                                             │    Command Guards, Hygiene Enforcer
├─────────────────────────────────────────────┤
│  Docker / Compose                           │  ← Containers, Volumes, Networking
└─────────────────────────────────────────────┘
```

**Works with Nx and Turborepo. Does not replace them.**

---

## The AIRIS Suite

airis-workspace is part of the **AIRIS Suite**: Infrastructure for AI-assisted coding.

- **AIRIS Workspace (this repo):** The deterministic environment compiler.
- **[AIRIS Gateway](https://github.com/agiletec-inc/airis-mcp-gateway):** The MCP connectivity hub. Standardizes tool access for any LLM.
- **[MindBase](https://github.com/agiletec-inc/mindbase):** The durable semantic memory substrate.
- **[AIRIS Keeper](https://github.com/agiletec-inc/airis-keeper):** The scoped credential control plane. Contain the blast radius.

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

Edit `manifest.toml`. Run `airis gen`. Everything else is derived. No "hybrid" states, no guessing. It's a compiler.

### Generate everything from one file

```toml
# manifest.toml
mode = "docker-first"

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
curl -fsSL https://github.com/agiletec-inc/airis-workspace/releases/latest/download/install.sh | bash
```

From source:

```bash
cargo install --git https://github.com/agiletec-inc/airis-workspace
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
| `tsconfig.json` / `tsconfig.base.json` | TypeScript project references |
| Per-app `package.json` | App-level dependencies from catalog |
| `hooks/pre-commit`, `hooks/pre-push` | Native git hooks |
| `airis.lock` | Resolved catalog versions |

`compose.yml`, `Dockerfile`, `.env.example`, and `.github/workflows/` are project-owned — airis never writes to them. Hand-edit freely to use compose features (custom healthchecks, `env_file`, `entrypoint`, `depends_on` conditions) or language runtimes (Python, Rust, Go) that would not fit a uniform schema.

### Framework Auto-Detection

`airis init` scans your repo and detects Next.js, Vite, React, Hono, Node.js, Rust, and Python projects. Dependencies, scripts, and Docker configs are generated based on what it finds.

### File Ownership

airis tracks three levels of file ownership:

- **Tool-owned** — fully managed, regenerated on `airis gen` (package.json, pnpm-workspace.yaml, tsconfig)
- **Hybrid** — specific fields are managed, your edits are preserved (per-app package.json)
- **User-owned** — never touched (manifest.toml, compose.yml, Dockerfile, .env.example, README.md, source code)

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
  manifest.toml           # single source of truth for workspace tooling (edit this)
  package.json            # auto-generated (DO NOT EDIT)
  pnpm-workspace.yaml     # auto-generated (DO NOT EDIT)
  compose.yml             # user-owned (hand-edit for services, healthchecks, etc.)
  Dockerfile              # user-owned (hand-edit for build steps)
  .env.example            # user-owned (document env vars here)
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
| **[airis](https://github.com/agiletec-inc/airis-workspace)** | Workspace manager. `manifest.toml` → package.json, tsconfig, git hooks. Command guards keep AI inside Docker. |
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
