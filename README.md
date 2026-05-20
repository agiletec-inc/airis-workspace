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

AIRIS Workspace exists to stop that.

It is a **Rust-powered Docker-first workspace guard** for the AI coding era.  
Humans and AI agents can keep typing normal commands:

```bash
pnpm install
pnpm dev
uv sync
python scripts/foo.py
cargo build
```

AIRIS routes them into the right Docker workspace.  
No host pollution. No accidental package manager drift. No hidden local state.

Just a clean workspace boundary.

---

## The Core Idea

**The host should be a control plane. The workspace belongs in Docker.**

AIRIS detects your Docker Compose setup and intercepts development commands, routing them into containers instead of letting them pollute your host.

With guard shims enabled, you do not change how you code:
- type `pnpm install` → AIRIS intercepts and runs it in the workspace container
- type `python -V` → AIRIS intercepts and runs it in the workspace container
- type `uv sync` → AIRIS intercepts and runs it in the workspace container

Your host stays clean. Your AI agent moves fast. Your CI passes.

## Why Rust?

AIRIS is implemented in Rust because this layer has to be boring, fast, and reliable.  
It sits between your shell, your AI agent, and your Docker workspace.  
That means **it should not depend on the very runtimes it protects you from**.

AIRIS works even when your host has:
- no Node.js installed
- no pnpm, npm, or yarn
- no Python
- no Rust toolchain

The host is only a thin control plane. The real development environment lives in Docker.

---

## You Do Not Need to Adopt AIRIS All at Once

AIRIS works with plain Docker Compose projects.

If your repository already has a `compose.yml`, AIRIS can use it as the workspace boundary.  
**No manifest required. No migration required. No monorepo rewrite required.**

Start with guards. Grow into manifest-driven orchestration later.

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

### Enable Guard Shims (Recommended — Project-Local)

```bash
# Run inside a project that has a compose.yml
airis guards install
```

This generates project-local shims under `.airis/bin/` and adds an `init-shell` snippet so guarded commands (`pnpm`, `npm`, `python`, `uv`, `cargo`, …) route into the workspace container when you are inside the project.

> **Note:** `airis guards install --global` (shims under `~/.airis/bin`) still exists, but **global shims are no longer recommended** — they leak across unrelated projects. Prefer project-local guards.

Guard shims work by:
1. Detecting if you are in an AIRIS workspace (`manifest.toml` or `.airis/` directory)
2. Automatically delegating to `airis exec <cmd>` to route through Docker
3. Falling back to the host command if no workspace is detected (preserving coexistence with host development)

### Use Your Repo as Normal

Go to any project with a `compose.yml` and run your usual commands:

```bash
pnpm install      # intercepted → docker compose exec workspace pnpm install
pnpm dev          # intercepted → docker compose exec workspace pnpm dev
python main.py    # intercepted → docker compose exec workspace python main.py
```

If the workspace container is not running, AIRIS automatically starts it via `airis up`.

---

## How It Works: Command Routing

### Guard Shims → `airis exec` → Service Resolution

When a guard shim intercepts a command (e.g., `pnpm install`):

1. **Guard shim** (`~/.airis/bin/pnpm`) detects the AIRIS context
2. **Delegates to** `airis exec pnpm install`
3. **Service resolution** classifies the command by runtime family:
   - `node`, `npm`, `pnpm`, `yarn`, `bun`, `npm`, `tsc`, `tsx`, `next`, `vite` → Node family → `workspace` service
   - `python`, `python3`, `pip`, `uv`, `poetry`, `ruff`, `mypy`, `pytest` → Python family → `workspace` service
   - `cargo`, `rustc`, `rustup`, `clippy-driver`, `rustfmt` → Rust family → `workspace` service
4. **Executes** `docker compose exec workspace <command>`
5. **Auto-up** (optional): If the service is not running, `airis exec` automatically runs `airis up` first

### Bypass Mechanisms

Guard shims respect several bypass methods:

- **Environment variables**: `AIRIS_SKIP_GUARD=1`, `AIRIS_HOST=1`, or `AIRIS_BYPASS=1` run the host command
- **Arguments**: `pnpm bypass install` or `pnpm host install` also run on the host
- **Docker/CI detection**: Inside containers or CI environments, commands run on the host automatically
- **No workspace**: Outside an AIRIS project, commands run on the host normally

---

## What AIRIS Is (and Is Not)

AIRIS is **not** a replacement for Docker Compose, Nx, Turborepo, pnpm, uv, or cargo.  
It is the missing local guard layer around them.

- **Docker Compose** defines services
- **pnpm / uv / cargo** manage dependencies
- **Nx / Turborepo** orchestrate builds
- **AIRIS** keeps execution inside the workspace boundary

AIRIS is also not a one-size-fits-all project management tool.  
It assumes you already have Docker. It assumes you already manage dependencies.  
It just ensures those tools run in the right place.

---

## Advanced: Manifest-Driven Orchestration

When you need more structure, a `manifest.toml` becomes the source of truth for:

- **Apps and libs**: Convention-first discovery (`apps/*`, `libs/*`)
- **Runtimes**: Node.js, Python, Rust — per project
- **Docker workspace generation**: Automated `compose.yaml` from declarations
- **Named volumes**: Keep `node_modules`, `target/`, `.venv` inside containers
- **Command policies**: Define `airis run <task>` shortcuts
- **Guard policies**: Forbid or remap commands per workspace
- **AI agent rules**: Claude-specific guidance and safety policies
- **Generated config**: Synced `package.json`, `tsconfig.json`, Justfile

Start simple. Add manifest later.

```bash
airis gen              # Generate compose.yaml and other configs from manifest.toml
airis up               # Start the Docker workspace
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
- Agent-executed commands stay inside Docker containers
- Guard policies are enforced automatically

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

### Guards

```bash
airis guards install             # Install project-local shims (.airis/bin) — recommended
airis guards install --global    # Install global shims (~/.airis/bin) — not recommended
airis guards status              # Show installed guards and their status
airis guards uninstall           # Remove project-local guards
airis guards verify              # Verify guard functionality
airis guards check-docker        # Check if running inside Docker
airis guards check-allow         # Show allow/deny policy for the current workspace
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
airis status             # Show current workspace and guard status
airis ps                 # Show Docker container status
airis logs [service]     # Tail Docker logs
```

### Build, Test & Release

```bash
airis build [project]    # Build all or specific projects
airis build --affected   # Build only what changed (uses src/dag.rs)
airis test [--level unit|integration|e2e|smoke]
airis lint               # Run linting
airis format             # Run code formatting
airis typecheck          # Run type checking
airis affected           # List packages affected by current changes
airis deps               # Visualize the dependency graph
airis diff               # Preview changes before applying gen
airis bundle             # Generate a deployment bundle (image.tar + artifact.tar.gz + bundle.json)
airis bump-version       # Bump the package version (pre-commit auto-runs `--auto`)
airis upgrade            # Upgrade the airis binary
```

### Workspace Lifecycle

```bash
airis new <kind> <name>    # Create a new app, service, or library
airis workspace uninstall  # Remove AIRIS hooks and generated files from a repo
airis hooks install        # Install native git hooks (.git/hooks/{pre-commit,pre-push,post-commit})
airis hooks uninstall      # Remove the airis-workspace blocks from .git/hooks/
airis docs sync            # Regenerate CLAUDE.md / AGENTS.md / GEMINI.md from docs/ai/*
airis docs list            # List managed adapter files
airis init-shell           # Print the shell-integration snippet (prompt + project-local guards)
airis completion <shell>   # Generate shell completion scripts
```

### Direct Container Access

```bash
airis shell                  # Open an interactive shell in the workspace container
airis exec <cmd>             # Run any command in the resolved service (auto-up if down)
airis host <cmd>             # Bypass guards; run on the host (sets AIRIS_BYPASS=1)
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
