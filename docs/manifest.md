# Airis Manifest Specification

**Version**: 1.1.0
**Format**: TOML
**File**: `manifest.toml`

## Overview

Airis Manifest is a declarative configuration format for Docker-first monorepo workspaces. It replaces scattered configuration files (justfile, package.json, docker-compose.yml) with a single source of truth.

**Design Philosophy**:
- **Declarative**: Describe what you want, not how to achieve it
- **Version Policies**: Use `policy = "latest"` instead of hardcoded version numbers
- **Auto-Generation**: All derived files are generated from manifest.toml
- **Docker-First**: Enforce Docker-based development workflow
- **Command Unification**: All operations through `airis` CLI
- **LLM Policy Engine**: Control AI behavior via manifest.toml

---

## Schema Reference

### Root Level

```toml
version = 1                 # Manifest format version
```

Airis uses **Convention over Configuration**. It automatically discovers projects in `apps/*` and `libs/*`. `manifest.toml` is used for **exceptions and intent**.

---

### Catalog Section (Optional)

Define version policies for shared dependencies. Airis resolves these to `pnpm.catalog` in the root `package.json`.

```toml
[catalog]
react = "latest"
next = "lts"
typescript = "^5.0.0"
```

---

### Apps & Libs Section (Optional)

Configure individual applications or libraries. Only needed for **overrides** (e.g., custom ports, environment variables).

```toml
[apps.corporate-site]
port = 3000
framework = "nextjs"

[libs.ui]
deps = { "lucide-react" = "latest" }
```

**Key Fields**:
- `path`: (Auto-inferred) Custom path to project
- `framework`: (Auto-detected) nextjs | vite | hono | node | rust
- `port`: Port to expose in Docker Compose
- `deps`: Explicit dependency overrides (preserved in package.json)
- `"node"`: Node.js application
- `"rust"`: Rust application (supports `runtime: local` for GPU)
- `"python"`: Python application

---

### Docker Section

Configure Docker workspace settings.

```toml
[docker]
baseImage = "node:24-bookworm"
workdir = "/app"

[docker.workspace]
service = "workspace"
volumes = ["node_modules"]
```

---

### Dev Section

Define which apps to start automatically in development.

```toml
[dev]
autostart = [
  "corporate-site",
  "dashboard",
  "api",
]
```

**Behavior**:
- `just up` reads this list and runs `pnpm dev` for each app inside workspace container
- Order matters: apps are started in the specified order

---

### Orchestration Section

Define infrastructure stack composition.

```toml
[orchestration.dev]
workspace = "workspace/docker-compose.yml"
supabase = [
  "supabase/docker-compose.yml",
  "supabase/docker-compose.override.yml",
]
traefik = "traefik/docker-compose.yml"
```

**Auto-Detection**:
- The `workspace_init` MCP tool (invoked via `/airis:init` in Claude Code) detects existing docker-compose.yml files
- Generates this section based on discovered locations
- `migration_execute` MCP tool moves files to optimal locations (workspace/, supabase/, traefik/)

---

### Commands Section (NEW in v1.0.2)

Define user commands executed via `airis run <task>`.

```toml
[commands]
install = "docker compose exec workspace pnpm install"
up = "docker compose up -d"
down = "docker compose down"
shell = "docker compose exec workspace bash"
dev = "echo '🚀 Starting...'; docker compose exec workspace pnpm dev"
build = "docker compose exec workspace pnpm build"
test = "docker compose exec workspace pnpm test"
lint = "docker compose exec workspace pnpm lint"
clean = "find . -type d -name 'node_modules' -o -name 'dist' | xargs rm -rf"
```

**Usage**:
```bash
airis run up       # Executes commands.up
airis up           # Shorthand (built-in aliases)
airis dev          # Shorthand for commands.dev
```

**Built-in Shorthands**:
- `airis up` → `airis run up`
- `airis down` → `airis run down`
- `airis shell` → `airis run shell`
- `airis dev` → `airis run dev`
- `airis test` → `airis run test`
- `airis install` → `airis run install`
- `airis build` → `airis run build`
- `airis clean` → `airis run clean`

---

### Guards Section (NEW in v1.0.2)

Control command execution for humans and LLMs.

```toml
[guards]
# Deny these commands (both humans and LLMs)
deny = ["npm", "yarn", "pnpm", "bun"]

# LLM-specific: completely forbid
forbid = ["npm", "yarn", "pnpm", "docker", "docker-compose"]

# Dangerous commands (warn humans, block LLMs)
danger = ["rm -rf /", "chmod -R 777", "chown -R"]
```

**Behavior**:
- `deny`: Block for all users with helpful error message
- `forbid`: LLM-only blocking (via MCP/agent integration)
- `danger`: Prevent catastrophic commands

---

### Remap Section (NEW in v1.0.2)

Automatically translate banned commands to safe alternatives (LLM-targeted).

```toml
[remap]
"npm install" = "airis install"
"pnpm install" = "airis install"
"yarn install" = "airis install"
"npm run dev" = "airis dev"
"pnpm dev" = "airis dev"
"docker compose up" = "airis up"
"docker compose up -d" = "airis up"
"docker compose down" = "airis down"
"docker exec" = "airis shell"
```

**How It Works**:
1. LLM attempts to run `npm install`
2. Shell guard reads manifest.toml `[remap]`
3. Command is translated to `airis install`
4. Safe Docker-based command executes

**Integration Points**:
- MCP servers (airis-mcp-gateway)
- Claude Code / Cursor / Windsurf
- Custom shell guards

---

### Versioning Section (NEW in v1.1.0)

Define versioning strategy and source of truth for automatic version bumping.

```toml
[versioning]
strategy = "conventional-commits"   # or "auto" or "manual"
source = "1.1.0"                   # Current version (synced to Cargo.toml)
```

**Strategy Options**:
- `"conventional-commits"`: Auto-detect bump type from commit message
  - `feat:` → minor bump
  - `fix:` → patch bump
  - `BREAKING CHANGE` or `!:` → major bump
- `"auto"`: Default to minor bump
- `"manual"`: Disable auto-bump (use `airis bump-version` explicitly)

**Usage**:
```bash
# Install Git pre-commit hook
airis hooks install

# Manual version bump
airis bump-version --major    # 1.0.0 → 2.0.0
airis bump-version --minor    # 1.0.0 → 1.1.0
airis bump-version --patch    # 1.0.0 → 1.0.1

# Auto-detect from commit message (requires strategy = "conventional-commits")
airis bump-version --auto

# Or just commit with conventional format
git commit -m "feat: add dark mode"
# → Pre-commit hook auto-bumps 1.0.0 → 1.1.0
```

**Sync Behavior**:
- `manifest.toml` `[versioning.source]` is the single source of truth
- `Cargo.toml` version is automatically synced on bump
- Git pre-commit hook runs `airis bump-version --auto` before commit

---

### Just Section (Optional)

Configure justfile generation (optional in v1.0.2+).

```toml
[just]
output = "justfile.generated"
features = ["docker-first-guard", "type-specific-commands"]
```

**Note**: With `[commands]` section, justfile generation is now optional. You can use `airis` commands directly without `just`.

---

## Auto-Migration Workflow

When Claude Code calls `workspace_init` on an existing project:

1. **Discovery Phase**
   - Scans `apps/` and `libs/` directories
   - Detects docker-compose.yml locations
   - Parses existing package.json for catalog info

2. **Migration Phase**
   - Creates `workspace/` directory if missing
   - Moves docker-compose.yml to correct locations
   - Creates backups (.bak) before moving
   - **Never overwrites** existing files

3. **Generation Phase**
   - Generates `manifest.toml` with detected configuration
   - Generates `workspace.yaml`, `justfile`, `package.json`, etc.

4. **Verification Phase**
   - Shows diff/changes
   - Asks for confirmation (unless `--force`)

---

## Generated Files

All these files are generated from `manifest.toml`:

- `package.json` - Root package configuration with `pnpm.catalog`
- `pnpm-workspace.yaml` - pnpm workspace definition
- `compose.yml` - Docker Compose for services and workspace
- `tsconfig.json` / `tsconfig.base.json` - TypeScript project references
- Per-app `package.json` - App-level dependencies from catalog
- `.env.example` - Environment variable template
- `.github/workflows/` - CI/CD pipelines

**DO NOT EDIT** generated files directly. Always edit `manifest.toml` and run `airis gen`.

---

## Example

Full example demonstrating all features:

```toml
version = 1
name = "agiletec"
mode = "docker-first"

# Catalog: Version policies
[catalog.react]
policy = "latest"

[catalog.next]
policy = "latest"

[catalog.typescript]
policy = "latest"

# Workspaces
[workspaces]
apps = ["corporate-site", "dashboard"]
libs = ["ui", "auth"]

# Apps
[apps.corporate-site]
path = "apps/corporate-site"
type = "nextjs"
port = 3000

[apps.dashboard]
path = "apps/dashboard"
type = "nextjs"
port = 3100

# Docker
[docker]
baseImage = "node:24-bookworm"
workdir = "/app"

[docker.workspace]
service = "workspace"
volumes = ["node_modules"]

# Dev
[dev]
autostart = ["corporate-site", "dashboard"]

# Orchestration
[orchestration.dev]
workspace = "workspace/docker-compose.yml"
supabase = ["supabase/docker-compose.yml"]
traefik = "traefik/docker-compose.yml"

# Just (optional in v1.0.2+)
[just]
output = "justfile.generated"
features = ["docker-first-guard", "type-specific-commands"]

# Commands (v1.0.2+)
[commands]
install = "docker compose exec workspace pnpm install"
up = "docker compose up -d"
down = "docker compose down"
dev = "docker compose exec workspace pnpm dev"
build = "docker compose exec workspace pnpm build"

# Guards (v1.0.2+)
[guards]
deny = ["npm", "yarn", "pnpm", "bun"]
forbid = ["npm", "yarn", "pnpm", "docker", "docker-compose"]

# Remap (v1.0.2+)
[remap]
"npm install" = "airis install"
"pnpm install" = "airis install"

# Versioning (v1.1.0+)
[versioning]
strategy = "conventional-commits"
source = "1.1.0"
```

---

## Commands

### Initialize/Migrate Project
Run `/airis:init` inside Claude Code (or invoke the `workspace_init` MCP tool
directly). The LLM scans the repository and proposes a `manifest.toml` that
preserves comments and formatting — something a TOML re-serializer cannot do.
There is no `airis init` CLI entry point.

### Resolve Dependencies
```bash
airis gen    # Resolve catalog policies to versions and regenerate files
```

### Validate Configuration
```bash
airis validate          # Check manifest.toml for errors
```

---

## See Also

- [README.md](README.md) - User documentation
- [CLAUDE.md](CLAUDE.md) - Development guidelines
- [PROJECT_INDEX.md](PROJECT_INDEX.md) - Code structure
