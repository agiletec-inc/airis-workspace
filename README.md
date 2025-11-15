# AIris Workspace

**Docker-first monorepo workspace manager for rapid prototyping**

Stop fighting with dependencies, broken builds, and cross-platform issues. AIris Workspace enforces Docker-first development with a single manifest file and automatic Just/package.json generation.

---

## 🎯 Problem Solved

### ❌ Before
- LLMs break Docker-first rules by running `pnpm install` on host
- Dependency version conflicts across apps
- `.env.local` / `.env.development` proliferation
- Manual Makefile maintenance
- TypeScript build issues on different machines
- "Works on my machine" syndrome

### ✅ After
- **Docker-first enforced**: `just pnpm` → Error with helpful message
- **Single source of truth**: `MANIFEST.toml` → auto-generate everything
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
airis init          # Creates MANIFEST.toml + derived files
just up
```

### Migrate Existing Project
```bash
cd your-existing-monorepo
airis init          # Auto-detects apps/libs/compose files, generates manifest.toml
                    # Safely moves files to correct locations (no overwrites)
just up
```

**What `airis init` does for existing projects**:
1. Scans `apps/` and `libs/` directories
2. Detects docker-compose.yml locations
3. Generates `manifest.toml` with detected configuration
4. Moves files to optimal locations (creates backups, never overwrites)
5. Generates justfile, package.json, etc.

---

## 📁 File Structure

```
my-monorepo/
├── MANIFEST.toml         # Single source of truth
├── workspace.yaml        # Auto-generated metadata
├── justfile              # Auto-generated (DO NOT EDIT)
├── package.json          # Auto-generated (DO NOT EDIT)
├── pnpm-workspace.yaml   # Auto-generated
├── docker-compose.yml    # Auto-generated
├── apps/
│   ├── dashboard/
│   │   └── package.json  # References catalog: "react": "catalog:"
│   └── api/
│       └── package.json
└── libs/
    ├── ui/
    └── db/
```

---

## 💡 Core Concepts

### 1. Single Manifest (`MANIFEST.toml`)

```yaml
[workspace]
name = "my-monorepo"
package_manager = "pnpm@10.22.0"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"
volumes = ["workspace-node-modules:/app/node_modules"]

[dev]
apps = ["dashboard", "duplicate-finder"]
depends_on = ["supabase"]

[service.supabase]
image = "supabase/postgres"
port = 5432

[rule.verify]
commands = ["just lint", "just test-all"]
```

### 2. Docker-First Enforcement

```bash
$ just pnpm install
❌ ERROR: 'pnpm' must run inside Docker workspace

   To run pnpm:
     1. Enter workspace: just workspace
     2. Run command:     pnpm install
```

### 3. Just > Make

- ✅ No tab hell
- ✅ Cross-platform (Windows works!)
- ✅ Natural variable syntax: `{{project}}`
- ✅ LLM-friendly (simple syntax)
- ✅ Rust-powered (fast)

---

## 🛠️ Commands

### Workspace Management
```bash
airis init              # Create or optimize MANIFEST + derived files
airis validate          # Check configuration
airis doctor            # Diagnose environment
```

### Development (via Just)
```bash
just up                 # Start Docker services
just install            # Install deps (in Docker)
just workspace          # Enter container shell
just build              # Build project
just test               # Run tests
just clean              # Clean artifacts
```

### Special Cases
```bash
airis build duplicate-finder       # Auto-detects local build (GPU)
airis build duplicate-finder --docker  # Force Docker build (no GPU)
```

---

## 🎨 Features

### ✅ Implemented
- [x] Rust CLI skeleton
- [x] Manifest-driven templates
- [x] Example MANIFEST.toml
- [x] `airis init` (create + re-sync derived files)

### 🚧 In Progress
- [ ] `airis validate` (config validation)

### 📋 Planned
- [ ] Environment variable validation
- [ ] LLM context generation
- [ ] MCP server integration
- [ ] Migration from existing projects

---

## 📖 Documentation

- [Quick Start](docs/QUICKSTART.md)
- [Migration Guide](docs/MIGRATION.md) - Makefile → Just
- [Configuration Reference](docs/CONFIG.md)
- [LLM Integration](docs/LLM.md)

---

## 🤝 Contributing

We're in early development! Contributions welcome:

1. Fork the repo
2. Create feature branch (`git checkout -b feature/amazing`)
3. Commit changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing`)
5. Create Pull Request

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
