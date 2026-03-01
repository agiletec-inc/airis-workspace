# 🛡️ AIris Monorepo

**Stop LLMs from polluting your host environment**

Claude Code ran `pnpm install` on your host? Never again.

- 🔒 **Global guard**: Block `npm/pnpm/yarn` outside Docker
- 🔄 **Self-healing**: LLM broke package.json? `airis init` regenerates it
- 📦 **Single source of truth**: manifest.toml → everything else auto-generated

> **NX and Turborepo are build tools. airis is a guard rail for the AI coding era.**
>
> Use them together for maximum safety.

![airis init generates your entire monorepo config](assets/airis-init-demo.gif)

---

## 🤔 Why airis Exists

### The Problem: LLMs Break Your Environment

You're coding with Claude Code or Cursor. Things are going great. Then:

```bash
# Claude says: "I'll install the dependencies for you"
$ pnpm install
# 200 packages installed to your HOST machine
# node_modules now polluting your local environment
```

Or worse:

```bash
# Claude edits package.json directly
# Now your versions don't match your teammates
# CI fails, debugging begins...
```

**This happens constantly.** LLMs don't understand Docker-first workflows. They just run commands.

### The Solution: Guard Rails + Self-Healing

```bash
$ pnpm install
❌ ERROR: 'pnpm' must run inside Docker workspace

   Use: airis install

   Or configure [remap] in manifest.toml to auto-translate commands.
```

When Claude tries to run `pnpm install`, it gets blocked with a helpful error. **Your host stays clean.**

LLM broke your package.json? No problem:

```bash
$ airis init
✨ Regenerated package.json from manifest.toml
```

Since `manifest.toml` is the single source of truth, all derived files can be regenerated instantly.

---

## 🛡️ How airis Protects You

### 1. Command Guards

Block dangerous commands on your host:

```toml
# manifest.toml
[guards]
deny = ["npm", "yarn", "pnpm", "bun"]  # Block for everyone
forbid = ["docker compose down -v"]    # Block destructive commands
```

```bash
$ npm install
❌ BLOCKED: 'npm' is not allowed on host
   Use: airis install
```

### 2. Command Remapping

Auto-translate LLM commands to safe alternatives:

```toml
# manifest.toml
[remap]
"npm install" = "airis install"
"pnpm install" = "airis install"
"yarn add" = "airis shell"  # Opens container shell
```

When Claude runs `npm install`, it automatically becomes `airis install` (which runs inside Docker).

### 3. Self-Healing Config

All files are generated from `manifest.toml`:

```
manifest.toml (edit this only)
    ↓ airis init
package.json         ← auto-generated
pnpm-workspace.yaml  ← auto-generated
docker-compose.yml   ← auto-generated
```

LLM corrupted a file? Just regenerate:

```bash
$ airis doctor --fix
🔧 Fixing...
✨ Workspace healed successfully!
```

### 4. Version Catalog

No more "it works with latest but breaks in CI":

```toml
# manifest.toml
[packages.catalog]
react = "latest"      # → resolves to ^19.2.0
next = "lts"          # → resolves to LTS version
typescript = "^5.0"   # → used as-is
```

Real versions are resolved from npm and written to `pnpm-workspace.yaml`. Everyone gets the same versions.

---

## 🤝 Use with NX / Turborepo / Bazel

**airis is not a replacement. It's a layer that works with your existing tools.**

| Tool | What it does | Use with airis? |
|------|--------------|-----------------|
| **NX** | Build orchestration, dependency graph | ✅ airis guards + NX builds |
| **Turborepo** | Fast task execution, caching | ✅ airis guards + Turbo caching |
| **Bazel** | Hermetic builds at scale | ✅ airis guards + Bazel builds |

### Example: airis + Turborepo

```toml
# manifest.toml
[commands]
build = "docker compose exec workspace pnpm turbo run build"
test = "docker compose exec workspace pnpm turbo run test"

[guards]
deny = ["npm", "yarn", "pnpm"]  # Block host execution
```

Turborepo handles caching and orchestration. airis ensures everything runs inside Docker.

---

## 🚀 Quick Start

### Install

```bash
# One-line install
curl -fsSL https://raw.githubusercontent.com/agiletec-inc/airis-mcp-gateway/main/scripts/quick-install.sh | bash
```

Or build from source:

```bash
cargo install --git https://github.com/agiletec-inc/airis-monorepo
```

### New Project

```bash
mkdir my-monorepo && cd my-monorepo
airis init --write      # Creates manifest.toml
airis generate files    # Generates all config files
airis up                # Start Docker services
```

### Existing Project

```bash
cd your-monorepo
airis init              # Auto-discovers apps, libs, compose files (dry-run)
airis init --write      # Executes migration
airis generate files    # Generates workspace files
airis up                # Start everything
```

**What happens:**
1. Discovers apps in `apps/`, libs in `libs/`
2. Detects frameworks (Next.js, Vite, Hono, Rust, Python)
3. Finds docker-compose.yml files
4. Generates manifest.toml as single source of truth
5. Never overwrites existing manifest.toml

---

## ✨ Key Features

### Auto Version Resolution

```toml
[packages.catalog]
react = "latest"   # → ^19.2.0
next = "lts"       # → LTS version from npm dist-tags
```

No more manually updating 20 package.json files.

### Docker-First by Default

```toml
[apps.api]
runtime = "docker"        # Default

[apps.ml-inference]
runtime = "local"         # Escape hatch for GPU workloads
```

### Auto Versioning

```bash
$ git commit -m "feat: add dark mode"
# Pre-commit hook auto-bumps: 1.0.0 → 1.1.0
```

### Production Build Engine

```bash
# Parallel DAG-based build
airis build --affected --docker -j 8

# With remote cache
airis build --affected --docker --remote-cache s3://bucket
```

### Policy Gates

```bash
airis policy check    # Validate before deploy
airis policy enforce  # Fail on violations
```

---

## 📁 File Structure

```
my-monorepo/
├── manifest.toml         # ✅ SINGLE SOURCE OF TRUTH (edit this)
├── package.json          # ❌ Auto-generated (DO NOT EDIT)
├── pnpm-workspace.yaml   # ❌ Auto-generated (DO NOT EDIT)
├── docker-compose.yml    # ❌ Auto-generated (DO NOT EDIT)
├── apps/
│   ├── dashboard/
│   └── api/
└── libs/
    ├── ui/
    └── db/
```

**Philosophy**: Edit `manifest.toml` → Run `airis init` → Everything regenerates

---

## 🛠️ Commands

### Workspace
```bash
airis init                # Discover & create manifest (dry-run)
airis init --write        # Execute migration
airis generate files      # Regenerate from manifest
airis doctor              # Check workspace health
airis doctor --fix        # Auto-repair issues
airis guards install      # Install command guards
```

### Development
```bash
airis up        # Start Docker services
airis install   # Install deps (in Docker)
airis dev       # Start dev servers
airis build     # Build project
airis test      # Run tests
airis shell     # Enter container
airis down      # Stop services
```

### Build & Deploy
```bash
airis build --affected --docker    # Build changed projects
airis bundle apps/api              # Generate deployment package
airis policy check                 # Pre-deploy validation
```

---

## 🌟 Part of the AIRIS Ecosystem

| Component | Purpose |
|-----------|---------|
| **[airis-agent](https://github.com/agiletec-inc/airis-agent)** | 🧠 Intelligence layer for all editors |
| **[airis-mcp-gateway](https://github.com/agiletec-inc/airis-mcp-gateway)** | 🚪 Unified MCP proxy (90% token reduction) |
| **[mindbase](https://github.com/agiletec-inc/mindbase)** | 💾 Cross-session memory |
| **airis-monorepo** (this repo) | 🛡️ Docker-first monorepo guardrails |

---

## 📖 Documentation

- [Commands Reference](docs/airis-commands.md)
- [Init Architecture](docs/airis-init-architecture.md)
- [manifest.toml Reference](docs/CONFIG.md) (planned)

---

## 🤝 Contributing

Contributions welcome! Priority areas:
- Guard system improvements
- Multi-compose orchestration
- Validation tools

---

## 📄 License

MIT License - see [LICENSE](LICENSE)

---

## 💬 Author

[@agiletec-inc](https://github.com/agiletec-inc)

Born from frustration with LLMs breaking Docker-first rules repeatedly.

---

## ☕ Support

If airis saves you from LLM-induced environment pollution:

[![Buy Me A Coffee](https://img.shields.io/badge/Buy%20Me%20A%20Coffee-support-yellow?style=for-the-badge&logo=buy-me-a-coffee)](https://buymeacoffee.com/kazukinakad)
