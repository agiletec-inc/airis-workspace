# manifest.toml Configuration Reference

Complete reference for the `manifest.toml` file used by the `airis-workspace` CLI
(`airis workspace <cmd>` via the dispatcher) — a polyglot convention-unification
engine with an optional Docker workspace module.

`manifest.toml` is the **thin source of truth** for workspace tooling. airis uses
**Convention over Configuration**: projects are discovered from repository structure
(`apps/*`, `libs/*`), and the manifest declares intent and exceptions, not everything.

Generated files (`compose.yaml`, `tsconfig.json` / `tsconfig.base.json`, and the AI
adapter files `CLAUDE.md` / `AGENTS.md` / `GEMINI.md`) carry `DO NOT EDIT` markers —
change `manifest.toml` (or `docs/ai/*.md` for the adapters) and run `airis workspace gen`.
Per-project `package.json` dependencies/scripts and `pnpm-workspace.yaml` are
**user-owned**; airis reads them but never overwrites your edits.

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
- [\[commands\]](#commands)
- [\[remap\]](#remap)
- [\[versioning\]](#versioning)
- [\[docs\]](#docs)
- [\[ai\]](#ai)
- [\[templates\]](#templates)
- [\[runtimes\]](#runtimes)
- [\[env\]](#env)
- [\[secrets\]](#secrets)
- [\[rule.\<name\>\]](#rulename)
- [\[orchestration\]](#orchestration)
- [\[policy\]](#policy)

---

## Top-Level Fields

| Field     | Type | Default | Description            |
|-----------|------|---------|------------------------|
| `version` | u32  | `1`     | Schema version number. |

```toml
version = 1
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
description = "Polyglot convention-unification engine for the AI coding era"
authors = ["Agile Technology <hello@agiletec.jp>"]
license = "MIT"
repository = "https://github.com/agiletec-inc/airis-workspace"
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

Cleanup targets used by `airis workspace clean`.

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

Development environment configuration. Controls app discovery, infrastructure compose files, post-startup hooks, and URL display after workspace startup.

| Field          | Type                      | Default                          | Description                                       |
|----------------|---------------------------|----------------------------------|---------------------------------------------------|
| `apps_pattern` | string                    | `"apps/*/docker-compose.yml"`    | Glob pattern for auto-discovering app compose files. |
| `supabase`     | string[]?                 | `null`                           | Paths to Supabase compose files.                  |
| `traefik`      | string?                   | `null`                           | Path to Traefik compose file.                     |
| `urls`         | object?                   | `null`                           | URLs to display after workspace startup.          |
| `post_up`      | string[]                  | `[]`                             | Commands to run after workspace startup (e.g., DB migrations). |

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

Declare application and library projects in the monorepo. Only needed for
**overrides** — projects under `apps/*` and `libs/*` are discovered automatically.

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
| `verify`    | string[]             | `[]`    | Verification commands for `airis workspace verify`.        |
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

Docker module configuration. This section is **optional** — non-containerized repos
(Edge/Workers, native desktop) use airis without it.

| Field           | Type     | Default                | Description                                    |
|-----------------|----------|------------------------|------------------------------------------------|
| `baseImage`     | string   | `""`                   | Base Docker image for builds.                  |
| `workdir`       | string   | `""`                   | Working directory inside containers.           |
| `compose`       | string   | `"compose.yml"`        | Path to the compose file.                      |

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

Package manager configuration. Controls workspace patterns, root `package.json` seeding, and per-app package definitions.

| Field        | Type     | Default | Description                                       |
|--------------|----------|---------|---------------------------------------------------|
| `workspaces` | string[] | `[]`    | Workspace glob patterns (e.g., `["apps/*", "libs/*"]`). |

```toml
[packages]
workspaces = ["apps/*", "libs/*", "packages/*"]
```

> **Shared dependency versions** use the [pnpm catalog](https://pnpm.io/catalogs) in
> `pnpm-workspace.yaml`, which is **user-owned**. airis reads the resolved catalog
> during generation; individual `package.json` files reference versions as `"catalog:"`.

### [packages.root]

Fields seeded into the root `package.json`. Maps directly to standard `package.json` fields. Once generated, dependencies and scripts in `package.json` are yours to edit — airis preserves them.

| Field                  | Type              | Default | Description                              |
|------------------------|-------------------|---------|------------------------------------------|
| `dependencies`         | map<string,string> | `{}`   | Production dependencies.                 |
| `devDependencies`      | map<string,string> | `{}`   | Development dependencies.                |
| `optionalDependencies` | map<string,string> | `{}`   | Optional dependencies.                   |
| `scripts`              | map<string,string> | `{}`   | npm scripts.                             |
| `engines`              | map<string,string> | `{}`   | Engine constraints (e.g., `node >= 20`). |

### [packages.root.pnpm]

pnpm-specific configuration for the root `package.json`.

| Field                                      | Type              | Default | Description                              |
|--------------------------------------------|-------------------|---------|------------------------------------------|
| `overrides`                                | map<string,string> | `{}`   | Dependency overrides.                    |
| `peerDependencyRules.ignoreMissing`        | string[]          | `[]`    | Peer deps to ignore when missing.        |
| `peerDependencyRules.allowedVersions`      | map<string,string> | `{}`   | Allowed peer dependency version ranges.  |
| `onlyBuiltDependencies`                    | string[]          | `[]`    | Dependencies that require native builds. |
| `allowedScripts`                           | map<string,bool>  | `{}`    | Lifecycle scripts to allow.              |

### [[packages.app]]

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
```

---

## [commands]

User-defined commands documenting the repo's canonical tasks. Surfaced to AI
agents and tooling (e.g. via `airis workspace doctor --truth`); run them with
your shell or task runner.

```toml
[commands]
up = "docker compose up -d --build"
down = "docker compose down --remove-orphans"
shell = "docker compose exec -it workspace sh"
install = "docker compose exec workspace pnpm install"
build = "docker compose exec workspace pnpm build"
test = "docker compose exec workspace pnpm test"
lint = "docker compose exec workspace pnpm lint"
```

---

## [remap]

Command translation hints for AI agents (e.g. mapping a generic command to the
repo's preferred equivalent). Remap is opt-in per repo and ships no defaults,
since install/dev runtime is workload-dependent.

```toml
[remap]
"npm install" = "pnpm install"
"docker-compose up" = "docker compose up -d"
```

---

## [versioning]

Version management strategy. Used by `airis workspace bump-version`.

| Field      | Type   | Default     | Description                                  |
|------------|--------|-------------|----------------------------------------------|
| `strategy` | string | `"manual"`  | One of `"manual"`, `"auto"`, or `"conventional-commits"`. |
| `source`   | string | `"0.1.0"`   | Current version string.                      |

Strategies:

| Strategy               | Behavior                                                  |
|------------------------|-----------------------------------------------------------|
| `manual`               | Version bumps only when explicitly requested (`airis workspace bump-version --major/--minor/--patch`). |
| `auto`                 | `airis workspace bump-version --auto` defaults to a minor bump. |
| `conventional-commits` | `airis workspace bump-version --auto` parses the latest commit message to determine bump type (major/minor/patch). |

```toml
[versioning]
strategy = "conventional-commits"
source = "1.56.0"
```

---

## [docs]

AI documentation publication. Controls how airis generates vendor adapter files from shared project docs (`airis workspace docs sync`).

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

## [ai]

AI adapter configuration: which shared rules feed which vendor adapter files, and where per-vendor rule directories live.

```toml
[ai]
shared_rules = [
    "docs/ai/PROJECT_RULES.md",
    "docs/ai/WORKFLOW.md",
    "docs/ai/REVIEW.md",
    "docs/ai/STACK.md",
]

[ai.claude]
target = "CLAUDE.md"
rules_dir = ".claude/rules/generated/"
```

---

## [templates]

Template definitions for `airis workspace new`. Organized by category. Each category is a map of template names to their configurations.

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
```

---

## [runtimes]

Runtime alias configuration for `airis workspace new`. Provides short names that map to template identifiers.

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

Environment variable validation. Checked by `airis workspace doctor` and `airis workspace validate`.

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
| `example`     | string? | `null`  | Example value.                             |

```toml
[env]
required = ["DATABASE_URL", "API_KEY", "SUPABASE_URL"]
optional = ["SENTRY_DSN", "DEBUG"]

[env.validation.DATABASE_URL]
pattern = "^postgresql://"
description = "PostgreSQL connection string"
example = "postgresql://user:pass@localhost:5432/mydb"
```

---

## [secrets]

Secret provider configuration. When configured, `airis workspace gen` wraps its container-based lockfile sync with the provider CLI to inject environment variables automatically.

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

When configured, container commands run as `doppler run --project my-project --config dev -- docker compose ...`.

---

## [rule.\<name\>]

Named rule chains that execute a sequence of commands. Useful for defining composite tasks like CI pipelines or pre-push checks.

| Field      | Type     | Default | Description                        |
|------------|----------|---------|------------------------------------|
| `commands` | string[] | `[]`    | Commands to run in sequence.       |

```toml
[rule.verify]
commands = ["pnpm lint", "pnpm test"]

[rule.ci]
commands = ["pnpm lint", "pnpm test", "pnpm typecheck"]
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
| `proxy`            | string? | `null`  | External proxy network name.                   |
| `default_external` | bool    | `false` | Whether the default network should be external. |

```toml
[orchestration.dev]
workspace = "workspace/docker-compose.yml"
supabase = ["supabase/docker-compose.yml"]
traefik = "traefik/docker-compose.yml"

[orchestration.networks]
default_external = false
```

---

## [policy]

Code governance policy, checked by `airis workspace policy check` / `airis workspace policy enforce`.

```toml
[policy.testing]
mock_policy = "unit-only"
forbidden_patterns = ['vi\.mock.*supabase']
ai_rules = [
    "Never mock Supabase — use local `supabase start` or the test project.",
]

[policy.testing.coverage]
unit = 80
integration = 60

[policy.security]
banned_env_vars = ["SUPABASE_SERVICE_ROLE_KEY"]
scan_secrets = true
```

---

## Initialization

Run `/airis:init` inside Claude Code (or invoke the `workspace_init` MCP tool
directly). The LLM scans the repository and proposes a `manifest.toml` that
preserves comments and formatting. There is no `airis init` CLI entry point —
see [airis-init-architecture.md](airis-init-architecture.md).

For the authoritative field list, see `src/manifest/schema.rs`.

## See Also

- [README.md](../README.md) — User documentation
- [Commands Guide](commands.md) — CLI usage patterns
- [Example manifest](../examples/manifest.toml) — Full annotated example
