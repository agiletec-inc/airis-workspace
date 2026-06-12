# AIRIS Workspace

**AI agents are great at trying things.  
They are also great at making a mess.**

You ask Claude Code, Codex, Gemini CLI, Cursor, or Zed to fix something.  
A few minutes later, your Mac has:

- `node_modules` in the repo
- `package-lock.json` in a pnpm project
- `yarn.lock` you never asked for
- Python packages installed on the host
- build artifacts leaking through bind mounts
- local state that CI will never have
- three repos that each invented their own `tsconfig.json`, docs layout, and AI rules

AIRIS Workspace exists to stop that.

It is a **Rust-powered, polyglot convention-unification engine** for the AI coding era. From a thin `manifest.toml` it keeps a heterogeneous set of repositories consistent — AI rules, shared docs, `tsconfig.json`, version scheme, and scaffolding. For the subset of repos that are containerized, an **optional Docker module** generates the development environment (`compose.yaml`, volume hygiene) so host-side `node_modules` / `.venv` / build artifacts stay out of your working tree.

---

## The Core Idea

**airis is a convention-unification engine. Docker is one module within it, not the whole tool.**

- **Convention core** — applies to *every* repo, polyglot (Node / Python / Rust / edge / native):
  `gen`, `docs`, `new`, `validate`, `doctor`, `bump-version`, `verify`. These keep AI adapters,
  shared docs, `tsconfig.json`, the version scheme, and scaffolding consistent across repos.
- **Docker module** — *only* for containerized repos: `compose.yaml` + named-volume generation
  inside `gen`. The `[docker]` manifest section is optional (`#[serde(default)]`), so
  non-containerized repos use airis without it. You drive the containers with
  `docker compose` directly.

> **v4.0.0**: the binary is named `airis-workspace` and is normally invoked through the
> `airis` dispatcher as `airis workspace <cmd>`. The Docker wrapper subcommands
> (`up` / `exec` / `run` / `shell` / ...) were removed — run `docker compose` and your
> native toolchain directly. See [CHANGELOG.md](CHANGELOG.md) for the migration guide.

### Pick the runtime that matches where the code ships

airis does **not** force everything into Docker. Match the development runtime to the deploy runtime:

| Workload | Where you develop |
|---|---|
| Cloudflare Workers / edge (incl. Next.js on OpenNext) | **host-native** (`pnpm dev`, `wrangler dev`, `pnpm cf-preview`) |
| Native desktop (macOS / Swift / Tauri / Rust CLI) | **host** (`cargo`, `xcodebuild`, …) |
| Long-running container workloads (k3s daemons, Playwright) | **Docker** (`docker compose up -d` / `docker compose exec`) |
| Python / GPU (CUDA, ML, video) | **Docker** |
| Tests (all workloads) | **Docker** (for CI parity) |

For containerized repos, run your usual tools inside the workspace container so they never touch the host:

```bash
docker compose exec workspace pnpm install
docker compose exec workspace pnpm dev
docker compose exec workspace python main.py
```

For Workers / edge / native repos, run host-native — airis stays out of the runtime and just
keeps conventions consistent.

## Why Rust?

AIRIS is implemented in Rust because this layer has to be boring, fast, and reliable.  
It sits between your shell, your AI agent, and your workspace.  
That means **it should not depend on the very runtimes it manages**.

The convention core works even when your host has no Node.js, no pnpm, no Python, and no Rust
toolchain — and the Docker module keeps the heavy dependencies inside containers for the repos
that use it.

---

## You Do Not Need to Adopt AIRIS All at Once

AIRIS works with plain Docker Compose projects.

If your repository already has a `compose.yml`, AIRIS can use it as the workspace boundary.  
**No manifest required. No migration required. No monorepo rewrite required.**

Start with `airis workspace clean` / `airis workspace doctor` for hygiene. Grow into
manifest-driven generation (`airis workspace gen`) later.

### Install AIRIS

Pick whichever fits your toolchain:

```bash
# macOS / Linux: Homebrew
brew install agiletec-inc/tap/airis-workspace

# Any platform: shell installer
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/agiletec-inc/airis-workspace/releases/latest/download/airis-workspace-installer.sh | sh

# Rust users: prebuilt binary via cargo-binstall
cargo binstall airis-workspace

# Rust users: build from source
cargo install airis-workspace
```

### Use Your Repo as Normal

Go to any project with a `compose.yml` and run your usual commands inside the workspace
container:

```bash
docker compose up -d
docker compose exec workspace pnpm install
docker compose exec workspace pnpm dev
```

> Earlier versions shipped a **guard-shim** subsystem (host command interception) and later
> Docker wrapper subcommands (`airis exec` / `airis run` / `airis up` / ...). Both have been
> removed: you run `docker compose` and your toolchain directly, which is simpler and avoids
> surprising indirection.

---

## What AIRIS Is (and Is Not)

AIRIS is **not** a replacement for Docker Compose, Nx, Turborepo, pnpm, uv, or cargo.  
It is the convention layer around them, plus an optional Docker workspace module.

- **Docker Compose** defines services
- **pnpm / uv / cargo** manage dependencies
- **Nx / Turborepo** orchestrate builds
- **AIRIS** keeps conventions consistent across repos and, for containerized repos, keeps
  execution inside the workspace boundary

AIRIS is also not a one-size-fits-all project management tool. It assumes you already manage
dependencies and builds; it just keeps the cross-repo conventions coherent and the container
runtime tidy.

---

## Advanced: Manifest-Driven Orchestration

When you need more structure, a `manifest.toml` becomes the source of truth for:

- **Apps and libs**: Convention-first discovery (`apps/*`, `libs/*`)
- **Runtimes**: Node.js, Python, Rust — per project
- **Docker workspace generation**: Automated `compose.yaml` from declarations (containerized repos)
- **Named volumes**: Keep `node_modules`, `target/`, `.venv` inside containers
- **AI agent rules**: Shared guidance synced into `CLAUDE.md` / `AGENTS.md` / `GEMINI.md`
- **Generated config**: Synced `package.json`, `tsconfig.json`, Justfile

Start simple. Add manifest later.

```bash
airis workspace gen        # Generate compose.yaml and other configs from manifest.toml
docker compose up -d       # Start the Docker workspace (containerized repos)
```

---

## Claude Code Integration (MCP)

AIRIS integrates with Claude Code through the **MCP (Model Context Protocol)**.

### Setup

```bash
airis workspace claude setup
```

This:
1. Checks if the `airis-mcp-gateway` plugin is installed
2. Syncs Claude Code configuration files to `~/.claude/`
3. Enables Claude to call AIRIS commands safely (initialize, apply, execute)

### How It Works

Inside Claude Code:
- AIRIS provides MCP tools for workspace initialization (`/airis:init`)
- Claude can safely inspect and modify `manifest.toml`
- For containerized repos, agent-executed commands stay inside Docker containers

### Status & Cleanup

```bash
airis workspace claude status     # Check Claude integration status
airis workspace claude uninstall  # Remove Claude configuration
```

---

## Commands Reference

All commands are invoked through the `airis` dispatcher as `airis workspace <cmd>`
(or directly as `airis-workspace <cmd>`).

### Claude & AI Integration

```bash
airis workspace claude setup      # Sync Claude Code configuration to ~/.claude/
airis workspace claude status     # Check Claude Code integration status
airis workspace claude uninstall  # Remove Claude Code configuration
airis workspace mcp               # Start the MCP server (used by airis-mcp-gateway)
```

### Configuration & Diagnostics

```bash
airis workspace gen               # Generate compose.yaml and derived files from manifest.toml
airis workspace manifest json     # Print manifest.toml as JSON
airis workspace validate <type>   # Validate manifest, ports, networks, env, dependencies, architecture, or all
airis workspace verify            # Run system health checks
airis workspace doctor            # Diagnose workspace issues
airis workspace doctor --fix      # Auto-repair issues
airis workspace doctor --truth    # Print the resolved startup truth (where each setting came from)
```

### Build & Release

```bash
airis workspace deps              # Visualize the dependency graph
airis workspace diff              # Preview changes before applying gen
airis workspace policy check      # Run policy gates
airis workspace bump-version      # Bump the package version
airis workspace upgrade           # Upgrade the airis-workspace binary
```

### Workspace Lifecycle

```bash
airis workspace new <kind> <name>    # Create a new app, service, or library
airis workspace clean                # Remove build artifacts (dry-run by default; --force to delete)
airis workspace workspace uninstall  # Remove AIRIS-generated files from a repo
airis workspace docs sync            # Regenerate CLAUDE.md / AGENTS.md / GEMINI.md from docs/ai/*
airis workspace docs list            # List managed adapter files
airis workspace completion <shell>   # Generate shell completion scripts
```

### Container Access

Drive containers with Docker Compose directly:

```bash
docker compose up -d                   # Start the workspace
docker compose exec workspace <cmd>    # Run a command inside the workspace container
docker compose logs [service]          # Tail logs
docker compose down                    # Stop services
```

---

## Documentation

- **[manifest.toml Reference](docs/manifest.md)** — Schema, examples, and configuration guide
- **[Commands Guide](docs/commands.md)** — Extended command reference and usage patterns
- **[Project Rules](docs/ai/PROJECT_RULES.md)** — Architectural principles and design boundaries
- **[Workflow Guide](docs/ai/WORKFLOW.md)** — Step-by-step guides for common tasks
- **[Deployment & Release](docs/DEPLOYMENT.md)** — How to release and distribute AIRIS
- **[Architecture & Design](docs/ai/architecture-invariants.md)** — Deep dive into AIRIS design decisions

## 💖 Support

[agiletec](https://github.com/agiletec-inc) is a one-person studio building these tools full-time and open source. If they earn a spot in your workflow, a sponsorship keeps them maintained and independent.

[![Sponsor agiletec](https://img.shields.io/badge/Sponsor-agiletec-ea4aaa?logo=githubsponsors&logoColor=white)](https://github.com/sponsors/agiletec-inc)

---

License: MIT
