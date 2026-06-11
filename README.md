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
- **Docker module** — *only* for containerized repos: `up` / `down` / `exec` / `shell` / `run`,
  plus `compose.yaml` + named-volume generation inside `gen`. The `[docker]` manifest section is
  optional (`#[serde(default)]`), so non-containerized repos use airis without it.

### Pick the runtime that matches where the code ships

airis does **not** force everything into Docker. Match the development runtime to the deploy runtime:

| Workload | Where you develop |
|---|---|
| Cloudflare Workers / edge (incl. Next.js on OpenNext) | **host-native** (`pnpm dev`, `wrangler dev`, `pnpm cf-preview`) |
| Native desktop (macOS / Swift / Tauri / Rust CLI) | **host** (`cargo`, `xcodebuild`, …) |
| Long-running container workloads (k3s daemons, Playwright) | **Docker** (`airis up` / `airis exec`) |
| Python / GPU (CUDA, ML, video) | **Docker** |
| Tests (all workloads) | **Docker** (`airis test`, for CI parity) |

For containerized repos, run your usual tools through the workspace so they never touch the host:

```bash
airis exec pnpm install      # → docker compose exec workspace pnpm install
airis exec pnpm dev          # → docker compose exec workspace pnpm dev
airis exec python main.py    # → docker compose exec workspace python main.py
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

Start by routing commands through `airis exec` / `airis run`. Grow into manifest-driven
orchestration later.

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

Go to any project with a `compose.yml` and route your usual commands through airis:

```bash
airis exec pnpm install   # runs inside the workspace container
airis exec pnpm dev       # runs inside the workspace container
airis run <task>          # runs a task defined in manifest.toml [commands]
```

If the workspace container is not running, `airis exec` automatically starts it via `airis up`.

> Earlier versions shipped a **guard-shim** subsystem that transparently intercepted bare
> `pnpm` / `python` / `cargo` calls on the host. That subsystem has been removed: routing is now
> explicit (`airis exec` / `airis run`), which is simpler and avoids surprising host commands.

---

## How It Works: Command Routing

### `airis exec` / `airis run` → Service Resolution

When you run `airis exec pnpm install` (or `airis run <task>` that delegates to Docker):

1. **Service resolution** classifies the command by runtime family:
   - `node`, `npm`, `pnpm`, `yarn`, `bun`, `tsc`, `tsx`, `next`, `vite` → Node family → `workspace` service
   - `python`, `python3`, `pip`, `uv`, `poetry`, `ruff`, `mypy`, `pytest` → Python family → `workspace` service
   - `cargo`, `rustc`, `rustup`, `clippy-driver`, `rustfmt` → Rust family → `workspace` service
2. **Executes** `docker compose exec <service> <command>`
3. **Auto-up**: if the resolved service is not running, `airis exec` runs `airis up` first

### Command remapping (`[remap]`)

A repo's `manifest.toml` may declare a `[remap]` table that rewrites convenience commands when
they go through `airis run` — for example aliasing `docker compose up` → `airis up`. Remap is
opt-in per repo and deliberately does **not** ship default `pnpm dev` / `pnpm install` → `airis up`
entries, since install/dev runtime is workload-dependent.

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
- **Command policies**: Define `airis run <task>` shortcuts and `[remap]` aliases
- **AI agent rules**: Shared guidance synced into `CLAUDE.md` / `AGENTS.md` / `GEMINI.md`
- **Generated config**: Synced `package.json`, `tsconfig.json`, Justfile

Start simple. Add manifest later.

```bash
airis gen              # Generate compose.yaml and other configs from manifest.toml
airis up               # Start the Docker workspace (containerized repos)
airis shell            # Enter the workspace container
airis run <task>       # Run custom tasks (defined in manifest.toml [commands])
```

---

## Claude Code Integration (MCP)

AIRIS integrates with Claude Code through the **MCP (Model Context Protocol)**.

### Setup

```bash
airis claude setup
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
airis claude status      # Check Claude integration status
airis claude uninstall  # Remove Claude configuration
```

---

## Commands Reference

### Core Execution

```bash
airis exec <cmd>          # Run a command with automatic service routing (e.g., airis exec pnpm install)
airis run <task>          # Run a task defined in manifest.toml [commands]
airis up                  # Start Docker workspace (alias for airis run up)
airis down                # Stop all services (alias for airis run down)
airis shell               # Enter the workspace container interactive shell
```

### Claude & AI Integration

```bash
airis claude setup       # Sync Claude Code configuration to ~/.claude/
airis claude status      # Check Claude Code integration status
airis claude uninstall   # Remove Claude Code configuration
airis mcp                # Start the MCP server (used by airis-mcp-gateway)
```

### Configuration & Diagnostics

```bash
airis gen                # Generate compose.yaml and derived files from manifest.toml
airis manifest json      # Print manifest.toml as JSON
airis validate <type>    # Validate manifest, ports, networks, env, dependencies, architecture, or all
airis verify             # Run system health checks
airis doctor             # Diagnose workspace issues
airis doctor --fix       # Auto-repair issues
airis doctor --truth     # Print the resolved startup truth (where each setting came from)
airis status             # Show current workspace status
airis ps                 # Show Docker container status
airis logs [service]     # Tail Docker logs
```

### Build, Test & Release

```bash
airis run build          # Run the "build" task (defined in manifest or delegated to Docker)
airis test [--level unit|integration|e2e|smoke]
airis lint               # Run linting
airis format             # Run code formatting
airis typecheck          # Run type checking
airis deps               # Visualize the dependency graph
airis diff               # Preview changes before applying gen
airis bump-version       # Bump the package version
airis upgrade            # Upgrade the airis binary
```

### Workspace Lifecycle

```bash
airis new <kind> <name>    # Create a new app, service, or library
airis workspace uninstall  # Remove AIRIS-generated files from a repo
airis docs sync            # Regenerate CLAUDE.md / AGENTS.md / GEMINI.md from docs/ai/*
airis docs list            # List managed adapter files
airis init-shell           # Print the shell-integration snippet (prompt)
airis completion <shell>   # Generate shell completion scripts
```

### Direct Container Access

```bash
airis shell                  # Open an interactive shell in the workspace container
airis exec <cmd>             # Run any command in the resolved service (auto-up if down)
airis run <task>             # Run a task defined in manifest.toml [commands]
airis restart [service]      # Restart Docker services
airis network <subcommand>   # Manage Docker networks
```

---

## Auto-Up Behavior

When using `airis exec`, if the resolved service is not running, AIRIS automatically brings it up.

### Suppression

Auto-up is suppressed in these cases:

- **Recent `airis down`**: Within 30 seconds of running `airis down`, auto-up is skipped (prevents racing `airis exec` from relaunching a stack you just tore down)
- **Explicit suppression**: `AIRIS_NO_AUTO_UP=1 airis exec pnpm install` disables auto-up for that invocation

This design ensures that stopping a workspace does not automatically restart it on the next command.

---

## Documentation

- **[manifest.toml Reference](docs/manifest.md)** — Schema, examples, and configuration guide
- **[Commands Guide](docs/commands.md)** — Extended command reference and usage patterns
- **[Project Rules](docs/ai/PROJECT_RULES.md)** — Architectural principles and design boundaries
- **[Workflow Guide](docs/ai/WORKFLOW.md)** — Step-by-step guides for common tasks
- **[Deployment & Release](docs/DEPLOYMENT.md)** — How to release and distribute AIRIS
- **[Architecture & Design](docs/ai/architecture-invariants.md)** — Deep dive into AIRIS design decisions

---

License: MIT
