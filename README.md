# 🚀 AIris Workspace

**The first Rust-powered, Docker-first Monorepo Engine built for the LLM era**

- Auto-generate all `package.json`, `pnpm-workspace.yaml`, `docker-compose.yml` from a single `manifest.toml`
- Write `"latest"` or `"lts"` → automatically resolves to real semver (no more broken tests)
- LLM breaks your config? → `airis init` self-heals instantly

> **NX and Turborepo were built for humans. airis-workspace is built for AI-assisted development.**
>
> Zero-Config Monorepo Engine for 2025+

![airis init generates your entire monorepo config](assets/airis-init-demo.gif)

---

## 💡 Why I Built This

### The Pain Points

Running a monorepo comes with these frustrations:

- **Version hell** - React version in root `package.json` differs from `apps/`. Manually updating everything is tedious
- **Config file sprawl** - `package.json`, `pnpm-workspace.yaml`, `docker-compose.yml`... Which one is the source of truth?
- **LLMs break things** - Claude Code or Cursor says "I ran `pnpm install` for you" and pollutes your host environment
- **"Works on my machine"** - TypeScript builds pass locally but fail on teammates' machines

### The Solution: Single Source of Truth

**Make `manifest.toml` the only config file. Auto-generate everything else.**

```
manifest.toml (edit this only)
    ↓ airis init
package.json, pnpm-workspace.yaml, docker-compose.yml (all auto-generated)
```

This gives you:
- Centralized version management in `[packages.catalog]`. Write `react = "latest"` and all apps get the same resolved version
- When LLMs break `package.json`, just run `airis init` to regenerate instantly
- Docker-first guards prevent host environment pollution

### Why Rust?

- **Fast** - `airis init` completes in tens of milliseconds. Run it before every commit without noticing
- **Single binary** - No Node.js or Python dependencies. Just `brew install` and go
- **Cross-platform** - Same binary works on macOS (Apple Silicon/Intel), Linux, and Windows

---

## 🌟 Part of the AIRIS Ecosystem

AIris Workspace is the **development environment enforcer** of the **AIRIS Suite** - ensuring consistent, Docker-first monorepo workflows.

### The AIRIS Suite

| Component | Purpose | For Who |
|-----------|---------|---------|
| **[airis-agent](https://github.com/agiletec-inc/airis-agent)** | 🧠 Intelligence layer for all editors (confidence checks, deep research, self-review) | All developers using Claude Code, Cursor, Windsurf, Codex, Gemini CLI |
| **[airis-mcp-gateway](https://github.com/agiletec-inc/airis-mcp-gateway)** | 🚪 Unified MCP proxy with 90% token reduction via lazy loading | Claude Code users who want faster startup |
| **[mindbase](https://github.com/agiletec-inc/mindbase)** | 💾 Local cross-session memory with semantic search | Developers who want persistent conversation history |
| **airis-workspace** (this repo) | 🏗️ Docker-first monorepo manager | Teams building monorepos |
| **[airiscode](https://github.com/agiletec-inc/airiscode)** | 🖥️ Terminal-first autonomous coding agent | CLI-first developers |

### MCP Servers (Included via Gateway)

- **[airis-mcp-supabase-selfhost](https://github.com/agiletec-inc/airis-mcp-supabase-selfhost)** - Self-hosted Supabase MCP with RLS support
- **mindbase** - Memory search & storage tools (`mindbase_search`, `mindbase_store`)

### Quick Install: Complete AIRIS Suite

```bash
# Install all AIRIS tools via Homebrew (recommended)
brew tap agiletec-inc/tap
brew install airis-workspace airis-mcp-gateway

# For Claude Code users: Add airis-agent plugin
/plugin marketplace add agiletec-inc/airis-agent
/plugin install airis-agent

# Then start using airis-workspace
cd your-monorepo && airis init
```

**What you get with the full suite:**
- ✅ Confidence-gated workflows (prevents wrong-direction coding)
- ✅ Deep research with evidence synthesis
- ✅ 94% token reduction via repository indexing
- ✅ Cross-session memory across all editors
- ✅ Self-review and post-implementation validation

---

## ✨ Killer Features

### 1. Auto Version Resolution (No Other Tool Has This)

```toml
# manifest.toml
[packages.catalog]
# Frontend: Always latest for security patches and best practices
react = "latest"           # → ^19.2.0
next = "latest"            # → ^15.1.0
tailwindcss = "latest"     # → ^4.0.0

# Backend: Stable versions to avoid breaking changes
express = "lts"            # → LTS version
typescript = "^5.0"        # → Pinned major version
```

```bash
$ airis init
📦 Resolving catalog versions from npm registry...
  ✓ react latest → ^19.2.0
  ✓ next latest → ^15.1.0
  ✓ express lts → ^4.21.0
  ✓ typescript ^5.0 → ^5.0
```

**Real version numbers are resolved and written to `pnpm-workspace.yaml`.** Tests and builds use actual semver ranges, not string literals like `"latest"`. No more "it works with latest but breaks in CI" issues.

**No more manually updating 20 package.json files when React releases a new version.** Write `"latest"` once, and every app in your monorepo gets the same resolved version.

### 2. Flexible Runtime: Docker-First with Local Escape Hatches

```toml
# manifest.toml
[apps.api]
runtime = "docker"         # Default: runs in container

[apps.ml-inference]
runtime = "local"          # Escape hatch for GPU workloads
```

**Why this matters:**

- **Default Docker-first**: Clean host, reproducible builds, no "works on my machine"
- **Local runtime option**: For Apple Silicon GPU inference (Ollama, MLX), Docker adds latency
- **Best of both worlds**: Develop with local GPU speed, deploy to Linux containers in production

```bash
# Local development with Apple Silicon GPU
$ airis dev ml-inference  # Runs locally, uses Metal GPU

# Production deployment
$ docker build -t ml-inference .  # Same code, Linux container
```

### 3. Auto Versioning with Conventional Commits

```bash
$ git commit -m "feat: add dark mode"
# Pre-commit hook auto-bumps: 1.0.0 → 1.1.0

$ git commit -m "fix: button alignment"
# Auto-bumps: 1.1.0 → 1.1.1

$ git commit -m "feat!: breaking API change"
# Auto-bumps: 1.1.1 → 2.0.0
```

Semantic versioning happens automatically. No more forgetting to bump versions or arguing about what version to use.

### 4. LLM-Proof Monorepo

```bash
$ pnpm install
❌ ERROR: 'pnpm' must run inside Docker workspace

   Use: airis install

   Or configure [remap] in manifest.toml to auto-translate commands.
```

When Claude Code or Cursor tries to run `pnpm install` on your host, it gets blocked with a helpful error. **Your host environment stays clean.**

### 5. Self-Healing Config Files

![airis doctor auto-heals broken configs](assets/airis-doctor-demo.gif)

```bash
# LLM broke your package.json? No problem.
$ airis doctor
🔍 Diagnosing workspace health...

⚠️  Detected inconsistencies:
   ❌ package.json - Content mismatch (15 lines differ)

💡 Run `airis doctor --fix` to auto-repair

$ airis doctor --fix
🔧 Fixing...
✨ Workspace healed successfully!
```

Since `manifest.toml` is the single source of truth, all derived files can be regenerated instantly. LLMs can't permanently break your config.

### 6. Production-Grade Build Engine (v1.35+)

```bash
# Parallel DAG-based build (respects dependencies)
airis build --affected --docker -j 8

# Output:
🚀 Starting parallel build (5 tasks, 8 workers)
  ✅ [1/5] libs/ui (1234ms)
  ✅ [2/5] libs/api-client (890ms)
  ✅ [3/5] apps/web (4521ms)       # waited for libs/ui
  ✅ [4/5] apps/api (3200ms)       # waited for libs/api-client
  ✅ [5/5] apps/worker (2100ms)

✅ All 5 tasks completed successfully (12945ms total)
```

**Build System Features:**

| Feature | Description |
|---------|-------------|
| **Hermetic Builds** | Docker-isolated, reproducible across environments |
| **BLAKE3 Cache** | Content-addressable cache at `~/.airis/.cache/` |
| **Remote Cache** | S3 (`s3://bucket`) or OCI (`oci://registry`) |
| **Parallel DAG** | Dependency-aware parallel execution |
| **Multi-Target** | Build for node, edge, bun, deno simultaneously |
| **Channel Resolver** | `lts`, `current`, `edge`, `bun`, `deno` → Docker images |

### 7. Homebrew Distribution with Auto-Release

```bash
$ git push origin main
# GitHub Actions automatically:
# 1. Determines version from commits (feat: → minor, fix: → patch)
# 2. Builds release binary
# 3. Creates GitHub release
# 4. Updates Homebrew formula
```

Your users just run `brew upgrade airis-workspace` and get the latest version. Zero manual release work.

---

## 🧠 Design Philosophy: Why Not NX/Turborepo/Bazel?

**airis-workspace is not competing with these tools. It's built for a different era.**

### Different Eras, Different Tools

| Tool | Era | Philosophy |
|------|-----|------------|
| **NX/Turborepo/Bazel** | Human-managed monorepo | Humans manually maintain complex config files |
| **airis** | LLM-managed monorepo | AI auto-generates and self-repairs everything |

### NX's Problems

**1. Plugin Hell**

NX relies heavily on plugins (`@nx/react`, `@nx/next`, etc.). If the official plugin isn't updated for your framework version, you're stuck. I've wasted days waiting for plugin updates.

**2. Config File Sprawl**

```
workspace.json
nx.json
project.json × N projects
.nxignore
various .rc files
```

LLMs will break these. There's no single source of truth. When Claude edits `project.json` incorrectly, you have to manually fix it.

**3. Dependency Graph is Less Useful in LLM Era**

NX's dependency graph visualization was great when humans needed to understand impact. But with LLMs doing the analysis, we don't need fancy UIs—we need auto-regeneration.

### Turborepo's Problems

**1. No Version Catalog**

You still manually update versions in every `package.json`. No `"latest"` → `"^19.2.0"` resolution.

**2. No Docker-First Enforcement**

Nothing stops LLMs from running `pnpm install` on your host.

### Bazel's Problems

**1. Massive Learning Curve**

BUILD files, Starlark, etc. Overkill for most monorepos.

**2. Not LLM-Friendly**

LLMs struggle with Bazel's unique syntax and concepts.

### airis's Solution: Self-Healing Monorepo

```
manifest.toml (Single Source of Truth)
    ↓ airis init
Everything else (auto-generated)
```

- **LLM breaks package.json?** → `airis init` regenerates it
- **LLM runs `pnpm install` on host?** → Guards block it with helpful error
- **Version conflicts?** → Catalog auto-resolves from npm registry

**This is "break-proof" monorepo design for the AI age.**

### What NX/Turbo Has (That airis Also Has)

- **Affected dependency graph** - `airis affected` analyzes git changes and shows impacted packages
- Transitive dependency tracking (if A depends on B, changing B marks A as affected)

### What NX/Turbo Has (That airis NOW Has Too!)

- ✅ **Distributed build cache** - S3 and OCI registry support
- ✅ **Remote cache sharing** - `--remote-cache s3://bucket` or `oci://registry`
- ✅ **Parallel DAG builds** - `--parallel` or `-j` flag
- ✅ **Affected-only builds** - `--affected` flag
- ✅ **Multi-target builds** - `--targets node,edge,bun`

**airis v1.40+ has feature parity with NX/Turborepo, plus Docker-first hermetic builds.**

### Example: Affected Analysis

```bash
$ airis affected
🔍 Analyzing affected packages...
  📝 Changed files: 12
  📦 Packages found: 35
  🎯 Directly changed: 3

📊 Affected packages:
   - @agiletec/ui
   - @airis/dashboard      # depends on @agiletec/ui
   - @airis/voice-gateway  # depends on @agiletec/ui
```

### Production-Grade Build System (v1.35+)

```bash
# Hermetic Docker build with channel resolver
airis build apps/web --docker --channel lts

# Build only affected projects in parallel
airis build --affected --docker -j 8

# Multi-target build (node + edge + bun simultaneously)
airis build apps/api --docker --targets node,edge,bun

# With remote cache (S3 or OCI registry)
airis build --affected --docker --remote-cache s3://my-bucket/cache

# Generate deployment bundle
airis bundle apps/api
# → dist/api/bundle.json, image.tar, artifact.tar.gz

# Policy gates (pre-deployment validation)
airis policy check
airis policy enforce
```

**airis is not a NX/Turbo alternative. It's the monorepo OS for the LLM era.**

---

## 🚀 Quick Start

### Install (Recommended: Homebrew)

```bash
# Install airis-workspace
brew install agiletec-inc/tap/airis-workspace

# Optional: Install AIRIS MCP Gateway for Claude Code integration
brew install agiletec-inc/tap/airis-mcp-gateway
```

**Note**: airis-workspace requires Docker. Install [OrbStack](https://orbstack.dev) (Apple Silicon) or [Docker Desktop](https://www.docker.com/products/docker-desktop/) (Intel).

### Install (Alternative: Cargo)

For developers who want to build from source:

```bash
git clone https://github.com/agiletec-inc/airis-workspace.git
cd airis-workspace
cargo install --path .
```

### Create New Workspace

```bash
mkdir my-monorepo && cd my-monorepo
airis init --write  # Creates manifest.toml template
# Edit manifest.toml to configure your workspace
airis generate files  # Generates package.json, pnpm-workspace.yaml, docker-compose.yml
airis up            # Start Docker services
```

### Migrate Existing Project (Auto-Discovery)

```bash
cd your-existing-monorepo
airis init          # Auto-discovers apps, libs, compose files (dry-run)
airis init --write  # Executes migration and generates manifest.toml
airis generate files  # Generates all workspace files
airis up            # Start everything
```

**What `airis init` does (v1.43+)**:
1. **Discovery Phase**: Scans apps/, libs/ for projects (Next.js, Vite, Hono, Rust, Python)
2. **Compose Detection**: Finds docker-compose.yml files (root, workspace/, supabase/, traefik/)
3. **Catalog Extraction**: Reads existing package.json devDependencies
4. **Migration Plan**: Shows what will be created/moved
5. With `--write`: Executes migration and generates manifest.toml
6. **Never overwrites existing manifest.toml**

```bash
$ airis init
🔍 Discovering project structure...

📦 Detected Apps:
   apps/web          nextjs      (has Dockerfile)
   apps/api          hono        (has Dockerfile)

📚 Detected Libraries:
   libs/ui           TypeScript

🐳 Docker Compose Files:
   ./docker-compose.yml        → workspace/docker-compose.yml

📄 Migration Plan:
   1. Create directory: workspace
   2. Move docker-compose.yml → workspace/docker-compose.yml
   3. Generate manifest.toml

Run `airis init --write` to execute this plan.
```

**Options**:
- `--skip-discovery`: Use empty template instead of auto-detection

**What `airis generate files` does**:
1. Reads manifest.toml
2. Resolves catalog version policies ("latest" → "^19.2.0") from npm registry
3. Generates package.json, pnpm-workspace.yaml, docker-compose.yml

**New in v1.0.2**: All operations now via `airis` commands. No `just` dependency required.

---

## 📁 File Structure

```
my-monorepo/
├── manifest.toml         # ✅ SINGLE SOURCE OF TRUTH (EDIT THIS)
├── package.json          # ❌ Auto-generated (DO NOT EDIT)
├── pnpm-workspace.yaml   # ❌ Auto-generated (DO NOT EDIT)
├── docker-compose.yml    # ❌ Auto-generated (DO NOT EDIT)
├── Cargo.toml            # ❌ Auto-generated (DO NOT EDIT) - for Rust projects
├── apps/
│   ├── dashboard/
│   │   └── package.json  # Resolved versions: "react": "^19.2.0"
│   └── api/
│       └── package.json
└── libs/
    ├── ui/
    └── db/
```

**Philosophy**: Edit `manifest.toml` → Run `airis init` → Everything else regenerates

---

## 💡 Core Concepts

### 1. Single Manifest (`manifest.toml`)

```toml
[workspace]
name = "my-monorepo"
package_manager = "pnpm@10.22.0"
service = "workspace"
image = "node:22-alpine"

# Version catalog with auto-resolution policies
[packages.catalog]
react = "latest"      # → airis init resolves to ^19.2.0
next = "lts"          # → resolves to LTS version
typescript = "^5.0.0" # → used as-is

[dev]
autostart = ["dashboard", "api"]

[apps.dashboard]
path = "apps/dashboard"
type = "nextjs"

[service.postgres]
image = "postgres:16-alpine"

[commands]
install = "docker compose exec workspace pnpm install"
dev = "docker compose exec workspace pnpm dev"
build = "docker compose exec workspace pnpm build"
test = "docker compose exec workspace pnpm test"
lint = "docker compose exec workspace pnpm lint"

[guards]
deny = ["npm", "yarn", "pnpm", "bun"]
forbid = ["npm", "yarn", "pnpm", "docker", "docker-compose"]

[remap]
"npm install" = "airis install"
"pnpm install" = "airis install"

[versioning]
strategy = "conventional-commits"
source = "1.1.0"
```

### 2. Version Policy Resolution

```toml
# In manifest.toml
[packages.catalog]
react = "latest"
next = "lts"
typescript = "^5.6.0"
```

Run `airis init` (or `airis generate files`):
```bash
📦 Resolving catalog versions from npm registry...
  ✓ react latest → ^19.2.0
  ✓ next lts → ^16.0.3
  ✓ typescript ^5.6.0 → ^5.6.0
📝 Updated pnpm-workspace.yaml
✅ Workspace files generated!
```

> **Note**: `airis sync-deps` is deprecated. Version resolution is now integrated into `airis init`.

Result in `pnpm-workspace.yaml`:
```yaml
catalog:
  react: ^19.2.0
  next: ^16.0.3
  typescript: ^5.6.0
```

**You never manually update version numbers again.**

### 3. Docker-First Enforcement (v1.0.2+)

```bash
$ pnpm install
❌ ERROR: 'pnpm' must run inside Docker workspace

   Use: airis install

   Or configure [remap] in manifest.toml to auto-translate commands.
```

**Guard System**:
- `[guards.deny]`: Block commands for all users
- `[guards.forbid]`: LLM-specific blocking
- `[remap]`: Auto-translate banned commands to safe alternatives

### 4. Version Automation (v1.1.0+)

```bash
# Edit code, then commit
git commit -m "feat: add dark mode support"

# Pre-commit hook auto-bumps version
🔄 Auto-bumping version...
🚀 Bumping version: 1.0.2 → 1.1.0
✅ Version auto-bumped

# manifest.toml and Cargo.toml updated automatically
```

**Conventional Commits Support**:
- `feat:` → minor bump (1.0.0 → 1.1.0)
- `fix:` → patch bump (1.0.0 → 1.0.1)
- `BREAKING CHANGE` → major bump (1.0.0 → 2.0.0)

---

## 🛠️ Commands

### Workspace Management
```bash
airis init                    # Auto-discover & create manifest.toml (dry-run)
airis init --write            # Execute discovery and create manifest.toml
airis init --skip-discovery   # Use empty template (legacy mode)
airis generate files          # Regenerate workspace files from manifest.toml
airis doctor                  # Diagnose workspace health, detect config drift
airis doctor --fix            # Auto-repair detected issues
airis validate                # Check configuration
airis guards install          # Install command guards to block host package managers
airis new <type> <name>       # Scaffold new app/lib (api, web, lib)
```

### Development (v1.0.2+)
```bash
airis up                # Start Docker services
airis install           # Install deps (in Docker)
airis shell             # Enter container shell
airis dev               # Start development servers
airis build             # Build project
airis test              # Run tests
airis clean             # Clean artifacts
airis down              # Stop services
```

### Hermetic Docker Build (v1.35+)
```bash
# Single project build with channel
airis build apps/web --docker --channel lts

# Build affected projects only
airis build --affected --docker

# Parallel build (default: CPU count)
airis build --affected --docker -j 8

# Multi-target build
airis build apps/api --docker --targets node,edge,bun

# With remote cache
airis build --affected --docker --remote-cache s3://bucket/cache
airis build --affected --docker --remote-cache oci://ghcr.io/org/cache
```

### Bundle & Deploy (v1.38+)
```bash
airis bundle apps/api              # Generate deployment package
airis bundle apps/api -o ./release # Custom output directory
```

Output:
```
dist/api/
├── bundle.json      # Metadata (version, hash, deps, git SHA)
├── image.tar        # Docker image (docker save)
└── artifact.tar.gz  # Build artifacts (.next/standalone, dist/)
```

### Policy Gates (v1.39+)
```bash
airis policy init      # Create .airis/policies.toml
airis policy check     # Run validation checks
airis policy enforce   # Fail on violations
```

Configuration (`.airis/policies.toml`):
```toml
[gates]
require_clean_git = true
require_env = ["DATABASE_URL", "API_KEY"]
forbid_files = [".env.local", "secrets.json"]
forbid_patterns = ["**/*.secret"]

[security]
scan_secrets = true
```

### Custom Commands
```bash
airis run <task>        # Run任意のコマンド from manifest.toml [commands]
```

### Version Management (v1.1.0+)
```bash
airis bump-version --major    # Bump major version (1.0.0 → 2.0.0)
airis bump-version --minor    # Bump minor version (1.0.0 → 1.1.0)
airis bump-version --patch    # Bump patch version (1.0.0 → 1.0.1)
airis bump-version --auto     # Auto-detect from commit message (Conventional Commits)
airis hooks install           # Install Git pre-commit hook for auto-versioning
```

### Query Manifest
```bash
airis manifest dev-apps  # List autostart apps
airis manifest rule verify  # Get verify commands
```

---

## 🎨 Roadmap & Implementation Status

### ✅ Phase 1: Foundation (v0.1.0 - v0.2.1) - COMPLETED

- [x] Rust CLI skeleton with clap
- [x] Manifest-driven template engine (Handlebars)
- [x] `airis init` - manifest.toml creation & re-sync
- [x] manifest.toml immutability enforcement (no `--force` flag)
- [x] package.json generation
- [x] Cargo.toml generation (for Rust projects)
- [x] pnpm-workspace.yaml generation
- [x] docker-compose.yml generation
- [x] Project discovery (auto-detect apps/libs)
- [x] Command guards (block host-level pnpm/npm/yarn)

**Status**: ✅ Core workflow functional

---

### ✅ Phase 1.5: Command Unification (v1.0.2) - COMPLETED

- [x] `[commands]` section in manifest.toml
- [x] `airis run <task>` for custom commands
- [x] Built-in shorthands (up, down, shell, dev, test, install, build, clean)
- [x] `[guards]` section (deny, forbid, danger)
- [x] `[remap]` section for command translation
- [x] Eliminate just dependency

**Status**: ✅ Just is now optional, all operations via `airis` CLI

---

### ✅ Phase 1.6: Version Automation (v1.1.0) - COMPLETED

- [x] `[versioning]` section in manifest.toml
- [x] `airis bump-version` command (--major, --minor, --patch, --auto)
- [x] Conventional Commits support
- [x] `airis hooks install` for Git pre-commit hook
- [x] Auto-bump on commit
- [x] Sync manifest.toml ↔ Cargo.toml

**Status**: ✅ Fully automated version management

---

### ✅ Phase 2: Catalog Version Policy (v0.3.0) - COMPLETED

- [x] CatalogEntry enum (Policy | Version)
- [x] npm registry client for version resolution
- [x] Version resolution integrated into `airis init`
- [x] Support for "latest" policy
- [x] Support for "lts" policy
- [x] Support for semver (^X.Y.Z) passthrough
- [x] Auto-update pnpm-workspace.yaml catalog

**Status**: ✅ Version policies fully functional

**Usage**:
```bash
# Edit manifest.toml
[packages.catalog]
react = "latest"

# Resolve to actual versions (integrated into init)
airis init
# or
airis generate files

# Result: pnpm-workspace.yaml updated with ^19.2.0
```

---

### 🚧 Phase 3: Smart Generation & Orchestration (v0.4.0) - IN PROGRESS

**Goal**: Full package.json generation from manifest, multi-compose orchestration

#### 3.1 Package.json Full Generation
- [ ] Generate individual app package.json files
- [ ] Project-level scripts definition in manifest
  ```toml
  [[project]]
  name = "corporate-site"
  [project.scripts]
  dev = "vite dev"
  build = "vite build"
  [project.deps]
  react = "catalog"
  next = "catalog"
  ```
- [ ] Auto-inject catalog references (`"react": "catalog:"`)
- [ ] Sync scripts from manifest to package.json
- [ ] Workspace-level vs app-level dependency resolution

#### 3.2 Multi-Compose Orchestration
- [ ] Parse `[orchestration.dev]` section
- [ ] Support multiple docker-compose.yml files
  ```toml
  [orchestration.dev]
  workspace = "workspace/docker-compose.yml"
  supabase = ["supabase/docker-compose.yml", "supabase/docker-compose.override.yml"]
  traefik = "traefik/docker-compose.yml"
  ```
- [ ] Generate unified `airis up` that starts all compose stacks
- [ ] Dependency ordering (start supabase before workspace)

**Current Status**: 🟡 Schema defined, implementation pending

---

### 📋 Phase 4: Validation & Safety (v0.5.0) - PLANNED

#### 4.1 Configuration Validation
- [ ] `airis validate` command
- [ ] Check manifest.toml syntax
- [ ] Validate app paths exist
- [ ] Validate port conflicts
- [ ] Validate catalog references in package.json

#### 4.2 Environment Variable Validation
- [ ] Define required env vars in manifest
  ```toml
  [env]
  required = ["DATABASE_URL", "API_KEY"]
  optional = ["SENTRY_DSN"]

  [env.validation.DATABASE_URL]
  pattern = "^postgresql://"
  description = "PostgreSQL connection string"
  ```
- [ ] Runtime validation before `airis up`
- [ ] Auto-generate `.env.example`

#### 4.3 Drift Detection
- [ ] `airis doctor` command
- [ ] Detect manual edits to generated files
- [ ] Suggest re-running `airis init`
- [ ] Warn if pnpm-workspace.yaml catalog diverges from manifest

**Priority**: High (prevents runtime errors)

---

### 📋 Phase 5: LLM Integration (v0.6.0) - PLANNED

**Goal**: Make airis-workspace the ultimate LLM-friendly monorepo tool

#### 5.1 LLM Context Generation
- [ ] Generate `.workspace/llm-context.md` from manifest
- [ ] Include project structure, available commands, rules
- [ ] Auto-update on `airis init`

#### 5.2 Error Message Optimization
- [ ] Structured error output (JSON mode for LLMs)
- [ ] Actionable fix suggestions
- [ ] Link to relevant manifest sections

**Note**: MCP server integration is handled by [airis-mcp-gateway](https://github.com/agiletec-inc/airis-mcp-gateway). This repo focuses solely on monorepo management.

**Status**: Design phase

---

### ✅ Phase 6: Migration & Auto-Discovery (v1.43) - COMPLETED

**Goal**: Zero-friction migration from existing projects

#### 6.1 Enhanced Discovery
- [x] Detect Next.js/Vite/Hono/Node app types
- [x] Detect Rust/Python projects
- [x] Parse existing package.json catalog (devDependencies)
- [x] Detect compose file locations (root, workspace/, supabase/, traefik/)

#### 6.2 Safe Migration
- [x] Move docker-compose.yml to workspace/
- [x] Create backups (.bak) before moving
- [x] Never overwrite existing files
- [x] Dry-run mode by default (`airis init`)
- [x] Execute with `--write` flag

#### 6.3 Template Mode
- [x] `--skip-discovery` flag for empty template

**Status**: ✅ Auto-discovery and safe migration fully functional

---

### 📋 Phase 7: Advanced Features (v0.8.0+) - FUTURE

#### 7.1 Monorepo Modes
- [ ] `strict` mode - no host execution at all
- [ ] `hybrid` mode - allow some host tools (Rust, Python)
- [ ] Custom mode definitions

#### 7.2 CI/CD Integration
- [ ] GitHub Actions template generation
- [ ] GitLab CI template generation
- [ ] Vercel/Netlify config generation

#### 7.3 Performance
- [ ] Parallel npm queries in version resolution
- [ ] Cache npm registry responses
- [ ] Incremental generation (only changed files)

---

## 📊 Current Status Summary

| Phase | Status | Version | Key Features |
|-------|--------|---------|--------------|
| 1. Foundation | ✅ Done | v0.2.1 | init, generate, guards |
| 1.5 Command Unification | ✅ Done | v1.0.2 | airis commands, guards, remap |
| 1.6 Version Automation | ✅ Done | v1.1.0 | bump-version, hooks, auto-bump |
| 2. Catalog Policies | ✅ Done | v0.3.0 | latest/lts resolution (in init) |
| **3. Hermetic Build** | ✅ Done | **v1.35** | Docker build, channel resolver, BLAKE3 cache |
| **4. Remote Cache** | ✅ Done | **v1.37** | S3/OCI remote cache, cache hit/miss |
| **5. Bundle & Deploy** | ✅ Done | **v1.38** | bundle.json, image.tar, artifact.tar.gz |
| **6. Policy Gates** | ✅ Done | **v1.39** | Git clean, env check, secret scan |
| **7. Multi-Target** | ✅ Done | **v1.40** | --targets node,edge,bun |
| **8. Parallel Build** | ✅ Done | **v1.41** | DAG-based parallel execution, -j flag |
| **9. LTS Resolution** | ✅ Done | **v1.42** | npm dist-tags for proper LTS versions |
| **10. Auto-Migration** | ✅ Done | **v1.43** | discover apps/libs, safe migration |
| 11. K8s Manifests | 📋 Planned | v1.44+ | deployment.yaml, service.yaml generation |
| 12. Build Matrix | 🔮 Future | v1.50+ | linux/amd64, linux/arm64 cross-build |

---

## 🎯 Next Steps (What to Work On)

### Immediate (v1.44+)

1. **Kubernetes Manifest Generation**
   - `airis bundle --k8s` generates deployment.yaml, service.yaml, ingress.yaml
   - Helm chart generation
   - ConfigMap from env files

2. **Build Matrix**
   - `--platforms linux/amd64,linux/arm64`
   - Cross-compilation support
   - OCI multi-arch images

### Future

1. **Performance Optimization**
   - Build graph visualization
   - Distributed workers
   - Incremental builds

---

## 📖 Documentation

- [Airis Commands Usage](docs/airis-commands.md) - Complete command reference
- [Airis Init Architecture](docs/airis-init-architecture.md) - How `airis init` works (READ-ONLY mode, LLM integration)
- [Quick Start](docs/QUICKSTART.md) (planned)
- [Migration Guide](docs/MIGRATION.md) - Existing project → airis (planned)
- [Configuration Reference](docs/CONFIG.md) (planned)
- [LLM Integration](docs/LLM.md) (planned)

---

## 🤝 Contributing

We're in active development! Contributions welcome:

1. Fork the repo
2. Create feature branch (`git checkout -b feature/amazing`)
3. Commit changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing`)
5. Create Pull Request

**Priority areas**:
- Package.json generation (Phase 3)
- Multi-compose orchestration (Phase 3)
- Validation tools (Phase 4)

---

## 📄 License

MIT License - see [LICENSE](LICENSE)

---

## 💬 Author

[@agiletec-inc](https://github.com/agiletec-inc)

Born from frustration with LLMs breaking Docker-first rules repeatedly.
Hope it helps developers building rapid prototypes with monorepos.

---

## ☕ Support This Project

If airis-workspace saves you time or makes your workflow smoother, consider supporting its development:

[![Buy Me A Coffee](https://img.shields.io/badge/Buy%20Me%20A%20Coffee-support-yellow?style=for-the-badge&logo=buy-me-a-coffee)](https://buymeacoffee.com/kazukinakad)

Your support helps maintain and improve this tool. Thank you! 🙏

---

## 🔗 Related Projects

- [pnpm](https://pnpm.io) - Fast package manager with catalog support
- [OrbStack](https://orbstack.dev) - Fast Docker for macOS
