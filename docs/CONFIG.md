# manifest.toml Configuration Reference

Complete reference for the `manifest.toml` file used by the airis CLI -- a Docker-first monorepo workspace manager.

`manifest.toml` is the single source of truth for the entire workspace. All generated files (`package.json`, `pnpm-workspace.yaml`, `compose.yml`, `tsconfig.json`) are derived from it. Never edit generated files directly; change `manifest.toml` and run `airis gen`.

---

## Table of Contents

- [Top-Level Fields](#top-level-fields)
- [\[project\]](#project)
- [\[workspace\]](#workspace)
- [\[dev\]](#dev)
- [\[\[app\]\]](#app)
- [\[stack.\<name\>\]](#stackname)
- [\[service.\<name\>\]](#servicename)
- [\[docker\]](#docker)
- [\[packages\]](#packages)
- [\[packages.catalog\]](#packagescatalog)
- [\[packages.root\]](#packagesroot)
- [\[\[packages.app\]\]](#packagesapp)
- [\[guards\]](#guards)
- [\[commands\]](#commands)
- [\[remap\]](#remap)
- [\[versioning\]](#versioning)
- [\[docs\]](#docs)
- [\[ci\]](#ci)
- [\[templates\]](#templates)
- [\[runtimes\]](#runtimes)
- [\[env\]](#env)
- [\[secrets\]](#secrets)
- [\[rule.\<name\>\]](#rulename)
- [\[orchestration\]](#orchestration)

---

## Top-Level Fields

Schema version and workspace mode at the root of the file.

| Field     | Type   | Default          | Description                        |
|-----------|--------|------------------|------------------------------------|
| `version` | u32    | `1`              | Schema version number.             |
| `mode`    | string | `"docker-first"` | Workspace mode. One of `"docker-first"`, `"hybrid"`, or `"strict"`. |

```toml
version = 1
mode = "docker-first"
```

---

## [project]

Project metadata. Acts as the source of truth for `Cargo.toml`, Homebrew formula generation, and other downstream consumers.

| Field          | Type     | Default | Description                                    |
|----------------|----------|---------|------------------------------------------------|
| `id`           | string   | `""`    | Project identifier (e.g., `"airis-workspace"`). |
| `binary_name`  | string   | `""`    | CLI binary name (e.g., `"airis"`).             |
| `version`      | string   | `""`    | Semantic version (e.g., `"1.56.0"`).           |
| `description`  | string   | `""`    | Short project description.                     |
| `authors`      | string[] | `[]`    | List of authors.                               |
| `license`      | string   | `""`    | License identifier (e.g., `"MIT"`).            |
| `homepage`     | string   | `""`    | Project homepage URL.                          |
| `repository`   | string   | `""`    | Source repository URL.                         |
| `keywords`     | string[] | `[]`    | Keywords for discovery.                        |
| `categories`   | string[] | `[]`    | Categories for classification.                 |
| `rust_edition`  | string   | `""`    | Rust edition (e.g., `"2024"`).                 |

```toml
[project]
id = "airis-workspace"
binary_name = "airis"
version = "1.56.0"
description = "Docker-first monorepo workspace manager"
authors = ["Agile Technology <hello@agiletec.jp>"]
license = "MIT"
homepage = "https://github.com/agiletec-inc/airis-workspace"
repository = "https://github.com/agiletec-inc/airis-workspace"
keywords = ["monorepo", "docker", "workspace", "cli"]
categories = ["command-line-utilities", "development-tools"]
rust_edition = "2024"
```

---

## [workspace]

Core workspace settings for Docker Compose integration and cleanup behavior.

| Field             | Type     | Default                                  | Description                                  |
|-------------------|----------|------------------------------------------|----------------------------------------------|
| `name`            | string   | Git root directory name                  | Docker Compose project name.                 |
| `package_manager` | string   | `"pnpm@10.22.0"`                        | Package manager with version.                |
| `service`         | string   | `"workspace"`                            | Primary Docker Compose service name.         |
| `image`           | string   | `"node:22-alpine"`                       | Docker image for the workspace container.    |
| `workdir`         | string   | `"/app"`                                 | Working directory inside the container.      |
| `volumes`         | string[] | `["workspace-node-modules:/app/node_modules"]` | Docker volume mounts.               |

### [workspace.clean]

Cleanup targets used by `airis clean`.

| Field       | Type     | Default                                                        | Description                                    |
|-------------|----------|----------------------------------------------------------------|------------------------------------------------|
| `dirs`      | string[] | `[".next", "dist", "build", "out", ".turbo", ".swc", ".cache"]` | Root directories to remove.                  |
| `recursive` | string[] | `["node_modules"]`                                             | Patterns to find and remove recursively.       |

```toml
[workspace]
name = "my-project"
package_manager = "pnpm@10.22.0"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"
volumes = ["my-project-node-modules:/app/node_modules"]

[workspace.clean]
dirs = [".next", "dist", "build", "out", ".turbo"]
recursive = ["node_modules"]
```

---

## [dev]

Development environment configuration. Controls app discovery, infrastructure compose files, post-startup hooks, and URL display after `airis up`.

| Field          | Type                      | Default                          | Description                                       |
|----------------|---------------------------|----------------------------------|---------------------------------------------------|
| `apps_pattern` | string                    | `"apps/*/docker-compose.yml"`    | Glob pattern for auto-discovering app compose files. |
| `supabase`     | string[]?                 | `null`                           | Paths to Supabase compose files.                  |
| `traefik`      | string?                   | `null`                           | Path to Traefik compose file.                     |
| `urls`         | object?                   | `null`                           | URLs to display after `airis up`.                 |
| `post_up`      | string[]                  | `[]`                             | Commands to run after `airis up` (e.g., DB migrations). |

### [dev.urls]

| Field   | Type                    | Default | Description               |
|---------|-------------------------|---------|---------------------------|
| `infra` | `[{name, url}]`        | `[]`    | Infrastructure URLs.      |
| `apps`  | `[{name, url}]`        | `[]`    | Application URLs.         |

```toml
[dev]
apps_pattern = "apps/*/docker-compose.yml"
supabase = ["supabase/docker-compose.yml"]
traefik = "traefik/docker-compose.yml"
post_up = [
    "docker compose exec workspace pnpm db:migrate",
    "docker compose exec workspace pnpm db:seed",
]

[dev.urls]
infra = [
    { name = "Supabase Studio", url = "http://localhost:54323" },
    { name = "Traefik Dashboard", url = "http://localhost:8080" },
]
apps = [
    { name = "Dashboard", url = "http://localhost:3000" },
    { name = "API", url = "http://localhost:4000" },
]
```

---

## [[app]]

Declare application and library projects in the monorepo.

| Field       | Type    | Default | Description                                                                 |
|-------------|---------|---------|-----------------------------------------------------------------------------|
| `name`      | string  | --      | Project name.                                                               |
| `path`      | string? | `null`  | Project directory path (e.g., `"apps/web"`).                                |
| `use`       | string? | `null`  | Reference to a `[stack]` definition for automatic configuration.            |
| `framework` | string? | `null`  | Legacy framework type (e.g., `"nextjs"`, `"vite"`). Prefer `use`.           |
| `python`    | string? | `null`  | Python version constraint (e.g., `"3.12"`).                                 |
| `cuda`      | string? | `null`  | CUDA version for GPU support (e.g., `"12.4"`). Triggers GPU resource setup. |

```toml
[[app]]
name = "dashboard"
path = "apps/dashboard"
use = "nextjs"

[[app]]
name = "ai-model"
path = "apps/ai-model"
use = "python-ml"
python = "3.12"
cuda = "12.4"
```

---

## [stack.\<name\>]

Define reusable technology stacks. Stacks automate Docker volume isolation, environment variables, and quality verification.

| Field       | Type                 | Default | Description                                                |
|-------------|----------------------|---------|------------------------------------------------------------|
| `image`     | string?              | `null`  | Base Docker image for this stack.                          |
| `artifacts` | string[]             | `[]`    | Local directories to isolate in named volumes (e.g., `.next`). |
| `volumes`   | map<string,string>   | `{}`    | Global cache volumes (e.g., `pnpm-store`).                 |
| `verify`    | string[]             | `[]`    | Verification commands for `airis verify`.                  |
| `gpu`       | bool                 | `false` | Enable NVIDIA GPU resource reservation.                    |
| `scripts`   | map<string,string>   | `{}`    | npm scripts to inject into `package.json`.                 |

```toml
[stack.python-ml]
image = "nvidia/cuda:12.4-runtime-ubuntu22.04"
artifacts = [".venv", "__pycache__", ".pytest_cache"]
volumes = { "uv-cache" = "/root/.cache/uv" }
verify = ["pytest", "ruff check ."]
gpu = true

[stack.nextjs]
artifacts = [".next", ".turbo", "node_modules/.cache"]
verify = ["tsc --noEmit", "next build"]
```

---

## [service.\<name\>]

Define infrastructure services (databases, caches, etc.) for Docker Compose generation.

| Field     | Type              | Default | Description                              |
|-----------|-------------------|---------|------------------------------------------|
| `image`   | string            | `""`    | Docker image (e.g., `"postgres:16-alpine"`). |
| `port`    | u16?              | `null`  | Exposed port number.                     |
| `command` | string?           | `null`  | Override the default container command.   |
| `volumes` | string[]          | `[]`    | Volume mounts for the service.           |
| `env`     | map<string,string> | `{}`   | Environment variables.                   |

```toml
[service.postgres]
image = "postgres:16-alpine"
port = 5432
volumes = ["pg-data:/var/lib/postgresql/data"]

[service.postgres.env]
POSTGRES_USER = "dev"
POSTGRES_PASSWORD = "dev"
POSTGRES_DB = "myapp"

[service.redis]
image = "redis:7-alpine"
port = 6379
```

---

## [docker]

Docker-specific configuration for build images, command routing, and shim generation.

| Field           | Type     | Default                | Description                                    |
|-----------------|----------|------------------------|------------------------------------------------|
| `baseImage`     | string   | `""`                   | Base Docker image for builds.                  |
| `workdir`       | string   | `""`                   | Working directory inside containers.           |
| `compose`       | string   | `"docker-compose.yml"` | Path to the compose file.                      |
| `service`       | string   | `"workspace"`          | Default service for command execution.         |
| `shim_commands` | string[] | see below              | Commands to generate shims for (`airis shim install`). |

Default `shim_commands`: `["pnpm", "npm", "node", "npx", "bun", "tsx", "next", "eslint", "vitest", "tsc", "turbo"]`

### [docker.workspace]

| Field     | Type     | Default | Description                  |
|-----------|----------|---------|------------------------------|
| `service` | string   | --      | Workspace service name.      |
| `volumes` | string[] | `[]`    | Volume names for the service. |

### [[docker.routes]]

Command routing rules that map glob patterns to specific services and working directories.

| Field     | Type   | Description                                              |
|-----------|--------|----------------------------------------------------------|
| `glob`    | string | Glob pattern to match (e.g., `"apps/*"`).                |
| `service` | string | Docker service to route commands to.                     |
| `workdir` | string | Working directory template (supports `{match}` placeholder). |

```toml
[docker]
baseImage = "node:22-bookworm"
workdir = "/app"
compose = "docker-compose.yml"
service = "workspace"
shim_commands = ["pnpm", "npm", "node", "npx"]

[docker.workspace]
service = "workspace"
volumes = ["node_modules"]

[[docker.routes]]
glob = "apps/*"
service = "workspace"
workdir = "/app/{match}"
```

---

## [packages]

Package manager configuration. Controls workspace patterns, version catalogs, root `package.json` fields, and per-app package definitions.

| Field        | Type     | Default | Description                                       |
|--------------|----------|---------|---------------------------------------------------|
| `workspaces` | string[] | `[]`    | Workspace glob patterns (e.g., `["apps/*", "libs/*"]`). |

```toml
[packages]
workspaces = ["apps/*", "libs/*", "packages/*"]
```

---

## [packages.catalog]

Version catalog for dependency management. Dependencies declared here are resolved and written to `pnpm-workspace.yaml`'s catalog section. Individual `package.json` files use `"catalog:"` references instead of hardcoded versions.

Each entry can be one of:

| Format                    | Description                                     |
|---------------------------|-------------------------------------------------|
| `"latest"`                | Resolve to the latest npm version at generation time. |
| `"lts"`                   | Resolve to the current LTS version from npm dist-tags. |
| `"^5.0.0"` (semver)      | Use the specified semver range as-is.           |
| `{ follow = "react" }`   | Follow another catalog entry's resolved version. |

```toml
[packages.catalog]
react = "latest"
next = "latest"
typescript = "latest"
hono = "latest"
vitest = "latest"
eslint = "latest"
zod = "^3.22.0"

[packages.catalog.react-dom]
follow = "react"

[packages.catalog.react-is]
follow = "react"
```

---

## [packages.root]

Fields injected into the root `package.json`. Maps directly to standard `package.json` fields.

| Field                  | Type              | Default | Description                              |
|------------------------|-------------------|---------|------------------------------------------|
| `dependencies`         | map<string,string> | `{}`   | Production dependencies.                 |
| `devDependencies`      | map<string,string> | `{}`   | Development dependencies.                |
| `optionalDependencies` | map<string,string> | `{}`   | Optional dependencies.                   |
| `scripts`              | map<string,string> | `{}`   | npm scripts.                             |
| `engines`              | map<string,string> | `{}`   | Engine constraints (e.g., `node >= 20`). |

### [packages.root.pnpm]

pnpm-specific configuration injected into the root `package.json`.

| Field                                      | Type              | Default | Description                              |
|--------------------------------------------|-------------------|---------|------------------------------------------|
| `overrides`                                | map<string,string> | `{}`   | Dependency overrides.                    |
| `peerDependencyRules.ignoreMissing`        | string[]          | `[]`    | Peer deps to ignore when missing.        |
| `peerDependencyRules.allowedVersions`      | map<string,string> | `{}`   | Allowed peer dependency version ranges.  |
| `onlyBuiltDependencies`                    | string[]          | `[]`    | Dependencies that require native builds. |
| `allowedScripts`                           | map<string,bool>  | `{}`    | Lifecycle scripts to allow.              |

```toml
[packages.root.dependencies]

[packages.root.devDependencies]
turbo = "catalog:"

[packages.root.scripts]
dev = "turbo dev"
build = "turbo build"
lint = "turbo lint"
test = "turbo test"

[packages.root.engines]
node = ">=22"

[packages.root.pnpm.overrides]
react = "catalog:"

[packages.root.pnpm.peerDependencyRules]
ignoreMissing = ["@types/react"]

[packages.root.pnpm.peerDependencyRules.allowedVersions]
react = "19"

[packages.root.pnpm]
onlyBuiltDependencies = ["sharp"]

[packages.root.pnpm.allowedScripts]
prisma = true
```

---

## [[packages.app]]

Array of tables defining per-app package.json fields. Uses a glob pattern to match app directories.

| Field             | Type              | Default | Description                              |
|-------------------|-------------------|---------|------------------------------------------|
| `pattern`         | string            | --      | Glob pattern (e.g., `"apps/web"`).       |
| `dependencies`    | map<string,string> | `{}`   | Production dependencies for matched apps. |
| `devDependencies` | map<string,string> | `{}`   | Dev dependencies for matched apps.       |
| `scripts`         | map<string,string> | `{}`   | npm scripts for matched apps.            |

```toml
[[packages.app]]
pattern = "apps/web"
dependencies = { next = "catalog:", react = "catalog:", react-dom = "catalog:" }
devDependencies = { typescript = "catalog:" }
scripts = { dev = "next dev", build = "next build" }

[[packages.app]]
pattern = "apps/api"
dependencies = { hono = "catalog:" }
scripts = { dev = "tsx watch src/index.ts", build = "tsc" }
```

---

## [guards]

Docker-first enforcement. Blocks, wraps, or remaps commands to prevent host-level package manager usage.

| Field              | Type              | Default | Description                                       |
|--------------------|-------------------|---------|---------------------------------------------------|
| `deny`             | string[]          | `[]`    | Commands blocked for all users.                   |
| `wrap`             | map<string,string> | `{}`   | Commands wrapped with Docker execution.           |
| `deny_with_message`| map<string,string> | `{}`   | Commands blocked with a custom error message.     |
| `forbid`           | string[]          | `[]`    | Commands blocked specifically for LLM agents (via MCP). |
| `danger`           | string[]          | `[]`    | Dangerous commands (warn humans, block LLMs).     |

```toml
[guards]
deny = ["npm", "yarn", "pnpm", "bun"]
forbid = ["npm", "yarn", "pnpm", "docker", "docker-compose"]
danger = ["rm -rf /", "chmod -R 777"]

[guards.wrap]
pnpm = "docker compose exec workspace pnpm"

[guards.deny_with_message]
"pip install" = "Use airis shell to enter the container, then run pip install."
```

---

## [commands]

User-defined commands executed via `airis run <name>`. Built-in aliases (`airis up`, `airis dev`, `airis shell`, etc.) map to keys here.

Each entry is a key-value pair where the key is the command name and the value is the shell command to execute.

```toml
[commands]
up = "docker compose up -d --build"
down = "docker compose down --remove-orphans"
shell = "docker compose exec -it workspace sh"
install = "docker compose exec workspace pnpm install"
build = "docker compose exec workspace pnpm build"
test = "docker compose exec workspace pnpm test"
lint = "docker compose exec workspace pnpm lint"
clean = "find . -type d -name 'node_modules' -o -name 'dist' -o -name '.next' | xargs rm -rf"
logs = "docker compose logs -f"
ps = "docker compose ps"
```

---

## [remap]

Automatic command translation. When a user or LLM agent runs a blocked command, airis suggests the safe replacement defined here.

Each entry maps a "blocked command" to its "safe replacement".

```toml
[remap]
"npm install" = "airis install"
"pnpm install" = "airis install"
"yarn install" = "airis install"
"npm run dev" = "airis dev"
"pnpm dev" = "airis dev"
"docker compose up" = "airis up"
"docker compose down" = "airis down"
"docker exec" = "airis shell"
```

---

## [versioning]

Version management strategy. Works with `airis bump-version` and Git pre-commit hooks.

| Field      | Type   | Default     | Description                                  |
|------------|--------|-------------|----------------------------------------------|
| `strategy` | string | `"manual"`  | One of `"manual"`, `"auto"`, or `"conventional-commits"`. |
| `source`   | string | `"0.1.0"`   | Current version string.                      |

Strategies:

| Strategy               | Behavior                                                  |
|------------------------|-----------------------------------------------------------|
| `manual`               | Version bumps only when explicitly requested.             |
| `auto`                 | Auto-increment minor version on every commit.             |
| `conventional-commits` | Parse commit messages to determine bump type (major/minor/patch). |

```toml
[versioning]
strategy = "conventional-commits"
source = "1.56.0"
```

---

## [docs]

AI documentation publication. Controls how airis generates vendor adapter files from shared project docs.

| Field           | Type     | Default  | Description |
|----------------|----------|----------|-------------|
| `targets`      | string[] | `[]`     | Explicit adapter files to generate. If omitted, airis derives targets from `vendors`. |
| `mode`         | string   | `"warn"` | Overwrite mode: `"warn"` (refuse) or `"backup"` (create `.bak`). |
| `sources`      | string[] | `[]`     | Shared AI instruction files that act as the source of truth. |
| `vendors`      | string[] | `[]`     | Vendor adapters to generate. Supported values: `"codex"`, `"claude"`, `"gemini"`. |
| `skills_source`| string?  | `null`   | Shared playbook or skill source directory. |
| `hooks_policy` | string?  | `null`   | Shared hook-policy document for portable guard intent. |

```toml
[docs]
mode = "backup"
sources = [
  "docs/ai/PROJECT_RULES.md",
  "docs/ai/WORKFLOW.md",
  "docs/ai/REVIEW.md",
  "docs/ai/STACK.md",
]
vendors = ["codex", "claude", "gemini"]
skills_source = "docs/ai/playbooks"
hooks_policy = "docs/ai/hooks/HOOKS_POLICY.md"
```

---

## [ci]

CI/CD workflow generation for GitHub Actions.

| Field                | Type    | Default            | Description                                           |
|----------------------|---------|--------------------|-------------------------------------------------------|
| `enabled`            | bool    | `true`             | Enable CI workflow generation.                        |
| `auto_version`       | bool    | `true`             | Enable automatic versioning via Conventional Commits. |
| `repository`         | string? | `null`             | GitHub repository (`"owner/repo"`).                   |
| `homebrew_tap`       | string? | `null`             | Homebrew tap repository (`"owner/homebrew-tap"`).     |
| `runner`             | string? | `null`             | CI runner label (e.g., `"self-hosted"`). Default in workflow: `"ubuntu-latest"`. |
| `node_version`       | string? | `null`             | Node.js version (e.g., `"24"`). Default in workflow: `"22"`. |
| `affected`           | bool    | `false`            | Use `turbo --affected` for incremental builds.        |
| `concurrency_cancel` | bool    | `true`             | Enable concurrency cancel-in-progress for workflows.  |
| `cache`              | bool    | `true`             | Enable GitHub Actions cache for pnpm. Disable for self-hosted runners. |
| `pnpm_store_path`    | string? | `null`             | Path to persistent pnpm store (for self-hosted runners with volumes). |

### [ci.auto_merge]

| Field     | Type   | Default  | Description                           |
|-----------|--------|----------|---------------------------------------|
| `enabled` | bool   | `true`   | Enable auto-merge workflow.           |
| `from`    | string | `"next"` | Source branch for auto-merge.         |
| `to`      | string | `"main"` | Target branch for auto-merge.         |

```toml
[ci]
enabled = true
auto_version = true
repository = "agiletec-inc/my-project"
homebrew_tap = "agiletec-inc/homebrew-tap"
runner = "self-hosted"
node_version = "22"
affected = true
concurrency_cancel = true
cache = false
pnpm_store_path = "/mnt/cache/pnpm-store"

[ci.auto_merge]
enabled = true
from = "stg"
to = "main"
```

---

## [templates]

Template definitions for `airis new`. Organized by category. Each category is a map of template names to their configurations.

Categories: `api`, `web`, `worker`, `cli`, `lib`, `edge`, `supabase-trigger`, `supabase-realtime`.

### Template Fields

| Field            | Type     | Default | Description                                    |
|------------------|----------|---------|------------------------------------------------|
| `entry`          | string   | `""`    | Entry point file (e.g., `"src/index.ts"`).     |
| `dockerfile`     | string   | `""`    | Dockerfile template path.                      |
| `runtime`        | string   | `""`    | Runtime identifier (e.g., `"node"`, `"rust"`). |
| `deps`           | string[] | `[]`    | Production dependencies to inject.             |
| `dev_deps`       | string[] | `[]`    | Dev dependencies to inject.                    |
| `inject`         | string[] | `[]`    | Features/modules to inject into the template.  |
| `package_config` | string   | `""`    | Package config file path (`package.json`, `Cargo.toml`, etc.). |

```toml
[templates.api.hono]
entry = "src/index.ts"
dockerfile = "templates/api/Dockerfile.hono"
runtime = "node"
deps = ["hono", "@hono/node-server"]
dev_deps = ["tsx", "typescript"]
package_config = "package.json"

[templates.web.nextjs]
entry = "src/app/page.tsx"
dockerfile = "templates/web/Dockerfile.nextjs"
runtime = "node"
deps = ["next", "react", "react-dom"]
dev_deps = ["typescript", "@types/react"]
package_config = "package.json"

[templates.api.rust-axum]
entry = "src/main.rs"
dockerfile = "templates/api/Dockerfile.rust"
runtime = "rust"
deps = ["axum", "tokio", "serde"]
package_config = "Cargo.toml"
```

---

## [runtimes]

Runtime alias configuration for `airis new`. Provides short names that map to template identifiers.

| Field   | Type              | Default | Description                                |
|---------|-------------------|---------|--------------------------------------------|
| `alias` | map<string,string> | `{}`   | Short aliases (e.g., `"py"` -> `"fastapi"`). |

```toml
[runtimes.alias]
ts = "hono"
py = "fastapi"
rs = "rust-axum"
```

---

## [env]

Environment variable validation. Checked by `airis doctor` and `airis validate`.

| Field      | Type     | Default | Description                              |
|------------|----------|---------|------------------------------------------|
| `required` | string[] | `[]`    | Variables that must be set.              |
| `optional` | string[] | `[]`    | Variables that are recognized but not required. |

### [env.validation.\<name\>]

Per-variable validation rules.

| Field         | Type    | Default | Description                                |
|---------------|---------|---------|--------------------------------------------|
| `pattern`     | string? | `null`  | Regex pattern to validate the value.       |
| `description` | string? | `null`  | Human-readable description.                |
| `example`     | string? | `null`  | Example value (used in `.env.example`).    |

```toml
[env]
required = ["DATABASE_URL", "API_KEY", "SUPABASE_URL"]
optional = ["SENTRY_DSN", "DEBUG"]

[env.validation.DATABASE_URL]
pattern = "^postgresql://"
description = "PostgreSQL connection string"
example = "postgresql://user:pass@localhost:5432/mydb"

[env.validation.API_KEY]
description = "External API key"
example = "sk-xxxxxxxxxxxxxxxxxxxx"
```

---

## [secrets]

Secret provider configuration. When configured, `airis up` wraps Docker Compose with the provider CLI to inject environment variables automatically.

| Field      | Type    | Default | Description                                      |
|------------|---------|---------|--------------------------------------------------|
| `provider` | string  | —       | Provider name. Currently supported: `"doppler"`. |

### [secrets.doppler]

Required when `provider = "doppler"`.

| Field     | Type   | Default | Description                                           |
|-----------|--------|---------|-------------------------------------------------------|
| `project` | string | —       | Doppler project name.                                 |
| `config`  | string | —       | Doppler config name (e.g., `"dev"`, `"stg"`, `"prd"`). |

```toml
[secrets]
provider = "doppler"

[secrets.doppler]
project = "my-project"
config = "dev"
```

When configured, `airis up` becomes `doppler run --project my-project --config dev -- docker compose up ...`. The lockfile sync fallback also uses the provider when the workspace container is not running.

---

## [rule.\<name\>]

Named rule chains that execute a sequence of commands. Useful for defining composite tasks like CI pipelines or pre-push checks.

| Field      | Type     | Default | Description                        |
|------------|----------|---------|------------------------------------|
| `commands` | string[] | `[]`    | Commands to run in sequence.       |

```toml
[rule.verify]
commands = ["airis lint", "airis test"]

[rule.ci]
commands = ["airis lint", "airis test", "airis build"]

[rule.pre-push]
commands = ["airis lint", "airis typecheck", "airis test"]
```

---

## [orchestration]

Multi-compose orchestration for complex development environments with multiple infrastructure layers.

### [orchestration.dev]

| Field       | Type     | Default | Description                                        |
|-------------|----------|---------|----------------------------------------------------|
| `workspace` | string?  | `null`  | Path to workspace compose file.                    |
| `supabase`  | string[]?| `null`  | Paths to Supabase compose files.                   |
| `traefik`   | string?  | `null`  | Path to Traefik compose file.                      |

### [orchestration.networks]

| Field              | Type    | Default | Description                                    |
|--------------------|---------|---------|------------------------------------------------|
| `proxy`            | string? | `null`  | External proxy network name (e.g., `"coolify"`). |
| `default_external` | bool    | `false` | Whether the default network should be external. |

```toml
[orchestration.dev]
workspace = "workspace/docker-compose.yml"
supabase = [
    "supabase/docker-compose.yml",
    "supabase/docker-compose.override.yml",
]
traefik = "traefik/docker-compose.yml"

[orchestration.networks]
proxy = "coolify"
default_external = false
```
