# AIris Workspace

**Docker-first monorepo workspace manager for rapid prototyping**

Stop fighting with dependencies, broken builds, and cross-platform issues. AIris Workspace enforces Docker-first development with a single manifest file and automatic Just/package.json generation.

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
# Option 1: Install airis-agent plugin (recommended for Claude Code users)
/plugin marketplace add agiletec-inc/airis-agent
/plugin install airis-agent

# Option 2: Clone all AIRIS repositories at once
uv run airis-agent install-suite --profile core

# Option 3: Just use airis-workspace standalone
cargo install airis
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
- **Docker-first enforced**: `just pnpm` → Error with helpful message
- **Single source of truth**: `manifest.toml` → auto-generate everything
- **Auto-version resolution**: `react = "latest"` → automatically resolves to `^19.2.0`
- **LLM-friendly**: Clear error messages, MCP server integration
- **Cross-platform**: macOS/Linux/Windows via Docker
- **Rust special case**: Local builds for Apple Silicon GPU support

---

## 🚀 Quick Start

### Install Just (if not installed)
```bash
# macOS
brew install just

# Linux
curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash

# Windows
scoop install just
```

### Install AIris Workspace
```bash
# From source (development)
git clone https://github.com/agiletec-inc/airis-workspace.git
cd airis-workspace
cargo install --path .

# Or install from crates.io (when published)
cargo install airis
```

### Create New Workspace
```bash
mkdir my-monorepo && cd my-monorepo
airis init          # Creates manifest.toml + derived files
airis sync-deps     # Resolve "latest" policies to actual versions
just up
```

### Migrate Existing Project
```bash
cd your-existing-monorepo
airis init          # Auto-detects apps/libs/compose files, generates manifest.toml
                    # Safely moves files to correct locations (no overwrites)
airis sync-deps     # Update catalog with latest versions
just up
```

**What `airis init` does for existing projects**:
1. Scans `apps/` and `libs/` directories
2. Detects docker-compose.yml locations
3. Generates `manifest.toml` with detected configuration
4. Generates justfile, package.json, pnpm-workspace.yaml
5. **Never overwrites existing manifest.toml** (read-only after creation)

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
│   │   └── package.json  # References catalog: "react": "catalog:"
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
port = 3000

[service.postgres]
image = "postgres:16-alpine"
port = 5432

[rule.verify]
commands = ["just lint", "just test-all"]
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

### 3. Docker-First Enforcement

```bash
$ just pnpm install
❌ ERROR: 'pnpm' must run inside Docker workspace

   To run pnpm:
     1. Enter workspace: just workspace
     2. Run command:     pnpm install
```

### 4. Just > Make

- ✅ No tab hell
- ✅ Cross-platform (Windows works!)
- ✅ Natural variable syntax: `{{project}}`
- ✅ LLM-friendly (simple syntax)
- ✅ Rust-powered (fast)

---

## 🛠️ Commands

### Workspace Management
```bash
airis init              # Create or re-sync derived files from manifest.toml
airis sync-deps         # Resolve "latest"/"lts" policies to actual versions
airis validate          # Check configuration (planned)
airis guards install    # Install command guards to block host package managers
```

### Development (via Just)
```bash
just up                 # Start Docker services
just install            # Install deps (in Docker)
just workspace          # Enter container shell
just dev-all            # Start all autostart apps
just build              # Build project
just test               # Run tests
just clean              # Clean artifacts
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
