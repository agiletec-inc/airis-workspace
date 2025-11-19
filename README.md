# AIris Workspace

**Docker-first monorepo workspace manager for rapid prototyping**

A blazing-fast CLI built in Rust that enforces Docker-first development with a single manifest file and automatic generation of all derived files.

---

## 💡 Why I Built This

### The Pain Points

Running a monorepo comes with these frustrations:

- **Version hell** - React version in root `package.json` differs from `apps/`. Manually updating everything is tedious
- **Config file sprawl** - `package.json`, `pnpm-workspace.yaml`, `docker-compose.yml`, `justfile`... Which one is the source of truth?
- **LLMs break things** - Claude Code or Cursor says "I ran `pnpm install` for you" and pollutes your host environment
- **"Works on my machine"** - TypeScript builds pass locally but fail on teammates' machines

### The Solution: Single Source of Truth

**Make `manifest.toml` the only config file. Auto-generate everything else.**

```
manifest.toml (edit this only)
    ↓ airis init
package.json, pnpm-workspace.yaml, docker-compose.yml, justfile (all auto-generated)
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
| **[mindbase](https://github.com/kazukinakai/mindbase)** | 💾 Local cross-session memory with semantic search | Developers who want persistent conversation history |
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

## 🎯 Problem Solved

### ❌ Before
- LLMs break Docker-first rules by running `pnpm install` on host
- Dependency version conflicts across apps
- Manual version updates for every package
- `.env.local` / `.env.development` proliferation
- Manual Makefile maintenance
- TypeScript build issues on different machines
- "Works on my machine" syndrome

### ✅ After
- **Docker-first enforced**: `pnpm install` → Blocked with helpful error message
- **Single source of truth**: `manifest.toml` → auto-generate everything
- **Auto-version resolution**: `react = "latest"` → automatically resolves to `^19.2.0`
- **Command unification**: All operations via `airis` CLI (no just dependency)
- **LLM-friendly**: Clear error messages, command guards, MCP server integration
- **Cross-platform**: macOS/Linux/Windows via Docker
- **Rust special case**: Local builds for Apple Silicon GPU support

---

## 🧠 Design Philosophy: Why Not NX/Turborepo?

**airis-workspace is not competing with NX/Turborepo. The design philosophy is fundamentally different.**

### Different Eras, Different Tools

| Tool | Era | Philosophy |
|------|-----|------------|
| **NX/Turborepo** | Human-managed monorepo | Humans manually maintain complex config files |
| **airis** | LLM-managed monorepo | AI auto-generates and self-repairs everything |

### NX/Turbo's Problem: Config File Sprawl

NX/Turbo requires multiple config files:
- `workspace.json`
- `nx.json`
- `project.json` × N projects
- Various `.rc` files

**LLMs will break these.** There's no single source of truth.

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

### What NX/Turbo Has (That airis Doesn't Yet)

- Distributed build cache (for large-scale optimization)
- Remote cache sharing across teams

**But these features assume "humans maintaining a 3-year-old monorepo".**

For AI-assisted development with auto-regeneration, these become less critical.

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

### Future Roadmap

- `airis build --affected` - Build only affected packages
- `airis test --affected` - Test only affected packages
- manifest-driven code generation

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
airis init          # Creates manifest.toml + resolves versions + generates all files
airis up            # Start Docker services
```

### Migrate Existing Project

```bash
cd your-existing-monorepo
airis init          # Auto-detects apps/libs/compose files, generates manifest.toml
                    # Resolves catalog versions and generates all derived files
airis up            # Start everything
```

**What `airis init` does**:
1. Scans `apps/` and `libs/` directories (for existing projects)
2. Detects docker-compose.yml locations
3. Generates `manifest.toml` with detected configuration (first run only)
4. Resolves catalog version policies ("latest" → "^19.2.0") from npm registry
5. Generates package.json, pnpm-workspace.yaml, justfile with resolved versions
6. **Never overwrites existing manifest.toml** (read-only after creation)

**New in v1.0.2**: All operations now via `airis` commands. No `just` dependency required.

---

## 📁 File Structure

```
my-monorepo/
├── manifest.toml         # ✅ SINGLE SOURCE OF TRUTH (EDIT THIS)
├── justfile              # ❌ Auto-generated (DO NOT EDIT)
├── package.json          # ❌ Auto-generated (DO NOT EDIT)
├── pnpm-workspace.yaml   # ❌ Auto-generated (DO NOT EDIT)
├── docker-compose.yml    # ❌ Auto-generated (DO NOT EDIT)
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
react = "latest"      # → airis sync-deps resolves to ^19.2.0
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

Run `airis sync-deps`:
```bash
🔄 Syncing dependencies from manifest.toml...
📦 Found 3 catalog entries
  react latest → ^19.2.0
  next lts → ^16.0.3
  typescript ^5.6.0
📝 Updated pnpm-workspace.yaml
✅ Dependency sync complete!
```

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
airis init              # Create or re-sync derived files from manifest.toml
airis sync-deps         # Resolve "latest"/"lts" policies to actual versions
airis validate          # Check configuration (planned)
airis guards install    # Install command guards to block host package managers
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
- [x] Justfile generation from manifest
- [x] package.json generation
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
- [x] `airis sync-deps` command
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

# Resolve to actual versions
airis sync-deps

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
- [ ] Generate unified `just up` that starts all compose stacks
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
- [ ] Runtime validation before `just up`
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
- [ ] MCP server integration for Claude Code

#### 5.2 Error Message Optimization
- [ ] Structured error output (JSON mode for LLMs)
- [ ] Actionable fix suggestions
- [ ] Link to relevant manifest sections

#### 5.3 MCP Server
- [ ] `airis-workspace` MCP server
- [ ] Tools: `get_manifest`, `sync_deps`, `validate`, `list_apps`
- [ ] Integration with airis-mcp-gateway

**Status**: Design phase

---

### 📋 Phase 6: Migration & Auto-Discovery (v0.7.0) - PLANNED

**Goal**: Zero-friction migration from existing projects

#### 6.1 Enhanced Discovery
- [ ] Detect Next.js/Vite/React app types
- [ ] Detect Rust/Python/Go projects
- [ ] Parse existing package.json catalog
- [ ] Detect compose file locations

#### 6.2 Safe Migration
- [ ] Move docker-compose.yml to workspace/
- [ ] Create backups (.bak) before moving
- [ ] Never overwrite existing files
- [ ] Interactive confirmation mode
- [ ] Dry-run mode (`airis init --dry-run`)

#### 6.3 Wizard Mode
- [ ] Interactive project setup
- [ ] Ask about app types, ports, dependencies
- [ ] Generate optimal manifest.toml

**Status**: Discovery already implemented, migration pending

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
- [ ] Parallel npm queries in sync-deps
- [ ] Cache npm registry responses
- [ ] Incremental generation (only changed files)

---

## 📊 Current Status Summary

| Phase | Status | Version | Key Features |
|-------|--------|---------|--------------|
| 1. Foundation | ✅ Done | v0.2.1 | init, generate, guards |
| 1.5 Command Unification | ✅ Done | v1.0.2 | airis commands, guards, remap |
| 1.6 Version Automation | ✅ Done | v1.1.0 | bump-version, hooks, auto-bump |
| 2. Catalog Policies | ✅ Done | v0.3.0 | sync-deps, latest/lts |
| 3. Smart Generation | 🚧 In Progress | v0.4.0 | Full package.json gen, orchestration |
| 4. Validation | 📋 Planned | v0.5.0 | validate, doctor, env checks |
| 5. LLM Integration | 📋 Planned | v0.6.0 | MCP server, context gen |
| 6. Migration | 📋 Planned | v0.7.0 | Auto-discovery, wizard |
| 7. Advanced | 🔮 Future | v0.8.0+ | CI/CD, modes, perf |

---

## 🎯 Next Steps (What to Work On)

### Immediate (v0.4.0)

1. **Package.json Full Generation**
   - Add `[[project]]` section to manifest schema
   - Implement project-level scripts/deps
   - Generate app-specific package.json files

2. **Multi-Compose Orchestration**
   - Parse `[orchestration.dev]` section
   - Generate unified `just up` command
   - Handle dependency ordering

### After v0.4.0

1. **Validation & Safety** (v0.5.0)
   - Implement `airis validate`
   - Add env var validation
   - Build `airis doctor`

2. **LLM Integration** (v0.6.0)
   - Generate llm-context.md
   - Build MCP server
   - Optimize error messages

---

## 📖 Documentation

- [Quick Start](docs/QUICKSTART.md) (planned)
- [Migration Guide](docs/MIGRATION.md) - Makefile → Just (planned)
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

## 🔗 Related Projects

- [makefile-global](https://github.com/kazukinakai/makefile-global) - Predecessor (Make-based)
- [Just](https://just.systems) - Command runner (Make alternative)
- [pnpm](https://pnpm.io) - Fast package manager with catalog support
