# airis Commands Reference

Guide for using the `airis` CLI to safely work within Docker workspaces across your monorepo.

- **CLI Tool**: [airis-workspace](https://github.com/agiletec-inc/airis-workspace)
- **Config File**: `manifest.toml` (single source of truth)
- **Parser**: Rust TOML parser reads `manifest.toml` directly

---

## Core Principles

1. **Docker-First** -- `airis` always routes commands through `docker compose` / workspace containers. Never run `pnpm install` or `docker compose up` directly on the host.

2. **Single Entry Point** -- `airis up` -> `airis install` -> `airis shell` is the standard workflow. If you need `pnpm` directly, use it inside `airis shell`.

3. **Built-in Guards** -- Running `pnpm` / `npm` / `yarn` on the host triggers an error via `airis guards`. Always use `airis` commands.

4. **Manifest = manifest.toml** -- `packages.workspaces`, `apps.*`, `libs.*`, and `dev.autostart` define all app configuration, startup order, and infrastructure layout.

5. **Auto-generation** -- `pnpm-workspace.yaml`, `package.json`, GitHub workflows, etc. are all generated from `manifest.toml`. Manual editing is unnecessary (backups in `.airis/backups/`).

---

## Setup & Startup

```bash
airis init                    # Auto-discover existing projects + create manifest.toml
airis up                      # Docker-First: Sync configs, install deps, and start services
airis down                    # Stop all services
airis shell                   # Enter workspace container shell (/app)
```

### The `airis up` Workflow (One Command)

1. **Sync**: Compares `manifest.toml` with generated files and updates them.
2. **Install**: Runs `pnpm install` (or equivalent) inside the Docker container.
3. **Startup**: Starts all services (containers). Development servers start within the containers.

---

## Development

Use `airis up` for your daily development workflow. For specific tasks:

```bash
airis run <task>              # Run any command defined in manifest.toml (build, test, etc.)
airis build                   # Build all apps (alias for 'run build')
airis test                    # Run tests (alias for 'run test')
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
airis validate                # Validate configuration
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

Run with `airis run <command>` or use built-in aliases (`airis up`, `airis build`, etc.).

---

## Guards

```toml
[guards]
deny = ["npm", "yarn", "pnpm", "bun"]
deny_with_message = { "docker" = "Use 'airis' commands instead" }
```

Install guards globally:

```bash
airis guards install          # Install shell guards (~/.airis/bin)
```

This creates shims that intercept `npm`, `pnpm`, `yarn`, etc. and block them outside airis projects.

---

## Version Catalog

```toml
[packages.catalog]
next = "latest"
react = "latest"
typescript = "latest"

[packages.catalog.react-dom]
follow = "react"
```

Centralize dependency versions across all apps. Versions are resolved from the npm registry and written to `pnpm-workspace.yaml`.

---

## Usage Example

```bash
# 1. Initial setup
airis init --write

# 2. Start Docker stack
airis up

# 3. Install dependencies
airis install

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
airis install
```

### Changes to manifest.toml not reflected

```bash
airis gen          # Regenerate workspace files
```

---

## Best Practices

**Do:**
- Run `airis` from the repository root
- Use `airis shell` to access `pnpm` directly
- Add new apps/config to `manifest.toml`, then run `airis gen`
- Run `airis clean` periodically to remove build artifacts

**Don't:**
- Run `pnpm install` on the host (guards will block it)
- Run `docker compose up` directly
- Commit `.env` or `node_modules`
- Manually edit `package.json` or `pnpm-workspace.yaml` (they are auto-generated)

---

## References

- [manifest.toml Reference](CONFIG.md)
- [Init Architecture](airis-init-architecture.md)
- [pnpm Catalog](https://pnpm.io/catalogs)
- [Docker Compose Spec](https://docs.docker.com/compose/compose-file/)
