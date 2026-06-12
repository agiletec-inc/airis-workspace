# airis-workspace Commands Reference

Guide for using the `airis-workspace` CLI (invoked through the `airis` dispatcher as
`airis workspace <cmd>`) to keep conventions consistent across repos.

- **CLI Tool**: [airis-workspace](https://github.com/agiletec-inc/airis-workspace)
- **Config File**: `manifest.toml` (thin source of truth)
- **Parser**: Rust TOML parser reads `manifest.toml` directly

---

## Core Principles

1. **Convention core, Docker module** -- airis-workspace is a convention-unification
   engine (AI adapters, shared docs, `tsconfig.json`, version scheme, scaffolding).
   Docker is one optional module for the repos that are containerized; the `[docker]`
   manifest section may be omitted entirely.

2. **Match dev runtime to deploy runtime** -- containerized workloads run through
   `docker compose`; Workers/edge and native desktop repos run host-native and
   use airis-workspace only for conventions.

3. **Direct execution** -- run `docker compose` and your native toolchain
   (`pnpm` / `pip` / `cargo`) directly. The Docker wrapper subcommands
   (`up` / `exec` / `run` / `shell` / ...) were removed in v4.0.0.

4. **Manifest = manifest.toml** -- `packages.workspaces`, `apps.*`, `libs.*`, and
   `dev.autostart` define app configuration, startup order, and infrastructure layout.
   Convention-first discovery (`apps/*`, `libs/*`) covers the rest.

5. **Generation** -- `airis workspace gen` produces `compose.yaml`, `tsconfig.json`,
   and the AI adapter files from `manifest.toml`. Dependencies and scripts in each
   project's `package.json` are yours to edit; airis-workspace preserves them.

---

## Setup & Startup

```bash
# manifest.toml bootstrap:
#   A) Claude Code: run /airis:init (invokes the workspace_init MCP tool)
#   B) Write manifest.toml by hand — see docs/manifest.md
airis workspace gen           # Generate downstream files from manifest.toml
docker compose up -d          # Start services (containerized repos)
docker compose down           # Stop all services
docker compose exec workspace sh   # Enter workspace container shell (/app)
```

---

## Development

Run commands inside the workspace container with Docker Compose:

```bash
docker compose exec workspace pnpm install
docker compose exec workspace pnpm test
docker compose exec workspace pnpm lint
```

---

## Monitoring

```bash
docker compose ps             # List containers
docker compose logs           # Tail all service logs
docker compose logs <app>     # Tail specific app logs
```

---

## Utilities

```bash
airis workspace clean             # Remove build artifacts (dry-run by default; --force to delete)
airis workspace validate <type>   # Validate manifest, ports, networks, env, dependencies, architecture, or all
airis workspace doctor            # Diagnose workspace issues
airis workspace doctor --fix      # Auto-repair issues
airis workspace verify            # Run system health checks
airis workspace diff              # Preview manifest vs generated changes
airis workspace deps tree         # Visualize dependency graph
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

The `[commands]` table documents the repo's canonical tasks (e.g. for AI agents via
`airis workspace doctor --truth`); run them with your shell or task runner.

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

Individual `package.json` files reference versions as `"catalog:"`. airis-workspace
reads the resolved catalog during generation (e.g. for `tsconfig.json`).

---

## Usage Example

```bash
# 1. Initial setup: run /airis:init in Claude Code, or hand-write manifest.toml
airis workspace gen

# 2. Start Docker stack
docker compose up -d

# 3. Install dependencies (inside the container)
docker compose exec workspace pnpm install

# 4. Work in the container
docker compose exec workspace sh
pnpm lint
pnpm test

# 5. Shut down
docker compose down
```

---

## Troubleshooting

### Containers won't start

```bash
airis workspace doctor --fix  # Auto-repair
```

### Dependency issues

```bash
airis workspace clean --force
docker compose exec workspace pnpm install
```

### Changes to manifest.toml not reflected

```bash
airis workspace gen           # Regenerate workspace files
```

---

## Best Practices

**Do:**
- Run `airis workspace` commands from the repository root
- Run commands via `docker compose exec workspace ...` instead of host-side `pnpm` in containerized repos
- Add new apps/config to `manifest.toml`, then run `airis workspace gen`
- Run `airis workspace clean` periodically to remove build artifacts

**Don't:**
- Run `pnpm install` on the host of a containerized repo (use `docker compose exec workspace pnpm install`)
- Commit `.env` or `node_modules`
- Hand-edit generated files (`compose.yaml`, `tsconfig.json`) — change `manifest.toml` and run `airis workspace gen`

---

## References

- [manifest.toml Reference](manifest.md)
- [Init Architecture](airis-init-architecture.md)
- [pnpm Catalog](https://pnpm.io/catalogs)
- [Docker Compose Spec](https://docs.docker.com/compose/compose-file/)
