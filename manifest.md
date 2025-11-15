# Airis Manifest Specification

**Version**: 1.0
**Format**: TOML
**File**: `manifest.toml`

## Overview

Airis Manifest is a declarative configuration format for Docker-first monorepo workspaces. It replaces scattered configuration files (justfile, package.json, docker-compose.yml) with a single source of truth.

**Design Philosophy**:
- **Declarative**: Describe what you want, not how to achieve it
- **Version Policies**: Use `policy = "latest"` instead of hardcoded version numbers
- **Auto-Generation**: All derived files are generated from manifest.toml
- **Docker-First**: Enforce Docker-based development workflow

---

## Schema Reference

### Root Level

```toml
version = 1                 # Manifest format version
name = "my-monorepo"        # Project name
mode = "docker-first"       # Workflow mode
```

**Mode Options**:
- `docker-first`: Default. All commands run in Docker (except Rust with `runtime: local`)
- `hybrid`: (Future) Allow selective host execution
- `strict`: (Future) Enforce Docker for all operations

---

### Catalog Section

Define version policies for dependencies. Avoid hardcoded version numbers.

```toml
[catalog.react]
policy = "latest"           # Always use latest version

[catalog.next]
policy = "lts"              # Use LTS version

[catalog.typescript]
policy = "^5.0.0"           # Use semver range
```

**Policy Types**:
- `"latest"`: Always use the latest version from npm registry
- `"lts"`: Use the latest LTS (long-term support) version
- `"^X.Y.Z"`: Use semver range (e.g., `^5.0.0` matches `5.x.x`)
- `"~X.Y.Z"`: Use patch range (e.g., `~5.1.0` matches `5.1.x`)

**Resolution**:
- Run `airis workspace sync-deps` to resolve policies to actual versions
- Writes resolved versions to `package.json` `pnpm.catalog` section
- Lock files maintain reproducibility

---

### Workspaces Section

Define apps and libraries in your monorepo.

```toml
[workspaces]
apps = ["corporate-site", "dashboard", "api"]
libs = ["ui", "auth", "db"]
```

---

### Apps Section

Configure individual applications.

```toml
[apps.corporate-site]
path = "apps/corporate-site"
type = "nextjs"
port = 3000

[apps.dashboard]
path = "apps/dashboard"
type = "nextjs"
port = 3100

[apps.api]
path = "apps/api"
type = "node"
port = 9000
```

**App Types**:
- `"nextjs"`: Next.js application
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
- `airis init` automatically detects existing docker-compose.yml files
- Generates this section based on discovered locations
- Safely moves files to optimal locations (workspace/, supabase/, traefik/)

---

### Just Section

Configure justfile generation.

```toml
[just]
output = "justfile.generated"
features = ["docker-first-guard", "type-specific-commands"]
```

**Features**:
- `docker-first-guard`: Block host-level `pnpm`/`npm`/`yarn` with helpful errors
- `type-specific-commands`: Generate type-specific recipes (dev-next, build-rust, etc.)

---

## Auto-Migration Workflow

When you run `airis init` on an existing project:

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

- `workspace.yaml` - Metadata for IDE/tooling compatibility
- `justfile` - Task runner commands
- `package.json` - Root package configuration with `pnpm.catalog`
- `pnpm-workspace.yaml` - pnpm workspace definition
- `docker-compose.yml` - (Future) Workspace compose file

**DO NOT EDIT** generated files directly. Always edit `manifest.toml` and re-run `airis init`.

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

# Just
[just]
output = "justfile.generated"
features = ["docker-first-guard", "type-specific-commands"]
```

---

## Commands

### Initialize/Migrate Project
```bash
airis init              # Auto-detect and generate manifest.toml
airis init --force      # Skip confirmation prompts
```

### Sync Dependencies
```bash
airis workspace sync-deps    # Resolve catalog policies to versions
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
