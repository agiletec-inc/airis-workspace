# airis Commands Reference

Guide for using the `airis` CLI to keep conventions consistent across repos and, for
containerized repos, to run commands inside the Docker workspace.

- **CLI Tool**: [airis-workspace](https://github.com/agiletec-inc/airis-workspace)
- **Config File**: `manifest.toml` (thin source of truth)
- **Parser**: Rust TOML parser reads `manifest.toml` directly

---

## Core Principles

1. **Convention core, Docker module** -- airis is a convention-unification engine
   (AI adapters, shared docs, `tsconfig.json`, version scheme, scaffolding). Docker is
   one optional module for the repos that are containerized; the `[docker]` manifest
   section may be omitted entirely.

2. **Match dev runtime to deploy runtime** -- containerized workloads run through
   `airis up` / `airis exec`; Workers/edge and native desktop repos run host-native and
   use airis only for conventions.

3. **Explicit routing** -- for containerized repos, route commands through
   `airis exec <cmd>` / `airis run <task>` instead of running `pnpm` / `pip` / `cargo`
   on the host. (There is no transparent host-command interception; routing is explicit.)

4. **Manifest = manifest.toml** -- `packages.workspaces`, `apps.*`, `libs.*`, and
   `dev.autostart` define app configuration, startup order, and infrastructure layout.
   Convention-first discovery (`apps/*`, `libs/*`) covers the rest.

5. **Generation** -- `airis gen` produces `compose.yaml`, `tsconfig.json`, and the AI
   adapter files from `manifest.toml`. Dependencies and scripts in each project's
   `package.json` are yours to edit; airis preserves them.

---

## Setup & Startup

```bash
# manifest.toml bootstrap:
#   A) Claude Code: run /airis:init (invokes the workspace_init MCP tool)
#   B) Write manifest.toml by hand — see docs/manifest.md
airis gen                     # Generate downstream files from manifest.toml
airis up                      # Sync configs, install deps, and start services (containerized repos)
airis down                    # Stop all services
airis shell                   # Enter workspace container shell (/app)
```

### The `airis up` Workflow (One Command)

1. **Sync**: Compares `manifest.toml` with generated files and updates them.
2. **Install**: Runs `pnpm install` (or equivalent) inside the Docker container.
3. **Startup**: Starts all services (containers). Development servers start within the containers.

---

## Development

```bash
airis exec <cmd>              # Run any command inside the resolved service (auto-up if down)
airis run <task>              # Run a task defined in manifest.toml [commands]
airis test                    # Run tests (--level unit|integration|e2e|smoke)
airis lint                    # Run linting
airis format                  # Run code formatting
airis typecheck               # Run type checking
```

---

## Monitoring

```bash
airis ps                      # List containers
airis logs                    # Tail all service logs
airis logs <app>              # Tail specific app logs
```

---

## Utilities

```bash
airis clean                   # Remove build artifacts
airis validate <type>         # Validate manifest, ports, networks, env, dependencies, architecture, or all
airis doctor                  # Diagnose workspace issues
airis doctor --fix            # Auto-repair issues
airis verify                  # Run system health checks
airis diff                    # Preview manifest vs generated changes
airis deps tree               # Visualize dependency graph
```

---

## Custom Commands

Define commands in `manifest.toml`:

```toml
[commands]
up = "docker compose up -d"
dev = "docker compose exec workspace pnpm dev"
build = "docker compose exec workspace pnpm build"
migrate = "docker compose exec workspace pnpm prisma migrate deploy"
```

Run with `airis run <command>` (or the built-in aliases `airis up` / `airis down` / `airis shell`).

---

## Version Catalog

Shared dependency versions use the [pnpm catalog](https://pnpm.io/catalogs) in
`pnpm-workspace.yaml` (user-owned):

```yaml
catalog:
  next: 15.3.0
  react: 19.1.1
  typescript: 5.8.2
```

Individual `package.json` files reference versions as `"catalog:"`. airis reads the
resolved catalog during generation (e.g. for `tsconfig.json`).

---

## Usage Example

```bash
# 1. Initial setup: run /airis:init in Claude Code, or hand-write manifest.toml
airis gen

# 2. Start Docker stack
airis up

# 3. Install dependencies (inside the container)
airis exec pnpm install

# 4. Work in the container
airis shell
pnpm lint
pnpm test

# 5. Shut down
airis down
```

---

## Troubleshooting

### Containers won't start

```bash
airis doctor --fix            # Auto-repair
airis network setup           # Rebuild networks
```

### Dependency issues

```bash
airis clean
airis exec pnpm install
```

### Changes to manifest.toml not reflected

```bash
airis gen          # Regenerate workspace files
```

---

## Best Practices

**Do:**
- Run `airis` from the repository root
- Use `airis exec` / `airis shell` instead of host-side `pnpm` in containerized repos
- Add new apps/config to `manifest.toml`, then run `airis gen`
- Run `airis clean` periodically to remove build artifacts

**Don't:**
- Run `pnpm install` on the host of a containerized repo (use `airis exec pnpm install`)
- Run `docker compose up` directly (use `airis up`)
- Commit `.env` or `node_modules`
- Hand-edit generated files (`compose.yaml`, `tsconfig.json`) — change `manifest.toml` and run `airis gen`

---

## References

- [manifest.toml Reference](manifest.md)
- [Init Architecture](airis-init-architecture.md)
- [pnpm Catalog](https://pnpm.io/catalogs)
- [Docker Compose Spec](https://docs.docker.com/compose/compose-file/)
